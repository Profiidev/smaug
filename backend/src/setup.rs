use argon2::password_hash::SaltString;
use axum::{
  Json, Router,
  extract::FromRequest,
  routing::{get, post},
};
use axum_extra::extract::CookieJar;
use centaurus::{auth::pw::PasswordState, bail, db::init::Connection, error::Result};
use rsa::rand_core::OsRng;
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{auth::jwt_state::JwtState, db::DBTrait};

pub fn router() -> Router {
  Router::new()
    .route("/", post(complete_setup))
    .route("/", get(is_setup))
}

pub async fn create_admin_group(db: &Connection) -> Result<()> {
  match db.setup().get_admin_group_id().await? {
    Some(id) => {
      info!("Admin group already created with ID {}", id);
      info!("Adding missing permissions to admin group");

      let existing_perms = db.group().get_group_permissions(id).await?;
      let all_perms = crate::permissions::permissions();
      let missing_perms: Vec<String> = all_perms
        .into_iter()
        .filter(|p| !existing_perms.contains(&p.to_string()))
        .map(|p| p.to_string())
        .collect();

      if !missing_perms.is_empty() {
        db.group()
          .add_permissions_to_group(id, missing_perms)
          .await?;
        info!("Added missing permissions to admin group");
      } else {
        info!("No missing permissions for admin group");
      }
    }
    None => {
      info!("Admin group not found, creating it with all permissions");

      let all_perms: Vec<String> = crate::permissions::permissions()
        .into_iter()
        .map(|p| p.to_string())
        .collect();

      let admin_group_id = db.group().create_group("Admin".to_string()).await?;
      db.group()
        .add_permissions_to_group(admin_group_id, all_perms)
        .await?;

      db.setup().set_admin_group_created(admin_group_id).await?;
      info!("Created admin group with ID {}", admin_group_id);
    }
  }

  Ok(())
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct SetupPayload {
  admin_username: String,
  admin_password: String,
  admin_email: String,
}

async fn complete_setup(
  db: Connection,
  jwt: JwtState,
  state: PasswordState,
  mut cookies: CookieJar,
  payload: SetupPayload,
) -> Result<CookieJar> {
  if db.setup().is_setup().await? {
    bail!(CONFLICT, "Setup has already been completed");
  }

  let Some(admin_group_id) = db.setup().get_admin_group_id().await? else {
    bail!(
      INTERNAL_SERVER_ERROR,
      "Admin group has not been created yet"
    );
  };

  let salt = SaltString::generate(OsRng {}).to_string();
  let hash = state.pw_hash(&salt, &payload.admin_password)?;

  let admin = db
    .user()
    .create_user(payload.admin_username, payload.admin_email, hash, salt)
    .await?;
  db.group()
    .add_user_to_groups(admin, vec![admin_group_id])
    .await?;

  db.setup().mark_completed().await?;
  info!("Setup completed, created admin user with ID {}", admin);

  let cookie = jwt.create_token(admin)?;
  cookies = cookies.add(cookie);
  info!("Created post setup login token for admin user");

  Ok(cookies)
}

#[derive(Serialize)]
struct IsSetupResponse {
  is_setup: bool,
  db_backend: String,
}

async fn is_setup(db: Connection) -> Result<Json<IsSetupResponse>> {
  let db_backend = match db.0.get_database_backend() {
    sea_orm::DatabaseBackend::Postgres => "PostgreSQL",
    sea_orm::DatabaseBackend::MySql => "MySQL",
    sea_orm::DatabaseBackend::Sqlite => "SQLite",
  }
  .to_string();

  Ok(Json(IsSetupResponse {
    is_setup: db.setup().is_setup().await?,
    db_backend,
  }))
}
