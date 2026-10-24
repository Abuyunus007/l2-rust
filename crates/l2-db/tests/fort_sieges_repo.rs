#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `FortSiegeRepository` against a real MySQL/MariaDB server: Java's fortress
//! siege statements (`FortSiege.loadSiegeClan`, `saveSiegeClan`, `removeSiegeClan`,
//! `clearSiegeClan`, `saveSiegeDate`, `FortSiegeManager.checkIsRegistered`, `Fort.initNpcs` and
//! the other spawn lists, `FortSiegeGuardManager.loadSiegeGuard`) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{FortSiegeRepository, Pool, connect_with};
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
            "l2_test_fort_sieges_{}_{}_{}",
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

    async fn exec(&self, sql: &str) {
        sqlx::query(AssertSqlSafe(sql.to_owned()))
            .execute(&self.pool)
            .await
            .unwrap();
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
async fn registrations_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    db.exec("INSERT INTO fort VALUES (101,'Shanty',0,0,0,0,0,0,0),(102,'Southern',0,0,0,0,0,0,0)")
        .await;
    let repo = FortSiegeRepository::new(db.pool.clone());
    assert!(repo.siege_clans(101).await.unwrap().is_empty());

    // saveSiegeClan: INSERT INTO fortsiege_clans (clan_id,fort_id) values (?,?)
    repo.insert_siege_clan(900, 101).await.unwrap();
    repo.insert_siege_clan(901, 101).await.unwrap();
    repo.insert_siege_clan(900, 102).await.unwrap();
    let mut clans = repo.siege_clans(101).await.unwrap();
    clans.sort_unstable();
    assert_eq!(clans, [900, 901]);
    assert!(repo.is_registered(900, 101).await.unwrap());
    assert!(repo.is_registered(900, 102).await.unwrap());
    assert!(!repo.is_registered(901, 102).await.unwrap());
    // The primary key is (clan_id, fort_id): a clan registers once per fortress.
    assert!(repo.insert_siege_clan(900, 101).await.is_err());

    // removeSiegeClan: one clan, then (clan 0) every clan of the fortress.
    repo.delete_siege_clan(101, 900).await.unwrap();
    assert_eq!(repo.siege_clans(101).await.unwrap(), [901]);
    assert!(repo.is_registered(900, 102).await.unwrap());
    repo.insert_siege_clan(902, 101).await.unwrap();
    repo.delete_siege_clan(101, 0).await.unwrap();
    assert!(repo.siege_clans(101).await.unwrap().is_empty());
    assert_eq!(repo.siege_clans(102).await.unwrap(), [900]);

    // clearSiegeClan: the fortress's rows, and with an owner the owner's rows everywhere.
    repo.insert_siege_clan(901, 101).await.unwrap();
    repo.insert_siege_clan(903, 101).await.unwrap();
    repo.insert_siege_clan(903, 102).await.unwrap();
    repo.clear_siege_clans(101, None).await.unwrap();
    assert!(repo.siege_clans(101).await.unwrap().is_empty());
    assert_eq!(repo.siege_clans(102).await.unwrap().len(), 2);
    repo.clear_siege_clans(101, Some(903)).await.unwrap();
    assert_eq!(repo.siege_clans(102).await.unwrap(), [900]);

    // saveSiegeDate: UPDATE fort SET siegeDate = ? WHERE id = ?
    repo.update_siege_date(101, 1_700_000_000_000).await.unwrap();
    assert_eq!(
        db.scalar("SELECT siegeDate FROM fort WHERE id = 101").await,
        1_700_000_000_000
    );
    assert_eq!(db.scalar("SELECT siegeDate FROM fort WHERE id = 102").await, 0);
    repo.update_siege_date(101, 0).await.unwrap();
    assert_eq!(db.scalar("SELECT siegeDate FROM fort WHERE id = 101").await, 0);
    db.drop_database().await;
}

#[tokio::test]
async fn spawn_lists_and_siege_guards() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    // Rows as Java's fort_spawnlist.sql and fort_siege_guards.sql write them (a NULL id counts up).
    db.exec(
        "INSERT INTO fort_spawnlist VALUES \
         (101,null,35658,-53273,156650,-1896,62000,0,0),\
         (101,null,35659,-58672,154703,-2688,0,2,0),\
         (101,null,35660,-52128,157752,-2024,29864,1,0),\
         (101,null,36393,-53000,156000,-1896,100,3,1),\
         (102,null,35690,1,2,3,4,0,0)",
    )
    .await;
    db.exec(
        "INSERT INTO fort_siege_guards VALUES \
         (101,null,35682,-53815,155646,-2024,56240,600,0),\
         (101,null,35685,-53378,154775,-2048,0,0,0),\
         (102,null,35700,5,6,7,8,300,0)",
    )
    .await;
    let repo = FortSiegeRepository::new(db.pool.clone());

    let spawns = repo.spawns(101).await.unwrap();
    assert_eq!(spawns.len(), 4);
    // In the order of `id`.
    assert!(spawns.windows(2).all(|w| w[0].id < w[1].id));
    let envoy = spawns.iter().find(|s| s.spawn_type == 3).unwrap();
    assert_eq!(
        (
            envoy.npc_id,
            envoy.x,
            envoy.y,
            envoy.z,
            envoy.heading,
            envoy.castle_id
        ),
        (36393, -53000, 156_000, -1896, 100, 1)
    );
    let types: Vec<i32> = spawns.iter().map(|s| s.spawn_type).collect();
    assert_eq!(types, [0, 2, 1, 3]);
    assert_eq!(repo.spawns(102).await.unwrap().len(), 1);
    assert!(repo.spawns(103).await.unwrap().is_empty());

    let guards = repo.siege_guards(101).await.unwrap();
    assert_eq!(guards.len(), 2);
    assert_eq!(
        (
            guards[0].npc_id,
            guards[0].x,
            guards[0].heading,
            guards[0].respawn_delay
        ),
        (35682, -53815, 56240, 600)
    );
    assert_eq!(guards[1].respawn_delay, 0);
    db.drop_database().await;
}

#[tokio::test]
async fn blood_oaths_with_javas_statement() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    db.exec("INSERT INTO clan_data (clan_id, clan_name, blood_oath_count) VALUES (900, 'Lions', 4), (901, 'Wolves', 0)")
        .await;
    let repo = FortSiegeRepository::new(db.pool.clone());
    assert_eq!(repo.blood_oath_count(900).await.unwrap(), Some(4));
    assert_eq!(repo.blood_oath_count(777).await.unwrap(), None);
    // Clan.updateBloodOathCountInDB
    repo.update_blood_oath_count(900, 5).await.unwrap();
    repo.update_blood_oath_count(900, 0).await.unwrap();
    assert_eq!(repo.blood_oath_count(900).await.unwrap(), Some(0));
    assert_eq!(repo.blood_oath_count(901).await.unwrap(), Some(0));
    db.drop_database().await;
}
