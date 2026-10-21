//! Chunked streams, `docs/API.md` §3.4. The op id of a stream is its stream id.

use std::{
  collections::{BTreeMap, BTreeSet},
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use axum::{
  Extension, Json, Router,
  body::Bytes,
  extract::{Path, Query},
  middleware::from_fn,
  response::{IntoResponse, Response},
  routing::{get, post, put},
};
use dashmap::DashMap;
use futures::StreamExt;
use http::{Method, StatusCode};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use shared::msg::{
  CompleteStream, ErrorCode, Event, OpResult, OpStarted, OpenInStream, OpenOutStream, STREAM_END,
  STREAM_SHA256, STREAM_SIZE, ServerSpec, ServerState, StreamOpened, StreamSource, StreamStatus,
  StreamTarget,
};
use tokio::{
  sync::{Notify, mpsc, oneshot},
  task::AbortHandle,
  time::Instant,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
  agent, auth,
  config::Config,
  docker::Agent,
  error::{ApiError, ApiResult},
  files::agent_for,
  logs::Logs,
  servers::{Runtime, busy},
  store::Store,
};

const IDLE: Duration = Duration::from_secs(10 * 60);
/// Archive and extract requests run as long as the stream does.
const STREAMING: Duration = Duration::from_secs(24 * 60 * 60);
/// How long a GET waits for its chunk to be generated.
const CHUNK_WAIT: Duration = Duration::from_secs(60);
/// How long a restore waits for the server to stop.
const STOP_WAIT: Duration = Duration::from_secs(10 * 60);
const UPLOADS: &str = ".smaug-uploads";
/// Archives one server streams at once.
const MAX_OUT: usize = 2;

#[derive(Clone)]
pub struct Streams(Arc<Inner>);

struct Inner {
  map: DashMap<Uuid, Arc<Stream>>,
  store: Store,
  chunk_size: u64,
  window: u32,
}

struct Stream {
  server: Uuid,
  last: Mutex<Instant>,
  /// `complete` started: no more chunks, no abort, the finisher owns it.
  completing: AtomicBool,
  kind: Kind,
}

enum Kind {
  Out(Out),
  File(Box<FileIn>),
  Restore(Box<RestoreIn>),
}

struct Out {
  window: Mutex<OutWindow>,
  /// Woken on every new chunk and every ack.
  changed: Notify,
  producer: Mutex<Option<AbortHandle>>,
}

struct FileIn {
  agent: Agent,
  spec: ServerSpec,
  path: String,
  size: u64,
  count: u32,
  received: Mutex<BTreeSet<u32>>,
}

struct RestoreIn {
  agent: Agent,
  spec: ServerSpec,
  /// Locked while chunks are fed, so they reach the extractor in order.
  reorder: tokio::sync::Mutex<Reorder>,
  tx: tokio::sync::Mutex<Option<mpsc::Sender<Bytes>>>,
  checked: Mutex<Option<oneshot::Sender<Result<(), String>>>>,
  size: Option<u64>,
  task: Mutex<Option<AbortHandle>>,
}

impl Stream {
  fn touch(&self) {
    *self.last.lock().unwrap() = Instant::now();
  }

  fn completing(&self) -> bool {
    self.completing.load(Ordering::SeqCst)
  }
}

fn completing() -> ApiError {
  ApiError::new(ErrorCode::Conflict, "Stream is completing")
}

pub fn router() -> Router {
  Router::new()
    .route("/servers/{uuid}/streams/out", post(open_out))
    .route(
      "/servers/{uuid}/streams/out/{stream}/{index}",
      get(get_chunk),
    )
    .route("/servers/{uuid}/streams/in", post(open_in))
    .route(
      "/servers/{uuid}/streams/in/{stream}/{index}",
      put(put_chunk),
    )
    .route(
      "/servers/{uuid}/streams/in/{stream}/complete",
      post(complete),
    )
    .route(
      "/servers/{uuid}/streams/{stream}",
      get(status).delete(abort),
    )
    .layer(from_fn(auth::verify))
}

impl Streams {
  pub fn new(store: Store, config: &Config) -> Self {
    let streams = Self(Arc::new(Inner {
      map: DashMap::new(),
      store,
      chunk_size: config.chunk_size,
      window: config.stream_window,
    }));
    tokio::spawn(streams.clone().reap());
    streams
  }

  /// Out-streams of `server`.
  fn outs(&self, server: Uuid) -> usize {
    self
      .0
      .map
      .iter()
      .filter(|e| e.server == server && matches!(e.kind, Kind::Out(_)))
      .count()
  }

  fn opened(&self, stream: Uuid) -> Json<StreamOpened> {
    Json(StreamOpened {
      stream,
      chunk_size: self.0.chunk_size,
      window: self.0.window,
    })
  }

  fn insert(&self, server: Uuid, kind: Kind) -> (Uuid, Arc<Stream>) {
    let id = Uuid::new_v4();
    let stream = Arc::new(Stream {
      server,
      last: Mutex::new(Instant::now()),
      completing: AtomicBool::new(false),
      kind,
    });
    self.0.map.insert(id, stream.clone());
    (id, stream)
  }

  fn get(&self, server: Uuid, id: Uuid) -> ApiResult<Arc<Stream>> {
    let stream = self
      .0
      .map
      .get(&id)
      .map(|s| s.clone())
      .filter(|s| s.server == server)
      .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "Stream not found"))?;
    stream.touch();
    Ok(stream)
  }

  /// Ends the stream, reported to the panel as its op.
  fn finish(&self, server: Uuid, id: Uuid, res: Result<Option<OpResult>, String>) {
    if self.0.map.remove(&id).is_none() {
      return;
    }
    let (ok, error, result) = match res {
      Ok(result) => (true, None, result),
      Err(error) => (false, Some(error), None),
    };
    self.0.store.send(Event::OpFinished {
      op: id,
      uuid: server,
      ok,
      error,
      result,
    });
  }

  /// Aborts every stream idle for longer than `IDLE`.
  async fn reap(self) {
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    loop {
      tick.tick().await;
      let idle: Vec<(Uuid, Arc<Stream>)> = self
        .0
        .map
        .iter()
        .filter(|e| !e.completing() && e.last.lock().unwrap().elapsed() > IDLE)
        .map(|e| (*e.key(), e.value().clone()))
        .collect();
      for (id, stream) in idle {
        info!("Stream {id} is idle, aborting");
        self.cancel(id, &stream, "Stream idle for 10 min").await;
      }
    }
  }

  async fn cancel(&self, id: Uuid, stream: &Stream, why: &str) {
    match &stream.kind {
      Kind::Out(out) => {
        if let Some(producer) = out.producer.lock().unwrap().take() {
          producer.abort();
        }
      }
      Kind::File(file) => remove_upload(&file.agent, &file.spec, id).await,
      Kind::Restore(restore) => {
        if let Some(task) = restore.task.lock().unwrap().take() {
          // dropping the task drops the op lock
          task.abort();
        }
        remove_stage(&restore.agent, &restore.spec, id).await;
        if let Err(err) = self
          .0
          .store
          .set_state(stream.server, ServerState::Offline)
          .await
        {
          warn!("Failed to mark server {} offline: {err:?}", stream.server);
        }
      }
    }
    self.finish(stream.server, id, Err(why.into()));
  }
}

