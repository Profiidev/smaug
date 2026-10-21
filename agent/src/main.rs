//! File helper next to every server. Wings calls it over HTTP; it is the only
//! process that touches the server's volume, mounted at `ROOT`.

use std::{
  path::PathBuf,
  sync::{Arc, Mutex},
};

use axum::{
  Json, Router,
  extract::{Request, State},
  http::{StatusCode, header::AUTHORIZATION},
  middleware::{self, Next},
  response::{IntoResponse, Response},
  routing::{get, post},
};
use shared::msg::{PrepareRequest, PrepareResponse};
use tracing::{error, info};

mod config_files;
mod files;

struct Agent {
  token: String,
  root: PathBuf,
  /// One prepare at a time. A request wings gave up on keeps running, a
  /// retry must not start a second walk over the volume next to it.
  prepare: Mutex<()>,
}

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt().init();
  let agent = Agent {
    token: std::env::var("TOKEN").expect("TOKEN must be set"),
    root: std::env::var("ROOT")
      .unwrap_or_else(|_| "/data".into())
      .into(),
    prepare: Mutex::new(()),
  };
  let port = std::env::var("PORT").unwrap_or_else(|_| "8000".into());
  let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
    .await
    .expect("Failed to bind");
  info!("Agent listening on {port}");
  axum::serve(listener, router(agent))
    .with_graceful_shutdown(async {
      let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .expect("Server failed");
}

fn router(agent: Agent) -> Router {
  let agent = Arc::new(agent);
  Router::new()
    .route("/health", get(|| async {}))
    .route("/prepare", post(prepare))
    .layer(middleware::from_fn_with_state(agent.clone(), auth))
    .with_state(agent)
}

async fn auth(State(agent): State<Arc<Agent>>, req: Request, next: Next) -> Response {
  let given = req
    .headers()
    .get(AUTHORIZATION)
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.strip_prefix("Bearer "))
    .unwrap_or_default();
  if !constant_eq(given.as_bytes(), agent.token.as_bytes()) {
    return StatusCode::UNAUTHORIZED.into_response();
  }
  next.run(req).await
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
  a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Everything Wings does before a start: config files, permissions, disk usage.
async fn prepare(
  State(agent): State<Arc<Agent>>,
  Json(req): Json<PrepareRequest>,
) -> Result<Json<PrepareResponse>, StatusCode> {
  tokio::task::spawn_blocking(move || {
    // held in the blocking task, it outlives a dropped request
    let _prepare = agent.prepare.lock().unwrap_or_else(|e| e.into_inner());
    let root = &agent.root;
    config_files::apply(root, &req.config_files, &req.interface);
    files::chown_all(root)?;
    Ok::<_, std::io::Error>(PrepareResponse {
      disk_bytes: files::disk_usage(root),
    })
  })
  .await
  .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
  .map(Json)
  .map_err(|err| {
    error!("Failed to prepare files: {err}");
    StatusCode::INTERNAL_SERVER_ERROR
  })
}

#[cfg(test)]
mod test {
  use axum::body::Body;
  use serde_json::json;
  use shared::msg::{ConfigFile, ConfigParser, ConfigReplace};
  use tower::ServiceExt;

  use super::*;

  #[tokio::test]
  async fn prepare_needs_token_and_rewrites_files() {
    let root = std::env::temp_dir().join(format!("smaug-agent-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let app = router(Agent {
      token: "secret".into(),
      root: root.clone(),
      prepare: Mutex::new(()),
    });
    let body = serde_json::to_string(&PrepareRequest {
      config_files: vec![ConfigFile {
        file: "server.properties".into(),
        parser: ConfigParser::Properties,
        replace: vec![ConfigReplace {
          matcher: "server-ip".into(),
          if_value: None,
          replace_with: json!("{{config.docker.interface}}"),
        }],
      }],
      interface: "172.18.0.1".into(),
    })
    .unwrap();
    let req = |token: &str| {
      Request::post("/prepare")
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.clone()))
        .unwrap()
    };

    let res = app.clone().oneshot(req("wrong")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(!root.join("server.properties").exists());

    let res = app.oneshot(req("secret")).await.unwrap();
    // chown to 988 needs root, so a normal test run only checks the rewrite
    if res.status() == StatusCode::OK {
      let bytes = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
      let res: PrepareResponse = serde_json::from_slice(&bytes).unwrap();
      assert_eq!(res.disk_bytes, "server-ip=172.18.0.1\n".len() as u64);
    }
    assert_eq!(
      std::fs::read_to_string(root.join("server.properties")).unwrap(),
      "server-ip=172.18.0.1\n"
    );
    std::fs::remove_dir_all(root).unwrap();
  }
}
