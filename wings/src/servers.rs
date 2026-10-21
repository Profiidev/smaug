use std::collections::HashSet;

use axum::{
  Extension, Json, Router,
  extract::Path,
  middleware::from_fn,
  routing::{post, put},
};
use centaurus::{bail, error::Result};
use chrono::Utc;
use shared::msg::{
  Observed, Power, PowerRequest, ServerSpec, ServerState, SyncRequest, SyncResponse,
};
use uuid::Uuid;

use crate::{
  auth,
  docker::Docker,
  store::{Desired, DesiredPower, Store},
};

/// The Docker runtime, `None` on Kubernetes.
pub type Runtime = Extension<Option<Docker>>;

// Wings only stores what the panel asks for. Every rule (suspended, not
// installed, ...) is checked by the panel.
pub fn router() -> Router {
  Router::new()
    .route("/servers", put(sync))
    .route("/servers/{uuid}", put(put_server).delete(delete_server))
    .route("/servers/{uuid}/power", post(power))
    .layer(from_fn(auth::verify))
}

/// Replaces the whole desired set. Unlisted servers are reported, not deleted.
async fn sync(
  store: Store,
  Extension(docker): Runtime,
  Json(req): Json<SyncRequest>,
) -> Result<Json<SyncResponse>> {
  let listed: HashSet<Uuid> = req.servers.iter().map(|s| s.uuid).collect();
  for spec in req.servers {
    upsert(&store, docker.as_ref(), spec).await?;
  }

  let mut res = SyncResponse::default();
  for (uuid, observed) in store.list().await? {
    if !listed.contains(&uuid) {
      res.orphans.push(uuid);
    }
    res.servers.insert(uuid, observed);
  }
  Ok(Json(res))
}

async fn put_server(
  store: Store,
  Extension(docker): Runtime,
  Path(uuid): Path<Uuid>,
  Json(spec): Json<ServerSpec>,
) -> Result<()> {
  if spec.uuid != uuid {
    bail!(BAD_REQUEST, "Server uuid does not match the path");
  }
  upsert(&store, docker.as_ref(), spec).await
}

async fn delete_server(store: Store, Path(uuid): Path<Uuid>) -> Result<()> {
  store.delete(uuid).await
}

async fn power(store: Store, Path(uuid): Path<Uuid>, Json(req): Json<PowerRequest>) -> Result<()> {
  let mut desired = get(&store, uuid).await?;
  apply_power(&mut desired, req.action);
  store.put(desired).await
}

/// Sets the spec, keeping power of an existing server, and starts a pending
/// install that has not run yet.
pub(crate) async fn upsert(store: &Store, docker: Option<&Docker>, spec: ServerSpec) -> Result<()> {
  let (uuid, pending) = (spec.uuid, spec.install_generation);
  let mut restored = false;
  let desired = match store.get(uuid).await? {
    Some(existing) => Desired {
      server: spec,
      ..existing
    },
    None => {
      let mut desired = Desired::new(spec);
      // the Docker store is lost when wings restarts, only the containers are
      // left: a running one was meant to run, and the sync response says so
      if let Some(docker) = docker
        && docker.inspect(uuid).await?.is_some_and(|c| c.running)
      {
        desired.power = DesiredPower::Running;
        restored = true;
      }
      desired
    }
  };
  store.put(desired).await?;
  if restored {
    store.set_state(uuid, ServerState::Running).await?;
  }

  let observed = store.observed(uuid).await?.unwrap_or_default();
  if needs_install(pending, &observed) {
    // ponytail: only the state, the runtime runs the install script later
    store.set_state(uuid, ServerState::Installing).await?;
  }
  Ok(())
}

/// A pending install runs once: not while it runs, not after it finished.
fn needs_install(pending: Option<u64>, observed: &Observed) -> bool {
  pending.is_some_and(|generation| {
    observed.state != ServerState::Installing
      && observed.last_install.map(|i| i.generation) != Some(generation)
  })
}

async fn get(store: &Store, uuid: Uuid) -> Result<Desired> {
  match store.get(uuid).await? {
    Some(desired) => Ok(desired),
    None => Err(centaurus::anyhow!(NOT_FOUND, "Server not found")),
  }
}

fn apply_power(desired: &mut Desired, action: Power) {
  desired.power = match action {
    Power::Start => DesiredPower::Running,
    Power::Stop => DesiredPower::Stopped,
    Power::Kill => {
      desired.killed_at = Some(Utc::now());
      DesiredPower::Stopped
    }
    Power::Restart => {
      desired.restarted_at = Some(Utc::now());
      DesiredPower::Running
    }
  };
}

#[cfg(test)]
mod test {
  use shared::msg::InstallResult;

  use super::*;
  use crate::store::test::{memory, spec};

  #[tokio::test]
  async fn sync_keeps_power_and_reports_orphans() {
    let store = memory();
    let (kept, orphan) = (Uuid::new_v4(), Uuid::new_v4());
    let mut desired = Desired::new(spec(kept));
    apply_power(&mut desired, Power::Restart);
    store.put(desired.clone()).await.unwrap();
    store.put(Desired::new(spec(orphan))).await.unwrap();

    let mut new_spec = spec(kept);
    new_spec.image = "new".into();
    let Json(res) = sync(
      store.clone(),
      Extension(None),
      Json(SyncRequest {
        servers: vec![new_spec.clone()],
      }),
    )
    .await
    .unwrap();

    assert_eq!(res.orphans, vec![orphan]);
    assert_eq!(res.servers.len(), 2);
    let stored = store.get(kept).await.unwrap().unwrap();
    assert_eq!(stored.server, new_spec);
    assert_eq!(stored.power, DesiredPower::Running);
    assert!(stored.restarted_at.is_some());
    assert!(store.get(orphan).await.unwrap().is_some());
  }

  #[test]
  fn kill_is_recorded_apart_from_stop() {
    let mut desired = Desired::new(spec(Uuid::new_v4()));
    apply_power(&mut desired, Power::Stop);
    assert_eq!(
      (desired.power, desired.killed_at),
      (DesiredPower::Stopped, None)
    );
    apply_power(&mut desired, Power::Kill);
    assert_eq!(desired.power, DesiredPower::Stopped);
    assert!(desired.killed_at.is_some());
  }

  #[tokio::test]
  async fn pending_install_runs_once() {
    let store = memory();
    let uuid = Uuid::new_v4();
    let mut spec = spec(uuid);
    spec.install_generation = Some(1);
    upsert(&store, None, spec.clone()).await.unwrap();
    assert_eq!(
      store.observed(uuid).await.unwrap().unwrap().state,
      ServerState::Installing
    );

    let done = |generation| Observed {
      state: ServerState::Offline,
      last_install: Some(InstallResult {
        generation,
        ok: true,
      }),
    };
    assert!(!needs_install(None, &Observed::default()));
    assert!(!needs_install(Some(1), &done(1)));
    assert!(needs_install(Some(2), &done(1)));
    assert!(!needs_install(
      Some(2),
      &Observed {
        state: ServerState::Installing,
        last_install: None
      }
    ));
  }
}
