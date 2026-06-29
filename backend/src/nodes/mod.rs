use aide::axum::ApiRouter;
use axum::Extension;
use centaurus::db::init::Connection;

use crate::{nodes::state::Wings, utils::Updater};

mod auth;
mod connection;
mod management;
mod state;

pub fn router() -> ApiRouter {
  management::router()
}

pub async fn state(router: ApiRouter, db: &Connection, updater: Updater) -> ApiRouter {
  router.layer(Extension(
    Wings::new(db, updater)
      .await
      .expect("Failed to create Wings state"),
  ))
}
