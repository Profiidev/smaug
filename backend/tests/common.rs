//! Shared harness for the backend integration tests.
//!
//! Mirrors the structure used by the reference backend
//! (github.com/ichwilldich/hydra): every test boots a *real* server on an
//! OS-assigned port (`PORT=0`) backed by an in-memory SQLite database, then
//! drives it over HTTP with `reqwest`. Because `cargo nextest` runs each test
//! in its own process, the process-global environment configured by
//! [`prepare_env`] is isolated per test, so the shared `sqlite::memory:`
//! database never leaks between tests.
//!
//! [`TestServer::start_pg`] boots the same server against a throwaway postgres
//! container (testcontainers, needs Docker) instead of sqlite.
//!
//! The server sets its auth cookies with the `Secure` flag, which `reqwest`'s
//! own cookie store refuses to persist over plain HTTP. We therefore keep a
//! tiny cookie jar of our own ([`TestServer`]) that captures every
//! `Set-Cookie` and replays them on subsequent requests — this transparently
//! handles the JWT session cookie as well as the short-lived "special access"
//! and TOTP cookies.
//!
//! Run with the `test` feature so centaurus uses fast (512-bit) RSA keys:
//!
//! ```text
//! cargo nextest run --features test
//! ```
#![allow(dead_code)]

use std::{
  collections::HashMap,
  sync::{
    Mutex,
    atomic::{AtomicU32, Ordering},
  },
  time::Duration,
};

