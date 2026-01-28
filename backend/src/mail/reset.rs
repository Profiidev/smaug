use axum::{Json, Router, extract::FromRequest, routing::post};
use centaurus::db::init::Connection;
use serde::Deserialize;
use tokio::spawn;
use tracing::warn;

use crate::{
  db::{DBTrait, settings::GeneralSettings},
  mail::{
    state::{Mailer, ResetPasswordState},
    templates,
  },
};

pub fn router() -> Router {
  Router::new().route("/send", post(send_reset_link))
}

#[derive(FromRequest, Deserialize)]
#[from_request(via(Json))]
struct ResetRequest {
  email: String,
}

#[axum::debug_handler]
async fn send_reset_link(
  mailer: Mailer,
  state: ResetPasswordState,
  db: Connection,
  ResetRequest { email }: ResetRequest,
) -> Result<(), ()> {
  // Spawn a new task to handle the email sending asynchronously and avoid exposing timing information
  spawn(async move {
    let Some(user) = db.user().get_user_by_email(&email).await.ok() else {
      warn!("Password reset requested for non-existent email: {}", email);
      return;
    };

    let Some(settings) = db.settings().get_settings::<GeneralSettings>().await.ok() else {
      warn!(
        "Failed to retrieve general settings for password reset email to: {}",
        email
      );
      return;
    };

    let token = state.generate_token(user.email.clone()).await;

    let mut reset_link = settings.site_url.clone();
    if let Ok(segments) = &mut reset_link.path_segments_mut() {
      segments.pop_if_empty();
      segments.push("reset-password");
    }
    reset_link.query_pairs_mut().append_pair("token", &token);

    if let Err(e) = mailer
      .send_mail(
        user.name,
        user.email,
        "Smaug Password Reset".to_string(),
        templates::reset_link(reset_link.as_str(), settings.site_url.as_str()),
      )
      .await
    {
      warn!("Failed to send password reset email to {}: {:?}", email, e);
    }
  });

  Ok(())
}
