use axum::Router;

mod nodes;

pub fn router() -> Router {
  Router::new().nest("/nodes", nodes::router())
}
