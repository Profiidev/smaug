use entity::{group, group_permission, group_user, user};
use sea_orm::{IntoActiveModel, JoinType, QuerySelect, Set, prelude::*};
use serde::{Deserialize, Serialize};

pub struct GroupTable<'db> {
  db: &'db DatabaseConnection,
}

#[derive(Serialize, Deserialize)]
pub struct GroupInfo {
  pub id: Uuid,
  pub name: String,
  pub permissions: Vec<String>,
  pub users: Vec<SimpleUserInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct SimpleUserInfo {
  pub id: Uuid,
  pub name: String,
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

  pub async fn get_group_users(&self, group_id: Uuid) -> Result<Vec<SimpleUserInfo>, DbErr> {
    let users = group_user::Entity::find()
      .filter(group_user::Column::GroupId.eq(group_id))
      .find_also_related(user::Entity)
      .all(self.db)
      .await?
      .into_iter()
      .filter_map(|(_, user)| {
        user.map(|u| SimpleUserInfo {
          id: u.id,
          name: u.name,
        })
      })
      .collect();

    Ok(users)
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

  pub async fn get_user_permissions(&self, user_id: Uuid) -> Result<Vec<String>, DbErr> {
    let group_permissions = group_permission::Entity::find()
      .join(JoinType::InnerJoin, group_permission::Relation::Group.def())
      .join(JoinType::InnerJoin, group::Relation::GroupUser.def())
      .filter(group_user::Column::UserId.eq(user_id))
      .all(self.db)
      .await?;

    let permissions = group_permissions
      .into_iter()
      .map(|gp| gp.permission)
      .collect();

    Ok(permissions)
  }

  pub async fn list_groups(&self) -> Result<Vec<GroupInfo>, DbErr> {
    let groups = group::Entity::find().all(self.db).await?;
    let group_user = groups
      .load_many_to_many(user::Entity, group_user::Entity, self.db)
      .await?;
    let group_permissions = groups.load_many(group_permission::Entity, self.db).await?;

    let result = groups
      .into_iter()
      .zip(group_user.into_iter())
      .zip(group_permissions.into_iter())
      .map(|((group, users), permissions)| GroupInfo {
        id: group.id,
        name: group.name,
        users: users
          .into_iter()
          .map(|user| SimpleUserInfo {
            id: user.id,
            name: user.name,
          })
          .collect(),
        permissions: permissions.into_iter().map(|gp| gp.permission).collect(),
      })
      .collect();

    Ok(result)
  }

  pub async fn group_info(&self, group_id: Uuid) -> Result<Option<GroupInfo>, DbErr> {
    let group = group::Entity::find_by_id(group_id).one(self.db).await?;
    let Some(group) = group else {
      return Ok(None);
    };

    let permissions = self.get_group_permissions(group_id).await?;
    let users = self.get_group_users(group_id).await?;

    Ok(Some(GroupInfo {
      id: group.id,
      name: group.name,
      permissions,
      users,
    }))
  }

  pub async fn delete_group(&self, group_id: Uuid) -> Result<(), DbErr> {
    group::Entity::delete_by_id(group_id).exec(self.db).await?;
    Ok(())
  }

  pub async fn find_group_by_name(&self, name: &str) -> Result<Option<Uuid>, DbErr> {
    let group = group::Entity::find()
      .filter(group::Column::Name.eq(name))
      .one(self.db)
      .await?;

    Ok(group.map(|g| g.id))
  }

  pub async fn edit_group(
    &self,
    uuid: Uuid,
    name: String,
    permissions: Vec<String>,
    users: Vec<Uuid>,
  ) -> Result<(), DbErr> {
    // Update group name
    let mut group_model = group::Entity::find_by_id(uuid)
      .one(self.db)
      .await?
      .ok_or(DbErr::RecordNotFound("Group not found".to_string()))?
      .into_active_model();
    group_model.name = Set(name);
    group_model.update(self.db).await?;

    // Clear existing permissions and users
    group_permission::Entity::delete_many()
      .filter(group_permission::Column::GroupId.eq(uuid))
      .exec(self.db)
      .await?;
    group_user::Entity::delete_many()
      .filter(group_user::Column::GroupId.eq(uuid))
      .exec(self.db)
      .await?;

    // Add new permissions and users
    self.add_permissions_to_group(uuid, permissions).await?;
    self.add_user_to_groups(uuid, users).await?;

    Ok(())
  }
}
