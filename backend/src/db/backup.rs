use centaurus::error::ErrorReportStatusExt;
use entity::{sea_orm_active_enums::BackupStatus, server_backup};
use http::StatusCode;
use sea_orm::{ActiveValue::Set, QueryOrder, prelude::*};

pub struct BackupTable<'db> {
  db: &'db DatabaseConnection,
}

impl<'db> BackupTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  /// Newest first.
  pub async fn list(&self, server: Uuid) -> Result<Vec<server_backup::Model>, DbErr> {
    server_backup::Entity::find()
      .filter(server_backup::Column::ServerId.eq(server))
      .order_by_desc(server_backup::Column::CreatedAt)
      .all(self.db)
      .await
  }

  /// A backup of `server`, `404` for any other.
  pub async fn find(
    &self,
    server: Uuid,
    id: Uuid,
  ) -> centaurus::error::Result<server_backup::Model> {
    server_backup::Entity::find_by_id(id)
      .filter(server_backup::Column::ServerId.eq(server))
      .one(self.db)
      .await?
      .status_context(StatusCode::NOT_FOUND, "Backup not found")
  }

  pub async fn insert(&self, model: server_backup::Model) -> Result<server_backup::Model, DbErr> {
    server_backup::ActiveModel::from(model)
      .insert(self.db)
      .await
  }

  pub async fn set_locked(&self, id: Uuid, locked: bool) -> Result<(), DbErr> {
    self.update(id, |m| m.locked = Set(locked)).await
  }

  pub async fn set_status(&self, id: Uuid, status: BackupStatus) -> Result<(), DbErr> {
    self.update(id, |m| m.status = Set(status)).await
  }

  pub async fn complete(&self, id: Uuid, size: u64, sha256: String) -> Result<(), DbErr> {
    self
      .update(id, |m| {
        m.status = Set(BackupStatus::Completed);
        m.size_bytes = Set(size as i64);
        m.sha256 = Set(Some(sha256));
      })
      .await
  }

  pub async fn fail(&self, id: Uuid, error: String) -> Result<(), DbErr> {
    self
      .update(id, |m| {
        m.status = Set(BackupStatus::Failed);
        m.error = Set(Some(error));
      })
      .await
  }

  /// Completed -> restoring, `false` when it is not completed (anymore).
  pub async fn start_restore(&self, id: Uuid) -> Result<bool, DbErr> {
    let res = server_backup::Entity::update_many()
      .col_expr(
        server_backup::Column::Status,
        BackupStatus::Restoring.as_enum(),
      )
      .filter(server_backup::Column::Id.eq(id))
      .filter(server_backup::Column::Status.eq(BackupStatus::Completed))
      .exec(self.db)
      .await?;
    Ok(res.rows_affected == 1)
  }

  /// Deletes unless locked, creating or restoring, `false` then.
  pub async fn delete_idle(&self, id: Uuid) -> Result<bool, DbErr> {
    let res = server_backup::Entity::delete_many()
      .filter(server_backup::Column::Id.eq(id))
      .filter(server_backup::Column::Locked.eq(false))
      .filter(server_backup::Column::Status.is_in([BackupStatus::Completed, BackupStatus::Failed]))
      .exec(self.db)
      .await?;
    Ok(res.rows_affected == 1)
  }

  pub async fn delete(&self, id: Uuid) -> Result<(), DbErr> {
    server_backup::Entity::delete_by_id(id)
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// After a panel restart: unfinished creations failed, restores ended.
  pub async fn reset_unfinished(&self) -> Result<(), DbErr> {
    server_backup::Entity::update_many()
      .col_expr(
        server_backup::Column::Status,
        BackupStatus::Failed.as_enum(),
      )
      .col_expr(
        server_backup::Column::Error,
        Expr::value("Interrupted by a panel restart"),
      )
      .filter(server_backup::Column::Status.eq(BackupStatus::Creating))
      .exec(self.db)
      .await?;
    server_backup::Entity::update_many()
      .col_expr(
        server_backup::Column::Status,
        BackupStatus::Completed.as_enum(),
      )
      .filter(server_backup::Column::Status.eq(BackupStatus::Restoring))
      .exec(self.db)
      .await?;
    Ok(())
  }

  async fn update(
    &self,
    id: Uuid,
    change: impl FnOnce(&mut server_backup::ActiveModel),
  ) -> Result<(), DbErr> {
    let mut model = server_backup::ActiveModel {
      id: Set(id),
      ..Default::default()
    };
    change(&mut model);
    model.update(self.db).await?;
    Ok(())
  }
}

