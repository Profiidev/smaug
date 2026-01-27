use centaurus::eyre::Context;
use http::Extensions;
use reqwest_middleware::{Middleware, Next};
use shared::auth::SignData;
use tokio::net::TcpStream;
use tokio_tungstenite::{
  MaybeTlsStream, WebSocketStream, connect_async, tungstenite::client::IntoClientRequest,
};
use tracing::debug;

pub struct WingsAuth {
  token: String,
}

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[async_trait::async_trait]
impl Middleware for WingsAuth {
  async fn handle(
    &self,
    mut req: reqwest::Request,
    client: &mut Extensions,
    next: Next<'_>,
  ) -> reqwest_middleware::Result<reqwest::Response> {
    debug!("Signing request with token middleware to {}", req.url());

    let data = SignData::new();
    data
      .add_to_header_map(req.headers_mut(), &self.token)
      .map_err(reqwest_middleware::Error::middleware)?;

    let res = next.run(req, client).await?;

    debug!(
      "Verifying response with token middleware from {}",
      res.url()
    );

    SignData::validate_header_map(res.headers(), &self.token, Some(data))
      .map_err(reqwest_middleware::Error::middleware)?;

    Ok(res)
  }
}

impl WingsAuth {
  pub fn new(token: String) -> Self {
    Self { token }
  }

  pub async fn connect_websocket(addr: &str, token: &str) -> centaurus::error::Result<WsStream> {
    let mut request = addr
      .into_client_request()
      .context("Invalid wings address")?;

    debug!("Signing websocket request to {}", addr);

    let data = SignData::new();
    data
      .add_to_header_map(request.headers_mut(), token)
      .context("Failed to sign websocket request")?;

    let (stream, res) = connect_async(request)
      .await
      .context("Failed to connect to wings")?;

    debug!("Verifying wings connection to {}", addr);

    SignData::validate_header_map(res.headers(), token, Some(data))
      .context("Failed to verify wings websocket connection")?;

    Ok(stream)
  }
}
