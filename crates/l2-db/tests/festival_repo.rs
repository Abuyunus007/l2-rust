#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of the Festival of Darkness rows (`seven_signs_festival`, Java's
//! `restoreFestivalData` / `saveFestivalData`) and of `DimensionalRiftRepository` (the rooms of
//! `DimensionalRiftManager.loadRooms`) against a real MySQL/MariaDB server, M8-FEST-01.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{DimensionalRiftRepository, FestivalRow, Pool, SevenSignsRepository, connect_with};
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
            "l2_test_festival_{}_{}_{}",
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

fn row(festival_id: i32, cabal: &str, cycle: i32, score: i32, members: &str) -> FestivalRow {
    FestivalRow {
        festival_id,
        cabal: cabal.into(),
        cycle,
        date: if score > 0 { 1_790_000_000_123 } else { 0 },
        score,
        members: members.into(),
    }
}

/// `REPLACE INTO` writes a new row and overwrites the one with the same key
/// (`festivalId`, `cabal`, `cycle`); `date` is a 13 digit millisecond number.
#[tokio::test]
async fn festival_rows_are_replaced_and_read() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SevenSignsRepository::new(db.pool.clone());
    assert!(repo.load_festival().await.unwrap().is_empty());
    repo.replace_festival(&[
        row(0, "dawn", 4, 0, ""),
        row(0, "dusk", 4, 0, ""),
        row(1, "dawn", 4, 0, ""),
    ])
    .await
    .unwrap();
    assert_eq!(repo.load_festival().await.unwrap().len(), 3);
    // A better score for the same key replaces the row; the other cycle makes a row of its own.
    repo.replace_festival(&[
        row(0, "dawn", 4, 120, "Alpha,Bravo,Charlie,Delta,Echo"),
        row(0, "dawn", 5, 15, "Zed"),
    ])
    .await
    .unwrap();
    let mut rows = repo.load_festival().await.unwrap();
    rows.sort_by_key(|r| (r.cycle, r.cabal.clone(), r.festival_id));
    assert_eq!(rows.len(), 4);
    let best = rows
        .iter()
        .find(|r| r.festival_id == 0 && r.cabal == "dawn" && r.cycle == 4)
        .unwrap();
    assert_eq!(best.score, 120);
    assert_eq!(best.members, "Alpha,Bravo,Charlie,Delta,Echo");
    assert_eq!(best.date, 1_790_000_000_123);
    db.drop_database().await;
}

/// The rooms of the rift: `SELECT *` into the fields Java reads by column name.
#[tokio::test]
async fn rift_rooms_are_read() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    for statement in [
        "INSERT INTO dimensional_rift VALUES (0, 0, -115440, -114160, -182160, -178048, -6816, -6240, -114790, -180576, -6752, 0)",
        "INSERT INTO dimensional_rift VALUES (1, 9, -110688, -109136, -181968, -181072, -6816, -6240, -109912, -181520, -6752, 1)",
    ] {
        sqlx::query(statement).execute(&db.pool).await.unwrap();
    }
    let rooms = DimensionalRiftRepository::new(db.pool.clone())
        .load_all()
        .await
        .unwrap();
    assert_eq!(rooms.len(), 2);
    let waiting = rooms.iter().find(|r| r.kind == 0 && r.room_id == 0).unwrap();
    assert_eq!((waiting.x_min, waiting.x_max, waiting.z_t), (-115_440, -114_160, -6_752));
    assert_eq!(waiting.boss, 0);
    let boss = rooms.iter().find(|r| r.kind == 1 && r.room_id == 9).unwrap();
    assert_eq!((boss.x_t, boss.y_t), (-109_912, -181_520));
    assert_eq!(boss.boss, 1);
    db.drop_database().await;
}
