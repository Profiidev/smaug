use std::{sync::LazyLock, time::Duration};

use centaurus::{error::Result, eyre::Context};
use shared::msg::{ConfigFile, PrepareRequest, PrepareResponse};

use crate::docker::Agent;

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
  reqwest::Client::builder()
    .timeout(Duration::from_secs(60))
    .build()
    .expect("valid client")
});

/// Applies config files and permissions, returns the bytes used. A freshly
/// created agent gets 10 s to come up.
pub async fn prepare(agent: &Agent, config_files: &[ConfigFile], interface: &str) -> Result<u64> {
  for _ in 0..50 {
    let health = CLIENT
      .get(format!("{}/health", agent.url))
      .bearer_auth(&agent.token)
      .send()
      .await;
    if health.is_ok_and(|r| r.status().is_success()) {
      break;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
  }
  let res: PrepareResponse = CLIENT
    .post(format!("{}/prepare", agent.url))
    .bearer_auth(&agent.token)
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
