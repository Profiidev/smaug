//! Schedules run in the panel: cron (UTC) triggers ordered tasks.

use std::{str::FromStr, sync::Arc, time::Duration};

use aide::{
  OperationIo,
  axum::{
    ApiRouter,
    routing::{get_with, patch_with, post_with},
  },
};
use axum::{Extension, Json, extract::FromRequestParts, extract::Path};
use centaurus::{bail, db::init::Connection, error::Result};
use chrono::{DateTime, FixedOffset, Timelike, Utc};
use croner::Cron;
use dashmap::{DashMap, Entry};
use entity::{sea_orm_active_enums::ScheduleAction, server, server_schedule, server_schedule_task};
use http::StatusCode;
use schemars::JsonSchema;
use sea_orm::{
  ActiveModelTrait,
  ActiveValue::{Set, Unchanged},
};
use serde::{Deserialize, Serialize};
use shared::msg::CommandRequest;
use tokio::{
  spawn,
  time::{MissedTickBehavior, interval, sleep},
};
use tracing::warn;
use uuid::Uuid;

use super::{
  access::{Access, load},
  backups::{Backups, CreateBackupRequest},
  error::{FieldResult, ServerError, field_err},
  live::{LiveFrame, ScheduleRunState},
  power::{apply_power, send_command},
  status::{ServerStatus, status},
  types::{PowerAction, ServerUuid},
};
use crate::{
  auth::jwt_auth::JwtAuth,
  db::DBTrait,
  utils::{UpdateMessage, Updater},
};

const NAME_MAX: usize = 191;

pub(super) fn router() -> ApiRouter {
  ApiRouter::new()
    .api_route(
      "/",
      get_with(list_schedules, |op| op.id("list_schedules"))
        .post_with(create_schedule, |op| op.id("create_schedule")),
    )
    .api_route(
      "/{schedule}",
      get_with(schedule_info, |op| op.id("schedule_info"))
        .patch_with(update_schedule, |op| op.id("update_schedule"))
        .delete_with(delete_schedule, |op| op.id("delete_schedule")),
    )
    .api_route(
      "/{schedule}/run",
      post_with(run_schedule, |op| op.id("run_schedule")),
    )
    .api_route(
      "/{schedule}/tasks",
      post_with(create_schedule_task, |op| op.id("create_schedule_task")),
    )
    .api_route(
      "/{schedule}/tasks/{task}",
      patch_with(update_schedule_task, |op| op.id("update_schedule_task"))
        .delete_with(delete_schedule_task, |op| op.id("delete_schedule_task")),
    )
}

/// Runs schedules, due ones every second and on request.
// ponytail: running state is in memory, a single panel replica is assumed;
// move it to the DB (with a lease) before running several replicas
#[derive(Clone, FromRequestParts, OperationIo)]
#[from_request(via(Extension))]
pub struct Scheduler {
  backups: Backups,
  /// Schedule id -> index of the task running or waiting for its offset.
  running: Arc<DashMap<Uuid, u32>>,
}

/// Releases a [`Scheduler::claim`] on drop, also when a run panics.
struct Claim {
  running: Arc<DashMap<Uuid, u32>>,
  id: Uuid,
}

impl Drop for Claim {
  fn drop(&mut self) {
    self.running.remove(&self.id);
  }
}

/// The five cron fields, UTC.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq, Eq)]
pub struct CronFields {
  pub minute: String,
  pub hour: String,
  pub day_of_month: String,
  pub month: String,
  pub day_of_week: String,
}

#[derive(Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ScheduleStatus {
  Active,
  Inactive,
  Running,
}

#[derive(Serialize, JsonSchema)]
struct ScheduleInfo {
  id: Uuid,
  name: String,
  cron: CronFields,
  enabled: bool,
  only_when_online: bool,
  last_run_at: Option<DateTime<FixedOffset>>,
  /// `None` while disabled.
  next_run_at: Option<DateTime<Utc>>,
  status: ScheduleStatus,
  /// Index into `tasks` while running.
  running_task: Option<u32>,
  created_at: DateTime<FixedOffset>,
  tasks: Vec<TaskInfo>,
}

