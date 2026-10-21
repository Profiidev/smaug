use std::{
  collections::{BTreeMap, HashMap},
  sync::LazyLock,
};

use centaurus::{db::init::Connection, error::Result};
use entity::{egg, server};
use regex::{Captures, Regex};
use serde_json::Value;
use shared::msg::{
  Allocation, ConfigFile, ConfigParser, ConfigReplace, InstallSpec, Limits, ServerSpec, Signal,
  StopAction,
};
use tracing::warn;

use crate::{
  db::DBTrait,
  eggs::{EggConfig, EggInstallScript},
  nodes::Wings,
};

static PLACEHOLDER: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\{\{\s*([\w.-]+)\s*\}\}").expect("valid regex"));

/// Everything wings needs to install, start and stop the server. Pure, the
/// egg and the server's rows are passed in.
pub fn build(
  server: &server::Model,
  egg: &egg::Model,
  allocations: &[(String, i32)],
  variables: impl IntoIterator<Item = (String, String)>,
) -> Result<ServerSpec> {
  let config: EggConfig = serde_json::from_value(egg.config.clone())?;
  let install: EggInstallScript = serde_json::from_value(egg.install_script.clone())?;

  let mut env: BTreeMap<String, String> = variables.into_iter().collect();
  env.insert("SERVER_MEMORY".into(), server.memory_mb.to_string());
  env.insert("P_SERVER_UUID".into(), server.id.to_string());
  if let Some((ip, port)) = allocations.first() {
    env.insert("SERVER_IP".into(), ip.clone());
    env.insert("SERVER_PORT".into(), port.to_string());
  }
  let values = placeholders(server, allocations, &env);
  let startup = render(&server.startup, &values);
  // the yolks images run `$STARTUP`
  env.insert("STARTUP".into(), startup.clone());

  Ok(ServerSpec {
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
    stop: stop_action(config.stop.as_deref()),
    // validated to 1..=3600 before it is stored
    stop_timeout_s: server.stop_timeout as u32,
    restart_on_crash: server.restart_on_crash,
    startup_done: startup_done(&config.startup),
    config_files: config_files(&config.files, &values),
    file_denylist: serde_json::from_value(egg.file_denylist.clone())?,
    install: InstallSpec {
      image: install.container,
      entrypoint: install.entrypoint,
      script: install
        .script
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.replace("\r\n", "\n")),
    },
  })
}

pub async fn load(db: &Connection, server: &server::Model) -> Result<ServerSpec> {
  let (egg, _) = db.egg().find_by_id(server.egg_id).await?;
  let allocations: Vec<_> = db
    .server()
    .allocations(vec![server.id])
    .await?
    .into_iter()
    .map(|a| (a.ip, a.port))
    .collect();
  let variables = db.server().variables(server.id).await?;
  build(server, &egg, &allocations, variables)
}

/// Loads the current spec from the DB and sends it to the server's node.
/// Best-effort like every wings push.
pub async fn push(db: &Connection, wings: &Wings, server: &server::Model) {
  match load(db, server).await {
    Ok(spec) => wings.put_server(server.node_id, &spec).await,
    Err(err) => warn!("Failed to build spec for server {}: {:?}", server.id, err),
  }
}

/// Values for `{{...}}`: env names as is (startup commands) plus the
/// Pterodactyl config-file names, including their legacy aliases.
fn placeholders(
  server: &server::Model,
  allocations: &[(String, i32)],
  env: &BTreeMap<String, String>,
) -> HashMap<String, String> {
  let mut values = HashMap::new();
  for (key, value) in env {
    for prefix in ["", "env.", "server.build.env.", "server.environment."] {
      values.insert(format!("{prefix}{key}"), value.clone());
    }
  }
  if let Some((ip, port)) = allocations.first() {
    for prefix in ["server.allocations.default", "server.build.default"] {
      values.insert(format!("{prefix}.ip"), ip.clone());
      values.insert(format!("{prefix}.port"), port.to_string());
    }
  }
  for (key, value) in [
    ("memory", server.memory_mb),
    ("memory_limit", server.memory_mb),
    ("disk", server.disk_mb),
    ("disk_space", server.disk_mb),
    ("cpu", server.cpu_limit.into()),
    ("cpu_limit", server.cpu_limit.into()),
  ] {
    values.insert(format!("server.build.{key}"), value.to_string());
  }
  values
}

/// Unknown placeholders stay, wings fills `{{config.*}}` itself.
fn render(text: &str, values: &HashMap<String, String>) -> String {
  PLACEHOLDER
    .replace_all(text, |caps: &Captures| {
      values
        .get(&caps[1])
        .cloned()
        .unwrap_or_else(|| caps[0].to_string())
    })
    .into_owned()
}

fn render_value(value: &Value, values: &HashMap<String, String>) -> Value {
  match value {
    Value::String(s) => Value::String(render(s, values)),
    other => other.clone(),
  }
}

/// Egg `config.stop`: `^C` is SIGINT, empty means SIGTERM, anything else is a console command.
fn stop_action(stop: Option<&str>) -> StopAction {
  match stop.map(str::trim) {
    None | Some("") => StopAction::Signal(Signal::Term),
    Some("^C") => StopAction::Signal(Signal::Int),
    Some(command) => StopAction::Command(command.to_string()),
  }
}

