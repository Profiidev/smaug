//! Moves Docker containers toward the desired state, one task per server so
//! each server's steps never overlap. Behaviour visible to eggs follows
//! Pterodactyl Wings, see `docs/API.md` §6.2.

use std::{
  sync::{Arc, LazyLock},
  time::Duration,
};

use centaurus::error::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures::StreamExt;
use regex::Regex;
use shared::msg::{InstallResult, Observed, ServerState, StopAction};
use tokio::{
  io::AsyncWriteExt,
  sync::{Notify, broadcast::error::RecvError},
  time::Instant,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
  agent,
  docker::{self, Container, Docker, Output, Stdin},
  store::{Desired, DesiredPower, Store},
};

const RESYNC: Duration = Duration::from_secs(60);
const RETRY: Duration = Duration::from_secs(10);
/// Longest wait between retries of a failing step, the wait doubles up to it.
const MAX_RETRY: Duration = Duration::from_secs(300);
/// An install script running longer is killed and the install fails.
/// ponytail: fixed for every egg, make it configurable if real installs need longer
const INSTALL_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
/// Wings stops a server for a reinstall with this timeout, then kills it.
const INSTALL_STOP_TIMEOUT: Duration = Duration::from_secs(10);
/// A crash within this time of the previous one is not restarted, like Wings.
const CRASH_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopStep {
  /// The egg's stop: console command or signal.
  Graceful,
  Term,
  Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
  Teardown,
  Exited {
    crashed: bool,
    restart: bool,
  },
  /// Run this stop step now, then wait for the exit.
  Stop(StopStep, Duration),
  Wait(Duration),
  Install(u64),
  Start,
  /// Running container without an attached console, after a wings restart.
  Adopt,
  Idle,
}

/// What a worker remembers between steps. Lost on a wings restart.
#[derive(Debug, Clone, Copy, Default)]
struct Memory {
  attached: bool,
  stop: Option<(StopStep, Instant)>,
  last_crash: Option<Instant>,
}

/// The reconcile rules as a pure function, so every rule is testable without Docker.
fn decide(
  desired: Option<&Desired>,
  observed: &Observed,
  container: Option<Container>,
  mem: Memory,
  now: Instant,
) -> Action {
  let running = container.is_some_and(|c| c.running);
  let Some(desired) = desired else {
    return match running {
      true => Action::Stop(StopStep::Kill, RETRY),
      false => Action::Teardown,
    };
  };

  let started = container.and_then(|c| c.started_at);
  let requested_since_start =
    |at: Option<DateTime<Utc>>| at.is_some_and(|at| started.is_none_or(|s| s < at));
  let restart = requested_since_start(desired.restarted_at);
  let install = desired
    .server
    .install_generation
    .filter(|g| observed.last_install.map(|i| i.generation) != Some(*g));

  match container {
    Some(c) if !c.running => {
      // like Wings, any exit the panel did not ask for is a crash, exit code 0 included
      let crashed = desired.power == DesiredPower::Running && !restart && install.is_none();
      let restart = crashed
        && desired.server.restart_on_crash
        && mem.last_crash.is_none_or(|t| now - t >= CRASH_WINDOW);
      Action::Exited { crashed, restart }
    }
    Some(_) => {
      if desired.power == DesiredPower::Running && !restart && install.is_none() {
        return if mem.attached {
          Action::Idle
        } else {
          Action::Adopt
        };
      }
      let timeout = match install {
        Some(_) => INSTALL_STOP_TIMEOUT,
        None => Duration::from_secs(desired.server.stop_timeout_s.into()),
      };
      if requested_since_start(desired.killed_at) {
        return Action::Stop(StopStep::Kill, timeout);
      }
      match mem.stop {
        None => Action::Stop(StopStep::Graceful, timeout),
        Some((_, since)) if now - since < timeout => Action::Wait(timeout - (now - since)),
        Some((step, _)) => {
          let next = match step {
            // Wings kills right after the timeout on a reinstall
            StopStep::Graceful if install.is_some() => StopStep::Kill,
            StopStep::Graceful => StopStep::Term,
            StopStep::Term | StopStep::Kill => StopStep::Kill,
          };
          Action::Stop(next, timeout)
        }
      }
    }
    None => match install {
      Some(generation) => Action::Install(generation),
      None if desired.power == DesiredPower::Running => Action::Start,
      None => Action::Idle,
    },
  }
}

