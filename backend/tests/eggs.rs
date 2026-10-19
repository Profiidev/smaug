mod common;

use std::time::Duration;

use common::{JWT_COOKIE_NAME, TestServer, egg_body, egg_envs, egg_keys, unique};
use futures_util::StreamExt;
use reqwest::StatusCode;
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use uuid::Uuid;

async fn create(server: &TestServer, body: Value) -> Uuid {
  let resp = server.post("/eggs", body).await;
  assert_eq!(resp.status(), StatusCode::OK);
  resp.json::<Value>().await.unwrap()["uuid"]
    .as_str()
    .unwrap()
    .parse()
    .unwrap()
}

async fn info(server: &TestServer, uuid: Uuid) -> (String, Value) {
  let resp = server.get(&format!("/eggs/{uuid}")).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let raw = resp.text().await.unwrap();
  let value = serde_json::from_str(&raw).unwrap();
  (raw, value)
}

#[tokio::test]
async fn egg_crud_flow() {
  let (server, _) = TestServer::start_with_admin().await;

  let list: Value = server.get("/eggs").await.json().await.unwrap();
  assert_eq!(list, json!({ "eggs": [] }));

  let uuid = create(&server, egg_body()).await;
  let mut other = egg_body();
  other["name"] = json!("Another");
  other["description"] = Value::Null;
  let other_uuid = create(&server, other).await;

  // list is ordered by name and only carries the summary fields
  let list: Value = server.get("/eggs").await.json().await.unwrap();
  assert_eq!(
    list,
    json!({ "eggs": [
      { "uuid": other_uuid, "name": "Another", "author": "dev@example.com", "description": null },
      { "uuid": uuid, "name": "Paper", "author": "dev@example.com", "description": "Minecraft" },
    ] })
  );

  let (raw, egg) = info(&server, uuid).await;
  assert_eq!(egg["uuid"], json!(uuid));
  assert_eq!(
    egg_keys(&raw, "docker_images"),
    ["Zulu 21", "Alpha 17", "Mid 11"]
  );
  assert_eq!(egg_keys(&raw, "startup_commands"), ["Zeta", "Default"]);
  assert_eq!(egg_envs(&egg), ["FIRST", "LATE", "UNSORTED"]);
  assert_eq!(
    egg["config"]["files"]["server.properties"]["find"]["server-port"],
    "{{server.allocations.default.port}}"
  );
  assert_eq!(egg["config"]["logs"], json!({}));
  assert_eq!(egg["meta"], json!({ "update_url": null }));
  assert_eq!(egg["tags"], json!(["minecraft"]));
  assert_eq!(egg["features"], json!(["eula"]));
  assert_eq!(egg["file_denylist"], json!(["*.jar"]));
  assert_eq!(egg["scripts"]["installation"]["container"], "alpine");
  assert_eq!(egg["variables"][0]["rules"], json!(["required", "string"]));

  // a fetched egg can be sent straight back as an edit
  let mut edit = egg.clone();
  edit["name"] = json!("Paper 2");
  edit["meta"] = json!({ "update_url": "https://example.com/egg.json" });
  edit["docker_images"] = json!({ "B": "img:b", "A": "img:a" });
  edit["variables"] = json!([
    { "name": "Only", "description": "d", "env_variable": "LATE", "default_value": "x", "user_viewable": false, "user_editable": false, "rules": [], "sort": null }
  ]);
  assert_eq!(server.put("/eggs", edit).await.status(), StatusCode::OK);
  let (raw, egg) = info(&server, uuid).await;
  assert_eq!(egg["name"], "Paper 2");
  assert_eq!(egg["meta"]["update_url"], "https://example.com/egg.json");
  assert_eq!(egg_keys(&raw, "docker_images"), ["B", "A"]);
  assert_eq!(egg_envs(&egg), ["LATE"]);
  assert_eq!(egg["variables"][0]["name"], "Only");

  assert_eq!(
    server
      .delete("/eggs", json!({ "uuid": uuid }))
      .await
      .status(),
    StatusCode::OK
  );
  let list: Value = server.get("/eggs").await.json().await.unwrap();
  assert_eq!(list["eggs"].as_array().unwrap().len(), 1);
  assert_eq!(
    server.get(&format!("/eggs/{uuid}")).await.status(),
    StatusCode::NOT_FOUND
  );
}

#[tokio::test]
async fn egg_error_paths() {
  let (server, _) = TestServer::start_with_admin().await;
  let missing = Uuid::now_v7();

  // 404s
  assert_eq!(
    server.get(&format!("/eggs/{missing}")).await.status(),
    StatusCode::NOT_FOUND
  );
  let mut edit = egg_body();
  edit["uuid"] = json!(missing);
  assert_eq!(
    server.put("/eggs", edit).await.status(),
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    server
      .delete("/eggs", json!({ "uuid": missing }))
      .await
      .status(),
    StatusCode::NOT_FOUND
  );

  // 400 from validation, on create and on edit (which validates before lookup)
  let mut bad = egg_body();
  bad["variables"][0]["env_variable"] = json!("SERVER_PORT");
  assert_eq!(
    server.post("/eggs", bad.clone()).await.status(),
    StatusCode::BAD_REQUEST
  );
  let uuid = create(&server, egg_body()).await;
  bad["uuid"] = json!(uuid);
  assert_eq!(
    server.put("/eggs", bad).await.status(),
    StatusCode::BAD_REQUEST
  );
  let mut bad = egg_body();
  bad["config"]["files"] = json!("{broken");
  assert_eq!(
    server.post("/eggs", bad).await.status(),
    StatusCode::BAD_REQUEST
  );

  // malformed bodies / paths are rejected before the handler
  let mut no_name = egg_body();
  no_name.as_object_mut().unwrap().remove("name");
  assert!(
    server
      .post("/eggs", no_name)
      .await
      .status()
      .is_client_error()
  );
  assert!(
    server
      .get("/eggs/not-a-uuid")
      .await
      .status()
      .is_client_error()
  );

  // a failed edit left the egg untouched
  let (_, egg) = info(&server, uuid).await;
  assert_eq!(egg["name"], "Paper");
  assert_eq!(egg_envs(&egg).len(), 3);
}

