use std::sync::Arc;

use centaurus::{error::Result, eyre::Context};
use shared::auth::SignData;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};
use tracing::info;

#[derive(Default, Clone)]
struct Wings {
  wings: Arc<Mutex<Vec<WingConnection>>>,
}

struct WingConnection {}

impl Wings {
  pub async fn connect(&self, addr: &str, token: &str) -> Result<()> {
    let mut request = addr.into_client_request().context("Invalid wing address")?;

    let data = SignData::new();
    data.add_to_header_map(request.headers_mut(), token);

    info!("Connecting to wing at {}", addr);

    let (_stream, res) = connect_async(request)
      .await
      .context("Failed to connect to wing")?;

    info!("Verified wing connection to {}", addr);

    SignData::validate_header_map(res.headers(), token, Some(data))?;

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
