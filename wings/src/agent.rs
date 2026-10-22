use std::{sync::LazyLock, time::Duration};

use centaurus::{error::Result, eyre::Context};
use reqwest::{Method, RequestBuilder};
use shared::msg::{ConfigFile, ErrorBody, PrepareRequest, PrepareResponse, ServerSpec};

use crate::{
  docker::Agent,
  error::{ApiError, ApiResult},
};

/// Walking a big volume for permissions and disk usage takes a while.
const PREPARE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
  reqwest::Client::builder()
    .timeout(Duration::from_secs(60))
    // an agent on the internal network answers at once or not at all
    .connect_timeout(Duration::from_secs(2))
    .build()
    .expect("valid client")
});

/// A freshly created agent gets 10 s to come up, a later request fails when it does not.
pub async fn wait_up(agent: &Agent) {
  let up = async {
    loop {
      let health = CLIENT
        .get(format!("{}/health", agent.url))
        .bearer_auth(&agent.token)
        .timeout(Duration::from_secs(2))
        .send()
        .await;
      // any answer means it is up, a rejected token fails the prepare below
      if health.is_ok() {
        return;
      }
      tokio::time::sleep(Duration::from_millis(200)).await;
    }
  };
  let _ = tokio::time::timeout(Duration::from_secs(10), up).await;
}

/// Applies config files and permissions, returns the bytes used.
pub async fn prepare(agent: &Agent, config_files: &[ConfigFile], interface: &str) -> Result<u64> {
  wait_up(agent).await;
  let res: PrepareResponse = CLIENT
    .post(format!("{}/prepare", agent.url))
    .bearer_auth(&agent.token)
    .timeout(PREPARE_TIMEOUT)
    .json(&PrepareRequest {
      config_files: config_files.to_vec(),
      interface: interface.into(),
    })
    .send()
    .await
    .context("Failed to reach agent")?
    .error_for_status()
    .context("Agent failed to prepare files")?
    .json()
    .await
    .context("Invalid agent response")?;
  Ok(res.disk_bytes)
}

pub fn request(agent: &Agent, method: Method, path: &str) -> RequestBuilder {
  CLIENT
    .request(method, format!("{}{path}", agent.url))
    .bearer_auth(&agent.token)
}

/// A file route request, carrying the server's denylist and disk limit.
pub fn files(agent: &Agent, spec: &ServerSpec, method: Method, path: &str) -> RequestBuilder {
  let denylist = serde_json::to_string(&spec.file_denylist).expect("always serializable");
  request(agent, method, path)
    .header("x-denylist", denylist)
    .header("x-disk-limit", disk_limit(spec).to_string())
}

/// 0 means unlimited.
pub fn disk_limit(spec: &ServerSpec) -> u64 {
  u64::try_from(spec.limits.disk_mb).unwrap_or(0) * 1024 * 1024
}

/// Passes an agent error on as it is.
pub async fn ok(res: reqwest::Response) -> ApiResult<reqwest::Response> {
  let status = res.status();
  if status.is_success() {
    return Ok(res);
  }
  let body = res.bytes().await.unwrap_or_default();
  Err(match serde_json::from_slice::<ErrorBody>(&body) {
    Ok(body) => ApiError { body, status },
    Err(_) => ApiError::internal(format!("Agent answered {status}")),
  })
}

/// `cached` takes the agent's last walk plus the writes since, no full walk.
pub async fn usage(agent: &Agent, spec: &ServerSpec, cached: bool) -> ApiResult<u64> {
  let res = files(agent, spec, Method::GET, "/files/usage")
    .query(&[("cached", cached)])
    .send()
    .await?;
  let usage: shared::msg::UsageResponse = ok(res).await?.json().await?;
  Ok(usage.bytes)
}
