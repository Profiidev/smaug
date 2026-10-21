use std::{
  collections::{HashMap, HashSet},
  net::IpAddr,
};

use centaurus::{
  db::{init::Connection, tables::ConnectionExt},
  error::Result,
};
use entity::{egg_variable, server, server_group_access, server_user_access, user};
use http::StatusCode;
use sea_orm::EntityTrait;
use uuid::Uuid;

use super::{
  error::{FieldError, FieldResult, fail_if, field_err},
  rules,
  types::{Grant, GrantsBody, Limits, ServerSpec, UpdateServer},
};
use crate::db::{DBTrait, server::Grants};

pub(super) fn parse_allocation(
  ip: &str,
  port: i32,
  prefix: &str,
  errors: &mut Vec<FieldError>,
) -> Option<(String, i32)> {
  let ip = ip.trim().parse::<IpAddr>();
  if ip.is_err() {
    errors.push(field_err(format!("{prefix}ip"), "Invalid IP address."));
  }
  if !(1024..=65535).contains(&port) {
    errors.push(field_err(
      format!("{prefix}port"),
      "Port must be between 1024 and 65535.",
    ));
  }
  ip.ok()
    .filter(|_| (1024..=65535).contains(&port))
    .map(|ip| (ip.to_string(), port))
}

/// Checks the request-only rules and returns the normalized allocations.
/// One hour; longer would block a stop (and a node shutdown) for ages.
const MAX_STOP_TIMEOUT: i32 = 3600;

pub(super) fn validate_spec(spec: &ServerSpec, errors: &mut Vec<FieldError>) -> Vec<(String, i32)> {
  let name = spec.name.trim();
  if name.is_empty() {
    errors.push(field_err("name", "Name is required."));
  } else if name.chars().count() > 191 {
    errors.push(field_err("name", "Name must be at most 191 characters."));
  }
  if spec.startup.trim().is_empty() {
    errors.push(field_err("startup", "Startup command is required."));
  }
  if spec.image.trim().is_empty() {
    errors.push(field_err("image", "Image is required."));
  }
  if spec.limits.cpu < 0 {
    errors.push(field_err(
      "limits.cpu",
      "CPU limit must be 0 or more millicores.",
    ));
  }
  if spec.limits.memory < 0 {
    errors.push(field_err("limits.memory", "Memory must be 0 or more MiB."));
  }
  if spec.limits.disk < 0 {
    errors.push(field_err("limits.disk", "Storage must be 0 or more MiB."));
  }
  if spec.backup_limit < 0 {
    errors.push(field_err("backup_limit", "Backup limit must be 0 or more."));
  }
  if !(1..=MAX_STOP_TIMEOUT).contains(&spec.stop_timeout) {
    errors.push(field_err(
      "stop_timeout",
      format!("Stop timeout must be between 1 and {MAX_STOP_TIMEOUT} seconds."),
    ));
  }

  if spec.allocations.is_empty() {
    errors.push(field_err(
      "allocations",
      "At least one allocation is required.",
    ));
  }
  let mut out: Vec<(String, i32)> = Vec::new();
  for (i, a) in spec.allocations.iter().enumerate() {
    if let Some(pair) = parse_allocation(&a.ip, a.port, &format!("allocations.{i}."), errors) {
      let field = format!("allocations.{i}");
      if out.contains(&pair) {
        errors.push(field_err(field, "Duplicate allocation."));
      } else if out.iter().any(|o| overlaps(o, &pair)) {
        errors.push(field_err(
          field,
          "Overlaps another allocation on the same port.",
        ));
      }
      out.push(pair);
    }
  }
  out
}

/// Whether two IPs can not be bound on the same port at once. A wildcard
/// covers every address of its family, `::` (dual-stack) covers IPv4 too.
pub(super) fn conflicts(a: &IpAddr, b: &IpAddr) -> bool {
  let covers = |x: &IpAddr, y: &IpAddr| x.is_unspecified() && (x.is_ipv6() || y.is_ipv4());
  a == b || covers(a, b) || covers(b, a)
}

