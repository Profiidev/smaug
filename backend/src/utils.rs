use centaurus::{
  UpdateMessage,
  backend::{
    auth::permission::{self, Permission},
    endpoints::websocket,
  },
  permission,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[allow(unused)]
pub type Updater = websocket::state::Updater<UpdateMessage>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, UpdateMessage)]
#[serde(tag = "type")]
pub enum UpdateMessage {
  #[update_message(settings)]
  Settings,
  #[update_message(user)]
  User {
    uuid: Uuid,
  },
  #[update_message(user_permissions)]
  UserPermissions,
  #[update_message(group)]
  Group {
    uuid: Uuid,
  },
  Nodes {
    uuid: Uuid,
  },
}

pub fn permissions() -> Vec<&'static str> {
  let mut perms = permission::permissions();
  perms.extend_from_slice(&[NodeViewPerm::name(), NodeEditPerm::name()]);
  perms
}

permission!(NodeViewPerm, "node:view");
permission!(NodeEditPerm, "node:edit");
