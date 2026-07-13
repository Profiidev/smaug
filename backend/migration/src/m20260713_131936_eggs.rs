use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

const EGG_VARIABLE_EGG_ID_INDEX_NAME: &str = "egg_variable.egg_id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .create_table(
        Table::create()
          .table(Egg::Table)
          .if_not_exists()
          .col(pk_uuid(Egg::Id))
          .col(string(Egg::Name))
          .col(string(Egg::Author))
          .col(string_null(Egg::Description))
          .col(json(Egg::DockerImages))
          .col(json(Egg::FileDenylist))
          .col(json_null(Egg::Features))
          .col(string(Egg::Startup))
          .col(json(Egg::Config))
          .col(json(Egg::InstallScript))
          .col(string_null(Egg::UpdateUrl))
          .to_owned(),
      )
      .await?;

    manager
      .create_table(
        Table::create()
          .table(EggVariable::Table)
          .if_not_exists()
          .col(pk_uuid(EggVariable::Id))
          .col(uuid(EggVariable::EggId))
          .col(string(EggVariable::Name))
          .col(string(EggVariable::Description))
          .col(string(EggVariable::EnvVariable))
          .col(string(EggVariable::DefaultValue))
          .col(boolean(EggVariable::UserViewable))
          .col(boolean(EggVariable::UserEditable))
          .col(string(EggVariable::Rules))
          .col(string(EggVariable::FieldType))
          .foreign_key(
            ForeignKey::create()
              .from(EggVariable::Table, EggVariable::EggId)
              .to(Egg::Table, Egg::Id)
              .on_delete(ForeignKeyAction::Cascade)
              .on_update(ForeignKeyAction::Cascade),
          )
          .to_owned(),
      )
      .await?;

    manager
      .create_index(
        Index::create()
          .if_not_exists()
          .name(EGG_VARIABLE_EGG_ID_INDEX_NAME)
          .table(EggVariable::Table)
          .col(EggVariable::EggId)
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .drop_index(
        Index::drop()
          .name(EGG_VARIABLE_EGG_ID_INDEX_NAME)
          .to_owned(),
      )
      .await?;

    manager
      .drop_table(
        Table::drop()
          .table(EggVariable::Table)
          .table(Egg::Table)
          .to_owned(),
      )
      .await
  }
}

#[derive(DeriveIden)]
enum Egg {
  Table,
  Id,
  Name,
  Author,
  Description,
  DockerImages,
  FileDenylist,
  Features,
  Startup,
  Config,
  InstallScript,
  UpdateUrl,
}

#[derive(DeriveIden)]
enum EggVariable {
  Table,
  Id,
  EggId,
  Name,
  Description,
  EnvVariable,
  DefaultValue,
  UserViewable,
  UserEditable,
  Rules,
  FieldType,
}
