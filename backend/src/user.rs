use axum::{Json, Router, routing::get};
use centaurus::{db::init::Connection, error::Result};
use serde::Serialize;
use uuid::Uuid;

use crate::{auth::jwt_auth::JwtAuth, db::DBTrait};

pub fn router() -> Router {
  Router::new().route("/", get(info))
}

#[derive(Serialize)]
struct UserInfo {
  uuid: Uuid,
  name: String,
  email: String,
  permissions: Vec<String>,
}

async fn info(auth: JwtAuth, db: Connection) -> Result<Json<UserInfo>> {
  let user = db.user().get_user_by_id(auth.user_id).await?;
  let permissions = db.group().get_user_permissions(auth.user_id).await?;

  Ok(Json(UserInfo {
    uuid: user.id,
    name: user.name,
    email: user.email,
    permissions,
  }))
}
