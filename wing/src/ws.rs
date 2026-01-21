use axum::{
  Router,
  extract::{WebSocketUpgrade, ws::WebSocket},
  response::Response,
  routing::any,
};
use centaurus::error::Result;
use http::HeaderMap;
use shared::auth::SignData;
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

async fn connection(_socket: WebSocket) {}
