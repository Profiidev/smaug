use std::{sync::Arc, time::Duration};

use axum::{
  Json,
  extract::{
    Path, WebSocketUpgrade,
    ws::{Message, WebSocket},
  },
  response::Response,
};
use centaurus::{backend::config::SiteConfig, bail, db::init::Connection, error::Result};
use dashmap::DashMap;
use entity::sea_orm_active_enums::BackupStatus;
use http::{HeaderMap, header};
use schemars::JsonSchema;
use serde::Serialize;
use shared::msg::LogLine;
use tokio::{
  sync::{
    broadcast::{self, error::RecvError},
    oneshot,
  },
  time::interval,
};
use uuid::Uuid;

use super::{
  access::{Access, load},
  status::{ServerStatus, status},
  types::ServerUuid,
};
use crate::{auth::jwt_auth::JwtAuth, nodes::Wings};

/// Frames buffered per server before a slow browser lags. Lagging behind on
/// logs and stats skips some, on events closes the socket.
const CAPACITY: usize = 256;
const PING_INTERVAL: Duration = Duration::from_secs(30);

/// One text JSON frame on `GET /servers/{uuid}/ws`, tagged by `type`.
#[derive(Serialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveFrame {
  Logs {
    lines: Vec<LogLine>,
  },
  /// Sent first on every connection, then on every change.
  State {
    status: ServerStatus,
  },
  Stats(LiveStats),
  /// End of a slow wings operation, e.g. a completed upload.
  OpFinished {
    op: Uuid,
    ok: bool,
    error: Option<String>,
  },
  /// Progress of a backup being created (`bytes` archived so far) or
  /// restored (`bytes` sent so far). A restore ends with `completed` or
  /// `failed` for the restore itself, the backup stays usable.
  Backup {
    id: Uuid,
    status: BackupStatus,
    bytes: u64,
  },
  /// `task_index` (from 0) is the task running or waiting for its offset.
  Schedule {
    id: Uuid,
    status: ScheduleRunState,
    task_index: Option<u32>,
  },
}

#[derive(Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleRunState {
  Running,
  Idle,
}

/// Limits are 0 when unlimited.
#[derive(Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LiveStats {
  pub cpu_millicores: u64,
  pub cpu_limit_millicores: u64,
  pub memory_bytes: u64,
  pub memory_limit_bytes: u64,
  pub disk_bytes: u64,
  pub disk_limit_bytes: u64,
  pub net_rx_bps: u64,
  pub net_tx_bps: u64,
  pub uptime_ms: u64,
}

/// Per-server fan-out to browsers. A channel exists only while someone
/// watches, frames for unwatched servers are dropped. Also hands the end of
/// wings operations to whoever waits for one.
#[derive(Clone, Default)]
pub struct Live {
  /// `(events, live)`, `live` carries logs and stats only.
  channels: Arc<DashMap<Uuid, (Sender, Sender)>>,
  ops: Arc<DashMap<Uuid, oneshot::Sender<std::result::Result<(), String>>>>,
}

type Sender = broadcast::Sender<Arc<str>>;
type Receiver = broadcast::Receiver<Arc<str>>;

impl Live {
  /// Returns `(events, live)`, see [`Live::publish`].
  pub fn subscribe(&self, server: Uuid) -> (Receiver, Receiver) {
    let pair = self.channels.entry(server).or_insert_with(|| {
      (
        broadcast::channel(CAPACITY).0,
        broadcast::channel(CAPACITY).0,
      )
    });
    (pair.0.subscribe(), pair.1.subscribe())
  }

  /// Call after dropping the receiver.
  pub fn unsubscribe(&self, server: Uuid) {
    self
      .channels
      .remove_if(&server, |_, (tx, _)| tx.receiver_count() == 0);
  }

  pub fn is_watched(&self, server: Uuid) -> bool {
    // an entry left by a failed upgrade has no receiver
    self
      .channels
      .get(&server)
      .is_some_and(|pair| pair.0.receiver_count() > 0)
  }

  pub fn publish(&self, server: Uuid, frame: &LiveFrame) {
    if let Some(pair) = self.channels.get(&server) {
      let text = serde_json::to_string(frame).expect("LiveFrame is always serializable");
      let tx = match frame {
        LiveFrame::Logs { .. } | LiveFrame::Stats(_) => &pair.1,
        _ => &pair.0,
      };
      // no receiver left is fine, the last one is about to unsubscribe
      let _ = tx.send(text.into());
    }
  }
}

impl Live {
  /// Resolves with the end of wings operation `op`, register before starting
  /// it. Dropped (receiver errors) when replaced by another wait on `op`.
  pub fn wait_op(&self, op: Uuid) -> oneshot::Receiver<std::result::Result<(), String>> {
    let (tx, rx) = oneshot::channel();
    self.ops.insert(op, tx);
    rx
  }

