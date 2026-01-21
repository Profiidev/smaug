use std::sync::Arc;

use centaurus::{
  bail,
  error::{ErrorReportStatusExt, Result},
  eyre::Context,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use http::StatusCode;
use rand::RngCore;
use sha3::Sha3_512;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};
use tracing::info;

type HmacSha3_512 = Hmac<Sha3_512>;

#[derive(Default, Clone)]
struct Wings {
  wings: Arc<Mutex<Vec<WingConnection>>>,
}

struct WingConnection {}

impl Wings {
  pub async fn connect(&self, addr: &str, token: &str) -> Result<()> {
    let mut request = addr.into_client_request().context("Invalid wing address")?;

    let mut rng = rand::rng();
    let mut raw_nonce = [0u8; 16];
    rng.fill_bytes(&mut raw_nonce);
    let nonce = hex::encode(raw_nonce);

    let timestamp = Utc::now().timestamp_millis();

    let msg = format!("{}{}", nonce, timestamp);

    let mut mac = HmacSha3_512::new_from_slice(token.as_bytes())?;
    mac.update(msg.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let headers = request.headers_mut();
    headers.insert("x-wing-nonce", nonce.parse().unwrap());
    headers.insert("x-wing-timestamp", timestamp.to_string().parse().unwrap());
    headers.insert("x-wing-signature", signature.parse().unwrap());

    info!("Connecting to wing at {}", addr);

    let (stream, res) = connect_async(request)
      .await
      .context("Failed to connect to wing")?;

    info!("Verified wing connection to {}", addr);

    let headers = res.headers();
    let res_nonce = headers
      .get("x-wing-nonce")
      .status_context(StatusCode::UNAUTHORIZED, "Missing x-wing-nonce header")?
      .to_str()
      .status_context(StatusCode::UNAUTHORIZED, "Invalid x-wing-nonce header")?;
    let timestamp = headers
      .get("x-wing-timestamp")
      .status_context(StatusCode::UNAUTHORIZED, "Missing x-wing-timestamp header")?
      .to_str()
      .status_context(StatusCode::UNAUTHORIZED, "Invalid x-wing-timestamp header")?
      .to_string();
    let signature = headers
      .get("x-wing-signature")
      .status_context(StatusCode::UNAUTHORIZED, "Missing x-wing-signature header")?
      .to_str()
      .status_context(StatusCode::UNAUTHORIZED, "Invalid x-wing-signature header")?
      .to_string();

    // Nonce MUST be different so the wing can't replay the same signature
    if res_nonce == nonce {
      bail!(UNAUTHORIZED, "Invalid wing nonce");
    }

    let msg = format!("{}{}", res_nonce, timestamp);
    let mut mac = HmacSha3_512::new_from_slice(token.as_bytes())?;
    mac.update(msg.as_bytes());
    let correct_signature = hex::encode(mac.finalize().into_bytes());

    if signature != correct_signature {
      bail!(UNAUTHORIZED, "Invalid wing signature");
    }

    Ok(())
  }
}

pub async fn test() {
  let wings = Wings::default();

  wings
    .connect("ws://wing:8000/api", "test-token")
    .await
    .unwrap();
}
