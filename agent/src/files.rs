//! `/files/*`, `/archive` and `/extract`. Every user path goes through
//! [`jail`], every user read or write through the denylist.

use std::{
  fs::{self, File, OpenOptions},
  io::{self, Read, Seek, SeekFrom, Write},
  os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
  path::{Component, Path, PathBuf},
  sync::Arc,
  time::{Duration, Instant},
};

use axum::{
  Json,
  body::{Body, Bytes},
  extract::{FromRequestParts, Query, State},
  http::{HeaderMap, StatusCode, header, request::Parts},
  response::{IntoResponse, Response},
};
use futures_util::TryStreamExt;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::Deserialize;
use shared::msg::{
  CopyRequest, DeleteRequest, ErrorBody, ErrorCode, FileEntry, FileKind, MkdirRequest, PathQuery,
  RenameRequest, UsageResponse,
};
use tokio::sync::mpsc;
use tracing::warn;

use crate::Agent;

/// Same uid/gid as Pterodactyl's default `pterodactyl` user, the user every
/// server container runs as.
pub const CONTAINER_UID: u32 = 988;
/// Wings stages chunked uploads here (via `/files/write-at`), hidden from
/// listings and archives.
pub const UPLOADS: &str = ".smaug-uploads";
/// JSON array of gitignore patterns, the server's `file_denylist`.
pub const DENYLIST_HEADER: &str = "x-denylist";
/// Disk quota in bytes, no header means no quota.
pub const DISK_LIMIT_HEADER: &str = "x-disk-limit";
/// Ignore files merged into the `/archive` ignore list.
const IGNORE_FILES: [&str; 3] = [".smaugignore", ".pteroignore", ".pelicanignore"];
/// Disk usage is walked again once it is this old.
const USAGE_TTL: Duration = Duration::from_secs(30);

/// Recursively chowns to the container user without following symlinks, like
/// Wings' `CheckPermissionsOnBoot`.
pub fn chown_all(root: &Path) -> io::Result<()> {
  for entry in walkdir::WalkDir::new(root) {
    let entry = entry?;
    std::os::unix::fs::lchown(entry.path(), Some(CONTAINER_UID), Some(CONTAINER_UID))?;
  }
  Ok(())
}

/// Best effort, only works when the agent runs as root (it does in its image).
fn chown(path: &Path) {
  let _ = std::os::unix::fs::lchown(path, Some(CONTAINER_UID), Some(CONTAINER_UID));
}

/// Bytes used under `root`, symlinks not followed.
pub fn disk_usage(root: &Path) -> u64 {
  walkdir::WalkDir::new(root)
    .into_iter()
    .filter_map(|e| e.ok()?.metadata().ok())
    .filter(|m| m.is_file())
    .map(|m| m.len())
    .sum()
}

#[derive(Debug)]
pub struct Error(ErrorCode, String);

impl Error {
  fn bad(msg: impl Into<String>) -> Self {
    Self(ErrorCode::BadRequest, msg.into())
  }
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.1)
  }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
  fn from(err: io::Error) -> Self {
    let code = match err.kind() {
      io::ErrorKind::NotFound => ErrorCode::NotFound,
      io::ErrorKind::AlreadyExists | io::ErrorKind::DirectoryNotEmpty => ErrorCode::Conflict,
      io::ErrorKind::InvalidInput
      | io::ErrorKind::InvalidFilename
      | io::ErrorKind::IsADirectory
      | io::ErrorKind::NotADirectory => ErrorCode::BadRequest,
      // O_NOFOLLOW on a symlink
      _ if err.raw_os_error() == Some(libc::ELOOP) => ErrorCode::BadRequest,
      io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => ErrorCode::Quota,
      _ => ErrorCode::Internal,
    };
    Self(code, err.to_string())
  }
}

