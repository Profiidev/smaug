use axum::{Json, Router, routing::get};
use centaurus::{db::init::Connection, error::Result};
use serde::Serialize;

use crate::{
  auth::oidc::OidcState,
  db::{DBTrait, settings::UserSettings},
  mail::state::Mailer,
};

pub fn router() -> Router {
  Router::new().route("/", get(config))
}

#[derive(Serialize)]
enum SSOType {
  Oidc,
  None,
}

#[derive(Serialize)]
struct AuthConfig {
  sso_type: SSOType,
  instant_redirect: bool,
  mail_enabled: bool,
}

async fn config(oidc: OidcState, mailer: Mailer, db: Connection) -> Result<Json<AuthConfig>> {
  let sso_type = if oidc.is_enabled().await {
    SSOType::Oidc
  } else {
    SSOType::None
  };

  let user_settings = db.settings().get_settings::<UserSettings>().await?;
  let mail_enabled = mailer.is_active().await;

  Ok(Json(AuthConfig {
    sso_type,
    instant_redirect: user_settings.sso_instant_redirect,
    mail_enabled,
  }))
}
