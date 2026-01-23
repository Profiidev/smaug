use std::{collections::HashMap, sync::Arc};

use axum::{Extension, extract::FromRequestParts};
use centaurus::error::Result;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::admin::nodes::connection::WingsConnection;

#[derive(Default, Clone, FromRequestParts)]
#[from_request(via(Extension))]
pub struct Wings {
  wings: Arc<Mutex<HashMap<Uuid, Arc<Mutex<WingsConnection>>>>>,
}

impl Wings {
  pub async fn connect(
    &self,
    uuid: Uuid,
    addr: &str,
    port: i16,
    secure: bool,
    token: &str,
  ) -> Result<()> {
    let conn = WingsConnection::new(addr, port, secure, token.to_string()).await?;
    self.wings.lock().await.insert(uuid, conn);
    Ok(())
  }

  pub async fn disconnect(&self, uuid: Uuid) -> Result<()> {
    if let Some(conn) = self.wings.lock().await.remove(&uuid) {
      conn.lock().await.disconnect().await;
    }
    Ok(())
  }

  pub async fn is_connected(&self, uuid: Uuid) -> bool {
    if let Some(conn) = self.wings.lock().await.get(&uuid) {
      return conn.lock().await.is_connected();
    }
    false
  }
}
