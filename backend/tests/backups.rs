//! Backups against a fake wings: archive streams into local storage, restore
//! streams back out of it.
mod common;

use std::time::Duration;

use axum::{
  http::{Method, StatusCode as AxumStatus},
  response::{IntoResponse, Response},
};
use common::{storage_path, wings::*};
use reqwest::StatusCode;
use serde_json::{Value, json};
use tokio::time::timeout;
use uuid::Uuid;

const OUT: Uuid = Uuid::from_u128(1);
const IN: Uuid = Uuid::from_u128(2);
const SHA: &str = "c0ffee";
/// Chunks wings sends, the first two above the storage part size.
const SIZES: [usize; 3] = [CHUNK, CHUNK, 100];
const RESTORE_CHUNK: usize = 5 * 1024 * 1024;

fn byte(i: usize) -> u8 {
  (i % 253) as u8
}

fn total() -> usize {
  SIZES.iter().sum()
}

fn archive() -> Vec<u8> {
  (0..total()).map(byte).collect()
}

fn json_res(v: Value) -> Option<Response> {
  Some(axum::Json(v).into_response())
}

/// Serves a three chunk backup and accepts restores; `fail_chunk` answers
/// that chunk with an error.
fn backup_wings(fail_chunk: Option<u32>) -> impl Fn(&Req) -> Option<Response> {
  move |r: &Req| {
    if is(r, Method::POST, "/streams/out") {
      return json_res(json!({ "stream": OUT, "chunk_size": CHUNK, "window": 4 }));
    }
    let prefix = format!("/streams/out/{OUT}/");
    if r.method == Method::GET
      && let Some(index) = r.path.split_once(&prefix).map(|(_, i)| i)
    {
      let index: usize = index.parse().unwrap();
      if fail_chunk == Some(index as u32) {
        return Some(wings_error(AxumStatus::INTERNAL_SERVER_ERROR, "internal"));
      }
      if index == SIZES.len() {
        return Some(AxumStatus::NO_CONTENT.into_response());
      }
      let start: usize = SIZES[..index].iter().sum();
      let body: Vec<u8> = (start..start + SIZES[index]).map(byte).collect();
      let mut res = body.into_response();
      if index == SIZES.len() - 1 {
        let h = res.headers_mut();
        h.insert("x-stream-end", "true".parse().unwrap());
        h.insert("x-stream-sha256", SHA.parse().unwrap());
        h.insert("x-stream-size", total().to_string().parse().unwrap());
      }
      return Some(res);
    }
    if is(r, Method::POST, "/streams/in") {
      return json_res(json!({ "stream": IN, "chunk_size": RESTORE_CHUNK, "window": 4 }));
    }
    if is(r, Method::POST, "/complete") {
      return Some((AxumStatus::ACCEPTED, axum::Json(json!({ "op": IN }))).into_response());
    }
    if r.method == Method::DELETE {
      return Some(AxumStatus::NO_CONTENT.into_response());
    }
    None
  }
}

async fn list(c: &Ctx) -> Value {
  let resp = c.server.get(&c.path("/backups")).await;
  assert_eq!(resp.status(), StatusCode::OK);
  resp.json().await.unwrap()
}

