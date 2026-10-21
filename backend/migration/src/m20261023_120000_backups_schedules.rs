use sea_orm_migration::{
  prelude::{extension::postgres::Type, *},
  schema::*,
  sea_orm::DatabaseBackend,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

fn is_postgres(manager: &SchemaManager<'_>) -> bool {
  manager.get_database_backend() == DatabaseBackend::Postgres
}

fn cascade(
  from: (impl IntoIden, impl IntoIden),
  to: (impl IntoIden, impl IntoIden),
) -> ForeignKeyCreateStatement {
  ForeignKey::create()
    .from(from.0.into_iden(), from.1.into_iden())
    .to(to.0.into_iden(), to.1.into_iden())
    .on_delete(ForeignKeyAction::Cascade)
    .on_update(ForeignKeyAction::Cascade)
    .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    if is_postgres(manager) {
      manager
        .create_type(
          Type::create()
            .as_enum(BackupStatus::Enum)
            .values([
              BackupStatus::Creating,
              BackupStatus::Completed,
              BackupStatus::Failed,
              BackupStatus::Restoring,
            ])
            .to_owned(),
        )
        .await?;
      manager
        .create_type(
          Type::create()
            .as_enum(ScheduleAction::Enum)
            .values([
              ScheduleAction::Command,
              ScheduleAction::Power,
              ScheduleAction::Backup,
            ])
            .to_owned(),
        )
        .await?;
    }

    manager
      .create_table(
        Table::create()
          .table(ServerBackup::Table)
          .if_not_exists()
          .col(pk_uuid(ServerBackup::Id))
          .col(uuid(ServerBackup::ServerId))
          .col(string(ServerBackup::Name))
          .col(big_integer(ServerBackup::SizeBytes))
          .col(string_null(ServerBackup::Sha256))
          .col(boolean(ServerBackup::Locked))
          .col(custom(ServerBackup::Status, BackupStatus::Enum))
          .col(text(ServerBackup::Ignored))
          .col(text_null(ServerBackup::Error))
          .col(timestamp_with_time_zone(ServerBackup::CreatedAt))
          .foreign_key(&mut cascade(
            (ServerBackup::Table, ServerBackup::ServerId),
            (Server::Table, Server::Id),
          ))
          .to_owned(),
      )
      .await?;
    manager
      .create_table(
        Table::create()
          .table(ServerSchedule::Table)
          .if_not_exists()
          .col(pk_uuid(ServerSchedule::Id))
          .col(uuid(ServerSchedule::ServerId))
          .col(string(ServerSchedule::Name))
          .col(string(ServerSchedule::Minute))
          .col(string(ServerSchedule::Hour))
          .col(string(ServerSchedule::DayOfMonth))
          .col(string(ServerSchedule::Month))
          .col(string(ServerSchedule::DayOfWeek))
          .col(boolean(ServerSchedule::Enabled))
          .col(boolean(ServerSchedule::OnlyWhenOnline))
          .col(timestamp_with_time_zone_null(ServerSchedule::LastRunAt))
          .col(timestamp_with_time_zone(ServerSchedule::CreatedAt))
          .foreign_key(&mut cascade(
            (ServerSchedule::Table, ServerSchedule::ServerId),
            (Server::Table, Server::Id),
          ))
          .to_owned(),
      )
      .await?;
    manager
      .create_table(
        Table::create()
          .table(ServerScheduleTask::Table)
          .if_not_exists()
          .col(pk_uuid(ServerScheduleTask::Id))
          .col(uuid(ServerScheduleTask::ScheduleId))
          .col(integer(ServerScheduleTask::Position))
          .col(custom(ServerScheduleTask::Action, ScheduleAction::Enum))
          .col(text(ServerScheduleTask::Payload))
          .col(integer(ServerScheduleTask::OffsetSeconds))
          .col(boolean(ServerScheduleTask::ContinueOnFailure))
          .foreign_key(&mut cascade(
            (ServerScheduleTask::Table, ServerScheduleTask::ScheduleId),
            (ServerSchedule::Table, ServerSchedule::Id),
          ))
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    for table in [
      ServerScheduleTask::Table.into_iden(),
      ServerSchedule::Table.into_iden(),
      ServerBackup::Table.into_iden(),
    ] {
      manager
        .drop_table(Table::drop().if_exists().table(table).to_owned())
        .await?;
    }
    if is_postgres(manager) {
      manager
        .drop_type(Type::drop().name(ScheduleAction::Enum).to_owned())
        .await?;
      manager
        .drop_type(Type::drop().name(BackupStatus::Enum).to_owned())
        .await?;
    }
    Ok(())
  }
}

#[derive(DeriveIden)]
enum Server {
  Table,
  Id,
}

#[derive(DeriveIden)]
enum ServerBackup {
  Table,
  Id,
  ServerId,
  Name,
  SizeBytes,
  Sha256,
  Locked,
  Status,
  Ignored,
  Error,
  CreatedAt,
}

#[derive(DeriveIden)]
enum ServerSchedule {
  Table,
  Id,
  ServerId,
  Name,
  Minute,
  Hour,
  DayOfMonth,
  Month,
  DayOfWeek,
  Enabled,
  OnlyWhenOnline,
  LastRunAt,
  CreatedAt,
}

#[derive(DeriveIden)]
enum ServerScheduleTask {
  Table,
  Id,
  ScheduleId,
  Position,
  Action,
  Payload,
  OffsetSeconds,
  ContinueOnFailure,
}

#[derive(DeriveIden)]
enum BackupStatus {
  #[sea_orm(iden = "backup_status")]
  Enum,
  Creating,
  Completed,
  Failed,
  Restoring,
}

#[derive(DeriveIden)]
enum ScheduleAction {
  #[sea_orm(iden = "schedule_action")]
  Enum,
  Command,
  Power,
  Backup,
}
