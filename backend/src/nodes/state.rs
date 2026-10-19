use std::sync::Arc;

use aide::OperationIo;
use axum::{Extension, extract::FromRequestParts};
use centaurus::{bail, db::init::Connection, error::Result};
use dashmap::DashMap;
use tracing::warn;
use uuid::Uuid;

use crate::{db::DBTrait, nodes::connection::WingsConnection, utils::Updater};

#[derive(Clone, FromRequestParts, OperationIo)]
#[from_request(via(Extension))]
pub struct Wings {
  wings: Arc<DashMap<Uuid, WingsConnection>>,
  updater: Updater,
  allow_insecure: bool,
}

impl Wings {
  pub async fn new(db: &Connection, updater: Updater, allow_insecure: bool) -> Result<Self> {
    let wings = Self {
      wings: Arc::new(DashMap::new()),
      updater,
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
}
