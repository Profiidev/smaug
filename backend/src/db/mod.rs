use centaurus::db::init::Connection;

pub mod node;

#[allow(unused)]
pub trait DBTrait {
  fn node(&self) -> node::NodeTable<'_>;
}

impl DBTrait for Connection {
  fn node(&self) -> node::NodeTable<'_> {
    node::NodeTable::new(&self.0)
  }
}
