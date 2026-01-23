use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Node::Table)
          .if_not_exists()
          .col(pk_uuid(Node::Id))
          .col(string(Node::Name))
          .col(string(Node::Address))
          .col(tiny_unsigned(Node::Port))
          .col(boolean(Node::Secure))
          .col(unsigned_null(Node::DiskLimitMb))
          .col(unsigned_null(Node::MemoryLimitMb))
          .col(unsigned_null(Node::CpuLimit))
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_table(Table::drop().table(Node::Table).to_owned())
      .await
  }
}

#[derive(DeriveIden)]
enum Node {
  Table,
  Id,
  Name,
  Address,
  Port,
  Secure,
  DiskLimitMb,
  MemoryLimitMb,
  CpuLimit,
}
