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

mod auth;
mod config;
mod db;
mod mail;
mod nodes;
mod permissions;
mod settings;
mod setup;
mod user;
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
  let app = state(router, config).await;

  info!("Starting application");
  run_app(listener, app).await;
}

fn api_router() -> Router {
  Router::new()
    .nest("/nodes", nodes::router())
    .nest("/ws", ws::router())
    .nest("/setup", setup::router())
    .nest("/auth", auth::router())
    .nest("/user", user::router())
    .nest("/settings", settings::router())
    .nest("/mail", mail::router())
}

async fn state(router: Router, config: Config) -> Router {
  let db = init_db::<migration::Migrator>(&config.db, &config.db_url).await;
  setup::create_admin_group(&db)
    .await
    .expect("Failed to create admin group");

  let (mut router, updater) = ws::state(router).await;
  router = nodes::state(router, &db, updater).await;
  router = auth::state(router, &config, &db).await;
  router = mail::state(router, &db).await;

  router.layer(Extension(db)).layer(Extension(config))
}
