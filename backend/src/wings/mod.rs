use std::sync::Arc;

use centaurus::{error::Result, eyre::Context};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use shared::{auth::SignData, msg::WingsMessage};
use tokio::{net::TcpStream, spawn, sync::Mutex, task::JoinHandle};
use tokio_tungstenite::{
  MaybeTlsStream, WebSocketStream, connect_async,
  tungstenite::{self, client::IntoClientRequest},
};
use tracing::info;

mod management;

#[derive(Default, Clone)]
struct Wings {
  wings: Arc<Mutex<Vec<WingsConnection>>>,
}

struct WingsConnection {
  sender: WingsSender,
  _receiver: JoinHandle<()>,
}

struct WingsSender(SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, tungstenite::Message>);

impl Wings {
  pub async fn connect(&self, addr: &str, token: &str) -> Result<()> {
    let mut request = addr
      .into_client_request()
      .context("Invalid wings address")?;

    let data = SignData::new();
    data.add_to_header_map(request.headers_mut(), token);

    info!("Connecting to wings at {}", addr);

    let (stream, res) = connect_async(request)
      .await
      .context("Failed to connect to wings")?;

    info!("Verifying wings connection to {}", addr);

    SignData::validate_header_map(res.headers(), token, Some(data))?;

    info!("Connected to wings at {}", addr);

    let mut connection = WingsConnection::new(stream);

    connection.send(&WingsMessage::Hello).await?;

    self.wings.lock().await.push(connection);

    Ok(())
  }
}

impl WingsConnection {
  fn new(stream: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
    let (write, mut read) = stream.split();

    let receiver = spawn(async move {
      while let Some(Ok(next)) = read.next().await {
        info!("Received wings message: {:?}", next);
        match next {
          tungstenite::Message::Binary(raw_msg) => {
            match serde_json::from_slice::<WingsMessage>(&raw_msg) {
              Ok(msg) => {
                info!("Parsed wings message: {:?}", msg);
              }
              Err(err) => {
                info!("Failed to parse wings message: {}", err);
              }
            }
          }
          tungstenite::Message::Close(_) => {
            info!("Wings connection closed");
            break;
          }
          _ => (),
        }
      }
    });

    Self {
      sender: WingsSender(write),
      _receiver: receiver,
    }
  }

  pub async fn send(&mut self, msg: &WingsMessage) -> Result<()> {
    self.sender.send(msg).await
  }
}

impl WingsSender {
  async fn send(&mut self, msg: &WingsMessage) -> Result<()> {
    let msg = serde_json::to_string(msg)?;
    self
      .0
      .send(tungstenite::Message::Binary(msg.into()))
      .await
      .context("Failed to send Message")?;

    Ok(())
  }
}
