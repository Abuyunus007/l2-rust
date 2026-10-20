#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of the punishment repository against a real MySQL/MariaDB server.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{NewPunishment, Pool, PunishmentRepository, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: &str = include_str!("../sql/game/punishments.sql");

static COUNTER: AtomicU32 = AtomicU32::new(0);

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
            "l2_test_punish_{}_{}_{}",
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
        sqlx::query(AssertSqlSafe(SCHEMA.trim().trim_end_matches(';').to_owned()))
            .execute(&pool)
            .await
            .unwrap();
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

fn jail(key: &str, expiration: i64) -> NewPunishment {
    NewPunishment {
        key: key.to_owned(),
        affect: "CHARACTER".to_owned(),
        kind: "JAIL".to_owned(),
        expiration,
        reason: "Jailed by admin".to_owned(),
        punished_by: "GM".to_owned(),
    }
}

#[tokio::test]
async fn a_punishment_is_stored_loaded_and_ended() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = PunishmentRepository::new(db.pool.clone());
    assert!(repo.load_all().await.unwrap().is_empty());

    let first = repo.insert(&jail("268500001", 0)).await.unwrap();
    let second = repo.insert(&jail("268500002", 1_700_000_000_000)).await.unwrap();
    assert_eq!(second, first + 1);

    let rows = repo.load_all().await.unwrap();
    assert_eq!(rows.len(), 2);
    let row = rows.iter().find(|r| u64::from(r.id) == first).unwrap();
    assert_eq!(
        (
            row.key.as_str(),
            row.affect.as_str(),
            row.kind.as_str(),
            row.expiration,
            row.reason.as_str(),
            row.punished_by.as_str()
        ),
        ("268500001", "CHARACTER", "JAIL", 0, "Jailed by admin", "GM")
    );

    // Ending a punishment only changes its expiration (to the time it ended).
    repo.update_expiration(u32::try_from(first).unwrap(), 1_700_000_999_000)
        .await
        .unwrap();
    let rows = repo.load_all().await.unwrap();
    let row = rows.iter().find(|r| u64::from(r.id) == first).unwrap();
    assert_eq!(row.expiration, 1_700_000_999_000);
    let other = rows.iter().find(|r| u64::from(r.id) == second).unwrap();
    assert_eq!(other.expiration, 1_700_000_000_000);

    db.drop_database().await;
}