enum Next {
  Now,
  Wait(Duration),
  Idle,
  Gone,
}

#[derive(Default)]
struct Live {
  stdin: Option<Stdin>,
  mem: Memory,
}

struct Ctx {
  store: Store,
  docker: Docker,
  workers: DashMap<Uuid, Arc<Notify>>,
}

/// Runs forever. Workers start on the first change of a server, so containers
/// unknown after a wings restart are never touched until the panel resyncs.
pub async fn run(store: Store, docker: Docker) {
  let mut changes = store.subscribe_changes();
  let mut events = docker.events();
  let ctx = Arc::new(Ctx {
    store,
    docker,
    workers: DashMap::new(),
  });
  // servers stored before the subscription would otherwise wait for their next change
  for (uuid, _) in ctx.store.list().await.unwrap_or_default() {
    ctx.wake(uuid, true);
  }
  loop {
    tokio::select! {
      change = changes.recv() => match change {
        Ok(uuid) => ctx.wake(uuid, true),
        Err(RecvError::Lagged(_)) => {
          let uuids: Vec<Uuid> = ctx.workers.iter().map(|w| *w.key()).collect();
          for uuid in uuids {
            ctx.wake(uuid, false);
          }
          for (uuid, _) in ctx.store.list().await.unwrap_or_default() {
            ctx.wake(uuid, true);
          }
        }
        Err(RecvError::Closed) => return,
      },
      Some(uuid) = events.recv() => ctx.wake(uuid, false),
    }
  }
}

impl Ctx {
  fn wake(self: &Arc<Self>, uuid: Uuid, spawn: bool) {
    if let Some(notify) = self.workers.get(&uuid) {
      notify.notify_one();
      return;
    }
    if spawn {
      let notify = Arc::new(Notify::new());
      self.workers.insert(uuid, notify.clone());
      tokio::spawn(self.clone().worker(uuid, notify));
    }
  }

  async fn worker(self: Arc<Self>, uuid: Uuid, notify: Arc<Notify>) {
    let mut live = Live::default();
    let mut failures = 0;
    loop {
      let next = match self.step(uuid, &mut live, &notify).await {
        Ok(next) => {
          failures = 0;
          next
        }
        Err(err) => {
          warn!("Failed to reconcile server {uuid}: {err:?}");
          failures += 1;
          Next::Wait(backoff(failures))
        }
      };
      let wait = match next {
        Next::Now => continue,
        Next::Wait(wait) => wait,
        Next::Idle => RESYNC,
        Next::Gone => {
          self.workers.remove(&uuid);
          // recreated while tearing down: the change went to this worker
          if matches!(self.store.get(uuid).await, Ok(Some(_))) {
            self.wake(uuid, true);
          }
          return;
        }
      };
      tokio::select! {
        _ = notify.notified() => (),
        _ = tokio::time::sleep(wait) => (),
      }
    }
  }

