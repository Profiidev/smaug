use std::collections::{HashMap, HashSet};

use centaurus::error::ErrorReportStatusExt;
use entity::{
  egg_variable, group_user, sea_orm_active_enums::ServerAccessLevel, server, server_allocation,
  server_group_access, server_user_access, server_variable,
};
use http::StatusCode;
use sea_orm::{Condition, ExprTrait, QueryOrder, TransactionTrait, prelude::*};

pub struct ServerTable<'db> {
  db: &'db DatabaseConnection,
}

/// Replacement grant lists, applied together with an update.
pub struct Grants {
  pub users: Vec<server_user_access::Model>,
  pub groups: Vec<server_group_access::Model>,
}

impl<'db> ServerTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  /// All servers (`None`) or only the given ids, ordered by name.
  pub async fn list(&self, ids: Option<Vec<Uuid>>) -> Result<Vec<server::Model>, DbErr> {
    let mut query = server::Entity::find().order_by_asc(server::Column::Name);
    if let Some(ids) = ids {
      query = query.filter(server::Column::Id.is_in(ids));
    }
    query.all(self.db).await
  }

  pub async fn find_by_id(&self, id: Uuid) -> centaurus::error::Result<server::Model> {
    server::Entity::find_by_id(id)
      .one(self.db)
      .await?
      .status_context(StatusCode::NOT_FOUND, "Server not found")
  }

  /// Allocations of the given servers, primary first.
  pub async fn allocations(
    &self,
    server_ids: Vec<Uuid>,
  ) -> Result<Vec<server_allocation::Model>, DbErr> {
    server_allocation::Entity::find()
      .filter(server_allocation::Column::ServerId.is_in(server_ids))
      .order_by_asc(server_allocation::Column::Position)
      .all(self.db)
      .await
  }

  pub async fn variables(&self, id: Uuid) -> Result<HashMap<String, String>, DbErr> {
    Ok(
      server_variable::Entity::find()
        .filter(server_variable::Column::ServerId.eq(id))
        .all(self.db)
        .await?
        .into_iter()
        .map(|v| (v.env_variable, v.value))
        .collect(),
    )
  }

  pub async fn grants(&self, id: Uuid) -> Result<Grants, DbErr> {
    Ok(Grants {
      users: server_user_access::Entity::find()
        .filter(server_user_access::Column::ServerId.eq(id))
        .all(self.db)
        .await?,
      groups: server_group_access::Entity::find()
        .filter(server_group_access::Column::ServerId.eq(id))
        .all(self.db)
        .await?,
    })
  }

  /// `(server, level)` for every grant that applies to `user`, directly or
  /// through one of its groups; limited to `server` when given.
  pub async fn grants_for_user(
    &self,
    user: Uuid,
    server: Option<Uuid>,
  ) -> Result<Vec<(Uuid, ServerAccessLevel)>, DbErr> {
    let groups: Vec<Uuid> = group_user::Entity::find()
      .filter(group_user::Column::UserId.eq(user))
      .all(self.db)
      .await?
      .into_iter()
      .map(|g| g.group_id)
      .collect();

    let mut users =
      server_user_access::Entity::find().filter(server_user_access::Column::UserId.eq(user));
    let mut group_grants = server_group_access::Entity::find()
      .filter(server_group_access::Column::GroupId.is_in(groups));
    if let Some(server) = server {
      users = users.filter(server_user_access::Column::ServerId.eq(server));
      group_grants = group_grants.filter(server_group_access::Column::ServerId.eq(server));
    }

    let mut out: Vec<_> = users
      .all(self.db)
      .await?
      .into_iter()
      .map(|g| (g.server_id, g.access))
      .collect();
    out.extend(
      group_grants
        .all(self.db)
        .await?
        .into_iter()
        .map(|g| (g.server_id, g.access)),
    );
    Ok(out)
  }

  pub async fn owned_ids(&self, user: Uuid) -> Result<Vec<Uuid>, DbErr> {
    Ok(
      server::Entity::find()
        .filter(server::Column::OwnerId.eq(user))
        .all(self.db)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect(),
    )
  }

  /// The `(ip, port)` pairs already used on `node` by servers other than `exclude`.
  pub async fn taken(
    &self,
    node: Uuid,
    pairs: &[(String, i32)],
    exclude: Option<Uuid>,
  ) -> Result<HashSet<(String, i32)>, DbErr> {
    if pairs.is_empty() {
      return Ok(HashSet::new());
    }
    let mut any = Condition::any();
    for (ip, port) in pairs {
      any = any.add(
        server_allocation::Column::Ip
          .eq(ip)
          .and(server_allocation::Column::Port.eq(*port)),
      );
    }
    let mut query = server_allocation::Entity::find()
      .filter(server_allocation::Column::NodeId.eq(node))
      .filter(any);
    if let Some(id) = exclude {
      query = query.filter(server_allocation::Column::ServerId.ne(id));
    }
    Ok(
      query
        .all(self.db)
        .await?
        .into_iter()
        .map(|a| (a.ip, a.port))
        .collect(),
    )
  }

  pub async fn count_by_node(&self, node: Uuid) -> Result<u64, DbErr> {
    server::Entity::find()
      .filter(server::Column::NodeId.eq(node))
      .count(self.db)
      .await
  }

  pub async fn count_by_egg(&self, egg: Uuid) -> Result<u64, DbErr> {
    server::Entity::find()
      .filter(server::Column::EggId.eq(egg))
      .count(self.db)
      .await
  }

  pub async fn create(
    &self,
    server: server::Model,
    allocations: Vec<(String, i32)>,
    variables: Vec<(String, String)>,
  ) -> Result<(), DbErr> {
    let txn = self.db.begin().await?;
    let (id, node) = (server.id, server.node_id);
    server::ActiveModel::from(server).insert(&txn).await?;
    insert_allocations(&txn, id, node, allocations).await?;
    insert_variables(&txn, id, variables).await?;
    txn.commit().await
  }

  /// Replaces every column, all allocations and variables and, when given, all grants.
  pub async fn update(
    &self,
    server: server::Model,
    allocations: Vec<(String, i32)>,
    variables: Vec<(String, String)>,
    grants: Option<Grants>,
  ) -> Result<(), DbErr> {
    let txn = self.db.begin().await?;
    let (id, node) = (server.id, server.node_id);
    server::ActiveModel::from(server)
      .reset_all()
      .update(&txn)
      .await?;
    server_allocation::Entity::delete_many()
      .filter(server_allocation::Column::ServerId.eq(id))
      .exec(&txn)
      .await?;
    insert_allocations(&txn, id, node, allocations).await?;
    server_variable::Entity::delete_many()
      .filter(server_variable::Column::ServerId.eq(id))
      .exec(&txn)
      .await?;
    insert_variables(&txn, id, variables).await?;

    if let Some(grants) = grants {
      server_user_access::Entity::delete_many()
        .filter(server_user_access::Column::ServerId.eq(id))
        .exec(&txn)
        .await?;
      server_group_access::Entity::delete_many()
        .filter(server_group_access::Column::ServerId.eq(id))
        .exec(&txn)
        .await?;
      if !grants.users.is_empty() {
        server_user_access::Entity::insert_many(
          grants
            .users
            .into_iter()
            .map(server_user_access::ActiveModel::from),
        )
        .exec(&txn)
        .await?;
      }
      if !grants.groups.is_empty() {
        server_group_access::Entity::insert_many(
          grants
            .groups
            .into_iter()
            .map(server_group_access::ActiveModel::from),
        )
        .exec(&txn)
        .await?;
      }
    }
    txn.commit().await
  }

  /// Saves the server columns only.
  pub async fn save(&self, server: server::Model) -> Result<(), DbErr> {
    server::ActiveModel::from(server)
      .reset_all()
      .update(self.db)
      .await?;
    Ok(())
  }

  /// Moves the server to `server.node_id` with `allocation` as its only allocation.
  pub async fn transfer(
    &self,
    server: server::Model,
    allocation: (String, i32),
  ) -> Result<(), DbErr> {
    let txn = self.db.begin().await?;
    let (id, node) = (server.id, server.node_id);
    server_allocation::Entity::delete_many()
      .filter(server_allocation::Column::ServerId.eq(id))
      .exec(&txn)
      .await?;
    server::ActiveModel::from(server)
      .reset_all()
      .update(&txn)
      .await?;
    insert_allocations(&txn, id, node, vec![allocation]).await?;
    txn.commit().await
  }

  /// Allocations, variables and grants go with it via the FK cascade.
  pub async fn delete(&self, id: Uuid) -> Result<u64, DbErr> {
    Ok(
      server::Entity::delete_by_id(id)
        .exec(self.db)
        .await?
        .rows_affected,
    )
  }
}