// ---- out-streams ----

/// The chunks of an out-stream in `[next, next + window)`.
#[derive(Debug)]
struct OutWindow {
  next: u32,
  window: u32,
  chunks: BTreeMap<u32, Bytes>,
  /// Chunks generated so far.
  produced: u32,
  /// `(sha256, size)` once the generator finished.
  end: Option<(String, u64)>,
}

#[derive(Debug, PartialEq)]
enum OutGet {
  /// The chunk, with `(sha256, size)` when it is the final one.
  Chunk(Bytes, Option<(String, u64)>),
  /// In the window, not generated yet.
  Pending,
  /// Every chunk was acked.
  Done,
}

impl OutWindow {
  fn new(window: u32) -> Self {
    Self {
      next: 0,
      window,
      chunks: BTreeMap::new(),
      produced: 0,
      end: None,
    }
  }

  /// Every chunk below `n` is done. Never moves past what was generated.
  fn ack(&mut self, n: u32) {
    let n = n.min(self.produced);
    if n > self.next {
      self.next = n;
      self.chunks = self.chunks.split_off(&n);
    }
  }

  fn done(&self) -> bool {
    self.end.is_some() && self.next >= self.produced
  }

  fn get(&self, index: u32) -> ApiResult<OutGet> {
    if self.done() {
      return Ok(OutGet::Done);
    }
    if index < self.next {
      return Err(ApiError::gone("Chunk already acked"));
    }
    if index >= self.next + self.window {
      return Err(ApiError::new(
        ErrorCode::Conflict,
        "Chunk outside the window",
      ));
    }
    Ok(match self.chunks.get(&index) {
      Some(chunk) => {
        let last = self.end.is_some() && index + 1 == self.produced;
        OutGet::Chunk(chunk.clone(), self.end.clone().filter(|_| last))
      }
      None if self.end.is_some() => {
        return Err(ApiError::new(ErrorCode::NotFound, "Past the final chunk"));
      }
      None => OutGet::Pending,
    })
  }

