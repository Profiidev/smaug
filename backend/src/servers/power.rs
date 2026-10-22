use axum::{Json, extract::Path};
use centaurus::{db::init::Connection, error::Result};
use entity::server;
use http::Method;
use shared::msg::{CommandRequest, Power};

use super::{
  access::{Access, load},
  status::{Action, ensure_allowed_in, status},
  types::{PowerAction, ServerPowerRequest, ServerUuid},
};
use crate::{
  auth::jwt_auth::JwtAuth,
  db::DBTrait,
  nodes::{Wings, call},
  utils::{UpdateMessage, Updater},
};

/// Start and stop change whether the server should run, restart and kill are
/// one-shot. `409` when the current status does not allow the action.
pub(super) async fn server_power(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<ServerPowerRequest>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  apply_power(&db, &wings, &updater, &server, req.action).await
}

/// Shared with schedules. `409` when the current status does not allow it.
pub(crate) async fn apply_power(
  db: &Connection,
  wings: &Wings,
  updater: &Updater,
  server: &server::Model,
  power: PowerAction,
) -> Result<()> {
  let action = match power {
    PowerAction::Start => Action::Start,
    PowerAction::Stop => Action::Stop,
    PowerAction::Restart => Action::Restart,
    PowerAction::Kill => Action::Kill,
  };
  ensure_allowed_in(
    status(server, wings.state(server.node_id, server.id)),
    action,
  )?;

  let (node, id) = (server.node_id, server.id);
  match power {
    PowerAction::Start | PowerAction::Stop => {
      db.server()
        .set_power(id, power == PowerAction::Start)
        .await?;
      wings.put_server(node, id);
    }
    PowerAction::Restart => wings.power(node, id, Power::Restart),
    // power off first, a kill of a server that should run restarts it
    PowerAction::Kill => {
      db.server().set_power(id, false).await?;
      wings.put_and_kill(node, id);
    }
  }

  updater.broadcast(UpdateMessage::Servers { uuid: id }).await;
  Ok(())
}

/// `409` when the server is not running.
pub(super) async fn server_command(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(path): Path<ServerUuid>,
  Json(req): Json<CommandRequest>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  send_command(&wings, &server, req).await
}

/// Shared with schedules.
pub(crate) async fn send_command(
  wings: &Wings,
  server: &server::Model,
  req: CommandRequest,
) -> Result<()> {
  call(
    wings
      .server_request(server.node_id, server.id, Method::POST, "/command")?
      .json(&req),
  )
  .await?;
  Ok(())
}