impl IntoResponse for Error {
  fn into_response(self) -> Response {
    let status = match self.0 {
      ErrorCode::BadRequest => StatusCode::BAD_REQUEST,
      ErrorCode::Denied => StatusCode::FORBIDDEN,
      ErrorCode::NotFound => StatusCode::NOT_FOUND,
      ErrorCode::Conflict | ErrorCode::Busy => StatusCode::CONFLICT,
      ErrorCode::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
      ErrorCode::Unsupported => StatusCode::NOT_IMPLEMENTED,
      ErrorCode::Quota => StatusCode::INSUFFICIENT_STORAGE,
      ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let body = ErrorBody {
      code: self.0,
      message: self.1,
    };
    (status, Json(body)).into_response()
  }
}

type Result<T, E = Error> = std::result::Result<T, E>;

/// Resolves the user path `path` inside `root`. Rejects `..` and a symlink as
/// any but the last component, so nothing is reached through a link. The last
/// component is not followed either: callers lstat it or open it with
/// `O_NOFOLLOW`, renames and deletes act on the link itself.
///
/// ponytail: check-then-use, a running game could swap a checked directory
/// for a symlink in between. Move to `openat2(RESOLVE_IN_ROOT)` fds if that
/// race matters.
pub fn jail(root: &Path, path: &str) -> Result<PathBuf> {
  let mut out = root.to_path_buf();
  for component in Path::new(path).components() {
    match component {
      Component::Normal(part) => {
        if out != root && fs::symlink_metadata(&out).is_ok_and(|m| m.file_type().is_symlink()) {
          return Err(Error::bad("Path goes through a symlink"));
        }
        out.push(part);
      }
      Component::RootDir | Component::CurDir => (),
      _ => return Err(Error::bad("Path escapes the server root")),
    }
  }
  Ok(out)
}

/// Per request view of the agent: jail, denylist and quota.
pub struct Files {
  agent: Arc<Agent>,
  deny: Gitignore,
  limit: Option<u64>,
}

impl FromRequestParts<Arc<Agent>> for Files {
  type Rejection = Error;

  async fn from_request_parts(parts: &mut Parts, agent: &Arc<Agent>) -> Result<Self> {
    let deny = deny_from(&agent.root, &parts.headers)?;
    let limit = match parts.headers.get(DISK_LIMIT_HEADER) {
      Some(v) => Some(
        v.to_str()
          .ok()
          .and_then(|v| v.parse().ok())
          .ok_or_else(|| Error::bad("Invalid disk limit"))?,
      ),
      None => None,
    };
    Ok(Self {
      agent: agent.clone(),
      deny,
      limit,
    })
  }
}

fn deny_from(root: &Path, headers: &HeaderMap) -> Result<Gitignore> {
  let patterns: Vec<String> = match headers.get(DENYLIST_HEADER) {
    Some(v) => serde_json::from_slice(v.as_bytes()).map_err(|_| Error::bad("Invalid denylist"))?,
    None => Vec::new(),
  };
  let mut builder = GitignoreBuilder::new(root);
  for p in &patterns {
    builder
      .add_line(None, p)
      .map_err(|e| Error::bad(e.to_string()))?;
  }
  builder.build().map_err(|e| Error::bad(e.to_string()))
}

impl Files {
  fn root(&self) -> &Path {
    &self.agent.root
  }

  /// Jailed path the denylist allows.
  fn path(&self, path: &str) -> Result<PathBuf> {
    let path = jail(self.root(), path)?;
    if path != self.root() && self.denied(&path, path.is_dir()) {
      return Err(Error(ErrorCode::Denied, "Path is on the denylist".into()));
    }
    Ok(path)
  }

  fn denied(&self, path: &Path, is_dir: bool) -> bool {
    self
      .deny
      .matched_path_or_any_parents(path, is_dir)
      .is_ignore()
  }

  /// Holds `more` bytes of the quota until settled or dropped, so parallel
  /// writes can't pass the disk limit together. Fails with `quota` if they would.
  fn reserve(&self, more: u64) -> Result<Reserved<'_>> {
    let mut usage = self.agent.usage.lock().unwrap_or_else(|e| e.into_inner());
    let used = usage.current(&self.agent.root);
    if let Some(limit) = self.limit
      && more > 0
      && used.saturating_add(more) > limit
    {
      return Err(Error(ErrorCode::Quota, "Disk quota exceeded".into()));
    }
    usage.inflight += more;
    Ok(Reserved {
      agent: &self.agent,
      bytes: more,
    })
  }
}

/// Disk usage for quota checks: the last walk plus what was written since.
/// ponytail: a write that shrinks or deletes is only seen by the next walk,
/// and parallel chunks of one new file each reserve up to their end, both
/// overcount until the next walk (at most `USAGE_TTL`).
#[derive(Default)]
pub struct Usage {
  walked: Option<(Instant, u64)>,
  /// Settled writes since the walk.
  written: u64,
  /// Reserved by writes still running.
  inflight: u64,
}

