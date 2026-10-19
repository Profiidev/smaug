mod common;

use std::time::Duration;

use common::{JWT_COOKIE_NAME, TestServer, unique};
use futures_util::StreamExt;
use reqwest::StatusCode;
use serde_json::{Value, json};
use shared::auth::SignData;
use tokio::{net::TcpListener, spawn, time::timeout};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use uuid::Uuid;

/// Nothing listens here, so the backend's wings connection never comes up.
fn node_body(name: &str) -> Value {
  json!({
    "name": name,
    "address": "http://127.0.0.1:1",
    "secure": false,
    "disk_limit_mb": 1024.0,
    "memory_limit_mb": 512.0,
    "cpu_limit": 2
  })
}

async fn create(server: &TestServer, body: Value) -> Uuid {
  create_with_token(server, body).await.0
}

/// The token is only returned on create (and regenerate).
async fn create_with_token(server: &TestServer, body: Value) -> (Uuid, String) {
  let resp = server.post("/nodes", body).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let res: Value = resp.json().await.unwrap();
  let token = res["token"].as_str().unwrap().to_string();
  assert_eq!(token.len(), 64);
  assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
  (res["uuid"].as_str().unwrap().parse().unwrap(), token)
}

async fn info(server: &TestServer, uuid: Uuid) -> Value {
  let resp = server.get(&format!("/nodes/{uuid}")).await;
  assert_eq!(resp.status(), StatusCode::OK);
  resp.json().await.unwrap()
}

#[tokio::test]
async fn node_ports_above_i16_range_round_trip() {
  let (server, _) = TestServer::start_with_admin().await;
  let mut body = node_body("high-port");
  body["address"] = json!("http://127.0.0.1:65535");
  let uuid = create(&server, body).await;
  assert_eq!(info(&server, uuid).await["port"], json!(65535));

  let mut update = node_body("high-port");
  update["address"] = json!("http://127.0.0.1:40000");
  let resp = server.post(&format!("/nodes/{uuid}"), update).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(info(&server, uuid).await["port"], json!(40000));
}