#[derive(Serialize, JsonSchema)]
struct TaskInfo {
  id: Uuid,
  position: i32,
  action: ScheduleAction,
  /// Command line, power action (`start|stop|restart|kill`) or backup ignore
  /// patterns (one per line).
  payload: String,
  /// Wait after the previous task finished.
  offset_seconds: i32,
  continue_on_failure: bool,
}

impl From<server_schedule_task::Model> for TaskInfo {
  fn from(t: server_schedule_task::Model) -> Self {
    Self {
      id: t.id,
      position: t.position,
      action: t.action,
      payload: t.payload,
      offset_seconds: t.offset_seconds,
      continue_on_failure: t.continue_on_failure,
    }
  }
}

fn default_true() -> bool {
  true
}

#[derive(Deserialize, JsonSchema)]
struct CreateScheduleRequest {
  name: String,
  cron: CronFields,
  #[serde(default = "default_true")]
  enabled: bool,
  #[serde(default)]
  only_when_online: bool,
}

/// Only the given fields change.
#[derive(Deserialize, JsonSchema)]
struct UpdateScheduleRequest {
  name: Option<String>,
  cron: Option<CronFields>,
  enabled: Option<bool>,
  only_when_online: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct CreateTaskRequest {
  action: ScheduleAction,
  #[serde(default)]
  payload: String,
  #[serde(default)]
  offset_seconds: i32,
  #[serde(default)]
  continue_on_failure: bool,
}

/// Only the given fields change.
#[derive(Deserialize, JsonSchema)]
struct UpdateTaskRequest {
  action: Option<ScheduleAction>,
  payload: Option<String>,
  offset_seconds: Option<i32>,
  continue_on_failure: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct SchedulePath {
  uuid: Uuid,
  schedule: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct TaskPath {
  uuid: Uuid,
  schedule: Uuid,
  task: Uuid,
}

fn fields(s: &server_schedule::Model) -> CronFields {
  CronFields {
    minute: s.minute.clone(),
    hour: s.hour.clone(),
    day_of_month: s.day_of_month.clone(),
    month: s.month.clone(),
    day_of_week: s.day_of_week.clone(),
  }
}

fn invalid(field: &str, message: &str) -> ServerError {
  ServerError::Fields(StatusCode::BAD_REQUEST, vec![field_err(field, message)])
}

/// `400` naming the first invalid field, e.g. `cron.minute`.
fn parse_cron(c: &CronFields) -> FieldResult<Cron> {
  let values = [
    ("minute", &c.minute),
    ("hour", &c.hour),
    ("day_of_month", &c.day_of_month),
    ("month", &c.month),
    ("day_of_week", &c.day_of_week),
  ];
  for (i, (name, value)) in values.iter().enumerate() {
    let mut parts = ["*"; 5];
    parts[i] = value.as_str();
    if value.is_empty()
      || value.contains(char::is_whitespace)
      || Cron::from_str(&parts.join(" ")).is_err()
    {
      return Err(invalid(&format!("cron.{name}"), "Invalid value"));
    }
  }
  let pattern = values.map(|(_, v)| v.as_str()).join(" ");
  Cron::from_str(&pattern).map_err(|err| invalid("cron", &err.to_string()))
}

/// Whether the minute `now` is in matches and has not run yet.
fn is_due(cron: &Cron, last_run: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
  let slot = now
    .with_second(0)
    .and_then(|t| t.with_nanosecond(0))
    .expect("minute start exists");
  last_run.is_none_or(|at| at < slot) && cron.is_time_matching(&slot).unwrap_or(false)
}

fn next_run(cron: &Cron, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
  cron.find_next_occurrence(&now, false).ok()
}

fn valid_name(name: &str) -> FieldResult<String> {
  let name = name.trim();
  if name.is_empty() {
    return Err(invalid("name", "Name is required"));
  }
  if name.chars().count() > NAME_MAX {
    return Err(invalid("name", "Name is too long"));
  }
  Ok(name.to_string())
}

fn parse_power(payload: &str) -> Result<PowerAction> {
  serde_json::from_value(serde_json::Value::String(payload.trim().to_string()))
    .map_err(|_| centaurus::anyhow!(BAD_REQUEST, "{POWER_ACTIONS}"))
}

const POWER_ACTIONS: &str = "Power action must be start, stop, restart or kill";

/// The payload as stored, `400` when it does not fit the action.
fn valid_payload(
  server: &server::Model,
  action: ScheduleAction,
  payload: &str,
) -> FieldResult<String> {
  Ok(match action {
    ScheduleAction::Command => {
      let line = payload.trim();
      if line.is_empty() {
        return Err(invalid("payload", "Command is required"));
      }
      line.to_string()
    }
    ScheduleAction::Power => {
      parse_power(payload).map_err(|_| invalid("payload", POWER_ACTIONS))?;
      payload.trim().to_string()
    }
    ScheduleAction::Backup => {
      if server.backup_limit <= 0 {
        return Err(invalid("action", "Backups are disabled for this server"));
      }
      payload.to_string()
    }
  })
}

fn valid_offset(offset: i32) -> FieldResult<i32> {
  if offset < 0 {
    return Err(invalid("offset_seconds", "Offset can not be negative"));
  }
  Ok(offset)
}

impl Scheduler {
  pub(crate) fn new(backups: Backups) -> Self {
    Self {
      backups,
      running: Default::default(),
    }
  }

  fn db(&self) -> &Connection {
    &self.backups.db
  }

  fn updater(&self) -> &Updater {
    &self.backups.updater
  }

  fn info(
    &self,
    s: server_schedule::Model,
    tasks: Vec<server_schedule_task::Model>,
    now: DateTime<Utc>,
  ) -> ScheduleInfo {
    let running_task = self.running.get(&s.id).map(|i| *i);
    let next_run_at = if s.enabled {
      parse_cron(&fields(&s)).ok().and_then(|c| next_run(&c, now))
    } else {
      None
    };
    ScheduleInfo {
      status: match (running_task, s.enabled) {
        (Some(_), _) => ScheduleStatus::Running,
        (None, true) => ScheduleStatus::Active,
        (None, false) => ScheduleStatus::Inactive,
      },
      cron: fields(&s),
      id: s.id,
      name: s.name,
      enabled: s.enabled,
      only_when_online: s.only_when_online,
      last_run_at: s.last_run_at,
      next_run_at,
      running_task,
      created_at: s.created_at,
      tasks: tasks
        .into_iter()
        .filter(|t| t.schedule_id == s.id)
        .map(Into::into)
        .collect(),
    }
  }

  async fn load_info(&self, s: server_schedule::Model) -> Result<ScheduleInfo> {
    let tasks = self.db().schedule().tasks(vec![s.id]).await?;
    Ok(self.info(s, tasks, Utc::now()))
  }

  async fn is_online(&self, server: &server::Model) -> bool {
    status(server, self.backups.wings.state(server.node_id, server.id)) == ServerStatus::Running
  }

  fn publish(&self, s: &server_schedule::Model, task_index: Option<u32>) {
    let status = if task_index.is_some() || self.running.contains_key(&s.id) {
      ScheduleRunState::Running
    } else {
      ScheduleRunState::Idle
    };
    self.backups.wings.live().publish(
      s.server_id,
      &LiveFrame::Schedule {
        id: s.id,
        status,
        task_index,
      },
    );
  }

  /// Marks the schedule running, `false` when it already is.
  fn claim(&self, id: Uuid) -> bool {
    match self.running.entry(id) {
      Entry::Occupied(_) => false,
      Entry::Vacant(v) => {
        v.insert(0);
        true
      }
    }
  }

  /// After a successful [`Self::claim`], released whatever happens.
  async fn start(&self, s: server_schedule::Model) -> Result<()> {
    let claim = Claim {
      running: self.running.clone(),
      id: s.id,
    };
    self
      .db()
      .schedule()
      .set_last_run(s.id, Utc::now().into())
      .await?;
    let this = self.clone();
    spawn(async move { this.execute(s, claim).await });
    Ok(())
  }

  async fn execute(self, s: server_schedule::Model, claim: Claim) {
    let tasks = match self.db().schedule().tasks(vec![s.id]).await {
      Ok(tasks) => tasks,
      Err(err) => {
        warn!("Failed to load tasks of schedule {}: {err:?}", s.id);
        Vec::new()
      }
    };
    self
      .updater()
      .broadcast(UpdateMessage::Servers { uuid: s.server_id })
      .await;

    for (index, task) in tasks.iter().enumerate() {
      let index = index as u32;
      self.running.insert(s.id, index);
      self.publish(&s, Some(index));
      sleep(Duration::from_secs(task.offset_seconds.max(0) as u64)).await;
      if let Err(err) = self.run_task(&s, task).await {
        warn!(
          "Task {} of schedule {} failed: {:?}",
          index, s.id, err.error
        );
        if !task.continue_on_failure {
          break;
        }
      }
    }

    drop(claim);
    self.publish(&s, None);
    self
      .updater()
      .broadcast(UpdateMessage::Servers { uuid: s.server_id })
      .await;
  }

  async fn run_task(
    &self,
    s: &server_schedule::Model,
    task: &server_schedule_task::Model,
  ) -> Result<()> {
    let server = self.db().server().find_by_id(s.server_id).await?;
    let b = &self.backups;
    match task.action {
      ScheduleAction::Command => {
        let req = CommandRequest {
          line: task.payload.clone(),
        };
        send_command(&b.wings, &server, req).await
      }
      ScheduleAction::Power => {
        let action = parse_power(&task.payload)?;
        apply_power(&b.db, &b.wings, &b.updater, &server, action).await
      }
      // only starting it can fail the task, the backup runs on its own
      ScheduleAction::Backup => {
        let req = CreateBackupRequest {
          ignored: Some(task.payload.clone()),
          ..Default::default()
        };
        b.start(&server, req).await.map(|_| ())
      }
    }
  }

  /// Starts every due schedule, forever.
  pub async fn run(self) {
    let mut tick = interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
      tick.tick().await;
      if let Err(err) = self.tick(Utc::now()).await {
        warn!("Schedule tick failed: {:?}", err.error);
      }
    }
  }

  async fn tick(&self, now: DateTime<Utc>) -> Result<()> {
    for s in self.db().schedule().list_enabled().await? {
      let Ok(cron) = parse_cron(&fields(&s)) else {
        continue;
      };
      if !is_due(&cron, s.last_run_at.map(|at| at.to_utc()), now) {
        continue;
      }
      let server = self.db().server().find_by_id(s.server_id).await?;
      // a suspended server is read-only, see `access::load`
      if server.suspended || (s.only_when_online && !self.is_online(&server).await) {
        continue;
      }
      if self.claim(s.id) {
        self.start(s).await?;
      }
    }
    Ok(())
  }
}

async fn list_schedules(
  auth: JwtAuth,
  db: Connection,
  scheduler: Scheduler,
  Path(path): Path<ServerUuid>,
) -> Result<Json<Vec<ScheduleInfo>>> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Read).await?;
  let schedules = db.schedule().list(server.id).await?;
  let tasks = db
    .schedule()
    .tasks(schedules.iter().map(|s| s.id).collect())
    .await?;
  let now = Utc::now();
  Ok(Json(
    schedules
      .into_iter()
      .map(|s| scheduler.info(s, tasks.clone(), now))
      .collect(),
  ))
}

