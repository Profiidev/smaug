use aide::axum::ApiRouter;
use axum::Extension;
use centaurus::{
  backend::{
    init::{listener_setup, run_app_connect_info},
    middleware::rate_limiter::RateLimiter,
    router::build_router,
  },
  logging::init_logging,
};
#[cfg(debug_assertions)]
use dotenvy::dotenv;
use tracing::info;

use crate::config::Config;

extern crate centaurus_wings as centaurus;

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
  run_app_connect_info(listener, app).await;
}

fn router(_limiter: &mut RateLimiter) -> ApiRouter {
  dummy::router().merge(ws::router()).into()
}

async fn state(router: ApiRouter, config: Config) -> ApiRouter {
  let router = auth::state(router.into(), &config);
  let router = dummy::state(router);
  router.layer(Extension(config)).into()
}
