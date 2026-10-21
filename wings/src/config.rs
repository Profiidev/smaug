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
  /// Docker: server data lives in `<data_dir>/<uuid>/files`. Must be the same
  /// path on the host when wings itself runs in a container.
  pub data_dir: PathBuf,
  pub docker_network: String,
  /// `TZ` of every server container.
  pub timezone: String,
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
      data_dir: "/var/lib/smaug/volumes".into(),
      docker_network: "smaug".to_string(),
      timezone: "UTC".to_string(),
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

    let config: Self = config.extract().expect("Failed to parse configuration");

    config
  }
}
