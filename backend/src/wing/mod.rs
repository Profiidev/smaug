use std::sync::Arc;

use centaurus::{error::Result, eyre::Context};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use shared::{auth::SignData, msg::WingMessage};
use tokio::{net::TcpStream, spawn, sync::Mutex, task::JoinHandle};
use tokio_tungstenite::{
  MaybeTlsStream, WebSocketStream, connect_async,
  tungstenite::{self, client::IntoClientRequest},
};
use tracing::info;

#[derive(Default, Clone)]
struct Wings {
  wings: Arc<Mutex<Vec<WingConnection>>>,
}

struct WingConnection {
  sender: WingSender,
  _receiver: JoinHandle<()>,
}

struct WingSender(SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, tungstenite::Message>);

impl Wings {
  pub async fn connect(&self, addr: &str, token: &str) -> Result<()> {
    let mut request = addr.into_client_request().context("Invalid wing address")?;

    let data = SignData::new();
    data.add_to_header_map(request.headers_mut(), token);

    info!("Connecting to wing at {}", addr);

    let (stream, res) = connect_async(request)
      .await
      .context("Failed to connect to wing")?;

    info!("Verifying wing connection to {}", addr);

    SignData::validate_header_map(res.headers(), token, Some(data))?;

    info!("Connected to wing at {}", addr);

    let mut connection = WingConnection::new(stream);

    connection.send(&WingMessage::Hello).await?;

    self.wings.lock().await.push(connection);

    Ok(())
  }
}

impl WingConnection {
  fn new(stream: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
    let (write, mut read) = stream.split();

    let receiver = spawn(async move {
      while let Some(Ok(next)) = read.next().await {
        info!("Received wing message: {:?}", next);
        match next {
          tungstenite::Message::Binary(raw_msg) => {
            match serde_json::from_slice::<WingMessage>(&raw_msg) {
              Ok(msg) => {
                info!("Parsed wing message: {:?}", msg);
              }
              Err(err) => {
                info!("Failed to parse wing message: {}", err);
              }
            }
          }
          tungstenite::Message::Close(_) => {
            info!("Wing connection closed");
            break;
          }
          _ => (),
        }
      }
    });

    Self {
      sender: WingSender(write),
      _receiver: receiver,
    }
  }

  pub async fn send(&mut self, msg: &WingMessage) -> Result<()> {
    self.sender.send(msg).await
  }
}

impl WingSender {
  async fn send(&mut self, msg: &WingMessage) -> Result<()> {
    let msg = serde_json::to_string(msg)?;
    self
      .0
      .send(tungstenite::Message::Binary(msg.into()))
      .await
      .context("Failed to send Message")?;

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
