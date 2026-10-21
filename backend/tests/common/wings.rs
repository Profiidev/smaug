//! A fake wings serving its websocket and signed REST API, plus a panel
//! with one installed server on it.
use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
  time::Duration,
};

use super::{MEMBER_PASSWORD, TestServer, create_egg, create_user, server_body, server_egg_body};
use axum::{
  Router,
  body::Bytes,
  extract::{DefaultBodyLimit, State, WebSocketUpgrade, ws::Message as AxumMessage},
  http::{HeaderMap, Method, StatusCode as AxumStatus, Uri},
  response::{IntoResponse, Response},
  routing::get,
};
use futures_util::StreamExt;
use reqwest::StatusCode;
use serde_json::{Value, json};
use shared::auth::SignData;
use tokio::{net::TcpListener, spawn, sync::broadcast, time::timeout};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use uuid::Uuid;

pub const CHUNK: usize = 8 * 1024 * 1024;

/// A request wings received, `path` without the query.
#[derive(Clone, Debug)]
pub struct Req {
  pub method: Method,
  pub path: String,
  pub query: HashMap<String, String>,
  pub headers: HeaderMap,
  pub body: Bytes,
}

impl Req {
  pub fn json(&self) -> Value {
    serde_json::from_slice(&self.body).unwrap()
  }
}

pub type Respond = Arc<dyn Fn(&Req) -> Option<Response> + Send + Sync>;

#[derive(Clone)]
pub struct Fake {
  pub token: String,
  pub events: broadcast::Sender<Value>,
  /// Reported by the sync, kept in step with `State` events.
  pub states: Arc<Mutex<HashMap<Uuid, String>>>,
  pub requests: Arc<Mutex<Vec<Req>>>,
  pub respond: Respond,
}

impl Fake {
  pub fn event(&self, event: Value) {
    self.events.send(event).unwrap();
  }

  pub fn set_state(&self, uuid: Uuid, state: &str) {
    self.states.lock().unwrap().insert(uuid, state.to_string());
    let _ = self
      .events
      .send(json!({ "type": "State", "uuid": uuid, "state": state }));
  }

  pub fn requests(&self) -> Vec<Req> {
    self.requests.lock().unwrap().clone()
  }

  /// Waits for a request matching `check`, spec pushes run in the background.
  pub async fn wait(&self, check: impl Fn(&Req) -> bool) -> Req {
    timeout(Duration::from_secs(10), async {
      loop {
        if let Some(req) = self.requests().into_iter().find(|r| check(r)) {
          return req;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
      }
    })
    .await
    .expect("wings never got the expected request")
  }
}

pub fn wings_error(status: AxumStatus, code: &str) -> Response {
  (
    status,
    axum::Json(json!({ "code": code, "message": "nope" })),
  )
    .into_response()
}

pub async fn fake_ws(
  State(fake): State<Fake>,
  headers: HeaderMap,
  ws: WebSocketUpgrade,
) -> Response {
  let data = SignData::validate_header_map(&headers, &fake.token, "GET", "/api", b"").unwrap();
  let mut events = fake.events.subscribe();
  let mut res = ws.on_upgrade(move |mut socket| async move {
    let hello = json!({ "type": "Hello", "runtime": "docker" });
    if socket
      .send(AxumMessage::Binary(hello.to_string().into()))
      .await
      .is_err()
    {
      return;
    }
    loop {
      tokio::select! {
        Ok(event) = events.recv() => {
          if socket.send(AxumMessage::Binary(event.to_string().into())).await.is_err() {
            return;
          }
        }
        msg = socket.recv() => if !matches!(msg, Some(Ok(_))) { return },
      }
    }
  });
  data
    .add_response_to_header_map(res.headers_mut(), &fake.token)
    .unwrap();
  res
}

pub async fn fake_rest(
  State(fake): State<Fake>,
  method: Method,
  uri: Uri,
  headers: HeaderMap,
  body: Bytes,
) -> Response {
  let full = uri.path_and_query().unwrap().as_str();
  let data =
    SignData::validate_header_map(&headers, &fake.token, method.as_str(), full, &body).unwrap();
  let req = Req {
    method: method.clone(),
    path: uri.path().to_string(),
    query: url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
      .into_owned()
      .collect(),
    headers,
    body,
  };
  fake.requests.lock().unwrap().push(req.clone());

  let mut res = if let Some(res) = (fake.respond)(&req) {
    res
  } else if method == Method::PUT && req.path == "/api/servers" {
    let servers: HashMap<Uuid, Value> = fake
      .states
      .lock()
      .unwrap()
      .iter()
      .map(|(uuid, state)| (*uuid, json!({ "state": state, "last_install": null })))
      .collect();
    axum::Json(json!({ "orphans": [], "servers": servers })).into_response()
  } else if method == Method::POST && req.path.ends_with("/power") {
    (
      AxumStatus::ACCEPTED,
      axum::Json(json!({ "op": Uuid::now_v7() })),
    )
      .into_response()
  } else if method == Method::PUT {
    AxumStatus::NO_CONTENT.into_response()
  } else {
    wings_error(AxumStatus::NOT_FOUND, "not_found")
  };
  data
    .add_response_to_header_map(res.headers_mut(), &fake.token)
    .unwrap();
  res
}

pub async fn fake_wings(token: String, respond: Respond) -> (Fake, u16) {
  let fake = Fake {
    token,
    events: broadcast::channel(64).0,
    states: Default::default(),
    requests: Default::default(),
    respond,
  };
  let app = Router::new()
    .route("/api", get(fake_ws))
    .fallback(fake_rest)
    .layer(DefaultBodyLimit::disable())
    .with_state(fake.clone());
  let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
  let port = listener.local_addr().unwrap().port();
  spawn(async move { axum::serve(listener, app).await.unwrap() });
  (fake, port)
}

pub struct Ctx {
  pub server: TestServer,
  pub fake: Fake,
  /// The game server, installed and on the fake's node.
  pub uuid: Uuid,
}

impl Ctx {
  pub fn path(&self, rest: &str) -> String {
    format!("/servers/{}{rest}", self.uuid)
  }

