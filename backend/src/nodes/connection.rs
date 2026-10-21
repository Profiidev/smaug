use std::{
  collections::{HashMap, HashSet},
  sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use centaurus::{bail, db::init::Connection, error::Result, eyre::Context};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use reqwest::{Method, StatusCode};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use serde_json::Value;
use shared::msg::{
  Event, InstallResult, Power, PowerRequest, Runtime, ServerState, SyncRequest, SyncResponse,
  WingsMessage,
};
use tokio::{
  spawn,
  sync::{Mutex, Notify, mpsc},
  task::JoinHandle,
  time::{Instant, interval, sleep, sleep_until, timeout},
};
use tokio_tungstenite::tungstenite;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
  db::DBTrait,
  nodes::auth::{WingsAuth, WsStream},
  servers::spec,
  utils::{UpdateMessage, Updater, client},
};

const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const PING_INTERVAL: Duration = Duration::from_secs(15);
/// Connection counts as dead if nothing (not even a pong) arrived for this long.
const LIVENESS_TIMEOUT: Duration = Duration::from_secs(45);
/// A request (or the websocket handshake) that takes longer than this failed.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The full sync sends every server, wings may need a while for it.
const SYNC_TIMEOUT: Duration = Duration::from_secs(300);
/// Full sync while connected, catches drift no failed request reported.
const RESYNC_INTERVAL: Duration = Duration::from_secs(300);
/// Earliest full sync after a failed one or after a failed request.
const RESYNC_RETRY: Duration = Duration::from_secs(30);

/// Handle to the task that owns the websocket to one wings node. Dropping it
/// closes the connection.
pub struct WingsConnection {
  link: Link,
  tx: mpsc::Sender<WingsMessage>,
  connected: Arc<AtomicBool>,
  task: JoinHandle<()>,
}

/// Everything needed to talk to one node and to apply what it reports.
#[derive(Clone)]
pub struct Link {
  uuid: Uuid,
  base: String,
  client: ClientWithMiddleware,
  /// Runtime state per server, as last reported. Empty while disconnected.
  states: Arc<DashMap<Uuid, ServerState>>,
  /// Last error per server, as last reported. Empty while disconnected.
  errors: Arc<DashMap<Uuid, String>>,
  /// Whether `states` holds a full sync. Servers missing from it are offline then.
  synced: Arc<AtomicBool>,
  /// Held by every request and the whole sync, so a request never lands in
  /// the middle of a sync and the sync never sends a spec older than a request.
  lock: Arc<Mutex<()>>,
  /// Asks the connection for a full sync, after a failed request.
  resync: Arc<Notify>,
  /// Why the node can not be reached or synced, `None` while it works.
  error: Arc<RwLock<Option<String>>>,
  runtime: Arc<RwLock<Option<Runtime>>>,
  db: Connection,
  updater: Updater,
}

impl WingsConnection {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    uuid: Uuid,
    addr: &str,
    port: u16,
    secure: bool,
    token: String,
    db: Connection,
    updater: Updater,
    lock: Arc<Mutex<()>>,
  ) -> Self {
    let ws_addr = format!(
      "{}://{}:{}/api",
      if secure { "wss" } else { "ws" },
      addr,
      port
    );
    let base = format!(
      "{}://{}:{}/api",
      if secure { "https" } else { "http" },
      addr,
      port
    );

    let link = Link {
      uuid,
      base,
      client: ClientBuilder::new(client())
        .with(WingsAuth::new(token.clone()))
        .build(),
      states: Default::default(),
      errors: Default::default(),
      synced: Default::default(),
      lock,
      resync: Default::default(),
      error: Default::default(),
      runtime: Default::default(),
      db,
      updater,
    };

    let (tx, rx) = mpsc::channel(64);
    let connected = Arc::new(AtomicBool::new(false));
    let task = spawn(connection_task(
      link.clone(),
      ws_addr,
      token,
      rx,
      connected.clone(),
    ));

    Self {
      link,
      tx,
      connected,
      task,
    }
  }

  pub fn is_connected(&self) -> bool {
    self.connected.load(Ordering::Relaxed)
  }

  /// The link to use for requests, `None` while disconnected.
  pub fn link(&self) -> Option<Link> {
    self.is_connected().then(|| self.link.clone())
  }

  /// `None` while unknown: disconnected, or neither reported nor synced yet.
  pub fn state(&self, server: Uuid) -> Option<ServerState> {
    if !self.is_connected() {
      return None;
    }
    let reported = self.link.states.get(&server).map(|s| *s);
    // after a sync, a server wings did not report is not stored there yet
    reported.or_else(|| {
      self
        .link
        .synced
        .load(Ordering::Relaxed)
        .then(ServerState::default)
    })
  }

  /// Why wings could not do what was asked, `None` while there is no error or
  /// it is unknown.
  pub fn error(&self, server: Uuid) -> Option<String> {
    if !self.is_connected() {
      return None;
    }
    self.link.errors.get(&server).map(|e| e.clone())
  }

  pub fn runtime(&self) -> Option<Runtime> {
    *self.link.runtime.read().unwrap()
  }

  /// Why the node can not be reached or synced.
  pub fn node_error(&self) -> Option<String> {
    self.link.error.read().unwrap().clone()
  }

  /// Shared with a replacing connection, so requests to the old and the new
  /// address stay serialized.
  pub fn lock(&self) -> Arc<Mutex<()>> {
    self.link.lock.clone()
  }

  #[allow(unused)]
  pub async fn send(&self, msg: WingsMessage) -> Result<()> {
    if !self.is_connected() || self.tx.send(msg).await.is_err() {
      bail!("Wings connection to {} is not established", self.link.uuid);
    }
    Ok(())
  }
}