use backend::App;
use base64::{Engine, prelude::BASE64_STANDARD};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use rsa::{Pkcs1v15Encrypt, RsaPublicKey, pkcs1::DecodeRsaPublicKey, rand_core::OsRng};
use serde_json::Value;
use testcontainers_modules::{
  postgres::Postgres,
  testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tokio::{spawn, time::sleep};
use uuid::Uuid;

/// The auth cookie centaurus sets on a successful login/setup.
pub const JWT_COOKIE_NAME: &str = "centaurus_jwt";

/// Configure the process environment for a throwaway in-memory server.
///
/// Safe to call repeatedly; nextest gives every test its own process so this
/// never races with another test.
pub fn prepare_env() {
  unsafe {
    std::env::set_var("PORT", "0");
    std::env::set_var("DB_URL", "sqlite::memory:");
    std::env::set_var("SITE_URL", "http://localhost/");
    // keep logs quiet during tests
    std::env::set_var("LOG_LEVEL", "off");
  }
}

/// A running test server plus a client (with a hand-rolled cookie jar).
pub struct TestServer {
  pub port: u16,
  client: Client,
  cookies: Mutex<HashMap<String, String>>,
  /// Postgres container the server depends on; kept alive as long as the server.
  _pg: Option<ContainerAsync<Postgres>>,
}

impl TestServer {
  /// Boot a fresh server with an empty in-memory database and wait for it to
  /// become ready.
  pub async fn start() -> TestServer {
    prepare_env();
    TestServer::boot(None).await
  }

  /// Boot a fresh server backed by a throwaway postgres container (same major
  /// version as production) instead of sqlite.
  pub async fn start_pg() -> TestServer {
    prepare_env();
    let container = Postgres::default()
      .with_tag("18-alpine")
      .start()
      .await
      .expect("start postgres container");
    let url = format!(
      "postgres://postgres:postgres@{}:{}/postgres",
      container.get_host().await.unwrap(),
      container.get_host_port_ipv4(5432).await.unwrap()
    );
    unsafe {
      std::env::set_var("DB_URL", url);
    }
    TestServer::boot(Some(container)).await
  }

  async fn boot(pg: Option<ContainerAsync<Postgres>>) -> TestServer {
    let app = App::new().await;
    let port = app.port();
    spawn(app.run());

    let client = Client::builder()
      .timeout(Duration::from_secs(30))
      .connect_timeout(Duration::from_secs(30))
      .build()
      .expect("build reqwest client");

    let server = TestServer {
      port,
      client,
      cookies: Mutex::new(HashMap::new()),
      _pg: pg,
    };

    // Wait until the server is accepting requests.
    for _ in 0..100 {
      if let Ok(resp) = server.client.get(server.url("/health")).send().await
        && resp.status().is_success()
      {
        return server;
      }
      sleep(Duration::from_millis(50)).await;
    }
    panic!("server did not become ready in time");
  }

  /// Boot a server and complete first-run setup with a default admin.
  pub async fn start_with_admin() -> (TestServer, Uuid) {
    TestServer::with_admin(TestServer::start().await).await
  }

  /// Complete first-run setup with a default admin on an already booted server.
  pub async fn with_admin(server: TestServer) -> (TestServer, Uuid) {
    let id = server
      .setup_admin("admin", "admin@example.com", "hunter2pass")
      .await;
    (server, id)
  }

  /// Build a full URL under the `/api` prefix, e.g. `url("/auth/config")`.
  pub fn url(&self, path: &str) -> String {
    format!("http://localhost:{}/api{}", self.port, path)
  }

  /// Build a full URL with no `/api` prefix (for `/.well-known/...`).
  pub fn root_url(&self, path: &str) -> String {
    format!("http://localhost:{}{}", self.port, path)
  }

  pub fn cookie_header(&self) -> Option<String> {
    let jar = self.cookies.lock().unwrap();
    if jar.is_empty() {
      return None;
    }
    Some(
      jar
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; "),
    )
  }

  /// Capture every `Set-Cookie` from a response into the jar (handling
  /// deletions signalled by an empty value or `Max-Age=0`).
  fn capture_cookies(&self, resp: &Response) {
    let mut jar = self.cookies.lock().unwrap();
    for value in resp.headers().get_all("set-cookie") {
      let Ok(value) = value.to_str() else { continue };
      let mut parts = value.split(';');
      let Some(pair) = parts.next() else { continue };
      let Some((name, val)) = pair.split_once('=') else {
        continue;
      };
      let name = name.trim().to_string();
      let val = val.trim().to_string();

      let deleted = val.is_empty()
        || parts.any(|attr| {
          let attr = attr.trim().to_ascii_lowercase();
          attr == "max-age=0" || attr.starts_with("expires=thu, 01 jan 1970")
        });

      if deleted {
        jar.remove(&name);
      } else {
        jar.insert(name, val);
      }
    }
  }

  async fn send(&self, req: RequestBuilder) -> Response {
    let req = match self.cookie_header() {
      Some(cookie) => req.header("Cookie", cookie),
      None => req,
    };
    let resp = req.send().await.expect("request failed");
    self.capture_cookies(&resp);
    resp
  }

  pub async fn get(&self, path: &str) -> Response {
    self.send(self.client.get(self.url(path))).await
  }

  pub async fn get_root(&self, path: &str) -> Response {
    self.send(self.client.get(self.root_url(path))).await
  }

  pub async fn post(&self, path: &str, body: Value) -> Response {
    self
      .send(self.client.post(self.url(path)).json(&body))
      .await
  }

  pub async fn put(&self, path: &str, body: Value) -> Response {
    self.send(self.client.put(self.url(path)).json(&body)).await
  }

  pub async fn delete(&self, path: &str, body: Value) -> Response {
    self
      .send(self.client.delete(self.url(path)).json(&body))
      .await
  }

  /// PUT a raw byte body (used by the note-edit endpoint, which reads `Bytes`).
  pub async fn put_bytes(&self, path: &str, body: Vec<u8>) -> Response {
    self.send(self.client.put(self.url(path)).body(body)).await
  }

  /// Fetch the password-transfer RSA public key and encrypt `plaintext` the
  /// same way the frontend does (RSA-PKCS1v15 + base64).
  pub async fn encrypt_password(&self, plaintext: &str) -> String {
    let resp = self.get("/auth/password").await;
    assert_eq!(resp.status(), StatusCode::OK, "fetch password key");
    let key: Value = resp.json().await.expect("key json");
    let pem = key["key"].as_str().expect("key pem");

    let public_key = RsaPublicKey::from_pkcs1_pem(pem).expect("parse rsa pub key");
    let ciphertext = public_key
      .encrypt(&mut OsRng, Pkcs1v15Encrypt, plaintext.as_bytes())
      .expect("rsa encrypt");
    BASE64_STANDARD.encode(ciphertext)
  }

  /// Run the first-run setup, creating the admin user and (via the cookie jar)
  /// storing its session. Returns the admin user id.
  pub async fn setup_admin(&self, username: &str, email: &str, password: &str) -> Uuid {
    let encrypted = self.encrypt_password(password).await;
    let resp = self
      .post(
        "/setup",
        serde_json::json!({
          "admin_username": username,
          "admin_email": email,
          "admin_password": encrypted,
          "application": "",
          "operating_system": "",
          "name": ""
        }),
      )
      .await;
    assert_eq!(resp.status(), StatusCode::OK, "setup should succeed");
    assert!(
      self.has_cookie(JWT_COOKIE_NAME),
      "setup must set the auth cookie"
    );

    let body: Value = resp.json().await.expect("setup json");
    Uuid::parse_str(body["user"].as_str().expect("user id")).expect("parse user id")
  }

  /// Log in via the password endpoint (cookies captured automatically).
  pub async fn login(&self, email: &str, password: &str) -> Response {
    let encrypted = self.encrypt_password(password).await;
    self
      .post(
        "/auth/password",
        serde_json::json!({ "email": email, "password": encrypted }),
      )
      .await
  }

  pub fn has_cookie(&self, name: &str) -> bool {
    self.cookies.lock().unwrap().contains_key(name)
  }

  pub fn set_cookie(&self, name: &str, value: &str) {
    self
      .cookies
      .lock()
      .unwrap()
      .insert(name.into(), value.into());
  }

  /// Drop all stored cookies so subsequent requests are unauthenticated.
  pub fn clear_cookies(&self) {
    self.cookies.lock().unwrap().clear();
  }
}

/// Pull the `centaurus_jwt` value out of a response's `Set-Cookie` headers.
pub fn extract_jwt_cookie(resp: &Response) -> Option<String> {
  for value in resp.headers().get_all("set-cookie") {
    let Ok(value) = value.to_str() else { continue };
    let prefix = format!("{JWT_COOKIE_NAME}=");
    if let Some(rest) = value.strip_prefix(&prefix) {
      let token = rest.split(';').next().unwrap_or("").to_string();
      if !token.is_empty() {
        return Some(token);
      }
    }
  }
  None
}

/// Monotonic counter for generating unique names within a test.
static COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn unique(prefix: &str) -> String {
  format!("{prefix}-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// A valid PLCN_v3-shaped egg body; map keys are deliberately not alphabetical.
pub fn egg_body() -> Value {
  serde_json::json!({
    "name": "Paper",
    "author": "dev@example.com",
    "description": "Minecraft",
    "tags": ["minecraft"],
    "features": ["eula"],
    // deliberately not alphabetical: order must survive the roundtrip
    "docker_images": { "Zulu 21": "ghcr.io/java:21", "Alpha 17": "ghcr.io/java:17", "Mid 11": "ghcr.io/java:11" },
    "file_denylist": ["*.jar"],
    "startup_commands": { "Zeta": "java -jar a.jar", "Default": "java -jar b.jar" },
    "config": {
      "files": "{\"server.properties\":{\"parser\":\"properties\",\"find\":{\"server-port\":\"{{server.allocations.default.port}}\"}}}",
      "startup": { "done": ")! For help, type " },
      "logs": "{}",
      "stop": "stop"
    },
    "scripts": { "installation": { "script": "echo hi", "container": "alpine", "entrypoint": "ash" } },
    "meta": { "update_url": null },
    "variables": [
      { "name": "Late", "description": "", "env_variable": "LATE", "default_value": "", "user_viewable": true, "user_editable": false, "rules": ["nullable"], "sort": 2 },
      { "name": "Unsorted", "description": "", "env_variable": "UNSORTED", "default_value": "", "user_viewable": true, "user_editable": false, "rules": [], "sort": null },
      { "name": "First", "description": "", "env_variable": "FIRST", "default_value": "1", "user_viewable": true, "user_editable": true, "rules": ["required", "string"], "sort": 1 }
    ]
  })
}

/// Keys of an ordered JSON map in a response body. Parsed via `IndexMap`
/// because `serde_json::Value` (without `preserve_order`) sorts keys.
pub fn egg_keys(raw: &str, field: &str) -> Vec<String> {
  #[derive(serde::Deserialize)]
  struct Maps {
    docker_images: indexmap::IndexMap<String, String>,
    startup_commands: indexmap::IndexMap<String, String>,
  }
  let m: Maps = serde_json::from_str(raw).unwrap();
  match field {
    "docker_images" => m.docker_images,
    "startup_commands" => m.startup_commands,
    _ => panic!("unknown field {field}"),
  }
  .into_keys()
  .collect()
}

pub fn egg_envs(info: &Value) -> Vec<&str> {
  info["variables"]
    .as_array()
    .unwrap()
    .iter()
    .map(|v| v["env_variable"].as_str().unwrap())
    .collect()
}

pub const MEMBER_PASSWORD: &str = "memberpass1";

pub fn uuid_of(v: &Value) -> Uuid {
  v["uuid"]
    .as_str()
    .expect("uuid")
    .parse()
    .expect("parse uuid")
}

/// Creates a user (as the current, admin, session); returns its id and email.
pub async fn create_user(server: &TestServer, name: &str) -> (Uuid, String) {
  let email = format!("{}@example.com", unique(name));
  let password = server.encrypt_password(MEMBER_PASSWORD).await;
  let resp = server
    .post(
      "/user/management",
      serde_json::json!({ "name": name, "email": email, "password": password }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK, "create user");
  (uuid_of(&resp.json().await.unwrap()), email)
}

/// Creates a group with `permissions` and `users` (as the admin session).
pub async fn create_group(server: &TestServer, permissions: Value, users: &[Uuid]) -> Uuid {
  let name = unique("group");
  let resp = server
    .post("/group", serde_json::json!({ "name": name }))
    .await;
  assert_eq!(resp.status(), StatusCode::OK, "create group");
  let uuid = uuid_of(&resp.json().await.unwrap());
  let resp = server
    .put(
      "/group",
      serde_json::json!({ "uuid": uuid, "name": name, "permissions": permissions, "users": users }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK, "edit group");
  uuid
}

/// Creates a node whose wings never connect.
pub async fn create_node(server: &TestServer, name: &str) -> Uuid {
  let resp = server
    .post(
      "/nodes",
      serde_json::json!({
        "name": name, "address": "http://127.0.0.1:1", "secure": false,
        "disk_limit_mb": null, "memory_limit_mb": null, "cpu_limit": null
      }),
    )
    .await;
  assert_eq!(resp.status(), StatusCode::OK, "create node");
  uuid_of(&resp.json().await.unwrap())
}

/// `egg_body` with variables covering every visibility combination:
/// FIRST and MAX_PLAYERS editable, LOCKED view-only, HIDDEN neither.
pub fn server_egg_body() -> Value {
  let mut egg = egg_body();
  egg["variables"] = serde_json::json!([
    { "name": "First", "description": "first var", "env_variable": "FIRST", "default_value": "1", "user_viewable": true, "user_editable": true, "rules": ["required", "string"], "sort": 1 },
    { "name": "Max players", "description": "", "env_variable": "MAX_PLAYERS", "default_value": "20", "user_viewable": true, "user_editable": true, "rules": ["required", "integer", "between:1,100"], "sort": 2 },
    { "name": "Locked", "description": "", "env_variable": "LOCKED", "default_value": "locked", "user_viewable": true, "user_editable": false, "rules": [], "sort": 3 },
    { "name": "Hidden", "description": "", "env_variable": "HIDDEN", "default_value": "secret", "user_viewable": false, "user_editable": false, "rules": ["nullable"], "sort": 4 }
  ]);
  egg
}

pub async fn create_egg(server: &TestServer, body: Value) -> Uuid {
  let resp = server.post("/eggs", body).await;
  assert_eq!(resp.status(), StatusCode::OK, "create egg");
  uuid_of(&resp.json().await.unwrap())
}

/// A valid create body; installed right away (`skip_scripts`).
pub fn server_body(node: Uuid, egg: Uuid, port: u16) -> Value {
  serde_json::json!({
    "name": unique("server"),
    "owner": null,
    "node": node,
    "egg": egg,
    "allocations": [{ "ip": "10.0.0.5", "port": port }],
    "skip_scripts": true,
    "start_on_completion": true,
    "startup": "java -jar a.jar",
    "startup_overridden": false,
    "image": "ghcr.io/java:21",
    "environment": { "FIRST": "hello" },
    "limits": { "cpu": 0, "memory": 0, "disk": 0 },
    "backup_limit": 0
  })
}

/// Turns a `server_info` response into an update body that changes nothing.
pub fn update_body(info: &Value) -> Value {
  let environment: serde_json::Map<String, Value> = info["variables"]
    .as_array()
    .unwrap()
    .iter()
    .map(|v| {
      (
        v["env_variable"].as_str().unwrap().to_string(),
        v["value"].clone(),
      )
    })
    .collect();
  serde_json::json!({
    "name": info["name"],
    "owner": info["owner"]["uuid"],
    "egg": info["egg"]["uuid"],
    "allocations": info["allocations"],
    "startup": info["startup"],
    "startup_overridden": info["startup_overridden"],
    "image": info["image"],
    "environment": environment,
    "limits": info["limits"],
    "backup_limit": info["backup_limit"]
  })
}
