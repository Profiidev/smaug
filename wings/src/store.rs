use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use axum::{Extension, extract::FromRequestParts};
use centaurus::{error::Result, eyre::Context};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use kube::{
  Api, CustomResource,
  api::{DeleteParams, ListParams, Patch, PatchParams, PostParams},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
pub use shared::msg::DesiredPower;
use shared::msg::{Event, InstallResult, Observed, Runtime, ServerSpec, ServerState};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{config::Config, docker::Stdin};

const FIELD_MANAGER: &str = "wings";

/// Desired state of one server, as told by the panel. On Kubernetes this is
/// the spec of the `GameServer` custom resource.
#[derive(CustomResource, Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[kube(
  group = "smaug.dev",
  version = "v1alpha1",
  kind = "GameServer",
  namespaced,
  status = "Observed",
  printcolumn = r#"{"name":"State","type":"string","jsonPath":".status.state"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct Desired {
  pub server: ServerSpec,
  pub power: DesiredPower,
  /// The `power_generation` of the spec `power` was last taken from.
  #[serde(default)]
  pub power_generation: Option<u64>,
  /// Bumped on restart; the runtime restarts servers started before it.
  pub restarted_at: Option<DateTime<Utc>>,
  /// Bumped on kill; the runtime kills (no stop steps) servers started before it.
  pub killed_at: Option<DateTime<Utc>>,
}

impl Desired {
  pub fn new(server: ServerSpec) -> Self {
    let mut desired = Self {
      server: server.clone(),
      power: DesiredPower::default(),
      power_generation: None,
      restarted_at: None,
      killed_at: None,
    };
    desired.set_server(server);
    desired
  }

  /// Takes the spec, and its power when the panel changed it since it was
  /// last applied. A stop wings made itself stays until the panel sends a
  /// newer power.
  pub fn set_server(&mut self, server: ServerSpec) {
    if self.power_generation != Some(server.power_generation) {
      self.power = server.power;
      self.power_generation = Some(server.power_generation);
    }
    self.server = server;
  }
}

/// Where wings keeps server state: in memory on Docker, in `GameServer`
/// custom resources on Kubernetes. Wings checks nothing, it stores what the panel says.
#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
pub struct Store {
  backend: Backend,
  events: broadcast::Sender<Event>,
  /// Logs and metrics, lossy so a flooding console cannot push state and op
  /// events out of `events`.
  live: broadcast::Sender<Event>,
  /// Uuids whose desired state changed, for the reconciler.
  changes: broadcast::Sender<Uuid>,
  /// Stdin of every attached server, written by commands and the stop command.
  consoles: Arc<DashMap<Uuid, Console>>,
  /// Servers an operation (restore) holds, see [`Store::lock`].
  locks: Arc<DashMap<Uuid, ()>>,
  /// Held while the op lock is taken, and by checks that must not see it
  /// taken in between, see [`Store::unlocked`].
  // ponytail: one gate for all servers, per-server if power requests queue up
  gate: Arc<tokio::sync::Mutex<()>>,
}

pub type Console = Arc<tokio::sync::Mutex<Stdin>>;

/// Held while an operation owns the server, the reconciler keeps it stopped.
/// Dropping it releases the server and wakes its worker.
pub struct OpGuard {
  store: Store,
  uuid: Uuid,
}

impl Drop for OpGuard {
  fn drop(&mut self) {
    self.store.locks.remove(&self.uuid);
    self.store.touch(self.uuid);
  }
}

#[derive(Clone)]
enum Backend {
  // ponytail: lost on restart, the panel resyncs everything on reconnect
  Memory(Arc<DashMap<Uuid, (Desired, Observed)>>),
  Kube(Api<GameServer>),
}

fn name(uuid: Uuid) -> String {
  format!("smaug-{uuid}")
}

impl Store {
  pub async fn new(config: &Config) -> Self {
    let backend = match config.runtime {
      Runtime::Docker => Backend::Memory(Default::default()),
      Runtime::Kubernetes => {
        let client = kube::Client::try_default()
          .await
          .expect("Failed to create kubernetes client");
        Backend::Kube(Api::namespaced(client, &config.k8s_namespace))
      }
    };
    Self {
      backend,
      // a full buffer drops the panel connection, which costs a full sync
      events: broadcast::channel(4096).0,
      // a full buffer only skips lines and metrics
      live: broadcast::channel(4096).0,
      changes: broadcast::channel(256).0,
      consoles: Default::default(),
      locks: Default::default(),
      gate: Default::default(),
    }
  }

  /// Reports to the panel; nobody listening means no panel is connected.
  pub fn send(&self, event: Event) {
    let _ = match event {
      Event::Logs { .. } | Event::Metrics { .. } => self.live.send(event),
      _ => self.events.send(event),
    };
  }