/// `400` naming an invalid cron field.
async fn create_schedule(
  auth: JwtAuth,
  db: Connection,
  scheduler: Scheduler,
  updater: Updater,
  Path(path): Path<ServerUuid>,
  Json(req): Json<CreateScheduleRequest>,
) -> FieldResult<Json<ScheduleInfo>> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let name = valid_name(&req.name)?;
  parse_cron(&req.cron)?;
  let c = req.cron;
  let s = db
    .schedule()
    .insert(server_schedule::Model {
      id: Uuid::now_v7(),
      server_id: server.id,
      name,
      minute: c.minute,
      hour: c.hour,
      day_of_month: c.day_of_month,
      month: c.month,
      day_of_week: c.day_of_week,
      enabled: req.enabled,
      only_when_online: req.only_when_online,
      last_run_at: None,
      created_at: Utc::now().into(),
    })
    .await?;
  updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(Json(scheduler.info(s, Vec::new(), Utc::now())))
}

async fn schedule_info(
  auth: JwtAuth,
  db: Connection,
  scheduler: Scheduler,
  Path(path): Path<SchedulePath>,
) -> FieldResult<Json<ScheduleInfo>> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Read).await?;
  let s = db.schedule().find(server.id, path.schedule).await?;
  Ok(Json(scheduler.load_info(s).await?))
}

