use argon2::password_hash::SaltString;
use axum::{
  Json, Router,
  extract::{FromRequest, FromRequestParts, Path},
  routing::{delete, get, post, put},
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
    user::{DetailUserInfo, SimpleGroupInfo, UserInfo},
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
    .route("/", put(edit_user))
    .route("/{uuid}", get(user_info))
    .route("/mail", get(mail_active))
    .route("/groups", get(list_groups_simple))
    .route("/avatar", delete(reset_user_avatar))
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
) -> Result<Json<DetailUserInfo>> {
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

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct UserEditReq {
  uuid: Uuid,
  name: String,
  groups: Vec<Uuid>,
}

async fn edit_user(
  auth: JwtAuth<UserEdit>,
  db: Connection,
  updater: Updater,
  req: UserEditReq,
) -> Result<()> {
  let self_permissions = db.group().get_user_permissions(auth.user_id).await?;
  let target_permissions = db
    .group()
    .get_groups_permissions(req.groups.clone())
    .await?;

  if target_permissions
    .iter()
    .any(|p| !self_permissions.contains(p))
  {
    tracing::warn!(
      "User {:?} tried to assign permissions {:?} which they do not have themselves {:?}",
      req.uuid,
      target_permissions,
      self_permissions
    );

    bail!(
      FORBIDDEN,
      "Cannot assign permissions that the editor does not have"
    );
  }

  let Some(admin_group) = db.setup().get_admin_group_id().await? else {
    bail!(INTERNAL_SERVER_ERROR, "Admin group is not set up");
  };

  if !req.groups.contains(&admin_group) && db.group().is_last_admin(admin_group, req.uuid).await? {
    bail!(CONFLICT, "Cannot remove the last user from the admin group");
  }

  db.user().edit_user(req.uuid, req.name, req.groups).await?;
  updater.broadcast(UpdateMessage::Users).await;

  Ok(())
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct UserAvatarResetRequest {
  uuid: Uuid,
}

async fn reset_user_avatar(
  _auth: JwtAuth<UserEdit>,
  db: Connection,
  updater: Updater,
  req: UserAvatarResetRequest,
) -> Result<()> {
  db.user().reset_avatar(req.uuid).await?;
  updater.broadcast(UpdateMessage::Users).await;

  Ok(())
}
