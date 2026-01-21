use axum::{Extension, Router};
use centaurus::{
  init::{
    axum::{listener_setup, run_app},
    logging::init_logging,
    router::base_router,
  },
  router_extension,
};
#[cfg(debug_assertions)]
use dotenv::dotenv;
use tracing::info;

use crate::config::Config;

mod config;
mod dummy;

#[tokio::main]
async fn main() {
  #[cfg(debug_assertions)]
  dotenv().ok();

  let config = Config::parse();
  init_logging(&config.base);

  let listener = listener_setup(config.base.port).await;

  let app = base_router(api_router(), &config.base, &config.metrics)
    .await
    .state(config)
    .await;

  info!("Starting application");
  run_app(listener, app).await;
}

fn api_router() -> Router {
  dummy::router()
}

router_extension!(
  async fn state(self, config: Config) -> Self {
    use dummy::dummy;

    self.dummy().await.layer(Extension(config))
  }
);