  fn room(&self) -> bool {
    self.produced < self.next + self.window
  }

  fn push(&mut self, chunk: Bytes) {
    self.chunks.insert(self.produced, chunk);
    self.produced += 1;
  }
}

async fn open_out(
  Extension(streams): Extension<Streams>,
  store: Store,
  docker: Runtime,
  Path(uuid): Path<Uuid>,
  Json(req): Json<OpenOutStream>,
) -> ApiResult<Json<StreamOpened>> {
  let (agent, spec) = agent_for(&store, &docker, uuid).await?;
  let StreamSource::Backup { ignore } = req.source;
  // inserted first and checked after, so racing opens never pass the cap
  // and a restore taking the lock meanwhile sees this stream
  let (id, stream) = streams.insert(
    uuid,
    Kind::Out(Out {
      window: Mutex::new(OutWindow::new(streams.0.window)),
      changed: Notify::new(),
      producer: Mutex::new(None),
    }),
  );
  let body = async {
    // a restore swaps the files under the archive
    if store.locked(uuid) || streams.outs(uuid) > MAX_OUT {
      return Err(busy());
    }
    let res = agent::files(&agent, &spec, Method::GET, "/archive")
      .query(&[("ignore", ignore.join("\n"))])
      .timeout(STREAMING)
      .send()
      .await?;
    Ok(agent::ok(res).await?.bytes_stream())
  };
  let body = match body.await {
    Ok(body) => body,
    Err(err) => {
      // never reported as an op, the panel only gets the error
      streams.0.map.remove(&id);
      return Err(err);
    }
  };
  let task = tokio::spawn(produce(streams.clone(), id, stream.clone(), body));
  if let Kind::Out(out) = &stream.kind {
    *out.producer.lock().unwrap() = Some(task.abort_handle());
  }
  Ok(streams.opened(id))
}

/// Waits until `ready` holds, rechecked on every change of the out-stream.
async fn wait_for(out: &Out, ready: impl Fn(&OutWindow) -> bool) {
  loop {
    let changed = out.changed.notified();
    tokio::pin!(changed);
    changed.as_mut().enable();
    if ready(&out.window.lock().unwrap()) {
      return;
    }
    changed.await;
  }
}

/// Cuts the archive into chunks, never more than the window holds.
async fn produce(
  streams: Streams,
  id: Uuid,
  stream: Arc<Stream>,
  mut body: impl futures::Stream<Item = reqwest::Result<Bytes>> + Unpin,
) {
  let Kind::Out(out) = &stream.kind else {
    return;
  };
  let chunk_size = streams.0.chunk_size as usize;
  let push = |chunk: Vec<u8>| {
    out.window.lock().unwrap().push(chunk.into());
    out.changed.notify_waiters();
  };
  let mut hasher = Sha256::new();
  let mut size = 0u64;
  let mut buf: Vec<u8> = Vec::new();
  while let Some(piece) = body.next().await {
    let piece = match piece {
      Ok(piece) => piece,
      Err(err) => {
        warn!("Archive of stream {id} broke: {err}");
        streams.finish(stream.server, id, Err(format!("Archive failed: {err}")));
        out.changed.notify_waiters();
        return;
      }
    };
    hasher.update(&piece);
    size += piece.len() as u64;
    buf.extend_from_slice(&piece);
    // one byte kept back, so the final chunk is known when the archive ends
    while buf.len() > chunk_size {
      let rest = buf.split_off(chunk_size);
      wait_for(out, OutWindow::room).await;
      push(std::mem::replace(&mut buf, rest));
    }
  }
  wait_for(out, OutWindow::room).await;
  {
    let mut window = out.window.lock().unwrap();
    window.push(buf.into());
    window.end = Some((hex::encode(hasher.finalize()), size));
  }
  out.changed.notify_waiters();
}

