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

mod config;
mod db;
mod dummy;
mod wing;

#[tokio::main]
async fn main() {
  #[cfg(debug_assertions)]
  dotenv().ok();

  let config = Config::parse();
  init_logging(&config.base);

  wing::test().await;

  let listener = listener_setup(config.base.port).await;

  let mut router = api_router();
  router = base_router(router, &config.base, &config.metrics).await;
  let app = state(router, config).await;

  info!("Starting application");
  run_app(listener, app).await;
}

fn api_router() -> Router {
  dummy::router()
}

async fn state(mut router: Router, config: Config) -> Router {
  let db = init_db::<migration::Migrator>(&config.db, &config.db_url).await;
  router = dummy::state(router);
  router.layer(Extension(db)).layer(Extension(config))
}
