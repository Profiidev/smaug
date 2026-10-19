use centaurus::error::ErrorReportStatusExt;
use entity::{egg, egg_variable};
use http::StatusCode;
use sea_orm::{QueryOrder, TransactionTrait, prelude::*};

pub struct EggTable<'db> {
  db: &'db DatabaseConnection,
}

impl<'db> EggTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  pub async fn list_eggs(&self) -> Result<Vec<egg::Model>, DbErr> {
    egg::Entity::find()
      .order_by_asc(egg::Column::Name)
      .all(self.db)
      .await
  }

  /// Egg plus its variables, ordered by `sort` (nulls last), then insertion (v7 id).
  pub async fn find_by_id(
    &self,
    id: Uuid,
  ) -> centaurus::error::Result<(egg::Model, Vec<egg_variable::Model>)> {
    let egg = egg::Entity::find_by_id(id)
      .one(self.db)
      .await?
      .status_context(StatusCode::NOT_FOUND, "Egg not found")?;
    let mut vars = egg_variable::Entity::find()
      .filter(egg_variable::Column::EggId.eq(id))
      .all(self.db)
      .await?;
    vars.sort_by_key(|v| (v.sort.is_none(), v.sort, v.id));
    Ok((egg, vars))
  }

  pub async fn create_egg(
    &self,
    egg: egg::Model,
    vars: Vec<egg_variable::Model>,
  ) -> Result<(), DbErr> {
    let txn = self.db.begin().await?;
    egg::ActiveModel::from(egg).insert(&txn).await?;
    insert_vars(&txn, vars).await?;
    txn.commit().await
  }

  /// Replaces every column and all variables in one transaction.
  pub async fn update_egg(
    &self,
    egg: egg::Model,
    vars: Vec<egg_variable::Model>,
  ) -> Result<(), DbErr> {
    let txn = self.db.begin().await?;
    let id = egg.id;
    egg::ActiveModel::from(egg).reset_all().update(&txn).await?;
    egg_variable::Entity::delete_many()
      .filter(egg_variable::Column::EggId.eq(id))
      .exec(&txn)
      .await?;
    insert_vars(&txn, vars).await?;
    txn.commit().await
  }

  /// Variables go with it via the FK cascade.
  pub async fn delete_egg(&self, id: Uuid) -> Result<u64, DbErr> {
    Ok(
      egg::Entity::delete_by_id(id)
        .exec(self.db)
        .await?
        .rows_affected,
    )
  }
}

async fn insert_vars(
  txn: &impl ConnectionTrait,
  vars: Vec<egg_variable::Model>,
) -> Result<(), DbErr> {
  if !vars.is_empty() {
    egg_variable::Entity::insert_many(vars.into_iter().map(egg_variable::ActiveModel::from))
      .exec(txn)
      .await?;
  }
  Ok(())
}
