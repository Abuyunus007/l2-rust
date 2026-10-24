#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `ManorRepository` against a real MySQL/MariaDB server: Java's manor
//! statements (`CastleManorManager.loadDb`, `storeMe`, `setNextSeedProduction`, `setNextCropProcure`,
//! `updateCurrentProduction`, `updateCurrentProcure`, `resetManorData`) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{ManorRepository, Pool, ProcureRow, ProductionRow, connect_with};
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
            "l2_test_manor_{}_{}_{}",
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

    async fn scalar(&self, sql: &str) -> i64 {
        sqlx::query_scalar(AssertSqlSafe(format!("SELECT CAST(({sql}) AS SIGNED)")))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
}

fn production(castle_id: i32, seed_id: i32, amount: i64, next: bool) -> ProductionRow {
    ProductionRow {
        castle_id,
        seed_id,
        amount,
        start_amount: 100,
        price: 7,
        next_period: next,
    }
}

fn procure(castle_id: i32, crop_id: i32, amount: i64, next: bool) -> ProcureRow {
    ProcureRow {
        castle_id,
        crop_id,
        amount,
        start_amount: 50,
        price: 9,
        reward_type: 2,
        next_period: next,
    }
}

#[tokio::test]
async fn manor_rows_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = ManorRepository::new(db.pool.clone());

    // storeMe: both tables are written whole, current and next period.
    repo.store_all(
        &[
            production(1, 5016, 40, false),
            production(1, 5016, 100, true),
            production(2, 5017, 10, false),
        ],
        &[procure(1, 5073, 20, false), procure(1, 5073, 50, true)],
    )
    .await
    .unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM castle_manor_production").await, 3);
    assert_eq!(db.scalar("SELECT COUNT(*) FROM castle_manor_procure").await, 2);

    // loadDb: the castle's rows of both periods.
    let rows = repo.production(1).await.unwrap();
    assert_eq!(rows.len(), 2);
    let current = rows.iter().find(|r| !r.next_period).unwrap();
    assert_eq!(
        (
            current.seed_id,
            current.amount,
            current.start_amount,
            current.price
        ),
        (5016, 40, 100, 7)
    );
    let rows = repo.procure(1).await.unwrap();
    let next = rows.iter().find(|r| r.next_period).unwrap();
    assert_eq!(
        (
            next.crop_id,
            next.amount,
            next.start_amount,
            next.price,
            next.reward_type
        ),
        (5073, 50, 50, 9, 2)
    );
    assert!(repo.production(3).await.unwrap().is_empty());

    // storeMe again: the old rows are gone first.
    repo.store_all(&[production(2, 5017, 5, false)], &[])
        .await
        .unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM castle_manor_production").await, 1);
    assert_eq!(db.scalar("SELECT COUNT(*) FROM castle_manor_procure").await, 0);

    // setNextSeedProduction / setNextCropProcure: only the castle's next-period rows are replaced.
    repo.store_all(
        &[
            production(1, 5016, 40, false),
            production(1, 5016, 100, true),
            production(2, 5017, 10, true),
        ],
        &[procure(1, 5073, 20, false), procure(1, 5073, 50, true)],
    )
    .await
    .unwrap();
    repo.replace_next_production(1, &[production(1, 5018, 60, true)])
        .await
        .unwrap();
    let rows = repo.production(1).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|r| !r.next_period && r.seed_id == 5016));
    assert!(rows.iter().any(|r| r.next_period && r.seed_id == 5018));
    assert_eq!(repo.production(2).await.unwrap().len(), 1);
    repo.replace_next_procure(1, &[]).await.unwrap();
    let rows = repo.procure(1).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].next_period);

    // updateCurrentProduction / updateCurrentProcure: the amount of the current period only.
    repo.update_current_production(1, &[(5016, 3)]).await.unwrap();
    repo.update_current_procure(1, &[(5073, 4)]).await.unwrap();
    assert_eq!(
        db.scalar("SELECT amount FROM castle_manor_production WHERE castle_id = 1 AND seed_id = 5016 AND next_period = 0")
            .await,
        3
    );
    assert_eq!(
        db.scalar("SELECT amount FROM castle_manor_procure WHERE castle_id = 1 AND crop_id = 5073 AND next_period = 0")
            .await,
        4
    );
    assert_eq!(
        db.scalar("SELECT amount FROM castle_manor_production WHERE castle_id = 1 AND seed_id = 5018")
            .await,
        60
    );

    // resetManorData: every row of the castle.
    repo.delete_castle(1).await.unwrap();
    assert!(repo.production(1).await.unwrap().is_empty());
    assert!(repo.procure(1).await.unwrap().is_empty());
    assert_eq!(repo.production(2).await.unwrap().len(), 1);

    db.drop_database().await;
}
