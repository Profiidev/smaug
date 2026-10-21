use std::sync::Arc;

use aide::OperationIo;
use axum::{Extension, extract::FromRequestParts};
use centaurus::{bail, db::init::Connection, error::Result};
use dashmap::DashMap;
use shared::msg::{Power, Runtime, ServerState};
use tokio::spawn;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
  db::DBTrait,
  nodes::connection::{Link, WingsConnection},
  utils::Updater,
};

#[derive(Clone, FromRequestParts, OperationIo)]
#[from_request(via(Extension))]
pub struct Wings {
  wings: Arc<DashMap<Uuid, WingsConnection>>,
  updater: Updater,
  db: Connection,
  allow_insecure: bool,
}

impl Wings {
  pub async fn new(db: &Connection, updater: Updater, allow_insecure: bool) -> Result<Self> {
    let wings = Self {
      wings: Arc::new(DashMap::new()),
      updater,
      db: db.clone(),
      allow_insecure,
    };

    for node in db.node().list_nodes().await? {
      if let Err(err) = wings.connect(node.id, &node.address, node.port, node.secure, &node.token) {
        warn!("Not connecting to node {}: {:?}", node.id, err);
      }
    }

    Ok(wings)
  }

  /// Connects to a wings node, replacing any existing connection to it.
  pub fn connect(
    &self,
    uuid: Uuid,
    addr: &str,
    port: u16,
    secure: bool,
    token: &str,
  ) -> Result<()> {
    self.check_secure(secure)?;

    let lock = self
      .wings
      .get(&uuid)
      .map(|conn| conn.lock())
      .unwrap_or_default();
    let conn = WingsConnection::new(
      uuid,
      addr,
      port,
      secure,
      token.to_string(),
      self.db.clone(),
      self.updater.clone(),
      lock,
    );
    self.wings.insert(uuid, conn);
    Ok(())
  }

  /// Fails for a connection `connect` would refuse.
  pub fn check_secure(&self, secure: bool) -> Result<()> {
    if !secure && !self.allow_insecure {
      bail!(BAD_REQUEST, "Insecure wings connections are disabled");
    }
    Ok(())
  }

  pub fn disconnect(&self, uuid: Uuid) {
    self.wings.remove(&uuid);
  }

  pub fn is_connected(&self, uuid: Uuid) -> bool {
    self
      .wings
      .get(&uuid)
      .is_some_and(|conn| conn.is_connected())
  }

  pub fn runtime(&self, uuid: Uuid) -> Option<Runtime> {
    self.wings.get(&uuid).and_then(|conn| conn.runtime())
  }

  /// Why the node can not be reached or synced.
  pub fn node_error(&self, uuid: Uuid) -> Option<String> {
    self.wings.get(&uuid).and_then(|conn| conn.node_error())
  }

  /// Runtime state of `server` on `node`, `None` while unknown.
  pub fn state(&self, node: Uuid, server: Uuid) -> Option<ServerState> {
    self.wings.get(&node).and_then(|conn| conn.state(server))
  }

  // Server calls are best-effort: skipped while the node is disconnected and
  // only logged on failure. The full sync on reconnect, or the one a failure
  // triggers, fixes any drift. They run in the background, a request never
  // waits behind a slow sync; the spec is read from the DB when it is sent.

  /// Sends the server's spec as currently stored in the DB.
  pub fn put_server(&self, node: Uuid, server: Uuid) {
    if let Some(link) = self.link(node) {
      spawn(async move { link.put_server(server).await });
    }
  }

  /// `keep_files` leaves the server's files on the node, for a transfer.
  pub fn delete_server(&self, node: Uuid, server: Uuid, keep_files: bool) {
    if let Some(link) = self.link(node) {
      spawn(async move { link.delete_server(server, keep_files).await });
    }
  }

  pub fn power(&self, node: Uuid, server: Uuid, action: Power) {
    if let Some(link) = self.link(node) {
      spawn(async move { link.power(server, action).await });
    }
  }

  /// Last error wings reported for `server` on `node`.
  pub fn error(&self, node: Uuid, server: Uuid) -> Option<String> {
    self.wings.get(&node).and_then(|conn| conn.error(server))
  }

  fn link(&self, node: Uuid) -> Option<Link> {
    let link = self.wings.get(&node).and_then(|conn| conn.link());
    if link.is_none() {
      debug!("Wings {} is not connected, skipping request", node);
    }
    link
  }
}
