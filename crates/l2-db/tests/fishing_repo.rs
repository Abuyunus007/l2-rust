#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `FishingChampionshipRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/fishing_championship.sql` and
//! `sql/game/global_variables.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{FishingChampionshipRepository, FishingChampionshipRow, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 2] = [
    include_str!("../sql/game/fishing_championship.sql"),
    include_str!("../sql/game/global_variables.sql"),
];

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

/// A throwaway database with the championship tables.
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
            "l2_test_fishing_{}_{}_{}",
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

/// `restoreData`: an empty table reads as no rows and the missing variable as 0.
#[tokio::test]
async fn an_empty_championship_reads_as_nothing() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = FishingChampionshipRepository::new(db.pool.clone());
    assert!(repo.load_rows().await.unwrap().is_empty());
    assert_eq!(repo.end_date().await.unwrap(), 0);
    db.drop_database().await;
}

/// `shutdown()`: the variable is replaced, the old rows go and every fisher is inserted, the
/// winners first. A second save replaces the first completely.
#[tokio::test]
async fn the_championship_is_saved_whole() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = FishingChampionshipRepository::new(db.pool.clone());
    let row = |name: &str, length, rewarded| FishingChampionshipRow {
        player_name: name.into(),
        fish_length: length,
        rewarded,
    };
    repo.save(1_792_000_000_123, &[row("Alpha", 91.234, 1), row("Bravo", 75.5, 2), row("Charlie", 60.001, 0)])
        .await
        .unwrap();
    assert_eq!(repo.end_date().await.unwrap(), 1_792_000_000_123);
    let mut rows = repo.load_rows().await.unwrap();
    rows.sort_by(|a, b| a.player_name.cmp(&b.player_name));
    assert_eq!(rows, [row("Alpha", 91.234, 1), row("Bravo", 75.5, 2), row("Charlie", 60.001, 0)]);

    repo.save(5, &[row("Delta", 88.0, 0)]).await.unwrap();
    assert_eq!(repo.end_date().await.unwrap(), 5);
    assert_eq!(repo.load_rows().await.unwrap(), [row("Delta", 88.0, 0)]);

    // A variable that is not a number reads as 0 (`getLong(.., 0)`).
    sqlx::query("REPLACE INTO global_variables (var, value) VALUES ('fishChampionshipEnd', 'soon')")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(repo.end_date().await.unwrap(), 0);
    db.drop_database().await;
}
