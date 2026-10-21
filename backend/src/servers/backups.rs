//! Backups: archives wings streams out of the server root, kept in the
//! panel's file storage under `backups/{server}/{backup}.tar.zst`.

use std::{panic::AssertUnwindSafe, time::Duration};

use aide::axum::{
  ApiRouter,
  routing::{delete_with, get_with, post_with},
};
use axum::{
  Json,
  body::{Body, Bytes},
  extract::Path,
  response::Response,
};
use centaurus::{
  bail,
  db::init::Connection,
  error::Result,
  eyre::{Context, eyre},
  storage::{FileStorage, StoragePath},
};
use chrono::{DateTime, FixedOffset, Utc};
use entity::{sea_orm_active_enums::BackupStatus, server, server_backup};
use futures_util::{FutureExt, StreamExt, TryStreamExt};
use http::{Method, header};
use object_store::{ObjectStoreExt, WriteMultipart};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shared::msg::{
  CompleteStream, OpenInStream, OpenOutStream, STREAM_END, STREAM_SHA256, STREAM_SIZE,
  StreamOpened, StreamSource, StreamTarget,
};
use tokio::{spawn, sync::Mutex, time::timeout};
use tracing::warn;
use uuid::Uuid;

use super::{
  access::{Access, load},
  files::attachment,
  live::LiveFrame,
  types::ServerUuid,
};
use crate::{
  auth::jwt_auth::JwtAuth,
  db::DBTrait,
  nodes::{Wings, call},
  utils::{UpdateMessage, Updater},
};

/// Parts uploaded to the storage at once while a backup is written.
const UPLOAD_CONCURRENCY: usize = 4;
/// Longest wait for wings to finish extracting a restore.
// ponytail: an OpFinished lost to a disconnect fails the restore after this,
// ask wings for the stream status on reconnect if that gets annoying
const RESTORE_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const NAME_MAX: usize = 191;
/// Serializes the backup limit check with the insert.
// ponytail: one lock for all servers in one panel process, it is only held
// for two queries; per-server or DB locking once replicas exist
static CREATE: Mutex<()> = Mutex::const_new(());

/// Runs `work` in the background, then `on_panic` if it panicked, so no
/// backup stays creating or restoring.
fn spawn_guarded(
  work: impl Future<Output = ()> + Send + 'static,
  on_panic: impl Future<Output = ()> + Send + 'static,
) {
  spawn(async move {
    if AssertUnwindSafe(work).catch_unwind().await.is_err() {
      on_panic.await;
    }
  });
}

pub(super) fn router() -> ApiRouter {
  ApiRouter::new()
    .api_route(
      "/",
      get_with(list_backups, |op| op.id("list_backups"))
        .post_with(create_backup, |op| op.id("create_backup")),
    )
    .api_route(
      "/{backup}",
      delete_with(delete_backup, |op| op.id("delete_backup")),
    )
    .api_route(
      "/{backup}/lock",
      post_with(lock_backup, |op| op.id("lock_backup")),
    )
    .api_route(
      "/{backup}/restore",
      post_with(restore_backup, |op| op.id("restore_backup")),
    )
    .api_route(
      "/{backup}/download",
      get_with(download_backup, |op| {
        op.id("download_backup").response::<200, Bytes>()
      }),
    )
}

