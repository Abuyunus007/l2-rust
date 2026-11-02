#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `RebirthRepository` against a real MySQL/MariaDB server (M9-C05).
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/rebirth_system.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{Pool, RebirthRepository, RebirthRow, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 1] = [l2_db::game_sql!("rebirth_system")];

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// The statements of a schema file; whole-line `#` comments are dropped.
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

/// A throwaway database with the rebirth table.
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
            "l2_test_rebirth_{}_{}_{}",
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
async fn the_first_rebirth_inserts_and_later_ones_update() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = RebirthRepository::new(db.pool.clone());

    assert_eq!(repo.rebirth_of(ALICE).await.unwrap(), None);
    assert!(repo.insert_first(ALICE).await.unwrap());
    assert_eq!(
        repo.rebirth_of(ALICE).await.unwrap(),
        Some(RebirthRow {
            rebirth_count: 1,
            selected_skills: String::new()
        })
    );
    // The key is the character: a second first rebirth is refused (Java logs it and aborts).
    assert!(repo.insert_first(ALICE).await.is_err());

    assert!(repo.update_count(ALICE, 2).await.unwrap());
    repo.save_selected_skills(ALICE, "1059,1204").await.unwrap();
    repo.insert_first(BOB).await.unwrap();
    assert_eq!(
        repo.rebirth_of(ALICE).await.unwrap(),
        Some(RebirthRow {
            rebirth_count: 2,
            selected_skills: "1059,1204".to_owned()
        })
    );
    // The other character's row stays, and a character without a row changes nothing.
    assert_eq!(repo.rebirth_of(BOB).await.unwrap().unwrap().rebirth_count, 1);
    assert!(!repo.update_count(BOB + 1, 5).await.unwrap());

    db.drop_database().await;
}
