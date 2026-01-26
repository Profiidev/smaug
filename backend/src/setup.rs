use argon2::password_hash::SaltString;
use axum::{Json, Router, extract::FromRequest, routing::post};
use centaurus::{auth::pw::PasswordState, bail, db::init::Connection, error::Result};
use rsa::rand_core::OsRng;
use serde::Deserialize;

use crate::db::DBTrait;

pub fn router() -> Router {
  Router::new().route("/", post(complete_setup))
}

pub async fn create_admin_group(db: &Connection) -> Result<()> {
  match db.setup().get_admin_group_id().await? {
    Some(id) => {
      tracing::info!("Admin group already created with ID {}", id);
      tracing::info!("Adding missing permissions to admin group");

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
        tracing::info!("Added missing permissions to admin group");
      } else {
        tracing::info!("No missing permissions for admin group");
      }
    }
    None => {
      tracing::info!("Admin group not found, creating it with all permissions");

      let all_perms: Vec<String> = crate::permissions::permissions()
        .into_iter()
        .map(|p| p.to_string())
        .collect();

      let admin_group_id = db.group().create_group("Admin".to_string()).await?;
      db.group()
        .add_permissions_to_group(admin_group_id, all_perms)
        .await?;

      db.setup().set_admin_group_created(admin_group_id).await?;
      tracing::info!("Created admin group with ID {}", admin_group_id);
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

async fn complete_setup(db: Connection, state: PasswordState, payload: SetupPayload) -> Result<()> {
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

  Ok(())
}
