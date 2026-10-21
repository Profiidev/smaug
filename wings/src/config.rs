use std::path::PathBuf;

use centaurus::{
  Config,
  backend::{
    auth::settings::AuthConfig,
    config::{BaseConfig, MetricsConfig, SiteConfig},
  },
};
use figment::{
  Figment,
  providers::{Env, Serialized},
};
use serde::{Deserialize, Serialize};
use shared::msg::Runtime;
use tracing::instrument;

#[derive(Deserialize, Serialize, Clone, Config)]
pub struct Config {
  #[serde(flatten)]
  #[base]
  pub base: BaseConfig,
  #[serde(flatten)]
  #[metrics]
  pub metrics: MetricsConfig,
  // required because of cargo feature unification
  #[site]
  pub site: SiteConfig,
  // required because of cargo feature unification
  #[auth]
  pub auth: AuthConfig,

  pub token: String,
  pub runtime: Runtime,
  pub k8s_namespace: String,
  pub docker_network: String,
  /// Image of the per-server file helper (`smaug-agent`).
  pub agent_image: String,
  /// Internal network wings talks to the agents on; game containers are not on it.
  pub agent_network: String,
  /// Local to wings, never mounted into a container.
  pub install_log_dir: PathBuf,
  /// `TZ` of every server container.
  pub timezone: String,
  /// Bytes per file piece and stream chunk, at most the 16 MiB body limit.
  pub chunk_size: u64,
  /// Chunks in flight per archive stream.
  pub stream_window: u32,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      base: BaseConfig::default(),
      metrics: MetricsConfig {
        metrics_name: "smaug-wings".to_string(),
        ..Default::default()
      },
      site: SiteConfig {
        site_url: "http://localhost:8080".parse().unwrap(),
      },
      token: "test-token".to_string(),
      runtime: Runtime::Docker,
      k8s_namespace: "smaug-servers".to_string(),
      docker_network: "smaug".to_string(),
      agent_image: "ghcr.io/profiidev/smaug/smaug-agent:main".to_string(),
      agent_network: "smaug_agents".to_string(),
      install_log_dir: "/var/lib/smaug/install-logs".into(),
      timezone: "UTC".to_string(),
      chunk_size: shared::msg::DEFAULT_CHUNK_SIZE,
      stream_window: shared::msg::DEFAULT_WINDOW,
      auth: AuthConfig::default(),
    }
  }
}

impl Config {
  #[instrument]
  pub fn parse() -> Self {
    let config = Figment::new()
      .merge(Serialized::defaults(Self::default()))
      .merge(Env::raw().global());

    let mut config: Self = config.extract().expect("Failed to parse configuration");
    // a chunk plus its request must fit the signed body limit
    config.chunk_size = config.chunk_size.clamp(1, 16 * 1024 * 1024);
    config.stream_window = config.stream_window.max(1);

    config
  }
}
