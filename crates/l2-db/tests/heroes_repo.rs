#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `HeroRepository` against a real MySQL/MariaDB server: Java's `Hero`
//! statements (`init`, `loadMessage`, `loadDiary`, `loadFights`, `updateHeroes`, `setDiaryData`,
//! `saveHeroMessage`, `deleteItemsInDb`) on the unchanged schema (M8-HERO-01).
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{HeroRepository, OlympiadFightRow, OlympiadRepository, Pool, connect_with};
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
            "l2_test_heroes_{}_{}_{}",
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
async fn hero_rows_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = HeroRepository::new(db.pool.clone());
    for (id, name, clan) in [(7, "Alpha", 500), (8, "Beta", 0)] {
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO characters (charId, account_name, char_name, clanid) VALUES ({id}, 'a{id}', '{name}', NULLIF({clan}, 0))"
        )))
        .execute(&db.pool)
        .await
        .unwrap();
    }
    sqlx::query(AssertSqlSafe(
        "INSERT INTO clan_data (clan_id, clan_name, clan_level, ally_id) VALUES (500, 'Clan', 5, 600)"
            .to_owned(),
    ))
    .execute(&db.pool)
    .await
    .unwrap();

    // The choice: INSERT for a new hero, UPDATE after; the load joins the names and the clans.
    assert!(repo.all_heroes().await.unwrap().is_empty());
    repo.insert_hero(7, 88, 1, 1, false).await.unwrap();
    repo.insert_hero(8, 89, 3, 1, true).await.unwrap();
    repo.update_hero(7, 88, 2, 1, true).await.unwrap();
    let mut heroes = repo.all_heroes().await.unwrap();
    heroes.sort_by_key(|h| h.char_id);
    assert_eq!(heroes.len(), 2);
    assert_eq!(heroes[0].char_name, "Alpha");
    assert_eq!(
        (heroes[0].class_id, heroes[0].count, heroes[0].played),
        (88, 2, 1)
    );
    assert!(heroes[0].claimed);
    assert_eq!((heroes[0].clan_id, heroes[0].ally_id), (500, 600));
    assert_eq!((heroes[1].clan_id, heroes[1].ally_id), (0, 0));
    repo.reset_played().await.unwrap();
    assert!(repo.all_heroes().await.unwrap().iter().all(|h| h.played == 0));

    // The message: empty by default, then saved.
    assert_eq!(repo.message(7).await.unwrap().as_deref(), Some(""));
    assert_eq!(repo.message(99).await.unwrap(), None);
    repo.save_message(7, "Hello").await.unwrap();
    assert_eq!(repo.message(7).await.unwrap().as_deref(), Some("Hello"));

    // The diary, oldest first.
    repo.insert_diary(7, 2_000, 2, 0).await.unwrap();
    repo.insert_diary(7, 1_000, 1, 29001).await.unwrap();
    let diary = repo.diary(7).await.unwrap();
    assert_eq!(
        diary
            .iter()
            .map(|d| (d.time, d.action, d.param))
            .collect::<Vec<_>>(),
        [(1_000, 1, 29001), (2_000, 2, 0)]
    );

    // The fights before a time, from either side.
    let olympiad = OlympiadRepository::new(db.pool.clone());
    for (one, two, start) in [(7, 8, 1_000), (8, 7, 2_000), (7, 8, 9_000)] {
        olympiad
            .insert_fight(&OlympiadFightRow {
                char_one_id: one,
                char_two_id: two,
                char_one_class: 88,
                char_two_class: 89,
                winner: 1,
                start,
                time: 60_000,
                classed: true,
            })
            .await
            .unwrap();
    }
    let fights = repo.fights(7, 5_000).await.unwrap();
    assert_eq!(fights.iter().map(|f| f.start).collect::<Vec<_>>(), [1_000, 2_000]);
    assert_eq!(repo.char_name(8).await.unwrap().as_deref(), Some("Beta"));
    assert_eq!(repo.char_name(99).await.unwrap(), None);

    // The hero items of everybody but the GMs.
    sqlx::query(AssertSqlSafe(
        "UPDATE characters SET accesslevel = 100 WHERE charId = 8".to_owned(),
    ))
    .execute(&db.pool)
    .await
    .unwrap();
    for (object, owner, item) in [(1, 7, 6842), (2, 8, 6842), (3, 7, 57)] {
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO items (object_id, owner_id, item_id, count, loc, loc_data) VALUES ({object}, {owner}, {item}, 1, 'INVENTORY', 0)"
        )))
        .execute(&db.pool)
        .await
        .unwrap();
    }
    repo.delete_hero_items().await.unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM items").await, 2);
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM items WHERE object_id = 1").await,
        0
    );

    db.drop_database().await;
}
