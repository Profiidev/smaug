use std::{
  collections::{HashMap, HashSet},
  net::IpAddr,
};

use aide::{
  OperationOutput,
  axum::{
    ApiRouter,
    routing::{get_with, post_with},
  },
  generate::GenContext,
  openapi::{Operation, Response},
};
use axum::{Json, extract::Path, response::IntoResponse};
use centaurus::{
  backend::auth::permission::Permission,
  bail,
  db::{init::Connection, tables::ConnectionExt},
  error::{ErrorReport, Result},
};
use chrono::Utc;
use entity::{
  egg_variable,
  sea_orm_active_enums::{ServerAccessLevel, ServerInstallState},
  server, server_group_access, server_user_access, user,
};
use http::StatusCode;
use indexmap::IndexMap;
use schemars::JsonSchema;
use sea_orm::{DbErr, EntityTrait};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::{
  auth::jwt_auth::JwtAuth,
  db::{DBTrait, server::Grants},
  eggs::{EggVariable, unpairs, variable_from_model},
  utils::{ServerEditPerm, ServerViewPerm, UpdateMessage, Updater},
};

mod rules;

pub fn router() -> ApiRouter {
  ApiRouter::new()
    .api_route("/", get_with(list_servers, |op| op.id("list_servers")))
    .api_route("/", post_with(create_server, |op| op.id("create_server")))
    .api_route(
      "/options",
      get_with(server_options, |op| op.id("server_options")),
    )
    .api_route(
      "/{uuid}",
      get_with(server_info, |op| op.id("server_info"))
        .put_with(update_server, |op| op.id("update_server"))
        .delete_with(delete_server, |op| op.id("delete_server")),
    )
    .api_route(
      "/{uuid}/reinstall",
      post_with(reinstall_server, |op| op.id("reinstall_server")),
    )
    .api_route(
      "/{uuid}/install",
      post_with(set_server_installed, |op| op.id("set_server_installed")),
    )
    .api_route(
      "/{uuid}/suspend",
      post_with(set_server_suspended, |op| op.id("set_server_suspended")),
    )
    .api_route(
      "/{uuid}/transfer",
      post_with(transfer_server, |op| op.id("transfer_server")),
    )
}

// ---- errors ----

#[derive(Serialize, JsonSchema, Debug, PartialEq, Eq)]
pub struct FieldError {
  /// Dotted path of the offending request field, e.g. `allocations.1.port`
  /// or `variables.SERVER_JARFILE`.
  pub field: String,
  pub message: String,
}

/// Body of a 400 (validation) or 409 (allocation in use) response.
#[derive(Serialize, JsonSchema)]
pub struct FieldErrors {
  pub errors: Vec<FieldError>,
}

fn field_err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
  FieldError {
    field: field.into(),
    message: message.into(),
  }
}

/// A plain centaurus error (status only) or field errors with a JSON body.
#[derive(Debug)]
pub enum ServerError {
  Report(ErrorReport),
  Fields(StatusCode, Vec<FieldError>),
}

impl From<ErrorReport> for ServerError {
  fn from(e: ErrorReport) -> Self {
    Self::Report(e)
  }
}

impl From<DbErr> for ServerError {
  fn from(e: DbErr) -> Self {
    Self::Report(e.into())
  }
}

impl IntoResponse for ServerError {
  fn into_response(self) -> axum::response::Response {
    match self {
      Self::Report(e) => e.into_response(),
      Self::Fields(status, errors) => (status, Json(FieldErrors { errors })).into_response(),
    }
  }
}

impl OperationOutput for ServerError {
  type Inner = FieldErrors;

  fn inferred_responses(
    ctx: &mut GenContext,
    operation: &mut Operation,
  ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
    let mut res = Vec::new();
    if let Some(body) = Json::<FieldErrors>::operation_response(ctx, operation) {
      for code in [400, 409] {
        res.push((Some(aide::openapi::StatusCode::Code(code)), body.clone()));
      }
    }
    res.extend(ErrorReport::inferred_responses(ctx, operation));
    res
  }
}

type FieldResult<T> = std::result::Result<T, ServerError>;

fn fail_if(status: StatusCode, errors: Vec<FieldError>) -> FieldResult<()> {
  if errors.is_empty() {
    Ok(())
  } else {
    Err(ServerError::Fields(status, errors))
  }
}

// ---- access ----

/// What the caller may do with a server, ordered from least to most.
#[derive(Serialize, JsonSchema, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Access {
  None,
  Read,
  Write,
  Admin,
}

/// `server:edit` is admin; owner or any write grant is write; `server:view`
/// or any (read) grant is read.
fn resolve_access(admin: bool, viewer: bool, owner: bool, grants: &[ServerAccessLevel]) -> Access {
  if admin {
    Access::Admin
  } else if owner || grants.contains(&ServerAccessLevel::Write) {
    Access::Write
  } else if viewer || !grants.is_empty() {
    Access::Read
  } else {
    Access::None
  }
}

