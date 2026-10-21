use std::{collections::HashSet, time::Duration};

use aide::axum::{
  ApiRouter,
  routing::{delete_with, get_with, post_with, put_with},
};
use axum::{Json, extract::Path};
use centaurus::{
  bail,
  db::init::Connection,
  error::{ErrorReportStatusExt, Result},
};
use entity::{egg, egg_variable};
use http::StatusCode;
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;
use uuid::Uuid;

use crate::{
  auth::jwt_auth::JwtAuth,
  db::DBTrait,
  nodes::Wings,
  utils::{EggEditPerm, EggViewPerm, UpdateMessage, Updater, client},
};

pub fn router() -> ApiRouter {
  ApiRouter::new()
    .api_route("/", get_with(list_eggs, |op| op.id("list_eggs")))
    .api_route("/", post_with(create_egg, |op| op.id("create_egg")))
    .api_route("/", put_with(edit_egg, |op| op.id("edit_egg")))
    .api_route("/", delete_with(delete_egg, |op| op.id("delete_egg")))
    .api_route("/{uuid}", get_with(egg_info, |op| op.id("egg_info")))
    .api_route(
      "/{uuid}/source",
      get_with(egg_source, |op| op.id("egg_source")),
    )
}

/// PLCN_v3 egg minus `_comment`, `exported_at` and `uuid`.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct EggBody {
  pub name: String,
  pub author: String,
  pub description: Option<String>,
  pub icon: Option<String>,
  #[serde(default)]
  pub tags: Vec<String>,
  pub features: Option<Vec<String>>,
  /// label -> image, order preserved
  pub docker_images: IndexMap<String, String>,
  #[serde(default)]
  pub file_denylist: Vec<String>,
  /// label -> command, order preserved
  pub startup_commands: IndexMap<String, String>,
  pub config: EggConfig,
  pub scripts: EggScripts,
  #[serde(default)]
  pub meta: Option<EggMeta>,
  pub variables: Vec<EggVariable>,
}

