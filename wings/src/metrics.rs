//! `Event::Metrics` every 2 s for every stored server. Live only, a panel not
//! connected misses them.

use std::{sync::Arc, time::Duration};

use bollard::models::ContainerStatsResponse;
use chrono::Utc;
use dashmap::DashMap;
use shared::msg::{Event, ServerMetrics, ServerState};
use tokio::time::Instant;
use tracing::warn;
use uuid::Uuid;

use crate::{agent, docker::Docker, store::Store};

const INTERVAL: Duration = Duration::from_secs(2);
const DISK_INTERVAL: Duration = Duration::from_secs(60);

/// Counters of the last sample, rates are computed against it.
#[derive(Clone, Copy)]
struct Sample {
  at: Instant,
  cpu: u64,
  system: u64,
  rx: u64,
  tx: u64,
}

pub async fn run(store: Store, docker: Docker) {
  let disk: Arc<DashMap<Uuid, u64>> = Default::default();
  tokio::spawn(disk_loop(store.clone(), docker.clone(), disk.clone()));
  let mut last: std::collections::HashMap<Uuid, Sample> = Default::default();
  let mut tick = tokio::time::interval(INTERVAL);
  loop {
    tick.tick().await;
    let Ok(servers) = store.list().await else {
      continue;
    };
    let mut metrics = Vec::with_capacity(servers.len());
    let mut seen = std::collections::HashMap::new();
    for (uuid, observed) in servers {
      let limit = match store.get(uuid).await {
        Ok(Some(d)) => u64::try_from(d.server.limits.memory_mb).unwrap_or(0) * 1024 * 1024,
        _ => 0,
      };
      let mut m = ServerMetrics {
        uuid,
        memory_limit_bytes: limit,
        disk_bytes: disk.get(&uuid).map(|d| *d).unwrap_or(0),
        ..Default::default()
      };
      let up = matches!(
        observed.state,
        ServerState::Starting | ServerState::Running | ServerState::Stopping
      );
      if up && let Some((stats, started)) = sample(&docker, uuid).await {
        let now = Instant::now();
        let current = counters(&stats, now);
        apply(&mut m, &stats, last.get(&uuid).copied(), current);
        m.uptime_ms = started
          .map(|s| (Utc::now() - s).num_milliseconds().max(0) as u64)
          .unwrap_or(0);
        seen.insert(uuid, current);
      }
      metrics.push(m);
    }
    last = seen;
    store.send(Event::Metrics { servers: metrics });
  }
}

async fn sample(
  docker: &Docker,
  uuid: Uuid,
) -> Option<(ContainerStatsResponse, Option<chrono::DateTime<Utc>>)> {
  let stats = match docker.stats(uuid).await {
    Ok(stats) => stats?,
    Err(err) => {
      warn!("Failed to get stats of server {uuid}: {err:?}");
      return None;
    }
  };
  let started = docker.inspect(uuid).await.ok().flatten()?.started_at;
  Some((stats, started))
}

fn counters(stats: &ContainerStatsResponse, at: Instant) -> Sample {
  let cpu = stats.cpu_stats.as_ref();
  let (rx, tx) = stats
    .networks
    .iter()
    .flatten()
    .fold((0, 0), |(rx, tx), (_, n)| {
      (rx + n.rx_bytes.unwrap_or(0), tx + n.tx_bytes.unwrap_or(0))
    });
  Sample {
    at,
    cpu: cpu
      .and_then(|c| c.cpu_usage.as_ref()?.total_usage)
      .unwrap_or(0),
    system: cpu.and_then(|c| c.system_cpu_usage).unwrap_or(0),
    rx,
    tx,
  }
}

fn apply(m: &mut ServerMetrics, stats: &ContainerStatsResponse, prev: Option<Sample>, now: Sample) {
  let memory = stats.memory_stats.as_ref();
  let usage = memory.and_then(|m| m.usage).unwrap_or(0);
  // like `docker stats`: page cache is not the server's memory
  let cache = memory
    .and_then(|m| m.stats.as_ref())
    .and_then(|s| {
      ["inactive_file", "total_inactive_file", "cache"]
        .iter()
        .find_map(|k| s.get(*k).copied())
    })
    .unwrap_or(0);
  m.memory_bytes = usage.saturating_sub(cache);
  let Some(prev) = prev else {
    return;
  };
  let online = stats
    .cpu_stats
    .as_ref()
    .and_then(|c| c.online_cpus)
    .unwrap_or(1);
  m.cpu_millicores = cpu_millicores(prev.cpu, now.cpu, prev.system, now.system, online);
  let secs = (now.at - prev.at).as_secs_f64();
  if secs > 0.0 {
    // a counter going down is a new container, no rate yet
    m.net_rx_bps = (now.rx.saturating_sub(prev.rx) as f64 / secs) as u64;
    m.net_tx_bps = (now.tx.saturating_sub(prev.tx) as f64 / secs) as u64;
  }
}

/// `docker stats` cpu: container share of the host's cpu time, times cores.
fn cpu_millicores(prev_cpu: u64, cpu: u64, prev_system: u64, system: u64, online: u32) -> u64 {
  let (Some(cpu), Some(system)) = (cpu.checked_sub(prev_cpu), system.checked_sub(prev_system))
  else {
    return 0;
  };
  if system == 0 {
    return 0;
  }
  (cpu as f64 / system as f64 * online as f64 * 1000.0).round() as u64
}

/// Disk usage from the agents, refreshed every 60 s.
async fn disk_loop(store: Store, docker: Docker, disk: Arc<DashMap<Uuid, u64>>) {
  let mut tick = tokio::time::interval(DISK_INTERVAL);
  loop {
    tick.tick().await;
    let Ok(servers) = store.list().await else {
      continue;
    };
    disk.retain(|uuid, _| servers.iter().any(|(u, _)| u == uuid));
    for (uuid, _) in servers {
      let Ok(Some(desired)) = store.get(uuid).await else {
        continue;
      };
      let spec = desired.server;
      // no volume yet means nothing used; ensure_agent would create one unlabelled
      if !docker.has_volume(uuid).await {
        continue;
      }
      let used = async {
        let agent = docker.ensure_agent(uuid).await?;
        agent::wait_up(&agent).await;
        agent::usage(&agent, &spec, false).await
      };
      match used.await {
        Ok(bytes) => {
          disk.insert(uuid, bytes);
        }
        Err(err) => warn!("Failed to get disk usage of server {uuid}: {err}"),
      }
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn cpu_like_docker_stats() {
    // half of the host's time on a 4 core host = 2 cores
    assert_eq!(cpu_millicores(100, 600, 1000, 2000, 4), 2000);
    assert_eq!(cpu_millicores(100, 100, 1000, 2000, 4), 0);
    // counters reset by a new container
    assert_eq!(cpu_millicores(600, 100, 1000, 2000, 4), 0);
    assert_eq!(cpu_millicores(100, 600, 1000, 1000, 4), 0);
  }
}
