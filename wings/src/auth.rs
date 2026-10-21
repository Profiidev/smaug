use std::sync::Arc;

use axum::{
  Extension, Router,
  body::{Body, to_bytes},
  extract::{OriginalUri, Request},
  middleware::Next,
  response::{IntoResponse, Response},
};
use centaurus::error::{ErrorReportStatusExt, Result};
use chrono::Utc;
use http::StatusCode;
use shared::auth::{NonceCache, SignData};
use tracing::debug;

use crate::config::Config;

const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
/// The full sync carries every server's spec, install scripts included.
const MAX_SYNC_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct WingsAuth {
  token: Arc<str>,
  nonces: Arc<NonceCache>,
}

/// Verifies the request signature (method, path, body, timestamp, nonce) and
/// signs the response so the backend can verify it came from this wings.
pub async fn verify(
  Extension(auth): Extension<WingsAuth>,
  OriginalUri(uri): OriginalUri,
  req: Request,
  next: Next,
) -> Result<Response> {
  // the timestamp is checked against the arrival, a big sync body may upload slowly
  let arrived = Utc::now().timestamp_millis();
  let (parts, body) = req.into_parts();
  let limit = match uri.path() {
    "/api/servers" => MAX_SYNC_BYTES,
    _ => MAX_BODY_BYTES,
  };
  let body = to_bytes(body, limit)
    .await
    .status_context(StatusCode::PAYLOAD_TOO_LARGE, "Request body too large")?;
  let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");

  let data = SignData::validate_header_map_at(
    &parts.headers,
    &auth.token,
    parts.method.as_str(),
    path,
    &body,
    arrived,
  )?;
  auth.nonces.check(&data)?;
  debug!("Authenticated wings request to {}", path);

  let mut res = next
    .run(Request::from_parts(parts, Body::from(body)))
    .await
    .into_response();
  data.add_response_to_header_map(res.headers_mut(), &auth.token)?;
  Ok(res)
}

pub fn state(router: Router, config: &Config) -> Router {
  router.layer(Extension(WingsAuth {
    token: config.token.as_str().into(),
    nonces: Default::default(),
  }))
}
