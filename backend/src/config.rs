use aide::OperationIo;
use axum::{Extension, extract::FromRequestParts};
use centaurus::{
  Config,
  backend::{
    auth::settings::{AuthConfig, UserSettings},
    config::{BaseConfig, MetricsConfig, SiteConfig},
  },
  db::config::DBConfig,
  mail::MailSettings,
  storage::StorageConfig,
};
use figment::{
  Figment,
  providers::{Env, Serialized},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Deserialize, Serialize, Clone, FromRequestParts, Config, OperationIo)]
#[from_request(via(Extension))]
pub struct Config {
  #[base]
  #[serde(flatten)]
  pub base: BaseConfig,
  #[serde(flatten)]
  pub db: DBConfig,
  #[metrics]
  #[serde(flatten)]
  pub metrics: MetricsConfig,
  #[site]
  #[serde(flatten)]
  pub site: SiteConfig,
  #[auth]
  #[serde(flatten)]
  pub auth: AuthConfig,
  #[mail]
  #[serde(flatten)]
  pub mail: MailSettings,
  #[oidc]
  #[serde(flatten)]
  pub oidc: UserSettings,
  /// Backups, `STORAGE_PATH` or `S3_*`.
  #[serde(flatten)]
  pub storage: StorageConfig,

  pub db_url: String,
  pub admin_group: String,
  /// Allow plain ws/http connections to wings nodes. Signed commands are then
  /// sent unencrypted, so this is only on by default in debug builds.
  pub allow_insecure_wings: bool,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      base: BaseConfig::default(),
      db: DBConfig::default(),
      site: SiteConfig::default(),
      mail: MailSettings::default(),
      oidc: UserSettings::default(),
      storage: StorageConfig {
        storage_path: "data/storage".to_string(),
        ..Default::default()
      },
      db_url: "".to_string(),
      admin_group: "Admin".to_string(),
      allow_insecure_wings: cfg!(debug_assertions),
      metrics: MetricsConfig {
        metrics_name: "smaug".to_string(),
        ..Default::default()
      },
      auth: AuthConfig {
        auth_pepper: "__smaug_PEPPER__".to_string(),
        auth_jwt_expiration: 60 * 60 * 24, // 1 days
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

    let mut config: Self = config.extract().expect("Failed to parse configuration");

    if config.db_url.is_empty() {
      panic!("Database URL is not set");
    }

    config.storage.validate();

    if config.db_url.starts_with("sqlite") {
      config.db.validate_sqlite();
    }

    config
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn default_values() {
    let config = Config::default();
    assert_eq!(config.db_url, "");
    assert_eq!(config.admin_group, "Admin");
    assert_eq!(config.metrics.metrics_name, "smaug");
    assert_eq!(config.auth.auth_pepper, "__smaug_PEPPER__");
    assert_eq!(config.auth.auth_jwt_expiration, 60 * 60 * 24);
    assert_eq!(config.storage.storage_path, "data/storage");
  }

  #[test]
  #[should_panic(expected = "Database URL is not set")]
  fn parse_requires_db_url() {
    // ponytail: mutates the process env; safe under nextest (one process per test),
    // and no other unit test reads DB_URL
    unsafe { std::env::set_var("DB_URL", "") };
    Config::parse();
  }
}