/// Waits until the backup has `status`, returns it.
async fn wait_backup(c: &Ctx, id: &Value, status: &str) -> Value {
  timeout(Duration::from_secs(20), async {
    loop {
      let list = list(c).await;
      if let Some(b) = list["backups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| &b["id"] == id && b["status"] == status)
      {
        return b.clone();
      }
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
  })
  .await
  .unwrap_or_else(|_| panic!("backup never became {status}"))
}

async fn create(c: &Ctx, body: Value) -> Value {
  let resp = c.server.post(&c.path("/backups"), body).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let backup: Value = resp.json().await.unwrap();
  let id = backup["id"].clone();
  wait_backup(c, &id, "completed").await
}

#[tokio::test]
async fn create_streams_chunks_into_storage() {
  let c = setup(backup_wings(None)).await;
  c.set_backup_limit(2).await;
  let mut ws = c.ws().await.unwrap();
  assert_eq!(ws().await["type"], "state");

  let resp = c
    .server
    .post(
      &c.path("/backups"),
      json!({ "name": " nightly ", "ignored": "logs/\n\n *.tmp ", "locked": true }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let created: Value = resp.json().await.unwrap();
  assert_eq!(created["status"], "creating");
  assert_eq!(created["name"], "nightly");
  assert_eq!(created["locked"], true);
  let id = created["id"].clone();

  let backup = wait_backup(&c, &id, "completed").await;
  assert_eq!(backup["size_bytes"], total());
  assert_eq!(backup["sha256"], SHA);
  assert_eq!(backup["ignored"], "logs/\n\n *.tmp ");

  // wings got the ignore lines and every chunk acked in order, the last call
  // acks the whole stream
  let open = c.fake.wait(|r| is(r, Method::POST, "/streams/out")).await;
  assert_eq!(
    open.json(),
    json!({ "source": { "type": "backup", "ignore": ["logs/", "*.tmp"] } })
  );
  let chunks: Vec<(String, String)> = c
    .fake
    .requests()
    .into_iter()
    .filter(|r| r.path.contains("/streams/out/"))
    .map(|r| {
      (
        r.path.rsplit('/').next().unwrap().to_string(),
        r.query["ack"].clone(),
      )
    })
    .collect();
  let expected: Vec<(String, String)> = (0..=SIZES.len())
    .map(|i| (i.to_string(), i.to_string()))
    .collect();
  assert_eq!(chunks, expected);

  let file = storage_path().join(format!(
    "backups/{}/{}.tar.zst",
    c.uuid,
    id.as_str().unwrap()
  ));
  assert_eq!(std::fs::read(file).unwrap(), archive());

  // progress frames, then completed
  let mut last = 0;
  loop {
    let frame = ws().await;
    assert_eq!(frame["type"], "backup");
    assert_eq!(frame["id"], id);
    let bytes = frame["bytes"].as_u64().unwrap();
    assert!(bytes >= last);
    last = bytes;
    if frame["status"] == "completed" {
      assert_eq!(bytes, total() as u64);
      break;
    }
    assert_eq!(frame["status"], "creating");
  }

  let list = list(&c).await;
  assert_eq!(list["limit"], 2);
  assert_eq!(list["used"], 1);

  // download streams the stored archive
  let resp = c
    .server
    .get(&c.path(&format!("/backups/{}/download", id.as_str().unwrap())))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert!(
    resp.headers()["content-disposition"]
      .to_str()
      .unwrap()
      .contains("filename=\"nightly.tar.zst\"")
  );
  assert_eq!(resp.bytes().await.unwrap().as_ref(), archive().as_slice());
}

#[tokio::test]
async fn create_defaults_name_and_respects_limit() {
  let c = setup(backup_wings(None)).await;
  // limit 0: backups are off
  let resp = c.server.post(&c.path("/backups"), json!({})).await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);

  c.set_backup_limit(1).await;
  let backup = create(&c, json!({ "name": "  " })).await;
  assert!(
    backup["name"].as_str().unwrap().starts_with("Backup at 20"),
    "{backup}"
  );
  assert_eq!(backup["locked"], false);
  let resp = c.server.post(&c.path("/backups"), json!({})).await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_refuses_long_names() {
  let c = setup(backup_wings(None)).await;
  c.set_backup_limit(1).await;
  let resp = c
    .server
    .post(&c.path("/backups"), json!({ "name": "x".repeat(192) }))
    .await;
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
  assert!(
    c.fake
      .requests()
      .iter()
      .all(|r| !r.path.contains("/streams"))
  );
}

#[tokio::test]
async fn failed_stream_marks_backup_failed() {
  let c = setup(backup_wings(Some(1))).await;
  c.set_backup_limit(1).await;
  let resp = c.server.post(&c.path("/backups"), json!({})).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let id = resp.json::<Value>().await.unwrap()["id"].clone();

  let backup = wait_backup(&c, &id, "failed").await;
  assert!(backup["error"].as_str().is_some());
  assert_eq!(backup["sha256"], Value::Null);
  c.fake
    .wait(|r| r.method == Method::DELETE && r.path.ends_with(&format!("/streams/{OUT}")))
    .await;
  // failed backups do not count, and can not be restored or downloaded
  assert_eq!(list(&c).await["used"], 0);
  let bid = id.as_str().unwrap();
  let resp = c
    .server
    .post(&c.path(&format!("/backups/{bid}/restore")), json!({}))
    .await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);
  let resp = c
    .server
    .get(&c.path(&format!("/backups/{bid}/download")))
    .await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);
  // but deleted
  let resp = c
    .server
    .delete(&c.path(&format!("/backups/{bid}")), json!({}))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn wings_refusing_the_stream_creates_nothing() {
  let c = setup(|r: &Req| {
    is(r, Method::POST, "/streams/out").then(|| wings_error(AxumStatus::CONFLICT, "busy"))
  })
  .await;
  c.set_backup_limit(1).await;
  let resp = c.server.post(&c.path("/backups"), json!({})).await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);
  assert_eq!(list(&c).await["backups"], json!([]));
}

#[tokio::test]
async fn lock_blocks_delete() {
  let c = setup(backup_wings(None)).await;
  c.set_backup_limit(1).await;
  let backup = create(&c, json!({ "locked": true })).await;
  let bid = backup["id"].as_str().unwrap();
  let file = storage_path().join(format!("backups/{}/{bid}.tar.zst", c.uuid));
  assert!(file.exists());

  let del = c.path(&format!("/backups/{bid}"));
  assert_eq!(
    c.server.delete(&del, json!({})).await.status(),
    StatusCode::CONFLICT
  );
  let lock = c.path(&format!("/backups/{bid}/lock"));
  let resp = c.server.post(&lock, json!({ "locked": false })).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(list(&c).await["backups"][0]["locked"], false);
  assert_eq!(
    c.server.delete(&del, json!({})).await.status(),
    StatusCode::OK
  );
  assert!(!file.exists());
  assert_eq!(list(&c).await["backups"], json!([]));
  // gone
  assert_eq!(
    c.server.delete(&del, json!({})).await.status(),
    StatusCode::NOT_FOUND
  );
  let resp = c.server.post(&lock, json!({ "locked": true })).await;
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn restore_pushes_chunks_and_completes() {
  let c = setup(backup_wings(None)).await;
  c.set_backup_limit(1).await;
  let backup = create(&c, json!({})).await;
  let id = backup["id"].clone();
  let bid = id.as_str().unwrap().to_string();
  let mut ws = c.ws().await.unwrap();
  assert_eq!(ws().await["type"], "state");

  let resp = c
    .server
    .post(
      &c.path(&format!("/backups/{bid}/restore")),
      json!({ "truncate": true }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let open = c.fake.wait(|r| is(r, Method::POST, "/streams/in")).await;
  assert_eq!(
    open.json(),
    json!({ "target": { "type": "restore", "truncate": true }, "size": total() })
  );

  let complete = c.fake.wait(|r| is(r, Method::POST, "/complete")).await;
  assert_eq!(
    complete.path,
    format!("/api/servers/{}/streams/in/{IN}/complete", c.uuid)
  );
  assert_eq!(complete.json(), json!({ "sha256": SHA }));
  // busy until wings finishes
  assert_eq!(list(&c).await["backups"][0]["status"], "restoring");
  let resp = c
    .server
    .delete(&c.path(&format!("/backups/{bid}")), json!({}))
    .await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);

  let mut pieces: Vec<_> = c
    .fake
    .requests()
    .into_iter()
    .filter(|r| r.method == Method::PUT && r.path.contains("/streams/in/"))
    .collect();
  pieces.sort_by_key(|r| r.path.rsplit('/').next().unwrap().parse::<u32>().unwrap());
  assert_eq!(pieces.len(), total().div_ceil(RESTORE_CHUNK));
  let sent: Vec<u8> = pieces.iter().flat_map(|r| r.body.to_vec()).collect();
  assert_eq!(sent, archive());

  c.fake.event(json!({
    "type": "OpFinished", "op": IN, "uuid": c.uuid, "ok": true, "error": null, "result": null
  }));
  wait_backup(&c, &id, "completed").await;

  let mut statuses = Vec::new();
  loop {
    let frame = ws().await;
    if frame["type"] != "backup" {
      continue;
    }
    statuses.push(frame["status"].as_str().unwrap().to_string());
    if frame["status"] != "restoring" {
      break;
    }
  }
  assert_eq!(statuses.first().unwrap(), "restoring");
  assert_eq!(statuses.last().unwrap(), "completed");
}

#[tokio::test]
async fn restore_failure_keeps_backup() {
  let c = setup(backup_wings(None)).await;
  c.set_backup_limit(1).await;
  let backup = create(&c, json!({})).await;
  let id = backup["id"].clone();
  let resp = c
    .server
    .post(
      &c.path(&format!("/backups/{}/restore", id.as_str().unwrap())),
      json!({}),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let open = c.fake.wait(|r| is(r, Method::POST, "/streams/in")).await;
  assert_eq!(open.json()["target"]["truncate"], false);
  c.fake.wait(|r| is(r, Method::POST, "/complete")).await;
  c.fake.event(json!({
    "type": "OpFinished", "op": IN, "uuid": c.uuid, "ok": false, "error": "broken", "result": null
  }));
  wait_backup(&c, &id, "completed").await;
}

#[tokio::test]
async fn backups_need_access() {
  let c = setup(backup_wings(None)).await;
  c.set_backup_limit(2).await;
  let backup = create(&c, json!({})).await;
  let bid = backup["id"].as_str().unwrap().to_string();

  c.login_with(Some("read")).await;
  assert_eq!(list(&c).await["used"], 1);
  let resp = c
    .server
    .get(&c.path(&format!("/backups/{bid}/download")))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  for (path, body) in [
    ("/backups".to_string(), json!({})),
    (format!("/backups/{bid}/lock"), json!({ "locked": true })),
    (format!("/backups/{bid}/restore"), json!({})),
  ] {
    let resp = c.server.post(&c.path(&path), body).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{path}");
  }
  let resp = c
    .server
    .delete(&c.path(&format!("/backups/{bid}")), json!({}))
    .await;
  assert_eq!(resp.status(), StatusCode::FORBIDDEN);

  c.as_admin().await;
  c.login_with(None).await;
  let resp = c.server.get(&c.path("/backups")).await;
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
  // another server's backup is not found
  c.as_admin().await;
  let resp = c
    .server
    .get(&c.path(&format!("/backups/{}/download", Uuid::now_v7())))
    .await;
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn concurrent_creates_respect_limit() {
  let c = setup(backup_wings(None)).await;
  c.set_backup_limit(1).await;
  let path = c.path("/backups");
  let statuses = futures_util::future::join_all(
    (0..5).map(|_| async { c.server.post(&path, json!({})).await.status() }),
  )
  .await;
  assert_eq!(
    statuses.iter().filter(|s| **s == StatusCode::OK).count(),
    1,
    "{statuses:?}"
  );
  assert!(
    statuses
      .iter()
      .all(|s| *s == StatusCode::OK || *s == StatusCode::CONFLICT)
  );
  assert_eq!(list(&c).await["backups"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn wings_refusing_the_restore_keeps_backup_completed() {
  let c = setup(|r: &Req| {
    if is(r, Method::POST, "/streams/in") {
      return Some(wings_error(AxumStatus::CONFLICT, "busy"));
    }
    backup_wings(None)(r)
  })
  .await;
  c.set_backup_limit(1).await;
  let backup = create(&c, json!({})).await;
  let bid = backup["id"].as_str().unwrap();
  let resp = c
    .server
    .post(&c.path(&format!("/backups/{bid}/restore")), json!({}))
    .await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);
  assert_eq!(list(&c).await["backups"][0]["status"], "completed");
}

#[tokio::test]
async fn deleting_the_server_deletes_its_backups() {
  let c = setup(backup_wings(None)).await;
  c.set_backup_limit(2).await;
  create(&c, json!({})).await;
  create(&c, json!({})).await;
  let dir = storage_path().join(format!("backups/{}", c.uuid));
  assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

  let resp = c.server.delete(&c.path(""), json!({})).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert!(
    !dir.exists() || std::fs::read_dir(&dir).unwrap().count() == 0,
    "backups left in {dir:?}"
  );
}
