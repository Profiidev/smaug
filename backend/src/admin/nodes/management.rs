use axum::{
  Json, Router,
  extract::FromRequest,
  routing::{delete, get, post},
};
use centaurus::{
  bail,
  db::init::Connection,
  error::Result,
  eyre::{Context, ContextCompat},
};
use http::Uri;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  admin::nodes::state::Wings,
  db::{DBTrait, node::Node},
};

pub fn router() -> Router {
  Router::new()
    .route("/", post(create_node))
    .route("/", get(list_nodes))
    .route("/", delete(delete_node))
}

#[derive(FromRequest, Deserialize)]
#[from_request(via(Json))]
struct CreateNode {
  name: String,
  address: String,
  secure: bool,
  disk_limit_mb: Option<u32>,
  memory_limit_mb: Option<u32>,
  cpu_limit: Option<u32>,
}

async fn create_node(db: Connection, wings: Wings, data: CreateNode) -> Result<()> {
  if db.node().find_by_name(data.name.clone()).await.is_ok() {
    bail!("Node with this name already exists");
  }

  let url = Uri::try_from(data.address).context("Invalid Address")?;
  let address = url
    .host()
    .context("Address must contain a valid host")?
    .to_string();
  let port = url.port_u16().unwrap_or(if data.secure { 443 } else { 80 }) as i16;

  let mut raw_token = [0u8; 32];
  rand::rng().fill_bytes(&mut raw_token);
  let token = hex::encode(raw_token);

  let id = Uuid::new_v4();

  wings
    .connect(id, &address, port, data.secure, &token)
    .await?;

  let model = Node {
    id,
    name: data.name,
    address,
    port,
    secure: data.secure,
    disk_limit_mb: data.disk_limit_mb.map(|v| v as i32),
    memory_limit_mb: data.memory_limit_mb.map(|v| v as i32),
    cpu_limit: data.cpu_limit.map(|v| v as i32),
    token,
  };

  db.node().create_node(model).await?;

  Ok(())
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct NodeInfo {
  pub id: Uuid,
  pub name: String,
  pub address: String,
  pub port: i16,
  pub secure: bool,
  pub disk_limit_mb: Option<i32>,
  pub memory_limit_mb: Option<i32>,
  pub cpu_limit: Option<i32>,
  pub token: String,
  pub connected: bool,
}

async fn list_nodes(db: Connection, wings: Wings) -> Result<Json<Vec<NodeInfo>>> {
  let nodes = db.node().list_nodes().await?;
  let mut node_infos = Vec::new();

  for node in nodes {
    node_infos.push(NodeInfo {
      id: node.id,
      name: node.name,
      address: node.address,
      port: node.port,
      secure: node.secure,
      disk_limit_mb: node.disk_limit_mb,
      memory_limit_mb: node.memory_limit_mb,
      cpu_limit: node.cpu_limit,
      token: node.token,
      connected: wings.is_connected(node.id).await,
    });
  }

  Ok(Json(node_infos))
}

#[derive(FromRequest, Deserialize)]
#[from_request(via(Json))]
struct DeleteNode {
  uuid: Uuid,
}

async fn delete_node(db: Connection, wings: Wings, data: DeleteNode) -> Result<()> {
  wings.disconnect(data.uuid).await?;

  db.node().delete_node(data.uuid).await?;

  Ok(())
}