  /// Wakes the server's reconcile worker.
  pub fn touch(&self, uuid: Uuid) {
    let _ = self.changes.send(uuid);
  }

  pub fn set_console(&self, uuid: Uuid, console: Option<Console>) {
    match console {
      Some(console) => self.consoles.insert(uuid, console),
      None => self.consoles.remove(&uuid).map(|e| e.1),
    };
  }

  pub fn console(&self, uuid: Uuid) -> Option<Console> {
    self.consoles.get(&uuid).map(|c| c.clone())
  }

  /// Takes the server's op lock, `None` when an operation already holds it.
  pub async fn lock(&self, uuid: Uuid) -> Option<OpGuard> {
    let _gate = self.gate.lock().await;
    match self.locks.entry(uuid) {
      dashmap::Entry::Occupied(_) => None,
      dashmap::Entry::Vacant(e) => {
        e.insert(());
        self.touch(uuid);
        Some(OpGuard {
          store: self.clone(),
          uuid,
        })
      }
    }
  }

  pub fn locked(&self, uuid: Uuid) -> bool {
    self.locks.contains_key(&uuid)
  }

  /// `None` while the server is locked, else holds off [`Store::lock`]
  /// until dropped, so a check and the change after it are atomic.
  pub async fn unlocked(&self, uuid: Uuid) -> Option<tokio::sync::MutexGuard<'_, ()>> {
    let gate = self.gate.lock().await;
    (!self.locked(uuid)).then_some(gate)
  }

  pub fn runtime(&self) -> Runtime {
    match self.backend {
      Backend::Memory(_) => Runtime::Docker,
      Backend::Kube(_) => Runtime::Kubernetes,
    }
  }

  pub fn subscribe(&self) -> broadcast::Receiver<Event> {
    self.events.subscribe()
  }

  /// Logs and metrics, see [`Store::send`].
  pub fn subscribe_live(&self) -> broadcast::Receiver<Event> {
    self.live.subscribe()
  }

  pub fn subscribe_changes(&self) -> broadcast::Receiver<Uuid> {
    self.changes.subscribe()
  }

  pub async fn get(&self, uuid: Uuid) -> Result<Option<Desired>> {
    Ok(match &self.backend {
      Backend::Memory(map) => map.get(&uuid).map(|e| e.0.clone()),
      Backend::Kube(api) => api
        .get_opt(&name(uuid))
        .await
        .context("Failed to get GameServer")?
        .map(|gs| gs.spec),
    })
  }

  pub async fn observed(&self, uuid: Uuid) -> Result<Option<Observed>> {
    Ok(match &self.backend {
      Backend::Memory(map) => map.get(&uuid).map(|e| e.1.clone()),
      Backend::Kube(api) => api
        .get_opt(&name(uuid))
        .await
        .context("Failed to get GameServer")?
        .map(|gs| gs.status.unwrap_or_default()),
    })
  }

  /// Creates or replaces the desired state, keeping the observed state.
  pub async fn put(&self, desired: Desired) -> Result<()> {
    let uuid = desired.server.uuid;
    match &self.backend {
      Backend::Memory(map) => {
        map
          .entry(uuid)
          .and_modify(|e| e.0 = desired.clone())
          .or_insert((desired, Observed::default()));
      }
      Backend::Kube(api) => {
        let mut gs = GameServer::new(&name(uuid), desired);
        gs.metadata.labels = Some([("smaug.server".into(), uuid.to_string())].into());
        api
          .patch(
            &name(uuid),
            &PatchParams::apply(FIELD_MANAGER).force(),
            &Patch::Apply(gs),
          )
          .await
          .context("Failed to apply GameServer")?;
      }
    }
    let _ = self.changes.send(uuid);
    Ok(())
  }

  /// Changes the desired state in place, `false` when the server is not
  /// stored. Unlike `get` + `put` it never undoes a concurrent change.
  pub async fn update(&self, uuid: Uuid, change: impl Fn(&mut Desired)) -> Result<bool> {
    match &self.backend {
      Backend::Memory(map) => match map.get_mut(&uuid) {
        Some(mut e) => change(&mut e.0),
        None => return Ok(false),
      },
      Backend::Kube(api) => loop {
        let Some(mut gs) = api
          .get_opt(&name(uuid))
          .await
          .context("Failed to get GameServer")?
        else {
          return Ok(false);
        };
        change(&mut gs.spec);
        // the resource version makes a concurrent change fail, retry on top of it
        match api.replace(&name(uuid), &PostParams::default(), &gs).await {
          Err(kube::Error::Api(err)) if err.code == 409 => continue,
          res => {
            res.context("Failed to update GameServer")?;
            break;
          }
        }
      },
    }
    let _ = self.changes.send(uuid);
    Ok(true)
  }

  pub async fn delete(&self, uuid: Uuid) -> Result<()> {
    match &self.backend {
      Backend::Memory(map) => {
        map.remove(&uuid);
      }
      Backend::Kube(api) => match api.delete(&name(uuid), &DeleteParams::default()).await {
        Err(kube::Error::Api(err)) if err.code == 404 => (),
        res => {
          res.context("Failed to delete GameServer")?;
        }
      },
    }
    let _ = self.changes.send(uuid);
    Ok(())
  }

  /// Every stored server with its observed state.
  pub async fn list(&self) -> Result<Vec<(Uuid, Observed)>> {
    Ok(match &self.backend {
      Backend::Memory(map) => map.iter().map(|e| (*e.key(), e.1.clone())).collect(),
      Backend::Kube(api) => api
        .list(&ListParams::default().labels("smaug.server"))
        .await
        .context("Failed to list GameServers")?
        .into_iter()
        .map(|gs| (gs.spec.server.uuid, gs.status.unwrap_or_default()))
        .collect(),
    })
  }

  /// Records the observed state and reports it to the panel.
  pub async fn set_state(&self, uuid: Uuid, state: ServerState) -> Result<()> {
    self
      .update_observed(
        uuid,
        |o| {
          o.state = state;
          Some(Event::State { uuid, state })
        },
        json!({ "state": state }),
      )
      .await
  }

  /// `set_state(to)`, but only while the server is in `from`.
  pub async fn set_state_if(&self, uuid: Uuid, from: ServerState, to: ServerState) -> Result<()> {
    self
      .update_observed(
        uuid,
        |o| {
          (o.state == from).then(|| {
            o.state = to;
            Event::State { uuid, state: to }
          })
        },
        json!({ "state": to }),
      )
      .await
  }

  /// Records the server's last error and reports it to the panel, unless it
  /// did not change. Retries keep failing the same way, they send nothing.
  pub async fn set_error(&self, uuid: Uuid, error: Option<String>) -> Result<()> {
    self.change_error(uuid, |_| true, error).await
  }

  /// Clears the error, but only while it is still `expected`.
  pub async fn clear_error_if(&self, uuid: Uuid, expected: &str) -> Result<()> {
    self
      .change_error(uuid, |e| e.as_deref() == Some(expected), None)
      .await
  }

  pub(crate) async fn change_error(
    &self,
    uuid: Uuid,
    when: impl Fn(&Option<String>) -> bool,
    error: Option<String>,
  ) -> Result<()> {
    self
      .update_observed(
        uuid,
        |o| {
          (o.error != error && when(&o.error)).then(|| {
            o.error = error.clone();
            Event::Error {
              uuid,
              error: error.clone(),
            }
          })
        },
        json!({ "error": error }),
      )
      .await
  }

  /// Records the end of an install and reports it to the panel.
  pub async fn finish_install(&self, uuid: Uuid, result: InstallResult) -> Result<()> {
    self
      .update_observed(
        uuid,
        |o| {
          o.last_install = Some(result);
          Some(Event::InstallFinished { uuid, result })
        },
        json!({ "last_install": result }),
      )
      .await
  }

  /// Wings turns the server off on its own (crash without restart, failed
  /// start) and tells the panel, which turns its power off too. Only while the
  /// power is still from `generation`, the one the decision was based on: a
  /// newer power from the panel wins. Returns whether it applied.
  pub async fn stop_self(&self, uuid: Uuid, generation: Option<u64>) -> Result<bool> {
    let applied = AtomicBool::new(false);
    self
      .update(uuid, |d| {
        let current = d.power_generation == generation;
        if current {
          d.power = DesiredPower::Stopped;
        }
        applied.store(current, Ordering::Relaxed);
      })
      .await?;
    let (true, Some(generation)) = (applied.into_inner(), generation) else {
      return Ok(false);
    };
    self
      .update_observed(
        uuid,
        |o| {
          o.stopped_generation = Some(generation);
          Some(Event::Stopped { uuid, generation })
        },
        json!({ "stopped_generation": generation }),
      )
      .await?;
    Ok(true)
  }

  /// `apply` changes the memory store and returns the event to report, `None`
  /// when nothing changed; `patch` is the same change as a status merge patch.
  /// On the memory store the event is sent while the entry is locked, so
  /// events reach the panel in the order the store changed.
  async fn update_observed(
    &self,
    uuid: Uuid,
    apply: impl FnOnce(&mut Observed) -> Option<Event>,
    patch: serde_json::Value,
  ) -> Result<()> {
    match &self.backend {
      Backend::Memory(map) => {
        if let Some(mut e) = map.get_mut(&uuid)
          && let Some(event) = apply(&mut e.1)
        {
          // no receiver means no panel is connected, it gets the state on its next sync
          let _ = self.events.send(event);
        }
      }
      Backend::Kube(api) => {
        // ponytail: read then patch, not atomic; fine while nothing reconciles on Kubernetes
        let Some(mut observed) = self.observed(uuid).await? else {
          return Ok(());
        };
        let Some(event) = apply(&mut observed) else {
          return Ok(());
        };
        api
          .patch_status(
            &name(uuid),
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": patch })),
          )
          .await
          .context("Failed to update GameServer status")?;
        let _ = self.events.send(event);
      }
    }
    Ok(())
  }
}

