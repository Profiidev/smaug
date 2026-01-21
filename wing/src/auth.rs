use axum::{Extension, Router, extract::FromRequestParts};
use centaurus::{
  bail,
  error::{ErrorReport, ErrorReportStatusExt},
  state::extract::StateExtractExt,
};
use hmac::{Hmac, Mac};
use http::{StatusCode, request::Parts};
use sha3::Sha3_512;
use tracing::info;

use crate::config::Config;

type HmacSha3_512 = Hmac<Sha3_512>;

pub struct Auth {
  pub timestamp: String,
}

#[derive(FromRequestParts, Clone)]
#[from_request(via(Extension))]
pub struct WingToken(pub String);

impl<S: Sync> FromRequestParts<S> for Auth {
  type Rejection = ErrorReport;

  async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
    let headers = &parts.headers;
    let nonce = headers
      .get("x-wing-nonce")
      .status_context(StatusCode::UNAUTHORIZED, "Missing x-wing-nonce header")?
      .to_str()
      .status_context(StatusCode::UNAUTHORIZED, "Invalid x-wing-nonce header")?;
    let timestamp = headers
      .get("x-wing-timestamp")
      .status_context(StatusCode::UNAUTHORIZED, "Missing x-wing-timestamp header")?
      .to_str()
      .status_context(StatusCode::UNAUTHORIZED, "Invalid x-wing-timestamp header")?
      .to_string();
    let signature = headers
      .get("x-wing-signature")
      .status_context(StatusCode::UNAUTHORIZED, "Missing x-wing-signature header")?
      .to_str()
      .status_context(StatusCode::UNAUTHORIZED, "Invalid x-wing-signature header")?
      .to_string();

    let msg = format!("{}{}", nonce, timestamp);

    let token = parts.extract_state::<WingToken>().await.0;

    let mut mac = HmacSha3_512::new_from_slice(token.as_bytes())?;
    mac.update(msg.as_bytes());
    let correct_signature = hex::encode(mac.finalize().into_bytes());

    if signature != correct_signature {
      bail!(UNAUTHORIZED, "Invalid wing signature");
    }

    info!("Authenticated wing request with timestamp {}", timestamp);

    Ok(Auth { timestamp })
  }
}

pub fn state(router: Router, config: &Config) -> Router {
  router.layer(Extension(WingToken(config.token.clone())))
}
