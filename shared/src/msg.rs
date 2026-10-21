use std::collections::{BTreeMap, HashMap};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Panel -> wings messages over the websocket. State transfer uses REST, this
/// stays for future push messages.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum WingsMessage {
  Hello,
}

/// Wings -> panel messages over the websocket.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum Event {
  /// First message on every connection.
  Hello {
    runtime: Runtime,
  },
  State {
    uuid: Uuid,
    state: ServerState,
  },
  InstallFinished {
    uuid: Uuid,
    ok: bool,
  },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
  Docker,
  Kubernetes,
}

/// Desired server config, owned by the panel.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct ServerSpec {
  pub uuid: Uuid,
  pub image: String,
  /// Startup command with `{{VAR}}` already substituted.
  pub startup: String,
  pub env: BTreeMap<String, String>,
  pub limits: Limits,
  /// First entry is the primary allocation.
  pub allocations: Vec<Allocation>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct Limits {
  pub cpu_limit: i32,
  pub memory_mb: i64,
  pub disk_mb: i64,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
  pub ip: String,
  pub port: u16,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Power {
  Start,
  Stop,
  Restart,
  Kill,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerRequest {
  pub action: Power,
}

/// Runtime state observed by wings.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
  #[default]
  Offline,
  Installing,
  Starting,
  Running,
  Stopping,
}

/// Full desired set for one node, sent on every (re)connect.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncRequest {
  pub servers: Vec<ServerSpec>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SyncResponse {
  /// Servers wings stores that the panel did not list. Never deleted by wings.
  pub orphans: Vec<Uuid>,
  pub states: HashMap<Uuid, ServerState>,
}