#[derive(Deserialize)]
struct AckQuery {
  ack: Option<u32>,
}

async fn get_chunk(
  Extension(streams): Extension<Streams>,
  Path((uuid, id, index)): Path<(Uuid, Uuid, u32)>,
  Query(query): Query<AckQuery>,
) -> ApiResult<Response> {
  let stream = streams.get(uuid, id)?;
  let Kind::Out(out) = &stream.kind else {
    return Err(ApiError::new(ErrorCode::BadRequest, "Not an out-stream"));
  };
  if let Some(n) = query.ack {
    out.window.lock().unwrap().ack(n);
    out.changed.notify_waiters();
  }
  let wait = tokio::time::timeout(CHUNK_WAIT, async {
    loop {
      let changed = out.changed.notified();
      tokio::pin!(changed);
      changed.as_mut().enable();
      let got = out.window.lock().unwrap().get(index);
      match got {
        Ok(OutGet::Pending) if streams.0.map.contains_key(&id) => changed.await,
        // the generator failed meanwhile
        Ok(OutGet::Pending) => {
          return Err(ApiError::internal("Archive failed"));
        }
        res => return res,
      }
    }
  });
  let got = wait
    .await
    .map_err(|_| ApiError::internal("Chunk not generated in time"))??;
  Ok(match got {
    OutGet::Chunk(chunk, None) => chunk.into_response(),
    OutGet::Chunk(chunk, Some((sha256, size))) => (
      [
        (STREAM_END, "true".to_string()),
        (STREAM_SHA256, sha256),
        (STREAM_SIZE, size.to_string()),
      ],
      chunk,
    )
      .into_response(),
    OutGet::Done => {
      let end = out.window.lock().unwrap().end.clone();
      let result = end.map(|(sha256, size)| OpResult::Stream { sha256, size });
      streams.finish(uuid, id, Ok(result));
      StatusCode::NO_CONTENT.into_response()
    }
    OutGet::Pending => unreachable!("waited for"),
  })
}

// ---- in-streams ----

async fn open_in(
  Extension(streams): Extension<Streams>,
  store: Store,
  docker: Runtime,
  Path(uuid): Path<Uuid>,
  Json(req): Json<OpenInStream>,
) -> ApiResult<Json<StreamOpened>> {
  let (agent, spec) = agent_for(&store, &docker, uuid).await?;
  let limit = agent::disk_limit(&spec);
  if limit > 0
    && let Some(size) = req.size
    && agent::usage(&agent, &spec, true).await? + size > limit
  {
    return Err(ApiError::new(ErrorCode::Quota, "Over the disk limit"));
  }
  match req.target {
    StreamTarget::File { path } => {
      let size = req
        .size
        .ok_or_else(|| ApiError::new(ErrorCode::BadRequest, "File streams need a size"))?;
      let count = size.div_ceil(streams.0.chunk_size);
      let count =
        u32::try_from(count).map_err(|_| ApiError::new(ErrorCode::TooLarge, "Too many chunks"))?;
      // may exist already
      let _ = agent::files(&agent, &spec, Method::POST, "/files/mkdir")
        .json(&shared::msg::MkdirRequest {
          path: UPLOADS.into(),
        })
        .send()
        .await;
      let (id, _) = streams.insert(
        uuid,
        Kind::File(Box::new(FileIn {
          agent,
          spec,
          path,
          size,
          count,
          received: Mutex::new(BTreeSet::new()),
        })),
      );
      Ok(streams.opened(id))
    }
    StreamTarget::Restore { truncate } => {
      let observed = store.observed(uuid).await?.unwrap_or_default();
      if observed.state == ServerState::Installing {
        return Err(busy());
      }
      let guard = store.lock(uuid).await.ok_or_else(busy)?;
      // a backup running would archive a half swapped root
      if streams.outs(uuid) > 0 {
        return Err(busy());
      }
      let (tx, rx) = mpsc::channel::<Bytes>(streams.0.window as usize);
      let (checked_tx, checked_rx) = oneshot::channel();
      let (id, stream) = streams.insert(
        uuid,
        Kind::Restore(Box::new(RestoreIn {
          agent: agent.clone(),
          spec: spec.clone(),
          reorder: tokio::sync::Mutex::new(Reorder::new(streams.0.window)),
          tx: tokio::sync::Mutex::new(Some(tx)),
          checked: Mutex::new(Some(checked_tx)),
          size: req.size,
          task: Mutex::new(None),
        })),
      );
      let docker = docker.0.clone().expect("agent_for checked the runtime");
      let restore = {
        let streams = streams.clone();
        async move {
          let res = restore(&store, &docker, &agent, &spec, id, truncate, rx, checked_rx).await;
          if let Err(err) = &res {
            warn!("Restore of server {uuid} failed: {err}");
            remove_stage(&agent, &spec, id).await;
          }
          if let Err(err) = store.set_state(uuid, ServerState::Offline).await {
            warn!("Failed to mark server {uuid} offline: {err:?}");
          }
          streams.finish(uuid, id, res.map(|()| None));
          drop(guard);
        }
      };
      let task = tokio::spawn(restore);
      if let Kind::Restore(r) = &stream.kind {
        *r.task.lock().unwrap() = Some(task.abort_handle());
      }
      Ok(streams.opened(id))
    }
  }
}