impl Drop for WingsConnection {
  fn drop(&mut self) {
    self.task.abort();
  }
}

impl Link {
  /// Best-effort request, the caller holds the lock. A failure is logged and
  /// triggers a full sync, which brings the node back in line with the DB.
  async fn send(&self, method: Method, path: &str, body: Option<Value>) {
    let failure = match self.build(method, path, body, REQUEST_TIMEOUT).send().await {
      Ok(res) if res.status().is_success() => return,
      // wings rejects the request itself, a sync would send the same again
      Ok(res) if res.status().is_client_error() && res.status() != StatusCode::NOT_FOUND => {
        warn!(
          "Wings {} rejected request {}: {}",
          self.uuid,
          path,
          res.status()
        );
        return;
      }
      Ok(res) => format!("status {}", res.status()),
      Err(err) => format!("{err:?}"),
    };
    warn!(
      "Wings request {} to {} failed: {}",
      path, self.uuid, failure
    );
    self.resync.notify_one();
  }

  /// Sends the server's current spec. Read from the DB under the lock, so
  /// concurrent updates reach wings in the order they were saved.
  pub async fn put_server(&self, server: Uuid) {
    let _lock = self.lock.lock().await;
    self.send_put(server).await;
  }

  async fn send_put(&self, server: Uuid) {
    let spec = match self.db.server().find_by_id(server).await {
      Ok(model) => spec::load(&self.db, &model).await,
      Err(err) => Err(err),
    };
    match spec {
      Ok(spec) => {
        let body = serde_json::to_value(&spec).ok();
        self
          .send(Method::PUT, &format!("/servers/{server}"), body)
          .await;
      }
      Err(err) => warn!("Failed to build spec for server {}: {:?}", server, err),
    }
  }

  /// `keep_files` leaves the server's files on the node, for a transfer.
  pub async fn delete_server(&self, server: Uuid, keep_files: bool) {
    let _lock = self.lock.lock().await;
    self.send_delete(server, keep_files).await;
  }

  async fn send_delete(&self, server: Uuid, keep_files: bool) {
    let path = format!("/servers/{server}?keep_files={keep_files}");
    self.send(Method::DELETE, &path, None).await;
  }

  pub async fn power(&self, server: Uuid, action: Power) {
    let _lock = self.lock.lock().await;
    self.send_power(server, action).await;
  }

  async fn send_power(&self, server: Uuid, action: Power) {
    let body = serde_json::to_value(PowerRequest { action }).ok();
    self
      .send(Method::POST, &format!("/servers/{server}/power"), body)
      .await;
  }

  fn build(
    &self,
    method: Method,
    path: &str,
    body: Option<Value>,
    timeout: Duration,
  ) -> reqwest_middleware::RequestBuilder {
    let req = self
      .client
      .request(method, format!("{}{}", self.base, path))
      .timeout(timeout);
    match body {
      Some(body) => req.json(&body),
      None => req,
    }
  }

  async fn request(
    &self,
    method: Method,
    path: &str,
    body: Option<Value>,
    timeout: Duration,
  ) -> Result<reqwest::Response> {
    Ok(
      self
        .build(method, path, body, timeout)
        .send()
        .await
        .context("Failed to reach wings")?
        .error_for_status()
        .context("Wings rejected the request")?,
    )
  }

