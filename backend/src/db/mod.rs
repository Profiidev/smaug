use centaurus::db::init::Connection;

use crate::db::invalid_jwt::InvalidJwtTable;
use crate::db::key::KeyTable;
use crate::db::node::NodeTable;

#[allow(unused)]
pub mod invalid_jwt;
#[allow(unused)]
pub mod key;
pub mod node;

pub trait DBTrait {
  #[allow(unused)]
  fn key(&self) -> KeyTable<'_>;
  #[allow(unused)]
  fn invalid_jwt(&self) -> InvalidJwtTable<'_>;
  fn node(&self) -> NodeTable<'_>;
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
}
