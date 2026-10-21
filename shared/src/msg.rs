use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use http::HeaderName;
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
  /// Wings stopped the server itself (crash without restart, failed start),
  /// see [`Observed::stopped_generation`].
  Stopped {
    uuid: Uuid,
    generation: u64,
  },
  /// Flushed every 250 ms or 256 lines.
  Logs {
    uuid: Uuid,
    lines: Vec<LogLine>,
  },
  /// Every running server, sent every 2 s.
  Metrics {
    servers: Vec<ServerMetrics>,
  },
  /// End of an operation started with `202 {op}`.
  OpFinished {
    op: Uuid,
    uuid: Uuid,
    ok: bool,
    error: Option<String>,
    result: Option<OpResult>,
  },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
  pub at: DateTime<Utc>,
  pub stream: LogStream,
  pub line: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
  Console,
  Install,
  // wings' own messages, e.g. "Pulling image…"
  Daemon,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServerMetrics {
  pub uuid: Uuid,
  /// 1000 = one full core.
  pub cpu_millicores: u64,
  pub memory_bytes: u64,
  pub memory_limit_bytes: u64,
  pub disk_bytes: u64,
  pub net_rx_bps: u64,
  pub net_tx_bps: u64,
  pub uptime_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpResult {
  Stream { sha256: String, size: u64 },
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
  /// Whether the server should run.
  pub power: DesiredPower,
  /// Bumped by the panel on every `power` change. Wings applies `power` only
  /// when this differs from the generation it applied, so a stop wings made
  /// itself is not undone by a spec that still carries the old power.
  pub power_generation: u64,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesiredPower {
  Running,
  #[default]
  Stopped,
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

/// One-shot power request. Never changes whether the server should run, that
/// is only the spec's `power` and `power_generation`, so wings can not drift
/// from the panel DB. Restarting a stopped server does nothing; a kill of a
/// server that should run restarts it without the stop steps.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Power {
  Restart,
  /// Skips the stop steps of the stop that runs or follows.
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
  /// A backup is being restored, the server is stopped meanwhile.
  Restoring,
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
  /// Power generation at which wings stopped the server itself. The panel
  /// turns its power off unless it sent a newer generation since.
  #[serde(default)]
  pub stopped_generation: Option<u64>,
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

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct CommandRequest {
  pub line: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
  File,
  Dir,
  Symlink,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
  pub name: String,
  pub kind: FileKind,
  pub size: u64,
  /// Unix permission bits.
  pub mode: u32,
  pub modified: DateTime<Utc>,
  /// Guessed from the name, `None` for directories.
  pub mime: Option<String>,
}

/// Paths are relative to the server root, `/` is the root.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct PathQuery {
  pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct MkdirRequest {
  pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct RenameRequest {
  pub moves: Vec<Move>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct Move {
  pub from: String,
  pub to: String,
}

/// Copies `path` to `name copy.ext` next to it.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct CopyRequest {
  pub path: String,
}

/// Directories are deleted recursively.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct DeleteRequest {
  pub paths: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageResponse {
  pub bytes: u64,
}

pub const DEFAULT_CHUNK_SIZE: u64 = 8 * 1024 * 1024;
pub const DEFAULT_WINDOW: u32 = 4;
/// Set on the final chunk of an out-stream.
pub const STREAM_END: HeaderName = HeaderName::from_static("x-stream-end");
pub const STREAM_SHA256: HeaderName = HeaderName::from_static("x-stream-sha256");
pub const STREAM_SIZE: HeaderName = HeaderName::from_static("x-stream-size");

/// What an out-stream produces.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamSource {
  /// tar.zst of the server files, `ignore` are gitignore globs.
  Backup { ignore: Vec<String> },
}

/// What an in-stream feeds.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamTarget {
  /// Requires `size`, chunks may arrive in any order.
  File { path: String },
  /// tar.zst, `truncate` empties the server root first.
  Restore { truncate: bool },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OpenOutStream {
  pub source: StreamSource,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OpenInStream {
  pub target: StreamTarget,
  pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamOpened {
  pub stream: Uuid,
  pub chunk_size: u64,
  pub window: u32,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct CompleteStream {
  pub sha256: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StreamStatus {
  /// Every chunk below is done.
  pub next: u32,
  /// Chunks at or above `next` wings already holds.
  pub received: Vec<u32>,
}

/// `202` body of a slow request, see [`Event::OpFinished`].
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpStarted {
  pub op: Uuid,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
  BadRequest,
  Denied,
  NotFound,
  Conflict,
  Busy,
  TooLarge,
  Unsupported,
  Quota,
  Internal,
}

/// Body of every wings error response.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct ErrorBody {
  pub code: ErrorCode,
  pub message: String,
}

#[cfg(test)]
mod test {
  use serde_json::json;

  use super::*;

  fn round_trip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(
    value: T,
    expected: Value,
  ) {
    assert_eq!(serde_json::to_value(&value).unwrap(), expected);
    assert_eq!(serde_json::from_value::<T>(expected).unwrap(), value);
  }

  #[test]
  fn events() {
    let uuid = Uuid::nil();
    let at = DateTime::from_timestamp_millis(1_000).unwrap();
    round_trip(
      Event::Logs {
        uuid,
        lines: vec![LogLine {
          at,
          stream: LogStream::Daemon,
          line: "hi".into(),
        }],
      },
      json!({"type": "Logs", "uuid": uuid, "lines": [
        {"at": "1970-01-01T00:00:01Z", "stream": "daemon", "line": "hi"}
      ]}),
    );
    round_trip(
      Event::OpFinished {
        op: uuid,
        uuid,
        ok: true,
        error: None,
        result: Some(OpResult::Stream {
          sha256: "ab".into(),
          size: 3,
        }),
      },
      json!({"type": "OpFinished", "op": uuid, "uuid": uuid, "ok": true, "error": null,
        "result": {"type": "stream", "sha256": "ab", "size": 3}}),
    );
    round_trip(
      Event::State {
        uuid,
        state: ServerState::Restoring,
      },
      json!({"type": "State", "uuid": uuid, "state": "restoring"}),
    );
  }

  #[test]
  fn requests() {
    round_trip(
      OpenInStream {
        target: StreamTarget::Restore { truncate: true },
        size: None,
      },
      json!({"target": {"type": "restore", "truncate": true}, "size": null}),
    );
    round_trip(
      OpenOutStream {
        source: StreamSource::Backup {
          ignore: vec!["*.log".into()],
        },
      },
      json!({"source": {"type": "backup", "ignore": ["*.log"]}}),
    );
    round_trip(
      ErrorBody {
        code: ErrorCode::TooLarge,
        message: "x".into(),
      },
      json!({"code": "too_large", "message": "x"}),
    );
    round_trip(
      FileEntry {
        name: "a".into(),
        kind: FileKind::Symlink,
        size: 1,
        mode: 0o644,
        modified: DateTime::from_timestamp_millis(0).unwrap(),
        mime: None,
      },
      json!({"name": "a", "kind": "symlink", "size": 1, "mode": 420,
        "modified": "1970-01-01T00:00:00Z", "mime": null}),
    );
  }
}
