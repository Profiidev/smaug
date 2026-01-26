use axum::{Router, routing::post};
use axum_extra::extract::CookieJar;
use centaurus::{
  db::init::Connection,
  error::{ErrorReportStatusExt, Result},
};
use chrono::DateTime;
use http::StatusCode;
use tracing::debug;

use crate::{
  auth::{
    jwt_auth::JwtAuth,
    jwt_state::{JWT_COOKIE_NAME, JwtInvalidState, JwtState},
    res::TokenRes,
  },
  db::DBTrait,
};

pub fn router() -> Router {
  Router::new().route("/", post(logout))
}

async fn logout(
  auth: JwtAuth,
  db: Connection,
  mut cookies: CookieJar,
  state: JwtInvalidState,
  jwt: JwtState,
) -> Result<(CookieJar, TokenRes)> {
  let cookie = cookies
    .get(JWT_COOKIE_NAME)
    .status_context(StatusCode::UNAUTHORIZED, "Missing auth cookie")?;
  let mut count = state.count.lock().await;

  db.invalid_jwt()
    .invalidate_jwt(
      cookie.value().to_string(),
      DateTime::from_timestamp(auth.exp, 0)
        .status_context(StatusCode::INTERNAL_SERVER_ERROR, "invalid timestamp")?,
      &mut count,
    )
    .await?;

  debug!("User logged out: {}", auth.user_id);
  cookies = cookies.remove(jwt.create_cookie(JWT_COOKIE_NAME, String::new()));

  Ok((cookies, TokenRes))
}
