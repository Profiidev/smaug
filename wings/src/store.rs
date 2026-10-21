use std::sync::Arc;

use axum::{Extension, extract::FromRequestParts};
use centaurus::{error::Result, eyre::Context};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use kube::{
  Api, CustomResource,
  api::{DeleteParams, ListParams, Patch, PatchParams},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared::msg::{Event, InstallResult, Observed, Runtime, ServerSpec, ServerState};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::config::Config;

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
  /// Bumped on restart; the runtime restarts servers started before it.
  pub restarted_at: Option<DateTime<Utc>>,
  /// Bumped on kill; the runtime kills (no stop steps) servers started before it.
  pub killed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesiredPower {
  Running,
  #[default]
  Stopped,
}

impl Desired {
  pub fn new(server: ServerSpec) -> Self {
    Self {
      server,
      power: DesiredPower::default(),
      restarted_at: None,
      killed_at: None,
    }
  }
}

/// Where wings keeps server state: in memory on Docker, in `GameServer`
/// custom resources on Kubernetes. Wings checks nothing, it stores what the panel says.
#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
pub struct Store {
  backend: Backend,
  events: broadcast::Sender<Event>,
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
      events: broadcast::channel(256).0,
    }
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
      Backend::Memory(map) => map.get(&uuid).map(|e| e.1),
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
    Ok(())
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
    Ok(())
  }

  /// Every stored server with its observed state.
  pub async fn list(&self) -> Result<Vec<(Uuid, Observed)>> {
    Ok(match &self.backend {
      Backend::Memory(map) => map.iter().map(|e| (*e.key(), e.1)).collect(),
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
      .update_observed(uuid, |o| o.state = state, json!({ "state": state }))
      .await?;
    // no receiver means no panel is connected, it gets the state on its next sync
    let _ = self.events.send(Event::State { uuid, state });
    Ok(())
  }

  /// Records the end of an install and reports it to the panel. Called by the runtime.
  #[allow(unused)]
  pub async fn finish_install(&self, uuid: Uuid, result: InstallResult) -> Result<()> {
    self
      .update_observed(
        uuid,
        |o| o.last_install = Some(result),
        json!({ "last_install": result }),
      )
      .await?;
    let _ = self.events.send(Event::InstallFinished { uuid, result });
    Ok(())
  }

  /// `apply` for the memory store, `patch` is the same change as a status merge patch.
  async fn update_observed(
    &self,
    uuid: Uuid,
    apply: impl FnOnce(&mut Observed),
    patch: serde_json::Value,
  ) -> Result<()> {
    match &self.backend {
      Backend::Memory(map) => {
        if let Some(mut e) = map.get_mut(&uuid) {
          apply(&mut e.1);
        }
      }
      Backend::Kube(api) => {
        api
          .patch_status(
            &name(uuid),
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": patch })),
          )
          .await
          .context("Failed to update GameServer status")?;
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
      events: broadcast::channel(16).0,
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
        }
      )]
    );

    store.delete(uuid).await.unwrap();
    assert!(store.get(uuid).await.unwrap().is_none());
  }
}