/// What backup work needs, shared with schedules.
#[derive(Clone)]
pub(crate) struct Backups {
  pub db: Connection,
  pub wings: Wings,
  pub storage: FileStorage,
  pub updater: Updater,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct BackupInfo {
  id: Uuid,
  name: String,
  size_bytes: i64,
  /// Set once completed.
  sha256: Option<String>,
  locked: bool,
  status: BackupStatus,
  /// Extra gitignore patterns, one per line.
  ignored: String,
  /// Why it failed.
  error: Option<String>,
  created_at: DateTime<FixedOffset>,
}

impl From<server_backup::Model> for BackupInfo {
  fn from(m: server_backup::Model) -> Self {
    Self {
      id: m.id,
      name: m.name,
      size_bytes: m.size_bytes,
      sha256: m.sha256,
      locked: m.locked,
      status: m.status,
      ignored: m.ignored,
      error: m.error,
      created_at: m.created_at,
    }
  }
}

/// `used` counts every backup that is not failed, creating ones included.
#[derive(Serialize, JsonSchema)]
struct BackupList {
  backups: Vec<BackupInfo>,
  limit: i32,
  used: u32,
}

#[derive(Deserialize, JsonSchema, Default)]
#[serde(default)]
pub(crate) struct CreateBackupRequest {
  /// Defaults to `Backup at YYYY-MM-DD HH:MM` (UTC).
  pub name: Option<String>,
  /// gitignore patterns, one per line, on top of the server's
  /// `.smaugignore`, `.pteroignore` and `.pelicanignore`.
  pub ignored: Option<String>,
  pub locked: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct LockBackupRequest {
  locked: bool,
}

#[derive(Deserialize, JsonSchema, Default)]
#[serde(default)]
struct RestoreBackupRequest {
  /// Deletes all current files first.
  truncate: bool,
}

#[derive(Deserialize, JsonSchema)]
struct BackupPath {
  uuid: Uuid,
  backup: Uuid,
}

fn key(server: Uuid, backup: Uuid) -> StoragePath {
  StoragePath::from(format!("backups/{server}/{backup}.tar.zst"))
}

fn counts(backup: &server_backup::Model) -> bool {
  backup.status != BackupStatus::Failed
}

fn ignore_lines(ignored: &str) -> Vec<String> {
  ignored
    .lines()
    .map(str::trim)
    .filter(|l| !l.is_empty())
    .map(String::from)
    .collect()
}

fn default_name(now: DateTime<Utc>) -> String {
  format!("Backup at {}", now.format("%Y-%m-%d %H:%M"))
}

impl Backups {
  fn request(
    &self,
    server: &server::Model,
    method: Method,
    path: &str,
  ) -> Result<reqwest_middleware::RequestBuilder> {
    self
      .wings
      .server_request(server.node_id, server.id, method, path)
  }

  fn progress(&self, server: Uuid, id: Uuid, status: BackupStatus, bytes: u64) {
    self
      .wings
      .live()
      .publish(server, &LiveFrame::Backup { id, status, bytes });
  }

  /// Best effort, wings also drops idle streams.
  async fn abort_stream(&self, server: &server::Model, stream: Uuid) {
    if let Ok(req) = self.request(server, Method::DELETE, &format!("/streams/{stream}"))
      && let Err(err) = call(req).await
    {
      warn!("Failed to abort stream {stream}: {err:?}");
    }
  }

  /// Opens the archive stream and pulls it into storage in the background.
  /// `409` at the backup limit, wings errors fail right away.
  pub(crate) async fn start(
    &self,
    server: &server::Model,
    req: CreateBackupRequest,
  ) -> Result<server_backup::Model> {
    let backup = self.reserve(server, req).await?;
    let opened = async {
      let body = OpenOutStream {
        source: StreamSource::Backup {
          ignore: ignore_lines(&backup.ignored),
        },
      };
      let opened: StreamOpened = call(
        self
          .request(server, Method::POST, "/streams/out")?
          .json(&body),
      )
      .await?
      .json()
      .await
      .context("Invalid stream response from the node")?;
      Ok::<_, centaurus::error::ErrorReport>(opened)
    }
    .await;
    let opened = match opened {
      Ok(opened) => opened,
      Err(err) => {
        if let Err(del) = self.db.backup().delete(backup.id).await {
          warn!("Failed to drop backup {}: {del:?}", backup.id);
        }
        return Err(err);
      }
    };

    let this = self.clone();
    let (server, id) = (server.clone(), backup.id);
    let cleanup = this.clone();
    let server_id = server.id;
    spawn_guarded(this.pump(server, id, opened.stream), async move {
      cleanup.progress(server_id, id, BackupStatus::Failed, 0);
      if let Err(err) = cleanup.db.backup().fail(id, "Backup crashed".into()).await {
        warn!("Failed to save backup {id}: {err:?}");
      }
    });
    self
      .updater
      .broadcast(UpdateMessage::Servers {
        uuid: backup.server_id,
      })
      .await;
    Ok(backup)
  }

