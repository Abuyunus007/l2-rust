#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Installs both schemas into a throwaway database.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI's MariaDB service, e.g.
//! `mysql://root@127.0.0.1:3306/`). The tests create and drop a database named
//! `l2_test_dbinst_*`; they never touch any other database. Never point the variable at a
//! server with real data.

use std::path::PathBuf;
use std::str::FromStr;

use l2_db_installer::installer::{ErrorChoice, ExistsChoice, InstallError, Level, Params, Ui, run};
use sqlx::mysql::MySqlConnectOptions;
use sqlx::{AssertSqlSafe, ConnectOptions, Connection, MySqlConnection};

fn base_options() -> Option<MySqlConnectOptions> {
    let url = std::env::var("L2_TEST_DATABASE_URL")
        .ok()
        .filter(|u| !u.is_empty());
    let Some(url) = url else {
        eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
        return None;
    };
    Some(MySqlConnectOptions::from_str(&url).unwrap())
}

struct Scripted {
    exists: ExistsChoice,
    asked_exists: usize,
    errors: usize,
    log: Vec<String>,
}

impl Scripted {
    fn new(exists: ExistsChoice) -> Self {
        Self {
            exists,
            asked_exists: 0,
            errors: 0,
            log: Vec::new(),
        }
    }
}

impl Ui for Scripted {
    fn log(&mut self, _level: Level, message: &str) {
        self.log.push(message.to_owned());
    }
    fn progress(&mut self, _done: usize, _total: usize) {}
    fn ask_exists(&mut self, _db_name: &str) -> ExistsChoice {
        self.asked_exists += 1;
        self.exists
    }
    fn ask_error(&mut self, _sql: &str, _error: &str) -> ErrorChoice {
        self.errors += 1;
        ErrorChoice::Abort
    }
}

fn sql_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/db_installer/sql")
}

fn params(options: &MySqlConnectOptions, db: &str, login: bool, game: bool) -> Params {
    Params {
        options: options.clone(),
        db_name: db.to_owned(),
        sql_dir: sql_dir(),
        login,
        game,
    }
}

async fn tables(options: &MySqlConnectOptions, db: &str) -> Vec<String> {
    let mut conn = options.clone().database(db).connect().await.unwrap();
    let names: Vec<String> = sqlx::query_scalar("SHOW TABLES")
        .fetch_all(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
    names
}

async fn exec(options: &MySqlConnectOptions, sql: String) {
    let mut conn: MySqlConnection = options.clone().connect().await.unwrap();
    sqlx::raw_sql(AssertSqlSafe(sql))
        .execute(&mut conn)
        .await
        .unwrap();
    conn.close().await.unwrap();
}

fn unique_name() -> String {
    format!("l2_test_dbinst_{}", std::process::id())
}

#[tokio::test]
async fn installs_both_schemas_then_keeps_then_resets() {
    let Some(options) = base_options() else {
        return;
    };
    let db = unique_name();
    exec(&options, format!("DROP DATABASE IF EXISTS `{db}`")).await;

    // 1. The database does not exist: it is created, nothing is asked.
    let mut ui = Scripted::new(ExistsChoice::Cancel);
    let summary = run(&params(&options, &db, true, true), &mut ui).await.unwrap();
    assert_eq!(ui.asked_exists, 0);
    assert_eq!(ui.errors, 0);
    assert_eq!(summary.statements_failed, 0);
    assert!(summary.files > 100);
    let names = tables(&options, &db).await;
    for expected in ["accounts", "gameservers", "characters", "items", "clan_data"] {
        assert!(names.iter().any(|n| n == expected), "missing table {expected}");
    }

    // 2. Keep: installs into the existing database; a foreign table survives.
    exec(&options, format!("CREATE TABLE `{db}`.`foreign_table` (x INT)")).await;
    let mut ui = Scripted::new(ExistsChoice::Keep);
    run(&params(&options, &db, true, false), &mut ui).await.unwrap();
    assert_eq!(ui.asked_exists, 1);
    assert!(tables(&options, &db).await.iter().any(|n| n == "foreign_table"));

    // 3. Cancel: nothing changes.
    let mut ui = Scripted::new(ExistsChoice::Cancel);
    let result = run(&params(&options, &db, true, true), &mut ui).await;
    assert!(matches!(result, Err(InstallError::Cancelled)));
    assert!(tables(&options, &db).await.iter().any(|n| n == "foreign_table"));

    // 4. Reset: the foreign table is gone, only the login schema is there.
    let mut ui = Scripted::new(ExistsChoice::Reset);
    run(&params(&options, &db, true, false), &mut ui).await.unwrap();
    let names = tables(&options, &db).await;
    assert!(!names.iter().any(|n| n == "foreign_table"));
    assert!(names.iter().any(|n| n == "accounts"));
    assert!(!names.iter().any(|n| n == "characters"));

    exec(&options, format!("DROP DATABASE `{db}`")).await;
}