async fn insert_allocations(
  txn: &impl ConnectionTrait,
  server: Uuid,
  node: Uuid,
  allocations: Vec<(String, i32)>,
) -> Result<(), DbErr> {
  if allocations.is_empty() {
    return Ok(());
  }
  server_allocation::Entity::insert_many(allocations.into_iter().enumerate().map(
    |(i, (ip, port))| {
      server_allocation::ActiveModel::from(server_allocation::Model {
        id: Uuid::now_v7(),
        server_id: server,
        node_id: node,
        ip,
        port,
        position: i as i16,
      })
    },
  ))
  .exec(txn)
  .await?;
  Ok(())
}

async fn insert_variables(
  txn: &impl ConnectionTrait,
  server: Uuid,
  variables: Vec<(String, String)>,
) -> Result<(), DbErr> {
  if variables.is_empty() {
    return Ok(());
  }
  server_variable::Entity::insert_many(variables.into_iter().map(|(env_variable, value)| {
    server_variable::ActiveModel::from(server_variable::Model {
      server_id: server,
      env_variable,
      value,
    })
  }))
  .exec(txn)
  .await?;
  Ok(())
}

/// After an egg update: servers that did not override their startup command
/// get `startup`, variables the egg dropped are removed and new ones are added
/// with their default. Returns the ids of the egg's servers.
pub async fn sync_egg(
  txn: &impl ConnectionTrait,
  egg: Uuid,
  startup: &str,
  vars: &[egg_variable::Model],
) -> Result<Vec<Uuid>, DbErr> {
  let ids: Vec<Uuid> = server::Entity::find()
    .filter(server::Column::EggId.eq(egg))
    .all(txn)
    .await?
    .into_iter()
    .map(|s| s.id)
    .collect();
  if ids.is_empty() {
    return Ok(ids);
  }

  server::Entity::update_many()
    .col_expr(server::Column::Startup, Expr::value(startup))
    .filter(server::Column::EggId.eq(egg))
    .filter(server::Column::StartupOverridden.eq(false))
    .exec(txn)
    .await?;

  let envs: Vec<&str> = vars.iter().map(|v| v.env_variable.as_str()).collect();
  server_variable::Entity::delete_many()
    .filter(server_variable::Column::ServerId.is_in(ids.clone()))
    .filter(server_variable::Column::EnvVariable.is_not_in(envs))
    .exec(txn)
    .await?;

  let existing: HashSet<(Uuid, String)> = server_variable::Entity::find()
    .filter(server_variable::Column::ServerId.is_in(ids.clone()))
    .all(txn)
    .await?
    .into_iter()
    .map(|v| (v.server_id, v.env_variable))
    .collect();
  let missing: Vec<_> = ids
    .iter()
    .flat_map(|id| vars.iter().map(move |v| (*id, v)))
    .filter(|(id, v)| !existing.contains(&(*id, v.env_variable.clone())))
    .map(|(id, v)| {
      server_variable::ActiveModel::from(server_variable::Model {
        server_id: id,
        env_variable: v.env_variable.clone(),
        value: v.default_value.clone(),
      })
    })
    .collect();
  if !missing.is_empty() {
    server_variable::Entity::insert_many(missing)
      .exec(txn)
      .await?;
  }
  Ok(ids)
}
