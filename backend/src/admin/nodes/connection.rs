use std::{sync::Arc, time::Duration};

use centaurus::{
  bail,
  error::Result,
  eyre::{Context, ContextCompat},
};
use futures_util::{
  SinkExt, StreamExt,
  stream::{SplitSink, SplitStream},
};
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use shared::msg::WingsMessage;
use tokio::{
  spawn,
  sync::{Mutex, Notify, oneshot},
  task::JoinHandle,
  time::sleep,
};
use tokio_tungstenite::tungstenite;
use tracing::{debug, error, info, warn};

use crate::admin::nodes::auth::{WingsAuth, WsStream};

pub struct WingsConnection {
  sender: Option<SplitSink<WsStream, tungstenite::Message>>,
  client: ClientWithMiddleware,
  receiver: Option<JoinHandle<()>>,
  reconnect: JoinHandle<()>,
  disconnect: Arc<Notify>,
}

impl WingsConnection {
  pub async fn new(addr: &str, port: i16, secure: bool, token: String) -> Result<Arc<Mutex<Self>>> {
    let addr = format!("{}://{}:{}", if secure { "wss" } else { "ws" }, addr, port);

    let client = Client::new();
    let client = ClientBuilder::new(client)
      .with(WingsAuth::new(token.clone()))
      .build();

    let (sender, receiver) = oneshot::channel();
    let disconnect = Arc::new(Notify::new());

    let reconnect = spawn({
      let disconnect = disconnect.clone();

      reconnect_task(receiver, addr, token, disconnect)
    });

    let conn = Arc::new(Mutex::new(Self {
      sender: None,
      receiver: None,
      client,
      reconnect,
      disconnect,
    }));

    sender
      .send(conn.clone())
      .ok()
      .context("Failed to init reconnect")?;

    Ok(conn)
  }

  pub fn is_connected(&self) -> bool {
    self.sender.is_some()
  }

  pub async fn disconnect(&self) {
    self.disconnect.notify_waiters();
    if let Some(handle) = &self.receiver {
      handle.abort();
    }
    self.reconnect.abort();
  }

  pub async fn send(&mut self, msg: &WingsMessage) -> Result<()> {
    let Some(sender) = &mut self.sender else {
      bail!("Wings connection is not established");
    };

    let msg = serde_json::to_string(msg)?;
    sender
      .send(tungstenite::Message::Binary(msg.into()))
      .await
      .context("Failed to send Message")?;

    Ok(())
  }
}

async fn reconnect_task(
  receiver: oneshot::Receiver<Arc<Mutex<WingsConnection>>>,
  addr: String,
  token: String,
  disconnect: Arc<Notify>,
) {
  let Ok(conn) = receiver.await else {
    error!("Wings connection task failed to receive initial connection");
    return;
  };

  let reconnect = Arc::new(Notify::new());
  reconnect.notify_one();

  loop {
    tokio::select! {
      _ = disconnect.notified() => {
        debug!("Wings connection received disconnect signal, stopping reconnect task");
        return;
      }
      _ = reconnect.notified() => {
        debug!("Wings connection lost, attempting to reconnect...");
      }
    }

    let mut conn_ref = conn.lock().await;
    conn_ref.sender = None;
    conn_ref.receiver = None;
    drop(conn_ref);

    let stream = match WingsAuth::connect_websocket(&addr, &token).await {
      Ok(stream) => stream,
      Err(err) => {
        warn!("Failed to reconnect to wings websocket: {}", err);
        spawn({
          let reconnect = reconnect.clone();
          async move {
            sleep(Duration::from_secs(5)).await;
            reconnect.notify_one();
          }
        });
        continue;
      }
    };

    let (sender, receiver) = stream.split();
    let receiver = spawn(receiver_task(
      receiver,
      reconnect.clone(),
      disconnect.clone(),
    ));

    let mut conn_ref = conn.lock().await;
    conn_ref.sender = Some(sender);
    conn_ref.receiver = Some(receiver);
    drop(conn_ref);

    debug!("Wings connection re-established");
  }
}

async fn receiver_task(
  mut receiver: SplitStream<WsStream>,
  reconnect: Arc<Notify>,
  disconnect: Arc<Notify>,
) {
  loop {
    let msg = tokio::select! {
      _ = disconnect.notified() => {
        debug!("Wings receiver task received disconnect signal, stopping receiver task");
        return;
      }
      msg = receiver.next() => msg
    };

    let Some(Ok(next)) = msg else {
      debug!("Wings websocket connection closed or errored");
      break;
    };

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
        break;
      }
      _ => (),
    }
  }

  debug!("Websocket connection closed");
  reconnect.notify_one();
}
