use axum::{Extension, Router};
use centaurus::db::init::Connection;

use crate::mail::state::{Mailer, ResetPasswordState};

mod reset;
pub mod state;
mod templates;
mod test;

pub fn router() -> Router {
  Router::new()
    .nest("/reset", reset::router())
    .nest("/test", test::router())
}

pub async fn state(router: Router, db: &Connection) -> Router {
  let mailer = Mailer::new(db).await;
  let password_reset_state = ResetPasswordState::default();

  router
    .layer(Extension(mailer))
    .layer(Extension(password_reset_state))
}