async fn has_perm<P: Permission>(db: &Connection, user: Uuid) -> Result<bool> {
  db.group().user_hash_permissions(user, P::name()).await
}

async fn access(db: &Connection, user: Uuid, server: &server::Model) -> Result<Access> {
  if has_perm::<ServerEditPerm>(db, user).await? {
    return Ok(Access::Admin);
  }
  let viewer = has_perm::<ServerViewPerm>(db, user).await?;
  let grants: Vec<_> = db
    .server()
    .grants_for_user(user, Some(server.id))
    .await?
    .into_iter()
    .map(|(_, level)| level)
    .collect();
  Ok(resolve_access(
    false,
    viewer,
    server.owner_id == Some(user),
    &grants,
  ))
}

/// 404 without any access (no existence leak), 403 below `need`.
async fn load(
  db: &Connection,
  user: Uuid,
  uuid: Uuid,
  need: Access,
) -> Result<(server::Model, Access)> {
  let server = db.server().find_by_id(uuid).await?;
  let access = access(db, user, &server).await?;
  if access == Access::None {
    bail!(NOT_FOUND, "Server not found");
  }
  if access < need {
    bail!(FORBIDDEN, "insufficient permissions");
  }
  Ok((server, access))
}

// ---- status and actions ----

#[derive(Serialize, JsonSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
  Offline,
  NotInstalled,
  Installing,
  InstallFailed,
  Suspended,
}

fn status(server: &server::Model) -> ServerStatus {
  if server.suspended {
    return ServerStatus::Suspended;
  }
  match server.install_state {
    ServerInstallState::NotInstalled => ServerStatus::NotInstalled,
    ServerInstallState::Installing => ServerStatus::Installing,
    ServerInstallState::InstallFailed => ServerStatus::InstallFailed,
    // TODO(wings): runtime state (running/starting/stopping/offline, transferring) reported by wings
    ServerInstallState::Installed => ServerStatus::Offline,
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Action {
  Reinstall,
  Install,
  Suspend,
  Unsuspend,
  Transfer,
  Delete,
}

/// Availability matrix from the design handoff.
fn allowed(status: ServerStatus, action: Action) -> bool {
  use Action as A;
  match status {
    ServerStatus::Offline | ServerStatus::InstallFailed | ServerStatus::NotInstalled => {
      action != A::Unsuspend
    }
    ServerStatus::Installing => matches!(action, A::Install | A::Suspend),
    ServerStatus::Suspended => matches!(action, A::Install | A::Unsuspend | A::Delete),
  }
}

fn ensure_allowed(server: &server::Model, action: Action) -> Result<()> {
  if !allowed(status(server), action) {
    bail!(CONFLICT, "Action is not available in the current state");
  }
  Ok(())
}

// ---- request and response types ----

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq, Eq)]
pub struct Allocation {
  pub ip: String,
  pub port: i32,
}

/// CPU in millicores (1000 = one core), memory and disk in MiB; 0 is unlimited.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
  pub cpu: i32,
  pub memory: i64,
  pub disk: i64,
}

