//! Schedules: CRUD, validation and runs against a fake wings.
mod common;

use std::time::Duration;

use axum::{
  http::{Method, StatusCode as AxumStatus},
  response::{IntoResponse, Response},
};
use common::wings::*;
use reqwest::StatusCode;
use serde_json::{Value, json};
use tokio::time::timeout;

/// Commands `fail` are refused, others accepted.
fn command_wings(r: &Req) -> Option<Response> {
  if is(r, Method::POST, "/command") {
    return Some(if r.json()["line"] == "fail" {
      wings_error(AxumStatus::CONFLICT, "conflict")
    } else {
      AxumStatus::NO_CONTENT.into_response()
    });
  }
  None
}

fn cron(minute: &str) -> Value {
  json!({ "minute": minute, "hour": "4", "day_of_month": "*", "month": "*", "day_of_week": "*" })
}

async fn create(c: &Ctx, body: Value) -> Value {
  let resp = c.server.post(&c.path("/schedules"), body).await;
  assert_eq!(resp.status(), StatusCode::OK);
  resp.json().await.unwrap()
}

async fn add_task(c: &Ctx, sid: &str, body: Value) -> Value {
  let resp = c
    .server
    .post(&c.path(&format!("/schedules/{sid}/tasks")), body)
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  resp.json().await.unwrap()
}

async fn get(c: &Ctx, sid: &str) -> Value {
  let resp = c.server.get(&c.path(&format!("/schedules/{sid}"))).await;
  assert_eq!(resp.status(), StatusCode::OK);
  resp.json().await.unwrap()
}

async fn run(c: &Ctx, sid: &str) -> StatusCode {
  c.server
    .post(&c.path(&format!("/schedules/{sid}/run")), json!({}))
    .await
    .status()
}