  /// Inserts the creating row, `409` at the backup limit.
  async fn reserve(
    &self,
    server: &server::Model,
    req: CreateBackupRequest,
  ) -> Result<server_backup::Model> {
    let _guard = CREATE.lock().await;
    let used = self
      .db
      .backup()
      .list(server.id)
      .await?
      .iter()
      .filter(|b| counts(b))
      .count();
    if server.backup_limit <= 0 || used >= server.backup_limit as usize {
      bail!(CONFLICT, "Backup limit reached");
    }
    let now = Utc::now();
    let name = match req.name.as_deref().map(str::trim) {
      Some(name) if !name.is_empty() => name.to_string(),
      _ => default_name(now),
    };
    if name.chars().count() > NAME_MAX {
      bail!(BAD_REQUEST, "Name is too long");
    }
    let ignored = req.ignored.unwrap_or_default();
    Ok(
      self
        .db
        .backup()
        .insert(server_backup::Model {
          id: Uuid::now_v7(),
          server_id: server.id,
          name,
          size_bytes: 0,
          sha256: None,
          locked: req.locked.unwrap_or(false),
          status: BackupStatus::Creating,
          ignored,
          error: None,
          created_at: now.into(),
        })
        .await?,
    )
  }

  async fn pump(self, server: server::Model, id: Uuid, stream: Uuid) {
    let result = async {
      let upload = self.storage.put_multipart(&key(server.id, id)).await?;
      let mut writer = WriteMultipart::new(upload);
      match self.copy_out(&server, id, stream, &mut writer).await {
        Ok(done) => {
          writer.finish().await?;
          Ok(done)
        }
        Err(err) => {
          if let Err(abort) = writer.abort().await {
            warn!("Failed to abort backup upload {id}: {abort:?}");
          }
          Err(err)
        }
      }
    }
    .await;

    let saved = match result {
      Ok((size, sha256)) => {
        self.progress(server.id, id, BackupStatus::Completed, size);
        self.db.backup().complete(id, size, sha256).await
      }
      Err(err) => {
        let err: centaurus::error::ErrorReport = err;
        warn!("Backup {id} failed: {:?}", err.error);
        self.abort_stream(&server, stream).await;
        self.progress(server.id, id, BackupStatus::Failed, 0);
        self.db.backup().fail(id, err.error.to_string()).await
      }
    };
    if let Err(err) = saved {
      warn!("Failed to save backup {id}: {err:?}");
    }
    self
      .updater
      .broadcast(UpdateMessage::Servers { uuid: server.id })
      .await;
  }

  /// Pulls every chunk, returns size and sha256 wings reported.
  // ponytail: one chunk at a time, use the stream window if backups are slow
  async fn copy_out(
    &self,
    server: &server::Model,
    id: Uuid,
    stream: Uuid,
    writer: &mut WriteMultipart,
  ) -> Result<(u64, String)> {
    let mut bytes = 0u64;
    for index in 0u32.. {
      let route = format!("/streams/out/{stream}/{index}");
      // acks every chunk before this one, they are written
      let res = call(
        self
          .request(server, Method::GET, &route)?
          .query(&[("ack", index)]),
      )
      .await?;
      let header = |name| {
        res
          .headers()
          .get(&name)
          .and_then(|v| v.to_str().ok())
          .map(String::from)
      };
      let end = header(STREAM_END).is_some_and(|v| v == "true");
      let sha256 = header(STREAM_SHA256);
      let size = header(STREAM_SIZE).and_then(|v| v.parse::<u64>().ok());
      let chunk = res
        .bytes()
        .await
        .context("Failed to read the backup from the node")?;

      writer.wait_for_capacity(UPLOAD_CONCURRENCY).await?;
      writer.write(&chunk);
      bytes += chunk.len() as u64;
      self.progress(server.id, id, BackupStatus::Creating, bytes);

      if end {
        let total = index + 1;
        let route = format!("/streams/out/{stream}/{total}");
        call(
          self
            .request(server, Method::GET, &route)?
            .query(&[("ack", total)]),
        )
        .await?;
        let (Some(sha256), Some(size)) = (sha256, size) else {
          return Err(eyre!("Node sent no checksum for the backup").into());
        };
        if size != bytes {
          return Err(eyre!("Backup size mismatch: node {size}, received {bytes}").into());
        }
        return Ok((size, sha256));
      }
    }
    unreachable!("a stream has fewer than u32::MAX chunks")
  }

