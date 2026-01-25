use axum::{Extension, Router};
use centaurus::init::{
  axum::{listener_setup, run_app},
  logging::init_logging,
  router::base_router,
};
#[cfg(debug_assertions)]
use dotenv::dotenv;
use tracing::info;

use crate::config::Config;

mod auth;
mod config;
mod dummy;
mod ws;

#[tokio::main]
async fn main() {
  #[cfg(debug_assertions)]
  dotenv().ok();

  let config = Config::parse();
  init_logging(&config.base);

  let listener = listener_setup(config.base.port).await;

  let mut router = api_router();
  router = base_router(router, &config.base, &config.metrics).await;
  let app = state(router, config);

  info!("Starting application");
  run_app(listener, app).await;
}

fn api_router() -> Router {
  dummy::router().merge(ws::router())
}

fn state(router: Router, config: Config) -> Router {
  let router = auth::state(router, &config);
  let router = dummy::state(router);
  router.layer(Extension(config))
}