/// Stops the server, feeds the chunks to the agent's extractor (into a
/// stage), and swaps the stage in once `complete` checked the archive.
#[allow(clippy::too_many_arguments)]
async fn restore(
  store: &Store,
  docker: &crate::docker::Docker,
  agent: &Agent,
  spec: &ServerSpec,
  id: Uuid,
  truncate: bool,
  rx: mpsc::Receiver<Bytes>,
  checked: oneshot::Receiver<Result<(), String>>,
) -> Result<(), String> {
  let uuid = spec.uuid;
  let logs = Logs::spawn(store.clone(), uuid);
  // the reconciler stops it, holding the lock keeps it stopped
  let stopped = async {
    loop {
      let state = store
        .observed(uuid)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
        .state;
      let settled = matches!(state, ServerState::Offline | ServerState::Crashed);
      if settled && matches!(docker.inspect(uuid).await, Ok(None)) {
        return;
      }
      tokio::time::sleep(Duration::from_millis(500)).await;
    }
  };
  tokio::time::timeout(STOP_WAIT, stopped)
    .await
    .map_err(|_| "Server did not stop".to_string())?;
  store
    .set_state(uuid, ServerState::Restoring)
    .await
    .map_err(|e| format!("{e:#}"))?;
  logs
    .daemon("Restoring backup, the server stays offline meanwhile...")
    .await;

  let body = futures::stream::unfold(rx, |mut rx| async move {
    rx.recv().await.map(|b| (Ok::<_, std::io::Error>(b), rx))
  });
  let res = agent::files(agent, spec, Method::PUT, "/extract")
    .query(&[("stage", stage_name(id))])
    .timeout(STREAMING)
    .body(reqwest::Body::wrap_stream(body))
    .send()
    .await
    .map_err(|e| format!("Failed to reach agent: {e}"))?;
  agent::ok(res).await.map_err(|e| e.to_string())?;
  // the extractor only ends once `complete` closed the body
  checked.await.map_err(|_| "Restore aborted".to_string())??;
  let res = agent::files(agent, spec, Method::POST, "/extract/commit")
    .query(&[("stage", stage_name(id))])
    .query(&[("truncate", truncate)])
    .timeout(STREAMING)
    .send()
    .await
    .map_err(|e| format!("Failed to reach agent: {e}"))?;
  agent::ok(res).await.map_err(|e| e.to_string())?;
  logs.daemon("Finished restoring backup").await;
  Ok(())
}

/// Orders the chunks of a `Restore` stream: only `[next, next + window)` is accepted.
struct Reorder {
  next: u32,
  window: u32,
  pending: BTreeMap<u32, Bytes>,
  hasher: Sha256,
  size: u64,
}

impl Reorder {
  fn new(window: u32) -> Self {
    Self {
      next: 0,
      window,
      pending: BTreeMap::new(),
      hasher: Sha256::new(),
      size: 0,
    }
  }