  pub async fn power(&self, action: &str) -> StatusCode {
    self
      .server
      .post(&self.path("/power"), json!({ "action": action }))
      .await
      .status()
  }

  pub async fn wait_status(&self, status: &str) {
    timeout(Duration::from_secs(10), async {
      loop {
        let info: Value = self.server.get(&self.path("")).await.json().await.unwrap();
        if info["status"] == status {
          return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
      }
    })
    .await
    .unwrap_or_else(|_| panic!("server never became {status}"));
  }

  pub async fn set_state(&self, state: &str) {
    self.fake.set_state(self.uuid, state);
    self.wait_status(state).await;
  }

  /// A user with `access` on the server, logged in afterwards.
  pub async fn login_with(&self, access: Option<&str>) {
    let (user, email) = create_user(&self.server, "member").await;
    if let Some(access) = access {
      let mut update =
        super::update_body(&self.server.get(&self.path("")).await.json().await.unwrap());
      update["grants"] = json!({ "users": [{ "uuid": user, "access": access }], "groups": [] });
      let resp = self.server.put(&self.path(""), update).await;
      assert_eq!(resp.status(), StatusCode::OK);
    }
    self.server.clear_cookies();
    assert_eq!(
      self.server.login(&email, MEMBER_PASSWORD).await.status(),
      StatusCode::OK
    );
  }

  pub async fn set_backup_limit(&self, limit: i32) {
    let mut update =
      super::update_body(&self.server.get(&self.path("")).await.json().await.unwrap());
    update["backup_limit"] = json!(limit);
    let resp = self.server.put(&self.path(""), update).await;
    assert_eq!(resp.status(), StatusCode::OK);
  }

  pub async fn as_admin(&self) {
    self.server.clear_cookies();
    self.server.login("admin@example.com", "hunter2pass").await;
  }

  pub async fn ws(
    &self,
  ) -> Result<impl AsyncFnMut() -> Value, tokio_tungstenite::tungstenite::Error> {
    self.ws_from(None).await
  }

  /// [`Self::ws`] as a browser on `origin`.
  pub async fn ws_from(
    &self,
    origin: Option<&str>,
  ) -> Result<impl AsyncFnMut() -> Value, tokio_tungstenite::tungstenite::Error> {
    let mut req = format!(
      "ws://localhost:{}/api/servers/{}/ws",
      self.server.port, self.uuid
    )
    .into_client_request()
    .unwrap();
    req.headers_mut().insert(
      "Cookie",
      self.server.cookie_header().unwrap().parse().unwrap(),
    );
    if let Some(origin) = origin {
      req.headers_mut().insert("Origin", origin.parse().unwrap());
    }
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await?;
    Ok(async move || {
      loop {
        let msg = timeout(Duration::from_secs(5), ws.next())
          .await
          .expect("no frame received")
          .unwrap()
          .unwrap();
        if let Message::Text(text) = msg {
          return serde_json::from_str(&text).unwrap();
        }
      }
    })
  }
}

pub async fn setup(respond: impl Fn(&Req) -> Option<Response> + Send + Sync + 'static) -> Ctx {
  let (server, _) = TestServer::start_with_admin().await;
  let mut node_body = json!({
    "name": "alpha", "address": "http://127.0.0.1:1", "secure": false,
    "disk_limit_mb": null, "memory_limit_mb": null, "cpu_limit": null
  });
  let resp: Value = server
    .post("/nodes", node_body.clone())
    .await
    .json()
    .await
    .unwrap();
  let (node, token) = (
    super::uuid_of(&resp),
    resp["token"].as_str().unwrap().to_string(),
  );
  let egg = create_egg(&server, server_egg_body()).await;
  let mut body = server_body(node, egg, 25565);
  body["limits"] = json!({ "cpu": 1500, "memory": 0, "disk": 2 });
  let resp = server.post("/servers", body).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let uuid = super::uuid_of(&resp.json().await.unwrap());

  let (fake, port) = fake_wings(token, Arc::new(respond)).await;
  node_body["address"] = json!(format!("http://127.0.0.1:{port}"));
  let resp = server.post(&format!("/nodes/{node}"), node_body).await;
  assert_eq!(resp.status(), StatusCode::OK);

  let c = Ctx { server, fake, uuid };
  // synced: a server wings does not report is offline
  c.wait_status("offline").await;
  c
}

pub fn is(req: &Req, method: Method, suffix: &str) -> bool {
  req.method == method && req.path.ends_with(suffix)
}
