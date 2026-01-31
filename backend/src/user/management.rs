use argon2::password_hash::SaltString;
use axum::{
  Json, Router,
  extract::{FromRequest, FromRequestParts, Path},
  routing::{delete, get, post},
};
use base64::prelude::*;
use centaurus::{
  auth::pw::PasswordState,
  bail,
  db::init::Connection,
  error::{ErrorReportStatusExt, Result},
};
use http::StatusCode;
use rand::Rng;
use rsa::rand_core::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  auth::jwt_auth::JwtAuth,
  db::{
    DBTrait,
    settings::GeneralSettings,
    user::{SimpleGroupInfo, UserInfo},
  },
  mail::{state::Mailer, templates},
  permissions::{UserEdit, UserView},
  ws::state::{UpdateMessage, Updater},
};

const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789)(*&^%$#@!~";

pub fn router() -> Router {
  Router::new()
    .route("/", get(list_users))
    .route("/", post(create_user))
    .route("/", delete(delete_user))
    .route("/{uuid}", get(user_info))
    .route("/mail", get(mail_active))
    .route("/groups", get(list_groups_simple))
}

async fn list_users(_auth: JwtAuth<UserView>, db: Connection) -> Result<Json<Vec<UserInfo>>> {
  let users = db.user().list_users().await?;
  Ok(Json(users))
}

#[derive(Deserialize, FromRequestParts)]
#[from_request(via(Path))]
struct UserViewPath {
  uuid: Uuid,
}

async fn user_info(
  _auth: JwtAuth<UserView>,
  db: Connection,
  path: UserViewPath,
) -> Result<Json<UserInfo>> {
  let info = db.user().user_info(path.uuid).await?;
  let Some(info) = info else {
    bail!(NOT_FOUND, "User not found");
  };
  Ok(Json(info))
}

#[derive(Serialize)]
struct MailActiveResponse {
  active: bool,
}

async fn mail_active(_auth: JwtAuth<UserView>, mailer: Mailer) -> Result<Json<MailActiveResponse>> {
  let active = mailer.is_active().await;
  Ok(Json(MailActiveResponse { active }))
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct CreateUser {
  name: String,
  email: String,
  password: Option<String>,
}

#[derive(Serialize)]
struct CreateUserResponse {
  uuid: Uuid,
}

async fn create_user(
  _auth: JwtAuth<UserEdit>,
  db: Connection,
  updater: Updater,
  mailer: Mailer,
  state: PasswordState,
  req: CreateUser,
) -> Result<Json<CreateUserResponse>> {
  if db.user().try_get_user_by_email(&req.email).await?.is_some() {
    bail!(CONFLICT, "User with this email already exists");
  }

  let settings = db.settings().get_settings::<GeneralSettings>().await?;

  let password = if mailer.is_active().await {
    let mut rng = rand::rng();
    (0..12)
      .map(|_| {
        let idx = rng.random_range(0..CHARSET.len());
        CHARSET[idx] as char
      })
      .collect::<String>()
  } else if let Some(pw) = req.password {
    let bytes = BASE64_STANDARD.decode(pw).status(StatusCode::BAD_REQUEST)?;
    let pw_bytes = state.decrypt(&bytes).status(StatusCode::BAD_REQUEST)?;
    String::from_utf8_lossy(&pw_bytes).to_string()
  } else {
    bail!(
      BAD_REQUEST,
      "Password must be provided when mail service is not active"
    );
  };

  let salt = SaltString::generate(OsRng {}).to_string();
  let password_hash = state.pw_hash_raw(&salt, &password)?;

  let user_id = db
    .user()
    .create_user(req.name.clone(), req.email.clone(), password_hash, salt)
    .await?;
  if mailer.is_active().await {
    let subject = "Your new account";
    mailer
      .send_mail(
        req.name,
        req.email,
        subject.to_string(),
        templates::init_password(settings.site_url.as_str(), &password),
      )
      .await?;
  }
  updater.broadcast(UpdateMessage::Users).await;

  Ok(Json(CreateUserResponse { uuid: user_id }))
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct DeleteUserRequest {
  uuid: Uuid,
}

async fn delete_user(
  _auth: JwtAuth<UserEdit>,
  db: Connection,
  updater: Updater,
  data: DeleteUserRequest,
) -> Result<()> {
  db.user().delete_user(data.uuid).await?;
  updater.broadcast(UpdateMessage::Users).await;

  Ok(())
}

async fn list_groups_simple(
  _auth: JwtAuth<UserView>,
  db: Connection,
) -> Result<Json<Vec<SimpleGroupInfo>>> {
  let groups = db.group().list_groups_simple().await?;
  Ok(Json(groups))
}
