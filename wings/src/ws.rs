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
use shared::msg::WingsMessage;
use tracing::info;

use crate::auth;

pub fn router() -> Router {
  Router::new()
    .route("/", any(init_connection))
    .layer(from_fn(auth::verify))
}

async fn init_connection(ws: WebSocketUpgrade) -> Response {
  info!("Established wings websocket connection");
  ws.on_upgrade(connection)
}

async fn connection(mut socket: WebSocket) {
  while let Some(Ok(next)) = socket.recv().await {
    match next {
      ws::Message::Binary(raw_msg) => match serde_json::from_slice::<WingsMessage>(&raw_msg) {
        Ok(msg) => {
          info!("Parsed wings message: {:?}", msg);
          if msg == WingsMessage::Hello {
            socket
              .send(ws::Message::Binary(
                serde_json::to_vec(&WingsMessage::World).unwrap().into(),
              ))
              .await
              .unwrap();
          }
        }
        Err(err) => {
          info!("Failed to parse wings message: {}", err);
        }
      },
      ws::Message::Close(_) => {
        info!("Wings websocket connection closed");
        break;
      }
      _ => (),
    }
  }
}
