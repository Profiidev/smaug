use centaurus::{bail, error::Result};
use entity::{sea_orm_active_enums::ServerInstallState, server};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
  Offline,
  NotInstalled,
  Installing,
  InstallFailed,
  Suspended,
}

pub(super) fn status(server: &server::Model) -> ServerStatus {
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
    ServerStatus::Offline | ServerStatus::InstallFailed | ServerStatus::NotInstalled => {
      action != A::Unsuspend
    }
    ServerStatus::Installing => matches!(action, A::Install | A::Suspend),
    ServerStatus::Suspended => matches!(action, A::Install | A::Unsuspend | A::Delete),
  }
}

pub(super) fn ensure_allowed(server: &server::Model, action: Action) -> Result<()> {
  if !allowed(status(server), action) {
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
      suspended: false,
      start_on_completion: true,
      cpu_limit: 1000,
      memory_mb: 512,
      disk_mb: 1024,
      backup_limit: 2,
      created_at: Utc::now().fixed_offset(),
    }
  }

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
}