#[derive(Serialize, JsonSchema, Debug)]
struct Named {
  uuid: Uuid,
  name: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
struct Grant {
  /// User or group id.
  uuid: Uuid,
  access: ServerAccessLevel,
}

#[derive(Deserialize, JsonSchema, Debug, Default)]
struct GrantsBody {
  #[serde(default)]
  users: Vec<Grant>,
  #[serde(default)]
  groups: Vec<Grant>,
}

#[derive(Serialize, JsonSchema, Debug)]
struct GrantInfo {
  uuid: Uuid,
  name: String,
  access: ServerAccessLevel,
}

#[derive(Serialize, JsonSchema, Debug)]
struct GrantsInfo {
  users: Vec<GrantInfo>,
  groups: Vec<GrantInfo>,
}

// Fields shared by create and update.
#[derive(Deserialize, JsonSchema, Debug, Clone)]
struct ServerSpec {
  name: String,
  owner: Option<Uuid>,
  egg: Uuid,
  /// First one is the primary allocation.
  allocations: Vec<Allocation>,
  startup: String,
  /// Overridden commands are left alone by egg updates.
  startup_overridden: bool,
  image: String,
  /// env name -> value; missing variables keep their current value (or the
  /// egg default).
  #[serde(default)]
  environment: HashMap<String, String>,
  limits: Limits,
  /// 0 disables backups.
  backup_limit: i32,
}

#[derive(Deserialize, JsonSchema, Debug)]
struct CreateServer {
  #[serde(flatten)]
  spec: ServerSpec,
  node: Uuid,
  /// Mark the server installed without running the egg install script.
  skip_scripts: bool,
  start_on_completion: bool,
}

#[derive(Deserialize, JsonSchema, Debug)]
struct UpdateServer {
  #[serde(flatten)]
  spec: ServerSpec,
  /// Replaces all grants when set (admin only).
  #[serde(default)]
  grants: Option<GrantsBody>,
}

#[derive(Serialize, JsonSchema)]
struct CreateServerResponse {
  uuid: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct ServerUuid {
  uuid: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct InstallRequest {
  installed: bool,
}

#[derive(Deserialize, JsonSchema)]
struct SuspendRequest {
  suspended: bool,
}

#[derive(Deserialize, JsonSchema)]
struct TransferRequest {
  node: Uuid,
  ip: String,
  port: i32,
}

#[derive(Serialize, JsonSchema)]
struct ServerListItem {
  uuid: Uuid,
  name: String,
  status: ServerStatus,
  node: Named,
  egg: Named,
  owner: Option<Named>,
  primary_allocation: Option<Allocation>,
  extra_allocations: Vec<Allocation>,
}

#[derive(Serialize, JsonSchema)]
struct ListServersResponse {
  servers: Vec<ServerListItem>,
}

#[derive(Serialize, JsonSchema, Debug)]
struct VariableInfo {
  name: String,
  description: String,
  env_variable: String,
  default_value: String,
  value: String,
  rules: Vec<String>,
  /// Whether the caller may change it.
  editable: bool,
}

#[derive(Serialize, JsonSchema)]
struct ServerInfo {
  uuid: Uuid,
  name: String,
  status: ServerStatus,
  /// The caller's access level.
  access: Access,
  owner: Option<Named>,
  node: Named,
  egg: Named,
  allocations: Vec<Allocation>,
  startup: String,
  startup_overridden: bool,
  image: String,
  install_state: ServerInstallState,
  suspended: bool,
  start_on_completion: bool,
  limits: Limits,
  backup_limit: i32,
  created_at: chrono::DateTime<chrono::FixedOffset>,
  /// Egg variables merged with the server's values; non-admins only see
  /// `user_viewable` ones.
  variables: Vec<VariableInfo>,
  /// Only returned to admins.
  grants: Option<GrantsInfo>,
}

#[derive(Serialize, JsonSchema)]
struct OptionEgg {
  uuid: Uuid,
  name: String,
  /// label -> image, order preserved
  docker_images: IndexMap<String, String>,
  /// label -> command, order preserved; the first one is the default
  startup_commands: IndexMap<String, String>,
  variables: Vec<EggVariable>,
}

#[derive(Serialize, JsonSchema)]
struct ServerOptions {
  users: Vec<Named>,
  groups: Vec<Named>,
  nodes: Vec<Named>,
  eggs: Vec<OptionEgg>,
}

// ---- validation ----

fn parse_allocation(
  ip: &str,
  port: i32,
  prefix: &str,
  errors: &mut Vec<FieldError>,
) -> Option<(String, i32)> {
  let ip = ip.trim().parse::<IpAddr>();
  if ip.is_err() {
    errors.push(field_err(format!("{prefix}ip"), "Invalid IP address."));
  }
  if !(1024..=65535).contains(&port) {
    errors.push(field_err(
      format!("{prefix}port"),
      "Port must be between 1024 and 65535.",
    ));
  }
  ip.ok()
    .filter(|_| (1024..=65535).contains(&port))
    .map(|ip| (ip.to_string(), port))
}

/// Checks the request-only rules and returns the normalized allocations.
fn validate_spec(spec: &ServerSpec, errors: &mut Vec<FieldError>) -> Vec<(String, i32)> {
  let name = spec.name.trim();
  if name.is_empty() {
    errors.push(field_err("name", "Name is required."));
  } else if name.chars().count() > 191 {
    errors.push(field_err("name", "Name must be at most 191 characters."));
  }
  if spec.startup.trim().is_empty() {
    errors.push(field_err("startup", "Startup command is required."));
  }
  if spec.image.trim().is_empty() {
    errors.push(field_err("image", "Image is required."));
  }
  if spec.limits.cpu < 0 {
    errors.push(field_err(
      "limits.cpu",
      "CPU limit must be 0 or more millicores.",
    ));
  }
  if spec.limits.memory < 0 {
    errors.push(field_err("limits.memory", "Memory must be 0 or more MiB."));
  }
  if spec.limits.disk < 0 {
    errors.push(field_err("limits.disk", "Storage must be 0 or more MiB."));
  }
  if spec.backup_limit < 0 {
    errors.push(field_err("backup_limit", "Backup limit must be 0 or more."));
  }

  if spec.allocations.is_empty() {
    errors.push(field_err(
      "allocations",
      "At least one allocation is required.",
    ));
  }
  let mut seen = HashSet::new();
  let mut out = Vec::new();
  for (i, a) in spec.allocations.iter().enumerate() {
    if let Some(pair) = parse_allocation(&a.ip, a.port, &format!("allocations.{i}."), errors) {
      if !seen.insert(pair.clone()) {
        errors.push(field_err(
          format!("allocations.{i}"),
          "Duplicate allocation.",
        ));
      }
      out.push(pair);
    }
  }
  out
}

/// Final value of every egg variable. Supplied values win, then the current
/// value, then the egg default. Values that changed (or are new) are checked
/// against the egg rules.
fn build_variables(
  egg_vars: &[egg_variable::Model],
  env: &HashMap<String, String>,
  current: Option<&HashMap<String, String>>,
  errors: &mut Vec<FieldError>,
) -> Vec<(String, String)> {
  for key in env.keys() {
    if !egg_vars.iter().any(|v| &v.env_variable == key) {
      errors.push(field_err(format!("variables.{key}"), "Unknown variable."));
    }
  }
  egg_vars
    .iter()
    .map(|v| {
      let old = current.and_then(|c| c.get(&v.env_variable));
      let value = env
        .get(&v.env_variable)
        .or(old)
        .unwrap_or(&v.default_value)
        .clone();
      if old != Some(&value) {
        let rules: Vec<String> = serde_json::from_value(v.rules.clone()).unwrap_or_default();
        if let Some(msg) = rules::check(&v.name, &value, &rules) {
          errors.push(field_err(format!("variables.{}", v.env_variable), msg));
        }
      }
      (v.env_variable.clone(), value)
    })
    .collect()
}

/// Whether a non-admin tries to change anything beyond name, startup and
/// the variables it may edit.
fn changes_admin_fields(
  server: &server::Model,
  allocations: &[(String, i32)],
  vars: &HashMap<String, String>,
  egg_vars: &[egg_variable::Model],
  req: &UpdateServer,
) -> bool {
  let spec = &req.spec;
  let normalized: Vec<_> = spec
    .allocations
    .iter()
    .map(|a| {
      (
        a.ip
          .trim()
          .parse::<IpAddr>()
          .map(|ip| ip.to_string())
          .unwrap_or_else(|_| a.ip.clone()),
        a.port,
      )
    })
    .collect();
  // hidden and unknown keys count regardless of value, so the 403 can not be
  // used to probe hidden values or variable names
  let locked_var_changed = spec.environment.iter().any(|(key, value)| {
    egg_vars
      .iter()
      .find(|v| &v.env_variable == key)
      .is_none_or(|v| !v.user_viewable || (!v.user_editable && vars.get(key) != Some(value)))
  });
  req.grants.is_some()
    || spec.owner != server.owner_id
    || spec.egg != server.egg_id
    || spec.image != server.image
    || spec.limits
      != (Limits {
        cpu: server.cpu_limit,
        memory: server.memory_mb,
        disk: server.disk_mb,
      })
    || spec.backup_limit != server.backup_limit
    || normalized != allocations
    || locked_var_changed
}

async fn check_owner(
  db: &Connection,
  owner: Option<Uuid>,
  errors: &mut Vec<FieldError>,
) -> Result<()> {
  if let Some(owner) = owner
    && user::Entity::find_by_id(owner).one(&**db).await?.is_none()
  {
    errors.push(field_err("owner", "User not found."));
  }
  Ok(())
}

async fn egg_vars(
  db: &Connection,
  egg: Uuid,
  errors: &mut Vec<FieldError>,
) -> Result<Option<Vec<egg_variable::Model>>> {
  match db.egg().find_by_id(egg).await {
    Ok((_, vars)) => Ok(Some(vars)),
    Err(e) if e.status == StatusCode::NOT_FOUND => {
      errors.push(field_err("egg", "Egg not found."));
      Ok(None)
    }
    Err(e) => Err(e),
  }
}

async fn node_exists(db: &Connection, node: Uuid) -> Result<bool> {
  match db.node().find_by_id(node).await {
    Ok(_) => Ok(true),
    Err(e) if e.status == StatusCode::NOT_FOUND => Ok(false),
    Err(e) => Err(e),
  }
}

/// 409 with a per-row error for allocations already used on `node`.
async fn check_free(
  db: &Connection,
  node: Uuid,
  allocations: &[(String, i32)],
  exclude: Option<Uuid>,
  field: impl Fn(usize) -> String,
) -> FieldResult<()> {
  let taken = db.server().taken(node, allocations, exclude).await?;
  let errors = allocations
    .iter()
    .enumerate()
    .filter(|(_, a)| taken.contains(a))
    .map(|(i, _)| field_err(field(i), "IP and port are already in use on this node."))
    .collect();
  fail_if(StatusCode::CONFLICT, errors)
}

async fn validate_grants(
  db: &Connection,
  server: Uuid,
  grants: &GrantsBody,
  errors: &mut Vec<FieldError>,
) -> Result<Grants> {
  let users: HashSet<Uuid> = db
    .user()
    .list_users_simple()
    .await?
    .into_iter()
    .map(|u| u.id)
    .collect();
  let groups: HashSet<Uuid> = db
    .group()
    .list_groups_simple()
    .await?
    .into_iter()
    .map(|g| g.uuid)
    .collect();

  let mut check = |list: &[Grant], known: &HashSet<Uuid>, kind: &str, missing: &str| {
    let mut seen = HashSet::new();
    for (i, g) in list.iter().enumerate() {
      let field = format!("grants.{kind}.{i}");
      if !known.contains(&g.uuid) {
        errors.push(field_err(field, missing));
      } else if !seen.insert(g.uuid) {
        errors.push(field_err(field, "Duplicate grant."));
      }
    }
  };
  check(&grants.users, &users, "users", "User not found.");
  check(&grants.groups, &groups, "groups", "Group not found.");

  Ok(Grants {
    users: grants
      .users
      .iter()
      .map(|g| server_user_access::Model {
        server_id: server,
        user_id: g.uuid,
        access: g.access,
      })
      .collect(),
    groups: grants
      .groups
      .iter()
      .map(|g| server_group_access::Model {
        server_id: server,
        group_id: g.uuid,
        access: g.access,
      })
      .collect(),
  })
}

fn apply_spec(server: &mut server::Model, spec: ServerSpec) {
  server.name = spec.name.trim().to_string();
  server.owner_id = spec.owner;
  server.egg_id = spec.egg;
  server.startup = spec.startup;
  server.startup_overridden = spec.startup_overridden;
  server.image = spec.image.trim().to_string();
  server.cpu_limit = spec.limits.cpu;
  server.memory_mb = spec.limits.memory;
  server.disk_mb = spec.limits.disk;
  server.backup_limit = spec.backup_limit;
}

// ---- handlers ----

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

async fn list_servers(auth: JwtAuth, db: Connection) -> Result<Json<ListServersResponse>> {
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
        status: status(&s),
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

async fn server_options(
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

async fn server_info(
  auth: JwtAuth,
  db: Connection,
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
    status: status(&server),
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
    created_at: server.created_at,
  }))
}

async fn create_server(
  _auth: JwtAuth<ServerEditPerm>,
  db: Connection,
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
    suspended: false,
    start_on_completion: req.start_on_completion,
    cpu_limit: 0,
    memory_mb: 0,
    disk_mb: 0,
    backup_limit: 0,
    created_at: Utc::now().fixed_offset(),
  };
  apply_spec(&mut server, req.spec);
  // ponytail: a concurrent create can still take a pair after check_free, the unique index then fails with 500
  db.server().create(server, allocations, variables).await?;
  // TODO(wings): send the server spec to its node and run the install script unless skip_scripts
  info!("Created server with ID {}", id);

