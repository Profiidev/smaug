use std::io::Cursor;

use axum::{Json, Router, extract::FromRequest, routing::post};
use base64::prelude::*;
use centaurus::{bail, db::init::Connection, error::Result};
use image::{ImageFormat, imageops::FilterType};
use serde::Deserialize;

use crate::{
  auth::jwt_auth::JwtAuth,
  db::DBTrait,
  ws::state::{UpdateMessage, Updater},
};

pub fn router() -> Router {
  Router::new()
    .route("/update", post(update_account))
    .route("/avatar", post(update_avatar))
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct AccountUpdate {
  username: String,
}

async fn update_account(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  data: AccountUpdate,
) -> Result<()> {
  db.user()
    .update_user_name(auth.user_id, data.username)
    .await?;
  updater.broadcast(UpdateMessage::Users).await;
  Ok(())
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct AvatarUpdate {
  avatar: String,
}

async fn update_avatar(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  data: AvatarUpdate,
) -> Result<()> {
  if data.avatar.len() > 10 * 1024 * 1024 {
    bail!(PAYLOAD_TOO_LARGE, "Avatar size exceeds 10MB limit");
  }

  let raw_data = BASE64_STANDARD.decode(data.avatar)?;
  let img = image::load_from_memory(&raw_data)?;
  let img = img.resize_exact(128, 128, FilterType::Lanczos3);

  let mut buf = Cursor::new(Vec::new());
  img.write_to(&mut buf, ImageFormat::WebP)?;
  let avatar = BASE64_STANDARD.encode(buf.into_inner());

  db.user().update_user_avatar(auth.user_id, avatar).await?;
  updater.broadcast(UpdateMessage::Users).await;
  Ok(())
}
