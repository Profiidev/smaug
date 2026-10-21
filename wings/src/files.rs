//! `/servers/{uuid}/files/*` (`docs/API.md` §3.3), forwarded to the server's agent.

use axum::{
  Extension, Router,
  body::{Body, Bytes},
  extract::{Path, RawQuery},
  middleware::from_fn,
  response::Response,
  routing::any,
};
use http::{HeaderMap, Method, header};
use shared::msg::{ErrorCode, FileEntry, ServerSpec};
use uuid::Uuid;

use crate::{
  agent, auth,
  config::Config,
  docker::Agent,
  error::{ApiError, ApiResult},
  servers::{Runtime, not_found, unsupported},
  store::Store,
};

pub fn router() -> Router {
  Router::new()
    .route("/servers/{uuid}/files/{op}", any(forward))
    .layer(from_fn(auth::verify))
}

/// The server's running agent. Started on demand, it then runs while the server exists.
// ponytail: a few docker calls per request, cache the agent if file traffic gets heavy
pub async fn agent_for(
  store: &Store,
  docker: &Runtime,
  uuid: Uuid,
) -> ApiResult<(Agent, ServerSpec)> {
  let docker = docker.0.as_ref().ok_or_else(unsupported)?;
  let spec = store.get(uuid).await?.ok_or_else(not_found)?.server;
  docker.ensure_volume(&spec).await?;
  let agent = docker.ensure_agent(uuid).await?;
  agent::wait_up(&agent).await;
  Ok((agent, spec))
}

#[allow(clippy::too_many_arguments)]
async fn forward(
  store: Store,
  docker: Runtime,
  Extension(config): Extension<Config>,
  Path((uuid, op)): Path<(Uuid, String)>,
  method: Method,
  RawQuery(query): RawQuery,
  headers: HeaderMap,
  body: Bytes,
) -> ApiResult<Response> {
  let allowed = matches!(
    (&method, op.as_str()),
    (&Method::GET, "list" | "stat" | "usage" | "content")
      | (&Method::PUT, "content")
      | (&Method::POST, "mkdir" | "rename" | "copy" | "delete")
  );
  if !allowed {
    return Err(ApiError::new(ErrorCode::NotFound, "No such file route"));
  }
  let too_large = || ApiError::new(ErrorCode::TooLarge, "Larger than one chunk");
  if body.len() as u64 > config.chunk_size {
    return Err(too_large());
  }
  let (agent, spec) = agent_for(&store, &docker, uuid).await?;
  let query = query.map(|q| format!("?{q}")).unwrap_or_default();
  let range = headers.get(header::RANGE).and_then(|r| r.to_str().ok());

  if method == Method::GET && op == "content" {
    let stat = agent::files(&agent, &spec, Method::GET, &format!("/files/stat{query}"))
      .send()
      .await?;
    let entry: FileEntry = agent::ok(stat).await?.json().await?;
    let len = range_len(range, entry.size)
      .ok_or_else(|| ApiError::new(ErrorCode::BadRequest, "Invalid range"))?;
    if len > config.chunk_size {
      return Err(too_large());
    }
  }

  let mut req = agent::files(&agent, &spec, method, &format!("/files/{op}{query}")).body(body);
  for name in [header::RANGE, header::CONTENT_TYPE] {
    if let Some(value) = headers.get(&name) {
      req = req.header(name, value);
    }
  }
  let res = agent::ok(req.send().await?).await?;
  let mut out = Response::builder().status(res.status());
  for name in [
    header::CONTENT_TYPE,
    header::CONTENT_LENGTH,
    header::CONTENT_RANGE,
    header::ACCEPT_RANGES,
  ] {
    if let Some(value) = res.headers().get(&name) {
      out = out.header(name, value);
    }
  }
  out
    .body(Body::from_stream(res.bytes_stream()))
    .map_err(ApiError::internal)
}

/// Bytes a `Range` header asks for from a file of `size`, the whole file
/// without one. `None` for a range wings does not serve (several ranges).
fn range_len(range: Option<&str>, size: u64) -> Option<u64> {
  let Some(range) = range else {
    return Some(size);
  };
  let (start, end) = range.strip_prefix("bytes=")?.split_once('-')?;
  let last = size.saturating_sub(1);
  Some(match (start.trim(), end.trim()) {
    ("", n) => n.parse::<u64>().ok()?.min(size),
    (a, "") => size.saturating_sub(a.parse().ok()?),
    (a, b) => {
      let (a, b): (u64, u64) = (a.parse().ok()?, b.parse().ok()?);
      if a > b {
        return None;
      }
      b.min(last).saturating_sub(a) + 1
    }
  })
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn range_lengths() {
    assert_eq!(range_len(None, 10), Some(10));
    assert_eq!(range_len(Some("bytes=0-3"), 10), Some(4));
    assert_eq!(range_len(Some("bytes=5-100"), 10), Some(5));
    assert_eq!(range_len(Some("bytes=4-"), 10), Some(6));
    assert_eq!(range_len(Some("bytes=-3"), 10), Some(3));
    assert_eq!(range_len(Some("bytes=3-1"), 10), None);
    assert_eq!(range_len(Some("bytes=0-1,4-5"), 10), None);
    assert_eq!(range_len(Some("items=0-1"), 10), None);
  }
}