impl Usage {
  fn walk(&mut self, root: &Path) -> u64 {
    let bytes = disk_usage(root);
    self.walked = Some((Instant::now(), bytes));
    self.written = 0;
    bytes
  }

  fn current(&mut self, root: &Path) -> u64 {
    let walked = match self.walked {
      Some((at, bytes)) if at.elapsed() < USAGE_TTL => bytes,
      _ => self.walk(root),
    };
    walked + self.written + self.inflight
  }
}

/// A quota reservation, released on drop unless settled.
struct Reserved<'a> {
  agent: &'a Agent,
  bytes: u64,
}

impl Reserved<'_> {
  /// The bytes were written, they count until the next walk.
  fn settle(mut self) {
    let mut usage = self.agent.usage.lock().unwrap_or_else(|e| e.into_inner());
    usage.inflight -= self.bytes;
    usage.written += self.bytes;
    self.bytes = 0;
  }
}

impl Drop for Reserved<'_> {
  fn drop(&mut self) {
    let mut usage = self.agent.usage.lock().unwrap_or_else(|e| e.into_inner());
    usage.inflight -= self.bytes;
  }
}

impl Agent {
  pub fn cached_usage(&self) -> u64 {
    let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
    usage.current(&self.root)
  }

  pub fn fresh_usage(&self) -> u64 {
    let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
    usage.walk(&self.root)
  }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
  tokio::task::spawn_blocking(f)
    .await
    .map_err(|e| Error(ErrorCode::Internal, e.to_string()))?
}

pub fn entry(path: &Path) -> io::Result<FileEntry> {
  let meta = fs::symlink_metadata(path)?;
  let kind = if meta.file_type().is_symlink() {
    FileKind::Symlink
  } else if meta.is_dir() {
    FileKind::Dir
  } else {
    FileKind::File
  };
  let name = path
    .file_name()
    .map(|n| n.to_string_lossy().into_owned())
    .unwrap_or_default();
  Ok(FileEntry {
    mime: (kind != FileKind::Dir).then(|| {
      mime_guess::from_path(&name)
        .first_or_octet_stream()
        .to_string()
    }),
    name,
    kind,
    size: meta.len(),
    mode: meta.mode() & 0o7777,
    modified: meta.modified()?.into(),
  })
}

fn open_nofollow() -> OpenOptions {
  let mut opts = OpenOptions::new();
  opts.custom_flags(libc::O_NOFOLLOW);
  opts
}

/// Creates `dir` and its missing parents, owned by the container user.
fn mkdirs(dir: &Path) -> io::Result<()> {
  if dir.is_dir() {
    return Ok(());
  }
  if let Some(parent) = dir.parent() {
    mkdirs(parent)?;
  }
  match fs::create_dir(dir) {
    Ok(()) => chown(dir),
    Err(e) if e.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() => (),
    Err(e) => return Err(e),
  }
  Ok(())
}

fn size_of(path: &Path) -> io::Result<u64> {
  let meta = fs::symlink_metadata(path)?;
  Ok(if meta.is_dir() {
    disk_usage(path)
  } else {
    meta.len()
  })
}

pub async fn list(files: Files, Query(q): Query<PathQuery>) -> Result<Json<Vec<FileEntry>>> {
  let dir = files.path(&q.path)?;
  let at_root = dir == files.root();
  blocking(move || {
    let mut out = Vec::new();
    for e in fs::read_dir(&dir)? {
      let e = e?;
      if at_root && e.file_name() == UPLOADS {
        continue;
      }
      out.push(entry(&e.path())?);
    }
    out.sort_by(|a, b| {
      (b.kind == FileKind::Dir)
        .cmp(&(a.kind == FileKind::Dir))
        .then_with(|| a.name.cmp(&b.name))
    });
    Ok(Json(out))
  })
  .await
}

