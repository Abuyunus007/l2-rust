#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `SevenSignsRepository` and `RandomSpawnRepository` against a real
//! MySQL/MariaDB server: Java's `SevenSigns` statements (LOAD_DATA, LOAD_STATUS, INSERT_PLAYER,
//! UPDATE_PLAYER, UPDATE_STATUS) and `AutoSpawnHandler.restoreSpawnData` on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{
    Pool, RandomSpawnRepository, SevenSignsRepository, SevenSignsRow, SevenSignsStatusRow, connect_with,
};
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
            "l2_test_seven_signs_{}_{}_{}",
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

/// The installer's status row reads as Java's start values; `UPDATE_STATUS` writes every column.
#[tokio::test]
async fn the_status_row_is_read_and_written() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SevenSignsRepository::new(db.pool.clone());
    let start = repo.load_status().await.unwrap().unwrap();
    assert_eq!(
        (
            start.current_cycle,
            start.festival_cycle,
            start.active_period,
            start.date
        ),
        (1, 1, 1, 1)
    );
    let row = SevenSignsStatusRow {
        current_cycle: 7,
        festival_cycle: 3,
        active_period: 3,
        date: 1_790_000_000_123,
        previous_winner: 2,
        dawn_stone_score: 12_345.0,
        dawn_festival_score: 40,
        dusk_stone_score: 678.0,
        dusk_festival_score: 5,
        avarice_owner: 2,
        gnosis_owner: 0,
        strife_owner: 1,
        avarice_dawn_score: 9,
        gnosis_dawn_score: 8,
        strife_dawn_score: 7,
        avarice_dusk_score: 6,
        gnosis_dusk_score: 5,
        strife_dusk_score: 4,
        accumulated_bonus: [1, 2, 3, 4, 5],
    };
    repo.update_status(&row).await.unwrap();
    assert_eq!(repo.load_status().await.unwrap(), Some(row));
    db.drop_database().await;
}

/// A member joins (only cabal and seal are written), then his row is updated alone and with
/// every other row; the `DECIMAL` columns come back as Java's doubles.
#[tokio::test]
async fn member_rows_are_inserted_and_updated() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SevenSignsRepository::new(db.pool.clone());
    repo.insert_player(0x6000_0001, "dawn", 1).await.unwrap();
    repo.insert_player(0x6000_0002, "dusk", 3).await.unwrap();
    let all = repo.load_all().await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(
        all[0],
        SevenSignsRow {
            char_id: 0x6000_0001,
            cabal: "dawn".into(),
            seal: 1,
            red_stones: 0,
            green_stones: 0,
            blue_stones: 0,
            ancient_adena_amount: 0.0,
            contribution_score: 0.0,
        }
    );
    let updated = SevenSignsRow {
        red_stones: 10,
        blue_stones: 2,
        ancient_adena_amount: 106.0,
        contribution_score: 106.0,
        ..all[0].clone()
    };
    repo.update_player(&updated).await.unwrap();
    assert_eq!(
        repo.load_player(0x6000_0001).await.unwrap(),
        Some(updated.clone())
    );
    assert_eq!(repo.load_player(0x6000_0003).await.unwrap(), None);

    let reset: Vec<SevenSignsRow> = repo
        .load_all()
        .await
        .unwrap()
        .into_iter()
        .map(|r| SevenSignsRow {
            cabal: String::new(),
            seal: 0,
            contribution_score: 0.0,
            ..r
        })
        .collect();
    repo.update_all_players(&reset).await.unwrap();
    let all = repo.load_all().await.unwrap();
    assert!(all.iter().all(|r| r.cabal.is_empty() && r.seal == 0));
    assert_eq!(all[0].ancient_adena_amount, 106.0);
    db.drop_database().await;
}

/// `random_spawn` with its places, by group id; the enum columns read like `parseBoolean`.
#[tokio::test]
async fn auto_spawn_groups_are_read_with_their_places() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    for statement in [
        "INSERT INTO random_spawn VALUES (12,31126,1,-1,-1,-1,'true','true')",
        "INSERT INTO random_spawn VALUES (11,31113,1,-1,-1,-1,'false','true')",
        "INSERT INTO random_spawn_loc VALUES (11,-52172,78884,-4741,-1)",
        "INSERT INTO random_spawn_loc VALUES (11,-41350,209876,-5087,-1)",
    ] {
        sqlx::query(statement).execute(&db.pool).await.unwrap();
    }
    let groups = RandomSpawnRepository::new(db.pool.clone())
        .load_all()
        .await
        .unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!((groups[0].group_id, groups[0].npc_id), (11, 31113));
    assert!(!groups[0].broadcast_spawn && groups[0].random_spawn);
    assert_eq!(groups[0].initial_delay, -1);
    assert_eq!(groups[0].locations.len(), 2);
    assert!(groups[1].broadcast_spawn && groups[1].locations.is_empty());
    db.drop_database().await;
}