  pub fn finish_op(&self, op: Uuid, result: std::result::Result<(), String>) {
    if let Some((_, tx)) = self.ops.remove(&op) {
      let _ = tx.send(result);
    }
  }

  /// Stops waiting for `op`, e.g. when it never started.
  pub fn forget_op(&self, op: Uuid) {
    self.ops.remove(&op);
  }
}

/// Receive-only live view of a server, see [`LiveFrame`]. Read access.
pub(super) async fn server_ws(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  site: SiteConfig,
  headers: HeaderMap,
  Path(path): Path<ServerUuid>,
  ws: WebSocketUpgrade,
) -> Result<Response> {
  if !origin_allowed(&headers, &site.site_url) {
    bail!(FORBIDDEN, "Cross-origin websocket rejected");
  }
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Read).await?;
  let live = wings.live();
  // before reading the state, so no change between the two is missed
  let (events, live_rx) = live.subscribe(server.id);
  let initial = LiveFrame::State {
    status: status(&server, wings.state(server.node_id, server.id)),
  };
  Ok(ws.on_upgrade(move |socket| async move {
    run(socket, events, live_rx, initial).await;
    live.unsubscribe(server.id);
  }))
}

/// Browsers always send `Origin` on websocket upgrades, the cookie (SameSite
/// Lax) still rides along from same-site origins, so a foreign one is
/// rejected. Matches the request `Host` or the configured site URL. Clients
/// without `Origin` are not browsers and pass.
fn origin_allowed(headers: &HeaderMap, site_url: &url::Url) -> bool {
  let Some(origin) = headers.get(header::ORIGIN) else {
    return true;
  };
  let Some(origin) = origin.to_str().ok().and_then(|o| url::Url::parse(o).ok()) else {
    return false;
  };
  let Some(host) = origin.host_str() else {
    return false;
  };
  let authority = match origin.port() {
    Some(port) => format!("{host}:{port}"),
    None => host.to_string(),
  };
  let request_host = headers.get(header::HOST).and_then(|h| h.to_str().ok());
  request_host.is_some_and(|h| h.eq_ignore_ascii_case(&authority))
    || origin.origin() == site_url.origin()
}

async fn run(mut socket: WebSocket, mut events: Receiver, mut live: Receiver, initial: LiveFrame) {
  let initial = serde_json::to_string(&initial).expect("LiveFrame is always serializable");
  if socket.send(Message::Text(initial.into())).await.is_err() {
    return;
  }
  let mut ping = interval(PING_INTERVAL);
  loop {
    tokio::select! {
      frame = next(&mut events, &mut live) => match frame {
        Some(text) => {
          if socket.send(Message::Text(text.as_ref().into())).await.is_err() {
            return;
          }
        }
        None => return,
      },
      msg = socket.recv() => match msg {
        Some(Ok(Message::Close(_)) | Err(_)) | None => return,
        // pongs are answered by axum, other frames are ignored
        Some(Ok(_)) => (),
      },
      _ = ping.tick() => {
        if socket.send(Message::Ping(Default::default())).await.is_err() {
          return;
        }
      }
    }
  }
}

/// The next frame for the browser, `None` once the socket must close.
/// Cancel safe, like `broadcast::Receiver::recv`.
async fn next(events: &mut Receiver, live: &mut Receiver) -> Option<Arc<str>> {
  loop {
    tokio::select! {
      biased;
      frame = events.recv() => match frame {
        Ok(text) => return Some(text),
        // a slow browser missed events, closing makes it reconnect and reload
        Err(RecvError::Lagged(_) | RecvError::Closed) => return None,
      },
      frame = live.recv() => match frame {
        Ok(text) => return Some(text),
        // logs and stats only, the browser just misses them
        Err(RecvError::Lagged(_)) => (),
        Err(RecvError::Closed) => return None,
      },
    }
  }
}

/// Documents the frames in OpenAPI, the route itself answers `101`.
pub(super) fn docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
  op.id("server_ws")
    .description("Websocket (JWT cookie, read access) sending `LiveFrame` text frames.")
    .response::<101, Json<LiveFrame>>()
}

#[cfg(test)]
mod test {
  use chrono::DateTime;
  use serde_json::json;
  use shared::msg::LogStream;

  use super::*;

