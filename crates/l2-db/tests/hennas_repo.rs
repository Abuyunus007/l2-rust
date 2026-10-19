#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `HennaRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/character_hennas.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{HennaRepository, HennaRow, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 1] = [include_str!("../sql/game/character_hennas.sql")];

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

/// A throwaway database with the henna table.
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
            "l2_test_henna_{}_{}_{}",
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

const ALICE: i32 = 268_500_001;
const BOB: i32 = 268_500_002;

#[tokio::test]
async fn hennas_are_read_per_character_and_class_index() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    // Alice: slots 1 and 2 on class index 0, a NULL symbol in slot 3, slot 1 on a subclass.
    // Bob has a dye of his own.
    sqlx::query(
        "INSERT INTO character_hennas (charId, symbol_id, slot, class_index) VALUES \
         (?, 1, 1, 0), (?, 2, 2, 0), (?, NULL, 3, 0), (?, 9, 1, 1), (?, 5, 1, 0)",
    )
    .bind(ALICE)
    .bind(ALICE)
    .bind(ALICE)
    .bind(ALICE)
    .bind(BOB)
    .execute(&db.pool)
    .await
    .unwrap();

    let repo = HennaRepository::new(db.pool.clone());
    let row = |slot, symbol_id| HennaRow { slot, symbol_id };

    let mut base = repo.hennas_of(ALICE, 0).await.unwrap();
    base.sort_by_key(|r| r.slot);
    assert_eq!(
        base,
        [row(1, Some(1)), row(2, Some(2)), row(3, None)],
        "the NULL symbol comes back as None; the loader reads it as 0 like Java"
    );
    assert_eq!(repo.hennas_of(ALICE, 1).await.unwrap(), [row(1, Some(9))]);
    assert_eq!(repo.hennas_of(BOB, 0).await.unwrap(), [row(1, Some(5))]);
    assert!(repo.hennas_of(BOB, 1).await.unwrap().is_empty());
    assert!(repo.hennas_of(268_500_999, 0).await.unwrap().is_empty());

    db.drop_database().await;
}
