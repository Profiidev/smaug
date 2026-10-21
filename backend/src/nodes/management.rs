use aide::axum::{
  ApiRouter,
  routing::{delete_with, get_with, post_with},
};
use axum::{Json, extract::Path};
use centaurus::{
  bail,
  db::init::Connection,
  error::{ErrorReportStatusExt, Result},
};
use http::{StatusCode, Uri};
use rand::Rng;
use schemars::JsonSchema;
use sea_orm::{IntoActiveModel, Set};
use serde::{Deserialize, Serialize};
use shared::msg::Runtime;
use tracing::info;
use uuid::Uuid;

use crate::{
  auth::jwt_auth::JwtAuth,
  db::{DBTrait, node::Node},
  nodes::state::Wings,
  utils::{NodeEditPerm, NodeViewPerm, UpdateMessage, Updater},
};

pub fn router() -> ApiRouter {
  ApiRouter::new()
    .api_route("/", post_with(create_node, |op| op.id("createNode")))
    .api_route("/", get_with(list_nodes, |op| op.id("listNodes")))
    .api_route("/", delete_with(delete_node, |op| op.id("deleteNode")))
    .api_route("/{uuid}", get_with(node_info, |op| op.id("nodeInfo")))
    .api_route("/{uuid}", post_with(update_node, |op| op.id("updateNode")))
    .api_route(
      "/{uuid}/token",
      post_with(regenerate_token, |op| op.id("nodeTokenRegenerate")),
    )
}

fn gen_token() -> String {
  let mut raw_token = [0u8; 32];
  rand::rng().fill_bytes(&mut raw_token);
  hex::encode(raw_token)
}

