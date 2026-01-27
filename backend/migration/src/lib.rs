pub use sea_orm_migration::prelude::*;

mod m20250301_215149_create_key_table;
mod m20260123_144736_invalid_jwt;
mod m20260123_144752_user;
mod m20260123_145152_node;
mod m20260126_155842_group;
mod m20260126_160754_setup;
mod m20260127_211643_settings;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
      Box::new(m20250301_215149_create_key_table::Migration),
      Box::new(m20260123_144736_invalid_jwt::Migration),
      Box::new(m20260123_144752_user::Migration),
      Box::new(m20260123_145152_node::Migration),
      Box::new(m20260126_155842_group::Migration),
      Box::new(m20260126_160754_setup::Migration),
      Box::new(m20260127_211643_settings::Migration),
    ]
  }
}
