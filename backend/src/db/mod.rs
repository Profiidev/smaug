use centaurus::db::init::Connection;

pub mod backup;
pub mod egg;
pub mod node;
pub mod schedule;
pub mod server;

#[allow(unused)]
pub trait DBTrait {
  fn node(&self) -> node::NodeTable<'_>;
  fn egg(&self) -> egg::EggTable<'_>;
  fn server(&self) -> server::ServerTable<'_>;
  fn backup(&self) -> backup::BackupTable<'_>;
  fn schedule(&self) -> schedule::ScheduleTable<'_>;
}

impl DBTrait for Connection {
  fn node(&self) -> node::NodeTable<'_> {
    node::NodeTable::new(&self.0)
  }

  fn egg(&self) -> egg::EggTable<'_> {
    egg::EggTable::new(&self.0)
  }

  fn server(&self) -> server::ServerTable<'_> {
    server::ServerTable::new(&self.0)
  }

  fn backup(&self) -> backup::BackupTable<'_> {
    backup::BackupTable::new(&self.0)
  }

  fn schedule(&self) -> schedule::ScheduleTable<'_> {
    schedule::ScheduleTable::new(&self.0)
  }
}

/// In-memory sqlite with every migration, foreign keys off so rows can be
/// inserted without their parents.
#[cfg(test)]
pub(crate) async fn sqlite_db() -> Connection {
  use migration::{Migrator, MigratorTrait};
  use sea_orm::{ConnectOptions, ConnectionTrait, Database};

  let mut opts = ConnectOptions::new("sqlite::memory:");
  opts.max_connections(1);
  let conn = Database::connect(opts).await.unwrap();
  Migrator::up(&conn, None).await.unwrap();
  conn
    .execute_unprepared("PRAGMA foreign_keys = OFF")
    .await
    .unwrap();
  Connection(conn)
}

