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

use shared::msg::Runtime;

use crate::{config::Config, docker::Docker, store::Store};

extern crate centaurus_wings as centaurus;

mod agent;
mod auth;
mod config;
mod docker;
mod error;
mod files;
mod logs;
mod metrics;
mod reconcile;
mod servers;
mod store;
mod streams;
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
  ws::router()
    .merge(servers::router())
    .merge(files::router())
    .merge(streams::router())
    .into()
}

async fn state(router: ApiRouter, config: Config) -> ApiRouter {
  let router = auth::state(router.into(), &config);
  let store = Store::new(&config).await;
  let docker = match config.runtime {
    Runtime::Docker => {
      let docker = Docker::new(&config).await.expect("Failed to set up docker");
      tokio::spawn(reconcile::run(store.clone(), docker.clone()));
      tokio::spawn(metrics::run(store.clone(), docker.clone()));
      Some(docker)
    }
    // TODO(wings): Kubernetes reconciler, until then GameServers are stored but never run
    Runtime::Kubernetes => None,
  };
  let streams = streams::Streams::new(store.clone(), &config);
  let router = store::state(router, store)
    .layer(Extension(docker))
    .layer(Extension(streams));
  router.layer(Extension(config)).into()
}