  async fn step(&self, uuid: Uuid, live: &mut Live, notify: &Notify) -> Result<Next> {
    let desired = self.store.get(uuid).await?;
    let observed = self.store.observed(uuid).await?.unwrap_or_default();
    let container = self.docker.inspect(uuid).await?;
    live.mem.attached = live.stdin.is_some();
    let now = Instant::now();

    let action = decide(desired.as_ref(), &observed, container, live.mem, now);
    let Some(desired) = desired else {
      return Ok(match action {
        Action::Teardown => {
          self.docker.teardown(uuid).await?;
          info!("Removed server {uuid}");
          Next::Gone
        }
        _ => {
          self.docker.signal(uuid, shared::msg::Signal::Kill).await?;
          Next::Wait(RETRY)
        }
      });
    };

    Ok(match action {
      Action::Teardown => Next::Gone,
      Action::Exited { crashed, restart } => {
        self.docker.remove(&docker::name(uuid)).await?;
        live.stdin = None;
        live.mem.stop = None;
        if !crashed {
          self.store.set_state(uuid, ServerState::Offline).await?;
          return Ok(Next::Now);
        }
        live.mem.last_crash = Some(now);
        self.store.set_state(uuid, ServerState::Crashed).await?;
        if restart {
          info!("Server {uuid} crashed, restarting");
        } else {
          info!("Server {uuid} crashed, not restarting");
          self
            .store
            .update(uuid, |d| d.power = DesiredPower::Stopped)
            .await?;
        }
        Next::Now
      }
      Action::Stop(step, wait) => {
        if live.mem.stop.is_none() {
          self.store.set_state(uuid, ServerState::Stopping).await?;
        }
        self.stop(uuid, &desired.server.stop, step, live).await?;
        live.mem.stop = Some((step, now));
        Next::Wait(wait)
      }
      Action::Wait(wait) => Next::Wait(wait),
      Action::Install(generation) => {
        self.store.set_state(uuid, ServerState::Installing).await?;
        let ok = match &desired.server.install.script {
          None => true,
          Some(script) => {
            info!("Installing server {uuid}");
            let install = async {
              self.docker.ensure_volume(&desired.server).await?;
              self.docker.install(&desired.server, script).await
            };
            tokio::select! {
              res = install => res
                .inspect_err(|err| warn!("Install of server {uuid} failed: {err:?}"))
                .is_ok(),
              () = self.superseded(uuid, generation, notify) => {
                info!("Install of server {uuid} is no longer wanted, aborting");
                self.docker.abort_install(uuid).await?;
                return Ok(Next::Now);
              }
              () = tokio::time::sleep(INSTALL_TIMEOUT) => {
                warn!("Install of server {uuid} timed out");
                self.docker.abort_install(uuid).await?;
                false
              }
            }
          }
        };
        let result = InstallResult { generation, ok };
        self.docker.save_install(uuid, result).await;
        self.store.finish_install(uuid, result).await?;
        self.store.set_state(uuid, ServerState::Offline).await?;
        Next::Now
      }
      Action::Start => self.start(&desired, live).await?,
      Action::Adopt => {
        let (stdin, output) = self.docker.attach(uuid).await?;
        live.stdin = Some(stdin);
        tokio::spawn(pump(self.store.clone(), uuid, output, Vec::new()));
        if observed.state != ServerState::Running {
          self.store.set_state(uuid, ServerState::Running).await?;
        }
        Next::Idle
      }
      Action::Idle => {
        live.mem.stop = None;
        // heals a state that lost a race, e.g. `Installing` set after the install finished
        let settled = matches!(observed.state, ServerState::Offline | ServerState::Crashed);
        if container.is_none() && !settled {
          self.store.set_state(uuid, ServerState::Offline).await?;
        }
        Next::Idle
      }
    })
  }

  /// Resolves once the server is deleted or no longer waits for this install.
  async fn superseded(&self, uuid: Uuid, generation: u64, notify: &Notify) {
    loop {
      notify.notified().await;
      match self.store.get(uuid).await {
        Ok(Some(d)) if d.server.install_generation == Some(generation) => (),
        Ok(_) => return,
        Err(err) => warn!("Failed to check install of server {uuid}: {err:?}"),
      }
    }
  }

  async fn stop(
    &self,
    uuid: Uuid,
    stop: &StopAction,
    step: StopStep,
    live: &mut Live,
  ) -> Result<()> {
    use shared::msg::Signal;
    let signal = match (step, stop) {
      (StopStep::Graceful, StopAction::Command(command)) => {
        if let Some(stdin) = &mut live.stdin {
          let line = format!("{command}\n");
          if stdin.write_all(line.as_bytes()).await.is_ok() && stdin.flush().await.is_ok() {
            return Ok(());
          }
          live.stdin = None;
        }
        // like Wings: not attached, fall back to a docker stop
        Signal::Term
      }
      (StopStep::Graceful, StopAction::Signal(signal)) => *signal,
      (StopStep::Term, _) => Signal::Term,
      (StopStep::Kill, _) => Signal::Kill,
    };
    self.docker.signal(uuid, signal).await
  }

