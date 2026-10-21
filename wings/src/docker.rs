use std::{
  collections::{BTreeMap, HashMap},
  path::{Path, PathBuf},
  pin::Pin,
  time::Duration,
};

use bollard::{
  container::LogOutput,
  errors::Error as DockerError,
  models::{
    ContainerCreateBody, HostConfig, HostConfigLogConfig, NetworkCreateRequest, PortBinding,
  },
  query_parameters::{
    AttachContainerOptionsBuilder, CreateContainerOptionsBuilder, CreateImageOptionsBuilder,
    EventsOptionsBuilder, KillContainerOptionsBuilder, LogsOptionsBuilder,
    RemoveContainerOptionsBuilder,
  },
};
use centaurus::{error::Result, eyre::Context};
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, TryStreamExt};
use shared::msg::{ServerSpec, Signal};
use tokio::{io::AsyncWrite, sync::mpsc, time::timeout};
use tracing::{info, warn};
use uuid::Uuid;

const LABEL: &str = "smaug.server";
/// Same uid/gid as Pterodactyl's default `pterodactyl` user.
pub const CONTAINER_UID: u32 = 988;
const PULL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
// Pterodactyl defaults for everything below.
const DNS: [&str; 2] = ["1.1.1.1", "1.0.0.1"];
const TMPFS_MB: u32 = 100;
const SERVER_PIDS: i64 = 512;
const INSTALL_PIDS: i64 = 4096;
const INSTALL_MEMORY_MB: i64 = 1024;
const INSTALL_CPU: i32 = 100;
const SERVER_CAP_DROP: [&str; 11] = [
  "setpcap",
  "mknod",
  "audit_write",
  "net_raw",
  "dac_override",
  "fowner",
  "fsetid",
  "net_bind_service",
  "sys_chroot",
  "setfcap",
  "sys_ptrace",
];
const INSTALL_CAP_DROP: [&str; 5] = ["setpcap", "mknod", "audit_write", "net_raw", "sys_chroot"];

pub type Stdin = Pin<Box<dyn AsyncWrite + Send>>;
pub type Output = Pin<Box<dyn Stream<Item = std::result::Result<LogOutput, DockerError>> + Send>>;

/// What wings needs from `docker inspect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Container {
  pub running: bool,
  pub started_at: Option<DateTime<Utc>>,
}

/// Host paths of one server.
pub struct Paths {
  pub root: PathBuf,
  pub files: PathBuf,
}

impl Paths {
  pub fn new(data_dir: &Path, uuid: Uuid) -> Self {
    let root = data_dir.join(uuid.to_string());
    Self {
      files: root.join("files"),
      root,
    }
  }
}

#[derive(Clone)]
pub struct Docker {
  client: bollard::Docker,
  network: String,
  /// Gateway of `network`, what Pterodactyl calls `docker.network.interface`.
  pub interface: String,
  timezone: String,
}

pub fn name(uuid: Uuid) -> String {
  format!("smaug-{uuid}")
}

fn installer_name(uuid: Uuid) -> String {
  format!("smaug-{uuid}_installer")
}

fn not_found(err: &DockerError) -> bool {
  matches!(
    err,
    DockerError::DockerResponseServerError {
      status_code: 404,
      ..
    }
  )
}

impl Docker {
  pub async fn new(network: &str, timezone: &str) -> Result<Self> {
    let client = bollard::Docker::connect_with_defaults().context("Failed to connect to docker")?;
    let interface = ensure_network(&client, network).await?;
    info!("Using docker network {network} with interface {interface}");
    Ok(Self {
      client,
      network: network.into(),
      interface,
      timezone: timezone.into(),
    })
  }