async fn update_schedule(
  auth: JwtAuth,
  db: Connection,
  scheduler: Scheduler,
  updater: Updater,
  Path(path): Path<SchedulePath>,
  Json(req): Json<UpdateScheduleRequest>,
) -> FieldResult<Json<ScheduleInfo>> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let s = db.schedule().find(server.id, path.schedule).await?;
  // only the edited columns, a run may set `last_run_at` meanwhile
  let mut edit = server_schedule::ActiveModel {
    id: Unchanged(s.id),
    ..Default::default()
  };
  if let Some(name) = req.name {
    edit.name = Set(valid_name(&name)?);
  }
  if let Some(c) = req.cron {
    parse_cron(&c)?;
    edit.minute = Set(c.minute);
    edit.hour = Set(c.hour);
    edit.day_of_month = Set(c.day_of_month);
    edit.month = Set(c.month);
    edit.day_of_week = Set(c.day_of_week);
  }
  if let Some(enabled) = req.enabled {
    edit.enabled = Set(enabled);
  }
  if let Some(only) = req.only_when_online {
    edit.only_when_online = Set(only);
  }
  let s = if edit.is_changed() {
    db.schedule().update(edit).await?
  } else {
    s
  };
  updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(Json(scheduler.load_info(s).await?))
}

