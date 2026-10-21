use centaurus::error::ErrorReportStatusExt;
use entity::{server_schedule, server_schedule_task};
use http::StatusCode;
use sea_orm::{ActiveValue::Set, IntoActiveModel, QueryOrder, QuerySelect, prelude::*};

pub struct ScheduleTable<'db> {
  db: &'db DatabaseConnection,
}

impl<'db> ScheduleTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  /// Oldest first.
  pub async fn list(&self, server: Uuid) -> Result<Vec<server_schedule::Model>, DbErr> {
    server_schedule::Entity::find()
      .filter(server_schedule::Column::ServerId.eq(server))
      .order_by_asc(server_schedule::Column::CreatedAt)
      .all(self.db)
      .await
  }

  pub async fn list_enabled(&self) -> Result<Vec<server_schedule::Model>, DbErr> {
    server_schedule::Entity::find()
      .filter(server_schedule::Column::Enabled.eq(true))
      .all(self.db)
      .await
  }

  /// A schedule of `server`, `404` for any other.
  pub async fn find(
    &self,
    server: Uuid,
    id: Uuid,
  ) -> centaurus::error::Result<server_schedule::Model> {
    server_schedule::Entity::find_by_id(id)
      .filter(server_schedule::Column::ServerId.eq(server))
      .one(self.db)
      .await?
      .status_context(StatusCode::NOT_FOUND, "Schedule not found")
  }

  pub async fn insert(
    &self,
    model: server_schedule::Model,
  ) -> Result<server_schedule::Model, DbErr> {
    server_schedule::ActiveModel::from(model)
      .insert(self.db)
      .await
  }

  /// Saves only the `Set` fields, e.g. a concurrent `last_run_at` survives.
  pub async fn update(
    &self,
    model: server_schedule::ActiveModel,
  ) -> Result<server_schedule::Model, DbErr> {
    model.update(self.db).await
  }

  pub async fn set_last_run(&self, id: Uuid, at: DateTimeWithTimeZone) -> Result<(), DbErr> {
    server_schedule::ActiveModel {
      id: Set(id),
      last_run_at: Set(Some(at)),
      ..Default::default()
    }
    .update(self.db)
    .await?;
    Ok(())
  }

  pub async fn delete(&self, id: Uuid) -> Result<(), DbErr> {
    server_schedule::Entity::delete_by_id(id)
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Tasks of the given schedules in run order.
  pub async fn tasks(
    &self,
    schedules: Vec<Uuid>,
  ) -> Result<Vec<server_schedule_task::Model>, DbErr> {
    server_schedule_task::Entity::find()
      .filter(server_schedule_task::Column::ScheduleId.is_in(schedules))
      .order_by_asc(server_schedule_task::Column::Position)
      .order_by_asc(server_schedule_task::Column::Id)
      .all(self.db)
      .await
  }

  pub async fn find_task(
    &self,
    schedule: Uuid,
    id: Uuid,
  ) -> centaurus::error::Result<server_schedule_task::Model> {
    server_schedule_task::Entity::find_by_id(id)
      .filter(server_schedule_task::Column::ScheduleId.eq(schedule))
      .one(self.db)
      .await?
      .status_context(StatusCode::NOT_FOUND, "Task not found")
  }

  /// Appended after the last task, `position` is ignored.
  pub async fn insert_task(
    &self,
    mut model: server_schedule_task::Model,
  ) -> Result<server_schedule_task::Model, DbErr> {
    let last: Option<Option<i32>> = server_schedule_task::Entity::find()
      .select_only()
      .column_as(server_schedule_task::Column::Position.max(), "max")
      .filter(server_schedule_task::Column::ScheduleId.eq(model.schedule_id))
      .into_tuple()
      .one(self.db)
      .await?;
    model.position = last.flatten().map_or(0, |p| p + 1);
    server_schedule_task::ActiveModel::from(model)
      .insert(self.db)
      .await
  }

  /// Saves every field.
  pub async fn update_task(&self, model: server_schedule_task::Model) -> Result<(), DbErr> {
    model
      .into_active_model()
      .reset_all()
      .update(self.db)
      .await?;
    Ok(())
  }

  pub async fn delete_task(&self, id: Uuid) -> Result<(), DbErr> {
    server_schedule_task::Entity::delete_by_id(id)
      .exec(self.db)
      .await?;
    Ok(())
  }
}

#[cfg(test)]
mod test {
  use chrono::Utc;
  use entity::{sea_orm_active_enums::ScheduleAction, server_schedule, server_schedule_task};
  use http::StatusCode;
  use sea_orm::{ActiveValue::Set, IntoActiveModel};
  use uuid::Uuid;

  use crate::db::{DBTrait, sqlite_db};

  fn task(schedule: Uuid) -> server_schedule_task::Model {
    server_schedule_task::Model {
      id: Uuid::now_v7(),
      schedule_id: schedule,
      position: 99,
      action: ScheduleAction::Command,
      payload: "x".into(),
      offset_seconds: 0,
      continue_on_failure: false,
    }
  }

  #[tokio::test]
  async fn tasks_append_in_order_and_cascade_scoped() {
    let db = sqlite_db().await;
    let server = Uuid::now_v7();
    let s = db
      .schedule()
      .insert(server_schedule::Model {
        id: Uuid::now_v7(),
        server_id: server,
        name: "s".into(),
        minute: "*".into(),
        hour: "*".into(),
        day_of_month: "*".into(),
        month: "*".into(),
        day_of_week: "*".into(),
        enabled: false,
        only_when_online: false,
        last_run_at: None,
        created_at: Utc::now().into(),
      })
      .await
      .unwrap();
    assert!(db.schedule().list_enabled().await.unwrap().is_empty());
    assert_eq!(
      db.schedule()
        .find(Uuid::now_v7(), s.id)
        .await
        .unwrap_err()
        .status,
      StatusCode::NOT_FOUND
    );

    let a = db.schedule().insert_task(task(s.id)).await.unwrap();
    let b = db.schedule().insert_task(task(s.id)).await.unwrap();
    assert_eq!((a.position, b.position), (0, 1));
    db.schedule().delete_task(a.id).await.unwrap();
    let c = db.schedule().insert_task(task(s.id)).await.unwrap();
    assert_eq!(c.position, 2);
    let ids: Vec<_> = db
      .schedule()
      .tasks(vec![s.id])
      .await
      .unwrap()
      .into_iter()
      .map(|t| t.id)
      .collect();
    assert_eq!(ids, vec![b.id, c.id]);
    assert!(db.schedule().tasks(vec![]).await.unwrap().is_empty());
    assert_eq!(
      db.schedule()
        .find_task(Uuid::now_v7(), b.id)
        .await
        .unwrap_err()
        .status,
      StatusCode::NOT_FOUND
    );

    // a run starts between loading and saving an edit
    let mut edit = s.clone().into_active_model();
    let now = Utc::now();
    db.schedule().set_last_run(s.id, now.into()).await.unwrap();
    edit.enabled = Set(true);
    let got = db.schedule().update(edit).await.unwrap();
    assert!(got.enabled);
    assert_eq!(db.schedule().list_enabled().await.unwrap().len(), 1);
    let got = db.schedule().find(server, s.id).await.unwrap();
    assert_eq!(got.last_run_at.unwrap().timestamp(), now.timestamp());
    assert_eq!(got.name, "s");
  }
}