  pub async fn inspect(&self, uuid: Uuid) -> Result<Option<Container>> {
    let info = match self.client.inspect_container(&name(uuid), None).await {
      Err(err) if not_found(&err) => return Ok(None),
      res => res.context("Failed to inspect container")?,
    };
    let state = info.state.unwrap_or_default();
    Ok(Some(Container {
      running: state.running.unwrap_or(false),
      started_at: state
        .started_at
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|t| t.with_timezone(&Utc))
        // docker reports never started as year 1
        .filter(|t| t.timestamp() > 0),
    }))
  }

  /// Env as Wings builds it: `TZ` first, `SERVER_IP` 127.0.0.1 points at the
  /// network interface so the server is reachable from other containers.
  pub fn env(&self, spec: &ServerSpec) -> Vec<String> {
    let mut env = BTreeMap::from([("TZ".to_string(), self.timezone.clone())]);
    env.extend(spec.env.clone());
    if env.get("SERVER_IP").is_some_and(|ip| ip == "127.0.0.1") {
      env.insert("SERVER_IP".into(), self.interface.clone());
    }
    env.into_iter().map(|(k, v)| format!("{k}={v}")).collect()
  }

  /// Pulls the image, falling back to a local copy like Wings. A `~` prefix
  /// means local only. Returns the image name to use.
  async fn pull(&self, image: &str) -> Result<String> {
    if let Some(local) = image.strip_prefix('~') {
      return Ok(local.to_string());
    }
    let options = CreateImageOptionsBuilder::default()
      .from_image(image)
      .build();
    let pull = self
      .client
      .create_image(Some(options), None, None)
      .try_for_each(|_| async { Ok(()) });
    let err = match timeout(PULL_TIMEOUT, pull).await {
      Ok(Ok(())) => return Ok(image.to_string()),
      Ok(Err(err)) => err.to_string(),
      Err(_) => "timed out".to_string(),
    };
    self
      .client
      .inspect_image(image)
      .await
      .with_context(|| format!("Failed to pull image {image}: {err}"))?;
    warn!("Failed to pull image {image}, using the local copy: {err}");
    Ok(image.to_string())
  }

  /// Recreates and starts the server container. Attaches before starting so no
  /// output is missed.
  pub async fn start(&self, spec: &ServerSpec, paths: &Paths) -> Result<(Stdin, Output)> {
    let image = self.pull(&spec.image).await?;
    let name = name(spec.uuid);
    self.remove(&name).await?;

    let (exposed, bindings) = self.ports(spec);
    let host = HostConfig {
      binds: Some(vec![format!("{}:/home/container", paths.files.display())]),
      port_bindings: Some(bindings),
      readonly_rootfs: Some(true),
      cap_drop: Some(SERVER_CAP_DROP.map(String::from).to_vec()),
      ..self.host_config(spec.limits.memory_mb, spec.limits.cpu_limit, SERVER_PIDS)
    };
    let config = ContainerCreateBody {
      image: Some(image),
      hostname: Some(spec.uuid.to_string()),
      user: Some(format!("{CONTAINER_UID}:{CONTAINER_UID}")),
      env: Some(self.env(spec)),
      exposed_ports: Some(exposed),
      labels: Some(labels(spec.uuid)),
      attach_stdin: Some(true),
      attach_stdout: Some(true),
      attach_stderr: Some(true),
      open_stdin: Some(true),
      tty: Some(true),
      host_config: Some(host),
      ..Default::default()
    };
    let options = CreateContainerOptionsBuilder::default().name(&name).build();
    self
      .client
      .create_container(Some(options), config)
      .await
      .context("Failed to create container")?;

    let attached = self.attach(spec.uuid).await?;
    self
      .client
      .start_container(&name, None)
      .await
      .context("Failed to start container")?;
    Ok(attached)
  }

  pub async fn attach(&self, uuid: Uuid) -> Result<(Stdin, Output)> {
    let options = AttachContainerOptionsBuilder::default()
      .stdin(true)
      .stdout(true)
      .stderr(true)
      .stream(true)
      .build();
    let res = self
      .client
      .attach_container(&name(uuid), Some(options))
      .await
      .context("Failed to attach to container")?;
    Ok((res.input, res.output))
  }

  pub async fn signal(&self, uuid: Uuid, signal: Signal) -> Result<()> {
    let signal = match signal {
      Signal::Int => "SIGINT",
      Signal::Term => "SIGTERM",
      Signal::Kill => "SIGKILL",
    };
    let options = KillContainerOptionsBuilder::default()
      .signal(signal)
      .build();
    match self.client.kill_container(&name(uuid), Some(options)).await {
      // 404 = gone, 409 = not running: both mean nothing left to signal
      Err(DockerError::DockerResponseServerError {
        status_code: 404 | 409,
        ..
      }) => Ok(()),
      res => Ok(res.context("Failed to signal container")?),
    }
  }

  pub async fn remove(&self, name: &str) -> Result<()> {
    let options = RemoveContainerOptionsBuilder::default().force(true).build();
    match self.client.remove_container(name, Some(options)).await {
      Err(err) if not_found(&err) => Ok(()),
      res => Ok(res.context("Failed to remove container")?),
    }
  }

  pub async fn teardown(&self, uuid: Uuid, paths: &Paths) -> Result<()> {
    self.remove(&name(uuid)).await?;
    self.remove(&installer_name(uuid)).await?;
    match tokio::fs::remove_dir_all(&paths.root).await {
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
      res => Ok(res.context("Failed to remove server data")?),
    }
  }

  /// Runs the install script like Wings: `install.image` as root, `files` at
  /// `/mnt/server`, the script at `/mnt/install/install.sh`. Like Wings, the
  /// script's exit code does not fail the install, only errors running it do.
  pub async fn install(&self, spec: &ServerSpec, script: &str, paths: &Paths) -> Result<()> {
    let name = installer_name(spec.uuid);
    let script_dir = paths.root.join("install");
    tokio::fs::create_dir_all(&script_dir).await?;
    tokio::fs::create_dir_all(&paths.files).await?;
    tokio::fs::write(script_dir.join("install.sh"), script).await?;
    self.remove(&name).await?;
    let image = self.pull(&spec.install.image).await?;

    let memory = spec.limits.memory_mb.max(INSTALL_MEMORY_MB);
    let cpu = match spec.limits.cpu_limit {
      0 => 0,
      cpu => cpu.max(INSTALL_CPU),
    };
    let host = HostConfig {
      binds: Some(vec![
        format!("{}:/mnt/server", paths.files.display()),
        format!("{}:/mnt/install:ro", script_dir.display()),
      ]),
      cap_drop: Some(INSTALL_CAP_DROP.map(String::from).to_vec()),
      ..self.host_config(memory, cpu, INSTALL_PIDS)
    };
    let env = self.env(spec);
    let config = ContainerCreateBody {
      image: Some(image.clone()),
      hostname: Some("installer".into()),
      cmd: Some(vec![
        spec.install.entrypoint.clone(),
        "/mnt/install/install.sh".into(),
      ]),
      env: Some(env.clone()),
      labels: Some(labels(spec.uuid)),
      attach_stdout: Some(true),
      attach_stderr: Some(true),
      open_stdin: Some(true),
      tty: Some(true),
      host_config: Some(host),
      ..Default::default()
    };
    let options = CreateContainerOptionsBuilder::default().name(&name).build();
    self
      .client
      .create_container(Some(options), config)
      .await
      .context("Failed to create install container")?;
    let res = self.run_installer(&name).await;

    let mut log = format!(
      "Smaug Server Installation Log\n\n|\n| Details\n| ------------------------------\n  Server UUID:          {}\n  Container Image:      {}\n  Container Entrypoint: {}\n\n|\n| Environment Variables\n| ------------------------------\n",
      spec.uuid, image, spec.install.entrypoint
    );
    for var in &env {
      log.push_str(&format!("  {var}\n"));
    }
    log.push_str("\n|\n| Script Output\n| ------------------------------\n");
    let options = LogsOptionsBuilder::default()
      .stdout(true)
      .stderr(true)
      .build();
    let mut logs = self.client.logs(&name, Some(options));
    while let Some(Ok(chunk)) = logs.next().await {
      log.push_str(&String::from_utf8_lossy(chunk.as_ref()));
    }
    match &res {
      Ok(code) => log.push_str(&format!("\nInstaller exited with code {code}\n")),
      Err(err) => log.push_str(&format!("\nInstaller failed: {err:?}\n")),
    }
    if let Err(err) = tokio::fs::write(paths.root.join("install.log"), log).await {
      warn!("Failed to write install log for {}: {err}", spec.uuid);
    }

    self.remove(&name).await?;
    let _ = tokio::fs::remove_dir_all(&script_dir).await;
    res.map(|_| ())
  }

  async fn run_installer(&self, name: &str) -> Result<i64> {
    self
      .client
      .start_container(name, None)
      .await
      .context("Failed to start install container")?;
    match self.client.wait_container(name, None).next().await {
      Some(Ok(res)) => Ok(res.status_code),
      Some(Err(DockerError::DockerContainerWaitError { code, .. })) => Ok(code),
      Some(Err(err)) => Err(err).context("Failed to wait for install container")?,
      None => Ok(-1),
    }
  }

  /// Uuids of servers whose containers started, died or ran out of memory.
  /// Reconnects on error, so the channel only closes with wings.
  pub fn events(&self) -> mpsc::Receiver<Uuid> {
    let (tx, rx) = mpsc::channel(256);
    let client = self.client.clone();
    tokio::spawn(async move {
      let filters = HashMap::from([
        ("type", vec!["container"]),
        ("label", vec![LABEL]),
        ("event", vec!["start", "die", "oom"]),
      ]);
      loop {
        let options = EventsOptionsBuilder::default().filters(&filters).build();
        let mut events = client.events(Some(options));
        while let Some(event) = events.next().await {
          let uuid = match event {
            Ok(event) => event
              .actor
              .and_then(|a| a.attributes)
              .and_then(|a| a.get(LABEL).and_then(|u| u.parse().ok())),
            Err(err) => {
              warn!("Docker event stream failed: {err}");
              break;
            }
          };
          if let Some(uuid) = uuid
            && tx.send(uuid).await.is_err()
          {
            return;
          }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
      }
    });
    rx
  }

  /// Every allocation on both tcp and udp, like Wings.
  fn ports(&self, spec: &ServerSpec) -> (Vec<String>, HashMap<String, Option<Vec<PortBinding>>>) {
    let mut bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();
    for alloc in &spec.allocations {
      let ip = match alloc.ip.as_str() {
        "127.0.0.1" => self.interface.clone(),
        ip => ip.to_string(),
      };
      for proto in ["tcp", "udp"] {
        bindings
          .entry(format!("{}/{proto}", alloc.port))
          .or_insert_with(|| Some(Vec::new()))
          .get_or_insert_default()
          .push(PortBinding {
            host_ip: Some(ip.clone()),
            host_port: Some(alloc.port.to_string()),
          });
      }
    }
    (bindings.keys().cloned().collect(), bindings)
  }

  /// Settings shared by server and install containers, limits as Wings sets them.
  fn host_config(&self, memory_mb: i64, cpu_limit: i32, pids: i64) -> HostConfig {
    let memory = (memory_mb > 0).then(|| bounded_memory(memory_mb));
    HostConfig {
      memory,
      memory_reservation: (memory_mb > 0).then_some(memory_mb * 1024 * 1024),
      // no swap, same as Wings with swap 0
      memory_swap: memory,
      cpu_period: (cpu_limit > 0).then_some(100_000),
      cpu_quota: (cpu_limit > 0).then_some(cpu_limit as i64 * 1_000),
      pids_limit: Some(pids),
      tmpfs: Some(HashMap::from([(
        "/tmp".into(),
        format!("rw,exec,nosuid,size={TMPFS_MB}M"),
      )])),
      dns: Some(DNS.map(String::from).to_vec()),
      log_config: Some(HostConfigLogConfig {
        typ: Some("local".into()),
        config: Some(HashMap::from([
          ("max-size".into(), "5m".into()),
          ("max-file".into(), "1".into()),
          ("compress".into(), "false".into()),
          ("mode".into(), "non-blocking".into()),
        ])),
      }),
      security_opt: Some(vec!["no-new-privileges".into()]),
      network_mode: Some(self.network.clone()),
      ..Default::default()
    }
  }
}

