use axum::{Extension, Router};
use centaurus::db::init::Connection;
use tower_governor::GovernorLayer;

use crate::{
  mail::state::{Mailer, ResetPasswordState},
  rate_limit::RateLimiter,
};

mod reset;
pub mod state;
mod templates;
mod test;

pub fn router(rate_limiter: &mut RateLimiter) -> Router {
  Router::new()
    .nest("/reset", reset::router())
    .nest("/test", test::router())
    .layer(GovernorLayer::new(rate_limiter.create_limiter()))
}

pub async fn state(router: Router, db: &Connection) -> Router {
  let mailer = Mailer::new(db).await;
  let password_reset_state = ResetPasswordState::default();

  router
    .layer(Extension(mailer))
    .layer(Extension(password_reset_state))
}
