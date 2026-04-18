use axum::{Extension, Router};
use centaurus::{
  backend::{
    init::{listener_setup, run_app},
    rate_limiter::RateLimiter,
    router::build_router,
  },
  logging::init_logging,
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
  init_logging(config.base.log_level);

  let listener = listener_setup(config.base.port).await;

  let app = build_router(router, state, config).await;

  info!("Starting application");
  run_app(listener, app).await;
}

fn router(_limiter: &mut RateLimiter) -> Router {
  dummy::router().merge(ws::router())
}

async fn state(router: Router, config: Config) -> Router {
  let router = auth::state(router, &config);
  let router = dummy::state(router);
  router.layer(Extension(config))
}