  /// The chunks now ready in order. A chunk below `next` is a retry, a no-op.
  fn put(&mut self, index: u32, chunk: Bytes) -> ApiResult<Vec<Bytes>> {
    if index < self.next {
      return Ok(Vec::new());
    }
    if index >= self.next + self.window {
      return Err(ApiError::new(
        ErrorCode::Conflict,
        "Chunk outside the window",
      ));
    }
    self.pending.insert(index, chunk);
    let mut ready = Vec::new();
    while let Some(chunk) = self.pending.remove(&self.next) {
      self.hasher.update(&chunk);
      self.size += chunk.len() as u64;
      self.next += 1;
      ready.push(chunk);
    }
    Ok(ready)
  }
}

async fn put_chunk(
  Extension(streams): Extension<Streams>,
  Path((uuid, id, index)): Path<(Uuid, Uuid, u32)>,
  body: Bytes,
) -> ApiResult<StatusCode> {
  let stream = streams.get(uuid, id)?;
  let chunk_size = streams.0.chunk_size;
  if stream.completing() {
    return Err(completing());
  }
  match &stream.kind {
    Kind::Out(_) => return Err(ApiError::new(ErrorCode::BadRequest, "Not an in-stream")),
    Kind::File(file) => {
      if index >= file.count {
        return Err(ApiError::new(ErrorCode::BadRequest, "Past the last chunk"));
      }
      let offset = index as u64 * chunk_size;
      let expected = (file.size - offset).min(chunk_size);
      if body.len() as u64 != expected {
        return Err(ApiError::new(
          ErrorCode::BadRequest,
          format!("Chunk {index} must be {expected} bytes"),
        ));
      }
      let res = agent::files(&file.agent, &file.spec, Method::PUT, "/files/write-at")
        .query(&[("path", upload_path(id)), ("offset", offset.to_string())])
        .body(body)
        .send()
        .await?;
      agent::ok(res).await?;
      file.received.lock().unwrap().insert(index);
    }
    Kind::Restore(restore) => {
      if body.len() as u64 > chunk_size {
        return Err(ApiError::new(ErrorCode::TooLarge, "Larger than one chunk"));
      }
      let mut reorder = restore.reorder.lock().await;
      // complete sets it under this lock
      if stream.completing() {
        return Err(completing());
      }
      let ready = reorder.put(index, body)?;
      let tx = restore.tx.lock().await;
      for chunk in ready {
        let sent = match tx.as_ref() {
          Some(tx) => tx.send(chunk).await.is_ok(),
          None => false,
        };
        if !sent {
          return Err(ApiError::internal("Restore failed"));
        }
      }
    }
  }
  Ok(StatusCode::NO_CONTENT)
}

fn upload_path(id: Uuid) -> String {
  format!("{UPLOADS}/{id}")
}

fn stage_name(id: Uuid) -> String {
  format!("restore-{id}")
}

/// The agent also removes a stage whose extract failed, this covers the rest.
async fn remove_stage(agent: &Agent, spec: &ServerSpec, id: Uuid) {
  let res = agent::files(agent, spec, Method::POST, "/files/delete")
    .json(&shared::msg::DeleteRequest {
      paths: vec![format!("{UPLOADS}/{}", stage_name(id))],
    })
    .send()
    .await;
  if let Err(err) = res {
    warn!("Failed to remove restore stage {id}: {err}");
  }
}

async fn remove_upload(agent: &Agent, spec: &ServerSpec, id: Uuid) {
  let res = agent::files(agent, spec, Method::POST, "/files/delete")
    .json(&shared::msg::DeleteRequest {
      paths: vec![upload_path(id)],
    })
    .send()
    .await;
  if let Err(err) = res {
    warn!("Failed to remove upload {id}: {err}");
  }
}

