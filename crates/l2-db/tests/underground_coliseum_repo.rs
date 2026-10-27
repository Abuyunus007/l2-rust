#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `UndergroundColiseumRepository` against a real MySQL/MariaDB server: Java's
//! coliseum statements (`restoreBestTeams`, `saveBestTeam`) and the two `global_variables` rows of
//! the battle period (M8-GAMES-01) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{ColiseumBestTeamRow, Pool, UndergroundColiseumRepository, connect_with};
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
            "l2_test_uc_{}_{}_{}",
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

#[tokio::test]
async fn best_teams_and_the_battle_period_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = UndergroundColiseumRepository::new(db.pool.clone());
    assert!(repo.best_teams().await.unwrap().is_empty());

    // saveBestTeam(team, true), read back by restoreBestTeams ordered by arena id.
    repo.insert_best_team(5, "Winner", 3).await.unwrap();
    repo.insert_best_team(2, "Other", 2).await.unwrap();
    assert_eq!(
        repo.best_teams().await.unwrap(),
        vec![
            ColiseumBestTeamRow {
                arena_id: 2,
                leader: "Other".to_owned(),
                wins: 2
            },
            ColiseumBestTeamRow {
                arena_id: 5,
                leader: "Winner".to_owned(),
                wins: 3
            },
        ]
    );

    // saveBestTeam(team, false) binds the arena id as the leader (a Java quirk, kept).
    repo.update_best_team(5, 4).await.unwrap();
    let rows = repo.best_teams().await.unwrap();
    assert_eq!(rows[1].leader, "5");
    assert_eq!(rows[1].wins, 4);
    assert_eq!(db.scalar("SELECT COUNT(*) FROM underground_coliseum").await, 2);

    // GlobalVariablesManager: a missing variable is 0, REPLACE keeps one row.
    assert_eq!(repo.variable_long("UC_START_TIME").await.unwrap(), 0);
    repo.set_variable_long("UC_START_TIME", 1_800_000_000_000)
        .await
        .unwrap();
    repo.set_variable_long("UC_START_TIME", 1_800_000_060_000)
        .await
        .unwrap();
    assert_eq!(
        repo.variable_long("UC_START_TIME").await.unwrap(),
        1_800_000_060_000
    );
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM global_variables WHERE `var` = 'UC_START_TIME'")
            .await,
        1
    );

    db.drop_database().await;
}
