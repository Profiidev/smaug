use aide::axum::ApiRouter;
use centaurus::db::init::Connection;

use crate::utils::Updater;

mod auth;
mod connection;
mod management;
mod state;

pub use state::{Wings, call};

pub fn router() -> ApiRouter {
  management::router()
}

pub async fn state(db: &Connection, updater: Updater, allow_insecure: bool) -> Wings {
  Wings::new(db, updater, allow_insecure)
    .await
    .expect("Failed to create Wings state")
}
