use axum::{
  Json, Router,
  routing::{get, post},
};
use centaurus::{db::init::Connection, error::Result};

use crate::{
  auth::jwt_auth::JwtAuth,
  db::{
    DBTrait,
    settings::{MailSettings, Settings, UserSettings},
  },
  permissions::{SettingsEdit, SettingsView},
};

pub fn router() -> Router {
  Router::new()
    .route("/user", get(get_settings::<UserSettings>))
    .route("/user", post(save_settings::<UserSettings>))
    .route("/mail", get(get_settings::<MailSettings>))
    .route("/mail", post(save_settings::<MailSettings>))
}

async fn get_settings<S: Settings>(
  _auth: JwtAuth<SettingsView>,
  db: Connection,
) -> Result<Json<S>> {
  Ok(Json(db.settings().get_settings::<S>().await?))
}

async fn save_settings<S: Settings>(
  _auth: JwtAuth<SettingsEdit>,
  db: Connection,
  settings: S,
) -> Result<()> {
  db.settings().save_settings(&settings).await
}
