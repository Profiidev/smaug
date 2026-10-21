use std::{sync::LazyLock, time::Duration};

use centaurus::{error::Result, eyre::Context};
use shared::msg::{ConfigFile, PrepareRequest, PrepareResponse};

use crate::docker::Agent;

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

/// Applies config files and permissions, returns the bytes used. A freshly
/// created agent gets 10 s to come up.
pub async fn prepare(agent: &Agent, config_files: &[ConfigFile], interface: &str) -> Result<u64> {
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
  // still down after 10 s: the prepare below fails
  let _ = tokio::time::timeout(Duration::from_secs(10), up).await;
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
