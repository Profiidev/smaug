use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

// one column per statement, SQLite can only add one at a time
#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Server::Table)
          .add_column(integer(Server::StopTimeout).default(60))
          .to_owned(),
      )
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(Server::Table)
          .add_column(boolean(Server::RestartOnCrash).default(true))
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Server::Table)
          .drop_column(Server::RestartOnCrash)
          .to_owned(),
      )
      .await?;
    manager
      .alter_table(
        Table::alter()
          .table(Server::Table)
          .drop_column(Server::StopTimeout)
          .to_owned(),
      )
      .await
  }
}

#[derive(DeriveIden)]
enum Server {
  Table,
  StopTimeout,
  RestartOnCrash,
}
