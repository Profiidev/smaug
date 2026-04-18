use aide::axum::ApiRouter;
use axum::{Extension, Router};
use centaurus::{
  backend::{
    init::{listener_setup, run_app_connect_info},
    middleware::rate_limiter::RateLimiter,
    router::build_router,
  },
  db::init::init_db,
  logging::init_logging,
};
#[cfg(debug_assertions)]
use dotenv::dotenv;
use tracing::info;

use crate::config::Config;

mod auth;
mod config;
mod db;
mod gravatar;
mod group;
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
  init_logging(config.base.log_level);

  let listener = listener_setup(config.base.port).await;
  let app = build_router(router, state, config).await;

  info!("Starting application");
  run_app_connect_info(listener, app).await;
}

fn router(rate_limiter: &mut RateLimiter) -> ApiRouter {
  Router::new()
    .nest("/nodes", nodes::router())
    .nest("/ws", ws::router())
    .nest("/setup", setup::router())
    .nest("/auth", auth::router(rate_limiter))
    .nest("/user", user::router(rate_limiter))
    .nest("/settings", settings::router())
    .nest("/mail", mail::router(rate_limiter))
    .nest("/group", group::router())
    .into()
}

async fn state(router: ApiRouter, config: Config) -> ApiRouter {
  let db = init_db::<migration::Migrator>(&config.db, &config.db_url).await;
  setup::create_admin_group(&db)
    .await
    .expect("Failed to create admin group");

  let (mut router, updater) = ws::state(router.into()).await;
  router = nodes::state(router, &db, updater).await;
  router = auth::state(router, &config, &db).await;
  router = mail::state(router, &db).await;

  router.layer(Extension(db)).layer(Extension(config)).into()
}
