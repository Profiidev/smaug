use std::{sync::Arc, time::Duration};

use centaurus::{
  bail,
  error::{ErrorReportStatusExt, Result},
};
use futures_util::{
  SinkExt, StreamExt,
  stream::{SplitSink, SplitStream},
};
use http::StatusCode;
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
use uuid::Uuid;

use crate::{
  admin::nodes::auth::{WingsAuth, WsStream},
  ws::state::{UpdateMessage, Updater},
};

pub struct WingsConnection {
  uuid: Uuid,
  sender: Option<SplitSink<WsStream, tungstenite::Message>>,
  #[allow(unused)]
  client: ClientWithMiddleware,
  receiver: Option<JoinHandle<()>>,
  reconnect: JoinHandle<()>,
  disconnect: Arc<Notify>,
}

impl WingsConnection {
  pub async fn new(
    uuid: Uuid,
    addr: &str,
    port: i16,
    secure: bool,
    token: String,
    updater: Updater,
  ) -> Result<Arc<Mutex<Self>>> {
    let addr = format!(
      "{}://{}:{}/api",
      if secure { "wss" } else { "ws" },
      addr,
      port
    );

    let client = Client::new();
    let client = ClientBuilder::new(client)
      .with(WingsAuth::new(token.clone()))
      .build();

    let (sender, receiver) = oneshot::channel();
    let disconnect = Arc::new(Notify::new());

    let reconnect = spawn({
      let disconnect = disconnect.clone();

      reconnect_task(uuid, receiver, addr, token, disconnect, updater)
    });

    let conn = Arc::new(Mutex::new(Self {
      uuid,
      sender: None,
      receiver: None,
      client,
      reconnect,
      disconnect,
    }));

    sender.send(conn.clone()).ok().status_context(
      StatusCode::INTERNAL_SERVER_ERROR,
      &format!("Failed to init reconnect for wings connection {}", uuid),
    )?;

    Ok(conn)
  }

  pub fn is_connected(&self) -> bool {
    self.sender.is_some()
  }

  pub fn disconnect(&self) {
    self.disconnect.notify_waiters();
    if let Some(handle) = &self.receiver {
      handle.abort();
    }
    self.reconnect.abort();
  }

  #[allow(unused)]
  pub async fn send(&mut self, msg: &WingsMessage) -> Result<()> {
    let Some(sender) = &mut self.sender else {
      bail!("Wings connection to {} is not established", self.uuid);
    };

    let msg = serde_json::to_string(msg)?;
    sender
      .send(tungstenite::Message::Binary(msg.into()))
      .await
      .status_context(
        StatusCode::INTERNAL_SERVER_ERROR,
        &format!("Failed to send Message to wings connection {}", self.uuid),
      )?;

    Ok(())
  }
}

impl Drop for WingsConnection {
  fn drop(&mut self) {
    self.disconnect();
  }
}

async fn reconnect_task(
  uuid: Uuid,
  receiver: oneshot::Receiver<Arc<Mutex<WingsConnection>>>,
  addr: String,
  token: String,
  disconnect: Arc<Notify>,
  updater: Updater,
) {
  let Ok(conn) = receiver.await else {
    error!(
      "Wings connection task failed to receive initial connection for {}",
      uuid
    );
    return;
  };

  let reconnect = Arc::new(Notify::new());
  reconnect.notify_one();

  loop {
    tokio::select! {
      _ = disconnect.notified() => {
        debug!("Wings connection for {} received disconnect signal, stopping reconnect task", uuid);
        return;
      }
      _ = reconnect.notified() => {
        debug!("Wings connection for {} lost, attempting to reconnect", uuid);
      }
    }

    let mut conn_ref = conn.lock().await;

    // only send update if we were previously connected
    if conn_ref.sender.is_some() {
      updater.broadcast(UpdateMessage::Nodes).await;
    }

    conn_ref.sender = None;
    conn_ref.receiver = None;
    drop(conn_ref);

    let stream = match WingsAuth::connect_websocket(&addr, &token).await {
      Ok(stream) => stream,
      Err(err) => {
        warn!(
          "Failed to reconnect to wings websocket for {}: {}",
          uuid, err
        );
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
      uuid,
      receiver,
      reconnect.clone(),
      disconnect.clone(),
    ));

    let mut conn_ref = conn.lock().await;
    conn_ref.sender = Some(sender);
    conn_ref.receiver = Some(receiver);
    drop(conn_ref);

    updater.broadcast(UpdateMessage::Nodes).await;

    debug!("Wings connection to {} re-established", uuid);
  }
}

async fn receiver_task(
  uuid: Uuid,
  mut receiver: SplitStream<WsStream>,
  reconnect: Arc<Notify>,
  disconnect: Arc<Notify>,
) {
  loop {
    let msg = tokio::select! {
      _ = disconnect.notified() => {
        debug!("Wings receiver task for {} received disconnect signal, stopping receiver task", uuid);
        return;
      }
      msg = receiver.next() => msg
    };

    let Some(Ok(next)) = msg else {
      break;
    };

    info!("Received wings message for {}: {:?}", uuid, next);
    match next {
      tungstenite::Message::Binary(raw_msg) => {
        match serde_json::from_slice::<WingsMessage>(&raw_msg) {
          Ok(msg) => {
            info!("Parsed wings message for {}: {:?}", uuid, msg);
          }
          Err(err) => {
            info!("Failed to parse wings message for {}: {}", uuid, err);
          }
        }
      }
      tungstenite::Message::Close(_) => {
        break;
      }
      _ => (),
    }
  }

  warn!(
    "Websocket connection closed for {}, notifying reconnect task",
    uuid
  );
  reconnect.notify_one();
}
