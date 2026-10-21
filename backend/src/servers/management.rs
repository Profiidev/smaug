use std::collections::HashMap;

use axum::{Json, extract::Path};
use centaurus::{
  db::{init::Connection, tables::ConnectionExt},
  error::Result,
};
use chrono::Utc;
use entity::{sea_orm_active_enums::ServerInstallState, server};
use http::StatusCode;
use tracing::info;
use uuid::Uuid;

use super::{
  access::{Access, has_perm, load},
  error::{FieldResult, fail_if, field_err},
  status::{Action, ensure_allowed, status},
  types::{
    Allocation, CreateServer, CreateServerResponse, GrantInfo, GrantsInfo, Limits,
    ListServersResponse, Named, OptionEgg, ServerInfo, ServerListItem, ServerOptions, ServerUuid,
    UpdateServer, VariableInfo,
  },
  validation::{
    apply_spec, build_variables, changes_admin_fields, check_free, check_owner, egg_vars,
    node_exists, validate_grants, validate_spec,
  },
};
use crate::{
  auth::jwt_auth::JwtAuth,
  db::DBTrait,
  eggs::{unpairs, variable_from_model},
  nodes::Wings,
  utils::{ServerEditPerm, ServerViewPerm, UpdateMessage, Updater},
};

async fn names(
  db: &Connection,
) -> Result<(
  HashMap<Uuid, String>,
  HashMap<Uuid, String>,
  HashMap<Uuid, String>,
)> {
  let users = db
    .user()
    .list_users_simple()
    .await?
    .into_iter()
    .map(|u| (u.id, u.name))
    .collect();
  let nodes = db
    .node()
    .list_nodes()
    .await?
    .into_iter()
    .map(|n| (n.id, n.name))
    .collect();
  let eggs = db
    .egg()
    .list_eggs()
    .await?
    .into_iter()
    .map(|e| (e.id, e.name))
    .collect();
  Ok((users, nodes, eggs))
}

fn named(names: &HashMap<Uuid, String>, uuid: Uuid) -> Named {
  Named {
    uuid,
    name: names.get(&uuid).cloned().unwrap_or_default(),
  }
}

pub(super) async fn list_servers(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
) -> Result<Json<ListServersResponse>> {
  let user = auth.user_id;
  let all =
    has_perm::<ServerEditPerm>(&db, user).await? || has_perm::<ServerViewPerm>(&db, user).await?;
  let ids = if all {
    None
  } else {
    let mut ids = db.server().owned_ids(user).await?;
    ids.extend(
      db.server()
        .grants_for_user(user, None)
        .await?
        .into_iter()
        .map(|(id, _)| id),
    );
    Some(ids)
  };
  let servers = db.server().list(ids).await?;

  let mut allocations: HashMap<Uuid, Vec<Allocation>> = HashMap::new();
  for a in db
    .server()
    .allocations(servers.iter().map(|s| s.id).collect())
    .await?
  {
    allocations
      .entry(a.server_id)
      .or_default()
      .push(Allocation {
        ip: a.ip,
        port: a.port,
      });
  }
  // ponytail: loads every user/node/egg name, page or join once lists get big
  let (users, nodes, eggs) = names(&db).await?;

  let servers = servers
    .into_iter()
    .map(|s| {
      let mut allocs = allocations.remove(&s.id).unwrap_or_default().into_iter();
      ServerListItem {
        uuid: s.id,
        status: status(&s, wings.state(s.node_id, s.id)),
        node: named(&nodes, s.node_id),
        egg: named(&eggs, s.egg_id),
        owner: s.owner_id.map(|o| named(&users, o)),
        primary_allocation: allocs.next(),
        extra_allocations: allocs.collect(),
        name: s.name,
      }
    })
    .collect();
  Ok(Json(ListServersResponse { servers }))
}

