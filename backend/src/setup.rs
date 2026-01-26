use argon2::password_hash::SaltString;
use axum::{Json, Router, extract::FromRequest, routing::post};
use centaurus::{auth::pw::PasswordState, bail, db::init::Connection, error::Result};
use rsa::rand_core::OsRng;
use serde::Deserialize;

use crate::db::DBTrait;

pub fn router() -> Router {
  Router::new().route("/", post(complete_setup))
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

  let admin = db.user()
    .create_user(payload.admin_username, payload.admin_email, hash, salt)
    .await?;
  db.group().add_user_to_groups(admin, vec![admin_group_id]).await?;

  db.setup().mark_completed().await?;

  Ok(())
}
