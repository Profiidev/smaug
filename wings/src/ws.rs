use axum::{
  Router,
  extract::{
    WebSocketUpgrade,
    ws::{self, WebSocket},
  },
  response::Response,
  routing::any,
};
use centaurus::error::Result;
use http::HeaderMap;
use shared::{auth::SignData, msg::WingsMessage};
use tracing::info;

use crate::auth::{Auth, WingsToken};

pub fn router() -> Router {
  Router::new().route("/", any(init_connection))
}

async fn init_connection(
  auth: Auth,
  token: WingsToken,
  ws: WebSocketUpgrade,
) -> Result<(HeaderMap, Response)> {
  let data = SignData::from_timestamp(auth.timestamp);
  let headers = data.to_header_map(&token.0)?;

  info!("Established wings websocket connection");

  Ok((headers, ws.on_upgrade(connection)))
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