  updater.broadcast(UpdateMessage::Servers { uuid: id }).await;
  Ok(Json(CreateServerResponse { uuid: id }))
}

async fn update_server(
  auth: JwtAuth,
  db: Connection,
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
    .update(server, allocations, variables, grants)
    .await?;
  // TODO(wings): push the updated spec (startup, image, limits, allocations, variables) to the node
  info!("Updated server with ID {}", path.uuid);

  updater
    .broadcast(UpdateMessage::Servers { uuid: path.uuid })
    .await;
  Ok(())
}

async fn delete_server(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(req): Path<ServerUuid>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, req.uuid, Access::Admin).await?;
  ensure_allowed(&server, Action::Delete)?;
  db.server().delete(server.id).await?;
  // TODO(wings): stop the server and remove its files on the node
  info!("Deleted server with ID {}", req.uuid);

  updater
    .broadcast(UpdateMessage::Servers { uuid: req.uuid })
    .await;
  Ok(())
}

async fn reinstall_server(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(req): Path<ServerUuid>,
) -> Result<()> {
  let (mut server, _) = load(&db, auth.user_id, req.uuid, Access::Write).await?;
  ensure_allowed(&server, Action::Reinstall)?;
  server.install_state = ServerInstallState::Installing;
  db.server().save(server).await?;
  // TODO(wings): stop the server and run the egg install script on the node
  info!("Reinstalling server with ID {}", req.uuid);

  updater
    .broadcast(UpdateMessage::Servers { uuid: req.uuid })
    .await;
  Ok(())
}

