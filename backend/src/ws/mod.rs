use axum::{Extension, Router};

use crate::ws::state::{UpdateState, Updater};

pub mod state;
mod updater;

pub fn router() -> Router {
  Router::new().merge(updater::router())
}

pub async fn state(router: Router) -> (Router, Updater) {
  let (state, updater) = UpdateState::init().await;

  (
    router
      .layer(Extension(state))
      .layer(Extension(updater.clone())),
    updater,
  )
}