  /// Marks the backup restoring (`409` unless completed), opens the restore
  /// stream, then feeds it from storage in the background.
  async fn restore(
    &self,
    server: &server::Model,
    backup: server_backup::Model,
    truncate: bool,
  ) -> Result<()> {
    if !self.db.backup().start_restore(backup.id).await? {
      bail!(CONFLICT, "Only completed backups can be restored");
    }
    let opened = async {
      let body = OpenInStream {
        target: StreamTarget::Restore { truncate },
        size: Some(backup.size_bytes as u64),
      };
      let opened: StreamOpened = call(
        self
          .request(server, Method::POST, "/streams/in")?
          .json(&body),
      )
      .await?
      .json()
      .await
      .context("Invalid stream response from the node")?;
      Ok::<_, centaurus::error::ErrorReport>(opened)
    }
    .await;
    let opened = match opened {
      Ok(opened) => opened,
      Err(err) => {
        self.end_restore(backup.id).await;
        return Err(err);
      }
    };
    let this = self.clone();
    let cleanup = self.clone();
    let (server, id) = (server.clone(), backup.id);
    let stream = opened.stream;
    let server_id = server.id;
    spawn_guarded(this.feed(server, backup, opened), async move {
      cleanup.wings.live().forget_op(stream);
      cleanup.progress(server_id, id, BackupStatus::Failed, 0);
      cleanup.end_restore(id).await;
    });
    Ok(())
  }

  /// The backup is usable again.
  async fn end_restore(&self, id: Uuid) {
    if let Err(err) = self
      .db
      .backup()
      .set_status(id, BackupStatus::Completed)
      .await
    {
      warn!("Failed to save backup {id}: {err:?}");
    }
  }

  async fn feed(self, server: server::Model, backup: server_backup::Model, opened: StreamOpened) {
    let live = self.wings.live();
    // op id is the stream id
    let finished = live.wait_op(opened.stream);
    self.progress(server.id, backup.id, BackupStatus::Restoring, 0);

    let result = async {
      let path = key(server.id, backup.id);
      let size = backup.size_bytes as u64;
      for (index, start) in (0..size).step_by(opened.chunk_size as usize).enumerate() {
        let end = (start + opened.chunk_size).min(size);
        let data = self.storage.get_range(&path, start..end).await?;
        let route = format!("/streams/in/{}/{index}", opened.stream);
        call(self.request(&server, Method::PUT, &route)?.body(data)).await?;
        self.progress(server.id, backup.id, BackupStatus::Restoring, end);
      }
      let route = format!("/streams/in/{}/complete", opened.stream);
      let body = CompleteStream {
        sha256: backup.sha256.clone().unwrap_or_default(),
      };
      call(self.request(&server, Method::POST, &route)?.json(&body)).await?;
      match timeout(RESTORE_TIMEOUT, finished).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(err))) => Err(eyre!("Restore failed: {err}").into()),
        _ => Err(eyre!("The node never finished the restore").into()),
      }
    }
    .await;

    let status = match result {
      Ok(()) => BackupStatus::Completed,
      Err(err) => {
        let err: centaurus::error::ErrorReport = err;
        warn!("Restore of backup {} failed: {:?}", backup.id, err.error);
        live.forget_op(opened.stream);
        self.abort_stream(&server, opened.stream).await;
        BackupStatus::Failed
      }
    };
    self.progress(server.id, backup.id, status, backup.size_bytes as u64);
    self.end_restore(backup.id).await;
    self
      .updater
      .broadcast(UpdateMessage::Servers { uuid: server.id })
      .await;
  }
}

/// Best effort removal of every archive of a deleted server.
// ponytail: a backup still uploading when the server is deleted leaves its
// archive behind, sweep orphaned prefixes if that ever matters
pub(super) async fn delete_server_backups(storage: &FileStorage, server: Uuid) {
  let prefix = StoragePath::from(format!("backups/{server}"));
  let locations = storage.list(Some(&prefix)).map_ok(|m| m.location).boxed();
  let failed = storage
    .delete_stream(locations)
    .filter_map(|res| async move { res.err() })
    .collect::<Vec<_>>()
    .await;
  for err in failed {
    warn!("Failed to delete a backup of server {server}: {err:?}");
  }
}

async fn list_backups(
  auth: JwtAuth,
  db: Connection,
  Path(path): Path<ServerUuid>,
) -> Result<Json<BackupList>> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Read).await?;
  let backups = db.backup().list(server.id).await?;
  Ok(Json(BackupList {
    used: backups.iter().filter(|b| counts(b)).count() as u32,
    backups: backups.into_iter().map(Into::into).collect(),
    limit: server.backup_limit,
  }))
}

