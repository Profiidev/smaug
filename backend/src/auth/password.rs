use axum::{Json, Router, routing::get};
use centaurus::auth::pw::PasswordState;
use serde::Serialize;

pub fn router() -> Router {
  Router::new().route("/", get(key))
}

#[derive(Serialize)]
struct KeyRes {
  key: String,
}

async fn key(state: PasswordState) -> Json<KeyRes> {
  Json(KeyRes { key: state.pub_key })
}
