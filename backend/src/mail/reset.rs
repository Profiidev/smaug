use axum::{Json, Router, extract::FromRequest, routing::post};
use centaurus::{auth::pw::PasswordState, db::init::Connection};
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
  Router::new()
    .route("/send", post(send_reset_link))
    .route("/confirm", post(reset_password))
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
      segments.push("password");
      segments.push("reset");
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

#[derive(FromRequest, Deserialize)]
#[from_request(via(Json))]
struct ResetPasswordPayload {
  token: String,
  new_password: String,
}

async fn reset_password(
  db: Connection,
  state: ResetPasswordState,
  pw: PasswordState,
  ResetPasswordPayload {
    token,
    new_password,
  }: ResetPasswordPayload,
) -> Result<(), ()> {
  // Do reset async to avoid exposing timing information
  spawn(async move {
    let Some(email) = state.validate_token(&token).await else {
      warn!("Invalid or expired password reset token used");
      return;
    };

    let Some(user) = db.user().get_user_by_email(&email).await.ok() else {
      warn!("Password reset attempted for non-existent email: {}", email);
      return;
    };

    let Ok(hashed_password) = pw.pw_hash(&user.salt, &new_password) else {
      warn!("Failed to hash new password for {}", email);
      return;
    };

    if let Err(e) = db
      .user()
      .update_user_password(user.id, hashed_password)
      .await
    {
      warn!("Failed to update password for {}: {:?}", email, e);
    }

    state.invalidate_token(&email).await;
  });

  Ok(())
}