/// `files`/`startup`/`logs` accept a JSON-encoded string or an object; they
/// are always stored and returned as objects (or null).
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct EggConfig {
  #[serde(default)]
  pub files: Value,
  #[serde(default)]
  pub startup: Value,
  #[serde(default)]
  pub logs: Value,
  pub stop: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct EggScripts {
  pub installation: EggInstallScript,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct EggInstallScript {
  pub script: Option<String>,
  pub container: String,
  pub entrypoint: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct EggMeta {
  pub update_url: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct EggVariable {
  pub name: String,
  pub description: String,
  pub env_variable: String,
  pub default_value: String,
  #[serde(default)]
  pub user_viewable: bool,
  #[serde(default)]
  pub user_editable: bool,
  #[serde(default)]
  pub rules: Vec<String>,
  pub sort: Option<i16>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct EggInfo {
  pub uuid: Uuid,
  #[serde(flatten)]
  pub egg: EggBody,
}

#[derive(Serialize, JsonSchema)]
struct EggListItem {
  uuid: Uuid,
  name: String,
  author: String,
  description: Option<String>,
  has_update_url: bool,
}

#[derive(Serialize, JsonSchema)]
struct EggSourceResponse {
  /// Raw egg JSON served at the update URL (any PTDL/PLCN version).
  source: Value,
}

#[derive(Serialize, JsonSchema)]
struct ListEggsResponse {
  eggs: Vec<EggListItem>,
}

#[derive(Serialize, JsonSchema)]
struct CreateEggResponse {
  uuid: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct EditEggRequest {
  uuid: Uuid,
  #[serde(flatten)]
  egg: EggBody,
}

#[derive(Deserialize, JsonSchema)]
struct EggUuid {
  uuid: Uuid,
}

const RESERVED_ENV_NAMES: &[&str] = &[
  "P_SERVER_UUID",
  "P_SERVER_ALLOCATION_LIMIT",
  "SERVER_MEMORY",
  "SERVER_IP",
  "SERVER_PORT",
  "ENV",
  "HOME",
  "USER",
  "STARTUP",
  "MODIFIED_STARTUP",
  "SERVER_UUID",
  "UUID",
  "INTERNAL_IP",
  "HOSTNAME",
  "TERM",
  "LANG",
  "PWD",
  "TZ",
  "TIMEZONE",
];

fn len_ok(s: &str) -> bool {
  (1..=255).contains(&s.chars().count())
}

// ponytail: loose email shape check, not full RFC 5322.
fn is_email(s: &str) -> bool {
  let Some((local, domain)) = s.split_once('@') else {
    return false;
  };
  !local.is_empty()
    && !s.chars().any(char::is_whitespace)
    && !domain.contains('@')
    && domain.contains('.')
    && !domain.starts_with('.')
    && !domain.ends_with('.')
}

/// JSON string -> parsed value; must end up an object or null.
fn normalize_config_value(v: Value, field: &str) -> Result<Value> {
  let v = match v {
    Value::String(s) if s.trim().is_empty() => Value::Null,
    Value::String(s) => match serde_json::from_str(&s) {
      Ok(v) => v,
      Err(_) => {
        bail!(BAD_REQUEST, "config.{field} is not valid JSON");
      }
    },
    v => v,
  };
  if !(v.is_object() || v.is_null()) {
    bail!(BAD_REQUEST, "config.{field} must be an object");
  }
  Ok(v)
}

/// Validates and normalizes (config strings -> objects) the egg.
fn validate(mut egg: EggBody) -> Result<EggBody> {
  if !len_ok(&egg.name) {
    bail!(BAD_REQUEST, "Name must be 1-255 characters");
  }
  if !is_email(&egg.author) {
    bail!(BAD_REQUEST, "Author must be a valid email");
  }
  if egg.docker_images.is_empty() {
    bail!(BAD_REQUEST, "At least one docker image is required");
  }
  if egg.startup_commands.is_empty() {
    bail!(BAD_REQUEST, "At least one startup command is required");
  }
  let mut commands = HashSet::new();
  if !egg.startup_commands.values().all(|c| commands.insert(c)) {
    bail!(BAD_REQUEST, "Startup commands must be distinct");
  }
  if let Some(icon) = &egg.icon
    && !["png", "jpeg", "webp"]
      .iter()
      .any(|t| icon.starts_with(&format!("data:image/{t};base64,")))
  {
    bail!(
      BAD_REQUEST,
      "Icon must be a base64 png, jpeg or webp data URL"
    );
  }

  let mut envs = HashSet::new();
  for var in &egg.variables {
    let env = &var.env_variable;
    if !len_ok(&var.name) {
      bail!(BAD_REQUEST, "Variable name must be 1-255 characters");
    }
    if !len_ok(env)
      || !env
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
      bail!(BAD_REQUEST, "Invalid environment variable name '{env}'");
    }
    if RESERVED_ENV_NAMES.contains(&env.as_str()) {
      bail!(BAD_REQUEST, "Environment variable '{env}' is reserved");
    }
    if !envs.insert(env) {
      bail!(BAD_REQUEST, "Duplicate environment variable '{env}'");
    }
  }

  let c = &mut egg.config;
  c.files = normalize_config_value(c.files.take(), "files")?;
  c.startup = normalize_config_value(c.startup.take(), "startup")?;
  c.logs = normalize_config_value(c.logs.take(), "logs")?;
  Ok(egg)
}

// Postgres receives JSON params as jsonb, which reorders object keys, so
// ordered maps are stored as `[[label, value], ...]`.
fn pairs(map: IndexMap<String, String>) -> serde_json::Result<Value> {
  serde_json::to_value(map.into_iter().collect::<Vec<_>>())
}

pub(crate) fn unpairs(v: Value) -> serde_json::Result<IndexMap<String, String>> {
  Ok(
    serde_json::from_value::<Vec<(String, String)>>(v)?
      .into_iter()
      .collect(),
  )
}

fn to_models(id: Uuid, egg: EggBody) -> Result<(egg::Model, Vec<egg_variable::Model>)> {
  let vars = egg
    .variables
    .into_iter()
    .map(|v| egg_variable::Model {
      id: Uuid::now_v7(),
      egg_id: id,
      name: v.name,
      description: v.description,
      env_variable: v.env_variable,
      default_value: v.default_value,
      user_viewable: v.user_viewable,
      user_editable: v.user_editable,
      sort: v.sort,
      rules: v.rules.into(),
    })
    .collect();
  let model = egg::Model {
    id,
    name: egg.name,
    author: egg.author,
    description: egg.description,
    docker_images: pairs(egg.docker_images)?,
    file_denylist: egg.file_denylist.into(),
    features: egg.features.map(Into::into),
    startup_commands: pairs(egg.startup_commands)?,
    config: serde_json::to_value(egg.config)?,
    install_script: serde_json::to_value(egg.scripts.installation)?,
    update_url: egg.meta.and_then(|m| m.update_url),
    icon: egg.icon,
    tags: egg.tags.into(),
  };
  Ok((model, vars))
}

pub(crate) fn variable_from_model(v: egg_variable::Model) -> Result<EggVariable> {
  Ok(EggVariable {
    name: v.name,
    description: v.description,
    env_variable: v.env_variable,
    default_value: v.default_value,
    user_viewable: v.user_viewable,
    user_editable: v.user_editable,
    rules: serde_json::from_value(v.rules)?,
    sort: v.sort,
  })
}

fn from_models(egg: egg::Model, vars: Vec<egg_variable::Model>) -> Result<EggInfo> {
  let variables = vars
    .into_iter()
    .map(variable_from_model)
    .collect::<Result<_>>()?;
  Ok(EggInfo {
    uuid: egg.id,
    egg: EggBody {
      name: egg.name,
      author: egg.author,
      description: egg.description,
      icon: egg.icon,
      tags: serde_json::from_value(egg.tags)?,
      features: egg.features.map(serde_json::from_value).transpose()?,
      docker_images: unpairs(egg.docker_images)?,
      file_denylist: serde_json::from_value(egg.file_denylist)?,
      startup_commands: unpairs(egg.startup_commands)?,
      config: serde_json::from_value(egg.config)?,
      scripts: EggScripts {
        installation: serde_json::from_value(egg.install_script)?,
      },
      meta: Some(EggMeta {
        update_url: egg.update_url,
      }),
      variables,
    },
  })
}

async fn list_eggs(_auth: JwtAuth<EggViewPerm>, db: Connection) -> Result<Json<ListEggsResponse>> {
  let eggs = db
    .egg()
    .list_eggs()
    .await?
    .into_iter()
    .map(|e| EggListItem {
      uuid: e.id,
      name: e.name,
      author: e.author,
      description: e.description,
      has_update_url: e.update_url.is_some(),
    })
    .collect();
  Ok(Json(ListEggsResponse { eggs }))
}

async fn egg_info(
  _auth: JwtAuth<EggViewPerm>,
  db: Connection,
  Path(req): Path<EggUuid>,
) -> Result<Json<EggInfo>> {
  let (egg, vars) = db.egg().find_by_id(req.uuid).await?;
  Ok(Json(from_models(egg, vars)?))
}

async fn egg_source(
  _auth: JwtAuth<EggViewPerm>,
  db: Connection,
  Path(req): Path<EggUuid>,
) -> Result<Json<EggSourceResponse>> {
  let (egg, _) = db.egg().find_by_id(req.uuid).await?;
  let Some(url) = egg.update_url else {
    bail!(BAD_REQUEST, "Egg has no update URL");
  };
  let source = client()
    .get(&url)
    .timeout(Duration::from_secs(10))
    .send()
    .await
    .and_then(|r| r.error_for_status())
    .status_context(StatusCode::BAD_GATEWAY, "Could not reach the update URL")?
    .json()
    .await
    .status_context(
      StatusCode::BAD_GATEWAY,
      "Update URL did not return valid JSON",
    )?;
  Ok(Json(EggSourceResponse { source }))
}

async fn create_egg(
  _auth: JwtAuth<EggEditPerm>,
  db: Connection,
  updater: Updater,
  Json(body): Json<EggBody>,
) -> Result<Json<CreateEggResponse>> {
  let id = Uuid::now_v7();
  let (egg, vars) = to_models(id, validate(body)?)?;
  db.egg().create_egg(egg, vars).await?;
  info!("Created egg with ID {}", id);

  updater.broadcast(UpdateMessage::Eggs { uuid: id }).await;
  Ok(Json(CreateEggResponse { uuid: id }))
}

async fn edit_egg(
  _auth: JwtAuth<EggEditPerm>,
  db: Connection,
  wings: Wings,
  updater: Updater,
  Json(req): Json<EditEggRequest>,
) -> Result<()> {
  let egg = validate(req.egg)?;
  db.egg().find_by_id(req.uuid).await?;
  // validate guarantees at least one command
  let startup = egg.startup_commands[0].clone();
  let (egg, vars) = to_models(req.uuid, egg)?;
  let servers = db.egg().update_egg(egg, vars, &startup).await?;
  info!("Updated egg with ID {}", req.uuid);

  updater
    .broadcast(UpdateMessage::Eggs { uuid: req.uuid })
    .await;
  for uuid in servers {
    let server = db.server().find_by_id(uuid).await?;
    wings.put_server(server.node_id, server.id);
    updater.broadcast(UpdateMessage::Servers { uuid }).await;
  }
  Ok(())
}

async fn delete_egg(
  _auth: JwtAuth<EggEditPerm>,
  db: Connection,
  updater: Updater,
  Json(req): Json<EggUuid>,
) -> Result<()> {
  if db.server().count_by_egg(req.uuid).await? > 0 {
    bail!(CONFLICT, "Egg is in use by servers");
  }
  if db.egg().delete_egg(req.uuid).await? == 0 {
    bail!(NOT_FOUND, "Egg not found");
  }
  info!("Deleted egg with ID {}", req.uuid);

  updater
    .broadcast(UpdateMessage::Eggs { uuid: req.uuid })
    .await;
  Ok(())
}

#[cfg(test)]
mod test {
  use http::StatusCode;
  use serde_json::json;

  use super::*;

  fn sample() -> Value {
    json!({
      "name": "Paper",
      "author": "dev@example.com",
      "description": null,
      "tags": ["minecraft"],
      "features": ["eula"],
      "docker_images": { "Java 21": "ghcr.io/java:21", "Java 17": "ghcr.io/java:17" },
      "file_denylist": [],
      "startup_commands": { "Default": "java -jar server.jar" },
      "config": {
        "files": "{\"server.properties\":{\"parser\":\"properties\",\"find\":{}}}",
        "startup": { "done": ")! For help, type " },
        "logs": "{}",
        "stop": "stop"
      },
      "scripts": { "installation": { "script": "echo hi", "container": "alpine", "entrypoint": "ash" } },
      "variables": [
        { "name": "Version", "description": "", "env_variable": "MC_VERSION", "default_value": "latest", "user_viewable": true, "user_editable": true, "rules": ["required"], "sort": 1 }
      ]
    })
  }

  fn check(f: impl FnOnce(&mut Value)) -> Result<EggBody> {
    let mut v = sample();
    f(&mut v);
    validate(serde_json::from_value(v).unwrap())
  }

  /// Asserts a 400 whose message contains `msg`.
  fn rejects(f: impl FnOnce(&mut Value), msg: &str) {
    let err = check(f).expect_err(msg);
    assert_eq!(err.status, StatusCode::BAD_REQUEST, "{msg}");
    assert!(err.to_string().contains(msg), "{err} !~ {msg}");
  }

  #[test]
  fn accepts_sample_and_parses_config_strings() {
    let egg = check(|_| {}).unwrap();
    assert_eq!(
      egg.config.files["server.properties"]["parser"],
      "properties"
    );
    assert_eq!(egg.config.logs, json!({}));
    assert_eq!(egg.config.startup, json!({ "done": ")! For help, type " }));
    assert_eq!(egg.config.stop.as_deref(), Some("stop"));
  }

  #[test]
  fn deserialize_defaults() {
    let mut v = sample();
    let o = v.as_object_mut().unwrap();
    for k in ["tags", "file_denylist", "icon", "meta"] {
      o.remove(k);
    }
    v["config"] = json!({ "stop": null });
    v["variables"] =
      json!([{ "name": "n", "description": "", "env_variable": "E", "default_value": "" }]);
    let egg: EggBody = serde_json::from_value(v).unwrap();
    assert!(egg.tags.is_empty() && egg.file_denylist.is_empty());
    assert!(egg.icon.is_none() && egg.meta.is_none());
    assert!(
      egg.config.files.is_null() && egg.config.startup.is_null() && egg.config.logs.is_null()
    );
    let var = &egg.variables[0];
    assert!(!var.user_viewable && !var.user_editable && var.rules.is_empty() && var.sort.is_none());
    validate(egg).unwrap();
  }

  #[test]
  fn config_value_normalization() {
    let obj = json!({ "a": 1 });
    let ok = |v: Value| normalize_config_value(v, "files").unwrap();
    assert_eq!(ok(json!("{\"a\":1}")), obj);
    assert_eq!(ok(obj.clone()), obj);
    assert_eq!(ok(Value::Null), Value::Null);
    assert_eq!(ok(json!("")), Value::Null);
    assert_eq!(ok(json!("  \n")), Value::Null);
    assert_eq!(ok(json!("null")), Value::Null);
    for bad in [
      json!("{nope"),
      json!("[1]"),
      json!("\"x\""),
      json!("1"),
      json!([]),
      json!(1),
      json!(true),
    ] {
      let err = normalize_config_value(bad.clone(), "logs").expect_err(&bad.to_string());
      assert_eq!(err.status, StatusCode::BAD_REQUEST);
      assert!(err.to_string().contains("config.logs"));
    }
    // every config field goes through it
    for field in ["files", "startup", "logs"] {
      rejects(
        |v| v["config"][field] = json!("{nope"),
        &format!("config.{field} is not valid JSON"),
      );
      rejects(
        |v| v["config"][field] = json!([]),
        &format!("config.{field} must be an object"),
      );
    }
  }

  #[test]
  fn name_and_author() {
    assert!(check(|v| v["name"] = json!("é".repeat(255))).is_ok());
    rejects(|v| v["name"] = json!(""), "Name must be");
    rejects(|v| v["name"] = json!("x".repeat(256)), "Name must be");
    for good in ["a@b.c", "first.last+tag@sub.example.org"] {
      assert!(is_email(good), "{good}");
    }
    for bad in [
      "", "plain", "@b.c", "a@", "a@b", "a@.b", "a@b.", "a b@c.d", "a@b@c.d", "a@b.c ",
    ] {
      assert!(!is_email(bad), "{bad}");
    }
    rejects(
      |v| v["author"] = json!("nope"),
      "Author must be a valid email",
    );
  }

  #[test]
  fn images_and_commands() {
    rejects(
      |v| v["docker_images"] = json!({}),
      "At least one docker image",
    );
    rejects(
      |v| v["startup_commands"] = json!({}),
      "At least one startup command",
    );
    rejects(
      |v| v["startup_commands"] = json!({ "A": "x", "B": "y", "C": "x" }),
      "distinct",
    );
    assert!(check(|v| v["startup_commands"] = json!({ "A": "x", "B": "y" })).is_ok());
  }

  #[test]
  fn icon() {
    for t in ["png", "jpeg", "webp"] {
      assert!(
        check(|v| v["icon"] = json!(format!("data:image/{t};base64,AAAA"))).is_ok(),
        "{t}"
      );
    }
    assert!(check(|v| v["icon"] = Value::Null).is_ok());
    for bad in [
      "data:image/gif;base64,AAAA",
      "data:image/png,AAAA",
      "https://x/y.png",
      "",
    ] {
      rejects(|v| v["icon"] = json!(bad), "Icon must be");
    }
  }

  #[test]
  fn variables() {
    rejects(
      |v| v["variables"][0]["name"] = json!(""),
      "Variable name must be",
    );
    rejects(
      |v| v["variables"][0]["name"] = json!("x".repeat(256)),
      "Variable name must be",
    );
    for bad in ["", "BAD NAME", "ÄÖ", "A.B", "A$"] {
      rejects(
        |v| v["variables"][0]["env_variable"] = json!(bad),
        "Invalid environment variable",
      );
    }
    rejects(
      |v| v["variables"][0]["env_variable"] = json!("A".repeat(256)),
      "Invalid environment variable",
    );
    for good in ["a-B_9", "server_port", "Z"] {
      assert!(
        check(|v| v["variables"][0]["env_variable"] = json!(good)).is_ok(),
        "{good}"
      );
    }
    assert!(check(|v| v["variables"][0]["env_variable"] = json!("A".repeat(255))).is_ok());
    for reserved in RESERVED_ENV_NAMES {
      rejects(
        |v| v["variables"][0]["env_variable"] = json!(reserved),
        "is reserved",
      );
    }
    rejects(
      |v| {
        let dup = v["variables"][0].clone();
        v["variables"].as_array_mut().unwrap().push(dup);
      },
      "Duplicate environment variable 'MC_VERSION'",
    );
    assert!(check(|v| v["variables"] = json!([])).is_ok());
  }

  #[test]
  fn pairs_keep_order() {
    let map: IndexMap<String, String> = [("z", "1"), ("a", "2"), ("m", "3")]
      .into_iter()
      .map(|(k, v)| (k.into(), v.into()))
      .collect();
    let stored = pairs(map.clone()).unwrap();
    assert_eq!(stored, json!([["z", "1"], ["a", "2"], ["m", "3"]]));
    let back = unpairs(stored).unwrap();
    assert_eq!(back.keys().collect::<Vec<_>>(), ["z", "a", "m"]);
    assert_eq!(back, map);
    assert!(unpairs(json!({ "z": "1" })).is_err());
  }

  #[test]
  fn model_roundtrip() {
    let mut v = sample();
    v["meta"] = json!({ "update_url": "https://example.com/egg.json" });
    v["icon"] = json!("data:image/png;base64,AAAA");
    let egg = check(|x| *x = v).unwrap();
    let id = Uuid::now_v7();
    let (model, vars) = to_models(id, egg.clone()).unwrap();

    assert_eq!(model.id, id);
    assert_eq!(
      model.update_url.as_deref(),
      Some("https://example.com/egg.json")
    );
    assert_eq!(
      model.docker_images,
      json!([
        ["Java 21", "ghcr.io/java:21"],
        ["Java 17", "ghcr.io/java:17"]
      ])
    );
    assert_eq!(model.install_script["entrypoint"], "ash");
    assert_eq!(
      model.config["files"]["server.properties"]["parser"],
      "properties"
    );
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].egg_id, id);
    assert_eq!(vars[0].rules, json!(["required"]));

    let info = from_models(model, vars).unwrap();
    assert_eq!(info.uuid, id);
    assert_eq!(
      serde_json::to_value(&info.egg).unwrap(),
      serde_json::to_value(&egg).unwrap()
    );
    // EggInfo flattens the body next to uuid
    let out = serde_json::to_value(&info).unwrap();
    assert_eq!(out["uuid"], json!(id));
    assert_eq!(out["name"], "Paper");
  }

  #[test]
  fn model_without_meta_or_features() {
    let egg = check(|v| {
      v["features"] = Value::Null;
      v.as_object_mut().unwrap().remove("meta");
    })
    .unwrap();
    let (model, vars) = to_models(Uuid::now_v7(), egg).unwrap();
    assert!(model.update_url.is_none() && model.features.is_none());
    let info = from_models(model, vars).unwrap();
    assert!(info.egg.features.is_none());
    assert!(info.egg.meta.unwrap().update_url.is_none());
  }

  #[test]
  fn corrupt_models_error() {
    let (model, vars) = to_models(Uuid::now_v7(), check(|_| {}).unwrap()).unwrap();
    let mut bad = model.clone();
    bad.docker_images = json!({ "legacy": "object" });
    assert!(from_models(bad, vars.clone()).is_err());
    let mut bad_vars = vars;
    bad_vars[0].rules = json!("required|string");
    assert!(from_models(model, bad_vars).is_err());
  }
}
