use entity::node;
use sea_orm::{IntoActiveModel, prelude::*};

pub struct NodeTable<'db> {
  db: &'db DatabaseConnection,
}

impl<'db> NodeTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  pub async fn create_node(&self, model: node::Model) -> Result<(), DbErr> {
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

  pub async fn list_nodes(&self) -> Result<Vec<node::Model>, DbErr> {
    let nodes = node::Entity::find().all(self.db).await?;
    Ok(nodes)
  }
}