/// Starts the backup, progress follows as `backup` frames on the server
/// websocket. `409` at the backup limit.
async fn create_backup(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  storage: FileStorage,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<CreateBackupRequest>,
) -> Result<Json<BackupInfo>> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let backups = Backups {
    db,
    wings,
    storage,
    updater,
  };
  Ok(Json(backups.start(&server, req).await?.into()))
}

async fn lock_backup(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<BackupPath>,
  Json(req): Json<LockBackupRequest>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let backup = db.backup().find(server.id, path.backup).await?;
  db.backup().set_locked(backup.id, req.locked).await?;
  updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(())
}

/// Wings stops the server and extracts the backup, progress follows as
/// `backup` frames. `409` unless the backup is completed or while the server
/// is busy.
async fn restore_backup(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  storage: FileStorage,
  updater: Updater,
  Path(path): Path<BackupPath>,
  Json(req): Json<RestoreBackupRequest>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let backup = db.backup().find(server.id, path.backup).await?;
  if backup.status != BackupStatus::Completed {
    bail!(CONFLICT, "Only completed backups can be restored");
  }
  let backups = Backups {
    db,
    wings,
    storage,
    updater,
  };
  backups.restore(&server, backup, req.truncate).await?;
  backups
    .updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(())
}

/// The archive (`.tar.zst`) as an attachment.
async fn download_backup(
  auth: JwtAuth,
  db: Connection,
  storage: FileStorage,
  Path(path): Path<BackupPath>,
) -> Result<Response> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Read).await?;
  let backup = db.backup().find(server.id, path.backup).await?;
  if !matches!(
    backup.status,
    BackupStatus::Completed | BackupStatus::Restoring
  ) {
    bail!(CONFLICT, "Backup is not completed");
  }
  let body: Body = storage.get_file(&key(server.id, backup.id), None).await?;
  Ok(
    Response::builder()
      .header(header::CONTENT_TYPE, "application/zstd")
      .header(header::CONTENT_LENGTH, backup.size_bytes)
      .header(
        header::CONTENT_DISPOSITION,
        attachment(&format!("{}.tar.zst", backup.name)),
      )
      .body(body)
      .expect("valid headers"),
  )
}

/// `409` while locked, creating or restoring.
async fn delete_backup(
  auth: JwtAuth,
  db: Connection,
  storage: FileStorage,
  updater: Updater,
  Path(path): Path<BackupPath>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let backup = db.backup().find(server.id, path.backup).await?;
  if backup.locked {
    bail!(CONFLICT, "Backup is locked");
  }
  if matches!(
    backup.status,
    BackupStatus::Creating | BackupStatus::Restoring
  ) {
    bail!(CONFLICT, "Backup is in use");
  }
  // a restore may have started since the checks above
  if !db.backup().delete_idle(backup.id).await? {
    bail!(CONFLICT, "Backup is in use");
  }
  if let Err(err) = storage.delete_file(&key(server.id, backup.id)).await {
    warn!(
      "Failed to delete backup file {}: {:?}",
      backup.id, err.error
    );
  }
  updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn ignore_lines_skip_blank() {
    assert_eq!(
      ignore_lines(" logs/ \n\n*.tmp\r\n  "),
      vec!["logs/".to_string(), "*.tmp".to_string()]
    );
    assert!(ignore_lines("").is_empty());
  }

  #[test]
  fn default_name_is_utc_minutes() {
    let at = DateTime::parse_from_rfc3339("2026-10-21T13:05:59+02:00")
      .unwrap()
      .with_timezone(&Utc);
    assert_eq!(default_name(at), "Backup at 2026-10-21 11:05");
  }

  #[tokio::test]
  async fn guarded_work_runs_cleanup_only_after_a_panic() {
    let (tx, rx) = tokio::sync::oneshot::channel();
    spawn_guarded(async { panic!("boom") }, async move {
      tx.send(()).unwrap();
    });
    rx.await.expect("cleanup ran");

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    spawn_guarded(
      async move {
        done_tx.send(()).unwrap();
      },
      async move {
        tx.send(()).unwrap();
      },
    );
    done_rx.await.unwrap();
    // the cleanup future is dropped unrun
    assert!(rx.await.is_err());
  }

  #[test]
  fn keys_are_per_server() {
    let (s, b) = (Uuid::nil(), Uuid::max());
    assert_eq!(
      key(s, b).as_ref(),
      format!("backups/{s}/{b}.tar.zst").as_str()
    );
  }
}