/// A running schedule finishes its current run.
async fn delete_schedule(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<SchedulePath>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let s = db.schedule().find(server.id, path.schedule).await?;
  db.schedule().delete(s.id).await?;
  updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(())
}

/// Runs now, also when disabled. `409` while running, or when it is only
/// for an online server and the server is not running.
async fn run_schedule(
  auth: JwtAuth,
  db: Connection,
  scheduler: Scheduler,
  Path(path): Path<SchedulePath>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let s = db.schedule().find(server.id, path.schedule).await?;
  if s.only_when_online && !scheduler.is_online(&server).await {
    bail!(CONFLICT, "The server is not running");
  }
  if !scheduler.claim(s.id) {
    bail!(CONFLICT, "The schedule is already running");
  }
  scheduler.start(s).await
}

/// Appended after the last task.
async fn create_schedule_task(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<SchedulePath>,
  Json(req): Json<CreateTaskRequest>,
) -> FieldResult<Json<TaskInfo>> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let s = db.schedule().find(server.id, path.schedule).await?;
  let task = db
    .schedule()
    .insert_task(server_schedule_task::Model {
      id: Uuid::now_v7(),
      schedule_id: s.id,
      position: 0,
      action: req.action,
      payload: valid_payload(&server, req.action, &req.payload)?,
      offset_seconds: valid_offset(req.offset_seconds)?,
      continue_on_failure: req.continue_on_failure,
    })
    .await?;
  updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(Json(task.into()))
}

async fn update_schedule_task(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<TaskPath>,
  Json(req): Json<UpdateTaskRequest>,
) -> FieldResult<Json<TaskInfo>> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let s = db.schedule().find(server.id, path.schedule).await?;
  let mut task = db.schedule().find_task(s.id, path.task).await?;
  if let Some(action) = req.action {
    task.action = action;
  }
  let payload = req.payload.unwrap_or_else(|| task.payload.clone());
  task.payload = valid_payload(&server, task.action, &payload)?;
  if let Some(offset) = req.offset_seconds {
    task.offset_seconds = valid_offset(offset)?;
  }
  if let Some(cont) = req.continue_on_failure {
    task.continue_on_failure = cont;
  }
  db.schedule().update_task(task.clone()).await?;
  updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(Json(task.into()))
}

