use std::collections::HashSet;

use axum::{
  Json, Router,
  extract::Path,
  middleware::from_fn,
  routing::{post, put},
};
use centaurus::{bail, error::Result};
use chrono::Utc;
use shared::msg::{Power, PowerRequest, ServerSpec, ServerState, SyncRequest, SyncResponse};
use uuid::Uuid;

use crate::{
  auth,
  store::{Desired, DesiredPower, Store},
};

// Wings only stores what the panel asks for. Every rule (suspended, not
// installed, ...) is checked by the panel.
pub fn router() -> Router {
  Router::new()
    .route("/servers", put(sync))
    .route("/servers/{uuid}", put(put_server).delete(delete_server))
    .route("/servers/{uuid}/install", post(install))
    .route("/servers/{uuid}/power", post(power))
    .layer(from_fn(auth::verify))
}

/// Replaces the whole desired set. Unlisted servers are reported, not deleted.
async fn sync(store: Store, Json(req): Json<SyncRequest>) -> Result<Json<SyncResponse>> {
  let listed: HashSet<Uuid> = req.servers.iter().map(|s| s.uuid).collect();
  for spec in req.servers {
    upsert(&store, spec).await?;
  }

  let mut res = SyncResponse::default();
  for (uuid, state) in store.list().await? {
    if !listed.contains(&uuid) {
      res.orphans.push(uuid);
    }
    res.states.insert(uuid, state);
  }
  Ok(Json(res))
}

async fn put_server(
  store: Store,
  Path(uuid): Path<Uuid>,
  Json(spec): Json<ServerSpec>,
) -> Result<()> {
  if spec.uuid != uuid {
    bail!(BAD_REQUEST, "Server uuid does not match the path");
  }
  upsert(&store, spec).await
}

async fn delete_server(store: Store, Path(uuid): Path<Uuid>) -> Result<()> {
  store.delete(uuid).await
}

async fn install(store: Store, Path(uuid): Path<Uuid>) -> Result<()> {
  let mut desired = get(&store, uuid).await?;
  desired.install_generation += 1;
  store.put(desired).await?;
  store.set_state(uuid, ServerState::Installing).await
}

async fn power(store: Store, Path(uuid): Path<Uuid>, Json(req): Json<PowerRequest>) -> Result<()> {
  let mut desired = get(&store, uuid).await?;
  apply_power(&mut desired, req.action);
  store.put(desired).await
}

/// Sets the spec, keeping power and install generation of an existing server.
async fn upsert(store: &Store, spec: ServerSpec) -> Result<()> {
  let desired = match store.get(spec.uuid).await? {
    Some(existing) => Desired {
      server: spec,
      ..existing
    },
    None => Desired::new(spec),
  };
  store.put(desired).await
}

async fn get(store: &Store, uuid: Uuid) -> Result<Desired> {
  match store.get(uuid).await? {
    Some(desired) => Ok(desired),
    None => Err(centaurus::anyhow!(NOT_FOUND, "Server not found")),
  }
}

// ponytail: kill only sets Stopped, the runtime adds the immediate kill
fn apply_power(desired: &mut Desired, action: Power) {
  desired.power = match action {
    Power::Start => DesiredPower::Running,
    Power::Stop | Power::Kill => DesiredPower::Stopped,
    Power::Restart => {
      desired.restarted_at = Some(Utc::now());
      DesiredPower::Running
    }
  };
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::store::test::{memory, spec};

  #[tokio::test]
  async fn sync_keeps_power_and_reports_orphans() {
    let store = memory();
    let (kept, orphan) = (Uuid::new_v4(), Uuid::new_v4());
    let mut desired = Desired::new(spec(kept));
    apply_power(&mut desired, Power::Restart);
    desired.install_generation = 3;
    store.put(desired.clone()).await.unwrap();
    store.put(Desired::new(spec(orphan))).await.unwrap();

    let mut new_spec = spec(kept);
    new_spec.image = "new".into();
    let Json(res) = sync(
      store.clone(),
      Json(SyncRequest {
        servers: vec![new_spec.clone()],
      }),
    )
    .await
    .unwrap();

    assert_eq!(res.orphans, vec![orphan]);
    assert_eq!(res.states.len(), 2);
    let stored = store.get(kept).await.unwrap().unwrap();
    assert_eq!(stored.server, new_spec);
    assert_eq!(stored.power, DesiredPower::Running);
    assert_eq!(stored.install_generation, 3);
    assert!(store.get(orphan).await.unwrap().is_some());
  }
}
