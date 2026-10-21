use centaurus::db::migrations::{m3_user::User, m4_groups::Group};
use sea_orm_migration::{
  prelude::{extension::postgres::Type, *},
  schema::*,
  sea_orm::DatabaseBackend,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const ALLOCATION_INDEX_NAME: &str = "server_allocation_node_id_ip_port";

fn database_backend(manager: &SchemaManager<'_>) -> DatabaseBackend {
  match manager.get_connection() {
    SchemaManagerConnection::Connection(conn) => conn.get_database_backend(),
    SchemaManagerConnection::Transaction(trans) => trans.get_database_backend(),
    SchemaManagerConnection::OwnedTransaction(trans) => trans.get_database_backend(),
  }
}

/// `table.server_id` -> `server.id`, cascading.
fn server_fk(table: impl IntoIden, col: impl IntoIden) -> ForeignKeyCreateStatement {
  ForeignKey::create()
    .from(table.into_iden(), col.into_iden())
    .to(Server::Table, Server::Id)
    .on_delete(ForeignKeyAction::Cascade)
    .on_update(ForeignKeyAction::Cascade)
    .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    if database_backend(manager) == DatabaseBackend::Postgres {
      manager
        .create_type(
          Type::create()
            .as_enum(ServerInstallState::Enum)
            .values([
              ServerInstallState::NotInstalled,
              ServerInstallState::Installing,
              ServerInstallState::InstallFailed,
              ServerInstallState::Installed,
            ])
            .to_owned(),
        )
        .await?;
      manager
        .create_type(
          Type::create()
            .as_enum(ServerAccessLevel::Enum)
            .values([ServerAccessLevel::Read, ServerAccessLevel::Write])
            .to_owned(),
        )
        .await?;
    }

    manager
      .create_table(
        Table::create()
          .table(Server::Table)
          .if_not_exists()
          .col(pk_uuid(Server::Id))
          .col(string_len(Server::Name, 191))
          .col(uuid_null(Server::OwnerId))
          .col(uuid(Server::NodeId))
          .col(uuid(Server::EggId))
          .col(text(Server::Startup))
          .col(boolean(Server::StartupOverridden))
          .col(text(Server::Image))
          .col(custom(Server::InstallState, ServerInstallState::Enum))
          .col(boolean(Server::Suspended))
          .col(boolean(Server::StartOnCompletion))
          .col(integer(Server::CpuLimit))
          .col(big_integer(Server::MemoryMb))
          .col(big_integer(Server::DiskMb))
          .col(integer(Server::BackupLimit))
          .col(timestamp_with_time_zone(Server::CreatedAt))
          .foreign_key(
            ForeignKey::create()
              .from(Server::Table, Server::OwnerId)
              .to(User::Table, User::Id)
              .on_delete(ForeignKeyAction::SetNull)
              .on_update(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .from(Server::Table, Server::NodeId)
              .to(Node::Table, Node::Id)
              .on_delete(ForeignKeyAction::Restrict)
              .on_update(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .from(Server::Table, Server::EggId)
              .to(Egg::Table, Egg::Id)
              .on_delete(ForeignKeyAction::Restrict)
              .on_update(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    manager
      .create_table(
        Table::create()
          .table(ServerAllocation::Table)
          .if_not_exists()
          .col(pk_uuid(ServerAllocation::Id))
          .col(uuid(ServerAllocation::ServerId))
          // copy of server.node_id so the unique index can cover it
          .col(uuid(ServerAllocation::NodeId))
          .col(string(ServerAllocation::Ip))
          .col(integer(ServerAllocation::Port))
          .col(small_integer(ServerAllocation::Position))
          .foreign_key(&mut server_fk(ServerAllocation::Table, ServerAllocation::ServerId))
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .name(ALLOCATION_INDEX_NAME)
          .table(ServerAllocation::Table)
          .col(ServerAllocation::NodeId)
          .col(ServerAllocation::Ip)
          .col(ServerAllocation::Port)
          .unique()
          .to_owned(),
      )
      .await?;

    manager
      .create_table(
        Table::create()
          .table(ServerVariable::Table)
          .if_not_exists()
          .col(uuid(ServerVariable::ServerId))
          .col(string(ServerVariable::EnvVariable))
          .col(text(ServerVariable::Value))
          .primary_key(
            Index::create()
              .col(ServerVariable::ServerId)
              .col(ServerVariable::EnvVariable),
          )
          .foreign_key(&mut server_fk(ServerVariable::Table, ServerVariable::ServerId))
          .to_owned(),
      )
      .await?;

    manager
      .create_table(
        Table::create()
          .table(ServerUserAccess::Table)
          .if_not_exists()
          .col(uuid(ServerUserAccess::ServerId))
          .col(uuid(ServerUserAccess::UserId))
          .col(custom(ServerUserAccess::Access, ServerAccessLevel::Enum))
          .primary_key(
            Index::create()
              .col(ServerUserAccess::ServerId)
              .col(ServerUserAccess::UserId),
          )
          .foreign_key(&mut server_fk(ServerUserAccess::Table, ServerUserAccess::ServerId))
          .foreign_key(
            ForeignKey::create()
              .from(ServerUserAccess::Table, ServerUserAccess::UserId)
              .to(User::Table, User::Id)
              .on_delete(ForeignKeyAction::Cascade)
              .on_update(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    manager
      .create_table(
        Table::create()
          .table(ServerGroupAccess::Table)
          .if_not_exists()
          .col(uuid(ServerGroupAccess::ServerId))
          .col(uuid(ServerGroupAccess::GroupId))
          .col(custom(ServerGroupAccess::Access, ServerAccessLevel::Enum))
          .primary_key(
            Index::create()
              .col(ServerGroupAccess::ServerId)
              .col(ServerGroupAccess::GroupId),
          )
          .foreign_key(&mut server_fk(ServerGroupAccess::Table, ServerGroupAccess::ServerId))
          .foreign_key(
            ForeignKey::create()
              .from(ServerGroupAccess::Table, ServerGroupAccess::GroupId)
              .to(Group::Table, Group::Id)
              .on_delete(ForeignKeyAction::Cascade)
              .on_update(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_index(
        Index::drop()
          .if_exists()
          .name(ALLOCATION_INDEX_NAME)
          .to_owned(),
      )
      .await?;

    for table in [
      ServerGroupAccess::Table.into_iden(),
      ServerUserAccess::Table.into_iden(),
      ServerVariable::Table.into_iden(),
      ServerAllocation::Table.into_iden(),
      Server::Table.into_iden(),
    ] {
      manager
        .drop_table(Table::drop().if_exists().table(table).to_owned())
        .await?;
    }

    if database_backend(manager) == DatabaseBackend::Postgres {
      manager
        .drop_type(Type::drop().name(ServerAccessLevel::Enum).to_owned())
        .await?;
      manager
        .drop_type(Type::drop().name(ServerInstallState::Enum).to_owned())
        .await?;
    }
    Ok(())
  }
}

#[derive(DeriveIden)]
enum Node {
  Table,
  Id,
}

#[derive(DeriveIden)]
enum Egg {
  Table,
  Id,
}

#[derive(DeriveIden)]
enum Server {
  Table,
  Id,
  Name,
  OwnerId,
  NodeId,
  EggId,
  Startup,
  StartupOverridden,
  Image,
  InstallState,
  Suspended,
  StartOnCompletion,
  CpuLimit,
  MemoryMb,
  DiskMb,
  BackupLimit,
  CreatedAt,
}

#[derive(DeriveIden)]
enum ServerAllocation {
  Table,
  Id,
  ServerId,
  NodeId,
  Ip,
  Port,
  Position,
}

#[derive(DeriveIden)]
enum ServerVariable {
  Table,
  ServerId,
  EnvVariable,
  Value,
}

#[derive(DeriveIden)]
enum ServerUserAccess {
  Table,
  ServerId,
  UserId,
  Access,
}

#[derive(DeriveIden)]
enum ServerGroupAccess {
  Table,
  ServerId,
  GroupId,
  Access,
}

#[derive(DeriveIden)]
enum ServerInstallState {
  #[sea_orm(iden = "server_install_state")]
  Enum,
  NotInstalled,
  Installing,
  InstallFailed,
  Installed,
}

#[derive(DeriveIden)]
enum ServerAccessLevel {
  #[sea_orm(iden = "server_access_level")]
  Enum,
  Read,
  Write,
}
