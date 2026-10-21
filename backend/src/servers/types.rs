use std::collections::HashMap;

use entity::sea_orm_active_enums::{ServerAccessLevel, ServerInstallState};
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{access::Access, status::ServerStatus};
use crate::eggs::EggVariable;

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
pub(super) struct Named {
  pub(super) uuid: Uuid,
  pub(super) name: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Grant {
  /// User or group id.
  pub(super) uuid: Uuid,
  pub(super) access: ServerAccessLevel,
}

#[derive(Deserialize, JsonSchema, Debug, Default)]
pub(super) struct GrantsBody {
  #[serde(default)]
  pub(super) users: Vec<Grant>,
  #[serde(default)]
  pub(super) groups: Vec<Grant>,
}

#[derive(Serialize, JsonSchema, Debug)]
pub(super) struct GrantInfo {
  pub(super) uuid: Uuid,
  pub(super) name: String,
  pub(super) access: ServerAccessLevel,
}

#[derive(Serialize, JsonSchema, Debug)]
pub(super) struct GrantsInfo {
  pub(super) users: Vec<GrantInfo>,
  pub(super) groups: Vec<GrantInfo>,
}

// Fields shared by create and update.
#[derive(Deserialize, JsonSchema, Debug, Clone)]
pub(super) struct ServerSpec {
  pub(super) name: String,
  pub(super) owner: Option<Uuid>,
  pub(super) egg: Uuid,
  /// First one is the primary allocation.
  pub(super) allocations: Vec<Allocation>,
  pub(super) startup: String,
  /// Overridden commands are left alone by egg updates.
  pub(super) startup_overridden: bool,
  pub(super) image: String,
  /// env name -> value; missing variables keep their current value (or the
  /// egg default).
  #[serde(default)]
  pub(super) environment: HashMap<String, String>,
  pub(super) limits: Limits,
  /// 0 disables backups.
  pub(super) backup_limit: i32,
  /// Seconds per stop step before wings escalates to SIGTERM, then SIGKILL.
  pub(super) stop_timeout: i32,
  /// Restart the server when it exits without being stopped.
  pub(super) restart_on_crash: bool,
}

#[derive(Deserialize, JsonSchema, Debug)]
pub(super) struct CreateServer {
  #[serde(flatten)]
  pub(super) spec: ServerSpec,
  pub(super) node: Uuid,
  /// Mark the server installed without running the egg install script.
  pub(super) skip_scripts: bool,
  pub(super) start_on_completion: bool,
}

#[derive(Deserialize, JsonSchema, Debug)]
pub(super) struct UpdateServer {
  #[serde(flatten)]
  pub(super) spec: ServerSpec,
  /// Replaces all grants when set (admin only).
  #[serde(default)]
  pub(super) grants: Option<GrantsBody>,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct CreateServerResponse {
  pub(super) uuid: Uuid,
}

#[derive(Deserialize, JsonSchema)]
pub(super) struct ServerUuid {
  pub(super) uuid: Uuid,
}

#[derive(Deserialize, JsonSchema)]
pub(super) struct InstallRequest {
  pub(super) installed: bool,
}

#[derive(Deserialize, JsonSchema)]
pub(super) struct SuspendRequest {
  pub(super) suspended: bool,
}

#[derive(Deserialize, JsonSchema)]
pub(super) struct TransferRequest {
  pub(super) node: Uuid,
  pub(super) ip: String,
  pub(super) port: i32,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct ServerListItem {
  pub(super) uuid: Uuid,
  pub(super) name: String,
  pub(super) status: ServerStatus,
  pub(super) node: Named,
  pub(super) egg: Named,
  pub(super) owner: Option<Named>,
  pub(super) primary_allocation: Option<Allocation>,
  pub(super) extra_allocations: Vec<Allocation>,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct ListServersResponse {
  pub(super) servers: Vec<ServerListItem>,
}

#[derive(Serialize, JsonSchema, Debug)]
pub(super) struct VariableInfo {
  pub(super) name: String,
  pub(super) description: String,
  pub(super) env_variable: String,
  pub(super) default_value: String,
  pub(super) value: String,
  pub(super) rules: Vec<String>,
  /// Whether the caller may change it.
  pub(super) editable: bool,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct ServerInfo {
  pub(super) uuid: Uuid,
  pub(super) name: String,
  pub(super) status: ServerStatus,
  /// The caller's access level.
  pub(super) access: Access,
  pub(super) owner: Option<Named>,
  pub(super) node: Named,
  pub(super) egg: Named,
  pub(super) allocations: Vec<Allocation>,
  pub(super) startup: String,
  pub(super) startup_overridden: bool,
  pub(super) image: String,
  pub(super) install_state: ServerInstallState,
  pub(super) suspended: bool,
  pub(super) start_on_completion: bool,
  pub(super) limits: Limits,
  pub(super) backup_limit: i32,
  pub(super) stop_timeout: i32,
  pub(super) restart_on_crash: bool,
  pub(super) created_at: chrono::DateTime<chrono::FixedOffset>,
  /// Egg variables merged with the server's values; non-admins only see
  /// `user_viewable` ones.
  pub(super) variables: Vec<VariableInfo>,
  /// Only returned to admins.
  pub(super) grants: Option<GrantsInfo>,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct OptionEgg {
  pub(super) uuid: Uuid,
  pub(super) name: String,
  /// label -> image, order preserved
  pub(super) docker_images: IndexMap<String, String>,
  /// label -> command, order preserved; the first one is the default
  pub(super) startup_commands: IndexMap<String, String>,
  pub(super) variables: Vec<EggVariable>,
}

#[derive(Serialize, JsonSchema)]
pub(super) struct ServerOptions {
  pub(super) users: Vec<Named>,
  pub(super) groups: Vec<Named>,
  pub(super) nodes: Vec<Named>,
  pub(super) eggs: Vec<OptionEgg>,
}
