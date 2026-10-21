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
use kube::CustomResourceExt;
use tracing::info;

use crate::{config::Config, store::Store};

extern crate centaurus_wings as centaurus;

mod auth;
mod config;
mod servers;
mod store;
mod ws;

#[tokio::main]
async fn main() {
  #[cfg(debug_assertions)]
  dotenv().ok();

  // `wings crd` prints the GameServer CRD for the admin to apply
  if std::env::args().nth(1).as_deref() == Some("crd") {
    let crd = serde_json::to_string_pretty(&store::GameServer::crd()).unwrap();
    println!("{crd}");
    return;
  }

  let config = Config::parse();
  init_logging(config.base.log_level);

  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();

  let listener = listener_setup(config.base.port).await;
  let app = build_router(router, state, config).await;

  info!("Starting application");
  run_app_connect_info(listener, app).await;
}

fn router(_limiter: &mut RateLimiter) -> ApiRouter {
  ws::router().merge(servers::router()).into()
}

async fn state(router: ApiRouter, config: Config) -> ApiRouter {
  let router = auth::state(router.into(), &config);
  let router = store::state(router, Store::new(&config).await);
  router.layer(Extension(config)).into()
}
