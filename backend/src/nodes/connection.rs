use std::{
  sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use centaurus::{bail, db::init::Connection, error::Result, eyre::Context};
use dashmap::DashMap;
use entity::sea_orm_active_enums::ServerInstallState;
use futures_util::{SinkExt, StreamExt};
use reqwest::Method;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use serde_json::Value;
use shared::msg::{
  Event, Power, PowerRequest, Runtime, ServerState, SyncRequest, SyncResponse, WingsMessage,
};
use tokio::{
  spawn,
  sync::mpsc,
  task::JoinHandle,
  time::{Instant, interval, sleep},
};
use tokio_tungstenite::tungstenite;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
  db::DBTrait,
  nodes::auth::{WingsAuth, WsStream},
  servers::spec,
  utils::{UpdateMessage, Updater, client},
};

const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const PING_INTERVAL: Duration = Duration::from_secs(15);
/// Connection counts as dead if nothing (not even a pong) arrived for this long.
const LIVENESS_TIMEOUT: Duration = Duration::from_secs(45);

/// Handle to the task that owns the websocket to one wings node. Dropping it
/// closes the connection.
pub struct WingsConnection {
  link: Link,
  tx: mpsc::Sender<WingsMessage>,
  connected: Arc<AtomicBool>,
  task: JoinHandle<()>,
}

/// Everything needed to talk to one node and to apply what it reports.
#[derive(Clone)]
pub struct Link {
  uuid: Uuid,
  base: String,
  client: ClientWithMiddleware,
  /// Runtime state per server, as last reported. Empty while disconnected.
  states: Arc<DashMap<Uuid, ServerState>>,
  runtime: Arc<RwLock<Option<Runtime>>>,
  db: Connection,
  updater: Updater,
}

impl WingsConnection {
  pub fn new(
    uuid: Uuid,
    addr: &str,
    port: u16,
    secure: bool,
    token: String,
    db: Connection,
    updater: Updater,
  ) -> Self {
    let ws_addr = format!(
      "{}://{}:{}/api",
      if secure { "wss" } else { "ws" },
      addr,
      port
    );
    let base = format!(
      "{}://{}:{}/api",
      if secure { "https" } else { "http" },
      addr,
      port
    );

    let link = Link {
      uuid,
      base,
      client: ClientBuilder::new(client())
        .with(WingsAuth::new(token.clone()))
        .build(),
      states: Default::default(),
      runtime: Default::default(),
      db,
      updater,
    };

    let (tx, rx) = mpsc::channel(64);
    let connected = Arc::new(AtomicBool::new(false));
    let task = spawn(connection_task(
      link.clone(),
      ws_addr,
      token,
      rx,
      connected.clone(),
    ));

    Self {
      link,
      tx,
      connected,
      task,
    }
  }

  pub fn is_connected(&self) -> bool {
    self.connected.load(Ordering::Relaxed)
  }

  /// The link to use for requests, `None` while disconnected.
  pub fn link(&self) -> Option<Link> {
    self.is_connected().then(|| self.link.clone())
  }

  pub fn state(&self, server: Uuid) -> Option<ServerState> {
    self.link.states.get(&server).map(|s| *s)
  }

  pub fn runtime(&self) -> Option<Runtime> {
    *self.link.runtime.read().unwrap()
  }

  #[allow(unused)]
  pub async fn send(&self, msg: WingsMessage) -> Result<()> {
    if !self.is_connected() || self.tx.send(msg).await.is_err() {
      bail!("Wings connection to {} is not established", self.link.uuid);
    }
    Ok(())
  }
}

impl Drop for WingsConnection {
  fn drop(&mut self) {
    self.task.abort();
  }
}

impl Link {
  /// Best-effort request: failures are only logged, the full sync on the next
  /// (re)connect brings the node back in line with the DB.
  pub async fn push(&self, method: Method, path: &str, body: Option<Value>) {
    if let Err(err) = self.request(method, path, body).await {
      warn!("Wings request {} to {} failed: {:?}", path, self.uuid, err);
    }
  }

  pub async fn power(&self, server: Uuid, action: Power) {
    let body = serde_json::to_value(PowerRequest { action }).ok();
    self
      .push(Method::POST, &format!("/servers/{server}/power"), body)
      .await;
  }

  async fn request(
    &self,
    method: Method,
    path: &str,
    body: Option<Value>,
  ) -> Result<reqwest::Response> {
    let mut req = self
      .client
      .request(method, format!("{}{}", self.base, path));
    if let Some(body) = body {
      req = req.json(&body);
    }
    Ok(
      req
        .send()
        .await
        .context("Failed to reach wings")?
        .error_for_status()
        .context("Wings rejected the request")?,
    )
  }

