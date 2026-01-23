use axum::Router;
use centaurus::db::init::Connection;

mod nodes;

pub fn router() -> Router {
  Router::new().nest("/nodes", nodes::router())
}

pub async fn state(router: Router, db: &Connection) -> Router {
  nodes::state(router, db).await
}