pub(super) async fn server_options(
  _auth: JwtAuth<ServerEditPerm>,
  db: Connection,
) -> Result<Json<ServerOptions>> {
  let users = db
    .user()
    .list_users_simple()
    .await?
    .into_iter()
    .map(|u| Named {
      uuid: u.id,
      name: u.name,
    })
    .collect();
  let groups = db
    .group()
    .list_groups_simple()
    .await?
    .into_iter()
    .map(|g| Named {
      uuid: g.uuid,
      name: g.name,
    })
    .collect();
  let nodes = db
    .node()
    .list_nodes()
    .await?
    .into_iter()
    .map(|n| Named {
      uuid: n.id,
      name: n.name,
    })
    .collect();
  let mut eggs = Vec::new();
  // ponytail: one query per egg, fine for the handful of eggs a panel has
  for egg in db.egg().list_eggs().await? {
    let (egg, vars) = db.egg().find_by_id(egg.id).await?;
    eggs.push(OptionEgg {
      uuid: egg.id,
      name: egg.name,
      docker_images: unpairs(egg.docker_images)?,
      startup_commands: unpairs(egg.startup_commands)?,
      variables: vars
        .into_iter()
        .map(variable_from_model)
        .collect::<Result<_>>()?,
    });
  }
  Ok(Json(ServerOptions {
    users,
    groups,
    nodes,
    eggs,
  }))
}

pub(super) async fn server_info(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(req): Path<ServerUuid>,
) -> Result<Json<ServerInfo>> {
  let (server, access) = load(&db, auth.user_id, req.uuid, Access::Read).await?;
  let admin = access == Access::Admin;
  let (users, nodes, eggs) = names(&db).await?;
  let (_, egg_vars) = db.egg().find_by_id(server.egg_id).await?;
  let values = db.server().variables(server.id).await?;

  let variables = egg_vars
    .into_iter()
    .filter(|v| admin || v.user_viewable)
    .map(|v| {
      Ok(VariableInfo {
        value: values
          .get(&v.env_variable)
          .cloned()
          .unwrap_or_else(|| v.default_value.clone()),
        editable: admin || (access == Access::Write && v.user_editable),
        rules: serde_json::from_value(v.rules)?,
        name: v.name,
        description: v.description,
        env_variable: v.env_variable,
        default_value: v.default_value,
      })
    })
    .collect::<Result<_>>()?;

  let grants = if admin {
    let groups: HashMap<Uuid, String> = db
      .group()
      .list_groups_simple()
      .await?
      .into_iter()
      .map(|g| (g.uuid, g.name))
      .collect();
    let grants = db.server().grants(server.id).await?;
    let info = |names: &HashMap<Uuid, String>, uuid: Uuid, access| GrantInfo {
      uuid,
      name: names.get(&uuid).cloned().unwrap_or_default(),
      access,
    };
    Some(GrantsInfo {
      users: grants
        .users
        .into_iter()
        .map(|g| info(&users, g.user_id, g.access))
        .collect(),
      groups: grants
        .groups
        .into_iter()
        .map(|g| info(&groups, g.group_id, g.access))
        .collect(),
    })
  } else {
    None
  };

  let allocations = db
    .server()
    .allocations(vec![server.id])
    .await?
    .into_iter()
    .map(|a| Allocation {
      ip: a.ip,
      port: a.port,
    })
    .collect();

  Ok(Json(ServerInfo {
    uuid: server.id,
    status: status(&server, wings.state(server.node_id, server.id)),
    error: wings.error(server.node_id, server.id),
    access,
    owner: server.owner_id.map(|o| named(&users, o)),
    node: named(&nodes, server.node_id),
    egg: named(&eggs, server.egg_id),
    allocations,
    limits: Limits {
      cpu: server.cpu_limit,
      memory: server.memory_mb,
      disk: server.disk_mb,
    },
    variables,
    grants,
    name: server.name,
    startup: server.startup,
    startup_overridden: server.startup_overridden,
    image: server.image,
    install_state: server.install_state,
    suspended: server.suspended,
    start_on_completion: server.start_on_completion,
    backup_limit: server.backup_limit,
    stop_timeout: server.stop_timeout,
    restart_on_crash: server.restart_on_crash,
    created_at: server.created_at,
  }))
}