  /// Sends every server of this node to wings and fixes what drifted while
  /// the node was unreachable.
  async fn sync(&self) -> Result<()> {
    let servers = self.db.server().list_by_node(self.uuid).await?;
    let mut specs = Vec::with_capacity(servers.len());
    // ponytail: two queries per server, batch once nodes hold many servers
    for server in &servers {
      specs.push(spec::load(&self.db, server).await?);
    }
    let body = serde_json::to_value(SyncRequest { servers: specs }).ok();
    let res: SyncResponse = self
      .request(Method::PUT, "/servers", body)
      .await?
      .json()
      .await
      .context("Invalid sync response from wings")?;

    if !res.orphans.is_empty() {
      warn!("Wings {} has unknown servers: {:?}", self.uuid, res.orphans);
    }
    self.states.clear();
    for (uuid, state) in &res.states {
      self.states.insert(*uuid, *state);
    }

    for mut server in servers {
      let state = res.states.get(&server.id).copied().unwrap_or_default();
      let uuid = server.id;
      if server.install_state == ServerInstallState::Installing && state != ServerState::Installing
      {
        // wings lost the install (restart, or the request never arrived)
        server.install_state = ServerInstallState::InstallFailed;
        self.db.server().save(server).await?;
      } else if server.suspended && state != ServerState::Offline {
        self.power(uuid, Power::Kill).await;
      }
      self
        .updater
        .broadcast(UpdateMessage::Servers { uuid })
        .await;
    }
    Ok(())
  }

  async fn handle(&self, event: Event) -> Result<()> {
    let uuid = match event {
      Event::Hello { runtime } => {
        *self.runtime.write().unwrap() = Some(runtime);
        self
          .updater
          .broadcast(UpdateMessage::Nodes { uuid: self.uuid })
          .await;
        return Ok(());
      }
      Event::State { uuid, state } => {
        self.states.insert(uuid, state);
        uuid
      }
      Event::InstallFinished { uuid, ok } => {
        let mut server = self.db.server().find_by_id(uuid).await?;
        if server.node_id != self.uuid || server.install_state != ServerInstallState::Installing {
          return Ok(());
        }
        server.install_state = if ok {
          ServerInstallState::Installed
        } else {
          ServerInstallState::InstallFailed
        };
        let start = ok && server.start_on_completion && !server.suspended;
        self.db.server().save(server).await?;
        if start {
          self.power(uuid, Power::Start).await;
        }
        uuid
      }
    };
    self
      .updater
      .broadcast(UpdateMessage::Servers { uuid })
      .await;
    Ok(())
  }

  /// Forgets everything the node reported, it is unknown while disconnected.
  async fn reset(&self) {
    *self.runtime.write().unwrap() = None;
    let servers: Vec<Uuid> = self.states.iter().map(|e| *e.key()).collect();
    self.states.clear();
    for uuid in servers {
      self
        .updater
        .broadcast(UpdateMessage::Servers { uuid })
        .await;
    }
  }
}

async fn connection_task(
  link: Link,
  addr: String,
  token: String,
  mut rx: mpsc::Receiver<WingsMessage>,
  connected: Arc<AtomicBool>,
) {
  let uuid = link.uuid;
  loop {
    match WingsAuth::connect_websocket(&addr, &token).await {
      Ok(stream) => {
        connected.store(true, Ordering::Relaxed);
        link.updater.broadcast(UpdateMessage::Nodes { uuid }).await;
        debug!("Wings connection to {} established", uuid);

        // events wait in the socket until the sync is done
        if let Err(err) = link.sync().await {
          warn!("Failed to sync wings {}: {:?}", uuid, err);
        }
        run_connection(&link, stream, &mut rx).await;

        connected.store(false, Ordering::Relaxed);
        link.reset().await;
        link.updater.broadcast(UpdateMessage::Nodes { uuid }).await;
        warn!("Wings connection to {} lost, reconnecting", uuid);
      }
      Err(err) => warn!(
        "Failed to connect to wings websocket for {}: {:?}",
        uuid, err
      ),
    }

    sleep(RECONNECT_DELAY).await;
  }
}

/// Runs until the connection is closed, errors or stops answering pings.
async fn run_connection(link: &Link, stream: WsStream, rx: &mut mpsc::Receiver<WingsMessage>) {
  let uuid = link.uuid;
  let (mut sink, mut stream) = stream.split();
  let mut ping = interval(PING_INTERVAL);
  let mut last_seen = Instant::now();

  loop {
    tokio::select! {
      msg = stream.next() => {
        let Some(Ok(msg)) = msg else {
          return;
        };
        last_seen = Instant::now();

        match msg {
          tungstenite::Message::Binary(raw_msg) => {
            match serde_json::from_slice::<Event>(&raw_msg) {
              Ok(event) => {
                debug!("Wings event from {}: {:?}", uuid, event);
                if let Err(err) = link.handle(event).await {
                  warn!("Failed to handle wings event from {}: {:?}", uuid, err);
                }
              }
              Err(err) => info!("Failed to parse wings message for {}: {}", uuid, err),
            }
          }
          tungstenite::Message::Close(_) => return,
          _ => (),
        }
      }
      Some(msg) = rx.recv() => {
        let msg = serde_json::to_vec(&msg).expect("WingsMessage is always serializable");
        if let Err(err) = sink.send(tungstenite::Message::Binary(msg.into())).await {
          warn!("Failed to send message to wings {}: {}", uuid, err);
          return;
        }
      }
      _ = ping.tick() => {
        if last_seen.elapsed() > LIVENESS_TIMEOUT {
          warn!("Wings connection to {} timed out", uuid);
          return;
        }
        if sink.send(tungstenite::Message::Ping(Default::default())).await.is_err() {
          return;
        }
      }
    }
  }
}
