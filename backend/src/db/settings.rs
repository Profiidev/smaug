use axum::{Json, extract::FromRequest};
use centaurus::error::ErrorReport;
use entity::settings;
use sea_orm::{IntoActiveModel, Set, prelude::*};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

pub struct SettingsTable<'db> {
  db: &'db DatabaseConnection,
}

impl<'db> SettingsTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  pub async fn get_settings<S: Settings>(&self) -> Result<S, ErrorReport> {
    let res = settings::Entity::find_by_id(S::id()).one(self.db).await?;
    let Some(model) = res else {
      return Ok(S::default());
    };

    Ok(serde_json::from_str(&model.content)?)
  }

  pub async fn save_settings<S: Settings>(&self, settings: &S) -> Result<(), ErrorReport> {
    let content = serde_json::to_string(settings)?;

    match settings::Entity::find_by_id(S::id()).one(self.db).await? {
      Some(m) => {
        let mut am = m.into_active_model();
        am.content = Set(content);
        am.update(self.db).await?;
      }
      None => {
        let model = settings::Model {
          id: S::id(),
          content,
        };

        model.into_active_model().insert(self.db).await?;
      }
    };

    Ok(())
  }
}

pub trait Settings: Serialize + DeserializeOwned + Default {
  fn id() -> i32;
}

macro_rules! settings {
  ($type:ident, $id:expr) => {
    impl Settings for $type {
      fn id() -> i32 {
        $id
      }
    }
  };
}

settings!(GeneralSettings, 1);
#[derive(Serialize, Deserialize, FromRequest)]
#[from_request(via(Json))]
pub struct GeneralSettings {
  pub site_url: Url,
}

impl Default for GeneralSettings {
  fn default() -> Self {
    Self {
      site_url: Url::parse("http://localhost:8000").unwrap(),
    }
  }
}

settings!(UserSettings, 2);
#[derive(Serialize, Deserialize, FromRequest)]
#[from_request(via(Json))]
pub struct UserSettings {
  pub oidc: Option<OidcSettings>,
  pub sso_instant_redirect: bool,
  pub sso_create_user: bool,
}

impl Default for UserSettings {
  fn default() -> Self {
    Self {
      oidc: None,
      sso_instant_redirect: true,
      sso_create_user: true,
    }
  }
}

#[derive(Serialize, Deserialize)]
pub struct OidcSettings {
  pub issuer: Url,
  pub client_id: String,
  pub client_secret: String,
  pub scopes: Vec<String>,
}

settings!(MailSettings, 3);
#[derive(Serialize, Deserialize, FromRequest, Default)]
#[from_request(via(Json))]
pub struct MailSettings {
  pub smtp: Option<SmtpSettings>,
}

#[derive(Serialize, Deserialize)]
pub struct SmtpSettings {
  pub server: String,
  pub port: u16,
  pub username: String,
  pub password: String,
  pub from_address: String,
  pub from_name: String,
  pub use_tls: bool,
}
