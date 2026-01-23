pub use sea_orm_migration::prelude::*;

mod m20250301_215149_create_key_table;
mod m20260123_144736_invalid_jwt;
mod m20260123_144752_user;
mod m20260123_145152_node;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
            Box::new(m20250301_215149_create_key_table::Migration),
            Box::new(m20260123_144736_invalid_jwt::Migration),
            Box::new(m20260123_144752_user::Migration),
            Box::new(m20260123_145152_node::Migration),
        ]
  }
}
