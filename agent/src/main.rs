//! File helper next to every server. Wings calls it over HTTP; it is the only
//! process that touches the server's volume, mounted at `ROOT`.

use std::{
  path::PathBuf,
  sync::{Arc, Mutex},
};

use axum::{
  Json, Router,
  extract::DefaultBodyLimit,
  extract::{Request, State},
  http::{StatusCode, header::AUTHORIZATION},
  middleware::{self, Next},
  response::{IntoResponse, Response},
  routing::{get, post, put},
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
  /// Last disk usage and when it was measured, for quota checks.
  usage: Mutex<files::Usage>,
}

/// Same as wings' request limit, one chunk always fits.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt().init();
  let agent = Agent {
    token: std::env::var("TOKEN").expect("TOKEN must be set"),
    root: std::env::var("ROOT")
      .unwrap_or_else(|_| "/data".into())
      .into(),
    prepare: Mutex::new(()),
    usage: Default::default(),
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
    .route("/files/list", get(files::list))
    .route("/files/stat", get(files::stat))
    .route("/files/content", get(files::read).put(files::write))
    .route("/files/write-at", put(files::write_at))
    .route("/files/mkdir", post(files::mkdir))
    .route("/files/rename", post(files::rename))
    .route("/files/copy", post(files::copy))
    .route("/files/delete", post(files::delete))
    .route("/files/usage", get(files::usage))
    .route("/archive", get(files::archive))
    .route("/extract", put(files::extract))
    .route("/extract/commit", post(files::commit))
    .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
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
    config_files::apply(&agent.root, &req.config_files, &req.interface);
    files::chown_all(&agent.root)?;
    Ok::<_, std::io::Error>(PrepareResponse {
      disk_bytes: agent.fresh_usage(),
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
      usage: Default::default(),
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

  fn app(root: &std::path::Path) -> Router {
    router(Agent {
      token: "secret".into(),
      root: root.to_path_buf(),
      prepare: Mutex::new(()),
      usage: Default::default(),
    })
  }

  fn tmp() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("smaug-agent-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    root
  }

  async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: impl Into<Body>,
  ) -> (StatusCode, axum::body::Bytes) {
    let mut req = Request::builder()
      .method(method)
      .uri(uri)
      .header(AUTHORIZATION, "Bearer secret")
      .header("content-type", "application/json");
    for (k, v) in headers {
      req = req.header(*k, *v);
    }
    let res = app
      .clone()
      .oneshot(req.body(body.into()).unwrap())
      .await
      .unwrap();
    let status = res.status();
    (
      status,
      axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap(),
    )
  }

  #[tokio::test]
  async fn list_is_ordered_and_hides_uploads() {
    let root = tmp();
    std::fs::write(root.join("b.txt"), "b").unwrap();
    std::fs::write(root.join("A.yml"), "a").unwrap();
    std::fs::create_dir_all(root.join("z")).unwrap();
    std::fs::create_dir_all(root.join(files::UPLOADS)).unwrap();
    let app = app(&root);
    let (status, body) = call(&app, "GET", "/files/list?path=/", &[], Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    let list: Vec<shared::msg::FileEntry> = serde_json::from_slice(&body).unwrap();
    let names: Vec<_> = list.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["z", "A.yml", "b.txt"]);
    assert_eq!(list[0].mime, None);
    assert_eq!(list[2].mime.as_deref(), Some("text/plain"));

    let (status, body) = call(&app, "GET", "/files/list?path=../", &[], Body::empty()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let err: shared::msg::ErrorBody = serde_json::from_slice(&body).unwrap();
    assert_eq!(err.code, shared::msg::ErrorCode::BadRequest);
    std::fs::remove_dir_all(root).unwrap();
  }

  #[tokio::test]
  async fn denylist_blocks_reads_and_writes() {
    let root = tmp();
    std::fs::create_dir_all(root.join("secret")).unwrap();
    std::fs::write(root.join("secret/key"), "k").unwrap();
    let app = app(&root);
    let deny = [(files::DENYLIST_HEADER, r#"["secret/", "*.jar"]"#)];
    for (method, uri) in [
      ("GET", "/files/content?path=secret/key"),
      ("PUT", "/files/content?path=server.jar"),
      ("PUT", "/files/write-at?path=/plugins/x.jar&offset=0"),
    ] {
      let (status, _) = call(&app, method, uri, &deny, Body::from("x")).await;
      assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
    }
    let (status, _) = call(
      &app,
      "POST",
      "/files/rename",
      &deny,
      r#"{"moves":[{"from":"secret/key","to":"key"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // without the header nothing is denied
    let (status, body) = call(
      &app,
      "GET",
      "/files/content?path=secret/key",
      &[],
      Body::empty(),
    )
    .await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"k"[..]));
    std::fs::remove_dir_all(root).unwrap();
  }

  #[tokio::test]
  async fn write_read_range_copy_rename_delete() {
    let root = tmp();
    let app = app(&root);
    let (status, _) = call(
      &app,
      "PUT",
      "/files/content?path=/cfg/a.txt",
      &[],
      "0123456789",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = call(
      &app,
      "GET",
      "/files/content?path=/cfg/a.txt",
      &[("range", "bytes=2-4")],
      Body::empty(),
    )
    .await;
    assert_eq!(
      (status, &body[..]),
      (StatusCode::PARTIAL_CONTENT, &b"234"[..])
    );

    // quota: 10 bytes used, 5 more don't fit in 12
    let (status, body) = call(
      &app,
      "PUT",
      "/files/content?path=b.txt",
      &[(files::DISK_LIMIT_HEADER, "12")],
      "01234",
    )
    .await;
    assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE, "{body:?}");

    let (status, body) = call(&app, "POST", "/files/copy", &[], r#"{"path":"cfg"}"#).await;
    assert_eq!(status, StatusCode::OK);
    let entry: shared::msg::FileEntry = serde_json::from_slice(&body).unwrap();
    assert_eq!(entry.name, "cfg copy");
    assert_eq!(
      std::fs::read_to_string(root.join("cfg copy/a.txt")).unwrap(),
      "0123456789"
    );

    let rename = r#"{"moves":[{"from":"cfg copy/a.txt","to":"cfg/a.txt"}]}"#;
    let (status, _) = call(&app, "POST", "/files/rename", &[], rename).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(&app, "POST", "/files/rename?overwrite=true", &[], rename).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!root.join("cfg copy/a.txt").exists());

    let (status, _) = call(
      &app,
      "POST",
      "/files/delete",
      &[],
      r#"{"paths":["cfg","nope"]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!root.join("cfg").exists());
    let (status, _) = call(&app, "POST", "/files/delete", &[], r#"{"paths":["/"]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    std::fs::remove_dir_all(root).unwrap();
  }

  #[tokio::test]
  async fn write_at_fills_any_order() {
    let root = tmp();
    let app = app(&root);
    let up = format!("{}/s/part", files::UPLOADS);
    for (offset, chunk) in [(4, "4567"), (0, "0123"), (8, "89")] {
      let uri = format!("/files/write-at?path={up}&offset={offset}");
      let (status, _) = call(&app, "PUT", &uri, &[], chunk).await;
      assert_eq!(status, StatusCode::NO_CONTENT);
    }
    let rename = format!(r#"{{"moves":[{{"from":"{up}","to":"big.bin"}}]}}"#);
    let (status, _) = call(&app, "POST", "/files/rename", &[], rename).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
      std::fs::read_to_string(root.join("big.bin")).unwrap(),
      "0123456789"
    );
    std::fs::remove_dir_all(root).unwrap();
  }

  #[tokio::test]
  async fn archive_respects_ignores_and_extracts() {
    let root = tmp();
    std::fs::create_dir_all(root.join("world/region")).unwrap();
    std::fs::create_dir_all(root.join("logs")).unwrap();
    std::fs::create_dir_all(root.join(files::UPLOADS)).unwrap();
    std::fs::write(root.join("world/region/r.0.mca"), "chunk").unwrap();
    std::fs::write(root.join("logs/latest.log"), "log").unwrap();
    std::fs::write(root.join("cache.tmp"), "tmp").unwrap();
    std::fs::write(root.join(files::UPLOADS).join("x"), "up").unwrap();
    std::fs::write(root.join(".pteroignore"), "*.tmp\n").unwrap();
    std::os::unix::fs::symlink("/etc/passwd", root.join("passwd")).unwrap();
    let src = app(&root);

    let (status, tar) = call(
      &src,
      "GET",
      "/archive?ignore=logs%2F%0A",
      &[],
      Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let dst = tmp();
    std::fs::write(dst.join("old.txt"), "old").unwrap();
    std::fs::write(dst.join("keep.key"), "deny").unwrap();
    let deny = [(files::DENYLIST_HEADER, r#"["*.key"]"#)];
    let (status, body) = call(&app(&dst), "PUT", "/extract?stage=r1", &[], tar.clone()).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body:?}");
    // staged only, the live files are untouched until the commit
    assert!(dst.join("old.txt").exists());
    assert!(!dst.join("world").exists());
    let (status, _) = call(
      &app(&dst),
      "POST",
      "/extract/commit?stage=r1&truncate=true",
      &deny,
      Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!dst.join("old.txt").exists());
    // denied files survive a truncate
    assert!(dst.join("keep.key").exists());
    assert!(!dst.join(files::UPLOADS).join("r1").exists());
    assert!(!dst.join("logs").exists());
    assert!(!dst.join("cache.tmp").exists());
    assert_eq!(
      std::fs::read_to_string(dst.join("world/region/r.0.mca")).unwrap(),
      "chunk"
    );
    // a link out of the root is skipped
    assert!(std::fs::symlink_metadata(dst.join("passwd")).is_err());

    // quota counts what the archive unpacks, a failed stage is removed
    let (status, _) = call(
      &app(&dst),
      "PUT",
      "/extract?stage=r2",
      &[(files::DISK_LIMIT_HEADER, "3")],
      tar,
    )
    .await;
    assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);
    assert!(!dst.join(files::UPLOADS).join("r2").exists());
    let (status, _) = call(
      &app(&dst),
      "POST",
      "/extract/commit?stage=r2",
      &[],
      Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app(&dst), "PUT", "/extract?stage=..", &[], Body::empty()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(dst).unwrap();
  }

  #[tokio::test]
  async fn extract_refuses_zip_slip() {
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_mode(0o644);
    // set_path refuses `..`, so write the raw name
    header.as_old_mut().name[..9].copy_from_slice(b"../escape");
    header.set_cksum();
    let mut enc = zstd::Encoder::new(Vec::new(), 3).unwrap();
    let mut inner = tar::Builder::new(&mut enc);
    inner.append(&header, &b"x"[..]).unwrap();
    inner.finish().unwrap();
    drop(inner);
    let data = enc.finish().unwrap();

    let dst = tmp();
    let (status, _) = call(&app(&dst), "PUT", "/extract?stage=s", &[], data).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!dst.parent().unwrap().join("escape").exists());
    std::fs::remove_dir_all(dst).unwrap();
  }

  #[tokio::test]
  async fn archive_and_copy_skip_the_denylist() {
    let root = tmp();
    std::fs::create_dir_all(root.join("cfg/secret")).unwrap();
    std::fs::write(root.join("cfg/a.txt"), "a").unwrap();
    std::fs::write(root.join("cfg/b.key"), "k").unwrap();
    std::fs::write(root.join("cfg/secret/x"), "x").unwrap();
    let app = app(&root);
    let deny = [(files::DENYLIST_HEADER, r#"["*.key", "secret/"]"#)];

    let (status, _) = call(&app, "POST", "/files/copy", &deny, r#"{"path":"cfg"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert!(root.join("cfg copy/a.txt").exists());
    assert!(!root.join("cfg copy/b.key").exists());
    assert!(!root.join("cfg copy/secret").exists());
    let (status, _) = call(
      &app,
      "POST",
      "/files/copy",
      &deny,
      r#"{"path":"cfg/b.key"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, tar) = call(&app, "GET", "/archive", &deny, Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    let dec = zstd::Decoder::new(&tar[..]).unwrap();
    let names: Vec<String> = tar::Archive::new(dec)
      .entries()
      .unwrap()
      .map(|e| e.unwrap().path().unwrap().to_string_lossy().into_owned())
      .collect();
    assert!(names.iter().any(|n| n == "cfg/a.txt"), "{names:?}");
    assert!(
      !names
        .iter()
        .any(|n| n.ends_with(".key") || n.contains("secret")),
      "{names:?}"
    );
    std::fs::remove_dir_all(root).unwrap();
  }

  #[tokio::test]
  async fn merge_restore_overwrites_and_keeps_the_rest() {
    let src = tmp();
    std::fs::write(src.join("a.txt"), "new").unwrap();
    let (_, tar) = call(&app(&src), "GET", "/archive", &[], Body::empty()).await;
    let dst = tmp();
    std::fs::write(dst.join("a.txt"), "old").unwrap();
    std::fs::write(dst.join("b.txt"), "b").unwrap();
    let dst_app = app(&dst);
    let (status, _) = call(&dst_app, "PUT", "/extract?stage=m", &[], tar).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(
      &dst_app,
      "POST",
      "/extract/commit?stage=m",
      &[],
      Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "new");
    assert_eq!(std::fs::read_to_string(dst.join("b.txt")).unwrap(), "b");
    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dst).unwrap();
  }

  #[tokio::test]
  async fn reading_a_fifo_does_not_block() {
    let root = tmp();
    let fifo = std::ffi::CString::new(root.join("pipe").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o644) }, 0);
    let res = tokio::time::timeout(
      std::time::Duration::from_secs(5),
      call(
        &app(&root),
        "GET",
        "/files/content?path=pipe",
        &[],
        Body::empty(),
      ),
    )
    .await
    .expect("open blocked");
    assert_eq!(res.0, StatusCode::BAD_REQUEST);
    std::fs::remove_dir_all(root).unwrap();
  }
}
