use entity::{group, group_permission, group_user};
use sea_orm::{IntoActiveModel, JoinType, QuerySelect, prelude::*};

pub struct GroupTable<'db> {
  db: &'db DatabaseConnection,
}

impl<'db> GroupTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  pub async fn create_group(&self, name: String) -> Result<Uuid, DbErr> {
    let group_id = Uuid::new_v4();
    let model = group::Model { id: group_id, name }.into_active_model();

    model.insert(self.db).await?;

    Ok(group_id)
  }

  pub async fn add_permissions_to_group(
    &self,
    group_id: Uuid,
    permissions: Vec<String>,
  ) -> Result<(), DbErr> {
    let mut models = Vec::new();

    for permission in permissions {
      let model = group_permission::Model {
        group_id,
        permission,
      }
      .into_active_model();
      models.push(model);
    }

    group_permission::Entity::insert_many(models)
      .exec(self.db)
      .await?;

    Ok(())
  }

  pub async fn get_group_permissions(&self, group_id: Uuid) -> Result<Vec<String>, DbErr> {
    let permissions = group_permission::Entity::find()
      .filter(group_permission::Column::GroupId.eq(group_id))
      .all(self.db)
      .await?
      .into_iter()
      .map(|gp| gp.permission)
      .collect();

    Ok(permissions)
  }

  pub async fn add_user_to_groups(&self, user_id: Uuid, group_ids: Vec<Uuid>) -> Result<(), DbErr> {
    let mut models = Vec::new();

    for group_id in group_ids {
      let model = group_user::Model { user_id, group_id }.into_active_model();
      models.push(model);
    }

    group_user::Entity::insert_many(models)
      .exec(self.db)
      .await?;

    Ok(())
  }

  pub async fn user_hash_permissions(
    &self,
    user_id: Uuid,
    permission: &str,
  ) -> Result<bool, DbErr> {
    let res = group_user::Entity::find()
      .join(JoinType::InnerJoin, group_user::Relation::Group.def())
      .join(JoinType::InnerJoin, group::Relation::GroupPermission.def())
      .filter(group_user::Column::UserId.eq(user_id))
      .filter(group_permission::Column::Permission.eq(permission))
      .all(self.db)
      .await?;

    Ok(!res.is_empty())
  }
}