#[tokio::test]
async fn node_crud_flow() {
  let (server, _) = TestServer::start_with_admin().await;
  let list: Value = server.get("/nodes").await.json().await.unwrap();
  assert_eq!(list, json!([]));

  let uuid = create(&server, node_body("alpha")).await;
  // no port in the address: the default follows `secure`
  let mut body = node_body("beta");
  body["address"] = json!("127.0.0.1");
  body["secure"] = json!(true);
  body["disk_limit_mb"] = Value::Null;
  let beta = create(&server, body).await;
  let mut body = node_body("gamma");
  body["address"] = json!("127.0.0.1");
  let gamma = create(&server, body).await;

  let list: Value = server.get("/nodes").await.json().await.unwrap();
  assert_eq!(list.as_array().unwrap().len(), 3);

  let node = info(&server, uuid).await;
  assert_eq!(
    node,
    json!({
      "id": uuid, "name": "alpha", "address": "127.0.0.1", "port": 1, "secure": false,
      "disk_limit_mb": 1024.0, "memory_limit_mb": 512.0, "cpu_limit": 2,
      "connected": false
    })
  );
  let node = info(&server, beta).await;
  assert_eq!(node["port"], 443);
  assert_eq!(node["disk_limit_mb"], Value::Null);
  assert_eq!(info(&server, gamma).await["port"], 80);

  let update = json!({
    "name": "alpha 2", "address": "https://localhost:2", "secure": true,
    "disk_limit_mb": null, "memory_limit_mb": 1.5, "cpu_limit": null
  });
  let resp = server.post(&format!("/nodes/{uuid}"), update).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let node = info(&server, uuid).await;
  assert_eq!(
    node,
    json!({
      "id": uuid, "name": "alpha 2", "address": "localhost", "port": 2, "secure": true,
      "disk_limit_mb": null, "memory_limit_mb": 1.5, "cpu_limit": null,
      "connected": false
    })
  );

  let resp = server.delete("/nodes", json!({ "uuid": uuid })).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(
    server.get(&format!("/nodes/{uuid}")).await.status(),
    StatusCode::NOT_FOUND
  );
  let list: Value = server.get("/nodes").await.json().await.unwrap();
  assert_eq!(list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn node_error_paths() {
  let (server, _) = TestServer::start_with_admin().await;
  let missing = Uuid::now_v7();
  let uuid = create(&server, node_body("alpha")).await;
  create(&server, node_body("beta")).await;

  // 409 on a duplicate name, on create and on update
  assert_eq!(
    server.post("/nodes", node_body("alpha")).await.status(),
    StatusCode::CONFLICT
  );
  assert_eq!(
    server
      .post(&format!("/nodes/{uuid}"), node_body("beta"))
      .await
      .status(),
    StatusCode::CONFLICT
  );
  // keeping its own name is fine
  assert_eq!(
    server
      .post(&format!("/nodes/{uuid}"), node_body("alpha"))
      .await
      .status(),
    StatusCode::OK
  );

  // 400 from validation, on create and on update
  for (field, value) in [
    ("disk_limit_mb", json!(-1.0)),
    ("memory_limit_mb", json!(-1.0)),
    ("address", json!("not an address")),
    ("address", json!("/no/host")),
  ] {
    let mut body = node_body("new");
    body[field] = value;
    assert_eq!(
      server.post("/nodes", body.clone()).await.status(),
      StatusCode::BAD_REQUEST,
      "create {field}"
    );
    body["name"] = json!("alpha");
    assert_eq!(
      server.post(&format!("/nodes/{uuid}"), body).await.status(),
      StatusCode::BAD_REQUEST,
      "update {field}"
    );
  }

  // 404 for a missing node
  assert_eq!(
    server.get(&format!("/nodes/{missing}")).await.status(),
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    server
      .post(&format!("/nodes/{missing}"), node_body("x"))
      .await
      .status(),
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    server
      .delete("/nodes", json!({ "uuid": missing }))
      .await
      .status(),
    StatusCode::NOT_FOUND
  );

  // nothing was created or changed by the failed requests
  let list: Value = server.get("/nodes").await.json().await.unwrap();
  assert_eq!(list.as_array().unwrap().len(), 2);
  let node = info(&server, uuid).await;
  assert_eq!(node["name"], "alpha");
  assert_eq!(node["disk_limit_mb"], 1024.0);
}

#[tokio::test]
async fn node_permissions() {
  let (server, _) = TestServer::start_with_admin().await;
  let uuid = create(&server, node_body("alpha")).await;
  let info_path = format!("/nodes/{uuid}");
  let del = json!({ "uuid": uuid });

  // member user without any node permission
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
    .post("/group", json!({ "name": "node-viewers" }))
    .await;
  let group: Uuid = resp.json::<Value>().await.unwrap()["uuid"]
    .as_str()
    .unwrap()
    .parse()
    .unwrap();
  let set_perms = |perms: Value| json!({ "uuid": group, "name": "node-viewers", "permissions": perms, "users": [member] });
  assert_eq!(
    server.put("/group", set_perms(json!([]))).await.status(),
    StatusCode::OK
  );

  // no cookie is rejected by centaurus with 400, a bogus token with 401
  let token_path = format!("/nodes/{uuid}/token");
  let all = async |server: &TestServer, view: StatusCode, edit: StatusCode| {
    assert_eq!(server.get("/nodes").await.status(), view);
    assert_eq!(server.post(&token_path, json!({})).await.status(), edit);
    assert_eq!(server.get(&info_path).await.status(), view);
    assert_eq!(server.post("/nodes", node_body("new")).await.status(), edit);
    assert_eq!(
      server.post(&info_path, node_body("alpha")).await.status(),
      edit
    );
    assert_eq!(server.delete("/nodes", del.clone()).await.status(), edit);
  };
  server.clear_cookies();
  all(&server, StatusCode::BAD_REQUEST, StatusCode::BAD_REQUEST).await;
  server.set_cookie(JWT_COOKIE_NAME, "not-a-jwt");
  all(&server, StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED).await;
  server.clear_cookies();

  // logged in, no node permissions: 403 everywhere
  assert_eq!(
    server.login(&email, "memberpass1").await.status(),
    StatusCode::OK
  );
  all(&server, StatusCode::FORBIDDEN, StatusCode::FORBIDDEN).await;

  let as_member_with = async |perms: Value| {
    server.clear_cookies();
    server.login("admin@example.com", "hunter2pass").await;
    server.put("/group", set_perms(perms)).await;
    server.clear_cookies();
    server.login(&email, "memberpass1").await;
  };

  // node:view only: reads work, writes stay forbidden
  as_member_with(json!(["node:view"])).await;
  all(&server, StatusCode::OK, StatusCode::FORBIDDEN).await;

  // node:edit (without view) can write
  as_member_with(json!(["node:edit"])).await;
  assert_eq!(server.get("/nodes").await.status(), StatusCode::FORBIDDEN);
  assert_eq!(
    server.post(&info_path, node_body("alpha")).await.status(),
    StatusCode::OK
  );
  assert_eq!(
    server.post("/nodes", node_body("new")).await.status(),
    StatusCode::OK
  );
  assert_eq!(
    server.post(&token_path, json!({})).await.status(),
    StatusCode::OK
  );
  assert_eq!(server.delete("/nodes", del).await.status(), StatusCode::OK);
}

/// Connect to the updater websocket and return a fn yielding the next `Nodes` message.
async fn nodes_updates(server: &TestServer) -> impl AsyncFnMut() -> Value {
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
        if v["type"] == "Nodes" {
          return v;
        }
      }
    }
  }
}

