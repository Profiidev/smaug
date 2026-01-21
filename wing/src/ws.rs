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
use shared::{auth::SignData, msg::WingMessage};
use tracing::info;

use crate::auth::{Auth, WingToken};

pub fn router() -> Router {
  Router::new().route("/", any(init_connection))
}

async fn init_connection(
  auth: Auth,
  token: WingToken,
  ws: WebSocketUpgrade,
) -> Result<(HeaderMap, Response)> {
  let data = SignData::from_timestamp(auth.timestamp);
  let headers = data.to_header_map(&token.0);

  info!("Established wing websocket connection");

  Ok((headers, ws.on_upgrade(connection)))
}

async fn connection(mut socket: WebSocket) {
  while let Some(Ok(next)) = socket.recv().await {
    match next {
      ws::Message::Binary(raw_msg) => match serde_json::from_slice::<WingMessage>(&raw_msg) {
        Ok(msg) => {
          info!("Parsed wing message: {:?}", msg);
          if msg == WingMessage::Hello {
            socket
              .send(ws::Message::Binary(
                serde_json::to_vec(&WingMessage::World).unwrap().into(),
              ))
              .await
              .unwrap();
          }
        }
        Err(err) => {
          info!("Failed to parse wing message: {}", err);
        }
      },
      ws::Message::Close(_) => {
        info!("Wing websocket connection closed");
        break;
      }
      _ => (),
    }
  }
}
