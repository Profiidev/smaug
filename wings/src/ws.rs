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
use tokio::sync::broadcast::error::RecvError;
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

async fn connection(mut socket: WebSocket, store: Store) {
  let mut events = store.subscribe();
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
  loop {
    tokio::select! {
      next = socket.recv() => match next {
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
      event = events.recv() => match event {
        Ok(event) => {
          if send(&mut socket, &event).await.is_err() {
            break;
          }
        }
        // dropping the connection makes the panel reconnect and resync
        Err(RecvError::Lagged(n)) => {
          warn!("Panel connection lagged {} events behind, closing", n);
          break;
        }
        Err(RecvError::Closed) => break,
      },
    }
  }
}

async fn send(socket: &mut WebSocket, event: &Event) -> Result<(), axum::Error> {
  let msg = serde_json::to_vec(event).expect("Event is always serializable");
  socket.send(ws::Message::Binary(msg.into())).await
}
