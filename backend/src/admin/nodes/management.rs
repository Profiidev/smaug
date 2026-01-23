use axum::{
  Json, Router,
  extract::FromRequest,
  routing::{get, post},
};
use centaurus::{
  bail,
  db::init::Connection,
  error::Result,
  eyre::{Context, ContextCompat},
};
use http::Uri;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::{DBTrait, node::Node};

pub fn router() -> Router {
  Router::new()
    .route("/", post(create_node))
    .route("/", get(list_nodes))
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

async fn create_node(db: Connection, data: CreateNode) -> Result<()> {
  if db.node().find_by_name(data.name.clone()).await.is_ok() {
    bail!("Node with this name already exists");
  }

  let url = Uri::try_from(data.address).context("Invalid Address")?;
  let address = url
    .host()
    .context("Address must contain a valid host")?
    .to_string();
  let port = url.port_u16().unwrap_or(if data.secure { 443 } else { 80 }) as i16;

  let model = Node {
    id: Uuid::new_v4(),
    name: data.name,
    address,
    port,
    secure: data.secure,
    disk_limit_mb: data.disk_limit_mb.map(|v| v as i32),
    memory_limit_mb: data.memory_limit_mb.map(|v| v as i32),
    cpu_limit: data.cpu_limit.map(|v| v as i32),
  };

  db.node().create_node(model).await?;

  Ok(())
}

async fn list_nodes(db: Connection) -> Result<Json<Vec<Node>>> {
  let nodes = db.node().list_nodes().await?;
  Ok(Json(nodes))
}
