use centaurus::db::init::Connection;

use crate::db::group::GroupTable;
use crate::db::invalid_jwt::InvalidJwtTable;
use crate::db::key::KeyTable;
use crate::db::node::NodeTable;
use crate::db::setup::SetupTable;
use crate::db::user::UserTable;

pub mod group;
pub mod invalid_jwt;
pub mod key;
pub mod node;
pub mod setup;
pub mod user;

pub trait DBTrait {
  fn key(&self) -> KeyTable<'_>;
  fn invalid_jwt(&self) -> InvalidJwtTable<'_>;
  fn node(&self) -> NodeTable<'_>;
  fn setup(&self) -> SetupTable<'_>;
  fn group(&self) -> GroupTable<'_>;
  fn user(&self) -> UserTable<'_>;
}

impl DBTrait for Connection {
  fn key(&self) -> KeyTable<'_> {
    KeyTable::new(self)
  }

  fn invalid_jwt(&self) -> InvalidJwtTable<'_> {
    InvalidJwtTable::new(self)
  }

  fn node(&self) -> NodeTable<'_> {
    NodeTable::new(self)
  }

  fn setup(&self) -> SetupTable<'_> {
    SetupTable::new(self)
  }

  fn group(&self) -> GroupTable<'_> {
    GroupTable::new(self)
  }

  fn user(&self) -> UserTable<'_> {
    UserTable::new(self)
  }
}
