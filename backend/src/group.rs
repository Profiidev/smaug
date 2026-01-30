use axum::{
  Json, Router,
  extract::{FromRequest, FromRequestParts, Path},
  routing::{delete, get, post, put},
};
use centaurus::{bail, db::init::Connection, error::Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
  auth::jwt_auth::JwtAuth,
  db::{DBTrait, group::GroupInfo},
  permissions::{GroupEdit, GroupView},
  ws::state::{UpdateMessage, Updater},
};

pub fn router() -> Router {
  Router::new()
    .route("/", get(list_groups))
    .route("/", post(create_group))
    .route("/", delete(delete_group))
    .route("/", put(edit_group))
    .route("/{uuid}", get(group_info))
}

#[derive(Serialize)]
struct ListGroupResponse {
  groups: Vec<GroupInfo>,
  admin_group: Option<Uuid>,
}

async fn list_groups(_auth: JwtAuth<GroupView>, db: Connection) -> Result<Json<ListGroupResponse>> {
  let groups = db.group().list_groups().await?;
  let admin_group = db.setup().get_admin_group_id().await?;
  Ok(Json(ListGroupResponse {
    groups,
    admin_group,
  }))
}

#[derive(Deserialize, FromRequestParts)]
#[from_request(via(Path))]
struct GroupViewPath {
  uuid: Uuid,
}

async fn group_info(
  _auth: JwtAuth<GroupView>,
  db: Connection,
  path: GroupViewPath,
) -> Result<Json<GroupInfo>> {
  let info = db.group().group_info(path.uuid).await?;
  let Some(info) = info else {
    bail!(NOT_FOUND, "Group not found");
  };
  Ok(Json(info))
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct CreateGroupRequest {
  name: String,
}

#[derive(Serialize)]
struct GroupCreateResponse {
  uuid: Uuid,
}

async fn create_group(
  _auth: JwtAuth<GroupEdit>,
  db: Connection,
  updater: Updater,
  data: CreateGroupRequest,
) -> Result<Json<GroupCreateResponse>> {
  if db.group().find_group_by_name(&data.name).await?.is_some() {
    bail!(CONFLICT, "A group with this name already exists");
  }

  let group_id = db.group().create_group(data.name).await?;
  updater.broadcast(UpdateMessage::Groups).await;

  Ok(Json(GroupCreateResponse { uuid: group_id }))
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct DeleteGroupRequest {
  uuid: Uuid,
}

async fn delete_group(
  _auth: JwtAuth<GroupEdit>,
  db: Connection,
  updater: Updater,
  data: DeleteGroupRequest,
) -> Result<()> {
  if let Some(admin_group) = db.setup().get_admin_group_id().await?
    && admin_group == data.uuid
  {
    bail!(BAD_REQUEST, "Cannot delete the admin group");
  }

  db.group().delete_group(data.uuid).await?;
  updater.broadcast(UpdateMessage::Groups).await;

  Ok(())
}

#[derive(Deserialize, FromRequest)]
#[from_request(via(Json))]
struct EditGroupRequest {
  uuid: Uuid,
  name: String,
  permissions: Vec<String>,
  users: Vec<Uuid>,
}

async fn edit_group(
  auth: JwtAuth<GroupEdit>,
  db: Connection,
  updater: Updater,
  data: EditGroupRequest,
) -> Result<()> {
  if let Some(existing_group) = db.group().find_group_by_name(&data.name).await?
    && existing_group != data.uuid
  {
    bail!(CONFLICT, "A group with this name already exists");
  }

  let Some(group) = db.group().group_info(data.uuid).await? else {
    bail!(NOT_FOUND, "Group not found");
  };

  let user_permissions = db.group().get_user_permissions(auth.user_id).await?;
  if group
    .permissions
    .iter()
    .any(|perm| !user_permissions.contains(perm))
  {
    bail!(
      FORBIDDEN,
      "Cannot edit a group with permissions you do not have"
    );
  }
  if data
    .permissions
    .iter()
    .any(|perm| !user_permissions.contains(perm))
  {
    bail!(
      FORBIDDEN,
      "Cannot assign permissions you do not have to a group"
    );
  }

  db.group()
    .edit_group(data.uuid, data.name, data.permissions, data.users)
    .await?;
  updater.broadcast(UpdateMessage::Groups).await;

  Ok(())
}
