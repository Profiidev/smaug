mod common;

use std::{
  sync::atomic::{AtomicU16, Ordering},
  time::Duration,
};

use common::{
  JWT_COOKIE_NAME, MEMBER_PASSWORD, TestServer, create_egg, create_group, create_node, create_user,
  egg_body, server_body, server_egg_body, update_body, uuid_of,
};
use entity::{sea_orm_active_enums::ServerInstallState, server};
use futures_util::StreamExt;
use reqwest::{Response, StatusCode};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, Database, EntityTrait};
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use uuid::Uuid;

static PORT: AtomicU16 = AtomicU16::new(20000);

fn port() -> u16 {
  PORT.fetch_add(1, Ordering::Relaxed)
}

struct Ctx {
  server: TestServer,
  admin: Uuid,
  node: Uuid,
  node2: Uuid,
  egg: Uuid,
  /// Set by `ctx_with_db` for states the API can not reach.
  db_url: Option<String>,
}

async fn ctx() -> Ctx {
  ctx_on(TestServer::start().await, None).await
}

/// Ctx on a sqlite file the test can open next to the server.
async fn ctx_with_db() -> Ctx {
  let (server, url) = TestServer::start_with_db_file().await;
  ctx_on(server, Some(url)).await
}

async fn ctx_on(server: TestServer, db_url: Option<String>) -> Ctx {
  let (server, admin) = TestServer::with_admin(server).await;
  let node = create_node(&server, "node-1").await;
  let node2 = create_node(&server, "node-2").await;
  let egg = create_egg(&server, server_egg_body()).await;
  Ctx {
    server,
    admin,
    node,
    node2,
    egg,
    db_url,
  }
}

impl Ctx {
  fn body(&self) -> Value {
    server_body(self.node, self.egg, port())
  }

  async fn create(&self, body: Value) -> Uuid {
    let resp = self.server.post("/servers", body).await;
    assert_eq!(resp.status(), StatusCode::OK);
    uuid_of(&resp.json().await.unwrap())
  }

