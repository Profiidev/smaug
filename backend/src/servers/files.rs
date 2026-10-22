//! File manager, proxied to the server's wings (which jails every path).

use std::time::Duration;

use aide::axum::{
  ApiRouter,
  routing::{get_with, post_with, put_with},
};
use axum::{
  Json,
  body::{Body, Bytes},
  extract::{DefaultBodyLimit, Path, Query},
  response::Response,
};
use centaurus::{
  bail,
  db::init::Connection,
  error::Result,
  eyre::{Context, eyre},
};
use futures_util::stream::try_unfold;
use http::{Method, StatusCode, header};
use reqwest_middleware::RequestBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shared::msg::{
  CompleteStream, CopyRequest, DEFAULT_CHUNK_SIZE, DeleteRequest, FileEntry, FileKind,
  MkdirRequest, Move, OpStarted, OpenInStream, PathQuery, RenameRequest, StreamOpened,
  StreamTarget,
};
use tokio::time::timeout;
use uuid::Uuid;

use super::{
  access::{Access, load},
  types::ServerUuid,
};
use crate::{
  auth::jwt_auth::JwtAuth,
  nodes::{Wings, call},
};

/// Largest file the editor opens.
const EDITOR_MAX: u64 = 1024 * 1024;
/// Room for an upload chunk plus headers of the extractors.
const BODY_LIMIT: usize = DEFAULT_CHUNK_SIZE as usize + 64 * 1024;
/// Longest wait for wings to checksum and move a finished upload.
const COMPLETE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub(super) fn router() -> ApiRouter {
  ApiRouter::new()
    .api_route("/", get_with(list_files, |op| op.id("list_files")))
    .api_route(
      "/content",
      get_with(file_content, |op| op.id("file_content"))
        .put_with(write_file_content, |op| op.id("write_file_content")),
    )
    .api_route(
      "/folder",
      post_with(create_folder, |op| op.id("create_folder")),
    )
    .api_route("/file", post_with(create_file, |op| op.id("create_file")))
    .api_route("/rename", post_with(rename_file, |op| op.id("rename_file")))
    .api_route("/copy", post_with(copy_file, |op| op.id("copy_file")))
    .api_route(
      "/delete",
      post_with(delete_files, |op| op.id("delete_files")),
    )
    .api_route(
      "/download",
      get_with(download_file, |op| {
        op.id("download_file").response::<200, Bytes>()
      }),
    )
    .api_route(
      "/upload",
      post_with(start_upload, |op| op.id("start_upload")),
    )
    .api_route(
      "/upload/{stream}",
      aide::axum::routing::delete_with(cancel_upload, |op| op.id("cancel_upload")),
    )
    .api_route(
      "/upload/{stream}/complete",
      post_with(complete_upload, |op| op.id("complete_upload")),
    )
    .api_route(
      "/upload/{stream}/{index}",
      put_with(upload_chunk, |op| op.id("upload_chunk")),
    )
    .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

#[derive(Deserialize, JsonSchema)]
struct PathRequest {
  path: String,
}

#[derive(Deserialize, JsonSchema)]
struct UploadRequest {
  /// Where the file ends up, replaced if it exists.
  path: String,
  size: u64,
}

/// Every chunk but the last is exactly `chunk_size` bytes, chunks may be sent
/// in parallel and in any order.
#[derive(Serialize, JsonSchema)]
struct UploadStarted {
  upload: Uuid,
  chunk_size: u64,
}

#[derive(Deserialize, JsonSchema)]
struct UploadPath {
  uuid: Uuid,
  stream: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct ChunkPath {
  uuid: Uuid,
  stream: Uuid,
  /// From 0, the chunk starts at `index * chunk_size`.
  index: u32,
}

/// The server's wings, once access is checked.
#[derive(Clone)]
struct Files {
  wings: Wings,
  node: Uuid,
  server: Uuid,
}

impl Files {
  async fn load(
    auth: &JwtAuth,
    db: &Connection,
    wings: Wings,
    uuid: Uuid,
    need: Access,
  ) -> Result<Self> {
    let (server, _) = load(db, auth.user_id, uuid, need).await?;
    Ok(Self {
      wings,
      node: server.node_id,
      server: server.id,
    })
  }

  fn req(&self, method: Method, path: &str) -> Result<RequestBuilder> {
    self
      .wings
      .server_request(self.node, self.server, method, path)
  }

  /// A request on one file, `path` as query.
  fn file(&self, method: Method, route: &str, path: &str) -> Result<RequestBuilder> {
    Ok(self.req(method, route)?.query(&[("path", path)]))
  }

  async fn stat(&self, path: &str) -> Result<FileEntry> {
    Ok(
      call(self.file(Method::GET, "/files/stat", path)?)
        .await?
        .json()
        .await
        .context("Invalid stat response from the node")?,
    )
  }

  async fn regular_file(&self, path: &str) -> Result<FileEntry> {
    let entry = self.stat(path).await?;
    if entry.kind != FileKind::File {
      bail!(BAD_REQUEST, "Not a file");
    }
    Ok(entry)
  }
}

/// Resolves `.` and `..` (never above the root) and empty segments; the
/// result starts with `/`, the server root.
fn normalize(path: &str) -> String {
  let mut parts = Vec::new();
  for part in path.split('/') {
    match part {
      "" | "." => (),
      ".." => {
        parts.pop();
      }
      part => parts.push(part),
    }
  }
  format!("/{}", parts.join("/"))
}

/// Like [`normalize`], but the root itself is refused.
fn below_root(path: &str) -> Result<String> {
  let path = normalize(path);
  if path == "/" {
    bail!(BAD_REQUEST, "Not allowed on the server root");
  }
  Ok(path)
}

async fn list_files(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Query(q): Query<PathQuery>,
) -> Result<Json<Vec<FileEntry>>> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Read).await?;
  let req = files.file(Method::GET, "/files/list", &normalize(&q.path))?;
  Ok(Json(
    call(req)
      .await?
      .json()
      .await
      .context("Invalid list response from the node")?,
  ))
}

