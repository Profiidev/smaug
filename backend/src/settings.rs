use axum::{
  Json, Router,
  routing::{get, post},
};
use centaurus::{
  db::init::Connection,
  error::{ErrorReportStatusExt, Result},
};
use http::StatusCode;

use crate::{
  auth::{jwt_auth::JwtAuth, oidc::OidcState},
  db::{
    DBTrait,
    settings::{GeneralSettings, MailSettings, Settings, UserSettings},
  },
  permissions::{SettingsEdit, SettingsView},
  ws::state::{UpdateMessage, Updater},
};

pub fn router() -> Router {
  Router::new()
    .route("/general", get(get_settings::<GeneralSettings>))
    .route("/general", post(save_settings::<GeneralSettings>))
    .route("/user", get(get_settings::<UserSettings>))
    .route("/user", post(save_user_settings))
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
  updater: Updater,
  settings: S,
) -> Result<()> {
  db.settings().save_settings(&settings).await?;
  updater.broadcast(UpdateMessage::Settings).await;
  Ok(())
}

async fn save_user_settings(
  _auth: JwtAuth<SettingsEdit>,
  db: Connection,
  state: OidcState,
  updater: Updater,
  settings: UserSettings,
) -> Result<()> {
  if let Some(oidc_settings) = &settings.oidc {
    state.try_init(oidc_settings).await.status_context(
      StatusCode::NOT_ACCEPTABLE,
      "Failed to initialize OIDC state",
    )?;
  } else {
    state.deactivate().await;
  }

  db.settings().save_settings(&settings).await?;
  updater.broadcast(UpdateMessage::Settings).await;

  Ok(())
}