  /// Records why the node can not be reached or synced, the UI shows it.
  async fn set_error(&self, error: Option<String>) {
    let changed = {
      let mut current = self.error.write().unwrap();
      let changed = *current != error;
      *current = error;
      changed
    };
    if changed {
      self
        .updater
        .broadcast(UpdateMessage::Nodes { uuid: self.uuid })
        .await;
    }
  }

  /// Sends every server of this node to wings and fixes what drifted while
  /// the node was unreachable.
  async fn sync(&self) -> Result<()> {
    let _lock = self.lock.lock().await;
    let servers = self.db.server().list_by_node(self.uuid).await?;
    let mut specs = Vec::with_capacity(servers.len());
    // ponytail: two queries per server, batch once nodes hold many servers
    for server in &servers {
      match spec::load(&self.db, server).await {
        Ok(spec) => specs.push(spec),
        // wings keeps the last spec it got, one broken server must not stop the sync
        Err(err) => warn!("Failed to build spec for server {}: {:?}", server.id, err),
      }
    }
    let body = serde_json::to_value(SyncRequest { servers: specs }).ok();
    let res: SyncResponse = self
      .request(Method::PUT, "/servers", body, SYNC_TIMEOUT)
      .await?
      .json()
      .await
      .context("Invalid sync response from wings")?;

    // the DB is the source of truth, servers it does not know are deleted
    let known: HashSet<Uuid> = servers.iter().map(|s| s.id).collect();
    for orphan in res.orphans.iter().filter(|o| !known.contains(o)) {
      // still in the DB means moved to another node, its files stay for the transfer
      let keep_files = self.db.server().find_by_id(*orphan).await.is_ok();
      info!(
        "Deleting server {} unknown to the panel from wings {}",
        orphan, self.uuid
      );
      self.send_delete(*orphan, keep_files).await;
    }
    let previous: HashMap<Uuid, (ServerState, Option<String>)> = self
      .states
      .iter()
      .map(|e| (*e.key(), (*e, self.errors.get(e.key()).map(|e| e.clone()))))
      .collect();
    self.states.clear();
    self.errors.clear();
    for (uuid, observed) in &res.servers {
      if known.contains(uuid) {
        self.states.insert(*uuid, observed.state);
        if let Some(error) = &observed.error {
          self.errors.insert(*uuid, error.clone());
        }
      }
    }
    self.synced.store(true, Ordering::Relaxed);

    for server in servers {
      let uuid = server.id;
      let observed = res.servers.get(&uuid).cloned().unwrap_or_default();
      let mut changed = previous.get(&uuid) != Some(&(observed.state, observed.error.clone()));
      if server.suspended && is_up(observed.state) {
        self.send_power(uuid, Power::Kill).await;
      }
      // both catch up on events missed while disconnected
      if let Some(result) = observed.last_install {
        match self.apply_install(uuid, result).await {
          Ok(true) => {
            self.send_put(uuid).await;
            changed = true;
          }
          Ok(false) => (),
          Err(err) => warn!("Failed to apply install of server {}: {:?}", uuid, err),
        }
      }
      if let Some(generation) = observed.stopped_generation {
        match self.apply_stopped(uuid, generation).await {
          Ok(applied) => changed |= applied,
          Err(err) => warn!("Failed to apply stop of server {}: {:?}", uuid, err),
        }
      }
      if changed {
        self
          .updater
          .broadcast(UpdateMessage::Servers { uuid })
          .await;
      }
    }
    Ok(())
  }

  /// Applies a finished install if it is the one the panel waits for; then
  /// the spec carries the power the install left. A pending install that has
  /// not finished is (re)started by the sync itself. Returns whether it applied.
  async fn apply_install(&self, server: Uuid, result: InstallResult) -> Result<bool> {
    Ok(
      self
        .db
        .server()
        .finish_install(server, self.uuid, result.generation, result.ok)
        .await?,
    )
  }

  /// Wings stopped the server itself, its power goes off unless the panel
  /// changed it since. Returns whether it applied.
  async fn apply_stopped(&self, server: Uuid, generation: u64) -> Result<bool> {
    Ok(
      self
        .db
        .server()
        .stopped_by_wings(server, self.uuid, generation)
        .await?,
    )
  }

