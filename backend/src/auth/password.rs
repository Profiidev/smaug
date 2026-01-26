use axum::{
  Json, Router,
  extract::FromRequest,
  routing::{get, post},
};
use axum_extra::extract::CookieJar;
use centaurus::{auth::pw::PasswordState, bail, db::init::Connection, error::Result};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
  auth::{jwt_state::JwtState, res::TokenRes},
  db::DBTrait,
};

pub fn router() -> Router {
  Router::new()
    .route("/", get(key))
    .route("/", post(authenticate))
}

#[derive(Serialize)]
struct KeyRes {
  key: String,
}

async fn key(state: PasswordState) -> Json<KeyRes> {
  Json(KeyRes { key: state.pub_key })
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct LoginReq {
  email: String,
  password: String,
}

async fn authenticate(
  state: PasswordState,
  jwt: JwtState,
  db: Connection,
  mut cookies: CookieJar,
  req: LoginReq,
) -> Result<(CookieJar, TokenRes)> {
  let user = db.user().get_user_by_email(&req.email).await?;
  let hash = state.pw_hash(&user.salt, &req.password)?;

  if hash != user.password {
    bail!(UNAUTHORIZED, "Invalid email or password");
  }

  let cookie = jwt.create_token(user.id)?;
  cookies = cookies.add(cookie);
  debug!("User logged in: {}", user.id);

  Ok((cookies, TokenRes))
}
