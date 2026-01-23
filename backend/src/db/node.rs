use entity::node;
use sea_orm::{IntoActiveModel, prelude::*};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Node {
  pub id: Uuid,
  pub name: String,
  pub address: String,
  pub port: i16,
  pub secure: bool,
  pub disk_limit_mb: Option<i32>,
  pub memory_limit_mb: Option<i32>,
  pub cpu_limit: Option<i32>,
}

pub struct NodeTable<'db> {
  db: &'db DatabaseConnection,
}

impl<'db> NodeTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  pub async fn create_node(&self, model: Node) -> Result<(), DbErr> {
    let model: node::Model = model.into();
    let model = model.into_active_model();
    model.insert(self.db).await?;
    Ok(())
  }

  pub async fn find_by_name(&self, name: String) -> Result<node::Model, DbErr> {
    let res = node::Entity::find()
      .filter(node::Column::Name.eq(name))
      .one(self.db)
      .await?;

    res.ok_or(DbErr::RecordNotFound("Not Found".into()))
  }

  pub async fn list_nodes(&self) -> Result<Vec<Node>, DbErr> {
    let nodes = node::Entity::find().all(self.db).await?;
    Ok(nodes.into_iter().map(Node::from).collect())
  }
}

impl From<node::Model> for Node {
  fn from(model: node::Model) -> Self {
    Self {
      id: model.id,
      name: model.name,
      address: model.address,
      port: model.port,
      secure: model.secure,
      disk_limit_mb: model.disk_limit_mb,
      memory_limit_mb: model.memory_limit_mb,
      cpu_limit: model.cpu_limit,
    }
  }
}

impl From<Node> for node::Model {
  fn from(node: Node) -> Self {
    Self {
      id: node.id,
      name: node.name,
      address: node.address,
      port: node.port,
      secure: node.secure,
      disk_limit_mb: node.disk_limit_mb,
      memory_limit_mb: node.memory_limit_mb,
      cpu_limit: node.cpu_limit,
    }
  }
}
