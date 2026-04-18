use centaurus::{
  Config,
  backend::config::{BaseConfig, MetricsConfig, SiteConfig},
};
use figment::{
  Figment,
  providers::{Env, Serialized},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Deserialize, Serialize, Clone, Config)]
pub struct Config {
  #[serde(flatten)]
  #[base]
  pub base: BaseConfig,
  #[serde(flatten)]
  #[metrics]
  pub metrics: MetricsConfig,
  #[site]
  pub site: SiteConfig,

  pub token: String,
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