#[cfg(test)]
mod test {
  use centaurus::db::init::Connection;
  use migration::{Migrator, MigratorTrait};
  use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement, prelude::Uuid};
  use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
  };

  /// Starts a throwaway Postgres container (same major version as production)
  /// and runs all migrations on it.
  ///
  /// The container must be kept alive for the duration of the test; dropping it
  /// stops and removes postgres.
  async fn test_pg_db() -> (Connection, ContainerAsync<Postgres>) {
    let container = Postgres::default()
      .with_tag("18-alpine")
      .start()
      .await
      .expect("Failed to start postgres container");
    let url = format!(
      "postgres://postgres:postgres@{}:{}/postgres",
      container.get_host().await.unwrap(),
      container.get_host_port_ipv4(5432).await.unwrap()
    );

    let conn = Database::connect(url)
      .await
      .expect("Failed to connect to postgres");

    Migrator::up(&conn, None)
      .await
      .expect("Failed to run database migrations");

    (Connection(conn), container)
  }

  async fn pg_table_exists(conn: &Connection, name: &str) -> bool {
    conn
      .query_one_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = $1",
        [name.into()],
      ))
      .await
      .unwrap()
      .is_some()
  }

  #[tokio::test]
  async fn pg_migrations_down_then_up() {
    let (conn, _pg) = test_pg_db().await;
    assert!(pg_table_exists(&conn, "user").await);
    assert!(pg_table_exists(&conn, "node").await);
    assert!(pg_table_exists(&conn, "egg").await);
    assert!(pg_table_exists(&conn, "egg_variable").await);
    for t in SERVER_TABLES {
      assert!(pg_table_exists(&conn, t).await, "{t}");
    }

    // every migration's `down` must work on postgres
    Migrator::down(&conn.0, None).await.unwrap();
    assert!(!pg_table_exists(&conn, "user").await);
    assert!(!pg_table_exists(&conn, "node").await);
    assert!(!pg_table_exists(&conn, "egg").await);
    assert!(!pg_table_exists(&conn, "egg_variable").await);
    for t in SERVER_TABLES {
      assert!(!pg_table_exists(&conn, t).await, "{t}");
    }

    Migrator::up(&conn.0, None).await.unwrap();
    assert!(pg_table_exists(&conn, "user").await);
    assert!(pg_table_exists(&conn, "node").await);
    assert!(pg_table_exists(&conn, "egg").await);
    assert!(pg_table_exists(&conn, "egg_variable").await);
    for t in SERVER_TABLES {
      assert!(pg_table_exists(&conn, t).await, "{t}");
    }
  }

  const SERVER_TABLES: [&str; 8] = [
    "server_schedule_task",
    "server_schedule",
    "server_backup",
    "server",
    "server_allocation",
    "server_variable",
    "server_user_access",
    "server_group_access",
  ];

  fn egg_model(name: &str) -> entity::egg::Model {
    use serde_json::json;
    entity::egg::Model {
      id: Uuid::now_v7(),
      name: name.into(),
      author: "a@b.c".into(),
      description: None,
      docker_images: json!([["a", "b"]]),
      file_denylist: json!([]),
      features: None,
      startup_commands: json!([["Default", "x"]]),
      config: json!({}),
      install_script: json!({}),
      update_url: None,
      icon: None,
      tags: json!([]),
    }
  }

  fn var(egg_id: Uuid, env: &str, sort: Option<i16>) -> entity::egg_variable::Model {
    entity::egg_variable::Model {
      id: Uuid::now_v7(),
      egg_id,
      name: env.into(),
      description: String::new(),
      env_variable: env.into(),
      default_value: String::new(),
      user_viewable: true,
      user_editable: true,
      sort,
      rules: serde_json::json!([]),
    }
  }

  async fn var_count(conn: &Connection) -> u64 {
    use sea_orm::{EntityTrait, PaginatorTrait};
    entity::egg_variable::Entity::find()
      .count(&conn.0)
      .await
      .unwrap()
  }

  fn envs(vars: &[entity::egg_variable::Model]) -> Vec<&str> {
    vars.iter().map(|v| v.env_variable.as_str()).collect()
  }

  #[tokio::test]
  async fn pg_egg_table() {
    use crate::db::DBTrait;

    let (conn, _pg) = test_pg_db().await;
    let eggs = conn.egg();
    assert!(eggs.list_eggs().await.unwrap().is_empty());

    // create: variables sorted by sort asc, nulls last, ties by insertion
    let b = egg_model("b");
    let id = b.id;
    let vars = vec![
      var(id, "TWO", Some(2)),
      var(id, "NULL_1", None),
      var(id, "ONE", Some(1)),
      var(id, "NULL_2", None),
      var(id, "NEG", Some(-1)),
    ];
    eggs.create_egg(b.clone(), vars).await.unwrap();
    let (found, vars) = eggs.find_by_id(id).await.unwrap();
    assert_eq!(found, b);
    assert_eq!(envs(&vars), ["NEG", "ONE", "TWO", "NULL_1", "NULL_2"]);

    // create without variables, list is ordered by name
    let a = egg_model("a");
    eggs.create_egg(a.clone(), vec![]).await.unwrap();
    let names: Vec<_> = eggs
      .list_eggs()
      .await
      .unwrap()
      .into_iter()
      .map(|e| e.name)
      .collect();
    assert_eq!(names, ["a", "b"]);
    assert!(eggs.find_by_id(a.id).await.unwrap().1.is_empty());

    // a failing variable insert rolls back the egg too
    let dup = egg_model("dup");
    let err = eggs
      .create_egg(
        dup.clone(),
        vec![var(dup.id, "X", None), var(dup.id, "X", None)],
      )
      .await;
    assert!(err.is_err());
    assert_eq!(
      eggs.find_by_id(dup.id).await.unwrap_err().status,
      http::StatusCode::NOT_FOUND
    );

    // update replaces columns and all variables (same env name reused)
    let mut changed = egg_model("b2");
    changed.id = id;
    changed.description = Some("new".into());
    eggs
      .update_egg(changed.clone(), vec![var(id, "ONE", None)], "x")
      .await
      .unwrap();
    let (found, vars) = eggs.find_by_id(id).await.unwrap();
    assert_eq!(found, changed);
    assert_eq!(envs(&vars), ["ONE"]);
    assert_eq!(var_count(&conn).await, 1);

    // update of a missing egg fails and leaves nothing behind
    let ghost = egg_model("ghost");
    assert!(
      eggs
        .update_egg(ghost.clone(), vec![var(ghost.id, "G", None)], "x")
        .await
        .is_err()
    );
    assert_eq!(var_count(&conn).await, 1);

    // delete: missing is 0 rows, real one cascades its variables
    assert_eq!(eggs.delete_egg(ghost.id).await.unwrap(), 0);
    assert_eq!(eggs.delete_egg(id).await.unwrap(), 1);
    assert_eq!(var_count(&conn).await, 0);
    assert_eq!(
      eggs.find_by_id(id).await.unwrap_err().status,
      http::StatusCode::NOT_FOUND
    );
    assert_eq!(eggs.list_eggs().await.unwrap(), [a]);
  }

  fn node(name: &str) -> crate::db::node::Node {
    crate::db::node::Node {
      id: Uuid::now_v7(),
      name: name.into(),
      address: "127.0.0.1".into(),
      port: 8080,
      secure: false,
      disk_limit_mb: Some(1.5),
      memory_limit_mb: None,
      cpu_limit: Some(2),
      token: "t".into(),
    }
  }

  #[tokio::test]
  async fn pg_node_table() {
    use crate::db::DBTrait;
    use sea_orm::{DbErr, IntoActiveModel, Set};

    let (conn, _pg) = test_pg_db().await;
    let nodes = conn.node();
    assert!(nodes.list_nodes().await.unwrap().is_empty());

    let a = node("a");
    nodes.create_node(a.clone()).await.unwrap();
    let b = node("b");
    let b_id = b.id;
    nodes.create_node(b.clone()).await.unwrap();

    let found = nodes.find_by_name("a".into()).await.unwrap();
    assert_eq!(found, entity::node::Model::from(a.clone()));
    assert!(matches!(
      nodes.find_by_name("missing".into()).await,
      Err(DbErr::RecordNotFound(_))
    ));
    assert_eq!(
      nodes.find_by_id(b.id).await.unwrap(),
      entity::node::Model::from(b)
    );
    assert_eq!(
      nodes.find_by_id(Uuid::now_v7()).await.unwrap_err().status,
      http::StatusCode::NOT_FOUND
    );

    let mut ids: Vec<_> = nodes
      .list_nodes()
      .await
      .unwrap()
      .into_iter()
      .map(|n| n.id)
      .collect();
    ids.sort();
    assert_eq!(ids, [a.id, b_id]);

    let mut model = found.into_active_model();
    model.name = Set("a2".into());
    model.port = Set(9000);
    model.disk_limit_mb = Set(None);
    nodes.update_node(model).await.unwrap();
    let updated = nodes.find_by_id(a.id).await.unwrap();
    assert_eq!(
      (updated.name.as_str(), updated.port, updated.disk_limit_mb),
      ("a2", 9000, None)
    );
    assert_eq!(updated.token, "t");

    // delete reports the rows affected
    assert_eq!(nodes.delete_node(Uuid::now_v7()).await.unwrap(), 0);
    assert_eq!(nodes.delete_node(a.id).await.unwrap(), 1);
    assert_eq!(
      nodes.find_by_id(a.id).await.unwrap_err().status,
      http::StatusCode::NOT_FOUND
    );
    assert_eq!(nodes.list_nodes().await.unwrap().len(), 1);
  }

  fn server_model(node: Uuid, egg: Uuid, owner: Option<Uuid>) -> entity::server::Model {
    entity::server::Model {
      id: Uuid::now_v7(),
      name: "s".into(),
      owner_id: owner,
      node_id: node,
      egg_id: egg,
      startup: "old".into(),
      startup_overridden: false,
      image: "img".into(),
      install_state: entity::sea_orm_active_enums::ServerInstallState::InstallFailed,
      install_generation: 0,
      suspended: false,
      start_on_completion: true,
      cpu_limit: 0,
      memory_mb: 5_000_000_000,
      disk_mb: 0,
      backup_limit: 0,
      stop_timeout: 60,
      restart_on_crash: true,
      desired_running: false,
      power_generation: 0,
      // postgres keeps microseconds
      created_at: chrono::SubsecRound::trunc_subsecs(chrono::Utc::now(), 6).fixed_offset(),
    }
  }

  fn pair(ip: &str, port: i32) -> (String, i32) {
    (ip.into(), port)
  }

  #[tokio::test]
  async fn pg_server_table() {
    use std::collections::HashMap;

    use centaurus::db::tables::ConnectionExt;
    use entity::sea_orm_active_enums::{ServerAccessLevel, ServerInstallState};

    use crate::db::{DBTrait, server::Grants};

    let (conn, _pg) = test_pg_db().await;
    let node_a = node("a");
    let node_b = node("b");
    conn.node().create_node(node_a.clone()).await.unwrap();
    conn.node().create_node(node_b.clone()).await.unwrap();
    let egg = egg_model("egg");
    let egg_id = egg.id;
    conn
      .egg()
      .create_egg(
        egg,
        vec![var(egg_id, "KEEP", None), var(egg_id, "DROP", None)],
      )
      .await
      .unwrap();
    let user = conn
      .user()
      .create_user(
        "u".into(),
        "u@x.com".into(),
        "h".into(),
        "s".into(),
        false,
        None,
      )
      .await
      .unwrap();
    let group = conn.group().create_group("g".into()).await.unwrap();
    conn
      .group()
      .add_users_to_group(group, vec![user])
      .await
      .unwrap();

    let servers = conn.server();
    let s = server_model(node_a.id, egg_id, Some(user));
    let id = s.id;
    servers
      .create(
        s.clone(),
        vec![pair("10.0.0.1", 2000), pair("10.0.0.1", 2001)],
        vec![("KEEP".into(), "k".into()), ("DROP".into(), "d".into())],
      )
      .await
      .unwrap();
    // enum and big integer columns round trip
    assert_eq!(
      servers.find_by_id(id).await.unwrap().install_state,
      s.install_state
    );
    assert_eq!(servers.find_by_id(id).await.unwrap(), s);
    assert_eq!(
      servers.find_by_id(Uuid::now_v7()).await.unwrap_err().status,
      http::StatusCode::NOT_FOUND
    );
    let allocs = servers.allocations(vec![id]).await.unwrap();
    assert_eq!(
      allocs
        .iter()
        .map(|a| (a.position, a.port, a.node_id))
        .collect::<Vec<_>>(),
      [(0, 2000, node_a.id), (1, 2001, node_a.id)]
    );

    // a second server can not reuse a pair on the same node (unique index)
    let other = server_model(node_a.id, egg_id, None);
    assert!(
      servers
        .create(other.clone(), vec![pair("10.0.0.1", 2001)], vec![])
        .await
        .is_err()
    );
    // the failed create rolled back
    assert_eq!(servers.list(None).await.unwrap().len(), 1);
    servers
      .create(other.clone(), vec![pair("10.0.0.2", 2001)], vec![])
      .await
      .unwrap();

    // taken: same node and port only, own server excluded, empty input is a no-op
    let ports = [2001, 1];
    let mut taken = servers.taken(node_a.id, &ports, None).await.unwrap();
    taken.sort();
    assert_eq!(taken, [pair("10.0.0.1", 2001), pair("10.0.0.2", 2001)]);
    assert_eq!(
      servers.taken(node_a.id, &ports, Some(id)).await.unwrap(),
      [pair("10.0.0.2", 2001)]
    );
    assert!(
      servers
        .taken(node_b.id, &ports, None)
        .await
        .unwrap()
        .is_empty()
    );
    assert!(
      servers
        .taken(node_a.id, &[], None)
        .await
        .unwrap()
        .is_empty()
    );

    // list: ordered by name, filtered by id
    let mut renamed = other.clone();
    renamed.name = "a".into();
    servers
      .update(renamed, vec![pair("10.0.0.2", 2001)], vec![], None)
      .await
      .unwrap();
    let ids: Vec<_> = servers
      .list(None)
      .await
      .unwrap()
      .into_iter()
      .map(|s| s.id)
      .collect();
    assert_eq!(ids, [other.id, id]);
    assert_eq!(servers.list(Some(vec![id])).await.unwrap().len(), 1);
    assert!(servers.list(Some(vec![])).await.unwrap().is_empty());
    assert_eq!(servers.owned_ids(user).await.unwrap(), [id]);
    assert_eq!(servers.count_by_node(node_a.id).await.unwrap(), 2);
    assert_eq!(servers.count_by_node(node_b.id).await.unwrap(), 0);
    assert_eq!(servers.count_by_egg(egg_id).await.unwrap(), 2);

    // update replaces allocations, variables and (when given) grants
    let mut changed = s.clone();
    changed.name = "z".into();
    // not a spec column, the update leaves it alone
    changed.install_state = ServerInstallState::Installed;
    servers
      .update(
        changed.clone(),
        vec![pair("10.0.0.9", 3000)],
        vec![("KEEP".into(), "k2".into())],
        Some(Grants {
          users: vec![entity::server_user_access::Model {
            server_id: id,
            user_id: user,
            access: ServerAccessLevel::Read,
          }],
          groups: vec![entity::server_group_access::Model {
            server_id: id,
            group_id: group,
            access: ServerAccessLevel::Write,
          }],
        }),
      )
      .await
      .unwrap();
    assert_eq!(
      servers.find_by_id(id).await.unwrap(),
      entity::server::Model {
        install_state: s.install_state,
        ..changed.clone()
      }
    );
    assert_eq!(servers.allocations(vec![id]).await.unwrap().len(), 1);
    assert_eq!(
      servers.variables(id).await.unwrap(),
      HashMap::from([("KEEP".to_string(), "k2".to_string())])
    );
    let mut levels = servers.grants_for_user(user, Some(id)).await.unwrap();
    levels.sort_by_key(|(_, l)| *l == ServerAccessLevel::Write);
    assert_eq!(
      levels,
      [
        (id, ServerAccessLevel::Read),
        (id, ServerAccessLevel::Write)
      ]
    );
    assert!(
      servers
        .grants_for_user(user, Some(other.id))
        .await
        .unwrap()
        .is_empty()
    );
    assert_eq!(servers.grants_for_user(user, None).await.unwrap().len(), 2);
    // without grants they are kept
    servers
      .update(changed.clone(), vec![pair("10.0.0.9", 3000)], vec![], None)
      .await
      .unwrap();
    let grants = servers.grants(id).await.unwrap();
    assert_eq!((grants.users.len(), grants.groups.len()), (1, 1));
    // a failing update rolls back
    assert!(
      servers
        .update(changed.clone(), vec![pair("10.0.0.2", 2001)], vec![], None)
        .await
        .is_err()
    );
    assert_eq!(servers.allocations(vec![id]).await.unwrap()[0].port, 3000);

    // egg sync: startup for non-overridden servers, variables pruned and added
    servers
      .update(
        changed.clone(),
        vec![pair("10.0.0.9", 3000)],
        vec![("KEEP".into(), "k".into()), ("DROP".into(), "d".into())],
        None,
      )
      .await
      .unwrap();
    let mut pinned = other.clone();
    pinned.name = "a".into();
    pinned.startup = "mine".into();
    pinned.startup_overridden = true;
    servers
      .update(pinned, vec![pair("10.0.0.2", 2001)], vec![], None)
      .await
      .unwrap();
    let mut new_egg = egg_model("egg");
    new_egg.id = egg_id;
    let mut fresh = var(egg_id, "NEW", None);
    fresh.default_value = "n".into();
    let mut ids = conn
      .egg()
      .update_egg(new_egg, vec![var(egg_id, "KEEP", None), fresh], "new start")
      .await
      .unwrap();
    ids.sort();
    let mut expected = vec![id, other.id];
    expected.sort();
    assert_eq!(ids, expected);
    assert_eq!(servers.find_by_id(id).await.unwrap().startup, "new start");
    assert_eq!(servers.find_by_id(other.id).await.unwrap().startup, "mine");
    assert_eq!(
      servers.variables(id).await.unwrap(),
      HashMap::from([
        ("KEEP".to_string(), "k".to_string()),
        ("NEW".to_string(), "n".to_string())
      ])
    );
    assert_eq!(
      servers.variables(other.id).await.unwrap(),
      HashMap::from([
        ("KEEP".to_string(), String::new()),
        ("NEW".to_string(), "n".to_string())
      ])
    );
    // an egg without servers syncs nothing
    let lonely = egg_model("lonely");
    conn.egg().create_egg(lonely.clone(), vec![]).await.unwrap();
    assert!(
      conn
        .egg()
        .update_egg(lonely, vec![], "x")
        .await
        .unwrap()
        .is_empty()
    );

    // transfer moves node and keeps only the new allocation
    let mut moved = servers.find_by_id(id).await.unwrap();
    moved.node_id = node_b.id;
    servers
      .transfer(moved.clone(), pair("10.0.0.1", 2001))
      .await
      .unwrap();
    assert_eq!(servers.find_by_id(id).await.unwrap().node_id, node_b.id);
    let allocs = servers.allocations(vec![id]).await.unwrap();
    assert_eq!(
      allocs
        .iter()
        .map(|a| (a.node_id, a.port, a.position))
        .collect::<Vec<_>>(),
      [(node_b.id, 2001, 0)]
    );

    // nodes and eggs in use are protected by the FK
    assert!(conn.node().delete_node(node_b.id).await.is_err());
    assert!(conn.egg().delete_egg(egg_id).await.is_err());

    // deleting the owner nulls it, deleting the group drops its grants
    conn.user().delete_user(user).await.unwrap();
    assert_eq!(servers.find_by_id(id).await.unwrap().owner_id, None);
    let grants = servers.grants(id).await.unwrap();
    assert_eq!((grants.users.len(), grants.groups.len()), (0, 1));
    conn.group().delete_group(group).await.unwrap();
    assert!(servers.grants(id).await.unwrap().groups.is_empty());

    // delete cascades and reports rows
    assert_eq!(servers.delete(Uuid::now_v7()).await.unwrap(), 0);
    assert_eq!(servers.delete(id).await.unwrap(), 1);
    assert!(servers.allocations(vec![id]).await.unwrap().is_empty());
    assert!(servers.variables(id).await.unwrap().is_empty());
    assert_eq!(conn.node().delete_node(node_b.id).await.unwrap(), 1);
  }
}