  async fn handle(&self, event: Event) -> Result<()> {
    let uuid = match event {
      Event::Hello { runtime } => {
        *self.runtime.write().unwrap() = Some(runtime);
        self
          .updater
          .broadcast(UpdateMessage::Nodes { uuid: self.uuid })
          .await;
        return Ok(());
      }
      Event::State { uuid, state } => {
        self.states.insert(uuid, state);
        uuid
      }
      Event::Error { uuid, error } => {
        match error {
          Some(error) => self.errors.insert(uuid, error),
          None => self.errors.remove(&uuid).map(|(_, e)| e),
        };
        uuid
      }
      Event::InstallFinished { uuid, result } => {
        if self.apply_install(uuid, result).await? {
          // in the background, the event loop must not wait for the lock
          let link = self.clone();
          spawn(async move { link.put_server(uuid).await });
        }
        uuid
      }
      Event::Stopped { uuid, generation } => {
        self.apply_stopped(uuid, generation).await?;
        uuid
      }
    };
    self
      .updater
      .broadcast(UpdateMessage::Servers { uuid })
      .await;
    Ok(())
  }

  /// Forgets everything the node reported, it is unknown while disconnected.
  async fn reset(&self) {
    *self.runtime.write().unwrap() = None;
    self.synced.store(false, Ordering::Relaxed);
    let servers: Vec<Uuid> = self.states.iter().map(|e| *e.key()).collect();
    self.states.clear();
    self.errors.clear();
    for uuid in servers {
      self
        .updater
        .broadcast(UpdateMessage::Servers { uuid })
        .await;
    }
  }
}

/// Whether the process is (about to be) up and would need a kill.
fn is_up(state: ServerState) -> bool {
  matches!(
    state,
    ServerState::Starting | ServerState::Running | ServerState::Stopping
  )
}

async fn connection_task(
  link: Link,
  addr: String,
  token: String,
  mut rx: mpsc::Receiver<WingsMessage>,
  connected: Arc<AtomicBool>,
) {
  let uuid = link.uuid;
  loop {
    let stream = timeout(REQUEST_TIMEOUT, WingsAuth::connect_websocket(&addr, &token))
      .await
      .unwrap_or_else(|_| Err(centaurus::anyhow!("Timed out")));
    match stream {
      Ok(stream) => {
        connected.store(true, Ordering::Relaxed);
        link.updater.broadcast(UpdateMessage::Nodes { uuid }).await;
        debug!("Wings connection to {} established", uuid);

        run_connection(&link, stream, &mut rx).await;

        connected.store(false, Ordering::Relaxed);
        link.reset().await;
        link.updater.broadcast(UpdateMessage::Nodes { uuid }).await;
        warn!("Wings connection to {} lost, reconnecting", uuid);
      }
      Err(err) => {
        warn!(
          "Failed to connect to wings websocket for {}: {:?}",
          uuid, err
        );
        link.set_error(Some(format!("{err:#}"))).await;
      }
    }

    sleep(RECONNECT_DELAY).await;
  }
}

/// Full sync, returns when the next one is due.
async fn sync(link: &Link) -> Instant {
  match link.sync().await {
    Ok(()) => {
      link.set_error(None).await;
      Instant::now() + RESYNC_INTERVAL
    }
    Err(err) => {
      warn!("Failed to sync wings {}: {:?}", link.uuid, err);
      link.set_error(Some(format!("Sync failed: {err:#}"))).await;
      Instant::now() + RESYNC_RETRY
    }
  }
}

/// Runs until the connection is closed, errors or stops answering pings.
async fn run_connection(link: &Link, stream: WsStream, rx: &mut mpsc::Receiver<WingsMessage>) {
  let uuid = link.uuid;
  let (mut sink, mut stream) = stream.split();
  let mut ping = interval(PING_INTERVAL);
  // events wait in the socket until the sync is done
  let mut last_sync = Instant::now();
  let mut sync_at = sync(link).await;
  // pongs waited in the socket during the sync as well
  let mut last_seen = Instant::now();

  loop {
    tokio::select! {
      _ = sleep_until(sync_at) => {
        last_sync = Instant::now();
        sync_at = sync(link).await;
        last_seen = Instant::now();
      }
      _ = link.resync.notified() => {
        sync_at = sync_at.min(last_sync + RESYNC_RETRY);
      }
      msg = stream.next() => {
        let Some(Ok(msg)) = msg else {
          return;
        };
        last_seen = Instant::now();

        match msg {
          tungstenite::Message::Binary(raw_msg) => {
            match serde_json::from_slice::<Event>(&raw_msg) {
              Ok(event) => {
                debug!("Wings event from {}: {:?}", uuid, event);
                if let Err(err) = link.handle(event).await {
                  warn!("Failed to handle wings event from {}: {:?}", uuid, err);
                }
              }
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
