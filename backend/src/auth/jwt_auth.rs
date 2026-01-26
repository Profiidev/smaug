use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use centaurus::{
  auth::jwt::jwt_from_request, bail, db::init::Connection, error::ErrorReport,
  state::extract::StateExtractExt,
};
use http::request::Parts;
use uuid::Uuid;

use crate::{
  auth::jwt_state::{JWT_COOKIE_NAME, JwtState},
  db::DBTrait,
};

#[derive(Debug)]
pub struct JwtAuth {
  pub user_id: Uuid,
  pub exp: i64,
}

impl<S: Sync> FromRequestParts<S> for JwtAuth {
  type Rejection = ErrorReport;

  async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
    let token = jwt_from_request(parts, JWT_COOKIE_NAME).await?;

    let state = parts.extract_state::<JwtState>().await;
    let db = parts.extract_state::<Connection>().await;

    let Ok(valid) = db.invalid_jwt().is_token_valid(&token).await else {
      bail!("failed to validate jwt");
    };
    if !valid {
      bail!(UNAUTHORIZED, "token is invalidated");
    }

    let Ok(claims) = state.validate_token(&token) else {
      bail!(UNAUTHORIZED, "invalid token");
    };

    Ok(JwtAuth {
      user_id: claims.sub,
      exp: claims.exp,
    })
  }
}

impl<S: Sync> OptionalFromRequestParts<S> for JwtAuth {
  type Rejection = ErrorReport;

  async fn from_request_parts(
    parts: &mut Parts,
    state: &S,
  ) -> Result<Option<Self>, Self::Rejection> {
    match <Self as FromRequestParts<S>>::from_request_parts(parts, state).await {
      Ok(auth) => Ok(Some(auth)),
      Err(_) => Ok(None),
    }
  }
}
