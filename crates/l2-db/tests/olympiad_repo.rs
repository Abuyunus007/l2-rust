#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `OlympiadRepository` against a real MySQL/MariaDB server: Java's
//! Olympiad statements (`Olympiad.load`, `saveNobleData`, `saveOlympiadStatus`,
//! `updateMonthlyData`, `deleteNobles`, `loadNoblesRank`, `getClassLeaderBoard`,
//! `getLastNobleOlympiadPoints`, `OlympiadGameNormal.saveResults`) on the unchanged schema
//! (M8-OLY-01).
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{OlympiadDataRow, OlympiadFightRow, OlympiadNobleRow, OlympiadRepository, Pool, connect_with};
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
            "l2_test_olympiad_{}_{}_{}",
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

fn noble(char_id: i32, class_id: i32, points: i32, done: i32, won: i32) -> OlympiadNobleRow {
    OlympiadNobleRow {
        char_id,
        class_id,
        char_name: String::new(),
        olympiad_points: points,
        competitions_done: done,
        competitions_won: won,
        competitions_lost: done - won,
        competitions_drawn: 0,
        competitions_done_week: 2,
        competitions_done_week_classed: 1,
        competitions_done_week_non_classed: 1,
        competitions_done_week_team: 0,
    }
}

#[tokio::test]
async fn olympiad_rows_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = OlympiadRepository::new(db.pool.clone());

    // No `olympiad_data` row yet: Java starts cycle 1.
    assert_eq!(repo.load_data().await.unwrap(), None);
    let data = OlympiadDataRow {
        current_cycle: 3,
        period: 1,
        olympiad_end: 1_800_000_000_000,
        validation_end: 1_800_086_400_000,
        next_weekly_change: 1_800_604_800_000,
    };
    repo.save_data(&data).await.unwrap();
    repo.save_data(&data).await.unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM olympiad_data").await, 1);
    assert_eq!(repo.load_data().await.unwrap(), Some(data));

    // The nobles: INSERT for a new one, UPDATE after; the load joins the names.
    for (id, name) in [(7, "Alpha"), (8, "Beta"), (9, "Gamma")] {
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO characters (charId, account_name, char_name) VALUES ({id}, 'a{id}', '{name}')"
        )))
        .execute(&db.pool)
        .await
        .unwrap();
    }
    repo.save_noble(&noble(7, 88, 50, 20, 12), true).await.unwrap();
    repo.save_noble(&noble(8, 88, 70, 16, 9), true).await.unwrap();
    repo.save_noble(&noble(9, 132, 30, 3, 1), true).await.unwrap();
    let mut updated = noble(7, 88, 60, 21, 13);
    repo.save_noble(&updated, false).await.unwrap();
    let mut loaded = repo.load_nobles().await.unwrap();
    loaded.sort_by_key(|n| n.char_id);
    updated.char_name = "Alpha".into();
    assert_eq!(loaded[0], updated);
    assert_eq!(loaded.len(), 3);

    // The current board (`OlympiadShowMonthlyWinners = False`) and the copy of the period end.
    assert_eq!(
        repo.class_leader_board(88, false, 15).await.unwrap(),
        ["Beta", "Alpha"]
    );
    assert!(repo.class_leader_board(88, true, 15).await.unwrap().is_empty());
    repo.update_monthly_data().await.unwrap();
    assert_eq!(
        repo.class_leader_board(88, true, 15).await.unwrap(),
        ["Beta", "Alpha"]
    );
    // The soul hound board lists both genders; Gamma has too few matches for 15.
    assert!(repo.class_leader_board(132, true, 15).await.unwrap().is_empty());
    assert_eq!(repo.class_leader_board(132, true, 3).await.unwrap(), ["Gamma"]);
    assert_eq!(repo.classified_nobles(15).await.unwrap(), [8, 7]);
    assert_eq!(repo.last_noble_points(7).await.unwrap(), 60);
    assert_eq!(repo.last_noble_points(99).await.unwrap(), 0);

    // deleteNobles: the table goes, the copy stays.
    repo.delete_nobles().await.unwrap();
    assert!(repo.load_nobles().await.unwrap().is_empty());
    assert_eq!(db.scalar("SELECT COUNT(*) FROM olympiad_nobles_eom").await, 3);

    // The unclaimed points of an offline noble: replaced, then read back.
    repo.store_unclaimed_points(7, "UNCLAIMED_OLYMPIAD_POINTS", 65)
        .await
        .unwrap();
    repo.store_unclaimed_points(7, "UNCLAIMED_OLYMPIAD_POINTS", 45)
        .await
        .unwrap();
    assert_eq!(
        repo.variable(7, "UNCLAIMED_OLYMPIAD_POINTS")
            .await
            .unwrap()
            .as_deref(),
        Some("45")
    );
    assert_eq!(repo.variable(8, "UNCLAIMED_OLYMPIAD_POINTS").await.unwrap(), None);

    // saveResults.
    repo.insert_fight(&OlympiadFightRow {
        char_one_id: 7,
        char_two_id: 8,
        char_one_class: 88,
        char_two_class: 88,
        winner: 2,
        start: 1_800_000_000_000,
        time: 42_000,
        classed: true,
    })
    .await
    .unwrap();
    assert_eq!(
        db.scalar("SELECT winner + 10 * classed FROM olympiad_fights WHERE charOneId = 7")
            .await,
        12
    );

    db.drop_database().await;
}
