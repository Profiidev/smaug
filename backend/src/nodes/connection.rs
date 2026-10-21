use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use centaurus::{bail, error::Result};
use futures_util::{SinkExt, StreamExt};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use shared::msg::WingsMessage;
use tokio::{
  spawn,
  sync::mpsc,
  task::JoinHandle,
  time::{Instant, interval, sleep},
};
use tokio_tungstenite::tungstenite;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
  nodes::auth::{WingsAuth, WsStream},
  utils::{UpdateMessage, Updater, client},
};

const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const PING_INTERVAL: Duration = Duration::from_secs(15);
/// Connection counts as dead if nothing (not even a pong) arrived for this long.
const LIVENESS_TIMEOUT: Duration = Duration::from_secs(45);

/// Handle to the task that owns the websocket to one wings node. Dropping it
/// closes the connection.
pub struct WingsConnection {
  uuid: Uuid,
  tx: mpsc::Sender<WingsMessage>,
  connected: Arc<AtomicBool>,
  #[allow(unused)]
  client: ClientWithMiddleware,
  task: JoinHandle<()>,
}

impl WingsConnection {
  pub fn new(
    uuid: Uuid,
    addr: &str,
    port: u16,
    secure: bool,
    token: String,
    updater: Updater,
  ) -> Self {
    let addr = format!(
      "{}://{}:{}/api",
      if secure { "wss" } else { "ws" },
      addr,
      port
    );

    let client = ClientBuilder::new(client())
      .with(WingsAuth::new(token.clone()))
      .build();

    let (tx, rx) = mpsc::channel(64);
    let connected = Arc::new(AtomicBool::new(false));
    let task = spawn(connection_task(
      uuid,
      addr,
      token,
      rx,
      connected.clone(),
      updater,
    ));

    Self {
      uuid,
      tx,
      connected,
      client,
      task,
    }
  }

  pub fn is_connected(&self) -> bool {
    self.connected.load(Ordering::Relaxed)
  }

  #[allow(unused)]
  pub async fn send(&self, msg: WingsMessage) -> Result<()> {
    if !self.is_connected() || self.tx.send(msg).await.is_err() {
      bail!("Wings connection to {} is not established", self.uuid);
    }
    Ok(())
  }
}

impl Drop for WingsConnection {
  fn drop(&mut self) {
    self.task.abort();
  }
}

async fn connection_task(
  uuid: Uuid,
  addr: String,
  token: String,
  mut rx: mpsc::Receiver<WingsMessage>,
  connected: Arc<AtomicBool>,
  updater: Updater,
) {
  loop {
    match WingsAuth::connect_websocket(&addr, &token).await {
      Ok(stream) => {
        connected.store(true, Ordering::Relaxed);
        // TODO(wings): full sync on (re)connect: send every server on this node
        // (spec, suspended/install state, allocations, variables) and drop unknown ones
        updater.broadcast(UpdateMessage::Nodes { uuid }).await;
        debug!("Wings connection to {} established", uuid);

        run_connection(uuid, stream, &mut rx).await;

        connected.store(false, Ordering::Relaxed);
        updater.broadcast(UpdateMessage::Nodes { uuid }).await;
        warn!("Wings connection to {} lost, reconnecting", uuid);
      }
      Err(err) => warn!(
        "Failed to connect to wings websocket for {}: {:?}",
        uuid, err
      ),
    }

    sleep(RECONNECT_DELAY).await;
  }
}

/// Runs until the connection is closed, errors or stops answering pings.
async fn run_connection(uuid: Uuid, stream: WsStream, rx: &mut mpsc::Receiver<WingsMessage>) {
  let (mut sink, mut stream) = stream.split();
  let mut ping = interval(PING_INTERVAL);
  let mut last_seen = Instant::now();

  loop {
    tokio::select! {
      msg = stream.next() => {
        let Some(Ok(msg)) = msg else {
          return;
        };
        last_seen = Instant::now();

        match msg {
          tungstenite::Message::Binary(raw_msg) => {
            match serde_json::from_slice::<WingsMessage>(&raw_msg) {
              Ok(msg) => info!("Parsed wings message for {}: {:?}", uuid, msg),
              Err(err) => info!("Failed to parse wings message for {}: {}", uuid, err),
            }
          }
          tungstenite::Message::Close(_) => return,
          _ => (),
        }
      }
      Some(msg) = rx.recv() => {
        let msg = serde_json::to_vec(&msg).expect("WingsMessage is always serializable");
        if let Err(err) = sink.send(tungstenite::Message::Binary(msg.into())).await {
          warn!("Failed to send message to wings {}: {}", uuid, err);
          return;
        }
      }
      _ = ping.tick() => {
        if last_seen.elapsed() > LIVENESS_TIMEOUT {
          warn!("Wings connection to {} timed out", uuid);
          return;
        }
        if sink.send(tungstenite::Message::Ping(Default::default())).await.is_err() {
          return;
        }
      }
    }
  }
}
