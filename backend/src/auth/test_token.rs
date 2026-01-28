use axum::{Json, Router, routing::get};
use axum_extra::extract::CookieJar;

use crate::auth::{
  jwt_auth::JwtAuth,
  jwt_state::{JWT_COOKIE_NAME, JwtState},
};

pub fn router() -> Router {
  Router::new().route("/", get(test_token))
}

async fn test_token(
  auth: Option<JwtAuth>,
  mut cookies: CookieJar,
  jwt: JwtState,
) -> (CookieJar, Json<bool>) {
  if auth.is_none() {
    cookies = cookies.remove(jwt.create_cookie(JWT_COOKIE_NAME, String::new()));

    (cookies, Json(false))
  } else {
    (cookies, Json(true))
  }
}
