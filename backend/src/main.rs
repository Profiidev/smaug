use axum::{Extension, Router};
use centaurus::{
  db::init::init_db,
  init::{
    axum::{listener_setup, run_app},
    logging::init_logging,
    router::base_router,
  },
};
#[cfg(debug_assertions)]
use dotenv::dotenv;
use tracing::info;

use crate::config::Config;

mod admin;
mod config;
mod db;

#[tokio::main]
async fn main() {
  #[cfg(debug_assertions)]
  dotenv().ok();

  let config = Config::parse();
  init_logging(&config.base);

  let listener = listener_setup(config.base.port).await;

  let mut router = api_router();
  router = base_router(router, &config.base, &config.metrics).await;
  let app = state(router, config).await;

  info!("Starting application");
  run_app(listener, app).await;
}

fn api_router() -> Router {
  Router::new().nest("/admin", admin::router())
}

async fn state(router: Router, config: Config) -> Router {
  let db = init_db::<migration::Migrator>(&config.db, &config.db_url).await;
  router.layer(Extension(db)).layer(Extension(config))
}