  async fn start(&self, desired: &Desired, live: &mut Live) -> Result<Next> {
    let spec = &desired.server;
    let uuid = spec.uuid;
    // like Wings before every start: config files, permissions, disk check
    self.docker.ensure_volume(spec).await?;
    let helper = self.docker.ensure_agent(uuid).await?;
    let used = agent::prepare(&helper, &spec.config_files, &self.docker.interface).await?;
    let limit = u64::try_from(spec.limits.disk_mb).unwrap_or(0) * 1024 * 1024;
    if limit > 0 && used > limit {
      warn!("Server {uuid} is over its disk limit, not starting");
      return self.give_up(uuid).await;
    }

    self.store.set_state(uuid, ServerState::Starting).await?;
    let (stdin, output) = match self.docker.start(spec).await {
      Ok(attached) => attached,
      Err(err) => {
        warn!("Failed to start server {uuid}: {err:?}");
        return self.give_up(uuid).await;
      }
    };
    live.stdin = Some(stdin);
    live.mem.stop = None;
    let done: Vec<Matcher> = spec.startup_done.iter().map(|s| Matcher::new(s)).collect();
    if done.is_empty() {
      self.store.set_state(uuid, ServerState::Running).await?;
    }
    tokio::spawn(pump(self.store.clone(), uuid, output, done));
    info!("Started server {uuid}");
    Ok(Next::Idle)
  }

  /// Like a failed start on Wings: the server ends up offline and stays there.
  async fn give_up(&self, uuid: Uuid) -> Result<Next> {
    self
      .store
      .update(uuid, |d| d.power = DesiredPower::Stopped)
      .await?;
    self.store.set_state(uuid, ServerState::Offline).await?;
    Ok(Next::Idle)
  }
}

/// Wait after `failures` failed steps in a row: doubles from `RETRY` up to `MAX_RETRY`.
fn backoff(failures: u32) -> Duration {
  RETRY
    .saturating_mul(1 << failures.saturating_sub(1).min(16))
    .min(MAX_RETRY)
}

/// An egg `startup.done` entry: text contained in the line, or `regex:<expr>`.
enum Matcher {
  Text(String),
  Regex(Regex),
}

impl Matcher {
  fn new(raw: &str) -> Self {
    match raw.strip_prefix("regex:").filter(|p| !p.is_empty()) {
      Some(pattern) => match Regex::new(pattern) {
        Ok(re) => Self::Regex(re),
        // like Wings: match it as text rather than never reaching running
        Err(_) => Self::Text(pattern.into()),
      },
      None => Self::Text(raw.into()),
    }
  }

  fn matches(&self, line: &str) -> bool {
    match self {
      Self::Text(text) => line.contains(text.as_str()),
      Self::Regex(re) => re.is_match(line),
    }
  }
}

/// Wings' `stripAnsiRegex`.
static ANSI: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"[\x1b\x{9b}][\[\]()#;?]*(?:(?:(?:[a-zA-Z\d]*(?:;[a-zA-Z\d]*)*)?\x07)|(?:(?:\d{1,4}(?:;\d{0,4})*)?[\dA-PRZcf-ntqry=><~]))")
    .expect("valid regex")
});

/// Eggs can't tell wings to strip ANSI (`strip_ansi` is not in the spec), so
/// the raw and the stripped line are both tried.
fn line_done(done: &[Matcher], line: &str) -> bool {
  let stripped = ANSI.replace_all(line, "");
  done.iter().any(|m| m.matches(line) || m.matches(&stripped))
}

