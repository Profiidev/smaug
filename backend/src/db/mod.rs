use centaurus::db::init::Connection;

pub mod egg;
pub mod node;

#[allow(unused)]
pub trait DBTrait {
  fn node(&self) -> node::NodeTable<'_>;
  fn egg(&self) -> egg::EggTable<'_>;
}

impl DBTrait for Connection {
  fn node(&self) -> node::NodeTable<'_> {
    node::NodeTable::new(&self.0)
  }

  fn egg(&self) -> egg::EggTable<'_> {
    egg::EggTable::new(&self.0)
  }
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

    // every migration's `down` must work on postgres
    Migrator::down(&conn.0, None).await.unwrap();
    assert!(!pg_table_exists(&conn, "user").await);
    assert!(!pg_table_exists(&conn, "node").await);
    assert!(!pg_table_exists(&conn, "egg").await);
    assert!(!pg_table_exists(&conn, "egg_variable").await);

    Migrator::up(&conn.0, None).await.unwrap();
    assert!(pg_table_exists(&conn, "user").await);
    assert!(pg_table_exists(&conn, "node").await);
    assert!(pg_table_exists(&conn, "egg").await);
    assert!(pg_table_exists(&conn, "egg_variable").await);
  }

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
      .update_egg(changed.clone(), vec![var(id, "ONE", None)])
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
        .update_egg(ghost.clone(), vec![var(ghost.id, "G", None)])
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
}
