//! Live server routes (power, command, console websocket, files) against a
//! fake wings that serves its websocket and signed REST API.
mod common;

use axum::{
  http::{Method, StatusCode as AxumStatus},
  response::{IntoResponse, Response},
};
use common::{TestServer, create_egg, server_body, server_egg_body, wings::*};
use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;
// ---- power ----

#[tokio::test]
async fn power_follows_status_matrix() {
  let c = setup(|_| None).await;
  let spec = format!("/api/servers/{}", c.uuid);
  let sync = c.fake.wait(|r| r.path == "/api/servers").await;
  let generation = sync.json()["servers"][0]["power_generation"]
    .as_u64()
    .unwrap();

  // offline: only start
  for action in ["stop", "restart", "kill"] {
    assert_eq!(c.power(action).await, StatusCode::CONFLICT, "{action}");
  }
  assert_eq!(c.power("start").await, StatusCode::OK);
  let put = c
    .fake
    .wait(|r| r.method == Method::PUT && r.path == spec)
    .await;
  assert_eq!(put.json()["power"], "Running");
  assert_eq!(put.json()["power_generation"], generation + 1);

  // running: no start; restart is one-shot, stop turns the power off
  c.set_state("running").await;
  assert_eq!(c.power("start").await, StatusCode::CONFLICT);
  assert_eq!(c.power("restart").await, StatusCode::OK);
  let restart = c.fake.wait(|r| is(r, Method::POST, "/power")).await;
  assert_eq!(restart.json(), json!({ "action": "restart" }));
  assert_eq!(c.power("stop").await, StatusCode::OK);
  c.fake
    .wait(|r| r.path == spec && r.json()["power"] == "Stopped")
    .await;

  // stopping: only kill, which sends the powered-off spec first
  c.set_state("stopping").await;
  for action in ["start", "stop", "restart"] {
    assert_eq!(c.power(action).await, StatusCode::CONFLICT, "{action}");
  }
  let before = c.fake.requests().len();
  assert_eq!(c.power("kill").await, StatusCode::OK);
  c.fake
    .wait(|r| is(r, Method::POST, "/power") && r.json()["action"] == "kill")
    .await;
  let after: Vec<_> = c.fake.requests().into_iter().skip(before).collect();
  let put = after.iter().position(|r| r.path == spec).unwrap();
  let kill = after
    .iter()
    .position(|r| is(r, Method::POST, "/power"))
    .unwrap();
  assert!(put < kill);
  assert_eq!(after[put].json()["power"], "Stopped");
  assert_eq!(after[put].json()["power_generation"], generation + 3);

  // starting: stop and kill; crashed: start
  c.set_state("starting").await;
  assert_eq!(c.power("restart").await, StatusCode::CONFLICT);
  assert_eq!(c.power("stop").await, StatusCode::OK);
  c.set_state("crashed").await;
  assert_eq!(c.power("stop").await, StatusCode::CONFLICT);
  assert_eq!(c.power("start").await, StatusCode::OK);

  // suspended refuses everything
  let resp = c
    .server
    .post(&c.path("/suspend"), json!({ "suspended": true }))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  for action in ["start", "stop", "restart", "kill"] {
    assert_eq!(c.power(action).await, StatusCode::CONFLICT, "{action}");
  }
  assert_eq!(c.power("explode").await, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn power_and_command_need_write_access() {
  let c = setup(|_| None).await;
  c.set_state("running").await;
  let command = json!({ "line": "say hi" });

  c.login_with(Some("read")).await;
  assert_eq!(c.power("restart").await, StatusCode::FORBIDDEN);
  let resp = c.server.post(&c.path("/command"), command.clone()).await;
  assert_eq!(resp.status(), StatusCode::FORBIDDEN);

  c.as_admin().await;
  c.login_with(None).await;
  assert_eq!(c.power("restart").await, StatusCode::NOT_FOUND);

  c.as_admin().await;
  c.login_with(Some("write")).await;
  assert_eq!(c.power("restart").await, StatusCode::OK);
}

#[tokio::test]
async fn disconnected_node_refuses_power_and_command() {
  let (server, _) = TestServer::start_with_admin().await;
  let node = common::create_node(&server, "down").await;
  let egg = create_egg(&server, server_egg_body()).await;
  let resp = server.post("/servers", server_body(node, egg, 25565)).await;
  let uuid = common::uuid_of(&resp.json().await.unwrap());

  // status unknown
  let resp = server
    .post(
      &format!("/servers/{uuid}/power"),
      json!({ "action": "start" }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);
  let resp = server
    .post(&format!("/servers/{uuid}/command"), json!({ "line": "x" }))
    .await;
  assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
  let resp = server.get(&format!("/servers/{uuid}/files?path=/")).await;
  assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

// ---- command ----

#[tokio::test]
async fn command_is_forwarded_and_conflict_mapped() {
  let c = setup(|r| {
    is(r, Method::POST, "/command").then(|| match r.json()["line"].as_str() {
      Some("offline") => wings_error(AxumStatus::CONFLICT, "conflict"),
      _ => AxumStatus::NO_CONTENT.into_response(),
    })
  })
  .await;
  let resp = c
    .server
    .post(&c.path("/command"), json!({ "line": "say hi" }))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let req = c.fake.wait(|r| is(r, Method::POST, "/command")).await;
  assert_eq!(req.path, format!("/api/servers/{}/command", c.uuid));
  assert_eq!(req.json(), json!({ "line": "say hi" }));

  let resp = c
    .server
    .post(&c.path("/command"), json!({ "line": "offline" }))
    .await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// ---- live websocket ----

#[tokio::test]
async fn live_ws_fans_out_logs_stats_and_state() {
  let c = setup(|_| None).await;
  let mut next = c.ws().await.unwrap();
  assert_eq!(
    next().await,
    json!({ "type": "state", "status": "offline" })
  );

  // a second watcher (read access) gets the same frames
  c.login_with(Some("read")).await;
  let mut reader = c.ws().await.unwrap();
  assert_eq!(reader().await["type"], "state");

  let other = Uuid::now_v7();
  let line = json!({ "at": "1970-01-01T00:00:01Z", "stream": "console", "line": "hello" });
  c.fake
    .event(json!({ "type": "Logs", "uuid": other, "lines": [line] }));
  c.fake
    .event(json!({ "type": "Logs", "uuid": c.uuid, "lines": [line] }));
  let metrics = |uuid: Uuid, cpu: u64| {
    json!({ "uuid": uuid, "cpu_millicores": cpu, "memory_bytes": 2, "memory_limit_bytes": 3,
      "disk_bytes": 4, "net_rx_bps": 5, "net_tx_bps": 6, "uptime_ms": 7 })
  };
  c.fake
    .event(json!({ "type": "Metrics", "servers": [metrics(other, 9), metrics(c.uuid, 1)] }));
  c.fake.set_state(c.uuid, "running");
  let op = Uuid::now_v7();
  c.fake.event(
    json!({ "type": "OpFinished", "op": op, "uuid": c.uuid, "ok": true,
    "error": null, "result": null }),
  );

  let expected = [
    json!({ "type": "logs", "lines": [line] }),
    json!({ "type": "stats", "cpu_millicores": 1, "cpu_limit_millicores": 1500,
      "memory_bytes": 2, "memory_limit_bytes": 3, "disk_bytes": 4,
      "disk_limit_bytes": 2 * 1024 * 1024, "net_rx_bps": 5, "net_tx_bps": 6, "uptime_ms": 7 }),
    json!({ "type": "state", "status": "running" }),
    json!({ "type": "op_finished", "op": op, "ok": true, "error": null }),
  ];
  for frame in &expected {
    assert_eq!(&next().await, frame);
    assert_eq!(&reader().await, frame);
  }
}

#[tokio::test]
async fn live_ws_needs_access() {
  let c = setup(|_| None).await;
  c.login_with(None).await;
  let err = c.ws().await.err().expect("connected without access");
  let tokio_tungstenite::tungstenite::Error::Http(res) = err else {
    panic!("unexpected error {err:?}");
  };
  assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn live_ws_rejects_foreign_origins() {
  let c = setup(|_| None).await;
  let port = c.server.port;
  for origin in [
    "http://evil.example.com".to_string(),
    format!("http://localhost:{}", port + 1),
  ] {
    let err = c
      .ws_from(Some(&origin))
      .await
      .err()
      .expect("foreign origin");
    let tokio_tungstenite::tungstenite::Error::Http(res) = err else {
      panic!("unexpected error {err:?}");
    };
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "{origin}");
  }
  let own = format!("http://localhost:{port}");
  let mut ws = c.ws_from(Some(&own)).await.unwrap();
  assert_eq!(ws().await["type"], "state");
}

// ---- files ----

fn entry(name: &str, kind: &str, size: u64) -> Value {
  json!({ "name": name, "kind": kind, "size": size, "mode": 420,
    "modified": "1970-01-01T00:00:00Z", "mime": null })
}

/// Byte `i` of the big file.
fn big_byte(i: usize) -> u8 {
  (i % 251) as u8
}
const BIG: usize = CHUNK + 100;

fn files_wings(r: &Req) -> Option<Response> {
  let path = r.query.get("path").map(String::as_str);
  let json = |v: Value| Some(axum::Json(v).into_response());
  if is(r, Method::GET, "/files/list") {
    return match path {
      Some("/secret") => Some(wings_error(AxumStatus::FORBIDDEN, "denied")),
      _ => json(json!([entry("a.yml", "file", 5)])),
    };
  }
  if is(r, Method::GET, "/files/stat") {
    return match path {
      Some("/a.yml") => json(entry("a.yml", "file", 5)),
      Some("/grown.log") => json(entry("grown.log", "file", 10)),
      Some("/empty.txt") => json(entry("empty.txt", "file", 0)),
      Some("/huge.log") => json(entry("huge.log", "file", 2 * 1024 * 1024)),
      Some("/big.bin") => json(entry("big bin.bin", "file", BIG as u64)),
      Some("/dir") => json(entry("dir", "dir", 0)),
      _ => Some(wings_error(AxumStatus::NOT_FOUND, "not_found")),
    };
  }
  if is(r, Method::GET, "/files/content") {
    let range = r.headers.get("range").unwrap();
    // the editor asks for one byte past its limit
    let partial = |body: &'static str, content_range: &str| {
      assert_eq!(range, "bytes=0-1048576");
      let mut res = (AxumStatus::PARTIAL_CONTENT, body).into_response();
      res
        .headers_mut()
        .insert("content-range", content_range.parse().unwrap());
      Some(res)
    };
    match path {
      Some("/a.yml") => return partial("k: v\n", "bytes 0-4/5"),
      // grew past the limit since the stat
      Some("/grown.log") => return partial("x", "bytes 0-0/2097152"),
      _ => (),
    }
    let (start, end) = range
      .to_str()
      .unwrap()
      .strip_prefix("bytes=")
      .unwrap()
      .split_once('-')
      .unwrap();
    let (start, end): (usize, usize) = (start.parse().unwrap(), end.parse().unwrap());
    assert!(end - start < CHUNK);
    let body: Vec<u8> = (start..=end).map(big_byte).collect();
    return Some((AxumStatus::PARTIAL_CONTENT, body).into_response());
  }
  if is(r, Method::POST, "/files/copy") {
    return json(entry("a copy.yml", "file", 5));
  }
  if is(r, Method::POST, "/streams/in") {
    return json(json!({ "stream": Uuid::nil(), "chunk_size": CHUNK, "window": 4 }));
  }
  if is(r, Method::POST, "/complete") {
    return Some(
      (
        AxumStatus::ACCEPTED,
        axum::Json(json!({ "op": Uuid::nil() })),
      )
        .into_response(),
    );
  }
  if r.method == Method::POST || r.method == Method::DELETE {
    return Some(AxumStatus::NO_CONTENT.into_response());
  }
  None
}

#[tokio::test]
async fn files_are_proxied() {
  let c = setup(files_wings).await;
  let s = &c.server;

  let resp = s.get(&c.path("/files?path=plugins/../")).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(
    resp.json::<Value>().await.unwrap(),
    json!([entry("a.yml", "file", 5)])
  );
  let list = c.fake.wait(|r| is(r, Method::GET, "/files/list")).await;
  assert_eq!(list.query["path"], "/");
  // wings errors keep their meaning
  let resp = s.get(&c.path("/files?path=/secret")).await;
  assert_eq!(resp.status(), StatusCode::FORBIDDEN);

  // editor content
  let resp = s.get(&c.path("/files/content?path=/a.yml")).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(resp.text().await.unwrap(), "k: v\n");
  // empty: wings is not asked for a range it would reject
  let resp = s.get(&c.path("/files/content?path=/empty.txt")).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(resp.text().await.unwrap(), "");
  assert!(
    !c.fake
      .requests()
      .iter()
      .any(|r| is(r, Method::GET, "/files/content") && r.query["path"] == "/empty.txt")
  );
  for (path, status) in [
    ("/huge.log", StatusCode::PAYLOAD_TOO_LARGE),
    ("/grown.log", StatusCode::PAYLOAD_TOO_LARGE),
    ("/dir", StatusCode::BAD_REQUEST),
    ("/missing", StatusCode::NOT_FOUND),
  ] {
    let resp = s.get(&c.path(&format!("/files/content?path={path}"))).await;
    assert_eq!(resp.status(), status, "{path}");
  }

  let resp = s
    .put_bytes(&c.path("/files/content?path=/a.yml"), b"k: w\n".to_vec())
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let put = c.fake.wait(|r| is(r, Method::PUT, "/files/content")).await;
  assert_eq!(
    (put.query["path"].as_str(), &put.body[..]),
    ("/a.yml", &b"k: w\n"[..])
  );

  // mutations
  let post = async |route: &str, body: Value| s.post(&c.path(route), body).await;
  assert_eq!(
    post("/files/folder", json!({ "path": "/plugins" }))
      .await
      .status(),
    StatusCode::OK
  );
  let mkdir = c.fake.wait(|r| is(r, Method::POST, "/files/mkdir")).await;
  assert_eq!(mkdir.json(), json!({ "path": "/plugins" }));

  assert_eq!(
    post("/files/file", json!({ "path": "/a.yml" }))
      .await
      .status(),
    StatusCode::CONFLICT
  );
  assert_eq!(
    post("/files/file", json!({ "path": "/new.txt" }))
      .await
      .status(),
    StatusCode::OK
  );
  let new = c
    .fake
    .wait(|r| is(r, Method::PUT, "/files/content") && r.query["path"] == "/new.txt")
    .await;
  assert!(new.body.is_empty());

  let resp = post(
    "/files/rename",
    json!({ "from": "/a.yml", "to": "/plugins/../b.yml" }),
  )
  .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let rename = c.fake.wait(|r| is(r, Method::POST, "/files/rename")).await;
  assert_eq!(
    rename.json(),
    json!({ "moves": [{ "from": "/a.yml", "to": "/b.yml" }] })
  );

  let resp = post("/files/copy", json!({ "path": "a.yml" })).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(resp.json::<Value>().await.unwrap()["name"], "a copy.yml");

  assert_eq!(
    post("/files/delete", json!({ "paths": ["/a", "b/"] }))
      .await
      .status(),
    StatusCode::OK
  );
  let delete = c.fake.wait(|r| is(r, Method::POST, "/files/delete")).await;
  assert_eq!(delete.json(), json!({ "paths": ["/a", "/b"] }));
  // the root itself is never touched
  assert_eq!(
    post("/files/delete", json!({ "paths": ["/x/.."] }))
      .await
      .status(),
    StatusCode::BAD_REQUEST
  );
  assert_eq!(
    post("/files/rename", json!({ "from": "/", "to": "/x" }))
      .await
      .status(),
    StatusCode::BAD_REQUEST
  );
}

#[tokio::test]
async fn files_need_access() {
  let c = setup(files_wings).await;
  c.login_with(Some("read")).await;
  let s = &c.server;
  assert_eq!(
    s.get(&c.path("/files?path=/")).await.status(),
    StatusCode::OK
  );
  assert_eq!(
    s.get(&c.path("/files/download?path=/a.yml")).await.status(),
    StatusCode::OK
  );
  for (route, body) in [
    ("/files/folder", json!({ "path": "/x" })),
    ("/files/delete", json!({ "paths": ["/x"] })),
    ("/files/upload", json!({ "path": "/x", "size": 1 })),
  ] {
    assert_eq!(
      s.post(&c.path(route), body).await.status(),
      StatusCode::FORBIDDEN,
      "{route}"
    );
  }
  let resp = s.put_bytes(&c.path("/files/content?path=/x"), vec![]).await;
  assert_eq!(resp.status(), StatusCode::FORBIDDEN);

  c.as_admin().await;
  c.login_with(None).await;
  assert_eq!(
    c.server.get(&c.path("/files?path=/")).await.status(),
    StatusCode::NOT_FOUND
  );
}

#[tokio::test]
async fn upload_chunks_are_forwarded() {
  let c = setup(files_wings).await;
  let s = &c.server;
  let stream = Uuid::nil();

  let resp = s
    .post(
      &c.path("/files/upload"),
      json!({ "path": "up.bin", "size": CHUNK + 3 }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(
    resp.json::<Value>().await.unwrap(),
    json!({ "upload": stream, "chunk_size": CHUNK })
  );
  let open = c.fake.wait(|r| is(r, Method::POST, "/streams/in")).await;
  assert_eq!(
    open.json(),
    json!({ "target": { "type": "file", "path": "/up.bin" }, "size": CHUNK + 3 })
  );

  // a full chunk fits the body limit
  let chunk: Vec<u8> = (0..CHUNK).map(big_byte).collect();
  let resp = s
    .put_bytes(&c.path(&format!("/files/upload/{stream}/0")), chunk.clone())
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let resp = s
    .put_bytes(&c.path(&format!("/files/upload/{stream}/1")), vec![1, 2, 3])
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let first = c
    .fake
    .wait(|r| r.path == format!("/api/servers/{}/streams/in/{stream}/0", c.uuid))
    .await;
  assert_eq!(first.body, chunk);
  let last = c
    .fake
    .wait(|r| r.path.ends_with(&format!("{stream}/1")))
    .await;
  assert_eq!(&last.body[..], &[1, 2, 3]);

  // answers once wings finished the op, with its error
  let c = &c;
  let complete = |sha256: &'static str, ok: bool| async move {
    let path = c.path(&format!("/files/upload/{stream}/complete"));
    let (resp, _) = tokio::join!(s.post(&path, json!({ "sha256": sha256 })), async {
      c.fake
        .wait(|r| is(r, Method::POST, "/complete") && r.json() == json!({ "sha256": sha256 }))
        .await;
      let error = (!ok).then_some("Checksum mismatch");
      c.fake.event(json!({
        "type": "OpFinished", "op": stream, "uuid": c.uuid, "ok": ok, "error": error, "result": null
      }));
    });
    resp
  };
  let resp = complete("ab", true).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(
    resp.json::<Value>().await.unwrap(),
    json!({ "op": Uuid::nil() })
  );
  let resp = complete("cd", false).await;
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

  let resp = s
    .delete(&c.path(&format!("/files/upload/{stream}")), json!({}))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let cancel = c.fake.wait(|r| r.method == Method::DELETE).await;
  assert_eq!(
    cancel.path,
    format!("/api/servers/{}/streams/{stream}", c.uuid)
  );
}

#[tokio::test]
async fn download_streams_range_pieces() {
  let c = setup(files_wings).await;
  let resp = c.server.get(&c.path("/files/download?path=/big.bin")).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let headers = resp.headers().clone();
  assert_eq!(headers["content-length"], BIG.to_string().as_str());
  assert_eq!(
    headers["content-disposition"],
    "attachment; filename=\"big bin.bin\"; filename*=UTF-8''big%20bin.bin"
  );
  let body = resp.bytes().await.unwrap();
  assert_eq!(body.len(), BIG);
  assert!(body.iter().enumerate().all(|(i, b)| *b == big_byte(i)));
  let ranges: Vec<String> = c
    .fake
    .requests()
    .iter()
    .filter(|r| is(r, Method::GET, "/files/content"))
    .map(|r| r.headers["range"].to_str().unwrap().to_string())
    .collect();
  assert_eq!(
    ranges,
    [
      format!("bytes=0-{}", CHUNK - 1),
      format!("bytes={CHUNK}-{}", BIG - 1)
    ]
  );

  let resp = c.server.get(&c.path("/files/download?path=/dir")).await;
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
