use std::collections::{BTreeMap, HashMap};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    #[serde(flatten)]
    result: InstallResult,
  },
  /// The server's last error changed, see [`Observed::error`].
  Error {
    uuid: Uuid,
    error: Option<String>,
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
  pub stop: StopAction,
  /// Seconds each stop step may take before wings escalates
  /// (stop action, then SIGTERM, then SIGKILL).
  pub stop_timeout_s: u32,
  /// Restart when the server exits without a stop or kill request.
  pub restart_on_crash: bool,
  /// The server counts as running once a console line contains one of these.
  /// Empty means running as soon as the process started.
  pub startup_done: Vec<String>,
  /// Rewritten before every start.
  pub config_files: Vec<ConfigFile>,
  /// Gitignore-style patterns users may not read or write.
  pub file_denylist: Vec<String>,
  pub install: InstallSpec,
  /// Generation of the install the panel wants run, `None` when none is
  /// pending. Wings runs it once, unless its last install already has it.
  pub install_generation: Option<u64>,
}

/// How to stop the server gracefully, before falling back to SIGTERM/SIGKILL.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopAction {
  /// Written to the console.
  Command(String),
  Signal(Signal),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
  Int,
  Term,
  Kill,
}

/// A file wings rewrites before start. Placeholders the panel knows are
/// already substituted; `{{config.*}}` ones are left for wings.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
  /// Relative to the server root.
  pub file: String,
  pub parser: ConfigParser,
  pub replace: Vec<ConfigReplace>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigParser {
  File,
  Yaml,
  Properties,
  Ini,
  Json,
  Xml,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct ConfigReplace {
  /// Key path (or line prefix for the `file` parser) to replace.
  pub matcher: String,
  /// Only replace when the current value equals this.
  pub if_value: Option<String>,
  /// Any JSON value, so the `json`/`yaml` parsers keep numbers and booleans.
  #[schemars(schema_with = "any_json")]
  pub replace_with: Value,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct InstallSpec {
  pub image: String,
  pub entrypoint: String,
  /// `None` means nothing to run, the install succeeds right away.
  pub script: Option<String>,
}

/// Arbitrary JSON; Kubernetes needs the marker to accept it in a CRD.
fn any_json(_: &mut SchemaGenerator) -> Schema {
  json_schema!({ "x-kubernetes-preserve-unknown-fields": true })
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
  /// Exited without a stop or kill request. Followed by `Starting` when
  /// `restart_on_crash` restarts it.
  Crashed,
}

/// What wings observed, next to the desired state it stores.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq, Default)]
pub struct Observed {
  pub state: ServerState,
  /// The last install that finished. Lets the panel catch up on
  /// `InstallFinished` events it missed while disconnected.
  pub last_install: Option<InstallResult>,
  /// Why wings could not do what the panel asked, e.g. start or install the
  /// server. Cleared once the server runs, installs or gets a power request.
  #[serde(default)]
  pub error: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallResult {
  pub generation: u64,
  pub ok: bool,
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
  /// Every stored server, orphans included.
  pub servers: HashMap<Uuid, Observed>,
}

/// Wings -> agent: everything to do before a start.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PrepareRequest {
  pub config_files: Vec<ConfigFile>,
  /// What `{{config.docker.interface}}` becomes.
  pub interface: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct PrepareResponse {
  pub disk_bytes: u64,
}
