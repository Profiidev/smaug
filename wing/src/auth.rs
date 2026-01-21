use axum::{Extension, Router, extract::FromRequestParts};
use centaurus::{error::ErrorReport, state::extract::StateExtractExt};
use http::request::Parts;
use shared::auth::SignData;
use tracing::info;

use crate::config::Config;

pub struct Auth {
  pub timestamp: String,
}

#[derive(FromRequestParts, Clone)]
#[from_request(via(Extension))]
pub struct WingToken(pub String);

impl<S: Sync> FromRequestParts<S> for Auth {
  type Rejection = ErrorReport;

  async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
    let token = parts.extract_state::<WingToken>().await.0;
    let timestamp = SignData::validate_header_map(&parts.headers, &token, None)?;

    info!("Authenticated wing request with timestamp {}", timestamp);

    Ok(Auth { timestamp })
  }
}

pub fn state(router: Router, config: &Config) -> Router {
  router.layer(Extension(WingToken(config.token.clone())))
}
