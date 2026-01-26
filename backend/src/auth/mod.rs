use axum::{Extension, Router};
use centaurus::{auth::pw::PasswordState, db::init::Connection};
use rsa::{
  RsaPrivateKey,
  pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey},
  pkcs8::LineEnding,
  rand_core::OsRng,
};
use uuid::Uuid;

use crate::{config::Config, db::DBTrait};

mod password;

pub fn router() -> Router {
  Router::new().nest("/password", password::router())
}

pub async fn state(router: Router, config: &Config, db: &Connection) -> Router {
  let pw_state = init_pw_state(config, db).await;
  router.layer(Extension(pw_state))
}

async fn init_pw_state(config: &Config, db: &Connection) -> PasswordState {
  let key = if let Ok(key) = db.key().get_key_by_name("password".into()).await {
    RsaPrivateKey::from_pkcs1_pem(&key.private_key).expect("Failed to parse private password key")
  } else {
    let mut rng = OsRng {};
    let private_key = RsaPrivateKey::new(&mut rng, 4096).expect("Failed to create Rsa key");
    let key = private_key
      .to_pkcs1_pem(LineEnding::CRLF)
      .expect("Failed to export private key")
      .to_string();

    db.key()
      .create_key("password".into(), key.clone(), Uuid::new_v4())
      .await
      .expect("Failed to save key");

    private_key
  };

  let pepper = config.auth_pepper.as_bytes().to_vec();
  PasswordState::init(pepper, key).await
}