pub fn state(router: axum::Router, store: Store) -> axum::Router {
  router.layer(Extension(store))
}

#[cfg(test)]
pub(crate) mod test {
  use std::collections::BTreeMap;

  use shared::msg::{InstallSpec, Limits, Signal, StopAction};

  use super::*;

  pub(crate) fn memory() -> Store {
    Store {
      backend: Backend::Memory(Default::default()),
      live: broadcast::channel(16).0,
      events: broadcast::channel(16).0,
      changes: broadcast::channel(16).0,
      consoles: Default::default(),
      locks: Default::default(),
      gate: Default::default(),
    }
  }

  pub(crate) fn spec(uuid: Uuid) -> ServerSpec {
    ServerSpec {
      uuid,
      image: "img".into(),
      startup: "run".into(),
      env: BTreeMap::new(),
      limits: Limits {
        cpu_limit: 100,
        memory_mb: 512,
        disk_mb: 1024,
      },
      allocations: vec![],
      stop: StopAction::Signal(Signal::Term),
      stop_timeout_s: 60,
      restart_on_crash: true,
      startup_done: vec![],
      config_files: vec![],
      file_denylist: vec![],
      install: InstallSpec {
        image: "img".into(),
        entrypoint: "bash".into(),
        script: None,
      },
      install_generation: None,
      power: DesiredPower::Stopped,
      power_generation: 0,
    }
  }

