use axum::{Json, extract::Path};
use centaurus::{bail, db::init::Connection, error::Result};
use entity::sea_orm_active_enums::ServerInstallState;
use http::StatusCode;
use shared::msg::Power;
use tracing::info;

use super::{
  access::{Access, load},
  error::{FieldResult, fail_if, field_err},
  spec,
  status::{Action, ensure_allowed},
  types::{InstallRequest, ServerUuid, SuspendRequest, TransferRequest},
  validation::{check_free, node_exists, parse_allocation},
};
use crate::{
  auth::jwt_auth::JwtAuth,
  db::DBTrait,
  nodes::Wings,
  utils::{UpdateMessage, Updater},
};

pub(super) async fn reinstall_server(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Path(req): Path<ServerUuid>,
) -> Result<()> {
  let (mut server, _) = load(&db, auth.user_id, req.uuid, Access::Write).await?;
  ensure_allowed(&server, Action::Reinstall)?;
  server.install_state = ServerInstallState::Installing;
  let node = server.node_id;
  db.server().save(server).await?;
  wings.install(node, req.uuid).await;
  info!("Reinstalling server with ID {}", req.uuid);

  updater
    .broadcast(UpdateMessage::Servers { uuid: req.uuid })
    .await;
  Ok(())
}

pub(super) async fn set_server_installed(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<InstallRequest>,
) -> Result<()> {
  let (mut server, _) = load(&db, auth.user_id, path.uuid, Access::Admin).await?;
  ensure_allowed(&server, Action::Install)?;
  // only flips: installed -> not installed, anything else -> installed
  if req.installed == (server.install_state == ServerInstallState::Installed) {
    bail!(CONFLICT, "Server already has this install state");
  }
  server.install_state = if req.installed {
    ServerInstallState::Installed
  } else {
    ServerInstallState::NotInstalled
  };
  db.server().save(server).await?;

  updater
    .broadcast(UpdateMessage::Servers { uuid: path.uuid })
    .await;
  Ok(())
}

pub(super) async fn set_server_suspended(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<SuspendRequest>,
) -> Result<()> {
  let (mut server, _) = load(&db, auth.user_id, path.uuid, Access::Admin).await?;
  ensure_allowed(
    &server,
    if req.suspended {
      Action::Suspend
    } else {
      Action::Unsuspend
    },
  )?;
  server.suspended = req.suspended;
  let node = server.node_id;
  db.server().save(server).await?;
  // unsuspend needs nothing on the node, the panel blocks starts while suspended
  if req.suspended {
    wings.power(node, path.uuid, Power::Kill).await;
  }

  updater
    .broadcast(UpdateMessage::Servers { uuid: path.uuid })
    .await;
  Ok(())
}

pub(super) async fn transfer_server(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<TransferRequest>,
) -> FieldResult<()> {
  let (mut server, _) = load(&db, auth.user_id, path.uuid, Access::Admin).await?;
  ensure_allowed(&server, Action::Transfer)?;

  let mut errors = Vec::new();
  if req.node == server.node_id {
    errors.push(field_err("node", "Server is already on this node."));
  } else if !node_exists(&db, req.node).await? {
    errors.push(field_err("node", "Node not found."));
  }
  let allocation = parse_allocation(&req.ip, req.port, "", &mut errors);
  fail_if(StatusCode::BAD_REQUEST, errors)?;
  let allocation = allocation.expect("valid when there are no errors");
  check_free(
    &db,
    req.node,
    std::slice::from_ref(&allocation),
    None,
    |_| "port".into(),
  )
  .await?;

  let source = server.node_id;
  server.node_id = req.node;
  let variables = db.server().variables(server.id).await?;
  let wings_spec = spec::build(&server, std::slice::from_ref(&allocation), variables);
  wings.put_server(req.node, &wings_spec).await;
  db.server().transfer(server, allocation).await?;
  // TODO(wings): stream the server's files from the source to the target node before deleting them
  wings.delete_server(source, path.uuid).await;
  info!("Transferred server {} to node {}", path.uuid, req.node);

  updater
    .broadcast(UpdateMessage::Servers { uuid: path.uuid })
    .await;
  Ok(())
}
