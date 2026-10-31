#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `BotReportRepository` against a real MySQL/MariaDB server: the statements
//! of `BotReportTable` (`SELECT * FROM bot_reported_char_data`, the `INSERT` of a report) on the
//! unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{BotReportDbRow, BotReportRepository, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 1] = [l2_db::game_sql!("bot_reported_char_data")];

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
            "l2_test_botreport_{}_{}_{}",
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

const BOT: i32 = 0x1000_0001;
const ALICE: i32 = 0x1000_0002;
const BOB: i32 = 0x1000_0003;
/// A date past 2^32 ms: the column is a `BIGINT`.
const DATE: i64 = 1_700_000_000_000;

#[tokio::test]
async fn reports_are_stored_and_read_back() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = BotReportRepository::new(db.pool.clone());
    assert!(repo.load_all().await.unwrap().is_empty());

    // `INSERT INTO bot_reported_char_data VALUES (?,?,?)`.
    repo.insert_report(BOT, ALICE, DATE).await.unwrap();
    repo.insert_report(BOT, BOB, DATE + 1).await.unwrap();
    let mut rows = repo.load_all().await.unwrap();
    rows.sort_by_key(|r| r.reporter_id);
    assert_eq!(
        rows,
        vec![
            BotReportDbRow {
                bot_id: BOT as u32,
                reporter_id: ALICE as u32,
                report_date: DATE as u64,
            },
            BotReportDbRow {
                bot_id: BOT as u32,
                reporter_id: BOB as u32,
                report_date: DATE as u64 + 1,
            },
        ]
    );

    // The same pair again (the Java game server wrote it meanwhile): the date is replaced, no
    // second row, no error.
    repo.insert_report(BOT, ALICE, DATE + 5).await.unwrap();
    let rows = repo.load_all().await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .find(|r| r.reporter_id == ALICE as u32)
            .unwrap()
            .report_date,
        DATE as u64 + 5
    );
    db.drop_database().await;
}