pub async fn stat(files: Files, Query(q): Query<PathQuery>) -> Result<Json<FileEntry>> {
  let path = files.path(&q.path)?;
  Ok(Json(entry(&path)?))
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n`, as an inclusive range in `len`.
fn parse_range(value: &str, len: u64) -> Option<(u64, u64)> {
  let (a, b) = value.strip_prefix("bytes=")?.split_once('-')?;
  let (start, end) = match (a.trim(), b.trim()) {
    ("", n) => (len.checked_sub(n.parse().ok()?)?, len.checked_sub(1)?),
    (a, "") => (a.parse().ok()?, len.checked_sub(1)?),
    (a, b) => (
      a.parse().ok()?,
      b.parse::<u64>().ok()?.min(len.checked_sub(1)?),
    ),
  };
  (start <= end).then_some((start, end))
}

pub async fn read(
  files: Files,
  Query(q): Query<PathQuery>,
  headers: HeaderMap,
) -> Result<Response> {
  let path = files.path(&q.path)?;
  // O_NONBLOCK: opening a FIFO would wait for a writer
  let (mut file, len) = blocking(move || {
    let file = open_nofollow()
      .read(true)
      .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
      .open(&path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
      return Err(Error::bad("Path is not a regular file"));
    }
    Ok((file, meta.len()))
  })
  .await?;
  let range = match headers.get(header::RANGE) {
    Some(v) => Some(
      v.to_str()
        .ok()
        .and_then(|v| parse_range(v, len))
        .ok_or_else(|| Error::bad("Invalid range"))?,
    ),
    None => None,
  };
  let (status, start, count) = match range {
    Some((a, b)) => (StatusCode::PARTIAL_CONTENT, a, b - a + 1),
    None => (StatusCode::OK, 0, len),
  };
  file.seek(SeekFrom::Start(start))?;
  let body = Body::from_stream(tokio_util::io::ReaderStream::new(
    tokio::io::AsyncReadExt::take(tokio::fs::File::from_std(file), count),
  ));
  let mut res = (status, [(header::CONTENT_LENGTH, count.to_string())], body).into_response();
  if let Some((a, b)) = range {
    res.headers_mut().insert(
      header::CONTENT_RANGE,
      format!("bytes {a}-{b}/{len}").parse().unwrap(),
    );
  }
  Ok(res)
}

/// Atomic write: temp file next to `path`, then rename over it.
pub async fn write(files: Files, Query(q): Query<PathQuery>, body: Bytes) -> Result<StatusCode> {
  let path = files.path(&q.path)?;
  if path == files.root() {
    return Err(Error::bad("Path is the root"));
  }
  blocking(move || {
    let old = fs::symlink_metadata(&path).map(|m| m.len()).unwrap_or(0);
    let reserved = files.reserve((body.len() as u64).saturating_sub(old))?;
    let parent = path
      .parent()
      .ok_or_else(|| Error::bad("Path is the root"))?;
    mkdirs(parent)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos();
    let tmp = parent.join(format!(".{name}.{nanos}.smaug-tmp"));
    let res = (|| {
      let mut f = open_nofollow().write(true).create_new(true).open(&tmp)?;
      f.write_all(&body)?;
      f.sync_all()?;
      chown(&tmp);
      fs::rename(&tmp, &path)
    })();
    if res.is_err() {
      let _ = fs::remove_file(&tmp);
    }
    res?;
    reserved.settle();
    Ok(StatusCode::NO_CONTENT)
  })
  .await
}

#[derive(Deserialize)]
pub struct WriteAtQuery {
  path: String,
  offset: u64,
}

/// Positional write for chunked uploads, creates the file if missing.
pub async fn write_at(
  files: Files,
  Query(q): Query<WriteAtQuery>,
  body: Bytes,
) -> Result<StatusCode> {
  let path = files.path(&q.path)?;
  if path == files.root() {
    return Err(Error::bad("Path is the root"));
  }
  blocking(move || {
    let old = fs::symlink_metadata(&path).map(|m| m.len()).unwrap_or(0);
    let end = q
      .offset
      .checked_add(body.len() as u64)
      .ok_or_else(|| Error::bad("Offset too large"))?;
    let reserved = files.reserve(end.saturating_sub(old))?;
    mkdirs(
      path
        .parent()
        .ok_or_else(|| Error::bad("Path is the root"))?,
    )?;
    let f = open_nofollow().write(true).create(true).open(&path)?;
    f.write_all_at(&body, q.offset)?;
    reserved.settle();
    chown(&path);
    Ok(StatusCode::NO_CONTENT)
  })
  .await
}

pub async fn mkdir(files: Files, Json(req): Json<MkdirRequest>) -> Result<StatusCode> {
  let path = files.path(&req.path)?;
  blocking(move || {
    mkdirs(&path)?;
    Ok(StatusCode::NO_CONTENT)
  })
  .await
}

#[derive(Deserialize)]
pub struct RenameQuery {
  #[serde(default)]
  overwrite: bool,
}

/// Every `to` is a full path from the root. An existing `to` is a conflict
/// unless `overwrite` (finishing an upload), which replaces a file atomically.
pub async fn rename(
  files: Files,
  Query(q): Query<RenameQuery>,
  Json(req): Json<RenameRequest>,
) -> Result<StatusCode> {
  let moves = req
    .moves
    .iter()
    .map(|m| Ok((files.path(&m.from)?, files.path(&m.to)?)))
    .collect::<Result<Vec<_>>>()?;
  let root = files.root().to_path_buf();
  blocking(move || {
    for (from, to) in moves {
      if from == root || to == root {
        return Err(Error::bad("Can't move the root"));
      }
      if to.starts_with(&from) {
        return Err(Error::bad("Can't move a directory into itself"));
      }
      fs::symlink_metadata(&from)?;
      if !q.overwrite && fs::symlink_metadata(&to).is_ok() {
        return Err(Error(ErrorCode::Conflict, "Target already exists".into()));
      }
      mkdirs(to.parent().unwrap_or(&root))?;
      fs::rename(&from, &to)?;
    }
    Ok(StatusCode::NO_CONTENT)
  })
  .await
}

/// `name copy.ext`, then `name copy 2.ext`, ... next to `path`.
fn copy_target(path: &Path) -> PathBuf {
  let name = path.file_name().unwrap_or_default().to_string_lossy();
  let (stem, ext) = match name.rfind('.') {
    Some(i) if i > 0 && !path.is_dir() => (&name[..i], &name[i..]),
    _ => (&name[..], ""),
  };
  (1..)
    .map(|n| match n {
      1 => path.with_file_name(format!("{stem} copy{ext}")),
      n => path.with_file_name(format!("{stem} copy {n}{ext}")),
    })
    .find(|p| fs::symlink_metadata(p).is_err())
    .unwrap()
}

/// Copies files and directories, symlinks are copied as links. Skips what
/// `skip` matches and special files (a FIFO would block the copy).
fn copy_all(from: &Path, to: &Path, skip: impl Fn(&Path, bool) -> bool) -> io::Result<()> {
  let walk = walkdir::WalkDir::new(from)
    .follow_root_links(false)
    .into_iter()
    .filter_entry(|e| !skip(e.path(), e.file_type().is_dir()));
  for e in walk {
    let e = e?;
    let dst = to.join(e.path().strip_prefix(from).unwrap());
    let ft = e.file_type();
    if ft.is_dir() {
      fs::create_dir(&dst)?;
    } else if ft.is_symlink() {
      std::os::unix::fs::symlink(fs::read_link(e.path())?, &dst)?;
    } else if ft.is_file() {
      io::copy(
        &mut File::open(e.path())?,
        &mut open_nofollow().write(true).create_new(true).open(&dst)?,
      )?;
    }
    chown(&dst);
  }
  Ok(())
}

pub async fn copy(files: Files, Json(req): Json<CopyRequest>) -> Result<Json<FileEntry>> {
  let path = files.path(&req.path)?;
  if path == files.root() {
    return Err(Error::bad("Can't copy the root"));
  }
  blocking(move || {
    let reserved = files.reserve(size_of(&path)?)?;
    let target = copy_target(&path);
    copy_all(&path, &target, |p, dir| files.denied(p, dir))?;
    reserved.settle();
    Ok(Json(entry(&target)?))
  })
  .await
}

/// Recursive, a missing path counts as deleted.
pub async fn delete(files: Files, Json(req): Json<DeleteRequest>) -> Result<StatusCode> {
  let paths = req
    .paths
    .iter()
    .map(|p| files.path(p))
    .collect::<Result<Vec<_>>>()?;
  if paths.iter().any(|p| p == files.root()) {
    return Err(Error::bad("Can't delete the root"));
  }
  blocking(move || {
    for path in paths {
      let res = match fs::symlink_metadata(&path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(&path),
        Ok(_) => fs::remove_file(&path),
        Err(e) => Err(e),
      };
      match res {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
        _ => (),
      }
    }
    Ok(StatusCode::NO_CONTENT)
  })
  .await
}

#[derive(Deserialize)]
pub struct UsageQuery {
  /// Up to `USAGE_TTL` old plus the writes since, instead of a full walk.
  #[serde(default)]
  cached: bool,
}

pub async fn usage(
  State(agent): State<Arc<Agent>>,
  Query(q): Query<UsageQuery>,
) -> Result<Json<UsageResponse>> {
  blocking(move || {
    let bytes = match q.cached {
      true => agent.cached_usage(),
      false => agent.fresh_usage(),
    };
    Ok(Json(UsageResponse { bytes }))
  })
  .await
}

#[derive(Deserialize)]
pub struct ArchiveQuery {
  /// Newline separated gitignore patterns.
  #[serde(default)]
  ignore: String,
}

/// `ignore` plus the ignore files in `root`, and always the upload dir.
fn archive_ignore(root: &Path, ignore: &str) -> Result<Gitignore> {
  let mut builder = GitignoreBuilder::new(root);
  builder.add_line(None, &format!("/{UPLOADS}")).unwrap();
  for line in ignore.lines() {
    builder
      .add_line(None, line)
      .map_err(|e| Error::bad(e.to_string()))?;
  }
  for file in IGNORE_FILES {
    let path = root.join(file);
    if path.is_file()
      && let Some(err) = builder.add(path)
    {
      warn!("Bad pattern in {file}: {err}");
    }
  }
  builder.build().map_err(|e| Error::bad(e.to_string()))
}

/// Sync `Write` into the response body. A failed archive ends the body with an
/// error, so wings never mistakes a cut archive for a whole one.
struct ChanWriter(mpsc::Sender<io::Result<Bytes>>);

impl Write for ChanWriter {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self
      .0
      .blocking_send(Ok(Bytes::copy_from_slice(buf)))
      .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Archive reader is gone"))?;
    Ok(buf.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

/// Skips what `ignore` or `deny` match, pruned dirs are not walked.
fn write_archive(
  root: &Path,
  ignore: &Gitignore,
  deny: &Gitignore,
  out: impl Write,
) -> io::Result<()> {
  let mut tar = tar::Builder::new(zstd::Encoder::new(out, 3)?);
  tar.follow_symlinks(false);
  let walk = walkdir::WalkDir::new(root)
    .min_depth(1)
    .into_iter()
    .filter_entry(|e| {
      let dir = e.file_type().is_dir();
      !ignore.matched(e.path(), dir).is_ignore() && !deny.matched(e.path(), dir).is_ignore()
    });
  for e in walk {
    let e = match e {
      Ok(e) => e,
      // the game may delete files while we walk
      Err(err) => {
        warn!("Skipping in archive: {err}");
        continue;
      }
    };
    let rel = e.path().strip_prefix(root).unwrap();
    if let Err(err) = tar.append_path_with_name(e.path(), rel) {
      if err.kind() != io::ErrorKind::NotFound {
        return Err(err);
      }
      warn!("Skipping in archive: {err}");
    }
  }
  tar.into_inner()?.finish()?.flush()
}

/// Streams a tar.zst of the root, without the denylist.
pub async fn archive(files: Files, Query(q): Query<ArchiveQuery>) -> Result<Response> {
  let ignore = archive_ignore(files.root(), &q.ignore)?;
  let (tx, mut rx) = mpsc::channel(8);
  tokio::task::spawn_blocking(move || {
    let out = ChanWriter(tx.clone());
    if let Err(err) = write_archive(files.root(), &ignore, &files.deny, out) {
      warn!("Archive failed: {err}");
      let _ = tx.blocking_send(Err(err));
    }
  });
  let stream = futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx));
  Ok(Body::from_stream(stream).into_response())
}

#[derive(Deserialize)]
pub struct StageQuery {
  /// Name of the staging dir in `UPLOADS`.
  stage: String,
}

#[derive(Deserialize)]
pub struct CommitQuery {
  stage: String,
  #[serde(default)]
  truncate: bool,
}

impl Files {
  fn stage(&self, name: &str) -> Result<PathBuf> {
    let mut parts = Path::new(name).components();
    match (parts.next(), parts.next()) {
      (Some(Component::Normal(_)), None) => Ok(self.root().join(UPLOADS).join(name)),
      _ => Err(Error::bad("Invalid stage name")),
    }
  }
}

/// Whether the target of a link at `entry` (both relative to the root)
/// points outside the root.
fn link_escapes(entry: &Path, target: &Path) -> bool {
  let mut depth = entry.components().count().saturating_sub(1);
  for part in target.components() {
    match part {
      Component::Normal(_) => depth += 1,
      Component::CurDir => (),
      Component::ParentDir if depth > 0 => depth -= 1,
      _ => return true,
    }
  }
  false
}

/// Unpacks into `stage`, entries the denylist matches and links out of the
/// root are skipped. `base` is the usage the unpacked bytes add to.
fn extract_into(files: &Files, stage: &Path, base: u64, input: impl Read) -> Result<()> {
  let root = files.root();
  let mut written = 0u64;
  let mut archive = tar::Archive::new(zstd::Decoder::new(input)?);
  for entry in archive.entries()? {
    let mut entry = entry?;
    let kind = entry.header().entry_type();
    if !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link()) {
      continue;
    }
    // zip-slip: refuse the whole archive, tar's own check stays as backup
    let path = entry.path()?.into_owned();
    let live = jail(root, &path.to_string_lossy())?;
    if kind.is_hard_link()
      && let Some(link) = entry.link_name()?
    {
      jail(root, &link.to_string_lossy())?;
    }
    if live.starts_with(root.join(UPLOADS)) || (live != root && files.denied(&live, kind.is_dir()))
    {
      continue;
    }
    if kind.is_symlink()
      && let Some(target) = entry.link_name()?
      && link_escapes(live.strip_prefix(root).unwrap_or(&path), &target)
    {
      warn!("Skipping link {} out of the root", path.display());
      continue;
    }
    written += entry.header().size()?;
    if files.limit.is_some_and(|limit| base + written > limit) {
      return Err(Error(ErrorCode::Quota, "Disk quota exceeded".into()));
    }
    if !entry.unpack_in(stage)? {
      return Err(Error::bad(format!(
        "{} escapes the server root",
        path.display()
      )));
    }
  }
  if let Err(err) = chown_all(stage) {
    // prepare chowns again before the next start
    warn!("Failed to chown extracted files: {err}");
  }
  Ok(())
}

/// Unpacks a tar.zst body into `UPLOADS/<stage>`, the live files stay
/// untouched until `/extract/commit`. A failed extract removes the stage.
pub async fn extract(files: Files, Query(q): Query<StageQuery>, body: Body) -> Result<StatusCode> {
  let stage = files.stage(&q.stage)?;
  let reader = tokio_util::io::StreamReader::new(body.into_data_stream().map_err(io::Error::other));
  let reader = tokio_util::io::SyncIoBridge::new(reader);
  blocking(move || {
    // ponytail: the stage counts on top of the live files (a truncating restore
    // briefly needs both on disk), the quota only checks usage + archive
    let _ = fs::remove_dir_all(&stage);
    let base = files.agent.fresh_usage();
    mkdirs(&stage)?;
    let res = extract_into(&files, &stage, base, reader);
    if res.is_err() {
      let _ = fs::remove_dir_all(&stage);
    }
    res
  })
  .await?;
  Ok(StatusCode::NO_CONTENT)
}

/// Deletes everything under `dir` but `UPLOADS` and what the denylist matches.
fn prune(files: &Files, dir: &Path) -> io::Result<()> {
  for e in fs::read_dir(dir)? {
    let path = e?.path();
    let is_dir = fs::symlink_metadata(&path)?.is_dir();
    if (dir == files.root() && path.file_name() == Some(UPLOADS.as_ref()))
      || files.denied(&path, is_dir)
    {
      continue;
    }
    if is_dir {
      prune(files, &path)?;
      // kept when it holds denied files
      match fs::remove_dir(&path) {
        Err(e) if e.kind() == io::ErrorKind::DirectoryNotEmpty => (),
        res => res?,
      }
    } else {
      fs::remove_file(&path)?;
    }
  }
  Ok(())
}

/// Moves everything in `from` into `to`, replacing what is there. Dirs merge.
fn merge(from: &Path, to: &Path) -> io::Result<()> {
  for e in fs::read_dir(from)? {
    let e = e?;
    let (src, dst) = (e.path(), to.join(e.file_name()));
    let src_dir = e.file_type()?.is_dir();
    match fs::symlink_metadata(&dst) {
      Ok(m) if m.is_dir() && src_dir => {
        merge(&src, &dst)?;
        continue;
      }
      // still holds denied files after a prune
      Ok(m) if m.is_dir() => {
        warn!("Not replacing directory {} with a file", dst.display());
        continue;
      }
      // a dir replaces a file or link of its name
      Ok(_) if src_dir => fs::remove_file(&dst)?,
      _ => (),
    }
    fs::rename(&src, &dst)?;
  }
  Ok(())
}

/// Swaps a checked stage in: with `truncate` the root is cleared first
/// (denied files and `UPLOADS` stay), else the stage is merged over it.
pub async fn commit(files: Files, Query(q): Query<CommitQuery>) -> Result<StatusCode> {
  let stage = files.stage(&q.stage)?;
  blocking(move || {
    if !stage.is_dir() {
      return Err(Error(ErrorCode::NotFound, "Stage not found".into()));
    }
    if q.truncate {
      prune(&files, files.root())?;
    }
    merge(&stage, files.root())?;
    fs::remove_dir_all(&stage)?;
    files.agent.fresh_usage();
    Ok(StatusCode::NO_CONTENT)
  })
  .await
}

#[cfg(test)]
mod test {
  use super::*;

  fn tmp() -> PathBuf {
    let root = std::env::temp_dir().join(format!("smaug-files-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    root
  }

  #[test]
  fn jail_rejects_escapes() {
    let root = tmp();
    std::os::unix::fs::symlink("/etc", root.join("link")).unwrap();
    assert_eq!(jail(&root, "../x").unwrap_err().0, ErrorCode::BadRequest);
    assert_eq!(
      jail(&root, "a/../../x").unwrap_err().0,
      ErrorCode::BadRequest
    );
    assert_eq!(
      jail(&root, "link/passwd").unwrap_err().0,
      ErrorCode::BadRequest
    );
    // absolute paths are relative to the root
    assert_eq!(jail(&root, "/etc/passwd").unwrap(), root.join("etc/passwd"));
    // the link itself may be named, it is never followed
    assert_eq!(jail(&root, "link").unwrap(), root.join("link"));
    assert!(open_nofollow().read(true).open(root.join("link")).is_err());
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn copy_names() {
    let root = tmp();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join(".env"), "").unwrap();
    fs::create_dir(root.join("dir.d")).unwrap();
    assert_eq!(copy_target(&root.join("a.txt")), root.join("a copy.txt"));
    fs::write(root.join("a copy.txt"), "").unwrap();
    assert_eq!(copy_target(&root.join("a.txt")), root.join("a copy 2.txt"));
    assert_eq!(copy_target(&root.join(".env")), root.join(".env copy"));
    assert_eq!(copy_target(&root.join("dir.d")), root.join("dir.d copy"));
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn ranges() {
    assert_eq!(parse_range("bytes=0-9", 100), Some((0, 9)));
    assert_eq!(parse_range("bytes=90-", 100), Some((90, 99)));
    assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
    assert_eq!(parse_range("bytes=50-500", 100), Some((50, 99)));
    assert_eq!(parse_range("bytes=9-0", 100), None);
    assert_eq!(parse_range("bytes=0-0", 0), None);
  }

  #[test]
  fn links_out_of_the_root() {
    let esc = |entry: &str, target: &str| link_escapes(Path::new(entry), Path::new(target));
    assert!(!esc("a/link", "b"));
    assert!(!esc("a/link", "../b"));
    assert!(!esc("a/b/link", "./../../c"));
    assert!(esc("link", "../x"));
    assert!(esc("a/link", "../../x"));
    assert!(esc("a/link", "/etc/passwd"));
    // back down after leaving the root still escaped on the way
    assert!(esc("link", "../root/x"));
  }

  #[test]
  fn reservations_hold_the_quota() {
    let root = tmp();
    let files = Files {
      agent: Arc::new(Agent {
        token: String::new(),
        root: root.clone(),
        prepare: Default::default(),
        usage: Default::default(),
      }),
      deny: Gitignore::empty(),
      limit: Some(10),
    };
    let a = files.reserve(6).unwrap();
    // a parallel write can't take what `a` holds
    assert_eq!(files.reserve(6).err().unwrap().0, ErrorCode::Quota);
    drop(a);
    files.reserve(6).unwrap().settle();
    // settled bytes count until the next walk
    assert_eq!(files.reserve(6).err().unwrap().0, ErrorCode::Quota);
    files.reserve(4).unwrap();
    fs::remove_dir_all(root).unwrap();
  }
}
