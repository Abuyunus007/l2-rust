#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `LotteryRepository` (S-HAND-03) against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/lottery.sql` and `sql/game/items.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{LotteryRepository, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 2] = [l2_db::game_sql!("lottery"), l2_db::game_sql!("items")];

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

/// A throwaway database with the lottery and items tables.
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
            "l2_test_lottery_{}_{}_{}",
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

#[tokio::test]
async fn a_lottery_is_stored_finished_and_read_back() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = LotteryRepository::new(db.pool.clone());
    assert_eq!(repo.last().await.unwrap(), None);

    repo.insert(1, 1_800_000_000_000, 50_000, 50_000).await.unwrap();
    let row = repo.last().await.unwrap().unwrap();
    assert_eq!(
        (row.idnr, row.prize, row.newprize, row.enddate, row.finished),
        (1, 50_000, 50_000, 1_800_000_000_000, 0)
    );
    repo.update_price(1, 52_000).await.unwrap();
    assert_eq!(repo.last().await.unwrap().unwrap().prize, 52_000);

    repo.finish(1, 52_000, 1_000, 31, 1, 30_000, 10_000, 5_000)
        .await
        .unwrap();
    repo.insert(2, 1_800_600_000_000, 1_000, 1_000).await.unwrap();
    let last = repo.last().await.unwrap().unwrap();
    assert_eq!((last.idnr, last.finished), (2, 0));
    let results = repo.results().await.unwrap();
    assert_eq!(results.len(), 2);
    let first = results.iter().find(|r| r.idnr == 1).unwrap();
    assert_eq!(
        (
            first.number1,
            first.number2,
            first.prize1,
            first.prize2,
            first.prize3
        ),
        (31, 1, 30_000, 10_000, 5_000)
    );

    // The tickets of a draw are the lottery ticket items with that custom type 1.
    for (object_id, item_id, custom1, enchant, custom2) in [
        (1, 4442, 1, 31, 1),
        (2, 4442, 1, 3, 0),
        (3, 4442, 2, 5, 0),
        (4, 57, 1, 9, 9),
    ] {
        sqlx::query(
            "INSERT INTO items (owner_id, object_id, item_id, count, enchant_level, loc, loc_data, custom_type1, custom_type2) VALUES (7, ?, ?, 1, ?, 'INVENTORY', 0, ?, ?)",
        )
        .bind(object_id)
        .bind(item_id)
        .bind(enchant)
        .bind(custom1)
        .bind(custom2)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    let mut tickets = repo.tickets(1).await.unwrap();
    tickets.sort_unstable();
    assert_eq!(tickets, vec![(3, 0), (31, 1)]);

    db.drop_database().await;
}