/// [`conflicts`] on normalized `(ip, port)` pairs.
fn overlaps((ip_a, port_a): &(String, i32), (ip_b, port_b): &(String, i32)) -> bool {
  port_a == port_b
    && match (ip_a.parse(), ip_b.parse()) {
      (Ok(a), Ok(b)) => conflicts(&a, &b),
      _ => ip_a == ip_b,
    }
}

/// Final value of every egg variable. Supplied values win, then the current
/// value, then the egg default. Values that changed (or are new) are checked
/// against the egg rules.
pub(super) fn build_variables(
  egg_vars: &[egg_variable::Model],
  env: &HashMap<String, String>,
  current: Option<&HashMap<String, String>>,
  errors: &mut Vec<FieldError>,
) -> Vec<(String, String)> {
  for key in env.keys() {
    if !egg_vars.iter().any(|v| &v.env_variable == key) {
      errors.push(field_err(format!("variables.{key}"), "Unknown variable."));
    }
  }
  egg_vars
    .iter()
    .map(|v| {
      let old = current.and_then(|c| c.get(&v.env_variable));
      let value = env
        .get(&v.env_variable)
        .or(old)
        .unwrap_or(&v.default_value)
        .clone();
      if old != Some(&value) {
        let rules: Vec<String> = serde_json::from_value(v.rules.clone()).unwrap_or_default();
        if let Some(msg) = rules::check(&v.name, &value, &rules) {
          errors.push(field_err(format!("variables.{}", v.env_variable), msg));
        }
      }
      (v.env_variable.clone(), value)
    })
    .collect()
}

/// Whether a non-admin tries to change anything beyond name, startup and
/// the variables it may edit.
pub(super) fn changes_admin_fields(
  server: &server::Model,
  allocations: &[(String, i32)],
  vars: &HashMap<String, String>,
  egg_vars: &[egg_variable::Model],
  req: &UpdateServer,
) -> bool {
  let spec = &req.spec;
  let normalized: Vec<_> = spec
    .allocations
    .iter()
    .map(|a| {
      (
        a.ip
          .trim()
          .parse::<IpAddr>()
          .map(|ip| ip.to_string())
          .unwrap_or_else(|_| a.ip.clone()),
        a.port,
      )
    })
    .collect();
  // hidden and unknown keys count regardless of value, so the 403 can not be
  // used to probe hidden values or variable names
  let locked_var_changed = spec.environment.iter().any(|(key, value)| {
    egg_vars
      .iter()
      .find(|v| &v.env_variable == key)
      .is_none_or(|v| !v.user_viewable || (!v.user_editable && vars.get(key) != Some(value)))
  });
  req.grants.is_some()
    || spec.owner != server.owner_id
    || spec.egg != server.egg_id
    || spec.image != server.image
    || spec.limits
      != (Limits {
        cpu: server.cpu_limit,
        memory: server.memory_mb,
        disk: server.disk_mb,
      })
    || spec.backup_limit != server.backup_limit
    || normalized != allocations
    || locked_var_changed
}

pub(super) async fn check_owner(
  db: &Connection,
  owner: Option<Uuid>,
  errors: &mut Vec<FieldError>,
) -> Result<()> {
  if let Some(owner) = owner
    && user::Entity::find_by_id(owner).one(&**db).await?.is_none()
  {
    errors.push(field_err("owner", "User not found."));
  }
  Ok(())
}

pub(super) async fn egg_vars(
  db: &Connection,
  egg: Uuid,
  errors: &mut Vec<FieldError>,
) -> Result<Option<Vec<egg_variable::Model>>> {
  match db.egg().find_by_id(egg).await {
    Ok((_, vars)) => Ok(Some(vars)),
    Err(e) if e.status == StatusCode::NOT_FOUND => {
      errors.push(field_err("egg", "Egg not found."));
      Ok(None)
    }
    Err(e) => Err(e),
  }
}

pub(super) async fn node_exists(db: &Connection, node: Uuid) -> Result<bool> {
  match db.node().find_by_id(node).await {
    Ok(_) => Ok(true),
    Err(e) if e.status == StatusCode::NOT_FOUND => Ok(false),
    Err(e) => Err(e),
  }
}

