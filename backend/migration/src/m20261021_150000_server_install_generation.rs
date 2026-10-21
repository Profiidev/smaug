use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Server::Table)
          .add_column(big_integer(Server::InstallGeneration).default(0))
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Server::Table)
          .drop_column(Server::InstallGeneration)
          .to_owned(),
      )
      .await
  }
}

#[derive(DeriveIden)]
enum Server {
  Table,
  InstallGeneration,
}
