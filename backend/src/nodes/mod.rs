use aide::axum::ApiRouter;
use axum::Extension;
use centaurus::db::init::Connection;

use crate::utils::Updater;

mod auth;
mod connection;
mod management;
mod state;

pub use state::Wings;

pub fn router() -> ApiRouter {
  management::router()
}

pub async fn state(
  router: ApiRouter,
  db: &Connection,
  updater: Updater,
  allow_insecure: bool,
) -> ApiRouter {
  router.layer(Extension(
    Wings::new(db, updater, allow_insecure)
      .await
      .expect("Failed to create Wings state"),
  ))
}