/// Egg `config.startup.done` is a string or a list of strings.
fn startup_done(startup: &Value) -> Vec<String> {
  match &startup["done"] {
    Value::String(s) if !s.is_empty() => vec![s.clone()],
    Value::Array(items) => items
      .iter()
      .filter_map(Value::as_str)
      .filter(|s| !s.is_empty())
      .map(String::from)
      .collect(),
    _ => Vec::new(),
  }
}

/// Egg `config.files`: `{file: {parser, find: {matcher: value | {if_value: value}}}}`.
fn config_files(files: &Value, values: &HashMap<String, String>) -> Vec<ConfigFile> {
  let Some(files) = files.as_object() else {
    return Vec::new();
  };
  files
    .iter()
    .filter_map(|(file, entry)| {
      let Ok(parser) = serde_json::from_value::<ConfigParser>(entry["parser"].clone()) else {
        warn!("Skipping config file {file} with unknown parser");
        return None;
      };
      let mut replace = Vec::new();
      for (matcher, value) in entry["find"].as_object().into_iter().flatten() {
        match value {
          Value::Object(cases) => {
            replace.extend(cases.iter().map(|(if_value, with)| ConfigReplace {
              matcher: matcher.clone(),
              if_value: Some(if_value.clone()),
              replace_with: render_value(with, values),
            }))
          }
          value => replace.push(ConfigReplace {
            matcher: matcher.clone(),
            if_value: None,
            replace_with: render_value(value, values),
          }),
        }
      }
      Some(ConfigFile {
        file: file.clone(),
        parser,
        replace,
      })
    })
    .collect()
}

#[cfg(test)]
mod test {
  use serde_json::json;
  use uuid::Uuid;

  use super::*;
  use crate::servers::status::test::model;

  fn egg_model(config: Value, install: Value) -> egg::Model {
    egg::Model {
      id: Uuid::now_v7(),
      name: "egg".into(),
      author: "a@b.c".into(),
      description: None,
      docker_images: json!([]),
      file_denylist: json!(["*.jar"]),
      features: None,
      startup_commands: json!([]),
      config,
      install_script: install,
      update_url: None,
      icon: None,
      tags: json!([]),
    }
  }

  #[test]
  fn build_renders_everything_wings_needs() {
    let mut server = model();
    server.startup =
      "java -Xmx{{SERVER_MEMORY}}M -jar {{JAR}} --port {{SERVER_PORT}} {{MISSING}}".into();
    let egg = egg_model(
      json!({
        "files": {
          "server.properties": {
            "parser": "properties",
            "find": {
              "server-port": "{{server.allocations.default.port}}",
              "server-ip": "{{server.build.default.ip}}",
              "motd": "{{env.JAR}} on {{config.docker.interface}}",
              "online-mode": { "true": "false" },
              "max-players": 20
            }
          },
          "broken.txt": { "parser": "nope", "find": {} }
        },
        "startup": { "done": ")! For help, type " },
        "logs": {},
        "stop": "stop"
      }),
      json!({ "script": "#!/bin/bash\r\necho hi\r\n", "container": "debian", "entrypoint": "bash" }),
    );

    let spec = build(
      &server,
      &egg,
      &[("0.0.0.0".into(), 25565), ("0.0.0.0".into(), 25566)],
      [("JAR".to_string(), "server.jar".to_string())],
    )
    .unwrap();

    assert_eq!(
      spec.startup,
      "java -Xmx512M -jar server.jar --port 25565 {{MISSING}}"
    );
    assert_eq!(spec.env["STARTUP"], spec.startup);
    assert_eq!(spec.env["SERVER_IP"], "0.0.0.0");
    assert_eq!(spec.env["P_SERVER_UUID"], server.id.to_string());
    assert_eq!(spec.allocations.len(), 2);
    assert_eq!(spec.stop, StopAction::Command("stop".into()));
    assert_eq!(spec.stop_timeout_s, 60);
    assert!(spec.restart_on_crash);
    assert_eq!(spec.startup_done, vec![")! For help, type ".to_string()]);
    assert_eq!(spec.file_denylist, vec!["*.jar".to_string()]);
    assert_eq!(
      spec.install,
      InstallSpec {
        image: "debian".into(),
        entrypoint: "bash".into(),
        script: Some("#!/bin/bash\necho hi\n".into()),
      }
    );

    assert_eq!(spec.config_files.len(), 1);
    let file = &spec.config_files[0];
    assert_eq!(file.file, "server.properties");
    assert_eq!(file.parser, ConfigParser::Properties);
    let find = |m: &str| file.replace.iter().find(|r| r.matcher == m).unwrap();
    assert_eq!(find("server-port").replace_with, json!("25565"));
    assert_eq!(find("server-ip").replace_with, json!("0.0.0.0"));
    assert_eq!(
      find("motd").replace_with,
      json!("server.jar on {{config.docker.interface}}")
    );
    assert_eq!(find("online-mode").if_value.as_deref(), Some("true"));
    assert_eq!(find("online-mode").replace_with, json!("false"));
    assert_eq!(find("max-players").replace_with, json!(20));
  }

  #[test]
  fn stop_and_done_variants() {
    assert_eq!(stop_action(None), StopAction::Signal(Signal::Term));
    assert_eq!(stop_action(Some(" ")), StopAction::Signal(Signal::Term));
    assert_eq!(stop_action(Some("^C")), StopAction::Signal(Signal::Int));
    assert_eq!(
      startup_done(&json!({ "done": ["a", "", "b"] })),
      vec!["a", "b"]
    );
    assert!(startup_done(&Value::Null).is_empty());
  }
}