#[tokio::test]
async fn node_changes_are_broadcast() {
  let (server, _) = TestServer::start_with_admin().await;
  let mut next = nodes_updates(&server).await;

  let uuid = create(&server, node_body("alpha")).await;
  assert_eq!(next().await, json!({ "type": "Nodes", "uuid": uuid }));

  server
    .post(&format!("/nodes/{uuid}"), node_body("alpha 2"))
    .await;
  assert_eq!(next().await, json!({ "type": "Nodes", "uuid": uuid }));

  server.delete("/nodes", json!({ "uuid": uuid })).await;
  assert_eq!(next().await, json!({ "type": "Nodes", "uuid": uuid }));
}

/// Minimal Wings stand-in (mirrors `wings/src/auth.rs`): verifies the signed
/// handshake, signs the response with `token` and accepts the websocket.
/// Returns its port.
// the handshake callback's error type is fixed by tungstenite
#[allow(clippy::result_large_err)]
async fn fake_wings(token: String) -> u16 {
  // Ephemeral ports are usually above 32767, which also covers ports that do not fit in i16.
  let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
  let port = listener.local_addr().unwrap().port();

  spawn(async move {
    while let Ok((stream, _)) = listener.accept().await {
      let token = token.clone();
      spawn(async move {
        let sign =
          |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
           mut res: tokio_tungstenite::tungstenite::handshake::server::Response| {
            let data =
              SignData::validate_header_map(req.headers(), &token, "GET", req.uri().path(), b"")
                .unwrap();
            data
              .add_response_to_header_map(res.headers_mut(), &token)
              .unwrap();
            Ok(res)
          };
        let mut ws = tokio_tungstenite::accept_hdr_async(stream, sign)
          .await
          .unwrap();
        while let Some(Ok(_)) = ws.next().await {}
      });
    }
  });
  port
}

#[tokio::test]
async fn node_connects_to_wings() {
  let (server, _) = TestServer::start_with_admin().await;
  let (uuid, token) = create_with_token(&server, node_body("alpha")).await;

  // the token only exists after create, so point the node at wings afterwards
  let mut next = nodes_updates(&server).await;
  let port = fake_wings(token).await;
  let mut body = node_body("alpha");
  body["address"] = json!(format!("http://127.0.0.1:{port}"));
  let resp = server.post(&format!("/nodes/{uuid}"), body).await;
  assert_eq!(resp.status(), StatusCode::OK);

  // one message from the update, one once the connection is up
  next().await;
  next().await;
  assert_eq!(info(&server, uuid).await["connected"], true);
  let list: Value = server.get("/nodes").await.json().await.unwrap();
  assert_eq!(list[0]["connected"], true);

  // deleting disconnects
  server.delete("/nodes", json!({ "uuid": uuid })).await;
  assert_eq!(
    server.get(&format!("/nodes/{uuid}")).await.status(),
    StatusCode::NOT_FOUND
  );
}

#[tokio::test]
async fn node_token_regenerate() {
  let (server, _) = TestServer::start_with_admin().await;
  let (uuid, token) = create_with_token(&server, node_body("alpha")).await;

  let resp = server
    .post(&format!("/nodes/{uuid}/token"), json!({}))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let new = resp.json::<Value>().await.unwrap()["token"]
    .as_str()
    .unwrap()
    .to_string();
  assert_eq!(new.len(), 64);
  assert_ne!(new, token);
  assert!(info(&server, uuid).await.get("token").is_none());

  let resp = server
    .post(&format!("/nodes/{}/token", Uuid::now_v7()), json!({}))
    .await;
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
