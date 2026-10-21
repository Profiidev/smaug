use std::collections::BTreeMap;

use centaurus::{db::init::Connection, error::Result};
use entity::server;
use shared::msg::{Allocation, Limits, ServerSpec};

use crate::db::DBTrait;

/// What wings gets for a server. Pure, so handlers can build it before the DB write.
pub fn build(
  server: &server::Model,
  allocations: &[(String, i32)],
  variables: impl IntoIterator<Item = (String, String)>,
) -> ServerSpec {
  let mut env: BTreeMap<String, String> = variables.into_iter().collect();
  env.insert("SERVER_MEMORY".into(), server.memory_mb.to_string());
  if let Some((ip, port)) = allocations.first() {
    env.insert("SERVER_IP".into(), ip.clone());
    env.insert("SERVER_PORT".into(), port.to_string());
  }
  let startup = env
    .iter()
    .fold(server.startup.clone(), |startup, (key, value)| {
      startup.replace(&format!("{{{{{key}}}}}"), value)
    });

  ServerSpec {
    uuid: server.id,
    image: server.image.clone(),
    startup,
    env,
    limits: Limits {
      cpu_limit: server.cpu_limit,
      memory_mb: server.memory_mb,
      disk_mb: server.disk_mb,
    },
    allocations: allocations
      .iter()
      .map(|(ip, port)| Allocation {
        ip: ip.clone(),
        // ports are validated to 1..=65535 before they are stored
        port: *port as u16,
      })
      .collect(),
  }
}

pub async fn load(db: &Connection, server: &server::Model) -> Result<ServerSpec> {
  let allocations: Vec<_> = db
    .server()
    .allocations(vec![server.id])
    .await?
    .into_iter()
    .map(|a| (a.ip, a.port))
    .collect();
  let variables = db.server().variables(server.id).await?;
  Ok(build(server, &allocations, variables))
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::servers::status::test::model;

  #[test]
  fn build_renders_startup_and_env() {
    let mut server = model();
    server.startup =
      "java -Xmx{{SERVER_MEMORY}}M -jar {{JAR}} --port {{SERVER_PORT}} {{MISSING}}".into();
    let spec = build(
      &server,
      &[("0.0.0.0".into(), 25565), ("0.0.0.0".into(), 25566)],
      [("JAR".to_string(), "server.jar".to_string())],
    );

    assert_eq!(
      spec.startup,
      "java -Xmx512M -jar server.jar --port 25565 {{MISSING}}"
    );
    assert_eq!(spec.env["SERVER_IP"], "0.0.0.0");
    assert_eq!(spec.env["JAR"], "server.jar");
    assert_eq!(spec.allocations.len(), 2);
    assert_eq!(spec.allocations[0].port, 25565);
  }
}
