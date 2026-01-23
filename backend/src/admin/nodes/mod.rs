use axum::{Extension, Router};
use centaurus::db::init::Connection;

use crate::admin::nodes::state::Wings;

mod auth;
mod connection;
mod management;
mod state;

pub fn router() -> Router {
  management::router()
}

pub async fn state(router: Router, db: &Connection) -> Router {
  router.layer(Extension(
    Wings::new(db).await.expect("Failed to create Wings state"),
  ))
}
