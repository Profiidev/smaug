use centaurus::{bail, error::Result};
use entity::{sea_orm_active_enums::ServerInstallState, server};
use schemars::JsonSchema;
use serde::Serialize;
use shared::msg::ServerState;

#[derive(Serialize, JsonSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
  Running,
  Starting,
  Stopping,
  // exited on its own; `restart_on_crash` may start it again
  // (plain comments: variant docs turn the schema into `oneOf`, not an enum)
  Crashed,
  Offline,
  // the node is disconnected or not synced yet
  Unknown,
  NotInstalled,
  Installing,
  InstallFailed,
  Suspended,
}

/// `runtime` is what wings last reported, `None` while unknown.
pub(super) fn status(server: &server::Model, runtime: Option<ServerState>) -> ServerStatus {
  if server.suspended {
    return ServerStatus::Suspended;
  }
  match server.install_state {
    ServerInstallState::NotInstalled => ServerStatus::NotInstalled,
    ServerInstallState::Installing => ServerStatus::Installing,
    ServerInstallState::InstallFailed => ServerStatus::InstallFailed,
    ServerInstallState::Installed => match runtime {
      Some(ServerState::Starting) => ServerStatus::Starting,
      Some(ServerState::Running) => ServerStatus::Running,
      Some(ServerState::Stopping) => ServerStatus::Stopping,
      Some(ServerState::Crashed) => ServerStatus::Crashed,
      Some(ServerState::Offline | ServerState::Installing) => ServerStatus::Offline,
      None => ServerStatus::Unknown,
    },
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Action {
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
    ServerStatus::Running
    | ServerStatus::Starting
    | ServerStatus::Stopping
    | ServerStatus::Crashed
    | ServerStatus::Offline
    | ServerStatus::Unknown
    | ServerStatus::InstallFailed
    | ServerStatus::NotInstalled => action != A::Unsuspend,
    ServerStatus::Installing => matches!(action, A::Install | A::Suspend),
    ServerStatus::Suspended => matches!(action, A::Install | A::Unsuspend | A::Delete),
  }
}

pub(super) fn ensure_allowed(server: &server::Model, action: Action) -> Result<()> {
  // every runtime state allows the same actions, so it is left unknown
  if !allowed(status(server, None), action) {
    bail!(CONFLICT, "Action is not available in the current state");
  }
  Ok(())
}
#[cfg(test)]
pub(super) mod test {
  use chrono::Utc;
  use http::StatusCode;
  use uuid::Uuid;

  use super::*;

  pub(crate) fn model() -> server::Model {
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
      install_generation: 0,
      suspended: false,
      start_on_completion: true,
      cpu_limit: 1000,
      memory_mb: 512,
      disk_mb: 1024,
      backup_limit: 2,
      stop_timeout: 60,
      restart_on_crash: true,
      desired_running: false,
      power_generation: 0,
      created_at: Utc::now().fixed_offset(),
    }
  }

  #[test]
  fn status_derivation() {
    let mut s = model();
    for (state, expected) in [
      (ServerInstallState::Installed, ServerStatus::Unknown),
      (ServerInstallState::NotInstalled, ServerStatus::NotInstalled),
      (ServerInstallState::Installing, ServerStatus::Installing),
      (
        ServerInstallState::InstallFailed,
        ServerStatus::InstallFailed,
      ),
    ] {
      s.install_state = state;
      s.suspended = false;
      assert_eq!(status(&s, None), expected);
      s.suspended = true;
      assert_eq!(
        status(&s, Some(ServerState::Running)),
        ServerStatus::Suspended
      );
    }

    s.suspended = false;
    s.install_state = ServerInstallState::Installed;
    for (state, expected) in [
      (ServerState::Starting, ServerStatus::Starting),
      (ServerState::Running, ServerStatus::Running),
      (ServerState::Stopping, ServerStatus::Stopping),
      (ServerState::Crashed, ServerStatus::Crashed),
      (ServerState::Offline, ServerStatus::Offline),
      (ServerState::Installing, ServerStatus::Offline),
    ] {
      assert_eq!(status(&s, Some(state)), expected);
    }
    s.install_state = ServerInstallState::NotInstalled;
    assert_eq!(
      status(&s, Some(ServerState::Running)),
      ServerStatus::NotInstalled
    );
  }

  #[test]
  fn action_availability_matrix() {
    use Action::*;
    let actions = [Reinstall, Install, Suspend, Unsuspend, Transfer, Delete];
    let rows = [
      (ServerStatus::Running, [true, true, true, false, true, true]),
      (
        ServerStatus::Starting,
        [true, true, true, false, true, true],
      ),
      (
        ServerStatus::Stopping,
        [true, true, true, false, true, true],
      ),
      (ServerStatus::Crashed, [true, true, true, false, true, true]),
      (ServerStatus::Offline, [true, true, true, false, true, true]),
      (ServerStatus::Unknown, [true, true, true, false, true, true]),
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
}
