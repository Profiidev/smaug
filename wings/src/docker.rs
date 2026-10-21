use std::{
  collections::{BTreeMap, HashMap, HashSet},
  path::PathBuf,
  pin::Pin,
  time::Duration,
};

use bollard::{
  container::LogOutput,
  errors::Error as DockerError,
  models::{
    ContainerCreateBody, HostConfig, HostConfigLogConfig, Mount, MountTypeEnum, MountVolumeOptions,
    NetworkConnectRequest, NetworkCreateRequest, PortBinding, RestartPolicy, RestartPolicyNameEnum,
    VolumeCreateRequest,
  },
  query_parameters::{
    AttachContainerOptionsBuilder, CreateContainerOptionsBuilder, CreateImageOptionsBuilder,
    EventsOptionsBuilder, KillContainerOptionsBuilder, ListContainersOptionsBuilder,
    ListVolumesOptionsBuilder, LogsOptionsBuilder, RemoveContainerOptionsBuilder,
    RemoveVolumeOptionsBuilder, UploadToContainerOptionsBuilder,
  },
};
use centaurus::{error::Result, eyre::Context};
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, TryStreamExt};
use shared::{
  auth::agent_token,
  msg::{InstallResult, ServerSpec, Signal},
};
use tokio::{io::AsyncWrite, sync::mpsc, time::timeout};
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::Config;

const LABEL: &str = "smaug.server";
/// On the installer: the install generation it runs.
const INSTALL_LABEL: &str = "smaug.install-generation";
/// Same uid/gid as Pterodactyl's default `pterodactyl` user.
const CONTAINER_UID: u32 = 988;
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

/// A running agent: where to reach it and how to authenticate.
pub struct Agent {
  pub url: String,
  pub token: String,
}

#[derive(Clone)]
pub struct Docker {
  client: bollard::Docker,
  network: String,
  /// Gateway of `network`, what Pterodactyl calls `docker.network.interface`.
  pub interface: String,
  timezone: String,
  agent_image: String,
  agent_network: String,
  /// The wings token, agent tokens are derived from it.
  secret: String,
  install_log_dir: PathBuf,
}

/// Game container and the server's volume.
pub fn name(uuid: Uuid) -> String {
  format!("smaug-{uuid}")
}

fn installer_name(uuid: Uuid) -> String {
  format!("smaug-{uuid}_installer")
}

fn agent_name(uuid: Uuid) -> String {
  format!("smaug-{uuid}_agent")
}