fn labels(uuid: Uuid) -> HashMap<String, String> {
  HashMap::from([(LABEL.into(), uuid.to_string())])
}

/// Wings' memory overhead, so a JVM given `-Xmx$SERVER_MEMORY` is not OOM killed.
fn bounded_memory(memory_mb: i64) -> i64 {
  let multiplier = match memory_mb {
    ..=2048 => 1.15,
    2049..=4096 => 1.10,
    _ => 1.05,
  };
  (memory_mb as f64 * multiplier * 1024.0 * 1024.0).round() as i64
}

/// Creates the bridge network if missing and returns its gateway.
async fn ensure_network(client: &bollard::Docker, network: &str) -> Result<String> {
  let info = match client.inspect_network(network, None).await {
    Err(err) if not_found(&err) => {
      client
        .create_network(NetworkCreateRequest {
          name: network.into(),
          driver: Some("bridge".into()),
          ..Default::default()
        })
        .await
        .context("Failed to create docker network")?;
      client.inspect_network(network, None).await
    }
    res => res,
  }
  .context("Failed to inspect docker network")?;
  let gateway = info
    .ipam
    .and_then(|ipam| ipam.config)
    .into_iter()
    .flatten()
    .find_map(|c| {
      c.gateway
        .filter(|g| g.parse::<std::net::Ipv4Addr>().is_ok())
    });
  Ok(gateway.unwrap_or_else(|| {
    warn!("Docker network {network} has no IPv4 gateway, using 172.18.0.1");
    "172.18.0.1".into()
  }))
}

/// Recursively chowns to the container user without following symlinks, like
/// Wings' `CheckPermissionsOnBoot`.
pub fn chown_all(root: &Path) -> std::io::Result<()> {
  for entry in walkdir::WalkDir::new(root) {
    let entry = entry?;
    std::os::unix::fs::lchown(entry.path(), Some(CONTAINER_UID), Some(CONTAINER_UID))?;
  }
  Ok(())
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

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn memory_overhead_matches_wings() {
    assert_eq!(bounded_memory(1024), 1234803098);
    assert_eq!(
      bounded_memory(4096),
      (4096.0 * 1.10 * 1048576.0_f64).round() as i64
    );
    assert_eq!(
      bounded_memory(8192),
      (8192.0 * 1.05 * 1048576.0_f64).round() as i64
    );
  }
}
