use centaurus::config::{BaseConfig, MetricsConfig};
use figment::{
  Figment,
  providers::{Env, Serialized},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
  #[serde(flatten)]
  pub base: BaseConfig,
  #[serde(flatten)]
  pub metrics: MetricsConfig,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      base: BaseConfig::default(),
      metrics: MetricsConfig {
        metrics_name: "smaug-wing".to_string(),
        ..Default::default()
      },
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