/// 409 with a per-row error for allocations already used on `node`.
pub(super) async fn check_free(
  db: &Connection,
  node: Uuid,
  allocations: &[(String, i32)],
  exclude: Option<Uuid>,
  field: impl Fn(usize) -> String,
) -> FieldResult<()> {
  let ports: Vec<i32> = allocations.iter().map(|(_, port)| *port).collect();
  let taken = db.server().taken(node, &ports, exclude).await?;
  let errors = allocations
    .iter()
    .enumerate()
    .filter(|(_, a)| taken.iter().any(|t| overlaps(t, a)))
    .map(|(i, _)| field_err(field(i), "IP and port are already in use on this node."))
    .collect();
  fail_if(StatusCode::CONFLICT, errors)
}

pub(super) async fn validate_grants(
  db: &Connection,
  server: Uuid,
  grants: &GrantsBody,
  errors: &mut Vec<FieldError>,
) -> Result<Grants> {
  let users: HashSet<Uuid> = db
    .user()
    .list_users_simple()
    .await?
    .into_iter()
    .map(|u| u.id)
    .collect();
  let groups: HashSet<Uuid> = db
    .group()
    .list_groups_simple()
    .await?
    .into_iter()
    .map(|g| g.uuid)
    .collect();

  let mut check = |list: &[Grant], known: &HashSet<Uuid>, kind: &str, missing: &str| {
    let mut seen = HashSet::new();
    for (i, g) in list.iter().enumerate() {
      let field = format!("grants.{kind}.{i}");
      if !known.contains(&g.uuid) {
        errors.push(field_err(field, missing));
      } else if !seen.insert(g.uuid) {
        errors.push(field_err(field, "Duplicate grant."));
      }
    }
  };
  check(&grants.users, &users, "users", "User not found.");
  check(&grants.groups, &groups, "groups", "Group not found.");

  Ok(Grants {
    users: grants
      .users
      .iter()
      .map(|g| server_user_access::Model {
        server_id: server,
        user_id: g.uuid,
        access: g.access,
      })
      .collect(),
    groups: grants
      .groups
      .iter()
      .map(|g| server_group_access::Model {
        server_id: server,
        group_id: g.uuid,
        access: g.access,
      })
      .collect(),
  })
}

pub(super) fn apply_spec(server: &mut server::Model, spec: ServerSpec) {
  server.name = spec.name.trim().to_string();
  server.owner_id = spec.owner;
  server.egg_id = spec.egg;
  server.startup = spec.startup;
  server.startup_overridden = spec.startup_overridden;
  server.image = spec.image.trim().to_string();
  server.cpu_limit = spec.limits.cpu;
  server.memory_mb = spec.limits.memory;
  server.disk_mb = spec.limits.disk;
  server.backup_limit = spec.backup_limit;
  server.stop_timeout = spec.stop_timeout;
  server.restart_on_crash = spec.restart_on_crash;
}
#[cfg(test)]
mod test {
  use serde_json::json;

  use super::*;
  use crate::servers::{status::test::model, types::Allocation};

  fn spec() -> ServerSpec {
    serde_json::from_value(json!({
      "name": "survival",
      "owner": null,
      "egg": Uuid::now_v7(),
      "allocations": [{ "ip": "10.0.0.5", "port": 25565 }],
      "startup": "run",
      "startup_overridden": false,
      "image": "img",
      "limits": { "cpu": 0, "memory": 0, "disk": 0 },
      "backup_limit": 0,
      "stop_timeout": 60,
      "restart_on_crash": true
    }))
    .unwrap()
  }

  fn spec_errors(f: impl FnOnce(&mut ServerSpec)) -> Vec<String> {
    let mut s = spec();
    f(&mut s);
    let mut errors = Vec::new();
    validate_spec(&s, &mut errors);
    errors.into_iter().map(|e| e.field).collect()
  }

