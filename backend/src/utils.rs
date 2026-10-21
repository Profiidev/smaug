use centaurus::{
  UpdateMessage,
  backend::{
    auth::permission::{self, Permission},
    endpoints::websocket,
  },
  permission,
};
use reqwest::Client;
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
  Eggs {
    uuid: Uuid,
  },
  Servers {
    uuid: Uuid,
  },
}

pub fn permissions() -> Vec<&'static str> {
  let mut perms = permission::permissions();
  perms.extend_from_slice(&[
    NodeViewPerm::name(),
    NodeEditPerm::name(),
    EggViewPerm::name(),
    EggEditPerm::name(),
    ServerViewPerm::name(),
    ServerEditPerm::name(),
  ]);
  perms
}

permission!(NodeViewPerm, "node:view");
permission!(NodeEditPerm, "node:edit");
permission!(EggViewPerm, "egg:view");
permission!(EggEditPerm, "egg:edit");
permission!(ServerViewPerm, "server:view");
permission!(ServerEditPerm, "server:edit");

pub fn client() -> Client {
  Client::builder()
    .user_agent(format!("Smaug v{}", env!("CARGO_PKG_VERSION")))
    .build()
    .expect("Failed to build HTTP client")
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn permissions_include_app_and_centaurus_ones() {
    let perms = permissions();
    for p in [
      "node:view",
      "node:edit",
      "egg:view",
      "egg:edit",
      "server:view",
      "server:edit",
    ] {
      assert!(perms.contains(&p), "missing {p}");
    }
    let base = permission::permissions();
    assert!(base.iter().all(|p| perms.contains(p)));
    assert_eq!(perms.len(), base.len() + 6);
  }
}
