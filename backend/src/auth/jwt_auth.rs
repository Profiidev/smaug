use std::marker::PhantomData;

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
  permissions::{NoPerm, Permission},
};

#[derive(Debug)]
pub struct JwtAuth<P: Permission = NoPerm> {
  pub user_id: Uuid,
  pub exp: i64,
  _perm: PhantomData<P>,
}

impl<S: Sync, P: Permission> FromRequestParts<S> for JwtAuth<P> {
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
      tracing::error!("invalid token claims for token: {}", token);
      bail!(UNAUTHORIZED, "invalid token");
    };

    // Empty permission means no permission required
    if !P::name().is_empty()
      && !db
        .group()
        .user_hash_permissions(claims.sub, P::name())
        .await?
    {
      bail!(FORBIDDEN, "insufficient permissions");
    }

    Ok(JwtAuth {
      user_id: claims.sub,
      exp: claims.exp,
      _perm: PhantomData,
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