async fn wait_idle(c: &Ctx, sid: &str) -> Value {
  timeout(Duration::from_secs(20), async {
    loop {
      let s = get(c, sid).await;
      if s["status"] != "running" {
        return s;
      }
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
  })
  .await
  .expect("schedule never finished")
}

fn commands(c: &Ctx) -> Vec<String> {
  c.fake
    .requests()
    .into_iter()
    .filter(|r| is(r, Method::POST, "/command"))
    .map(|r| r.json()["line"].as_str().unwrap().to_string())
    .collect()
}

#[tokio::test]
async fn schedule_crud() {
  let c = setup(|_| None).await;
  let s = create(&c, json!({ "name": " Restart ", "cron": cron("30") })).await;
  let sid = s["id"].as_str().unwrap().to_string();
  assert_eq!(s["name"], "Restart");
  assert_eq!(s["cron"], cron("30"));
  assert_eq!(s["enabled"], true);
  assert_eq!(s["only_when_online"], false);
  assert_eq!(s["status"], "active");
  assert_eq!(s["last_run_at"], Value::Null);
  let next = s["next_run_at"].as_str().unwrap();
  assert!(next.ends_with("04:30:00Z"), "{next}");
  assert_eq!(s["tasks"], json!([]));

  let a = add_task(
    &c,
    &sid,
    json!({ "action": "command", "payload": " say hi " }),
  )
  .await;
  assert_eq!(a["payload"], "say hi");
  assert_eq!(a["position"], 0);
  assert_eq!(a["offset_seconds"], 0);
  assert_eq!(a["continue_on_failure"], false);
  let b = add_task(
    &c,
    &sid,
    json!({ "action": "power", "payload": "restart", "offset_seconds": 60, "continue_on_failure": true }),
  )
  .await;
  assert_eq!(b["position"], 1);

  let tid = a["id"].as_str().unwrap();
  let task_path = c.path(&format!("/schedules/{sid}/tasks/{tid}"));
  let resp = c
    .server
    .patch(&task_path, json!({ "offset_seconds": 5 }))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let patched: Value = resp.json().await.unwrap();
  assert_eq!(patched["offset_seconds"], 5);
  assert_eq!(patched["payload"], "say hi");
  // switching the action revalidates the kept payload
  let resp = c
    .server
    .patch(&task_path, json!({ "action": "power" }))
    .await;
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

  let resp = c
    .server
    .patch(
      &c.path(&format!("/schedules/{sid}")),
      json!({ "enabled": false, "name": "Nightly" }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let s = get(&c, &sid).await;
  assert_eq!(s["name"], "Nightly");
  assert_eq!(s["status"], "inactive");
  assert_eq!(s["next_run_at"], Value::Null);
  assert_eq!(s["cron"], cron("30"));
  let tasks: Vec<_> = s["tasks"]
    .as_array()
    .unwrap()
    .iter()
    .map(|t| t["id"].clone())
    .collect();
  assert_eq!(tasks, vec![a["id"].clone(), b["id"].clone()]);

  let list: Value = c
    .server
    .get(&c.path("/schedules"))
    .await
    .json()
    .await
    .unwrap();
  assert_eq!(list.as_array().unwrap().len(), 1);
  assert_eq!(list[0]["tasks"].as_array().unwrap().len(), 2);

  assert_eq!(
    c.server.delete(&task_path, json!({})).await.status(),
    StatusCode::OK
  );
  assert_eq!(get(&c, &sid).await["tasks"].as_array().unwrap().len(), 1);
  assert_eq!(
    c.server.delete(&task_path, json!({})).await.status(),
    StatusCode::NOT_FOUND
  );

  let path = c.path(&format!("/schedules/{sid}"));
  assert_eq!(
    c.server.delete(&path, json!({})).await.status(),
    StatusCode::OK
  );
  assert_eq!(c.server.get(&path).await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn invalid_input_is_bad_request() {
  let c = setup(|_| None).await;
  let resp = c
    .server
    .post(
      &c.path("/schedules"),
      json!({ "name": "x", "cron": cron("61") }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
  assert_eq!(
    resp.json::<Value>().await.unwrap(),
    json!({ "errors": [{ "field": "cron.minute", "message": "Invalid value" }] })
  );
  let resp = c
    .server
    .post(
      &c.path("/schedules"),
      json!({ "name": " ", "cron": cron("1") }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

  let s = create(&c, json!({ "name": "x", "cron": cron("1") })).await;
  let sid = s["id"].as_str().unwrap();
  let mut bad = cron("1");
  bad["day_of_week"] = json!("someday");
  let resp = c
    .server
    .patch(
      &c.path(&format!("/schedules/{sid}")),
      json!({ "cron": bad }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
  assert_eq!(
    resp.json::<Value>().await.unwrap()["errors"][0]["field"],
    "cron.day_of_week"
  );

  let tasks = c.path(&format!("/schedules/{sid}/tasks"));
  for body in [
    json!({ "action": "command", "payload": "  " }),
    json!({ "action": "power", "payload": "explode" }),
    json!({ "action": "command", "payload": "x", "offset_seconds": -1 }),
    // backups are off for this server
    json!({ "action": "backup", "payload": "" }),
  ] {
    let resp = c.server.post(&tasks, body.clone()).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{body}");
  }
  c.set_backup_limit(1).await;
  add_task(&c, sid, json!({ "action": "backup", "payload": "logs/" })).await;
}

#[tokio::test]
async fn run_respects_only_when_online() {
  let c = setup(command_wings).await;
  let s = create(
    &c,
    json!({ "name": "x", "cron": cron("1"), "only_when_online": true, "enabled": false }),
  )
  .await;
  let sid = s["id"].as_str().unwrap();
  add_task(
    &c,
    sid,
    json!({ "action": "command", "payload": "say online" }),
  )
  .await;
  assert_eq!(run(&c, sid).await, StatusCode::CONFLICT);
  assert!(commands(&c).is_empty());

  c.set_state("running").await;
  // disabled schedules still run on request
  assert_eq!(run(&c, sid).await, StatusCode::OK);
  let s = wait_idle(&c, sid).await;
  assert!(s["last_run_at"].is_string());
  assert_eq!(commands(&c), vec!["say online"]);
}

#[tokio::test]
async fn run_executes_tasks_in_order() {
  let c = setup(command_wings).await;
  let s = create(&c, json!({ "name": "x", "cron": cron("1") })).await;
  let sid = s["id"].as_str().unwrap();
  add_task(&c, sid, json!({ "action": "command", "payload": "first" })).await;
  add_task(
    &c,
    sid,
    json!({ "action": "command", "payload": "second", "offset_seconds": 1 }),
  )
  .await;
  let mut ws = c.ws().await.unwrap();
  assert_eq!(ws().await["type"], "state");

  assert_eq!(run(&c, sid).await, StatusCode::OK);
  // the second task waits for its offset
  c.fake.wait(|r| is(r, Method::POST, "/command")).await;
  let running = timeout(Duration::from_secs(5), async {
    loop {
      let s = get(&c, sid).await;
      if s["running_task"] == 1 {
        return s;
      }
      tokio::time::sleep(Duration::from_millis(10)).await;
    }
  })
  .await
  .expect("second task never waited");
  assert_eq!(running["status"], "running");
  assert_eq!(run(&c, sid).await, StatusCode::CONFLICT);

  let s = wait_idle(&c, sid).await;
  assert_eq!(s["status"], "active");
  assert_eq!(s["running_task"], Value::Null);
  assert_eq!(commands(&c), vec!["first", "second"]);

  let mut frames = Vec::new();
  loop {
    let frame = ws().await;
    if frame["type"] != "schedule" {
      continue;
    }
    assert_eq!(frame["id"], sid);
    frames.push((frame["status"].clone(), frame["task_index"].clone()));
    if frame["status"] == "idle" {
      break;
    }
  }
  assert_eq!(
    frames,
    vec![
      (json!("running"), json!(0)),
      (json!("running"), json!(1)),
      (json!("idle"), Value::Null),
    ]
  );
}

#[tokio::test]
async fn failed_task_stops_unless_continue_on_failure() {
  let c = setup(command_wings).await;
  let s = create(&c, json!({ "name": "x", "cron": cron("1") })).await;
  let sid = s["id"].as_str().unwrap();
  let fail = add_task(&c, sid, json!({ "action": "command", "payload": "fail" })).await;
  add_task(&c, sid, json!({ "action": "command", "payload": "after" })).await;

  assert_eq!(run(&c, sid).await, StatusCode::OK);
  wait_idle(&c, sid).await;
  assert_eq!(commands(&c), vec!["fail"]);

  let resp = c
    .server
    .patch(
      &c.path(&format!(
        "/schedules/{sid}/tasks/{}",
        fail["id"].as_str().unwrap()
      )),
      json!({ "continue_on_failure": true }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(run(&c, sid).await, StatusCode::OK);
  wait_idle(&c, sid).await;
  assert_eq!(commands(&c), vec!["fail", "fail", "after"]);
}

#[tokio::test]
async fn power_and_backup_tasks_reuse_the_routes() {
  let c = setup(command_wings).await;
  c.set_backup_limit(1).await;
  let s = create(&c, json!({ "name": "x", "cron": cron("1") })).await;
  let sid = s["id"].as_str().unwrap();
  add_task(&c, sid, json!({ "action": "power", "payload": "start" })).await;
  // wings has no stream for it: fails right away, the schedule ends
  add_task(&c, sid, json!({ "action": "backup", "payload": "logs/" })).await;
  add_task(&c, sid, json!({ "action": "command", "payload": "never" })).await;

  assert_eq!(run(&c, sid).await, StatusCode::OK);
  wait_idle(&c, sid).await;
  // start bumps the power generation and pushes the spec
  let spec = format!("/api/servers/{}", c.uuid);
  c.fake
    .wait(|r| r.method == Method::PUT && r.path == spec && r.json()["power_generation"] == 1)
    .await;
  let open = c.fake.wait(|r| is(r, Method::POST, "/streams/out")).await;
  assert_eq!(
    open.json(),
    json!({ "source": { "type": "backup", "ignore": ["logs/"] } })
  );
  assert!(commands(&c).is_empty());
}

#[tokio::test]
async fn schedules_need_access() {
  let c = setup(|_| None).await;
  let s = create(&c, json!({ "name": "x", "cron": cron("1") })).await;
  let sid = s["id"].as_str().unwrap();
  let task = add_task(&c, sid, json!({ "action": "command", "payload": "x" })).await;
  let tid = task["id"].as_str().unwrap();

  c.login_with(Some("read")).await;
  assert_eq!(
    c.server.get(&c.path("/schedules")).await.status(),
    StatusCode::OK
  );
  get(&c, sid).await;
  for (path, body) in [
    (
      "/schedules".to_string(),
      json!({ "name": "y", "cron": cron("1") }),
    ),
    (format!("/schedules/{sid}/run"), json!({})),
    (
      format!("/schedules/{sid}/tasks"),
      json!({ "action": "command", "payload": "x" }),
    ),
  ] {
    let resp = c.server.post(&c.path(&path), body).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{path}");
  }
  for path in [
    format!("/schedules/{sid}"),
    format!("/schedules/{sid}/tasks/{tid}"),
  ] {
    let resp = c.server.patch(&c.path(&path), json!({})).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{path}");
    let resp = c.server.delete(&c.path(&path), json!({})).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{path}");
  }

  c.as_admin().await;
  c.login_with(None).await;
  assert_eq!(
    c.server.get(&c.path("/schedules")).await.status(),
    StatusCode::NOT_FOUND
  );
}
