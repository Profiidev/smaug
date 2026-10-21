use std::sync::Arc;

use aide::OperationIo;
use axum::{Extension, extract::FromRequestParts};
use centaurus::{bail, db::init::Connection, error::Result};
use dashmap::DashMap;
use reqwest::Method;
use shared::msg::{Power, Runtime, ServerSpec, ServerState};
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
    if !secure && !self.allow_insecure {
      bail!(BAD_REQUEST, "Insecure wings connections are disabled");
    }

    let conn = WingsConnection::new(
      uuid,
      addr,
      port,
      secure,
      token.to_string(),
      self.db.clone(),
      self.updater.clone(),
    );
    self.wings.insert(uuid, conn);
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

  /// Runtime state of `server` on `node`, `None` while unknown.
  pub fn state(&self, node: Uuid, server: Uuid) -> Option<ServerState> {
    self.wings.get(&node).and_then(|conn| conn.state(server))
  }

  // Server calls are best-effort: skipped while the node is disconnected and
  // only logged on failure. The full sync on reconnect fixes any drift.

  pub async fn put_server(&self, node: Uuid, spec: &ServerSpec) {
    if let Some(link) = self.link(node) {
      let body = serde_json::to_value(spec).ok();
      link
        .push(Method::PUT, &format!("/servers/{}", spec.uuid), body)
        .await;
    }
  }

  pub async fn delete_server(&self, node: Uuid, server: Uuid) {
    if let Some(link) = self.link(node) {
      link
        .push(Method::DELETE, &format!("/servers/{server}"), None)
        .await;
    }
  }

  pub async fn power(&self, node: Uuid, server: Uuid, action: Power) {
    if let Some(link) = self.link(node) {
      link.power(server, action).await;
    }
  }

  fn link(&self, node: Uuid) -> Option<Link> {
    let link = self.wings.get(&node).and_then(|conn| conn.link());
    if link.is_none() {
      debug!("Wings {} is not connected, skipping request", node);
    }
    link
  }
}
