use axum::{Json, Router, routing::get};
use centaurus::{db::init::Connection, error::Result};
use serde::Serialize;

use crate::{
  auth::oidc::OidcState,
  db::{DBTrait, settings::UserSettings},
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
struct SSOConfig {
  sso_type: SSOType,
  instant_redirect: bool,
}

async fn config(oidc: OidcState, db: Connection) -> Result<Json<SSOConfig>> {
  let sso_type = if oidc.is_enabled().await {
    SSOType::Oidc
  } else {
    SSOType::None
  };

  let user_settings = db.settings().get_settings::<UserSettings>().await?;

  Ok(Json(SSOConfig {
    sso_type,
    instant_redirect: user_settings.sso_instant_redirect,
  }))
}