async fn complete(
  Extension(streams): Extension<Streams>,
  Path((uuid, id)): Path<(Uuid, Uuid)>,
  Json(req): Json<CompleteStream>,
) -> ApiResult<(StatusCode, Json<OpStarted>)> {
  let stream = streams.get(uuid, id)?;
  let missing = || ApiError::new(ErrorCode::Conflict, "Chunks are missing");
  match &stream.kind {
    Kind::Out(_) => return Err(ApiError::new(ErrorCode::BadRequest, "Not an in-stream")),
    Kind::File(file) => {
      if file.received.lock().unwrap().len() as u32 != file.count {
        return Err(missing());
      }
      if stream.completing.swap(true, Ordering::SeqCst) {
        return Err(completing());
      }
      let streams = streams.clone();
      let stream = stream.clone();
      tokio::spawn(async move {
        let Kind::File(file) = &stream.kind else {
          return;
        };
        let res = finish_file(file, id, streams.0.chunk_size, &req.sha256).await;
        if res.is_err() {
          remove_upload(&file.agent, &file.spec, id).await;
        }
        streams.finish(uuid, id, res.map(|()| None));
      });
    }
    Kind::Restore(restore) => {
      let reorder = restore.reorder.lock().await;
      if !reorder.pending.is_empty() || restore.size.is_some_and(|s| s != reorder.size) {
        return Err(missing());
      }
      if stream.completing.swap(true, Ordering::SeqCst) {
        return Err(completing());
      }
      let sha256 = hex::encode(reorder.hasher.clone().finalize());
      let check = match sha256.eq_ignore_ascii_case(&req.sha256) {
        true => Ok(()),
        false => Err(format!("Checksum mismatch, got {sha256}")),
      };
      if let Some(checked) = restore.checked.lock().unwrap().take() {
        let _ = checked.send(check);
      }
      // closing the body lets the extractor finish
      restore.tx.lock().await.take();
    }
  }
  Ok((StatusCode::ACCEPTED, Json(OpStarted { op: id })))
}

/// Checks the uploaded file, then moves it onto its path.
async fn finish_file(file: &FileIn, id: Uuid, chunk_size: u64, sha256: &str) -> Result<(), String> {
  let tmp = upload_path(id);
  let failed = |e: ApiError| e.to_string();
  if file.count == 0 {
    let res = agent::files(&file.agent, &file.spec, Method::PUT, "/files/write-at")
      .query(&[("path", tmp.clone()), ("offset", "0".into())])
      .send()
      .await
      .map_err(|e| failed(e.into()))?;
    agent::ok(res).await.map_err(failed)?;
  }
  let mut hasher = Sha256::new();
  let mut size = 0;
  for index in 0..file.count as u64 {
    let start = index * chunk_size;
    let end = (start + chunk_size).min(file.size) - 1;
    let res = agent::files(&file.agent, &file.spec, Method::GET, "/files/content")
      .query(&[("path", tmp.as_str())])
      .header(http::header::RANGE, format!("bytes={start}-{end}"))
      .send()
      .await
      .map_err(|e| failed(e.into()))?;
    let piece = agent::ok(res)
      .await
      .map_err(failed)?
      .bytes()
      .await
      .map_err(|e| failed(e.into()))?;
    size += piece.len() as u64;
    hasher.update(&piece);
  }
  let actual = hex::encode(hasher.finalize());
  if size != file.size || !actual.eq_ignore_ascii_case(sha256) {
    return Err(format!(
      "Checksum or size mismatch, got {actual} with {size} bytes"
    ));
  }
  let res = agent::files(&file.agent, &file.spec, Method::POST, "/files/rename")
    .query(&[("overwrite", true)])
    .json(&shared::msg::RenameRequest {
      moves: vec![shared::msg::Move {
        from: tmp,
        to: file.path.clone(),
      }],
    })
    .send()
    .await
    .map_err(|e| failed(e.into()))?;
  agent::ok(res).await.map_err(failed)?;
  Ok(())
}