#[cfg(test)]
mod test {
  use chrono::{Duration, Utc};
  use entity::{sea_orm_active_enums::BackupStatus, server_backup};
  use http::StatusCode;
  use uuid::Uuid;

  use crate::db::{DBTrait, sqlite_db};

  fn backup(server: Uuid, status: BackupStatus, age: i64) -> server_backup::Model {
    server_backup::Model {
      id: Uuid::now_v7(),
      server_id: server,
      name: "b".into(),
      size_bytes: 0,
      sha256: None,
      locked: false,
      status,
      ignored: String::new(),
      error: None,
      created_at: (Utc::now() - Duration::minutes(age)).into(),
    }
  }

  #[tokio::test]
  async fn backups_are_scoped_ordered_and_reset() {
    let db = sqlite_db().await;
    let (server, other) = (Uuid::now_v7(), Uuid::now_v7());
    let creating = db
      .backup()
      .insert(backup(server, BackupStatus::Creating, 2))
      .await
      .unwrap();
    let restoring = db
      .backup()
      .insert(backup(server, BackupStatus::Restoring, 1))
      .await
      .unwrap();
    let foreign = db
      .backup()
      .insert(backup(other, BackupStatus::Completed, 0))
      .await
      .unwrap();

    let ids: Vec<_> = db
      .backup()
      .list(server)
      .await
      .unwrap()
      .into_iter()
      .map(|b| b.id)
      .collect();
    assert_eq!(ids, vec![restoring.id, creating.id]);
    assert_eq!(
      db.backup()
        .find(server, foreign.id)
        .await
        .unwrap_err()
        .status,
      StatusCode::NOT_FOUND
    );

    db.backup().reset_unfinished().await.unwrap();
    let c = db.backup().find(server, creating.id).await.unwrap();
    assert_eq!(c.status, BackupStatus::Failed);
    assert!(c.error.is_some());
    let r = db.backup().find(server, restoring.id).await.unwrap();
    assert_eq!(r.status, BackupStatus::Completed);
    assert!(r.error.is_none());

    db.backup().complete(c.id, 9, "ab".into()).await.unwrap();
    let c = db.backup().find(server, c.id).await.unwrap();
    assert_eq!(
      (c.status, c.size_bytes, c.sha256.as_deref()),
      (BackupStatus::Completed, 9, Some("ab"))
    );
    // only completed ones restore, once
    assert!(db.backup().start_restore(c.id).await.unwrap());
    assert!(!db.backup().start_restore(c.id).await.unwrap());
    assert!(!db.backup().delete_idle(c.id).await.unwrap());
    db.backup()
      .set_status(c.id, BackupStatus::Completed)
      .await
      .unwrap();
    db.backup().set_locked(c.id, true).await.unwrap();
    assert!(!db.backup().delete_idle(c.id).await.unwrap());
    db.backup().set_locked(c.id, false).await.unwrap();
    assert!(db.backup().delete_idle(c.id).await.unwrap());
    assert!(!db.backup().delete_idle(c.id).await.unwrap());
    db.backup().delete(c.id).await.unwrap();
    // idempotent
    db.backup().delete(c.id).await.unwrap();
    assert_eq!(db.backup().list(server).await.unwrap().len(), 1);
  }
}
