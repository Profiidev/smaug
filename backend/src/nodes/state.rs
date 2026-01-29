use std::sync::Arc;

use axum::{Extension, extract::FromRequestParts};
use centaurus::{db::init::Connection, error::Result};
use dashmap::DashMap;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{db::DBTrait, nodes::connection::WingsConnection, ws::state::Updater};

#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
pub struct Wings {
  wings: DashMap<Uuid, Arc<Mutex<WingsConnection>>>,
  updater: Updater,
}

impl Wings {
  pub async fn new(db: &Connection, updater: Updater) -> Result<Self> {
    let nodes = db.node().list_nodes().await?;
    let wings = DashMap::new();

    for node in nodes {
      let conn = WingsConnection::new(
        node.id,
        &node.address,
        node.port,
        node.secure,
        node.token.clone(),
        updater.clone(),
      )
      .await?;

      wings.insert(node.id, conn);
    }

    Ok(Self { wings, updater })
  }

  pub async fn connect(
    &self,
    uuid: Uuid,
    addr: &str,
    port: i16,
    secure: bool,
    token: &str,
  ) -> Result<()> {
    let conn = WingsConnection::new(
      uuid,
      addr,
      port,
      secure,
      token.to_string(),
      self.updater.clone(),
    )
    .await?;
    self.wings.insert(uuid, conn);
    Ok(())
  }

  pub async fn disconnect(&self, uuid: Uuid) -> Result<()> {
    if let Some((_, conn)) = self.wings.remove(&uuid) {
      conn.lock().await.disconnect();
    }
    Ok(())
  }

  pub async fn is_connected(&self, uuid: Uuid) -> bool {
    if let Some(conn) = self.wings.get(&uuid) {
      return conn.lock().await.is_connected();
    }
    false
  }
}
