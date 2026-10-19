//! Full-stack tests against a real postgres (testcontainers) instead of sqlite.
//!
//! Booting the app covers centaurus `init_db` on postgres (including every
//! migration), and the flows below hit the user and group handlers.
mod common;

use common::{TestServer, egg_body, egg_envs, egg_keys, unique};
use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

#[tokio::test]
async fn pg_boot_setup_and_login() {
  let (server, admin) = TestServer::with_admin(TestServer::start_pg().await).await;

  server.clear_cookies();
  let resp = server.login("admin@example.com", "hunter2pass").await;
  assert_eq!(resp.status(), StatusCode::OK);

  let info: Value = server.get("/user/info").await.json().await.unwrap();
  assert_eq!(info["uuid"].as_str().unwrap(), admin.to_string());
}

#[tokio::test]
async fn pg_user_and_group_flow() {
  let (server, _) = TestServer::with_admin(TestServer::start_pg().await).await;

  let email = format!("{}@example.com", unique("member"));
  let password = server.encrypt_password("memberpass1").await;
  let resp = server
    .post(
      "/user/management",
      json!({ "name": "Member", "email": email, "password": password }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let user: Uuid = resp.json::<Value>().await.unwrap()["uuid"]
    .as_str()
    .unwrap()
    .parse()
    .unwrap();

  // duplicate email hits the postgres unique constraint
  let password = server.encrypt_password("memberpass1").await;
  let resp = server
    .post(
      "/user/management",
      json!({ "name": "Other", "email": email, "password": password }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::CONFLICT);

  let name = unique("editors");
  let resp = server.post("/group", json!({ "name": name })).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let group: Uuid = resp.json::<Value>().await.unwrap()["uuid"]
    .as_str()
    .unwrap()
    .parse()
    .unwrap();

  let resp = server
    .put(
      "/group",
      json!({ "uuid": group, "name": name, "permissions": ["group:view"], "users": [user] }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK);

  // the new member can log in on postgres too
  server.clear_cookies();
  assert_eq!(
    server.login(&email, "memberpass1").await.status(),
    StatusCode::OK
  );
  let info: Value = server.get("/user/info").await.json().await.unwrap();
  assert_eq!(info["uuid"].as_str().unwrap(), user.to_string());

  // deleting the user cascades its group membership
  server.clear_cookies();
  server.login("admin@example.com", "hunter2pass").await;
  let resp = server
    .delete("/user/management", json!({ "uuid": user }))
    .await;
  assert_eq!(resp.status(), StatusCode::OK);
  let resp = server.delete("/group", json!({ "uuid": group })).await;
  assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn pg_egg_flow() {
  let (server, _) = TestServer::with_admin(TestServer::start_pg().await).await;

  let resp = server.post("/eggs", egg_body()).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let uuid: Uuid = resp.json::<Value>().await.unwrap()["uuid"]
    .as_str()
    .unwrap()
    .parse()
    .unwrap();

  let list: Value = server.get("/eggs").await.json().await.unwrap();
  assert_eq!(list["eggs"][0]["uuid"].as_str().unwrap(), uuid.to_string());
  assert_eq!(list["eggs"][0]["description"], "Minecraft");

  let raw = server
    .get(&format!("/eggs/{uuid}"))
    .await
    .text()
    .await
    .unwrap();
  let info: Value = serde_json::from_str(&raw).unwrap();
  assert_eq!(info["uuid"].as_str().unwrap(), uuid.to_string());
  assert_eq!(
    egg_keys(&raw, "docker_images"),
    ["Zulu 21", "Alpha 17", "Mid 11"]
  );
  assert_eq!(egg_keys(&raw, "startup_commands"), ["Zeta", "Default"]);
  assert_eq!(egg_envs(&info), ["FIRST", "LATE", "UNSORTED"]);
  // config strings come back as objects
  assert_eq!(
    info["config"]["files"]["server.properties"]["parser"],
    "properties"
  );
  assert_eq!(info["config"]["logs"], json!({}));
  assert_eq!(info["scripts"]["installation"]["entrypoint"], "ash");

  // edit replaces all variables (re-using an env name must not hit the unique index)
  let mut body = egg_body();
  body["uuid"] = json!(uuid);
  body["name"] = json!("Paper 2");
  body["variables"] = json!([
    { "name": "First again", "description": "", "env_variable": "FIRST", "default_value": "", "user_viewable": false, "user_editable": false, "rules": [], "sort": null }
  ]);
  let resp = server.put("/eggs", body).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let info: Value = server
    .get(&format!("/eggs/{uuid}"))
    .await
    .json()
    .await
    .unwrap();
  assert_eq!(info["name"], "Paper 2");
  assert_eq!(egg_envs(&info), ["FIRST"]);
  assert_eq!(info["variables"][0]["name"], "First again");

  // reserved env name is rejected
  let mut body = egg_body();
  body["variables"][0]["env_variable"] = json!("SERVER_PORT");
  assert_eq!(
    server.post("/eggs", body).await.status(),
    StatusCode::BAD_REQUEST
  );

  let resp = server.delete("/eggs", json!({ "uuid": uuid })).await;
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(
    server.get(&format!("/eggs/{uuid}")).await.status(),
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    server
      .delete("/eggs", json!({ "uuid": uuid }))
      .await
      .status(),
    StatusCode::NOT_FOUND
  );
}