#[derive(Deserialize, JsonSchema)]
struct CreateNode {
  name: String,
  address: String,
  secure: bool,
  disk_limit_mb: Option<f64>,
  memory_limit_mb: Option<f64>,
  cpu_limit: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
struct CreateNodeRes {
  uuid: Uuid,
  /// Only returned here and on regenerate, it can not be viewed later.
  token: String,
}

async fn create_node(
  _auth: JwtAuth<NodeEditPerm>,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Json(data): Json<CreateNode>,
) -> Result<Json<CreateNodeRes>> {
  if db.node().find_by_name(data.name.clone()).await.is_ok() {
    bail!(CONFLICT, "Node with this name already exists");
  }

  if let Some(disk) = data.disk_limit_mb
    && disk < 0.0
  {
    bail!(BAD_REQUEST, "Disk and Memory limits must be non-negative");
  }
  if let Some(memory) = data.memory_limit_mb
    && memory < 0.0
  {
    bail!(BAD_REQUEST, "Disk and Memory limits must be non-negative");
  }

  let url =
    Uri::try_from(data.address).status_context(StatusCode::BAD_REQUEST, "Invalid Address")?;
  let address = url
    .host()
    .status_context(StatusCode::BAD_REQUEST, "Address must contain a valid host")?
    .to_string();
  let port = url.port_u16().unwrap_or(if data.secure { 443 } else { 80 });

  wings.check_secure(data.secure)?;
  let token = gen_token();
  let id = Uuid::now_v7();

  let model = Node {
    id,
    name: data.name,
    address: address.clone(),
    port,
    secure: data.secure,
    disk_limit_mb: data.disk_limit_mb,
    memory_limit_mb: data.memory_limit_mb,
    cpu_limit: data.cpu_limit.map(|v| v as i32),
    token: token.clone(),
  };

  db.node().create_node(model).await?;
  wings.connect(id, &address, port, data.secure, &token)?;
  info!("Created node with ID {}", id);

  updater.broadcast(UpdateMessage::Nodes { uuid: id }).await;

  Ok(Json(CreateNodeRes { uuid: id, token }))
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct NodeInfo {
  pub id: Uuid,
  pub name: String,
  pub address: String,
  pub port: u16,
  pub secure: bool,
  pub disk_limit_mb: Option<f64>,
  pub memory_limit_mb: Option<f64>,
  pub cpu_limit: Option<i32>,
  pub connected: bool,
  /// Reported by wings, `None` while disconnected.
  pub runtime: Option<Runtime>,
}

impl NodeInfo {
  fn from_node(node: Node, wings: &Wings) -> Self {
    NodeInfo {
      id: node.id,
      name: node.name,
      address: node.address,
      port: node.port,
      secure: node.secure,
      disk_limit_mb: node.disk_limit_mb,
      memory_limit_mb: node.memory_limit_mb,
      cpu_limit: node.cpu_limit,
      connected: wings.is_connected(node.id),
      runtime: wings.runtime(node.id),
    }
  }
}

async fn list_nodes(
  _auth: JwtAuth<NodeViewPerm>,
  db: Connection,
  wings: Wings,
) -> Result<Json<Vec<NodeInfo>>> {
  let nodes = db.node().list_nodes().await?;
  let mut node_infos = Vec::new();

  for node in nodes {
    node_infos.push(NodeInfo::from_node(node, &wings));
  }

  Ok(Json(node_infos))
}

#[derive(Deserialize, JsonSchema)]
struct DeleteNode {
  uuid: Uuid,
}

async fn delete_node(
  _auth: JwtAuth<NodeEditPerm>,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Json(data): Json<DeleteNode>,
) -> Result<()> {
  if db.server().count_by_node(data.uuid).await? > 0 {
    bail!(CONFLICT, "Node is in use by servers");
  }
  if db.node().delete_node(data.uuid).await? == 0 {
    bail!(NOT_FOUND, "Node not found");
  }
  wings.disconnect(data.uuid);
  info!("Deleted node with ID {}", data.uuid);

  updater
    .broadcast(UpdateMessage::Nodes { uuid: data.uuid })
    .await;

  Ok(())
}

#[derive(Deserialize, JsonSchema)]
struct NodeInfoRequest {
  uuid: Uuid,
}

async fn node_info(
  _auth: JwtAuth<NodeViewPerm>,
  db: Connection,
  wings: Wings,
  Path(req): Path<NodeInfoRequest>,
) -> Result<Json<NodeInfo>> {
  let node = db.node().find_by_id(req.uuid).await?;
  let node_info = NodeInfo::from_node(node.into(), &wings);

  Ok(Json(node_info))
}

#[derive(Deserialize, JsonSchema)]
struct UpdateNode {
  name: String,
  address: String,
  secure: bool,
  disk_limit_mb: Option<f64>,
  memory_limit_mb: Option<f64>,
  cpu_limit: Option<u32>,
}

async fn update_node(
  _auth: JwtAuth<NodeEditPerm>,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Path(req): Path<NodeInfoRequest>,
  Json(data): Json<UpdateNode>,
) -> Result<()> {
  if let Some(disk) = data.disk_limit_mb
    && disk < 0.0
  {
    bail!(BAD_REQUEST, "Disk and Memory limits must be non-negative");
  }
  if let Some(memory) = data.memory_limit_mb
    && memory < 0.0
  {
    bail!(BAD_REQUEST, "Disk and Memory limits must be non-negative");
  }

  let read_node = db.node().find_by_id(req.uuid).await?;
  let mut node = read_node.clone().into_active_model();

  if read_node.name != data.name {
    if let Ok(node) = db.node().find_by_name(data.name.clone()).await
      && node.id != req.uuid
    {
      bail!(CONFLICT, "Node with this name already exists");
    }
    node.name = Set(data.name);
  }

  let url =
    Uri::try_from(data.address).status_context(StatusCode::BAD_REQUEST, "Invalid Address")?;
  let address = url
    .host()
    .status_context(StatusCode::BAD_REQUEST, "Address must contain a valid host")?
    .to_string();
  let port = url.port_u16().unwrap_or(if data.secure { 443 } else { 80 });

  let reconnect = read_node.address != address
    || read_node.port != i32::from(port)
    || read_node.secure != data.secure;
  if reconnect {
    wings.check_secure(data.secure)?;
    node.address = Set(address.clone());
    node.port = Set(i32::from(port));
    node.secure = Set(data.secure);
  }

  node.disk_limit_mb = Set(data.disk_limit_mb);
  node.memory_limit_mb = Set(data.memory_limit_mb);
  node.cpu_limit = Set(data.cpu_limit.map(|v| v as i32));

  db.node().update_node(node).await?;
  if reconnect {
    // replaces the existing connection
    wings.connect(read_node.id, &address, port, data.secure, &read_node.token)?;
  }
  info!("Updated node with ID {}", req.uuid);
  updater
    .broadcast(UpdateMessage::Nodes { uuid: req.uuid })
    .await;

  Ok(())
}

#[derive(Serialize, JsonSchema)]
struct TokenRegenerateRes {
  token: String,
}

async fn regenerate_token(
  _auth: JwtAuth<NodeEditPerm>,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Path(req): Path<NodeInfoRequest>,
) -> Result<Json<TokenRegenerateRes>> {
  let read_node = db.node().find_by_id(req.uuid).await?;
  let token = gen_token();

  let mut node = read_node.clone().into_active_model();
  node.token = Set(token.clone());
  db.node().update_node(node).await?;

  if let Err(err) = wings.connect(
    read_node.id,
    &read_node.address,
    read_node.port as u16,
    read_node.secure,
    &token,
  ) {
    // old token is invalid now, drop the connection that still uses it
    wings.disconnect(read_node.id);
    info!(
      "Not reconnecting node {} after token regenerate: {:?}",
      req.uuid, err
    );
  }
  info!("Regenerated token for node with ID {}", req.uuid);

  updater
    .broadcast(UpdateMessage::Nodes { uuid: req.uuid })
    .await;

  Ok(Json(TokenRegenerateRes { token }))
}