  #[test]
  fn frames_serialize_tagged() {
    let at = DateTime::from_timestamp_millis(1_000).unwrap();
    let frame = LiveFrame::Logs {
      lines: vec![LogLine {
        at,
        stream: LogStream::Console,
        line: "hi".into(),
      }],
    };
    assert_eq!(
      serde_json::to_value(frame).unwrap(),
      json!({"type": "logs", "lines": [{"at": "1970-01-01T00:00:01Z", "stream": "console", "line": "hi"}]})
    );
    assert_eq!(
      serde_json::to_value(LiveFrame::State {
        status: ServerStatus::InstallFailed
      })
      .unwrap(),
      json!({"type": "state", "status": "install_failed"})
    );
    let stats = serde_json::to_value(LiveFrame::Stats(LiveStats {
      cpu_millicores: 5,
      ..Default::default()
    }))
    .unwrap();
    assert_eq!(stats["type"], "stats");
    assert_eq!(stats["cpu_millicores"], 5);
    assert_eq!(stats.as_object().unwrap().len(), 10);
  }

  #[tokio::test]
  async fn registry_exists_only_while_watched() {
    let live = Live::default();
    let server = Uuid::now_v7();
    let frame = LiveFrame::State {
      status: ServerStatus::Running,
    };
    // nobody watches: dropped, no channel created
    live.publish(server, &frame);
    assert!(!live.is_watched(server));

    let mut a = live.subscribe(server);
    let b = live.subscribe(server);
    live.publish(server, &frame);
    assert_eq!(
      &*a.0.recv().await.unwrap(),
      r#"{"type":"state","status":"running"}"#
    );
    // still one receiver left
    drop(b);
    live.unsubscribe(server);
    assert!(live.is_watched(server));
    drop(a);
    live.unsubscribe(server);
    assert!(!live.is_watched(server));
    // dropped without unsubscribe, e.g. a failed upgrade
    drop(live.subscribe(server));
    assert!(!live.is_watched(server));
  }

  #[tokio::test]
  async fn ops_resolve_once() {
    let live = Live::default();
    let op = Uuid::now_v7();
    // nobody waits: ignored
    live.finish_op(op, Ok(()));
    let rx = live.wait_op(op);
    live.finish_op(op, Err("boom".into()));
    assert_eq!(rx.await.unwrap(), Err("boom".into()));
    let rx = live.wait_op(op);
    live.forget_op(op);
    assert!(rx.await.is_err());
  }

  #[tokio::test]
  async fn log_lag_keeps_the_connection() {
    let live = Live::default();
    let server = Uuid::now_v7();
    let (mut events, mut logs) = live.subscribe(server);
    for _ in 0..CAPACITY * 4 {
      live.publish(server, &LiveFrame::Logs { lines: vec![] });
    }
    live.publish(
      server,
      &LiveFrame::State {
        status: ServerStatus::Running,
      },
    );
    // events come first, skipped lines do not close
    assert_eq!(
      next(&mut events, &mut logs).await.as_deref(),
      Some(r#"{"type":"state","status":"running"}"#)
    );
    assert_eq!(
      next(&mut events, &mut logs).await.as_deref(),
      Some(r#"{"type":"logs","lines":[]}"#)
    );
  }

  #[test]
  fn origin_must_match_host_or_site() {
    let site = url::Url::parse("https://panel.example.com").unwrap();
    let check = |origin: Option<&str>, host: &str| {
      let mut h = HeaderMap::new();
      h.insert(header::HOST, host.parse().unwrap());
      if let Some(o) = origin {
        h.insert(header::ORIGIN, o.parse().unwrap());
      }
      origin_allowed(&h, &site)
    };
    // non-browser client
    assert!(check(None, "localhost:8000"));
    assert!(check(Some("http://localhost:8000"), "localhost:8000"));
    assert!(check(Some("http://LOCALHOST:8000"), "localhost:8000"));
    // behind a proxy rewriting Host, the site URL still matches
    assert!(check(Some("https://panel.example.com"), "backend:8000"));
    assert!(!check(Some("http://localhost:8001"), "localhost:8000"));
    assert!(!check(
      Some("https://evil.example.com"),
      "panel.example.com"
    ));
    assert!(!check(Some("http://panel.example.com"), "backend:8000"));
    assert!(!check(Some("null"), "localhost:8000"));
  }

  #[test]
  fn backup_and_schedule_frames_serialize() {
    let id = Uuid::nil();
    assert_eq!(
      serde_json::to_value(LiveFrame::Backup {
        id,
        status: BackupStatus::Creating,
        bytes: 7
      })
      .unwrap(),
      json!({"type": "backup", "id": id, "status": "creating", "bytes": 7})
    );
    assert_eq!(
      serde_json::to_value(LiveFrame::Schedule {
        id,
        status: ScheduleRunState::Running,
        task_index: Some(1)
      })
      .unwrap(),
      json!({"type": "schedule", "id": id, "status": "running", "task_index": 1})
    );
  }
}
