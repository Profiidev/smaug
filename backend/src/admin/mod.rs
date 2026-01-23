use axum::Router;

mod nodes;

pub fn router() -> Router {
  Router::new().nest("/nodes", nodes::router())
}

pub fn state(router: Router) -> Router {
  nodes::state(router)
}
