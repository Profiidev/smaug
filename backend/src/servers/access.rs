use centaurus::{
  backend::auth::permission::Permission,
  bail,
  db::{init::Connection, tables::ConnectionExt},
  error::Result,
};
use entity::{sea_orm_active_enums::ServerAccessLevel, server};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::{
  db::DBTrait,
  utils::{ServerEditPerm, ServerViewPerm},
};

/// What the caller may do with a server, ordered from least to most.
#[derive(Serialize, JsonSchema, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Access {
  None,
  Read,
  Write,
  Admin,
}

/// `server:edit` is admin; owner or any write grant is write; `server:view`
/// or any (read) grant is read.
fn resolve_access(admin: bool, viewer: bool, owner: bool, grants: &[ServerAccessLevel]) -> Access {
  if admin {
    Access::Admin
  } else if owner || grants.contains(&ServerAccessLevel::Write) {
    Access::Write
  } else if viewer || !grants.is_empty() {
    Access::Read
  } else {
    Access::None
  }
}

pub(super) async fn has_perm<P: Permission>(db: &Connection, user: Uuid) -> Result<bool> {
  db.group().user_hash_permissions(user, P::name()).await
}

async fn access(db: &Connection, user: Uuid, server: &server::Model) -> Result<Access> {
  if has_perm::<ServerEditPerm>(db, user).await? {
    return Ok(Access::Admin);
  }
  let viewer = has_perm::<ServerViewPerm>(db, user).await?;
  let grants: Vec<_> = db
    .server()
    .grants_for_user(user, Some(server.id))
    .await?
    .into_iter()
    .map(|(_, level)| level)
    .collect();
  Ok(resolve_access(
    false,
    viewer,
    server.owner_id == Some(user),
    &grants,
  ))
}

/// 404 without any access (no existence leak), 403 below `need`, 409 for a
/// change to a suspended server by a non-admin.
fn check(access: Access, need: Access, suspended: bool) -> Result<()> {
  if access == Access::None {
    bail!(NOT_FOUND, "Server not found");
  }
  if access < need {
    bail!(FORBIDDEN, "insufficient permissions");
  }
  if suspended && need >= Access::Write && access < Access::Admin {
    bail!(CONFLICT, "The server is suspended");
  }
  Ok(())
}

pub(super) async fn load(
  db: &Connection,
  user: Uuid,
  uuid: Uuid,
  need: Access,
) -> Result<(server::Model, Access)> {
  let server = db.server().find_by_id(uuid).await?;
  let access = access(db, user, &server).await?;
  check(access, need, server.suspended)?;
  Ok((server, access))
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn access_resolution() {
    use Access::*;
    use ServerAccessLevel::{Read as R, Write as W};
    let cases: &[(bool, bool, bool, &[ServerAccessLevel], Access)] = &[
      (false, false, false, &[], None),
      (true, false, false, &[], Admin),
      (true, true, true, &[R, W], Admin),
      (false, true, false, &[], Read),
      (false, false, false, &[R], Read),
      (false, false, false, &[R, R], Read),
      (false, false, true, &[], Write),
      (false, true, true, &[R], Write),
      (false, false, false, &[W], Write),
      (false, false, false, &[R, W], Write),
      (false, true, false, &[W], Write),
    ];
    for (admin, viewer, owner, grants, expected) in cases {
      assert_eq!(
        resolve_access(*admin, *viewer, *owner, grants),
        *expected,
        "{admin} {viewer} {owner} {grants:?}"
      );
    }
    assert!(None < Read && Read < Write && Write < Admin);
  }

  #[test]
  fn suspended_is_read_only_below_admin() {
    use Access::*;
    assert!(check(Write, Write, false).is_ok());
    assert!(check(Write, Write, true).is_err());
    assert!(check(Admin, Write, true).is_ok());
    assert!(check(Admin, Admin, true).is_ok());
    assert!(check(Read, Read, true).is_ok());
    assert!(check(Write, Read, true).is_ok());
    assert!(check(Read, Write, false).is_err());
    assert!(check(None, Read, false).is_err());
  }
}
