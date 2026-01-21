use axum::{
  Router,
  extract::{WebSocketUpgrade, ws::WebSocket},
  response::Response,
  routing::any,
};
use centaurus::error::Result;
use hmac::{Hmac, Mac};
use http::HeaderMap;
use rand::RngCore;
use sha3::Sha3_512;
use tracing::info;

use crate::auth::{Auth, WingToken};

type HmacSha3_512 = Hmac<Sha3_512>;

pub fn router() -> Router {
  Router::new().route("/", any(init_connection))
}

async fn init_connection(
  auth: Auth,
  token: WingToken,
  ws: WebSocketUpgrade,
) -> Result<(HeaderMap, Response)> {
  let mut rng = rand::rng();
  let mut raw_nonce = [0u8; 16];
  rng.fill_bytes(&mut raw_nonce);
  let nonce = hex::encode(raw_nonce);

  let msg = format!("{}{}", nonce, auth.timestamp);

  let mut mac = HmacSha3_512::new_from_slice(token.0.as_bytes())?;
  mac.update(msg.as_bytes());
  let signature = hex::encode(mac.finalize().into_bytes());

  let mut headers = HeaderMap::new();
  headers.insert("x-wing-nonce", nonce.parse().unwrap());
  headers.insert("x-wing-timestamp", auth.timestamp.parse().unwrap());
  headers.insert("x-wing-signature", signature.parse().unwrap());

  info!("Established wing websocket connection");

  Ok((headers, ws.on_upgrade(connection)))
}

async fn connection(_socket: WebSocket) {}