async fn set_server_installed(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<InstallRequest>,
) -> Result<()> {
  let (mut server, _) = load(&db, auth.user_id, path.uuid, Access::Admin).await?;
  ensure_allowed(&server, Action::Install)?;
  // only flips: installed -> not installed, anything else -> installed
  if req.installed == (server.install_state == ServerInstallState::Installed) {
    bail!(CONFLICT, "Server already has this install state");
  }
  server.install_state = if req.installed {
    ServerInstallState::Installed
  } else {
    ServerInstallState::NotInstalled
  };
  db.server().save(server).await?;
  // TODO(wings): tell the node the install state changed (blocks or unblocks starts)

  updater
    .broadcast(UpdateMessage::Servers { uuid: path.uuid })
    .await;
  Ok(())
}

async fn set_server_suspended(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<SuspendRequest>,
) -> Result<()> {
  let (mut server, _) = load(&db, auth.user_id, path.uuid, Access::Admin).await?;
  ensure_allowed(
    &server,
    if req.suspended {
      Action::Suspend
    } else {
      Action::Unsuspend
    },
  )?;
  server.suspended = req.suspended;
  db.server().save(server).await?;
  // TODO(wings): stop the server on suspend, lift the block on unsuspend

  updater
    .broadcast(UpdateMessage::Servers { uuid: path.uuid })
    .await;
  Ok(())
}

