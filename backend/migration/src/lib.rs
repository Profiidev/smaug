pub use sea_orm_migration::prelude::*;

pub struct Migrator;

mod m20260123_145152_node;
mod m20260713_131936_eggs;
mod m20261021_120000_servers;
mod m20261021_140000_server_stop;
mod m20261021_150000_server_install_generation;
mod m20261022_120000_server_power;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
      Box::new(centaurus::db::migrations::m0_key::Migration),
      Box::new(centaurus::db::migrations::m1_invalid_jwt::Migration),
      Box::new(centaurus::db::migrations::m2_settings::Migration),
      Box::new(centaurus::db::migrations::m3_user::Migration),
      Box::new(centaurus::db::migrations::m4_groups::Migration),
      Box::new(centaurus::db::migrations::m5_setup::Migration),
      Box::new(centaurus::db::migrations::m6_user_oidc_subject::Migration),
      Box::new(m20260123_145152_node::Migration),
      Box::new(m20260713_131936_eggs::Migration),
      Box::new(m20261021_120000_servers::Migration),
      Box::new(m20261021_140000_server_stop::Migration),
      Box::new(m20261021_150000_server_install_generation::Migration),
      Box::new(m20261022_120000_server_power::Migration),
    ]
  }
}
