#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `WeddingRepository` against a real MySQL/MariaDB server: Java's
//! `mods_wedding` statements (`CoupleManager.load`, the `Couple` constructors, `marry`,
//! `divorce`) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{CoupleRow, Pool, WeddingRepository, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

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
            "l2_test_wedding_{}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            nanos % 1_000_000_000
        );
        assert!(name.starts_with("l2_test_"));
        sqlx::query(AssertSqlSafe(format!(
            "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        )))
        .execute(&mut admin)
        .await
        .unwrap();
        let pool = connect_with(admin_options.database(&name), 10).await.unwrap();
        for file in l2_db::GAME_SCHEMA {
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

fn couple(id: i32, a: i32, b: i32) -> CoupleRow {
    CoupleRow {
        id,
        player1_id: a,
        player2_id: b,
        married: false,
        affiance_date: 1_790_000_000_123,
        wedding_date: 1_790_000_000_123,
    }
}

/// An empty table reads as no couples.
#[tokio::test]
async fn an_empty_table_has_no_couples() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = WeddingRepository::new(db.pool.clone());
    assert!(repo.load_all().await.unwrap().is_empty());
    db.drop_database().await;
}

/// The life of a couple: engaged (insert), married (update), divorced (delete). The dates are
/// `DECIMAL(20,0)` columns that Java reads with `getLong`.
#[tokio::test]
async fn a_couple_is_engaged_married_and_divorced() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = WeddingRepository::new(db.pool.clone());
    repo.insert(&couple(0x6000_0001, 11, 12)).await.unwrap();
    repo.insert(&couple(0x6000_0002, 21, 22)).await.unwrap();
    let all = repo.load_all().await.unwrap();
    assert_eq!(all, [couple(0x6000_0001, 11, 12), couple(0x6000_0002, 21, 22)]);

    repo.marry(0x6000_0001, 1_790_000_999_000).await.unwrap();
    let all = repo.load_all().await.unwrap();
    assert_eq!(
        all[0],
        CoupleRow {
            married: true,
            wedding_date: 1_790_000_999_000,
            ..couple(0x6000_0001, 11, 12)
        }
    );
    assert!(!all[1].married);

    repo.delete(0x6000_0001).await.unwrap();
    assert_eq!(repo.load_all().await.unwrap(), [couple(0x6000_0002, 21, 22)]);
    db.drop_database().await;
}

/// `Boolean.parseBoolean(rs.getString("married"))`: a row Java wrote with `1` reads as not
/// married, a NULL too.
#[tokio::test]
async fn married_reads_like_parse_boolean() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    for (id, married) in [(1, "'1'"), (2, "'true'"), (3, "'false'"), (4, "NULL")] {
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO mods_wedding (id, player1Id, player2Id, married, affianceDate, weddingDate) VALUES ({id}, 1, 2, {married}, 5, 6)"
        )))
        .execute(&db.pool)
        .await
        .unwrap();
    }
    let repo = WeddingRepository::new(db.pool.clone());
    let married: Vec<bool> = repo.load_all().await.unwrap().iter().map(|c| c.married).collect();
    assert_eq!(married, [false, true, false, false]);
    db.drop_database().await;
}