  async fn info(&self, uuid: Uuid) -> Value {
    let resp = self.server.get(&format!("/servers/{uuid}")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.unwrap()
  }

  async fn put(&self, uuid: Uuid, body: Value) -> Response {
    self.server.put(&format!("/servers/{uuid}"), body).await
  }

  async fn action(&self, uuid: Uuid, action: &str, body: Value) -> StatusCode {
    self
      .server
      .post(&format!("/servers/{uuid}/{action}"), body)
      .await
      .status()
  }

  async fn as_admin(&self) {
    self.server.clear_cookies();
    let resp = self.server.login("admin@example.com", "hunter2pass").await;
    assert_eq!(resp.status(), StatusCode::OK);
  }

  async fn as_user(&self, email: &str) {
    self.server.clear_cookies();
    let resp = self.server.login(email, MEMBER_PASSWORD).await;
    assert_eq!(resp.status(), StatusCode::OK);
  }

  async fn list(&self) -> Vec<Uuid> {
    let resp = self.server.get("/servers").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let list: Value = resp.json().await.unwrap();
    list["servers"]
      .as_array()
      .unwrap()
      .iter()
      .map(uuid_of)
      .collect()
  }
}

/// `field -> message` pairs of a field error body, after asserting the status.
async fn field_errors(resp: Response, status: StatusCode) -> Vec<(String, String)> {
  assert_eq!(resp.status(), status);
  let body: Value = resp.json().await.unwrap();
  body["errors"]
    .as_array()
    .unwrap()
    .iter()
    .map(|e| {
      (
        e["field"].as_str().unwrap().to_string(),
        e["message"].as_str().unwrap().to_string(),
      )
    })
    .collect()
}

async fn fields(resp: Response, status: StatusCode) -> Vec<String> {
  field_errors(resp, status)
    .await
    .into_iter()
    .map(|(f, _)| f)
    .collect()
}

fn values(info: &Value) -> Vec<(String, String)> {
  info["variables"]
    .as_array()
    .unwrap()
    .iter()
    .map(|v| {
      (
        v["env_variable"].as_str().unwrap().to_string(),
        v["value"].as_str().unwrap().to_string(),
      )
    })
    .collect()
}

fn value(info: &Value, env: &str) -> String {
  values(info)
    .into_iter()
    .find(|(e, _)| e == env)
    .unwrap_or_else(|| panic!("{env} missing"))
    .1
}

// ---- crud ----

#[tokio::test]
async fn server_crud_flow() {
  let c = ctx().await;
  assert!(c.list().await.is_empty());

  let mut body = c.body();
  body["name"] = json!("  survival  ");
  body["owner"] = json!(c.admin);
  body["allocations"] = json!([
    { "ip": "10.0.0.5", "port": 25565 },
    { "ip": " 0:0:0:0:0:0:0:1 ", "port": 25566 }
  ]);
  body["environment"] = json!({ "FIRST": "hello", "MAX_PLAYERS": "50" });
  body["limits"] = json!({ "cpu": 1500, "memory": 4096, "disk": 10240 });
  body["backup_limit"] = json!(3);
  body["stop_timeout"] = json!(30);
  body["restart_on_crash"] = json!(false);
  body["skip_scripts"] = json!(false);
  body["start_on_completion"] = json!(false);
  let uuid = c.create(body).await;

  let list: Value = c.server.get("/servers").await.json().await.unwrap();
  assert_eq!(
    list,
    json!({ "servers": [{
      "uuid": uuid, "name": "survival", "status": "installing",
      "node": { "uuid": c.node, "name": "node-1" },
      "egg": { "uuid": c.egg, "name": "Paper" },
      "owner": { "uuid": c.admin, "name": "admin" },
      "primary_allocation": { "ip": "10.0.0.5", "port": 25565 },
      "extra_allocations": [{ "ip": "::1", "port": 25566 }]
    }] })
  );

  let mut info = c.info(uuid).await;
  assert!(info["created_at"].is_string());
  info.as_object_mut().unwrap().remove("created_at");
  assert_eq!(
    info,
    json!({
      "uuid": uuid, "name": "survival", "status": "installing", "error": null, "access": "admin",
      "owner": { "uuid": c.admin, "name": "admin" },
      "node": { "uuid": c.node, "name": "node-1" },
      "egg": { "uuid": c.egg, "name": "Paper" },
      "allocations": [{ "ip": "10.0.0.5", "port": 25565 }, { "ip": "::1", "port": 25566 }],
      "startup": "java -jar a.jar", "startup_overridden": false, "image": "ghcr.io/java:21",
      "install_state": "installing", "suspended": false, "start_on_completion": false,
      "limits": { "cpu": 1500, "memory": 4096, "disk": 10240 }, "backup_limit": 3,
      "stop_timeout": 30, "restart_on_crash": false,
      "variables": [
        { "name": "First", "description": "first var", "env_variable": "FIRST", "default_value": "1", "value": "hello", "rules": ["required", "string"], "editable": true },
        { "name": "Max players", "description": "", "env_variable": "MAX_PLAYERS", "default_value": "20", "value": "50", "rules": ["required", "integer", "between:1,100"], "editable": true },
        { "name": "Locked", "description": "", "env_variable": "LOCKED", "default_value": "locked", "value": "locked", "rules": [], "editable": true },
        { "name": "Hidden", "description": "", "env_variable": "HIDDEN", "default_value": "secret", "value": "secret", "rules": ["nullable"], "editable": true }
      ],
      "grants": { "users": [], "groups": [] }
    })
  );

  // update everything; missing variables keep their value
  let mut update = update_body(&info);
  update["name"] = json!("creative");
  update["owner"] = Value::Null;
  update["allocations"] = json!([{ "ip": "10.0.0.6", "port": 30000 }]);
  update["startup"] = json!("custom");
  update["startup_overridden"] = json!(true);
  update["image"] = json!("img:custom");
  update["environment"] = json!({ "LOCKED": "changed", "HIDDEN": "" });
  update["limits"] = json!({ "cpu": 0, "memory": 1, "disk": 2 });
  update["backup_limit"] = json!(0);
  update["stop_timeout"] = json!(3600);
  update["restart_on_crash"] = json!(true);
  assert_eq!(c.put(uuid, update).await.status(), StatusCode::OK);
  let info = c.info(uuid).await;
  assert_eq!(info["name"], "creative");
  assert_eq!(info["owner"], Value::Null);
  assert_eq!(
    info["allocations"],
    json!([{ "ip": "10.0.0.6", "port": 30000 }])
  );
  assert_eq!(
    (
      &info["startup"],
      &info["startup_overridden"],
      &info["image"]
    ),
    (&json!("custom"), &json!(true), &json!("img:custom"))
  );
  assert_eq!(info["limits"], json!({ "cpu": 0, "memory": 1, "disk": 2 }));
  assert_eq!(info["backup_limit"], 0);
  assert_eq!(
    (&info["stop_timeout"], &info["restart_on_crash"]),
    (&json!(3600), &json!(true))
  );
  assert_eq!(
    values(&info),
    [
      ("FIRST".into(), "hello".into()),
      ("MAX_PLAYERS".into(), "50".into()),
      ("LOCKED".into(), "changed".into()),
      ("HIDDEN".into(), "".into())
    ]
  );
  // state is not part of the update
  assert_eq!(info["install_state"], "installing");

  // the old pairs are free again, delete is blocked while installing
  c.create(server_body(c.node, c.egg, 25565)).await;
  assert_eq!(
    c.server
      .delete(&format!("/servers/{uuid}"), json!({}))
      .await
      .status(),
    StatusCode::CONFLICT
  );
  assert_eq!(
    c.action(uuid, "install", json!({ "installed": true }))
      .await,
    StatusCode::OK
  );
  assert_eq!(
    c.server
      .delete(&format!("/servers/{uuid}"), json!({}))
      .await
      .status(),
    StatusCode::OK
  );
  assert_eq!(
    c.server.get(&format!("/servers/{uuid}")).await.status(),
    StatusCode::NOT_FOUND
  );
  assert_eq!(c.list().await.len(), 1);
}

#[tokio::test]
async fn create_defaults_and_install_state() {
  let c = ctx().await;
  let mut body = c.body();
  body.as_object_mut().unwrap().remove("environment");
  body["environment"] = json!({});
  let uuid = c.create(body.clone()).await;
  let info = c.info(uuid).await;
  assert_eq!(info["install_state"], "installed");
  // no wings connected, the runtime state is unknown
  assert_eq!(info["status"], "unknown");
  assert_eq!(info["owner"], Value::Null);
  // every variable gets its egg default
  assert_eq!(
    values(&info),
    [
      ("FIRST".into(), "1".into()),
      ("MAX_PLAYERS".into(), "20".into()),
      ("LOCKED".into(), "locked".into()),
      ("HIDDEN".into(), "secret".into())
    ]
  );

  // environment can be omitted entirely
  let mut body = c.body();
  body.as_object_mut().unwrap().remove("environment");
  c.create(body).await;
}

#[tokio::test]
async fn missing_servers_are_not_found() {
  let c = ctx().await;
  let missing = Uuid::now_v7();
  let path = format!("/servers/{missing}");
  assert_eq!(c.server.get(&path).await.status(), StatusCode::NOT_FOUND);
  let body = update_body(&json!({
    "name": "x", "owner": null, "egg": { "uuid": c.egg }, "allocations": [{ "ip": "10.0.0.1", "port": 2000 }],
    "startup": "x", "startup_overridden": false, "image": "x", "variables": [],
    "limits": { "cpu": 0, "memory": 0, "disk": 0 }, "backup_limit": 0,
    "stop_timeout": 60, "restart_on_crash": true
  }));
  assert_eq!(c.put(missing, body).await.status(), StatusCode::NOT_FOUND);
  assert_eq!(
    c.server.delete(&path, json!({})).await.status(),
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    c.action(missing, "reinstall", json!({})).await,
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    c.action(missing, "install", json!({ "installed": true }))
      .await,
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    c.action(missing, "suspend", json!({ "suspended": true }))
      .await,
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    c.action(
      missing,
      "transfer",
      json!({ "node": c.node2, "ip": "10.0.0.1", "port": 2000 })
    )
    .await,
    StatusCode::NOT_FOUND
  );
}

#[tokio::test]
async fn endpoints_require_auth() {
  let c = ctx().await;
  let uuid = c.create(c.body()).await;
  let path = format!("/servers/{uuid}");
  let info = c.info(uuid).await;
  let check = async |status: StatusCode| {
    assert_eq!(c.server.get("/servers").await.status(), status);
    assert_eq!(c.server.get("/servers/options").await.status(), status);
    assert_eq!(c.server.post("/servers", c.body()).await.status(), status);
    assert_eq!(c.server.get(&path).await.status(), status);
    assert_eq!(c.put(uuid, update_body(&info)).await.status(), status);
    assert_eq!(c.server.delete(&path, json!({})).await.status(), status);
    assert_eq!(c.action(uuid, "reinstall", json!({})).await, status);
    assert_eq!(
      c.action(uuid, "install", json!({ "installed": false }))
        .await,
      status
    );
    assert_eq!(
      c.action(uuid, "suspend", json!({ "suspended": true }))
        .await,
      status
    );
    assert_eq!(
      c.action(
        uuid,
        "transfer",
        json!({ "node": c.node2, "ip": "10.0.0.1", "port": 2000 })
      )
      .await,
      status
    );
  };
  // centaurus rejects a missing cookie with 400 and a bogus token with 401
  c.server.clear_cookies();
  check(StatusCode::BAD_REQUEST).await;
  c.server.set_cookie(JWT_COOKIE_NAME, "not-a-jwt");
  check(StatusCode::UNAUTHORIZED).await;
}

// ---- validation ----

type Mutation = Box<dyn Fn(&mut Value)>;

#[tokio::test]
async fn create_validation_errors() {
  let c = ctx().await;
  let missing = Uuid::now_v7();
  let cases: Vec<(Mutation, &[&str])> = vec![
    (Box::new(|b| b["name"] = json!("")), &["name"]),
    (Box::new(|b| b["name"] = json!("   ")), &["name"]),
    (Box::new(|b| b["name"] = json!("x".repeat(192))), &["name"]),
    (Box::new(|b| b["allocations"] = json!([])), &["allocations"]),
    (
      Box::new(|b| b["allocations"][0]["ip"] = json!("nope")),
      &["allocations.0.ip"],
    ),
    (
      Box::new(|b| b["allocations"][0]["port"] = json!(1023)),
      &["allocations.0.port"],
    ),
    (
      Box::new(|b| b["allocations"][0]["port"] = json!(65536)),
      &["allocations.0.port"],
    ),
    (
      Box::new(|b| b["allocations"] = json!([{ "ip": "", "port": 0 }])),
      &["allocations.0.ip", "allocations.0.port"],
    ),
    (
      Box::new(|b| {
        b["allocations"] = json!([{ "ip": "::1", "port": 2000 }, { "ip": "0:0::1", "port": 2000 }])
      }),
      &["allocations.1"],
    ),
    (Box::new(|b| b["startup"] = json!(" ")), &["startup"]),
    (Box::new(|b| b["image"] = json!("")), &["image"]),
    (
      Box::new(|b| b["limits"]["cpu"] = json!(-1)),
      &["limits.cpu"],
    ),
    (
      Box::new(|b| b["limits"]["memory"] = json!(-1)),
      &["limits.memory"],
    ),
    (
      Box::new(|b| b["limits"]["disk"] = json!(-1)),
      &["limits.disk"],
    ),
    (
      Box::new(|b| b["backup_limit"] = json!(-1)),
      &["backup_limit"],
    ),
    (
      Box::new(|b| b["stop_timeout"] = json!(0)),
      &["stop_timeout"],
    ),
    (
      Box::new(|b| b["stop_timeout"] = json!(3601)),
      &["stop_timeout"],
    ),
    (Box::new(move |b| b["owner"] = json!(missing)), &["owner"]),
    (Box::new(move |b| b["node"] = json!(missing)), &["node"]),
    (Box::new(move |b| b["egg"] = json!(missing)), &["egg"]),
    (
      Box::new(|b| b["environment"] = json!({ "NOPE": "x" })),
      &["variables.NOPE"],
    ),
    (
      Box::new(|b| b["environment"] = json!({ "FIRST": "" })),
      &["variables.FIRST"],
    ),
    (
      Box::new(|b| b["environment"] = json!({ "MAX_PLAYERS": "101" })),
      &["variables.MAX_PLAYERS"],
    ),
    (
      Box::new(|b| {
        b["name"] = json!("");
        b["backup_limit"] = json!(-1);
        b["environment"] = json!({ "MAX_PLAYERS": "x" });
      }),
      &["name", "backup_limit", "variables.MAX_PLAYERS"],
    ),
  ];
  for (mutate, expected) in cases {
    let mut body = c.body();
    mutate(&mut body);
    let mut got = fields(
      c.server.post("/servers", body.clone()).await,
      StatusCode::BAD_REQUEST,
    )
    .await;
    got.sort();
    let mut expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(got, expected, "{body}");
  }

  // messages come from the egg rules
  let mut body = c.body();
  body["environment"] = json!({ "MAX_PLAYERS": "lots" });
  assert_eq!(
    field_errors(
      c.server.post("/servers", body).await,
      StatusCode::BAD_REQUEST
    )
    .await,
    [(
      "variables.MAX_PLAYERS".into(),
      "The Max players field must be an integer.".into()
    )]
  );

  // boundaries are accepted
  let mut body = c.body();
  body["name"] = json!("é".repeat(191));
  body["allocations"] =
    json!([{ "ip": "10.1.0.1", "port": 1024 }, { "ip": "10.1.0.1", "port": 65535 }]);
  c.create(body).await;

  // nothing invalid was stored
  assert_eq!(c.list().await.len(), 1);
}

#[tokio::test]
async fn update_validation_errors() {
  let c = ctx().await;
  let uuid = c.create(c.body()).await;
  let info = c.info(uuid).await;
  for (field, value, expected) in [
    ("name", json!(""), "name"),
    ("allocations", json!([]), "allocations"),
    ("image", json!(""), "image"),
    ("owner", json!(Uuid::now_v7()), "owner"),
    ("egg", json!(Uuid::now_v7()), "egg"),
    (
      "environment",
      json!({ "MAX_PLAYERS": "0" }),
      "variables.MAX_PLAYERS",
    ),
    // admins get the unknown key named, non-admins a plain 403
    ("environment", json!({ "NOPE": "x" }), "variables.NOPE"),
    (
      "grants",
      json!({ "users": [{ "uuid": Uuid::now_v7(), "access": "read" }] }),
      "grants.users.0",
    ),
    (
      "grants",
      json!({ "groups": [{ "uuid": Uuid::now_v7(), "access": "write" }] }),
      "grants.groups.0",
    ),
  ] {
    let mut body = update_body(&info);
    body[field] = value;
    assert_eq!(
      fields(c.put(uuid, body).await, StatusCode::BAD_REQUEST).await,
      [expected],
      "{field}"
    );
  }

  // duplicate grants
  let mut body = update_body(&info);
  body["grants"] = json!({ "users": [
    { "uuid": c.admin, "access": "read" },
    { "uuid": c.admin, "access": "write" }
  ] });
  assert_eq!(
    field_errors(c.put(uuid, body).await, StatusCode::BAD_REQUEST).await,
    [("grants.users.1".into(), "Duplicate grant.".into())]
  );

  // unchanged values are not re-validated, so an egg rule change does not
  // block saving, but a changed one is
  let mut egg = server_egg_body();
  egg["uuid"] = json!(c.egg);
  egg["variables"][0]["rules"] = json!(["required", "integer"]);
  assert_eq!(c.server.put("/eggs", egg).await.status(), StatusCode::OK);
  assert_eq!(
    c.put(uuid, update_body(&info)).await.status(),
    StatusCode::OK
  );
  let mut body = update_body(&info);
  body["environment"]["FIRST"] = json!("still text");
  assert_eq!(
    fields(c.put(uuid, body).await, StatusCode::BAD_REQUEST).await,
    ["variables.FIRST"]
  );
  assert_eq!(value(&c.info(uuid).await, "FIRST"), "hello");
}

#[tokio::test]
async fn egg_change_resets_variables() {
  let c = ctx().await;
  let uuid = c.create(c.body()).await;
  let mut other = egg_body();
  other["name"] = json!("Other");
  let other = create_egg(&c.server, other).await;

  let mut body = update_body(&c.info(uuid).await);
  body["egg"] = json!(other);
  // variables of the old egg are unknown to the new one
  let mut got = fields(c.put(uuid, body.clone()).await, StatusCode::BAD_REQUEST).await;
  got.sort();
  assert_eq!(
    got,
    [
      "variables.HIDDEN",
      "variables.LOCKED",
      "variables.MAX_PLAYERS"
    ]
  );
  body["environment"] = json!({ "LATE": "late" });
  assert_eq!(c.put(uuid, body).await.status(), StatusCode::OK);
  let info = c.info(uuid).await;
  assert_eq!(info["egg"], json!({ "uuid": other, "name": "Other" }));
  assert_eq!(
    values(&info),
    [
      ("FIRST".into(), "1".into()),
      ("LATE".into(), "late".into()),
      ("UNSORTED".into(), "".into())
    ]
  );
}

// ---- allocations ----

#[tokio::test]
async fn allocation_conflicts() {
  let c = ctx().await;
  let mut body = c.body();
  body["allocations"] =
    json!([{ "ip": "10.0.0.5", "port": 25565 }, { "ip": "10.0.0.5", "port": 25566 }]);
  let a = c.create(body).await;

  // same node: per-row 409
  let mut body = c.body();
  body["allocations"] =
    json!([{ "ip": "10.0.0.6", "port": 25565 }, { "ip": "10.0.0.5", "port": 25566 }]);
  assert_eq!(
    field_errors(
      c.server.post("/servers", body.clone()).await,
      StatusCode::CONFLICT
    )
    .await,
    [(
      "allocations.1".into(),
      "IP and port are already in use on this node.".into()
    )]
  );
  // the same IP written differently is the same pair
  let mut ipv6 = c.body();
  ipv6["allocations"] = json!([{ "ip": "::1", "port": 2000 }]);
  c.create(ipv6.clone()).await;
  ipv6["allocations"] = json!([{ "ip": "0:0::1", "port": 2000 }]);
  assert_eq!(
    fields(c.server.post("/servers", ipv6).await, StatusCode::CONFLICT).await,
    ["allocations.0"]
  );

  // another node is fine
  body["node"] = json!(c.node2);
  c.create(body).await;

  // update keeping its own pairs is fine, taking another server's is not
  let info = c.info(a).await;
  assert_eq!(c.put(a, update_body(&info)).await.status(), StatusCode::OK);
  let mut other = c.body();
  other["allocations"] = json!([{ "ip": "10.0.0.7", "port": 25570 }]);
  let d = c.create(other).await;
  let mut update = update_body(&c.info(d).await);
  update["allocations"] =
    json!([{ "ip": "10.0.0.7", "port": 25570 }, { "ip": "10.0.0.5", "port": 25565 }]);
  assert_eq!(
    fields(c.put(d, update.clone()).await, StatusCode::CONFLICT).await,
    ["allocations.1"]
  );
  // swapping its own pairs around is fine
  let mut swap = update_body(&info);
  swap["allocations"] =
    json!([{ "ip": "10.0.0.5", "port": 25566 }, { "ip": "10.0.0.5", "port": 25565 }]);
  assert_eq!(c.put(a, swap).await.status(), StatusCode::OK);
  assert_eq!(
    c.info(a).await["allocations"][0],
    json!({ "ip": "10.0.0.5", "port": 25566 })
  );

  // transfer onto a pair used on the target node
  let resp = c
    .server
    .post(
      &format!("/servers/{d}/transfer"),
      json!({ "node": c.node2, "ip": "10.0.0.6", "port": 25565 }),
    )
    .await;
  assert_eq!(fields(resp, StatusCode::CONFLICT).await, ["port"]);

  // deleting frees the pairs
  c.server.delete(&format!("/servers/{a}"), json!({})).await;
  assert_eq!(c.put(d, update).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn wildcard_allocation_conflicts() {
  let c = ctx().await;
  let with = |node: Uuid, ip: &str, port: i32| {
    let mut body = c.body();
    body["node"] = json!(node);
    body["allocations"] = json!([{ "ip": ip, "port": port }]);
    body
  };
  let conflict = |resp| async {
    assert_eq!(
      field_errors(resp, StatusCode::CONFLICT).await,
      [(
        "allocations.0".to_string(),
        "IP and port are already in use on this node.".to_string()
      )]
    );
  };

  // specific then wildcard, and the reverse
  c.create(with(c.node, "127.0.0.1", 4000)).await;
  conflict(
    c.server
      .post("/servers", with(c.node, "0.0.0.0", 4000))
      .await,
  )
  .await;
  conflict(c.server.post("/servers", with(c.node, "::", 4000)).await).await;
  c.create(with(c.node, "0.0.0.0", 4001)).await;
  conflict(
    c.server
      .post("/servers", with(c.node, "127.0.0.1", 4001))
      .await,
  )
  .await;
  // 0.0.0.0 does not cover IPv6, another port or another node
  c.create(with(c.node, "::1", 4001)).await;
  c.create(with(c.node, "127.0.0.1", 4002)).await;
  c.create(with(c.node2, "0.0.0.0", 4000)).await;
  // :: covers IPv4 too
  c.create(with(c.node, "::", 4003)).await;
  conflict(
    c.server
      .post("/servers", with(c.node, "10.0.0.1", 4003))
      .await,
  )
  .await;
  // in-request overlap is a 400
  let mut body = c.body();
  body["allocations"] =
    json!([{ "ip": "10.0.0.1", "port": 4004 }, { "ip": "0.0.0.0", "port": 4004 }]);
  assert_eq!(
    field_errors(
      c.server.post("/servers", body).await,
      StatusCode::BAD_REQUEST
    )
    .await,
    [(
      "allocations.1".to_string(),
      "Overlaps another allocation on the same port.".to_string()
    )]
  );

  // update: own wildcard swap is fine, overlapping another server is not
  let own = c.create(with(c.node, "10.0.0.9", 4005)).await;
  let mut update = update_body(&c.info(own).await);
  update["allocations"] = json!([{ "ip": "0.0.0.0", "port": 4005 }]);
  assert_eq!(c.put(own, update.clone()).await.status(), StatusCode::OK);
  update["allocations"] = json!([{ "ip": "0.0.0.0", "port": 4000 }]);
  conflict(c.put(own, update).await).await;

  // transfer onto a port covered by a wildcard on the target node
  let resp = c
    .server
    .post(
      &format!("/servers/{own}/transfer"),
      json!({ "node": c.node2, "ip": "10.0.0.1", "port": 4000 }),
    )
    .await;
  assert_eq!(fields(resp, StatusCode::CONFLICT).await, ["port"]);
  let resp = c
    .server
    .post(
      &format!("/servers/{own}/transfer"),
      json!({ "node": c.node2, "ip": "::1", "port": 4000 }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
}

// ---- actions ----

#[derive(Clone, Copy, Debug)]
enum State {
  Offline,
  Installing,
  NotInstalled,
  InstallFailed,
  Suspended,
}

impl Ctx {
  async fn in_state(&self, state: State) -> Uuid {
    let mut body = self.body();
    body["skip_scripts"] = json!(!matches!(state, State::Installing));
    let uuid = self.create(body).await;
    match state {
      State::NotInstalled => assert_eq!(
        self
          .action(uuid, "install", json!({ "installed": false }))
          .await,
        StatusCode::OK
      ),
      State::Suspended => assert_eq!(
        self
          .action(uuid, "suspend", json!({ "suspended": true }))
          .await,
        StatusCode::OK
      ),
      // only the node reports failed installs, so set it in the database
      State::InstallFailed => {
        let db = Database::connect(self.db_url.as_deref().expect("ctx_with_db"))
          .await
          .unwrap();
        let mut model: server::ActiveModel = server::Entity::find_by_id(uuid)
          .one(&db)
          .await
          .unwrap()
          .unwrap()
          .into();
        model.install_state = Set(ServerInstallState::InstallFailed);
        model.update(&db).await.unwrap();
      }
      _ => {}
    }
    let expected = match state {
      // installed, the runtime state is unknown without a connected wings
      State::Offline => "unknown",
      State::Installing => "installing",
      State::NotInstalled => "not_installed",
      State::InstallFailed => "install_failed",
      State::Suspended => "suspended",
    };
    assert_eq!(self.info(uuid).await["status"], expected);
    uuid
  }
}

#[tokio::test]
async fn action_matrix() {
  let c = ctx_with_db().await;
  const OK: StatusCode = StatusCode::OK;
  const NO: StatusCode = StatusCode::CONFLICT;
  // (action, body, offline, installing, not_installed, install_failed, suspended)
  let reinstall = json!({});
  let installed = json!({ "installed": true });
  let not_installed = json!({ "installed": false });
  let suspend = json!({ "suspended": true });
  let unsuspend = json!({ "suspended": false });
  let rows: Vec<(&str, &Value, [StatusCode; 5])> = vec![
    ("reinstall", &reinstall, [OK, NO, OK, OK, NO]),
    ("install", &installed, [NO, OK, OK, OK, NO]),
    ("install", &not_installed, [OK, NO, NO, NO, OK]),
    ("suspend", &suspend, [OK, OK, OK, OK, NO]),
    ("suspend", &unsuspend, [NO, NO, NO, NO, OK]),
    ("transfer", &Value::Null, [OK, NO, OK, OK, NO]),
    ("delete", &Value::Null, [OK, NO, OK, OK, OK]),
  ];
  let states = [
    State::Offline,
    State::Installing,
    State::NotInstalled,
    State::InstallFailed,
    State::Suspended,
  ];
  for (action, body, expected) in rows {
    for (state, expected) in states.into_iter().zip(expected) {
      let uuid = c.in_state(state).await;
      let before = c.info(uuid).await;
      let status = match action {
        "delete" => c
          .server
          .delete(&format!("/servers/{uuid}"), json!({}))
          .await
          .status(),
        "transfer" => {
          c.action(
            uuid,
            "transfer",
            json!({ "node": c.node2, "ip": "10.0.0.9", "port": port() }),
          )
          .await
        }
        _ => c.action(uuid, action, body.clone()).await,
      };
      assert_eq!(status, expected, "{action} {body} in {state:?}");
      if status != OK {
        // a refused action changes nothing
        assert_eq!(c.info(uuid).await, before, "{action} {state:?}");
        continue;
      }
      let resp = c.server.get(&format!("/servers/{uuid}")).await;
      if action == "delete" {
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        continue;
      }
      let after: Value = resp.json().await.unwrap();
      match (action, body.clone()) {
        ("reinstall", _) => assert_eq!(after["install_state"], "installing"),
        ("install", b) if b == installed => assert_eq!(after["install_state"], "installed"),
        ("install", _) => assert_eq!(after["install_state"], "not_installed"),
        ("suspend", b) => assert_eq!(after["suspended"], b["suspended"]),
        ("transfer", _) => assert_eq!(after["node"]["uuid"], json!(c.node2)),
        _ => unreachable!(),
      }
    }
  }
  // suspended wins over install state in the status
  let uuid = c.in_state(State::Installing).await;
  c.action(uuid, "suspend", suspend.clone()).await;
  let info = c.info(uuid).await;
  assert_eq!(
    (&info["status"], &info["install_state"]),
    (&json!("suspended"), &json!("installing"))
  );
}

#[tokio::test]
async fn power_follows_actions_and_survives_edits() {
  let c = ctx_with_db().await;
  let db = Database::connect(c.db_url.as_deref().unwrap())
    .await
    .unwrap();
  let power = async |uuid: Uuid| {
    let m = server::Entity::find_by_id(uuid)
      .one(&db)
      .await
      .unwrap()
      .unwrap();
    (m.desired_running, m.power_generation, m.suspended)
  };

  // skipped scripts with start on completion run right away
  let mut body = c.body();
  body["skip_scripts"] = json!(true);
  body["start_on_completion"] = json!(true);
  let uuid = c.create(body.clone()).await;
  assert_eq!(power(uuid).await, (true, 0, false));

  // not installed turns it off, nothing may run on missing files
  body["allocations"] = json!([{ "ip": "10.0.0.7", "port": 30001 }]);
  let other = c.create(body).await;
  assert_eq!(
    c.action(other, "install", json!({ "installed": false }))
      .await,
    StatusCode::OK
  );
  assert_eq!(power(other).await, (false, 1, false));

  // a reinstall turns it off, with a new generation for wings
  assert_eq!(c.action(uuid, "reinstall", json!({})).await, StatusCode::OK);
  assert_eq!(power(uuid).await, (false, 1, false));

  // an edit never undoes a suspend
  assert_eq!(
    c.action(uuid, "install", json!({ "installed": true }))
      .await,
    StatusCode::OK
  );
  assert_eq!(
    c.action(uuid, "suspend", json!({ "suspended": true }))
      .await,
    StatusCode::OK
  );
  let resp = c.put(uuid, update_body(&c.info(uuid).await)).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(power(uuid).await, (false, 2, true));
}

#[tokio::test]
async fn transfer_moves_node_and_drops_extras() {
  let c = ctx().await;
  let mut body = c.body();
  body["allocations"] =
    json!([{ "ip": "10.0.0.5", "port": 3000 }, { "ip": "10.0.0.5", "port": 3001 }]);
  let uuid = c.create(body).await;
  let path = format!("/servers/{uuid}/transfer");

  for (req, field) in [
    (
      json!({ "node": c.node, "ip": "10.0.0.1", "port": 3000 }),
      "node",
    ),
    (
      json!({ "node": Uuid::now_v7(), "ip": "10.0.0.1", "port": 3000 }),
      "node",
    ),
    (json!({ "node": c.node2, "ip": "x", "port": 3000 }), "ip"),
    (
      json!({ "node": c.node2, "ip": "10.0.0.1", "port": 80 }),
      "port",
    ),
  ] {
    assert_eq!(
      fields(c.server.post(&path, req).await, StatusCode::BAD_REQUEST).await,
      [field]
    );
  }

  let resp = c
    .server
    .post(
      &path,
      json!({ "node": c.node2, "ip": "::ffff:10.0.0.1", "port": 3000 }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let info = c.info(uuid).await;
  assert_eq!(info["node"], json!({ "uuid": c.node2, "name": "node-2" }));
  assert_eq!(
    info["allocations"],
    json!([{ "ip": "::ffff:10.0.0.1", "port": 3000 }])
  );

  // the old node's pairs are free, the new one's are taken
  let mut body = c.body();
  body["allocations"] = json!([{ "ip": "10.0.0.5", "port": 3001 }]);
  c.create(body).await;
  let mut body = c.body();
  body["node"] = json!(c.node2);
  body["allocations"] = json!([{ "ip": "::ffff:10.0.0.1", "port": 3000 }]);
  assert_eq!(
    c.server.post("/servers", body).await.status(),
    StatusCode::CONFLICT
  );
}

// ---- access ----

struct Users {
  owner: String,
  user_read: String,
  user_write: String,
  group_read: String,
  group_write: String,
  none: String,
  viewer: String,
  editor: String,
}

/// Two servers (offline, installing) with the same owner and grants.
async fn access_setup(c: &Ctx) -> (Users, Uuid, Uuid) {
  let (owner, owner_email) = create_user(&c.server, "owner").await;
  let (user_read, user_read_email) = create_user(&c.server, "uread").await;
  let (user_write, user_write_email) = create_user(&c.server, "uwrite").await;
  let (group_read_user, group_read) = create_user(&c.server, "gread").await;
  let (group_write_user, group_write) = create_user(&c.server, "gwrite").await;
  let (_, none) = create_user(&c.server, "none").await;
  let (viewer_user, viewer) = create_user(&c.server, "viewer").await;
  let (editor_user, editor) = create_user(&c.server, "editor").await;
  let read_group = create_group(&c.server, json!([]), &[group_read_user]).await;
  let write_group = create_group(&c.server, json!([]), &[group_write_user]).await;
  create_group(&c.server, json!(["server:view"]), &[viewer_user]).await;
  create_group(&c.server, json!(["server:edit"]), &[editor_user]).await;

  let mut ids = Vec::new();
  for skip in [true, false] {
    let mut body = c.body();
    body["owner"] = json!(owner);
    body["skip_scripts"] = json!(skip);
    let uuid = c.create(body).await;
    let mut update = update_body(&c.info(uuid).await);
    update["grants"] = json!({
      "users": [{ "uuid": user_read, "access": "read" }, { "uuid": user_write, "access": "write" }],
      "groups": [{ "uuid": read_group, "access": "read" }, { "uuid": write_group, "access": "write" }]
    });
    assert_eq!(c.put(uuid, update).await.status(), StatusCode::OK);
    ids.push(uuid);
  }

  let grants = &c.info(ids[0]).await["grants"];
  assert_eq!(grants["users"].as_array().unwrap().len(), 2);
  assert!(
    grants["users"]
      .as_array()
      .unwrap()
      .contains(&json!({ "uuid": user_write, "name": "uwrite", "access": "write" }))
  );
  let groups = grants["groups"].as_array().unwrap();
  assert_eq!(groups.len(), 2);
  assert!(
    groups
      .iter()
      .any(|g| g["uuid"] == json!(read_group) && g["access"] == "read")
  );

  let users = Users {
    owner: owner_email,
    user_read: user_read_email,
    user_write: user_write_email,
    group_read,
    group_write,
    none,
    viewer,
    editor,
  };
  (users, ids[0], ids[1])
}

#[tokio::test]
async fn access_matrix() {
  let c = ctx().await;
  let (u, offline, installing) = access_setup(&c).await;
  // a server nobody but admins can see
  let private = c.create(c.body()).await;

  const NF: StatusCode = StatusCode::NOT_FOUND;
  const FB: StatusCode = StatusCode::FORBIDDEN;
  const OK: StatusCode = StatusCode::OK;
  const CF: StatusCode = StatusCode::CONFLICT;
  for (email, access) in [
    (&u.owner, Some("write")),
    (&u.user_read, Some("read")),
    (&u.user_write, Some("write")),
    (&u.group_read, Some("read")),
    (&u.group_write, Some("write")),
    (&u.none, None),
    (&u.viewer, Some("read")),
    (&u.editor, Some("admin")),
  ] {
    c.as_user(email).await;
    let rank = match access {
      None => 0,
      Some("read") => 1,
      Some("write") => 2,
      _ => 3,
    };
    // pick the expected status by access rank
    let by = |none, read, write, admin| [none, read, write, admin][rank];

    let mut list = c.list().await;
    list.sort();
    let mut expected = match rank {
      0 => vec![],
      // viewers see every server
      1 if email == &u.viewer => vec![offline, installing, private],
      1 | 2 => vec![offline, installing],
      _ => vec![offline, installing, private],
    };
    expected.sort();
    assert_eq!(list, expected, "{email} list");

    let resp = c.server.get(&format!("/servers/{offline}")).await;
    assert_eq!(resp.status(), by(NF, OK, OK, OK), "{email} info");
    assert_eq!(
      c.server.get(&format!("/servers/{private}")).await.status(),
      if email == &u.viewer || rank == 3 {
        OK
      } else {
        NF
      },
      "{email} private"
    );
    let info: Value = if rank > 0 {
      let info: Value = resp.json().await.unwrap();
      assert_eq!(info["access"], json!(access.unwrap()), "{email}");
      assert_eq!(info["grants"].is_object(), rank == 3, "{email} grants");
      info
    } else {
      c.as_admin().await;
      let info = c.info(offline).await;
      c.as_user(email).await;
      info
    };

    // write fields
    let mut body = update_body(&info);
    body["name"] = json!(format!("renamed by {email}"));
    assert_eq!(
      c.put(offline, body).await.status(),
      by(NF, FB, OK, OK),
      "{email} rename"
    );
    // admin fields
    let mut body = update_body(&info);
    body["backup_limit"] = json!(5);
    assert_eq!(
      c.put(offline, body).await.status(),
      by(NF, FB, FB, OK),
      "{email} backup"
    );

    // actions on the installing server: passing the access check ends in 409
    assert_eq!(
      c.action(installing, "reinstall", json!({})).await,
      by(NF, FB, CF, CF),
      "{email} reinstall"
    );
    assert_eq!(
      c.action(installing, "install", json!({ "installed": false }))
        .await,
      by(NF, FB, FB, CF),
      "{email} install"
    );
    assert_eq!(
      c.action(installing, "suspend", json!({ "suspended": false }))
        .await,
      by(NF, FB, FB, CF),
      "{email} suspend"
    );
    assert_eq!(
      c.action(
        installing,
        "transfer",
        json!({ "node": c.node2, "ip": "10.0.0.1", "port": 5000 })
      )
      .await,
      by(NF, FB, FB, CF),
      "{email} transfer"
    );
    assert_eq!(
      c.server
        .delete(&format!("/servers/{installing}"), json!({}))
        .await
        .status(),
      by(NF, FB, FB, CF),
      "{email} delete"
    );
    // create and options are admin only
    assert_eq!(
      c.server.post("/servers", c.body()).await.status(),
      by(FB, FB, FB, OK),
      "{email} create"
    );
    assert_eq!(
      c.server.get("/servers/options").await.status(),
      by(FB, FB, FB, OK),
      "{email} options"
    );
  }

  // write users can reinstall
  c.as_user(&u.user_write).await;
  assert_eq!(c.action(offline, "reinstall", json!({})).await, OK);
}

#[tokio::test]
async fn write_user_field_restrictions() {
  let c = ctx().await;
  let (u, offline, _) = access_setup(&c).await;
  let mut other = egg_body();
  other["name"] = json!("Other");
  let other_egg = create_egg(&c.server, other).await;
  let admin_info = c.info(offline).await;

  c.as_user(&u.user_write).await;
  let info = c.info(offline).await;
  // hidden variables are not shown, only editable ones are editable
  let vars: Vec<_> = info["variables"]
    .as_array()
    .unwrap()
    .iter()
    .map(|v| {
      (
        v["env_variable"].as_str().unwrap(),
        v["editable"].as_bool().unwrap(),
      )
    })
    .collect();
  assert_eq!(
    vars,
    [("FIRST", true), ("MAX_PLAYERS", true), ("LOCKED", false)]
  );
  assert_eq!(info["grants"], Value::Null);

  let forbidden: Vec<(&str, Value)> = vec![
    ("owner", json!(c.admin)),
    ("owner", Value::Null),
    ("egg", json!(other_egg)),
    ("image", json!("evil:latest")),
    ("limits", json!({ "cpu": 1, "memory": 0, "disk": 0 })),
    ("backup_limit", json!(1)),
    ("allocations", json!([{ "ip": "10.0.0.5", "port": 9999 }])),
    ("grants", json!({ "users": [], "groups": [] })),
    ("environment", json!({ "LOCKED": "mine" })),
    ("environment", json!({ "HIDDEN": "mine" })),
    // the right hidden value is refused too, so a 403 can not confirm guesses
    ("environment", json!({ "HIDDEN": "secret" })),
    // unknown keys look like hidden ones, no hint which variables exist
    ("environment", json!({ "NOPE": "x" })),
  ];
  for (field, value) in forbidden {
    let mut body = update_body(&info);
    body[field] = value.clone();
    assert_eq!(
      c.put(offline, body).await.status(),
      StatusCode::FORBIDDEN,
      "{field} {value}"
    );
  }

  // invalid editable values are a 400
  let mut body = update_body(&info);
  body["environment"]["MAX_PLAYERS"] = json!("x");
  assert_eq!(
    fields(c.put(offline, body).await, StatusCode::BAD_REQUEST).await,
    ["variables.MAX_PLAYERS"]
  );

  // write fields; locked values sent unchanged are fine, hidden ones keep their value
  let mut body = update_body(&info);
  body["name"] = json!("mine");
  body["startup"] = json!("java -jar custom.jar");
  body["startup_overridden"] = json!(true);
  body["environment"]["FIRST"] = json!("changed");
  body["environment"]["MAX_PLAYERS"] = json!("99");
  assert_eq!(c.put(offline, body).await.status(), StatusCode::OK);

  c.as_admin().await;
  let after = c.info(offline).await;
  assert_eq!(after["name"], "mine");
  assert_eq!(after["startup"], "java -jar custom.jar");
  assert_eq!(after["startup_overridden"], true);
  assert_eq!(
    values(&after),
    [
      ("FIRST".into(), "changed".into()),
      ("MAX_PLAYERS".into(), "99".into()),
      ("LOCKED".into(), "locked".into()),
      ("HIDDEN".into(), "secret".into())
    ]
  );
  // admin-only fields and grants untouched
  assert_eq!(after["owner"], admin_info["owner"]);
  assert_eq!(after["grants"], admin_info["grants"]);
  assert_eq!(after["image"], admin_info["image"]);

  // read users see view-only variables
  c.as_user(&u.user_read).await;
  let info = c.info(offline).await;
  assert!(
    info["variables"]
      .as_array()
      .unwrap()
      .iter()
      .all(|v| v["editable"] == false)
  );
  assert_eq!(info["variables"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn write_user_can_update_server_without_owner() {
  let c = ctx().await;
  let (owner, _) = create_user(&c.server, "owner").await;
  let (writer, email) = create_user(&c.server, "writer").await;
  let mut body = c.body();
  body["owner"] = json!(owner);
  let uuid = c.create(body).await;
  let mut update = update_body(&c.info(uuid).await);
  update["grants"] = json!({ "users": [{ "uuid": writer, "access": "write" }] });
  assert_eq!(c.put(uuid, update).await.status(), StatusCode::OK);
  assert_eq!(
    c.server
      .delete("/user/management", json!({ "uuid": owner }))
      .await
      .status(),
    StatusCode::OK
  );

  c.as_user(&email).await;
  let info = c.info(uuid).await;
  assert_eq!(info["owner"], Value::Null);
  let mut update = update_body(&info);
  update["name"] = json!("orphan");
  assert_eq!(c.put(uuid, update).await.status(), StatusCode::OK);
  assert_eq!(c.info(uuid).await["name"], "orphan");
}

// ---- options ----

#[tokio::test]
async fn options_list_everything_needed_by_the_wizard() {
  let c = ctx().await;
  let (_, email) = create_user(&c.server, "member").await;
  let group = create_group(&c.server, json!(["server:view"]), &[]).await;

  let raw = c.server.get("/servers/options").await.text().await.unwrap();
  let options: Value = serde_json::from_str(&raw).unwrap();
  let names = |key: &str| -> Vec<Value> {
    let mut v: Vec<_> = options[key]
      .as_array()
      .unwrap()
      .iter()
      .map(|x| x["uuid"].clone())
      .collect();
    v.sort_by_key(|x| x.to_string());
    v
  };
  assert_eq!(options["users"].as_array().unwrap().len(), 2);
  assert!(names("users").contains(&json!(c.admin)));
  assert!(names("groups").contains(&json!(group)));
  let mut nodes = vec![json!(c.node), json!(c.node2)];
  nodes.sort_by_key(|x| x.to_string());
  assert_eq!(names("nodes"), nodes);
  let egg = &options["eggs"][0];
  assert_eq!(egg["uuid"], json!(c.egg));
  assert_eq!(egg["name"], "Paper");
  assert_eq!(egg["variables"].as_array().unwrap().len(), 4);
  assert_eq!(egg["variables"][1]["env_variable"], "MAX_PLAYERS");
  // ordered maps keep their order
  #[derive(serde::Deserialize)]
  struct Options {
    eggs: Vec<Egg>,
  }
  #[derive(serde::Deserialize)]
  struct Egg {
    docker_images: indexmap::IndexMap<String, String>,
    startup_commands: indexmap::IndexMap<String, String>,
  }
  let parsed: Options = serde_json::from_str(&raw).unwrap();
  assert_eq!(
    parsed.eggs[0].docker_images.keys().collect::<Vec<_>>(),
    ["Zulu 21", "Alpha 17", "Mid 11"]
  );
  assert_eq!(
    parsed.eggs[0].startup_commands.values().collect::<Vec<_>>(),
    ["java -jar a.jar", "java -jar b.jar"]
  );

  c.as_user(&email).await;
  assert_eq!(
    c.server.get("/servers/options").await.status(),
    StatusCode::FORBIDDEN
  );
}

// ---- egg and node interplay ----

#[tokio::test]
async fn egg_update_propagates_to_servers() {
  let c = ctx().await;
  let mut body = c.body();
  body["environment"] = json!({ "FIRST": "custom" });
  let plain = c.create(body).await;
  let mut body = c.body();
  body["startup"] = json!("my own");
  body["startup_overridden"] = json!(true);
  let overridden = c.create(body).await;
  let mut next = updates(&c.server).await;

  // new default command, LOCKED dropped, NEW added
  let mut egg = server_egg_body();
  egg["uuid"] = json!(c.egg);
  egg["startup_commands"] = json!({ "New": "java -jar new.jar", "Old": "java -jar a.jar" });
  let vars = egg["variables"].as_array_mut().unwrap();
  vars.remove(2);
  vars.push(json!({ "name": "New", "description": "", "env_variable": "NEW", "default_value": "fresh", "user_viewable": true, "user_editable": true, "rules": [], "sort": 5 }));
  assert_eq!(
    c.server.put("/eggs", egg.clone()).await.status(),
    StatusCode::OK
  );

  let mut got = vec![next().await, next().await];
  got.sort_by_key(|v| v["uuid"].to_string());
  let mut expected = vec![
    json!({ "type": "Servers", "uuid": plain }),
    json!({ "type": "Servers", "uuid": overridden }),
  ];
  expected.sort_by_key(|v| v["uuid"].to_string());
  assert_eq!(got, expected);

  let info = c.info(plain).await;
  assert_eq!(info["startup"], "java -jar new.jar");
  assert_eq!(
    values(&info),
    [
      ("FIRST".into(), "custom".into()),
      ("MAX_PLAYERS".into(), "20".into()),
      ("HIDDEN".into(), "secret".into()),
      ("NEW".into(), "fresh".into())
    ]
  );
  assert_eq!(c.info(overridden).await["startup"], "my own");

  // NEW got a stored row (a later default change does not leak in) and
  // LOCKED's row was dropped (re-adding it starts from the new default)
  egg["variables"][3]["default_value"] = json!("other");
  egg["variables"]
    .as_array_mut()
    .unwrap()
    .push(json!({ "name": "Locked", "description": "", "env_variable": "LOCKED", "default_value": "again", "user_viewable": true, "user_editable": false, "rules": [], "sort": 6 }));
  assert_eq!(c.server.put("/eggs", egg).await.status(), StatusCode::OK);
  let info = c.info(overridden).await;
  assert_eq!(value(&info, "NEW"), "fresh");
  assert_eq!(value(&info, "LOCKED"), "again");
}

#[tokio::test]
async fn nodes_and_eggs_in_use_can_not_be_deleted() {
  let c = ctx().await;
  let uuid = c.create(c.body()).await;
  assert_eq!(
    c.server
      .delete("/nodes", json!({ "uuid": c.node }))
      .await
      .status(),
    StatusCode::CONFLICT
  );
  assert_eq!(
    c.server
      .delete("/eggs", json!({ "uuid": c.egg }))
      .await
      .status(),
    StatusCode::CONFLICT
  );
  // unused node is still deletable
  assert_eq!(
    c.server
      .delete("/nodes", json!({ "uuid": c.node2 }))
      .await
      .status(),
    StatusCode::OK
  );
  c.server
    .delete(&format!("/servers/{uuid}"), json!({}))
    .await;
  assert_eq!(
    c.server
      .delete("/nodes", json!({ "uuid": c.node }))
      .await
      .status(),
    StatusCode::OK
  );
  assert_eq!(
    c.server
      .delete("/eggs", json!({ "uuid": c.egg }))
      .await
      .status(),
    StatusCode::OK
  );
}

#[tokio::test]
async fn user_and_group_deletes_cascade() {
  let c = ctx().await;
  let (user, email) = create_user(&c.server, "owner").await;
  let (member, _) = create_user(&c.server, "member").await;
  let group = create_group(&c.server, json!([]), &[]).await;
  let mut body = c.body();
  body["owner"] = json!(user);
  let uuid = c.create(body).await;
  let mut update = update_body(&c.info(uuid).await);
  update["grants"] = json!({
    "users": [{ "uuid": member, "access": "read" }, { "uuid": user, "access": "read" }],
    "groups": [{ "uuid": group, "access": "write" }]
  });
  assert_eq!(c.put(uuid, update).await.status(), StatusCode::OK);

  c.as_user(&email).await;
  assert_eq!(c.list().await, [uuid]);
  c.as_admin().await;

  assert_eq!(
    c.server
      .delete("/user/management", json!({ "uuid": user }))
      .await
      .status(),
    StatusCode::OK
  );
  let info = c.info(uuid).await;
  assert_eq!(info["owner"], Value::Null);
  assert_eq!(
    info["grants"]["users"],
    json!([{ "uuid": member, "name": "member", "access": "read" }])
  );

  assert_eq!(
    c.server
      .delete("/group", json!({ "uuid": group }))
      .await
      .status(),
    StatusCode::OK
  );
  assert_eq!(c.info(uuid).await["grants"]["groups"], json!([]));
}

// ---- updates ----

/// Connect to the updater websocket and return a fn yielding the next `Servers` message.
async fn updates(server: &TestServer) -> impl AsyncFnMut() -> Value {
  let mut req = format!("ws://localhost:{}/api/ws/updater", server.port)
    .into_client_request()
    .unwrap();
  req
    .headers_mut()
    .insert("Cookie", server.cookie_header().unwrap().parse().unwrap());
  let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();

  async move || {
    loop {
      let msg = timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("no update received")
        .unwrap()
        .unwrap();
      if let Message::Text(text) = msg {
        let v: Value = serde_json::from_str(&text).unwrap();
        if v["type"] == "Servers" {
          return v;
        }
      }
    }
  }
}

#[tokio::test]
async fn server_changes_are_broadcast() {
  let c = ctx().await;
  let mut next = updates(&c.server).await;
  let uuid = c.create(c.body()).await;
  let msg = json!({ "type": "Servers", "uuid": uuid });
  assert_eq!(next().await, msg);

  assert_eq!(
    c.put(uuid, update_body(&c.info(uuid).await)).await.status(),
    StatusCode::OK
  );
  assert_eq!(next().await, msg);
  for (action, body) in [
    ("install", json!({ "installed": false })),
    ("reinstall", json!({})),
    ("install", json!({ "installed": true })),
    ("suspend", json!({ "suspended": true })),
    ("suspend", json!({ "suspended": false })),
    (
      "transfer",
      json!({ "node": c.node2, "ip": "10.0.0.1", "port": 4000 }),
    ),
  ] {
    assert_eq!(
      c.action(uuid, action, body).await,
      StatusCode::OK,
      "{action}"
    );
    assert_eq!(next().await, msg, "{action}");
  }
  c.server
    .delete(&format!("/servers/{uuid}"), json!({}))
    .await;
  assert_eq!(next().await, msg);
}
