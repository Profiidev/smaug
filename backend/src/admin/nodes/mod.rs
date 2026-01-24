use axum::{Extension, Router};
use centaurus::db::init::Connection;

use crate::{admin::nodes::state::Wings, ws::state::Updater};

mod auth;
mod connection;
mod management;
mod state;

pub fn router() -> Router {
  management::router()
}

pub async fn state(router: Router, db: &Connection, updater: Updater) -> Router {
  router.layer(Extension(
    Wings::new(db, updater)
      .await
      .expect("Failed to create Wings state"),
  ))
}
