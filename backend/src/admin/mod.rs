use axum::Router;
use centaurus::db::init::Connection;

use crate::ws::state::Updater;

mod nodes;

pub fn router() -> Router {
  Router::new().nest("/nodes", nodes::router())
}

pub async fn state(router: Router, db: &Connection, updater: Updater) -> Router {
  nodes::state(router, db, updater).await
}
