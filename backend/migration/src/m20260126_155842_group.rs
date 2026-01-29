use sea_orm_migration::{prelude::*, schema::*};

use crate::m20260123_144752_user::User;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Group::Table)
          .if_not_exists()
          .col(pk_uuid(Group::Id))
          .col(string(Group::Name))
          .to_owned(),
      )
      .await?;

    manager
      .create_table(
        Table::create()
          .table(GroupUser::Table)
          .if_not_exists()
          .primary_key(
            Index::create()
              .table(GroupUser::Table)
              .col(GroupUser::GroupId)
              .col(GroupUser::UserId),
          )
          .col(uuid(GroupUser::GroupId))
          .col(uuid(GroupUser::UserId))
          .foreign_key(
            ForeignKey::create()
              .from(GroupUser::Table, GroupUser::GroupId)
              .to(Group::Table, Group::Id)
              .on_delete(ForeignKeyAction::Cascade)
              .on_update(ForeignKeyAction::Cascade),
          )
          .foreign_key(
            ForeignKey::create()
              .from(GroupUser::Table, GroupUser::UserId)
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
          .table(GroupPermission::Table)
          .if_not_exists()
          .primary_key(
            Index::create()
              .table(GroupPermission::Table)
              .col(GroupPermission::GroupId)
              .col(GroupPermission::Permission),
          )
          .col(uuid(GroupPermission::GroupId))
          .col(string(GroupPermission::Permission))
          .foreign_key(
            ForeignKey::create()
              .from(GroupPermission::Table, GroupPermission::GroupId)
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
      .drop_table(
        Table::drop()
          .table(Group::Table)
          .table(GroupUser::Table)
          .table(GroupPermission::Table)
          .to_owned(),
      )
      .await
  }
}

#[derive(DeriveIden)]
enum Group {
  Table,
  Id,
  Name,
}

#[derive(DeriveIden)]
enum GroupUser {
  Table,
  GroupId,
  UserId,
}

#[derive(DeriveIden)]
enum GroupPermission {
  Table,
  GroupId,
  Permission,
}
