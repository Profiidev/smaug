use axum::{
  Router,
  extract::{
    WebSocketUpgrade,
    ws::{self, WebSocket},
  },
  middleware::from_fn,
  response::Response,
  routing::any,
};
use shared::msg::{Event, WingsMessage};
use std::time::Duration;

use tokio::{
  sync::broadcast::{Receiver, error::RecvError},
  time::{Instant, interval, timeout},
};
use tracing::{info, warn};

use crate::{auth, store::Store};

pub fn router() -> Router {
  Router::new()
    .route("/", any(init_connection))
    .layer(from_fn(auth::verify))
}

async fn init_connection(ws: WebSocketUpgrade, store: Store) -> Response {
  info!("Established wings websocket connection");
  ws.on_upgrade(move |socket| connection(socket, store))
}

/// The panel pings every 15 s, a sync included. A connection silent for
/// longer is half-open, e.g. the panel host died.
const LIVENESS_TIMEOUT: Duration = Duration::from_secs(90);
/// A send blocked this long by a full socket buffer means the panel is gone.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);

async fn connection(mut socket: WebSocket, store: Store) {
  let mut events = store.subscribe();
  let mut live = store.subscribe_live();
  if send(
    &mut socket,
    &Event::Hello {
      runtime: store.runtime(),
    },
  )
  .await
  .is_err()
  {
    return;
  }
  let mut last_seen = Instant::now();
  let mut check = interval(Duration::from_secs(30));
  loop {
    tokio::select! {
      _ = check.tick() => {
        if last_seen.elapsed() > LIVENESS_TIMEOUT {
          warn!("Panel connection timed out, closing");
          break;
        }
      }
      next = socket.recv() => match next.inspect(|_| last_seen = Instant::now()) {
        Some(Ok(ws::Message::Binary(raw_msg))) => {
          match serde_json::from_slice::<WingsMessage>(&raw_msg) {
            Ok(msg) => info!("Parsed wings message: {:?}", msg),
            Err(err) => info!("Failed to parse wings message: {}", err),
          }
        }
        Some(Ok(ws::Message::Close(_))) | None | Some(Err(_)) => {
          info!("Wings websocket connection closed");
          break;
        }
        _ => (),
      },
      event = next(&mut events, &mut live) => match event {
        Some(event) => {
          if send(&mut socket, &event).await.is_err() {
            break;
          }
        }
        None => break,
      },
    }
  }
}

/// The next event for the panel, `None` once the connection must close.
/// Cancel safe, like `broadcast::Receiver::recv`.
async fn next(events: &mut Receiver<Event>, live: &mut Receiver<Event>) -> Option<Event> {
  loop {
    tokio::select! {
      biased;
      event = events.recv() => match event {
        Ok(event) => return Some(event),
        // dropping the connection makes the panel reconnect and resync
        Err(RecvError::Lagged(n)) => {
          warn!("Panel connection lagged {} events behind, closing", n);
          return None;
        }
        Err(RecvError::Closed) => return None,
      },
      event = live.recv() => match event {
        Ok(event) => return Some(event),
        // live only, the panel just misses them
        Err(RecvError::Lagged(n)) => warn!("Panel connection skipped {} log and metric events", n),
        Err(RecvError::Closed) => return None,
      },
    }
  }
}

/// Fails on a closed socket or one that took longer than `SEND_TIMEOUT`.
async fn send(socket: &mut WebSocket, event: &Event) -> Result<(), ()> {
  let msg = serde_json::to_vec(event).expect("Event is always serializable");
  match timeout(SEND_TIMEOUT, socket.send(ws::Message::Binary(msg.into()))).await {
    Ok(Ok(())) => Ok(()),
    Ok(Err(_)) => Err(()),
    Err(_) => {
      warn!("Sending to the panel timed out, closing");
      Err(())
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::store::test::memory;
  use uuid::Uuid;

  #[tokio::test]
  async fn live_lag_keeps_the_connection() {
    let store = memory();
    let mut events = store.subscribe();
    let mut live = store.subscribe_live();
    let uuid = Uuid::new_v4();
    for _ in 0..5000 {
      store.send(Event::Logs {
        uuid,
        lines: vec![],
      });
    }
    let finished = Event::OpFinished {
      op: Uuid::new_v4(),
      uuid,
      ok: true,
      error: None,
      result: None,
    };
    store.send(finished.clone());
    // state and op events come first, skipped lines do not close
    assert_eq!(next(&mut events, &mut live).await, Some(finished));
    assert!(matches!(
      next(&mut events, &mut live).await,
      Some(Event::Logs { .. })
    ));
  }
}