/// For the editor, `413` above 1 MiB.
async fn file_content(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Query(q): Query<PathQuery>,
) -> Result<Bytes> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Read).await?;
  let path = normalize(&q.path);
  let size = files.regular_file(&path).await?.size;
  if size > EDITOR_MAX {
    bail!(PAYLOAD_TOO_LARGE, "File is too large for the editor");
  }
  // wings rejects any range of an empty file
  if size == 0 {
    return Ok(Bytes::new());
  }
  // one byte past the limit, so a file grown since the stat shows
  let req = files
    .file(Method::GET, "/files/content", &path)?
    .header(header::RANGE, format!("bytes=0-{EDITOR_MAX}"));
  let res = call(req).await?;
  let total = res
    .headers()
    .get(header::CONTENT_RANGE)
    .and_then(|v| v.to_str().ok())
    .and_then(range_total);
  let bytes = res
    .bytes()
    .await
    .context("Failed to read the file from the node")?;
  if total.unwrap_or(bytes.len() as u64) > EDITOR_MAX || bytes.len() as u64 > EDITOR_MAX {
    bail!(PAYLOAD_TOO_LARGE, "File is too large for the editor");
  }
  Ok(bytes)
}

/// The complete length in `bytes a-b/len`.
fn range_total(content_range: &str) -> Option<u64> {
  content_range.rsplit_once('/')?.1.trim().parse().ok()
}

/// Creates or replaces the file, at most one upload chunk.
async fn write_file_content(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Query(q): Query<PathQuery>,
  body: Bytes,
) -> Result<()> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  if body.len() as u64 > DEFAULT_CHUNK_SIZE {
    bail!(PAYLOAD_TOO_LARGE, "Use an upload for files this large");
  }
  let req = files.file(Method::PUT, "/files/content", &below_root(&q.path)?)?;
  call(req.body(body)).await?;
  Ok(())
}

async fn create_folder(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Json(req): Json<PathRequest>,
) -> Result<()> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  let body = MkdirRequest {
    path: below_root(&req.path)?,
  };
  call(files.req(Method::POST, "/files/mkdir")?.json(&body)).await?;
  Ok(())
}

/// Empty file, `409` when the path exists.
async fn create_file(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Json(req): Json<PathRequest>,
) -> Result<()> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  let path = below_root(&req.path)?;
  match files.stat(&path).await {
    Ok(_) => return Err(centaurus::anyhow!(CONFLICT, "File already exists")),
    Err(err) if err.status == StatusCode::NOT_FOUND => (),
    Err(err) => return Err(err),
  }
  call(files.file(Method::PUT, "/files/content", &path)?).await?;
  Ok(())
}

/// Also moves, `to` is the full new path.
async fn rename_file(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Json(req): Json<Move>,
) -> Result<()> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  let body = RenameRequest {
    moves: vec![Move {
      from: below_root(&req.from)?,
      to: below_root(&req.to)?,
    }],
  };
  call(files.req(Method::POST, "/files/rename")?.json(&body)).await?;
  Ok(())
}

/// Copies to `name copy.ext` next to it, returns the copy.
async fn copy_file(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Json(req): Json<PathRequest>,
) -> Result<Json<FileEntry>> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  let body = CopyRequest {
    path: below_root(&req.path)?,
  };
  Ok(Json(
    call(files.req(Method::POST, "/files/copy")?.json(&body))
      .await?
      .json()
      .await
      .context("Invalid copy response from the node")?,
  ))
}

/// Directories are deleted recursively.
async fn delete_files(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Json(req): Json<DeleteRequest>,
) -> Result<()> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  let body = DeleteRequest {
    paths: req
      .paths
      .iter()
      .map(|p| below_root(p))
      .collect::<Result<_>>()?,
  };
  if body.paths.is_empty() {
    return Ok(());
  }
  call(files.req(Method::POST, "/files/delete")?.json(&body)).await?;
  Ok(())
}

