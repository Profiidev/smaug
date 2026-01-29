use axum::{Router, routing::post};
use centaurus::{db::init::Connection, error::Result};

use crate::{
  auth::jwt_auth::JwtAuth,
  db::{DBTrait, settings::GeneralSettings},
  mail::{state::Mailer, templates},
  permissions::SettingsEdit,
};

pub fn router() -> Router {
  Router::new().route("/", post(test_mail))
}

async fn test_mail(auth: JwtAuth<SettingsEdit>, mailer: Mailer, db: Connection) -> Result<()> {
  let user = db.user().get_user_by_id(auth.user_id).await?;
  let link = db
    .settings()
    .get_settings::<GeneralSettings>()
    .await?
    .site_url;

  mailer
    .send_mail(
      user.name,
      user.email,
      "Test Email".to_string(),
      templates::test_email(link.as_str()),
    )
    .await?;

  Ok(())
}
