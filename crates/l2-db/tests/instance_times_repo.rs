#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `InstanceTimeRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/character_instance_time.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{InstanceTimeRepository, InstanceTimeRow, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 1] = [include_str!("../sql/game/character_instance_time.sql")];

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// The statements of a schema file; whole-line `#` comments are dropped (a file may end with
/// one, which would otherwise be sent as an empty statement).
fn statements(file: &str) -> Vec<String> {
    let text: String = file
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    text.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A throwaway database with the instance time table.
struct TestDb {
    admin: MySqlConnection,
    name: String,
    pool: Pool,
}

impl TestDb {
    /// `None` (with a printed note) when `L2_TEST_DATABASE_URL` is not set.
    async fn create() -> Option<Self> {
        let url = match std::env::var("L2_TEST_DATABASE_URL") {
            Ok(url) if !url.trim().is_empty() => url,
            _ => {
                eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
                return None;
            }
        };
        let admin_options = MySqlConnectOptions::from_str(&url).unwrap();
        let mut admin = MySqlConnection::connect_with(&admin_options).await.unwrap();

        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let name = format!(
            "l2_test_inst_{}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            nanos % 1_000_000_000
        );
        assert!(name.starts_with("l2_test_"));
        // The name is generated above from the pid, a counter and the clock.
        sqlx::query(AssertSqlSafe(format!(
            "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        )))
        .execute(&mut admin)
        .await
        .unwrap();

        let pool = connect_with(admin_options.database(&name), 10).await.unwrap();
        for file in SCHEMA {
            for statement in statements(file) {
                sqlx::query(AssertSqlSafe(statement))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        Some(Self { admin, name, pool })
    }

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
}

const ALICE: i32 = 268_600_001;
const BOB: i32 = 268_600_002;

fn sorted(mut rows: Vec<InstanceTimeRow>) -> Vec<InstanceTimeRow> {
    rows.sort_by_key(|r| r.instance_id);
    rows
}

#[tokio::test]
async fn instance_times_are_set_replaced_read_and_deleted() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = InstanceTimeRepository::new(db.pool.clone());
    let row = |instance_id, time| InstanceTimeRow { instance_id, time };

    repo.set_instance_time(ALICE, 10, 1_000).await.unwrap();
    repo.set_instance_time(ALICE, 11, 2_000).await.unwrap();
    repo.set_instance_time(BOB, 10, 3_000).await.unwrap();
    // `ON DUPLICATE KEY UPDATE time=?`: the same template again replaces the time.
    repo.set_instance_time(ALICE, 10, 5_000).await.unwrap();

    assert_eq!(
        sorted(repo.instance_times_of(ALICE).await.unwrap()),
        [row(10, 5_000), row(11, 2_000)]
    );
    assert_eq!(repo.instance_times_of(BOB).await.unwrap(), [row(10, 3_000)]);

    repo.delete_instance_time(ALICE, 10).await.unwrap();
    assert_eq!(repo.instance_times_of(ALICE).await.unwrap(), [row(11, 2_000)]);
    // Bob's row of the same template stays; deleting a row that is not there is no error.
    assert_eq!(repo.instance_times_of(BOB).await.unwrap(), [row(10, 3_000)]);
    repo.delete_instance_time(ALICE, 99).await.unwrap();

    db.drop_database().await;
}