  fn var(env: &str, rules: &[&str], viewable: bool, editable: bool) -> egg_variable::Model {
    egg_variable::Model {
      id: Uuid::now_v7(),
      egg_id: Uuid::nil(),
      name: env.to_lowercase(),
      description: String::new(),
      env_variable: env.into(),
      default_value: "def".into(),
      user_viewable: viewable,
      user_editable: editable,
      sort: None,
      rules: json!(rules),
    }
  }

  fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect()
  }

  #[test]
  fn spec_validation() {
    assert!(spec_errors(|_| {}).is_empty());
    assert_eq!(spec_errors(|s| s.name = "".into()), ["name"]);
    assert_eq!(spec_errors(|s| s.name = " \t".into()), ["name"]);
    assert_eq!(spec_errors(|s| s.name = "x".repeat(192)), ["name"]);
    assert!(spec_errors(|s| s.name = "ä".repeat(191)).is_empty());
    assert_eq!(spec_errors(|s| s.startup = " ".into()), ["startup"]);
    assert_eq!(spec_errors(|s| s.image = "".into()), ["image"]);
    assert_eq!(spec_errors(|s| s.limits.cpu = -1), ["limits.cpu"]);
    assert_eq!(spec_errors(|s| s.limits.memory = -1), ["limits.memory"]);
    assert_eq!(spec_errors(|s| s.limits.disk = -1), ["limits.disk"]);
    assert_eq!(spec_errors(|s| s.backup_limit = -1), ["backup_limit"]);
    assert_eq!(spec_errors(|s| s.stop_timeout = 0), ["stop_timeout"]);
    assert_eq!(spec_errors(|s| s.stop_timeout = 3601), ["stop_timeout"]);
    assert!(spec_errors(|s| s.stop_timeout = 3600).is_empty());
    assert_eq!(spec_errors(|s| s.allocations.clear()), ["allocations"]);
    // everything at once
    assert_eq!(
      spec_errors(|s| {
        s.name = String::new();
        s.image = String::new();
        s.backup_limit = -5;
      }),
      ["name", "image", "backup_limit"]
    );
  }

  #[test]
  fn allocation_validation_and_normalization() {
    let alloc = |ip: &str, port| Allocation {
      ip: ip.into(),
      port,
    };
    let run = |allocs: Vec<Allocation>| {
      let mut s = spec();
      s.allocations = allocs;
      let mut errors = Vec::new();
      let out = validate_spec(&s, &mut errors);
      (out, errors)
    };
    let (out, errors) = run(vec![
      alloc(" 10.0.0.1 ", 1024),
      alloc("0:0:0:0:0:0:0:1", 65535),
    ]);
    assert!(errors.is_empty());
    assert_eq!(out, [("10.0.0.1".into(), 1024), ("::1".into(), 65535)]);

    let (out, errors) = run(vec![
      alloc("10.0.0.1", 2000),
      alloc("nope", 2000),
      alloc("10.0.0.1", 1023),
      alloc("10.0.0.1", 65536),
      alloc("10.0.0.01", 2000),
      alloc("10.0.0.1", 2000),
      alloc("::1", 2000),
      alloc("::0:1", 2000),
    ]);
    let fields: Vec<_> = errors.iter().map(|e| e.field.as_str()).collect();
    assert_eq!(
      fields,
      [
        "allocations.1.ip",
        "allocations.2.port",
        "allocations.3.port",
        "allocations.4.ip",
        "allocations.5",
        "allocations.7"
      ]
    );
    assert_eq!(errors[4].message, "Duplicate allocation.");
    // invalid rows are not returned
    assert_eq!(out.len(), 4);

    let mut errors = Vec::new();
    assert_eq!(parse_allocation("x", 1, "", &mut errors), Option::None);
    let fields: Vec<_> = errors.iter().map(|e| e.field.as_str()).collect();
    assert_eq!(fields, ["ip", "port"]);
  }

  #[test]
  fn variables_merge_and_validate() {
    let vars = vec![
      var("A", &["required", "integer"], true, true),
      var("B", &["in:def,x"], true, true),
      var("C", &[], false, false),
    ];
    let build = |env: &[(&str, &str)], current: Option<&HashMap<String, String>>| {
      let mut errors = Vec::new();
      let out = build_variables(&vars, &map(env), current, &mut errors);
      (out, errors)
    };

    // create: supplied value, then default; defaults are validated too
    let (out, errors) = build(&[("A", "5")], Option::None);
    assert_eq!(
      out,
      [
        ("A".into(), "5".into()),
        ("B".into(), "def".into()),
        ("C".into(), "def".into())
      ]
    );
    assert!(errors.is_empty());
    let (_, errors) = build(&[], Option::None);
    assert_eq!(
      errors,
      [field_err("variables.A", "The a field must be an integer.")]
    );

    // update: missing keeps current, unchanged invalid values pass
    let current = map(&[("A", "not a number"), ("B", "x")]);
    let (out, errors) = build(&[("B", "x")], Some(&current));
    assert!(errors.is_empty());
    assert_eq!(out[0], ("A".into(), "not a number".into()));
    assert_eq!(out[2], ("C".into(), "def".into()));
    let (_, errors) = build(&[("B", "y")], Some(&current));
    assert_eq!(
      errors,
      [field_err("variables.B", "The selected b is invalid.")]
    );

    // unknown keys
    let (_, errors) = build(&[("A", "1"), ("NOPE", "x")], Option::None);
    assert_eq!(errors, [field_err("variables.NOPE", "Unknown variable.")]);

    // unparsable stored rules are treated as none
    let mut broken = var("D", &[], true, true);
    broken.rules = json!("required");
    let mut errors = Vec::new();
    build_variables(&[broken], &map(&[("D", "")]), Option::None, &mut errors);
    assert!(errors.is_empty());
  }

  #[test]
  fn admin_field_detection() {
    let server = model();
    let allocs = vec![("10.0.0.5".to_string(), 25565)];
    let vars = vec![
      var("EDIT", &[], true, true),
      var("LOCKED", &[], true, false),
      var("HIDDEN_EDIT", &[], false, true),
    ];
    let current = map(&[("EDIT", "e"), ("LOCKED", "l"), ("HIDDEN_EDIT", "h")]);
    let base = || {
      let mut s = spec();
      s.owner = server.owner_id;
      s.egg = server.egg_id;
      s.image = server.image.clone();
      s.limits = Limits {
        cpu: server.cpu_limit,
        memory: server.memory_mb,
        disk: server.disk_mb,
      };
      s.backup_limit = server.backup_limit;
      UpdateServer {
        spec: s,
        grants: Option::None,
      }
    };
    let changes = |f: &dyn Fn(&mut UpdateServer)| {
      let mut req = base();
      f(&mut req);
      changes_admin_fields(&server, &allocs, &current, &vars, &req)
    };

    // write fields only
    assert!(!changes(&|_| {}));
    assert!(!changes(&|r| {
      r.spec.name = "new".into();
      r.spec.startup = "other".into();
      r.spec.startup_overridden = true;
      r.spec.environment = map(&[("EDIT", "changed"), ("LOCKED", "l")]);
      // same pair written differently
      r.spec.allocations[0].ip = " 10.0.0.5".into();
    }));

    for f in [
      &(|r: &mut UpdateServer| r.spec.owner = Option::None) as &dyn Fn(&mut UpdateServer),
      &|r| r.spec.egg = Uuid::now_v7(),
      &|r| r.spec.image = "x".into(),
      &|r| r.spec.limits.cpu += 1,
      &|r| r.spec.limits.memory += 1,
      &|r| r.spec.limits.disk += 1,
      &|r| r.spec.backup_limit += 1,
      &|r| r.spec.allocations[0].port += 1,
      &|r| {
        r.spec.allocations.push(Allocation {
          ip: "10.0.0.5".into(),
          port: 1,
        })
      },
      &|r| r.grants = Some(GrantsBody::default()),
      &|r| r.spec.environment = map(&[("LOCKED", "changed")]),
      &|r| r.spec.environment = map(&[("HIDDEN_EDIT", "changed")]),
      // hidden and unknown keys count even with the current value, no oracle
      &|r| r.spec.environment = map(&[("HIDDEN_EDIT", "h")]),
      &|r| r.spec.environment = map(&[("UNKNOWN", "x")]),
    ] {
      assert!(changes(f));
    }
  }

  #[test]
  fn ip_conflicts() {
    let c = |a: &str, b: &str| {
      let (a, b) = (a.parse().unwrap(), b.parse().unwrap());
      let res = conflicts(&a, &b);
      assert_eq!(res, conflicts(&b, &a), "symmetric for {a} {b}");
      res
    };
    // equal
    assert!(c("10.0.0.1", "10.0.0.1"));
    assert!(c("::1", "::1"));
    assert!(c("0.0.0.0", "0.0.0.0"));
    assert!(c("::", "::"));
    // distinct specific
    assert!(!c("10.0.0.1", "10.0.0.2"));
    assert!(!c("::1", "127.0.0.1"));
    // v4 wildcard covers v4 only
    assert!(c("0.0.0.0", "127.0.0.1"));
    assert!(!c("0.0.0.0", "::1"));
    assert!(!c("0.0.0.0", "::ffff:10.0.0.1"));
    // v6 wildcard covers everything
    assert!(c("::", "::1"));
    assert!(c("::", "127.0.0.1"));
    assert!(c("::", "0.0.0.0"));
  }

  #[test]
  fn pair_overlaps() {
    let p = |ip: &str, port| (ip.to_string(), port);
    assert!(overlaps(&p("0.0.0.0", 2000), &p("10.0.0.1", 2000)));
    assert!(!overlaps(&p("0.0.0.0", 2000), &p("10.0.0.1", 2001)));
    assert!(!overlaps(&p("10.0.0.1", 2000), &p("10.0.0.2", 2000)));
    // unparsable stored values fall back to exact match
    assert!(overlaps(&p("x", 2000), &p("x", 2000)));
    assert!(!overlaps(&p("x", 2000), &p("0.0.0.0", 2000)));
  }

  #[test]
  fn in_request_overlap() {
    let run = |allocs: &[(&str, i32)]| {
      let mut s = spec();
      s.allocations = allocs
        .iter()
        .map(|(ip, port)| Allocation {
          ip: ip.to_string(),
          port: *port,
        })
        .collect();
      let mut errors = Vec::new();
      validate_spec(&s, &mut errors);
      errors
    };
    let overlap = |i: usize| {
      field_err(
        format!("allocations.{i}"),
        "Overlaps another allocation on the same port.",
      )
    };
    assert_eq!(run(&[("10.0.0.1", 2000), ("0.0.0.0", 2000)]), [overlap(1)]);
    assert_eq!(run(&[("::", 2000), ("10.0.0.1", 2000)]), [overlap(1)]);
    assert_eq!(run(&[("::1", 2000), ("::", 2000)]), [overlap(1)]);
    // exact duplicate keeps its own message
    assert_eq!(
      run(&[("0.0.0.0", 2000), ("10.0.0.1", 2000), ("0.0.0.0", 2000)]),
      [
        overlap(1),
        field_err("allocations.2", "Duplicate allocation.")
      ]
    );
    // different port or family is fine
    assert!(run(&[("0.0.0.0", 2000), ("10.0.0.1", 2001)]).is_empty());
    assert!(run(&[("0.0.0.0", 2000), ("::1", 2000)]).is_empty());
  }

  #[tokio::test]
  async fn lookup_db_errors_propagate() {
    // no migrations: every query fails with a db error, not a not-found
    let db = Connection(sea_orm::Database::connect("sqlite::memory:").await.unwrap());
    let mut errors = Vec::new();
    assert!(
      check_owner(&db, Some(Uuid::now_v7()), &mut errors)
        .await
        .is_err()
    );
    assert!(check_owner(&db, Option::None, &mut errors).await.is_ok());
    let err = egg_vars(&db, Uuid::now_v7(), &mut errors)
      .await
      .unwrap_err();
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(errors.is_empty());
  }
}