/// Streams the file as an attachment, read from wings in chunk-sized ranges.
async fn download_file(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Query(q): Query<PathQuery>,
) -> Result<Response> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Read).await?;
  let path = normalize(&q.path);
  let entry = files.regular_file(&path).await?;
  let size = entry.size;

  let pieces = try_unfold(0u64, move |offset| {
    let (files, path) = (files.clone(), path.clone());
    async move {
      if offset >= size {
        return Ok::<_, centaurus::error::ErrorReport>(None);
      }
      let end = (offset + DEFAULT_CHUNK_SIZE).min(size);
      let req = files
        .file(Method::GET, "/files/content", &path)?
        .header(header::RANGE, format!("bytes={offset}-{}", end - 1));
      let bytes = call(req)
        .await?
        .bytes()
        .await
        .context("Failed to read the file from the node")?;
      // changed meanwhile, a short or long piece would corrupt the download
      if bytes.len() as u64 != end - offset {
        bail!(CONFLICT, "File changed during the download");
      }
      Ok(Some((bytes, end)))
    }
  });

  Ok(
    Response::builder()
      .header(header::CONTENT_TYPE, "application/octet-stream")
      .header(header::CONTENT_LENGTH, size)
      .header(header::CONTENT_DISPOSITION, attachment(&entry.name))
      .body(Body::from_stream(pieces))
      .expect("valid headers"),
  )
}

/// `Content-Disposition` with an ASCII fallback and the UTF-8 name.
pub(super) fn attachment(name: &str) -> String {
  let ascii: String = name
    .chars()
    .map(|c| match c {
      '"' | '\\' => '_',
      c if c == ' ' || c.is_ascii_graphic() => c,
      _ => '_',
    })
    .collect();
  let encoded: String = name
    .bytes()
    .map(|b| match b {
      b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
        (b as char).to_string()
      }
      b => format!("%{b:02X}"),
    })
    .collect();
  format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

async fn start_upload(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ServerUuid>,
  Json(req): Json<UploadRequest>,
) -> Result<Json<UploadStarted>> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  let body = OpenInStream {
    target: StreamTarget::File {
      path: below_root(&req.path)?,
    },
    size: Some(req.size),
  };
  let opened: StreamOpened = call(files.req(Method::POST, "/streams/in")?.json(&body))
    .await?
    .json()
    .await
    .context("Invalid stream response from the node")?;
  Ok(Json(UploadStarted {
    upload: opened.stream,
    chunk_size: opened.chunk_size,
  }))
}

// ponytail: any stream of the server is reachable with its (random) id, track
// upload ids per user if streams the browser must not touch get guessable ids
async fn upload_chunk(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<ChunkPath>,
  body: Bytes,
) -> Result<()> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  let route = format!("/streams/in/{}/{}", p.stream, p.index);
  call(files.req(Method::PUT, &route)?.body(body)).await?;
  Ok(())
}

/// Returns once the file is in place, fails when wings' checksum or rename does.
async fn complete_upload(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<UploadPath>,
  Json(req): Json<CompleteStream>,
) -> Result<Json<OpStarted>> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  let route = format!("/streams/in/{}/complete", p.stream);
  let live = files.wings.live();
  // op id is the stream id, wings answers before the file is in place
  let finished = live.wait_op(p.stream);
  let started = match call(files.req(Method::POST, &route)?.json(&req)).await {
    Ok(res) => res
      .json()
      .await
      .context("Invalid complete response from the node")?,
    Err(err) => {
      live.forget_op(p.stream);
      return Err(err);
    }
  };
  match timeout(COMPLETE_TIMEOUT, finished).await {
    Ok(Ok(Ok(()))) => Ok(Json(started)),
    Ok(Ok(Err(err))) => Err(eyre!("Upload failed: {err}").into()),
    _ => {
      live.forget_op(p.stream);
      Err(eyre!("The node never finished the upload").into())
    }
  }
}

async fn cancel_upload(
  auth: JwtAuth,
  db: Connection,
  wings: Wings,
  Path(p): Path<UploadPath>,
) -> Result<()> {
  let files = Files::load(&auth, &db, wings, p.uuid, Access::Write).await?;
  call(files.req(Method::DELETE, &format!("/streams/{}", p.stream))?).await?;
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn paths_normalize_into_the_root() {
    for (input, expected) in [
      ("", "/"),
      ("/", "/"),
      ("a", "/a"),
      ("/a//b/./c/", "/a/b/c"),
      ("/a/../b", "/b"),
      ("../../etc/passwd", "/etc/passwd"),
      ("/a/b/../../..", "/"),
    ] {
      assert_eq!(normalize(input), expected, "{input}");
    }
    assert_eq!(
      below_root("/x/..").unwrap_err().status,
      StatusCode::BAD_REQUEST
    );
    assert_eq!(below_root("x").unwrap(), "/x");
  }

  #[test]
  fn attachment_escapes_names() {
    assert_eq!(
      attachment("a b.txt"),
      "attachment; filename=\"a b.txt\"; filename*=UTF-8''a%20b.txt"
    );
    assert_eq!(
      attachment("ä\"x"),
      "attachment; filename=\"__x\"; filename*=UTF-8''%C3%A4%22x"
    );
  }
}