/// The server's volume at `target`. `no_copy` keeps image content out of it,
/// like the bind mount Wings uses.
fn volume(uuid: Uuid, target: &str) -> Mount {
  Mount {
    target: Some(target.into()),
    source: Some(name(uuid)),
    typ: Some(MountTypeEnum::VOLUME),
    volume_options: Some(MountVolumeOptions {
      no_copy: Some(true),
      ..Default::default()
    }),
    ..Default::default()
  }
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
  pub async fn new(config: &Config) -> Result<Self> {
    let client = bollard::Docker::connect_with_defaults().context("Failed to connect to docker")?;
    let network = &config.docker_network;
    let interface = ensure_network(&client, network, false).await?;
    info!("Using docker network {network} with interface {interface}");
    ensure_network(&client, &config.agent_network, true).await?;
    join_network(&client, &config.agent_network).await?;
    Ok(Self {
      client,
      network: network.clone(),
      interface,
      timezone: config.timezone.clone(),
      agent_image: config.agent_image.clone(),
      agent_network: config.agent_network.clone(),
      secret: config.token.clone(),
      install_log_dir: config.install_log_dir.clone(),
    })
  }

  /// Creates the server's volume, limited to `disk_mb` where the filesystem
  /// supports it (XFS with pquota). Elsewhere the agent's disk scan is the limit.
  /// ponytail: the size is set once, a later `disk_mb` change is only enforced by the scan
  pub async fn ensure_volume(&self, spec: &ServerSpec) -> Result<()> {
    let request = |size: bool| VolumeCreateRequest {
      name: Some(name(spec.uuid)),
      driver: Some("local".into()),
      driver_opts: (size && spec.limits.disk_mb > 0)
        .then(|| HashMap::from([("size".into(), format!("{}m", spec.limits.disk_mb))])),
      labels: Some(labels(spec.uuid)),
      ..Default::default()
    };
    match self.client.create_volume(request(true)).await {
      Err(err) if err.to_string().contains("quota") => {
        self
          .client
          .create_volume(request(false))
          .await
          .context("Failed to create volume")?;
      }
      res => {
        res.context("Failed to create volume")?;
      }
    }
    Ok(())
  }

  /// Starts the server's agent, recreating it when `agent_image` changed.
  pub async fn ensure_agent(&self, uuid: Uuid) -> Result<Agent> {
    let name = agent_name(uuid);
    let image = match self.client.inspect_image(&self.agent_image).await {
      Err(err) if not_found(&err) => {
        self.pull(&self.agent_image).await?;
        self.client.inspect_image(&self.agent_image).await
      }
      res => res,
    }
    .context("Failed to inspect agent image")?;

    let current = match self.client.inspect_container(&name, None).await {
      Err(err) if not_found(&err) => None,
      res => Some(res.context("Failed to inspect agent")?),
    };
    let token = format!("TOKEN={}", self.agent_token(uuid)?);
    // a new wings token means a new agent token
    let outdated = current.as_ref().is_some_and(|c| {
      let env = c.config.as_ref().and_then(|c| c.env.as_ref());
      c.image != image.id || !env.is_some_and(|env| env.contains(&token))
    });
    if outdated {
      info!("Agent image or token changed, recreating agent of {uuid}");
      self.remove(&name).await?;
    }
    if current.is_none() || outdated {
      let host = HostConfig {
        mounts: Some(vec![volume(uuid, "/data")]),
        network_mode: Some(self.agent_network.clone()),
        readonly_rootfs: Some(true),
        cap_drop: Some(vec!["ALL".into()]),
        // chown and reading or writing files of the container user
        cap_add: Some(vec!["CHOWN".into(), "DAC_OVERRIDE".into(), "FOWNER".into()]),
        security_opt: Some(vec!["no-new-privileges".into()]),
        restart_policy: Some(RestartPolicy {
          name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
          ..Default::default()
        }),
        pids_limit: Some(64),
        ..Default::default()
      };
      let config = ContainerCreateBody {
        image: image.id.clone(),
        user: Some("0:0".into()),
        env: Some(vec![token, "ROOT=/data".into()]),
        labels: Some(labels(uuid)),
        host_config: Some(host),
        ..Default::default()
      };
      let options = CreateContainerOptionsBuilder::default().name(&name).build();
      self
        .client
        .create_container(Some(options), config)
        .await
        .context("Failed to create agent")?;
    }
    // starting a running container is a no-op (304)
    self
      .client
      .start_container(&name, None)
      .await
      .context("Failed to start agent")?;

    let info = self
      .client
      .inspect_container(&name, None)
      .await
      .context("Failed to inspect agent")?;
    let ip = info
      .network_settings
      .and_then(|n| n.networks)
      .and_then(|n| n.get(&self.agent_network)?.ip_address.clone())
      .filter(|ip| !ip.is_empty())
      .ok_or_else(|| centaurus::eyre::eyre!("Agent of {uuid} has no ip"))?;
    Ok(Agent {
      url: format!("http://{ip}:8000"),
      token: self.agent_token(uuid)?,
    })
  }

  fn agent_token(&self, uuid: Uuid) -> Result<String> {
    agent_token(&self.secret, uuid)
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
  pub async fn pull(&self, image: &str) -> Result<String> {
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

  /// Recreates and starts the server container from `image`, as returned by
  /// `pull`. Attaches before starting so no output is missed.
  pub async fn start(&self, spec: &ServerSpec, image: String) -> Result<(Stdin, Output)> {
    let name = name(spec.uuid);
    self.remove(&name).await?;

    let (exposed, bindings) = self.ports(spec);
    let host = HostConfig {
      mounts: Some(vec![volume(spec.uuid, "/home/container")]),
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

  /// With `keep`, teardowns of `uuid` leave its volume: the files of a server
  /// transferred away stay until the transfer copies them. On disk, so a
  /// wings restart neither forgets nor reports it as an orphan.
  // TODO(wings): kept volumes are never removed, clean them up once transfers copy files
  pub async fn keep_volume(&self, uuid: Uuid, keep: bool) -> Result<()> {
    let marker = self.kept_marker(uuid);
    if keep {
      tokio::fs::create_dir_all(&self.install_log_dir).await?;
      tokio::fs::write(marker, b"").await?;
    } else if let Err(err) = tokio::fs::remove_file(marker).await
      && err.kind() != std::io::ErrorKind::NotFound
    {
      return Err(err.into());
    }
    Ok(())
  }

  fn kept_marker(&self, uuid: Uuid) -> PathBuf {
    self.install_log_dir.join(format!("{uuid}.kept"))
  }

  /// Servers with containers, or with a volume not kept for a transfer.
  pub async fn labelled(&self) -> Result<HashSet<Uuid>> {
    let filters = HashMap::from([("label", vec![LABEL])]);
    let containers = self
      .client
      .list_containers(Some(
        ListContainersOptionsBuilder::default()
          .all(true)
          .filters(&filters)
          .build(),
      ))
      .await
      .context("Failed to list containers")?;
    let volumes = self
      .client
      .list_volumes(Some(
        ListVolumesOptionsBuilder::default()
          .filters(&filters)
          .build(),
      ))
      .await
      .context("Failed to list volumes")?
      .volumes
      .unwrap_or_default();
    let uuid = |labels: Option<&HashMap<String, String>>| labels?.get(LABEL)?.parse().ok();
    let mut uuids: HashSet<Uuid> = containers
      .iter()
      .filter_map(|c| uuid(c.labels.as_ref()))
      .collect();
    for volume in &volumes {
      if let Some(uuid) = uuid(Some(&volume.labels))
        && !self.is_kept(uuid)
      {
        uuids.insert(uuid);
      }
    }
    Ok(uuids)
  }

  /// Whether the server's files are kept for a transfer.
  pub fn is_kept(&self, uuid: Uuid) -> bool {
    self.kept_marker(uuid).exists()
  }

  /// Removes the server's containers and, unless `keep_volume`, its files.
  pub async fn teardown(&self, uuid: Uuid, keep_volume: bool) -> Result<()> {
    for name in [name(uuid), installer_name(uuid), agent_name(uuid)] {
      self.remove(&name).await?;
    }
    if !keep_volume {
      let options = RemoveVolumeOptionsBuilder::default().force(true).build();
      match self.client.remove_volume(&name(uuid), Some(options)).await {
        Err(err) if not_found(&err) => (),
        res => res.context("Failed to remove volume")?,
      }
    }
    let _ = tokio::fs::remove_file(self.install_log(uuid)).await;
    let _ = tokio::fs::remove_file(self.install_result(uuid)).await;
    let _ = tokio::fs::remove_file(self.stopped_result(uuid)).await;
    Ok(())
  }

  pub fn install_log(&self, uuid: Uuid) -> PathBuf {
    self.install_log_dir.join(format!("{uuid}.log"))
  }

  fn install_result(&self, uuid: Uuid) -> PathBuf {
    self.install_log_dir.join(format!("{uuid}.result.json"))
  }

  fn stopped_result(&self, uuid: Uuid) -> PathBuf {
    self.install_log_dir.join(format!("{uuid}.stopped.json"))
  }

  /// Keeps the result on disk, the memory store loses it on a wings restart.
  pub async fn save_install(&self, uuid: Uuid, result: InstallResult) {
    self.save(self.install_result(uuid), &result).await;
  }

  pub fn last_install(&self, uuid: Uuid) -> Option<InstallResult> {
    load(&self.install_result(uuid))
  }

  /// The power generation wings stopped the server at, see `Store::stop_self`.
  /// On disk so a wings restart does not start it again.
  pub async fn save_stopped(&self, uuid: Uuid, generation: u64) {
    self.save(self.stopped_result(uuid), &generation).await;
  }

  pub fn last_stopped(&self, uuid: Uuid) -> Option<u64> {
    load(&self.stopped_result(uuid))
  }

  async fn save(&self, path: PathBuf, value: &impl serde::Serialize) {
    let json = serde_json::to_vec(value).expect("always serializable");
    let written = async {
      tokio::fs::create_dir_all(&self.install_log_dir).await?;
      tokio::fs::write(&path, json).await
    };
    if let Err(err) = written.await {
      warn!("Failed to save {}: {err}", path.display());
    }
  }

  /// Removes a running installer, its install then fails.
  pub async fn abort_install(&self, uuid: Uuid) -> Result<()> {
    self.remove(&installer_name(uuid)).await
  }

  /// Runs the install script like Wings: `install.image` as root, `files` at
  /// `/mnt/server`, the script at `/mnt/install/install.sh`. Like Wings, the
  /// script's exit code does not fail the install, only errors running it do.
  pub async fn install(&self, spec: &ServerSpec, script: &str) -> Result<()> {
    let name = installer_name(spec.uuid);
    let generation = spec.install_generation.unwrap_or_default().to_string();
    let env = self.env(spec);
    let (image, res) = match self.started_installer(&name, &generation).await? {
      // left by a wings restart: wait for that run instead of running the script again
      Some(image) => {
        info!("Waiting for the running install of {}", spec.uuid);
        (image, self.wait_installer(&name).await)
      }
      None => {
        let image = self
          .create_installer(spec, &name, &generation, &env)
          .await?;
        let res = match self.upload_script(&name, script).await {
          Ok(()) => self.run_installer(&name).await,
          Err(err) => Err(err),
        };
        (image, res)
      }
    };
    self
      .write_install_log(spec, &name, &image, &env, &res)
      .await;
    self.remove(&name).await?;
    res.map(|_| ())
  }

  /// The image of an installer for `generation` that was started, `None` when
  /// there is none. One only created may lack its script, it is replaced.
  async fn started_installer(&self, name: &str, generation: &str) -> Result<Option<String>> {
    let info = match self.client.inspect_container(name, None).await {
      Err(err) if not_found(&err) => return Ok(None),
      res => res.context("Failed to inspect install container")?,
    };
    let config = info.config.unwrap_or_default();
    let ours = config
      .labels
      .as_ref()
      .and_then(|l| l.get(INSTALL_LABEL))
      .is_some_and(|g| g == generation);
    let started = info
      .state
      .and_then(|s| s.started_at)
      .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
      // docker reports never started as year 1
      .is_some_and(|t| t.timestamp() > 0);
    Ok((ours && started).then(|| config.image.unwrap_or_default()))
  }

  /// Creates the installer, returns its image.
  async fn create_installer(
    &self,
    spec: &ServerSpec,
    name: &str,
    generation: &str,
    env: &[String],
  ) -> Result<String> {
    self.remove(name).await?;
    let image = self.pull(&spec.install.image).await?;

    let memory = spec.limits.memory_mb.max(INSTALL_MEMORY_MB);
    let cpu = match spec.limits.cpu_limit {
      0 => 0,
      cpu => cpu.max(INSTALL_CPU),
    };
    let host = HostConfig {
      mounts: Some(vec![volume(spec.uuid, "/mnt/server")]),
      cap_drop: Some(INSTALL_CAP_DROP.map(String::from).to_vec()),
      ..self.host_config(memory, cpu, INSTALL_PIDS)
    };
    let mut labels = labels(spec.uuid);
    labels.insert(INSTALL_LABEL.into(), generation.into());
    let config = ContainerCreateBody {
      image: Some(image.clone()),
      hostname: Some("installer".into()),
      cmd: Some(vec![
        spec.install.entrypoint.clone(),
        "/mnt/install/install.sh".into(),
      ]),
      env: Some(env.to_vec()),
      labels: Some(labels),
      attach_stdout: Some(true),
      attach_stderr: Some(true),
      open_stdin: Some(true),
      tty: Some(true),
      host_config: Some(host),
      ..Default::default()
    };
    let options = CreateContainerOptionsBuilder::default().name(name).build();
    self
      .client
      .create_container(Some(options), config)
      .await
      .context("Failed to create install container")?;
    Ok(image)
  }

  async fn write_install_log(
    &self,
    spec: &ServerSpec,
    name: &str,
    image: &str,
    env: &[String],
    res: &Result<i64>,
  ) {
    let mut log = format!(
      "Smaug Server Installation Log\n\n|\n| Details\n| ------------------------------\n  Server UUID:          {}\n  Container Image:      {}\n  Container Entrypoint: {}\n\n|\n| Environment Variables\n| ------------------------------\n",
      spec.uuid, image, spec.install.entrypoint
    );
    for var in env {
      log.push_str(&format!("  {var}\n"));
    }
    log.push_str("\n|\n| Script Output\n| ------------------------------\n");
    let options = LogsOptionsBuilder::default()
      .stdout(true)
      .stderr(true)
      .build();
    let mut logs = self.client.logs(name, Some(options));
    while let Some(Ok(chunk)) = logs.next().await {
      log.push_str(&String::from_utf8_lossy(chunk.as_ref()));
    }
    match res {
      Ok(code) => log.push_str(&format!("\nInstaller exited with code {code}\n")),
      Err(err) => log.push_str(&format!("\nInstaller failed: {err:?}\n")),
    }
    let written = async {
      tokio::fs::create_dir_all(&self.install_log_dir).await?;
      tokio::fs::write(self.install_log(spec.uuid), log).await
    };
    if let Err(err) = written.await {
      warn!("Failed to write install log for {}: {err}", spec.uuid);
    }
  }

  /// Puts the script at `/mnt/install/install.sh` of the created installer,
  /// so no host directory is needed.
  async fn upload_script(&self, name: &str, script: &str) -> Result<()> {
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(script.len() as u64);
    header.set_mode(0o755);
    tar.append_data(&mut header, "mnt/install/install.sh", script.as_bytes())?;
    let tar = tar.into_inner()?;
    let options = UploadToContainerOptionsBuilder::default().path("/").build();
    self
      .client
      .upload_to_container(name, Some(options), bollard::body_full(tar.into()))
      .await
      .context("Failed to upload install script")?;
    Ok(())
  }

  async fn run_installer(&self, name: &str) -> Result<i64> {
    self
      .client
      .start_container(name, None)
      .await
      .context("Failed to start install container")?;
    self.wait_installer(name).await
  }

  /// Exit code of the installer, right away when it already exited.
  async fn wait_installer(&self, name: &str) -> Result<i64> {
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

fn load<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Option<T> {
  serde_json::from_slice(&std::fs::read(path).ok()?).ok()
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
async fn ensure_network(client: &bollard::Docker, network: &str, internal: bool) -> Result<String> {
  let info = match client.inspect_network(network, None).await {
    Err(err) if not_found(&err) => {
      client
        .create_network(NetworkCreateRequest {
          name: network.into(),
          driver: Some("bridge".into()),
          internal: Some(internal),
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

/// When wings runs in a container it joins the agent network, otherwise it
/// could not reach the agents. On the host, bridge IPs are reachable as is.
async fn join_network(client: &bollard::Docker, network: &str) -> Result<()> {
  let Ok(hostname) = std::fs::read_to_string("/etc/hostname") else {
    return Ok(());
  };
  let me = match client.inspect_container(hostname.trim(), None).await {
    Err(err) if not_found(&err) => return Ok(()),
    res => res.context("Failed to inspect own container")?,
  };
  let joined = me
    .network_settings
    .and_then(|n| n.networks)
    .is_some_and(|n| n.contains_key(network));
  if !joined && let Some(id) = me.id {
    client
      .connect_network(
        network,
        NetworkConnectRequest {
          container: id,
          ..Default::default()
        },
      )
      .await
      .context("Failed to join agent network")?;
    info!("Joined docker network {network}");
  }
  Ok(())
}

#[cfg(test)]
impl Docker {
  /// Reads a file from the server's volume through a created, never started container.
  pub async fn read_file(&self, uuid: Uuid, path: &str) -> String {
    use std::io::Read;

    let name = format!("smaug-{uuid}_read");
    let config = ContainerCreateBody {
      image: Some(self.pull("alpine:3.20").await.unwrap()),
      host_config: Some(HostConfig {
        mounts: Some(vec![volume(uuid, "/v")]),
        ..Default::default()
      }),
      ..Default::default()
    };
    let options = CreateContainerOptionsBuilder::default().name(&name).build();
    self
      .client
      .create_container(Some(options), config)
      .await
      .unwrap();
    let options = bollard::query_parameters::DownloadFromContainerOptionsBuilder::default()
      .path(&format!("/v/{path}"))
      .build();
    let tar: Vec<u8> = self
      .client
      .download_from_container(&name, Some(options))
      .try_fold(Vec::new(), |mut all, chunk| async move {
        all.extend_from_slice(&chunk);
        Ok(all)
      })
      .await
      .unwrap();
    self.remove(&name).await.unwrap();
    let mut archive = tar::Archive::new(tar.as_slice());
    let mut entry = archive.entries().unwrap().next().unwrap().unwrap();
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    content
  }

  pub async fn volume_exists(&self, uuid: Uuid) -> bool {
    self.client.inspect_volume(&name(uuid)).await.is_ok()
  }
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