async fn status(
  Extension(streams): Extension<Streams>,
  Path((uuid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<StreamStatus>> {
  let stream = streams.get(uuid, id)?;
  Ok(Json(match &stream.kind {
    Kind::Out(out) => {
      let window = out.window.lock().unwrap();
      StreamStatus {
        next: window.next,
        received: window.chunks.keys().copied().collect(),
      }
    }
    Kind::File(file) => {
      let received = file.received.lock().unwrap();
      let next = (0..).find(|i| !received.contains(i)).unwrap_or(0);
      StreamStatus {
        next,
        received: received.range(next..).copied().collect(),
      }
    }
    Kind::Restore(restore) => {
      let reorder = restore.reorder.lock().await;
      StreamStatus {
        next: reorder.next,
        received: reorder.pending.keys().copied().collect(),
      }
    }
  }))
}

async fn abort(
  Extension(streams): Extension<Streams>,
  Path((uuid, id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
  let stream = streams.get(uuid, id)?;
  if stream.completing() {
    return Err(completing());
  }
  streams.cancel(id, &stream, "Aborted").await;
  Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod test {
  use super::*;

  fn code(res: ApiResult<OutGet>) -> (ErrorCode, StatusCode) {
    let err = res.unwrap_err();
    (err.body.code, err.status)
  }

  #[test]
  fn out_window_moves_with_acks() {
    let mut w = OutWindow::new(2);
    assert_eq!(w.get(0).unwrap(), OutGet::Pending);
    assert_eq!(code(w.get(2)).0, ErrorCode::Conflict);
    w.push("a".into());
    w.push("b".into());
    assert!(!w.room());
    assert_eq!(w.get(1).unwrap(), OutGet::Chunk("b".into(), None));
    // acks are cumulative, never past what was generated
    w.ack(1);
    w.ack(0);
    assert_eq!(w.next, 1);
    assert_eq!(code(w.get(0)).1, StatusCode::GONE);
    assert!(w.room());
    w.ack(9);
    assert_eq!(w.next, 2);
    w.push("c".into());
    w.end = Some(("sha".into(), 3));
    // the final chunk carries the checksum
    assert_eq!(
      w.get(2).unwrap(),
      OutGet::Chunk("c".into(), Some(("sha".into(), 3)))
    );
    assert_eq!(code(w.get(3)).0, ErrorCode::NotFound);
    assert!(!w.done());
    // an ack past the final chunk ends it
    w.ack(3);
    assert!(w.done());
    assert_eq!(w.get(3).unwrap(), OutGet::Done);
  }

  #[test]
  fn restore_chunks_are_reordered() {
    let mut r = Reorder::new(3);
    assert_eq!(r.put(1, "b".into()).unwrap(), Vec::<Bytes>::new());
    assert_eq!(r.put(2, "c".into()).unwrap(), Vec::<Bytes>::new());
    assert_eq!(
      r.put(3, "d".into()).unwrap_err().body.code,
      ErrorCode::Conflict
    );
    assert_eq!(
      r.put(0, "a".into()).unwrap(),
      vec![Bytes::from("a"), "b".into(), "c".into()]
    );
    // a retry below next is a no-op
    assert_eq!(r.put(1, "b".into()).unwrap(), Vec::<Bytes>::new());
    assert_eq!(r.put(3, "d".into()).unwrap(), vec![Bytes::from("d")]);
    assert_eq!(r.size, 4);
    assert_eq!(
      hex::encode(r.hasher.finalize()),
      hex::encode(Sha256::digest(b"abcd"))
    );
  }

  #[tokio::test]
  async fn completing_streams_take_no_more_chunks() {
    let streams = Streams(Arc::new(Inner {
      map: DashMap::new(),
      store: crate::store::test::memory(),
      chunk_size: 4,
      window: 2,
    }));
    let uuid = Uuid::new_v4();
    let (tx, _rx) = mpsc::channel(2);
    let (checked, _checked) = oneshot::channel();
    let (id, _) = streams.insert(
      uuid,
      Kind::Restore(Box::new(RestoreIn {
        agent: Agent {
          url: String::new(),
          token: String::new(),
        },
        spec: crate::store::test::spec(uuid),
        reorder: tokio::sync::Mutex::new(Reorder::new(2)),
        tx: tokio::sync::Mutex::new(Some(tx)),
        checked: Mutex::new(Some(checked)),
        size: None,
        task: Mutex::new(None),
      })),
    );
    let put = |index| {
      put_chunk(
        Extension(streams.clone()),
        Path((uuid, id, index)),
        "ab".into(),
      )
    };
    let done = || {
      let req = CompleteStream {
        sha256: hex::encode(Sha256::digest(b"ab")),
      };
      complete(Extension(streams.clone()), Path((uuid, id)), Json(req))
    };
    put(0).await.unwrap();
    let _ = done().await.unwrap();
    assert_eq!(put(1).await.unwrap_err().status, StatusCode::CONFLICT);
    assert_eq!(done().await.unwrap_err().status, StatusCode::CONFLICT);
    let aborted = abort(Extension(streams.clone()), Path((uuid, id))).await;
    assert_eq!(aborted.unwrap_err().status, StatusCode::CONFLICT);
  }
}
