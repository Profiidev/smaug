use axum::{Extension, Router};

use crate::admin::nodes::state::Wings;

mod auth;
mod connection;
mod management;
mod state;

pub fn router() -> Router {
  management::router()
}

pub fn state(router: Router) -> Router {
  router.layer(Extension(Wings::default()))
}