#[tokio::test]
async fn egg_permissions() {
  let (server, _) = TestServer::start_with_admin().await;
  let uuid = create(&server, egg_body()).await;
  let mut edit = egg_body();
  edit["uuid"] = json!(uuid);
  let del = json!({ "uuid": uuid });

  // member user without any egg permission
  let email = format!("{}@example.com", unique("member"));
  let password = server.encrypt_password("memberpass1").await;
  let resp = server
    .post(
      "/user/management",
      json!({ "name": "Member", "email": email, "password": password }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let member: Uuid = resp.json::<Value>().await.unwrap()["uuid"]
    .as_str()
    .unwrap()
    .parse()
    .unwrap();
  let resp = server
    .post("/group", json!({ "name": unique("egg-viewers") }))
    .await;
  let group: Uuid = resp.json::<Value>().await.unwrap()["uuid"]
    .as_str()
    .unwrap()
    .parse()
    .unwrap();
  let set_perms = |perms: Value| json!({ "uuid": group, "name": "egg-viewers", "permissions": perms, "users": [member] });
  assert_eq!(
    server.put("/group", set_perms(json!([]))).await.status(),
    StatusCode::OK
  );

  // no cookie is rejected by centaurus with 400, a bogus token with 401
  server.clear_cookies();
  let unauth = async |server: &TestServer, status: StatusCode| {
    assert_eq!(server.get("/eggs").await.status(), status);
    assert_eq!(server.get(&format!("/eggs/{uuid}")).await.status(), status);
    assert_eq!(server.post("/eggs", egg_body()).await.status(), status);
    assert_eq!(server.put("/eggs", edit.clone()).await.status(), status);
    assert_eq!(server.delete("/eggs", del.clone()).await.status(), status);
  };
  unauth(&server, StatusCode::BAD_REQUEST).await;
  server.set_cookie(JWT_COOKIE_NAME, "not-a-jwt");
  unauth(&server, StatusCode::UNAUTHORIZED).await;
  server.clear_cookies();

  // logged in, no egg permissions: 403 everywhere
  assert_eq!(
    server.login(&email, "memberpass1").await.status(),
    StatusCode::OK
  );
  let forbidden = async |server: &TestServer, view: StatusCode| {
    assert_eq!(server.get("/eggs").await.status(), view);
    assert_eq!(server.get(&format!("/eggs/{uuid}")).await.status(), view);
    assert_eq!(
      server.post("/eggs", egg_body()).await.status(),
      StatusCode::FORBIDDEN
    );
    assert_eq!(
      server.put("/eggs", edit.clone()).await.status(),
      StatusCode::FORBIDDEN
    );
    assert_eq!(
      server.delete("/eggs", del.clone()).await.status(),
      StatusCode::FORBIDDEN
    );
  };
  forbidden(&server, StatusCode::FORBIDDEN).await;

  // egg:view only: reads work, writes stay forbidden
  server.clear_cookies();
  server.login("admin@example.com", "hunter2pass").await;
  server.put("/group", set_perms(json!(["egg:view"]))).await;
  server.clear_cookies();
  server.login(&email, "memberpass1").await;
  forbidden(&server, StatusCode::OK).await;

  // egg:edit (without view) can write
  server.clear_cookies();
  server.login("admin@example.com", "hunter2pass").await;
  server.put("/group", set_perms(json!(["egg:edit"]))).await;
  server.clear_cookies();
  server.login(&email, "memberpass1").await;
  assert_eq!(server.get("/eggs").await.status(), StatusCode::FORBIDDEN);
  assert_eq!(server.put("/eggs", edit).await.status(), StatusCode::OK);
  assert_eq!(
    server.post("/eggs", egg_body()).await.status(),
    StatusCode::OK
  );
  assert_eq!(server.delete("/eggs", del).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn egg_changes_are_broadcast() {
  let (server, _) = TestServer::start_with_admin().await;

  let mut req = format!("ws://localhost:{}/api/ws/updater", server.port)
    .into_client_request()
    .unwrap();
  req
    .headers_mut()
    .insert("Cookie", server.cookie_header().unwrap().parse().unwrap());
  let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();

  let mut next = async || loop {
    let msg = timeout(Duration::from_secs(5), ws.next())
      .await
      .expect("no update received")
      .unwrap()
      .unwrap();
    if let Message::Text(text) = msg {
      let v: Value = serde_json::from_str(&text).unwrap();
      if v["type"] == "Eggs" {
        return v;
      }
    }
  };

  let uuid = create(&server, egg_body()).await;
  assert_eq!(next().await, json!({ "type": "Eggs", "uuid": uuid }));

  let mut edit = egg_body();
  edit["uuid"] = json!(uuid);
  server.put("/eggs", edit).await;
  assert_eq!(next().await, json!({ "type": "Eggs", "uuid": uuid }));

  server.delete("/eggs", json!({ "uuid": uuid })).await;
  assert_eq!(next().await, json!({ "type": "Eggs", "uuid": uuid }));
}