  #[tokio::test]
  async fn memory_put_keeps_state() {
    let store = memory();
    let uuid = Uuid::new_v4();
    store.put(Desired::new(spec(uuid))).await.unwrap();
    store.set_state(uuid, ServerState::Running).await.unwrap();
    let result = InstallResult {
      generation: 2,
      ok: true,
    };
    store.finish_install(uuid, result).await.unwrap();

    let mut desired = Desired::new(spec(uuid));
    desired.server.image = "new".into();
    store.put(desired.clone()).await.unwrap();

    assert_eq!(store.get(uuid).await.unwrap(), Some(desired));
    assert_eq!(
      store.list().await.unwrap(),
      vec![(
        uuid,
        Observed {
          state: ServerState::Running,
          last_install: Some(result),
          error: None,
          stopped_generation: None,
        }
      )]
    );

    assert!(
      store
        .update(uuid, |d| d.power = DesiredPower::Running)
        .await
        .unwrap()
    );
    assert_eq!(
      store.get(uuid).await.unwrap().unwrap().power,
      DesiredPower::Running
    );

    store.delete(uuid).await.unwrap();
    assert!(store.get(uuid).await.unwrap().is_none());
    assert!(!store.update(uuid, |_| ()).await.unwrap());
  }

  #[tokio::test]
  async fn stop_self_reports_the_generation() {
    let store = memory();
    let mut events = store.subscribe();
    let uuid = Uuid::new_v4();
    let mut running = spec(uuid);
    running.power = DesiredPower::Running;
    running.power_generation = 3;
    store.put(Desired::new(running)).await.unwrap();
    // a decision based on an older power does not stop it
    assert!(!store.stop_self(uuid, Some(2)).await.unwrap());
    assert_eq!(
      store.get(uuid).await.unwrap().unwrap().power,
      DesiredPower::Running
    );
    assert!(store.stop_self(uuid, Some(3)).await.unwrap());

    assert_eq!(
      store.get(uuid).await.unwrap().unwrap().power,
      DesiredPower::Stopped
    );
    let observed = store.observed(uuid).await.unwrap().unwrap();
    assert_eq!(observed.stopped_generation, Some(3));
    assert_eq!(
      events.try_recv().unwrap(),
      Event::Stopped {
        uuid,
        generation: 3
      }
    );
  }

  #[test]
  fn spec_power_applies_once_per_generation() {
    let mut s = spec(Uuid::new_v4());
    s.power = DesiredPower::Running;
    s.power_generation = 1;
    let mut desired = Desired::new(s.clone());
    assert_eq!(desired.power, DesiredPower::Running);

    // stopped by wings, the same generation does not start it again
    desired.power = DesiredPower::Stopped;
    s.image = "new".into();
    desired.set_server(s.clone());
    assert_eq!(desired.power, DesiredPower::Stopped);
    assert_eq!(desired.server.image, "new");

    s.power_generation = 2;
    desired.set_server(s);
    assert_eq!(desired.power, DesiredPower::Running);
  }
}