async fn transfer_server(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<TransferRequest>,
) -> FieldResult<()> {
  let (mut server, _) = load(&db, auth.user_id, path.uuid, Access::Admin).await?;
  ensure_allowed(&server, Action::Transfer)?;

  let mut errors = Vec::new();
  if req.node == server.node_id {
    errors.push(field_err("node", "Server is already on this node."));
  } else if !node_exists(&db, req.node).await? {
    errors.push(field_err("node", "Node not found."));
  }
  let allocation = parse_allocation(&req.ip, req.port, "", &mut errors);
  fail_if(StatusCode::BAD_REQUEST, errors)?;
  let allocation = allocation.expect("valid when there are no errors");
  check_free(
    &db,
    req.node,
    std::slice::from_ref(&allocation),
    None,
    |_| "port".into(),
  )
  .await?;

  server.node_id = req.node;
  db.server().transfer(server, allocation).await?;
  // TODO(wings): transfer the server's files to the new node, then remove them from the old one
  info!("Transferred server {} to node {}", path.uuid, req.node);

  updater
    .broadcast(UpdateMessage::Servers { uuid: path.uuid })
    .await;
  Ok(())
}

#[cfg(test)]
mod test {
  use serde_json::json;

  use super::*;

  fn model() -> server::Model {
    server::Model {
      id: Uuid::now_v7(),
      name: "s".into(),
      owner_id: Some(Uuid::now_v7()),
      node_id: Uuid::now_v7(),
      egg_id: Uuid::now_v7(),
      startup: "run".into(),
      startup_overridden: false,
      image: "img".into(),
      install_state: ServerInstallState::Installed,
      suspended: false,
      start_on_completion: true,
      cpu_limit: 1000,
      memory_mb: 512,
      disk_mb: 1024,
      backup_limit: 2,
      created_at: Utc::now().fixed_offset(),
    }
  }

  fn spec() -> ServerSpec {
    serde_json::from_value(json!({
      "name": "survival",
      "owner": null,
      "egg": Uuid::now_v7(),
      "allocations": [{ "ip": "10.0.0.5", "port": 25565 }],
      "startup": "run",
      "startup_overridden": false,
      "image": "img",
      "limits": { "cpu": 0, "memory": 0, "disk": 0 },
      "backup_limit": 0
    }))
    .unwrap()
  }

  fn spec_errors(f: impl FnOnce(&mut ServerSpec)) -> Vec<String> {
    let mut s = spec();
    f(&mut s);
    let mut errors = Vec::new();
    validate_spec(&s, &mut errors);
    errors.into_iter().map(|e| e.field).collect()
  }

  fn var(env: &str, rules: &[&str], viewable: bool, editable: bool) -> egg_variable::Model {
    egg_variable::Model {
      id: Uuid::now_v7(),
      egg_id: Uuid::nil(),
      name: env.to_lowercase(),
      description: String::new(),
      env_variable: env.into(),
      default_value: "def".into(),
      user_viewable: viewable,
      user_editable: editable,
      sort: None,
      rules: json!(rules),
    }
  }

  fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect()
  }

  // ---- access ----

  #[test]
  fn access_resolution() {
    use Access::*;
    use ServerAccessLevel::{Read as R, Write as W};
    let cases: &[(bool, bool, bool, &[ServerAccessLevel], Access)] = &[
      (false, false, false, &[], None),
      (true, false, false, &[], Admin),
      (true, true, true, &[R, W], Admin),
      (false, true, false, &[], Read),
      (false, false, false, &[R], Read),
      (false, false, false, &[R, R], Read),
      (false, false, true, &[], Write),
      (false, true, true, &[R], Write),
      (false, false, false, &[W], Write),
      (false, false, false, &[R, W], Write),
      (false, true, false, &[W], Write),
    ];
    for (admin, viewer, owner, grants, expected) in cases {
      assert_eq!(
        resolve_access(*admin, *viewer, *owner, grants),
        *expected,
        "{admin} {viewer} {owner} {grants:?}"
      );
    }
    assert!(None < Read && Read < Write && Write < Admin);
  }

  // ---- status and actions ----

  #[test]
  fn status_derivation() {
    let mut s = model();
    for (state, expected) in [
      (ServerInstallState::Installed, ServerStatus::Offline),
      (ServerInstallState::NotInstalled, ServerStatus::NotInstalled),
      (ServerInstallState::Installing, ServerStatus::Installing),
      (
        ServerInstallState::InstallFailed,
        ServerStatus::InstallFailed,
      ),
    ] {
      s.install_state = state;
      s.suspended = false;
      assert_eq!(status(&s), expected);
      s.suspended = true;
      assert_eq!(status(&s), ServerStatus::Suspended);
    }
  }

  #[test]
  fn action_availability_matrix() {
    use Action::*;
    let actions = [Reinstall, Install, Suspend, Unsuspend, Transfer, Delete];
    let rows = [
      (ServerStatus::Offline, [true, true, true, false, true, true]),
      (
        ServerStatus::InstallFailed,
        [true, true, true, false, true, true],
      ),
      (
        ServerStatus::NotInstalled,
        [true, true, true, false, true, true],
      ),
      (
        ServerStatus::Installing,
        [false, true, true, false, false, false],
      ),
      (
        ServerStatus::Suspended,
        [false, true, false, true, false, true],
      ),
    ];
    for (status, expected) in rows {
      for (action, expected) in actions.into_iter().zip(expected) {
        assert_eq!(allowed(status, action), expected, "{status:?} {action:?}");
      }
    }
    let mut s = model();
    s.install_state = ServerInstallState::Installing;
    assert_eq!(
      ensure_allowed(&s, Delete).unwrap_err().status,
      StatusCode::CONFLICT
    );
    assert!(ensure_allowed(&s, Install).is_ok());
  }

  // ---- validation ----

  #[test]
  fn spec_validation() {
    assert!(spec_errors(|_| {}).is_empty());
    assert_eq!(spec_errors(|s| s.name = "".into()), ["name"]);
    assert_eq!(spec_errors(|s| s.name = " \t".into()), ["name"]);
    assert_eq!(spec_errors(|s| s.name = "x".repeat(192)), ["name"]);
    assert!(spec_errors(|s| s.name = "ä".repeat(191)).is_empty());
    assert_eq!(spec_errors(|s| s.startup = " ".into()), ["startup"]);
    assert_eq!(spec_errors(|s| s.image = "".into()), ["image"]);
    assert_eq!(spec_errors(|s| s.limits.cpu = -1), ["limits.cpu"]);
    assert_eq!(spec_errors(|s| s.limits.memory = -1), ["limits.memory"]);
    assert_eq!(spec_errors(|s| s.limits.disk = -1), ["limits.disk"]);
    assert_eq!(spec_errors(|s| s.backup_limit = -1), ["backup_limit"]);
    assert_eq!(spec_errors(|s| s.allocations.clear()), ["allocations"]);
    // everything at once
    assert_eq!(
      spec_errors(|s| {
        s.name = String::new();
        s.image = String::new();
        s.backup_limit = -5;
      }),
      ["name", "image", "backup_limit"]
    );
  }

  #[test]
  fn allocation_validation_and_normalization() {
    let alloc = |ip: &str, port| Allocation {
      ip: ip.into(),
      port,
    };
    let run = |allocs: Vec<Allocation>| {
      let mut s = spec();
      s.allocations = allocs;
      let mut errors = Vec::new();
      let out = validate_spec(&s, &mut errors);
      (out, errors)
    };
    let (out, errors) = run(vec![
      alloc(" 10.0.0.1 ", 1024),
      alloc("0:0:0:0:0:0:0:1", 65535),
    ]);
    assert!(errors.is_empty());
    assert_eq!(out, [("10.0.0.1".into(), 1024), ("::1".into(), 65535)]);

    let (out, errors) = run(vec![
      alloc("10.0.0.1", 2000),
      alloc("nope", 2000),
      alloc("10.0.0.1", 1023),
      alloc("10.0.0.1", 65536),
      alloc("10.0.0.01", 2000),
      alloc("10.0.0.1", 2000),
      alloc("::1", 2000),
      alloc("::0:1", 2000),
    ]);
    let fields: Vec<_> = errors.iter().map(|e| e.field.as_str()).collect();
    assert_eq!(
      fields,
      [
        "allocations.1.ip",
        "allocations.2.port",
        "allocations.3.port",
        "allocations.4.ip",
        "allocations.5",
        "allocations.7"
      ]
    );
    assert_eq!(errors[4].message, "Duplicate allocation.");
    // invalid rows are not returned
    assert_eq!(out.len(), 4);

    let mut errors = Vec::new();
    assert_eq!(parse_allocation("x", 1, "", &mut errors), Option::None);
    let fields: Vec<_> = errors.iter().map(|e| e.field.as_str()).collect();
    assert_eq!(fields, ["ip", "port"]);
  }

  #[test]
  fn variables_merge_and_validate() {
    let vars = vec![
      var("A", &["required", "integer"], true, true),
      var("B", &["in:def,x"], true, true),
      var("C", &[], false, false),
    ];
    let build = |env: &[(&str, &str)], current: Option<&HashMap<String, String>>| {
      let mut errors = Vec::new();
      let out = build_variables(&vars, &map(env), current, &mut errors);
      (out, errors)
    };

    // create: supplied value, then default; defaults are validated too
    let (out, errors) = build(&[("A", "5")], Option::None);
    assert_eq!(
      out,
      [
        ("A".into(), "5".into()),
        ("B".into(), "def".into()),
        ("C".into(), "def".into())
      ]
    );
    assert!(errors.is_empty());
    let (_, errors) = build(&[], Option::None);
    assert_eq!(
      errors,
      [field_err("variables.A", "The a field must be an integer.")]
    );

    // update: missing keeps current, unchanged invalid values pass
    let current = map(&[("A", "not a number"), ("B", "x")]);
    let (out, errors) = build(&[("B", "x")], Some(&current));
    assert!(errors.is_empty());
    assert_eq!(out[0], ("A".into(), "not a number".into()));
    assert_eq!(out[2], ("C".into(), "def".into()));
    let (_, errors) = build(&[("B", "y")], Some(&current));
    assert_eq!(
      errors,
      [field_err("variables.B", "The selected b is invalid.")]
    );

    // unknown keys
    let (_, errors) = build(&[("A", "1"), ("NOPE", "x")], Option::None);
    assert_eq!(errors, [field_err("variables.NOPE", "Unknown variable.")]);

    // unparsable stored rules are treated as none
    let mut broken = var("D", &[], true, true);
    broken.rules = json!("required");
    let mut errors = Vec::new();
    build_variables(&[broken], &map(&[("D", "")]), Option::None, &mut errors);
    assert!(errors.is_empty());
  }

  #[test]
  fn admin_field_detection() {
    let server = model();
    let allocs = vec![("10.0.0.5".to_string(), 25565)];
    let vars = vec![
      var("EDIT", &[], true, true),
      var("LOCKED", &[], true, false),
      var("HIDDEN_EDIT", &[], false, true),
    ];
    let current = map(&[("EDIT", "e"), ("LOCKED", "l"), ("HIDDEN_EDIT", "h")]);
    let base = || {
      let mut s = spec();
      s.owner = server.owner_id;
      s.egg = server.egg_id;
      s.image = server.image.clone();
      s.limits = Limits {
        cpu: server.cpu_limit,
        memory: server.memory_mb,
        disk: server.disk_mb,
      };
      s.backup_limit = server.backup_limit;
      UpdateServer {
        spec: s,
        grants: Option::None,
      }
    };
    let changes = |f: &dyn Fn(&mut UpdateServer)| {
      let mut req = base();
      f(&mut req);
      changes_admin_fields(&server, &allocs, &current, &vars, &req)
    };

    // write fields only
    assert!(!changes(&|_| {}));
    assert!(!changes(&|r| {
      r.spec.name = "new".into();
      r.spec.startup = "other".into();
      r.spec.startup_overridden = true;
      r.spec.environment = map(&[("EDIT", "changed"), ("LOCKED", "l")]);
      // same pair written differently
      r.spec.allocations[0].ip = " 10.0.0.5".into();
    }));

    for f in [
      &(|r: &mut UpdateServer| r.spec.owner = Option::None) as &dyn Fn(&mut UpdateServer),
      &|r| r.spec.egg = Uuid::now_v7(),
      &|r| r.spec.image = "x".into(),
      &|r| r.spec.limits.cpu += 1,
      &|r| r.spec.limits.memory += 1,
      &|r| r.spec.limits.disk += 1,
      &|r| r.spec.backup_limit += 1,
      &|r| r.spec.allocations[0].port += 1,
      &|r| {
        r.spec.allocations.push(Allocation {
          ip: "10.0.0.5".into(),
          port: 1,
        })
      },
      &|r| r.grants = Some(GrantsBody::default()),
      &|r| r.spec.environment = map(&[("LOCKED", "changed")]),
      &|r| r.spec.environment = map(&[("HIDDEN_EDIT", "changed")]),
      // hidden and unknown keys count even with the current value, no oracle
      &|r| r.spec.environment = map(&[("HIDDEN_EDIT", "h")]),
      &|r| r.spec.environment = map(&[("UNKNOWN", "x")]),
    ] {
      assert!(changes(f));
    }
  }

  #[tokio::test]
  async fn lookup_db_errors_propagate() {
    // no migrations: every query fails with a db error, not a not-found
    let db = Connection(sea_orm::Database::connect("sqlite::memory:").await.unwrap());
    let mut errors = Vec::new();
    assert!(
      check_owner(&db, Some(Uuid::now_v7()), &mut errors)
        .await
        .is_err()
    );
    assert!(check_owner(&db, Option::None, &mut errors).await.is_ok());
    let err = egg_vars(&db, Uuid::now_v7(), &mut errors)
      .await
      .unwrap_err();
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(errors.is_empty());
  }

  #[test]
  fn field_errors_serialize_with_status() {
    let res = ServerError::Fields(
      StatusCode::CONFLICT,
      vec![field_err("allocations.0", "taken")],
    )
    .into_response();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let res = ServerError::from(centaurus::anyhow!(FORBIDDEN, "no")).into_response();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = ServerError::from(DbErr::Custom("x".into())).into_response();
    assert!(res.status().is_server_error());
    assert!(fail_if(StatusCode::BAD_REQUEST, vec![]).is_ok());
    assert!(matches!(
      fail_if(StatusCode::BAD_REQUEST, vec![field_err("a", "b")]),
      Err(ServerError::Fields(StatusCode::BAD_REQUEST, _))
    ));
    assert_eq!(
      serde_json::to_value(FieldErrors {
        errors: vec![field_err("name", "Name is required.")]
      })
      .unwrap(),
      json!({ "errors": [{ "field": "name", "message": "Name is required." }] })
    );
  }
}