async fn delete_schedule_task(
  auth: JwtAuth,
  db: Connection,
  updater: Updater,
  Path(path): Path<TaskPath>,
) -> Result<()> {
  let (server, _) = load(&db, auth.user_id, path.uuid, Access::Write).await?;
  let s = db.schedule().find(server.id, path.schedule).await?;
  let task = db.schedule().find_task(s.id, path.task).await?;
  db.schedule().delete_task(task.id).await?;
  updater
    .broadcast(UpdateMessage::Servers { uuid: server.id })
    .await;
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  use crate::servers::status::test::model;

  fn cron(minute: &str, hour: &str, dom: &str, month: &str, dow: &str) -> CronFields {
    CronFields {
      minute: minute.into(),
      hour: hour.into(),
      day_of_month: dom.into(),
      month: month.into(),
      day_of_week: dow.into(),
    }
  }

  /// The single field a 400 names.
  fn field<T: std::fmt::Debug>(res: FieldResult<T>) -> String {
    match res.unwrap_err() {
      ServerError::Fields(StatusCode::BAD_REQUEST, errors) if errors.len() == 1 => {
        errors[0].field.clone()
      }
      other => panic!("{other:?}"),
    }
  }

  fn at(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().to_utc()
  }

  #[test]
  fn cron_errors_name_the_field() {
    for (fields, name) in [
      (cron("61", "*", "*", "*", "*"), "minute"),
      (cron("", "*", "*", "*", "*"), "minute"),
      (cron("*", "25", "*", "*", "*"), "hour"),
      (cron("*", "1 2", "*", "*", "*"), "hour"),
      (cron("*", "*", "0", "*", "*"), "day_of_month"),
      (cron("*", "*", "*", "13", "*"), "month"),
      (cron("*", "*", "*", "*", "nope"), "day_of_week"),
    ] {
      assert_eq!(field(parse_cron(&fields)), format!("cron.{name}"));
    }
    assert!(parse_cron(&cron("*/5", "1,15", "1-10", "*", "mon")).is_ok());
  }

  #[test]
  fn next_run_is_utc() {
    let c = parse_cron(&cron("30", "4", "*", "*", "*")).unwrap();
    assert_eq!(
      next_run(&c, at("2026-10-21T04:30:00Z")),
      Some(at("2026-10-22T04:30:00Z"))
    );
    assert_eq!(
      next_run(&c, at("2026-10-21T03:00:10Z")),
      Some(at("2026-10-21T04:30:00Z"))
    );
    let every5 = parse_cron(&cron("*/5", "*", "*", "*", "*")).unwrap();
    assert_eq!(
      next_run(&every5, at("2026-10-21T10:01:00Z")),
      Some(at("2026-10-21T10:05:00Z"))
    );
  }

  #[test]
  fn due_once_per_matching_minute() {
    let c = parse_cron(&cron("30", "4", "*", "*", "*")).unwrap();
    let now = at("2026-10-21T04:30:12Z");
    assert!(is_due(&c, None, now));
    assert!(is_due(&c, Some(at("2026-10-20T04:30:05Z")), now));
    // already ran in this minute
    assert!(!is_due(&c, Some(at("2026-10-21T04:30:01Z")), now));
    assert!(!is_due(&c, None, at("2026-10-21T04:31:00Z")));
  }

  #[test]
  fn payloads_are_validated() {
    let mut server = model();
    server.backup_limit = 1;
    assert_eq!(
      valid_payload(&server, ScheduleAction::Command, " say hi ").unwrap(),
      "say hi"
    );
    assert!(valid_payload(&server, ScheduleAction::Command, "  ").is_err());
    assert_eq!(
      valid_payload(&server, ScheduleAction::Power, "restart").unwrap(),
      "restart"
    );
    assert!(valid_payload(&server, ScheduleAction::Power, "explode").is_err());
    assert_eq!(
      valid_payload(&server, ScheduleAction::Backup, "logs/\n").unwrap(),
      "logs/\n"
    );
    server.backup_limit = 0;
    assert_eq!(
      field(valid_payload(&server, ScheduleAction::Backup, "")),
      "action"
    );
    assert_eq!(
      field(valid_payload(&server, ScheduleAction::Power, "x")),
      "payload"
    );
    assert_eq!(field(valid_offset(-1)), "offset_seconds");
    assert_eq!(valid_offset(300).unwrap(), 300);
    assert_eq!(field(valid_name("  ")), "name");
    assert_eq!(field(valid_name(&"x".repeat(192))), "name");
    assert_eq!(valid_name(" a ").unwrap(), "a");
  }

  async fn scheduler() -> Scheduler {
    use centaurus::{
      backend::endpoints::websocket::state::UpdateState,
      storage::{FileStorage, StorageConfig},
    };

    use crate::{nodes::Wings, utils::UpdateMessage};

    let db = crate::db::sqlite_db().await;
    let (_, updater) = UpdateState::<UpdateMessage>::init().await;
    let storage = FileStorage::init(&StorageConfig {
      storage_path: std::env::temp_dir()
        .join(format!("smaug-unit-{}", Uuid::now_v7()))
        .to_string_lossy()
        .into(),
      ..Default::default()
    })
    .await
    .unwrap();
    let wings = Wings::new(&db, updater.clone(), true).await.unwrap();
    Scheduler::new(Backups {
      db,
      wings,
      storage,
      updater,
    })
  }

  async fn insert(sc: &Scheduler, server: Uuid, enabled: bool, online: bool) -> Uuid {
    let s = sc
      .db()
      .schedule()
      .insert(server_schedule::Model {
        id: Uuid::now_v7(),
        server_id: server,
        name: "s".into(),
        minute: "30".into(),
        hour: "4".into(),
        day_of_month: "*".into(),
        month: "*".into(),
        day_of_week: "*".into(),
        enabled,
        only_when_online: online,
        last_run_at: None,
        created_at: Utc::now().into(),
      })
      .await
      .unwrap();
    s.id
  }

  async fn last_run(sc: &Scheduler, server: Uuid, id: Uuid) -> Option<DateTime<FixedOffset>> {
    sc.db()
      .schedule()
      .find(server, id)
      .await
      .unwrap()
      .last_run_at
  }

  #[tokio::test]
  async fn claim_is_released_on_panic_and_failed_start() {
    let sc = scheduler().await;
    let id = Uuid::now_v7();
    assert!(sc.claim(id));
    let claim = Claim {
      running: sc.running.clone(),
      id,
    };
    let res = spawn(async move {
      let _claim = claim;
      panic!("task crashed");
    })
    .await;
    assert!(res.is_err());
    assert!(!sc.running.contains_key(&id));

    // the schedule row does not exist, saving the run fails
    assert!(sc.claim(id));
    let s = server_schedule::Model {
      id,
      server_id: Uuid::now_v7(),
      name: "s".into(),
      minute: "*".into(),
      hour: "*".into(),
      day_of_month: "*".into(),
      month: "*".into(),
      day_of_week: "*".into(),
      enabled: true,
      only_when_online: false,
      last_run_at: None,
      created_at: Utc::now().into(),
    };
    assert!(sc.start(s).await.is_err());
    assert!(sc.claim(id));
  }

  #[tokio::test]
  async fn tick_starts_due_schedules_once() {
    let sc = scheduler().await;
    let server = model();
    let sid = server.id;
    server::ActiveModel::from(server)
      .insert(&sc.db().0)
      .await
      .unwrap();
    let due = insert(&sc, sid, true, false).await;
    // the server state is unknown, it is not online
    let offline = insert(&sc, sid, true, true).await;
    let disabled = insert(&sc, sid, false, false).await;
    let suspended = server::Model {
      suspended: true,
      ..model()
    };
    let susp_id = suspended.id;
    server::ActiveModel::from(suspended)
      .insert(&sc.db().0)
      .await
      .unwrap();
    let on_suspended = insert(&sc, susp_id, true, false).await;

    let now = at("2026-01-01T04:30:10Z");
    sc.tick(now).await.unwrap();
    let ran = last_run(&sc, sid, due).await.unwrap();
    assert!(last_run(&sc, susp_id, on_suspended).await.is_none());
    assert!(last_run(&sc, sid, offline).await.is_none());
    assert!(last_run(&sc, sid, disabled).await.is_none());

    // same minute again, and a minute that does not match: nothing new
    tokio::time::sleep(Duration::from_millis(50)).await;
    sc.tick(now).await.unwrap();
    sc.tick(at("2026-01-01T04:31:00Z")).await.unwrap();
    assert_eq!(last_run(&sc, sid, due).await, Some(ran));
    assert!(!sc.running.contains_key(&due));
  }
}