pub(super) async fn create_server(
  _auth: JwtAuth<ServerEditPerm>,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Json(req): Json<CreateServer>,
) -> FieldResult<Json<CreateServerResponse>> {
  let mut errors = Vec::new();
  let allocations = validate_spec(&req.spec, &mut errors);
  check_owner(&db, req.spec.owner, &mut errors).await?;
  if !node_exists(&db, req.node).await? {
    errors.push(field_err("node", "Node not found."));
  }
  let variables = egg_vars(&db, req.spec.egg, &mut errors)
    .await?
    .map(|vars| build_variables(&vars, &req.spec.environment, None, &mut errors))
    .unwrap_or_default();
  fail_if(StatusCode::BAD_REQUEST, errors)?;
  check_free(&db, req.node, &allocations, None, |i| {
    format!("allocations.{i}")
  })
  .await?;

  let id = Uuid::now_v7();
  let mut server = server::Model {
    id,
    name: String::new(),
    owner_id: None,
    node_id: req.node,
    egg_id: req.spec.egg,
    startup: String::new(),
    startup_overridden: false,
    image: String::new(),
    install_state: if req.skip_scripts {
      ServerInstallState::Installed
    } else {
      ServerInstallState::Installing
    },
    install_generation: if req.skip_scripts { 0 } else { 1 },
    suspended: false,
    start_on_completion: req.start_on_completion,
    cpu_limit: 0,
    memory_mb: 0,
    disk_mb: 0,
    backup_limit: 0,
    stop_timeout: 0,
    restart_on_crash: false,
    desired_running: req.skip_scripts && req.start_on_completion,
    power_generation: 0,
    created_at: Utc::now().fixed_offset(),
  };
  apply_spec(&mut server, req.spec);
  // ponytail: a concurrent create can still take a pair after check_free, the unique index then fails with 500
  db.server()
    .create(server.clone(), allocations, variables)
    .await?;
  // a pending install is part of the spec, its end sets the power; without
  // one the spec already carries it
  wings.put_server(server.node_id, server.id);
  info!("Created server with ID {}", id);

  updater.broadcast(UpdateMessage::Servers { uuid: id }).await;
  Ok(Json(CreateServerResponse { uuid: id }))
}

pub(super) async fn update_server(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<UpdateServer>,
) -> FieldResult<()> {
  let (mut server, access) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let current_allocs: Vec<(String, i32)> = db
    .server()
    .allocations(vec![server.id])
    .await?
    .into_iter()
    .map(|a| (a.ip, a.port))
    .collect();
  let current_vars = db.server().variables(server.id).await?;
  let (_, current_egg_vars) = db.egg().find_by_id(server.egg_id).await?;
  if access < Access::Admin
    && changes_admin_fields(
      &server,
      &current_allocs,
      &current_vars,
      &current_egg_vars,
      &req,
    )
  {
    return Err(centaurus::anyhow!(FORBIDDEN, "insufficient permissions").into());
  }

  let mut errors = Vec::new();
  let allocations = validate_spec(&req.spec, &mut errors);
  check_owner(&db, req.spec.owner, &mut errors).await?;
  let egg_changed = req.spec.egg != server.egg_id;
  let variables = egg_vars(&db, req.spec.egg, &mut errors)
    .await?
    .map(|vars| {
      build_variables(
        &vars,
        &req.spec.environment,
        (!egg_changed).then_some(&current_vars),
        &mut errors,
      )
    })
    .unwrap_or_default();
  let grants = match &req.grants {
    Some(grants) => Some(validate_grants(&db, server.id, grants, &mut errors).await?),
    None => None,
  };
  fail_if(StatusCode::BAD_REQUEST, errors)?;
  check_free(&db, server.node_id, &allocations, Some(server.id), |i| {
    format!("allocations.{i}")
  })
  .await?;

  apply_spec(&mut server, req.spec);
  db.server()
    .update(server.clone(), allocations, variables, grants)
    .await?;
  wings.put_server(server.node_id, server.id);
  info!("Updated server with ID {}", path.uuid);

  updater
    .broadcast(UpdateMessage::Servers { uuid: path.uuid })
    .await;
  Ok(())
}

pub(super) async fn delete_server(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Path(req): Path<ServerUuid>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, req.uuid, Access::Admin).await?;
  ensure_allowed(&server, Action::Delete)?;
  db.server().delete(server.id).await?;
  wings.delete_server(server.node_id, server.id, false);
  info!("Deleted server with ID {}", req.uuid);

  updater
    .broadcast(UpdateMessage::Servers { uuid: req.uuid })
    .await;
  Ok(())
}
