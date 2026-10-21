use std::collections::HashSet;

use axum::{
  Extension, Json, Router,
  extract::{Path, Query},
  middleware::from_fn,
  routing::{post, put},
};
use centaurus::{bail, error::Result};
use chrono::Utc;
use serde::Deserialize;
use shared::msg::{
  Observed, Power, PowerRequest, ServerSpec, ServerState, SyncRequest, SyncResponse,
};
use tracing::warn;
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
    let uuid = spec.uuid;
    // one broken server must not keep the others from syncing, listed it is no orphan
    if let Err(err) = upsert(&store, docker.as_ref(), spec).await {
      warn!("Failed to sync server {uuid}: {err:?}");
      // shown by the panel, cleared by the next upsert that works
      let _ = store
        .set_error(uuid, Some(format!("{SPEC_ERROR}: {err:#}")))
        .await;
    }
  }

  let mut res = SyncResponse::default();
  for (uuid, observed) in store.list().await? {
    if !listed.contains(&uuid) {
      res.orphans.push(uuid);
    }
    res.servers.insert(uuid, observed);
  }
  // leftovers of servers deleted while the store was lost (wings restart):
  // a delete from the panel tears them down
  if let Some(docker) = &docker {
    for uuid in docker.labelled().await? {
      if !listed.contains(&uuid) && !res.servers.contains_key(&uuid) {
        res.orphans.push(uuid);
      }
    }
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

#[derive(Deserialize)]
struct DeleteQuery {
  /// Leaves the server's files on the node, for a transfer.
  #[serde(default)]
  keep_files: bool,
}

async fn delete_server(
  store: Store,
  Extension(docker): Runtime,
  Path(uuid): Path<Uuid>,
  Query(query): Query<DeleteQuery>,
) -> Result<()> {
  // TODO(wings): keep the PVC on Kubernetes too, once its reconciler tears servers down
  if let Some(docker) = docker {
    docker.keep_volume(uuid, query.keep_files).await?;
  }
  store.delete(uuid).await
}

async fn power(store: Store, Path(uuid): Path<Uuid>, Json(req): Json<PowerRequest>) -> Result<()> {
  if !store.update(uuid, |d| apply_power(d, req.action)).await? {
    bail!(NOT_FOUND, "Server not found");
  }
  // a new request, an error from the last one is no longer current
  store.set_error(uuid, None).await
}

/// Sets the spec, taking its power when the panel changed it, and starts a
/// pending install that has not run yet.
pub(crate) async fn upsert(store: &Store, docker: Option<&Docker>, spec: ServerSpec) -> Result<()> {
  let (uuid, pending) = (spec.uuid, spec.install_generation);
  if !store.update(uuid, |d| d.set_server(spec.clone())).await? {
    // the Docker store is lost when wings restarts, the install result and a
    // stop wings made itself are on disk
    let mut desired = Desired::new(spec);
    let stopped = docker.and_then(|d| d.last_stopped(uuid));
    if stopped.is_some() && stopped == desired.power_generation {
      desired.power = DesiredPower::Stopped;
    }
    store.put(desired).await?;
    if stopped.is_some() {
      store.stop_self(uuid, stopped).await?;
    }
    if let Some(result) = docker.and_then(|d| d.last_install(uuid)) {
      store.finish_install(uuid, result).await?;
    }
  }
  if let Some(docker) = docker {
    // listed again, e.g. transferred back: its files are no longer kept for a
    // transfer. After the store, so a teardown that missed the store sees the marker.
    docker.keep_volume(uuid, false).await?;
  }
  store
    .change_error(
      uuid,
      |e| e.as_deref().is_some_and(|e| e.starts_with(SPEC_ERROR)),
      None,
    )
    .await?;

  let observed = store.observed(uuid).await?.unwrap_or_default();
  if needs_install(pending, &observed) {
    // ponytail: only the state, the runtime runs the install script later
    store.set_state(uuid, ServerState::Installing).await?;
  }
  Ok(())
}

/// Prefix of the error a failed upsert during a sync leaves.
const SPEC_ERROR: &str = "Failed to apply the panel's spec";

/// A pending install runs once: not while it runs, not after it finished.
fn needs_install(pending: Option<u64>, observed: &Observed) -> bool {
  pending.is_some_and(|generation| {
    observed.state != ServerState::Installing
      && observed.last_install.map(|i| i.generation) != Some(generation)
  })
}

fn apply_power(desired: &mut Desired, action: Power) {
  match action {
    Power::Kill => desired.killed_at = Some(Utc::now()),
    Power::Restart => desired.restarted_at = Some(Utc::now()),
  }
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
    // differs from the spec of the same generation, the sync must keep it
    desired.power = DesiredPower::Running;
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
  fn power_requests_never_change_power() {
    let mut s = spec(Uuid::new_v4());
    s.power = DesiredPower::Running;
    let mut desired = Desired::new(s);
    apply_power(&mut desired, Power::Kill);
    assert_eq!(desired.power, DesiredPower::Running);
    assert!(desired.killed_at.is_some());
    desired.power = DesiredPower::Stopped;
    apply_power(&mut desired, Power::Restart);
    assert_eq!(desired.power, DesiredPower::Stopped);
    assert!(desired.restarted_at.is_some());
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
      error: None,
      state: ServerState::Offline,
      last_install: Some(InstallResult {
        generation,
        ok: true,
      }),
      stopped_generation: None,
    };
    assert!(!needs_install(None, &Observed::default()));
    assert!(!needs_install(Some(1), &done(1)));
    assert!(needs_install(Some(2), &done(1)));
    assert!(!needs_install(
      Some(2),
      &Observed {
        state: ServerState::Installing,
        ..Default::default()
      }
    ));
  }
}
