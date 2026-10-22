use aide::axum::ApiRouter;
use axum::{Extension, Router};
use centaurus::{
  backend::{
    auth,
    endpoints::{
      self, group, mail, setup, user,
      websocket::{self, state::UpdateState},
    },
    init::{listener_setup, run_app_connect_info},
    middleware::rate_limiter::RateLimiter,
    router::build_router,
  },
  db::init::init_db,
  logging::init_logging,
  storage::FileStorage,
  version_header,
};
use tokio::{net::TcpListener, spawn};
use tracing::info;

use crate::{
  config::Config,
  db::DBTrait,
  servers::{backups::Backups, schedules::Scheduler},
  utils::UpdateMessage,
};

mod config;
mod db;
mod eggs;
mod nodes;
mod servers;
mod settings;
mod utils;

pub async fn serve() {
  let config = Config::parse();
  init_logging(config.base.log_level);

  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();

  App::from_config(config).await.run().await;
}

pub struct App {
  app: Router,
  listener: TcpListener,
}

impl App {
  pub async fn new() -> App {
    App::from_config(Config::parse()).await
  }

  pub async fn from_config(config: Config) -> App {
    let listener = listener_setup(config.base.port).await;
    let mut app = build_router(api_router, state, config).await;
    version_header!(app);

    App { app, listener }
  }

  pub fn port(&self) -> u16 {
    self
      .listener
      .local_addr()
      .expect("Failed to read listener address")
      .port()
  }

  pub async fn run(self) {
    info!("Starting application...");
    run_app_connect_info(self.listener, self.app).await;
  }
}

fn api_router(rate_limiter: &mut RateLimiter) -> ApiRouter {
  ApiRouter::new()
    .nest("/ws", websocket::router::<UpdateMessage>())
    .nest("/setup", setup::router())
    .nest("/auth", auth::router::<UpdateMessage>(rate_limiter))
    .nest("/user", user::router::<UpdateMessage>(rate_limiter))
    .nest("/settings", settings::router())
    .nest("/mail", mail::router(rate_limiter))
    .nest("/group", group::router::<UpdateMessage>())
    .nest("/nodes", nodes::router())
    .nest("/eggs", eggs::router())
    .nest("/servers", servers::router())
}

async fn state(mut router: ApiRouter, config: Config) -> ApiRouter {
  let db = init_db::<migration::Migrator>(&config.db, &config.db_url).await;
  centaurus::backend::endpoints::setup::create_admin_group(
    &db,
    utils::permissions(),
    Some(config.admin_group.clone()),
  )
  .await
  .expect("Failed to create admin group");

  let (state, updater) = UpdateState::<UpdateMessage>::init().await;

  router = endpoints::user::state(router);
  router = auth::state(router, &config, &db).await;
  router = mail::state(router, &db, &config).await;
  let wings = nodes::state(&db, updater.clone(), config.allow_insecure_wings).await;
  let storage = FileStorage::init(&config.storage)
    .await
    .expect("Failed to initialize file storage");
  db.backup()
    .reset_unfinished()
    .await
    .expect("Failed to reset unfinished backups");
  let scheduler = Scheduler::new(Backups {
    db: db.clone(),
    wings: wings.clone(),
    storage: storage.clone(),
    updater: updater.clone(),
  });
  spawn(scheduler.clone().run());

  router
    .layer(Extension(wings))
    .layer(Extension(storage))
    .layer(Extension(scheduler))
    .layer(Extension(db))
    .layer(Extension(state))
    .layer(Extension(updater))
}

#[cfg(test)]
mod test {
  use super::*;
  use centaurus::backend::middleware::rate_limiter::RateLimiter;

  #[test]
  fn api_router_builds_all_module_routers() {
    // Exercises every module's `router()` builder (including the rate-limited
    // `auth`/`user`/`mail` ones) plus the top-level `api_router` wiring.
    let mut rate_limiter = RateLimiter::default();
    let _ = api_router(&mut rate_limiter);
  }
}