/// Reads the console until the container exits. Marks the server running on
/// the first `startup_done` line. Always drains, a full attach stream would
/// block the server's output.
async fn pump(store: Store, uuid: Uuid, mut output: Output, done: Vec<Matcher>) {
  let mut waiting = !done.is_empty();
  let mut buf: Vec<u8> = Vec::new();
  while let Some(Ok(chunk)) = output.next().await {
    if !waiting {
      continue;
    }
    buf.extend_from_slice(chunk.as_ref());
    while let Some(end) = buf.iter().position(|b| *b == b'\n') {
      let line: Vec<u8> = buf.drain(..=end).collect();
      let line = String::from_utf8_lossy(&line);
      if !line_done(&done, line.trim_end_matches(['\r', '\n'])) {
        continue;
      }
      waiting = false;
      let starting = matches!(
        store.observed(uuid).await,
        Ok(Some(Observed {
          state: ServerState::Starting,
          ..
        }))
      );
      if starting && let Err(err) = store.set_state(uuid, ServerState::Running).await {
        warn!("Failed to mark server {uuid} running: {err:?}");
      }
      break;
    }
    // ponytail: a line longer than 1 MiB is never matched
    if buf.len() > 1024 * 1024 {
      buf.clear();
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::store::test::spec;

  fn desired(power: DesiredPower) -> Desired {
    Desired {
      power,
      ..Desired::new(spec(Uuid::new_v4()))
    }
  }

  fn container(running: bool, started_at: DateTime<Utc>) -> Option<Container> {
    Some(Container {
      running,
      started_at: Some(started_at),
    })
  }

  #[test]
  fn deleted_server_is_killed_then_torn_down() {
    let now = Instant::now();
    let c = container(true, Utc::now());
    let m = Memory::default();
    assert!(matches!(
      decide(None, &Observed::default(), c, m, now),
      Action::Stop(StopStep::Kill, _)
    ));
    let c = container(false, Utc::now());
    assert_eq!(
      decide(None, &Observed::default(), c, m, now),
      Action::Teardown
    );
    assert_eq!(
      decide(None, &Observed::default(), None, m, now),
      Action::Teardown
    );
  }

  #[test]
  fn exits_are_crashes_unless_requested() {
    let now = Instant::now();
    let o = Observed::default();
    let m = Memory::default();
    let exited = container(false, Utc::now());

    let running = desired(DesiredPower::Running);
    assert_eq!(
      decide(Some(&running), &o, exited, m, now),
      Action::Exited {
        crashed: true,
        restart: true
      }
    );

    let stopped = desired(DesiredPower::Stopped);
    assert_eq!(
      decide(Some(&stopped), &o, exited, m, now),
      Action::Exited {
        crashed: false,
        restart: false
      }
    );

    let mut restarted = desired(DesiredPower::Running);
    restarted.restarted_at = Some(Utc::now() + chrono::Duration::seconds(1));
    assert_eq!(
      decide(Some(&restarted), &o, exited, m, now),
      Action::Exited {
        crashed: false,
        restart: false
      }
    );

    let recent = Memory {
      last_crash: Some(now),
      ..m
    };
    assert_eq!(
      decide(
        Some(&running),
        &o,
        exited,
        recent,
        now + Duration::from_secs(30)
      ),
      Action::Exited {
        crashed: true,
        restart: false
      }
    );
    assert_eq!(
      decide(
        Some(&running),
        &o,
        exited,
        recent,
        now + Duration::from_secs(61)
      ),
      Action::Exited {
        crashed: true,
        restart: true
      }
    );

    let mut no_restart = desired(DesiredPower::Running);
    no_restart.server.restart_on_crash = false;
    assert_eq!(
      decide(Some(&no_restart), &o, exited, m, now),
      Action::Exited {
        crashed: true,
        restart: false
      }
    );
  }

  #[test]
  fn stop_steps_escalate() {
    let now = Instant::now();
    let o = Observed::default();
    let started = Utc::now();
    let c = container(true, started);
    let d = desired(DesiredPower::Stopped);
    let timeout = Duration::from_secs(60);

    let m = Memory::default();
    assert_eq!(
      decide(Some(&d), &o, c, m, now),
      Action::Stop(StopStep::Graceful, timeout)
    );
    let m = Memory {
      stop: Some((StopStep::Graceful, now)),
      ..m
    };
    assert_eq!(
      decide(Some(&d), &o, c, m, now + Duration::from_secs(10)),
      Action::Wait(Duration::from_secs(50))
    );
    assert_eq!(
      decide(Some(&d), &o, c, m, now + timeout),
      Action::Stop(StopStep::Term, timeout)
    );
    let m = Memory {
      stop: Some((StopStep::Term, now)),
      ..m
    };
    assert_eq!(
      decide(Some(&d), &o, c, m, now + timeout),
      Action::Stop(StopStep::Kill, timeout)
    );

    let mut killed = d.clone();
    killed.killed_at = Some(started + chrono::Duration::seconds(1));
    assert_eq!(
      decide(Some(&killed), &o, c, m, now),
      Action::Stop(StopStep::Kill, timeout)
    );
    // a kill from before this start does not apply
    killed.killed_at = Some(started - chrono::Duration::seconds(1));
    assert_eq!(
      decide(Some(&killed), &o, c, Memory { stop: None, ..m }, now),
      Action::Stop(StopStep::Graceful, timeout)
    );
  }

  #[test]
  fn running_servers_restart_adopt_or_idle() {
    let now = Instant::now();
    let o = Observed::default();
    let started = Utc::now();
    let c = container(true, started);
    let mut d = desired(DesiredPower::Running);
    let attached = Memory {
      attached: true,
      ..Memory::default()
    };

    assert_eq!(decide(Some(&d), &o, c, attached, now), Action::Idle);
    assert_eq!(
      decide(Some(&d), &o, c, Memory::default(), now),
      Action::Adopt
    );
    d.restarted_at = Some(started + chrono::Duration::seconds(1));
    assert!(matches!(
      decide(Some(&d), &o, c, attached, now),
      Action::Stop(StopStep::Graceful, _)
    ));
    assert_eq!(decide(Some(&d), &o, None, attached, now), Action::Start);
    let stopped = desired(DesiredPower::Stopped);
    assert_eq!(
      decide(Some(&stopped), &o, None, attached, now),
      Action::Idle
    );
  }

  #[test]
  fn pending_install_stops_then_installs_once() {
    let now = Instant::now();
    let mut d = desired(DesiredPower::Running);
    d.server.install_generation = Some(2);
    let o = Observed::default();
    let c = container(true, Utc::now());
    let m = Memory {
      attached: true,
      ..Memory::default()
    };

    assert_eq!(
      decide(Some(&d), &o, c, m, now),
      Action::Stop(StopStep::Graceful, INSTALL_STOP_TIMEOUT)
    );
    let stopping = Memory {
      stop: Some((StopStep::Graceful, now)),
      ..m
    };
    assert_eq!(
      decide(Some(&d), &o, c, stopping, now + INSTALL_STOP_TIMEOUT),
      Action::Stop(StopStep::Kill, INSTALL_STOP_TIMEOUT)
    );
    // the kill for the install is not a crash
    let exited = container(false, Utc::now());
    assert_eq!(
      decide(Some(&d), &o, exited, m, now),
      Action::Exited {
        crashed: false,
        restart: false
      }
    );
    assert_eq!(decide(Some(&d), &o, None, m, now), Action::Install(2));

    let done = Observed {
      state: ServerState::Offline,
      last_install: Some(InstallResult {
        generation: 2,
        ok: true,
      }),
    };
    assert_eq!(decide(Some(&d), &done, None, m, now), Action::Start);
  }

  #[test]
  fn retries_back_off() {
    assert_eq!(backoff(1), RETRY);
    assert_eq!(backoff(2), RETRY * 2);
    assert_eq!(backoff(3), RETRY * 4);
    assert_eq!(backoff(100), MAX_RETRY);
  }

  #[test]
  fn startup_done_matching() {
    let done = vec![
      Matcher::new(")! For help, type "),
      Matcher::new("regex:^Listening on \\d+$"),
      Matcher::new("regex:(unclosed"),
    ];
    assert!(line_done(
      &done,
      "[12:00:00 INFO]: Done (3.2s)! For help, type \"help\""
    ));
    assert!(line_done(&done, "\x1b[32mListening on 25565\x1b[0m"));
    assert!(line_done(&done, "text (unclosed here"));
    assert!(!line_done(&done, "Loading libraries"));
  }

  /// Full lifecycle against a real Docker with the agent image in `AGENT_IMAGE`:
  /// `AGENT_IMAGE=smaug-agent:dev cargo test -p wings -- --ignored docker_lifecycle`.
  #[tokio::test]
  #[ignore = "needs docker and the agent image"]
  async fn docker_lifecycle() {
    use serde_json::json;
    use shared::msg::{ConfigFile, ConfigParser, ConfigReplace, InstallSpec, ServerSpec};

    use crate::store::test::memory;

    async fn until<F: Future<Output = bool>>(what: &str, mut check: impl FnMut() -> F) {
      for _ in 0..240 {
        if check().await {
          return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
      }
      panic!("timed out waiting for {what}");
    }

    let config = crate::config::Config {
      docker_network: "smaug-test".into(),
      agent_network: "smaug-test-agents".into(),
      agent_image: std::env::var("AGENT_IMAGE").unwrap_or_else(|_| "smaug-agent:dev".into()),
      install_log_dir: std::env::temp_dir().join(format!("smaug-it-{}", Uuid::new_v4())),
      ..Default::default()
    };
    let store = memory();
    let docker = Docker::new(&config).await.unwrap();
    tokio::spawn(run(store.clone(), docker.clone()));
    let uuid = Uuid::new_v4();
    let body = async {
      let state = || async {
        store
          .observed(uuid)
          .await
          .unwrap()
          .unwrap_or_default()
          .state
      };
      let started = || async {
        docker
          .inspect(uuid)
          .await
          .unwrap()
          .and_then(|c| c.started_at)
      };

      let mut server = spec(uuid);
      server.image = "alpine:3.20".into();
      server.install = InstallSpec {
        image: "alpine:3.20".into(),
        entrypoint: "ash".into(),
        script: Some("echo installed > /mnt/server/marker\nexit 3\n".into()),
      };
      server.install_generation = Some(1);
      // alpine runs a shell on the tty, `exit` is its stop command
      server.stop = StopAction::Command("exit".into());
      server.stop_timeout_s = 5;
      server.config_files = vec![ConfigFile {
        file: "server.properties".into(),
        parser: ConfigParser::Properties,
        replace: vec![ConfigReplace {
          matcher: "server-ip".into(),
          if_value: None,
          replace_with: json!("{{config.docker.interface}}"),
        }],
      }];
      store.put(Desired::new(server.clone())).await.unwrap();

      // a failing exit code still counts as installed, like Wings
      until("install", || async {
        store
          .observed(uuid)
          .await
          .unwrap()
          .unwrap_or_default()
          .last_install
          == Some(InstallResult {
            generation: 1,
            ok: true,
          })
      })
      .await;
      assert_eq!(docker.read_file(uuid, "marker").await, "installed\n");
      assert!(docker.install_log(uuid).exists());

      let mut desired = Desired::new(server.clone());
      desired.power = DesiredPower::Running;
      store.put(desired).await.unwrap();
      until("running", || async {
        state().await == ServerState::Running
      })
      .await;
      let props = docker.read_file(uuid, "server.properties").await;
      assert_eq!(props, format!("server-ip={}\n", docker.interface));

      // a crash restarts once, a second one within 60 s keeps it stopped
      let first = started().await;
      docker
        .signal(uuid, shared::msg::Signal::Kill)
        .await
        .unwrap();
      until("restart after crash", || async {
        state().await == ServerState::Running && started().await.is_some_and(|s| Some(s) != first)
      })
      .await;
      docker
        .signal(uuid, shared::msg::Signal::Kill)
        .await
        .unwrap();
      until("crashed", || async {
        state().await == ServerState::Crashed
          && store.get(uuid).await.unwrap().unwrap().power == DesiredPower::Stopped
      })
      .await;

      let mut desired = store.get(uuid).await.unwrap().unwrap();
      desired.power = DesiredPower::Running;
      store.put(desired.clone()).await.unwrap();
      until("running again", || async {
        state().await == ServerState::Running
      })
      .await;

      // a wings restart loses the store, the panel's sync learns it still runs
      let fresh = memory();
      let installed = ServerSpec {
        install_generation: None,
        ..server.clone()
      };
      crate::servers::upsert(&fresh, Some(&docker), installed)
        .await
        .unwrap();
      assert_eq!(
        fresh.get(uuid).await.unwrap().unwrap().power,
        DesiredPower::Running
      );
      assert_eq!(
        fresh.observed(uuid).await.unwrap().unwrap().state,
        ServerState::Running
      );

      // the stop command ends the shell well before the 5 s timeout
      desired.power = DesiredPower::Stopped;
      store.put(desired).await.unwrap();
      until("stopped", || async {
        state().await == ServerState::Offline && started().await.is_none()
      })
      .await;

      store.delete(uuid).await.unwrap();
      until("teardown", || async { !docker.volume_exists(uuid).await }).await;
      assert!(!docker.install_log(uuid).exists());
    };
    // a failed assertion must not leave the server's containers behind
    let res = futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(body)).await;
    docker.teardown(uuid).await.unwrap();
    let _ = std::fs::remove_dir_all(&config.install_log_dir);
    if let Err(panic) = res {
      std::panic::resume_unwind(panic);
    }
  }
}
