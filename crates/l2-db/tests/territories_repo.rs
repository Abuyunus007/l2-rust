#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `TerritoryRepository` against a real MySQL/MariaDB server: Java's Territory
//! War statements (`TerritoryWarManager.load`, `changeRegistration`, `updateTerritoryData` and
//! the `nextTWStartDate` global variable) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{Pool, TerritoryRegistrationRow, TerritoryRepository, TerritoryRow, connect_with};
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
            "l2_test_territories_{}_{}_{}",
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

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn spawn_list_and_territories_as_java_loads_them() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    // Rows as Java's territory_spawnlist.sql and territories.sql write them (a NULL id counts up).
    db.exec(
        "INSERT INTO territory_spawnlist VALUES \
         (1,null,36481,-15094,124113,-3119,50948,0),\
         (1,null,36499,-51598,156183,-2057,0,1),\
         (1,null,36572,-17780,109260,-2499,0,3),\
         (2,null,36491,1,2,3,-17076,0)",
    )
    .await;
    db.exec("INSERT INTO territories VALUES (81,1,101,'81;'),(82,2,103,'')")
        .await;
    let repo = TerritoryRepository::new(db.pool.clone());

    let spawns = repo.spawns().await.unwrap();
    assert_eq!(spawns.len(), 4);
    assert!(spawns.windows(2).all(|w| w[0].id < w[1].id));
    let ward = spawns.iter().find(|s| s.spawn_type == 3).unwrap();
    assert_eq!(
        (ward.castle_id, ward.npc_id, ward.x, ward.y, ward.z, ward.heading),
        (1, 36572, -17780, 109_260, -2499, 0)
    );
    assert_eq!(spawns[3].heading, -17076);

    let mut territories = repo.territories().await.unwrap();
    territories.sort_by_key(|t| t.territory_id);
    assert_eq!(
        territories,
        [
            TerritoryRow {
                territory_id: 81,
                castle_id: 1,
                fort_id: 101,
                owned_ward_ids: "81;".into(),
            },
            TerritoryRow {
                territory_id: 82,
                castle_id: 2,
                fort_id: 103,
                owned_ward_ids: String::new(),
            },
        ]
    );

    // updateTerritoryData: UPDATE territories SET ownedWardIds=? WHERE territoryId=?
    repo.update_owned_ward_ids(81, "81;82;").await.unwrap();
    repo.update_owned_ward_ids(82, "").await.unwrap();
    let mut territories = repo.territories().await.unwrap();
    territories.sort_by_key(|t| t.territory_id);
    assert_eq!(territories[0].owned_ward_ids, "81;82;");
    assert_eq!(territories[1].owned_ward_ids, "");
    db.drop_database().await;
}

#[tokio::test]
async fn registrations_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = TerritoryRepository::new(db.pool.clone());
    assert!(repo.registrations().await.unwrap().is_empty());
    // changeRegistration(castleId, objId, false): INSERT
    repo.insert_registration(1, 500).await.unwrap();
    repo.insert_registration(1, 0x6000_0001).await.unwrap();
    repo.insert_registration(5, 500).await.unwrap();
    // The primary key is (castleId, registeredId).
    assert!(repo.insert_registration(1, 500).await.is_err());
    let mut rows = repo.registrations().await.unwrap();
    rows.sort_by_key(|r| (r.castle_id, r.registered_id));
    assert_eq!(
        rows,
        [
            TerritoryRegistrationRow {
                castle_id: 1,
                registered_id: 500,
            },
            TerritoryRegistrationRow {
                castle_id: 1,
                registered_id: 0x6000_0001,
            },
            TerritoryRegistrationRow {
                castle_id: 5,
                registered_id: 500,
            },
        ]
    );
    // changeRegistration(castleId, objId, true): DELETE of the one row.
    repo.delete_registration(1, 500).await.unwrap();
    repo.delete_registration(1, 777).await.unwrap();
    let mut rows = repo.registrations().await.unwrap();
    rows.sort_by_key(|r| (r.castle_id, r.registered_id));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].registered_id, 0x6000_0001);
    assert_eq!(rows[1].castle_id, 5);
    db.drop_database().await;
}

#[tokio::test]
async fn the_next_war_date_is_a_global_variable() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = TerritoryRepository::new(db.pool.clone());
    assert_eq!(repo.next_tw_start_date().await.unwrap(), None);
    repo.store_next_tw_start_date(1_793_548_800_789).await.unwrap();
    assert_eq!(
        repo.next_tw_start_date().await.unwrap().as_deref(),
        Some("1793548800789")
    );
    // REPLACE: a second store replaces the row.
    repo.store_next_tw_start_date(5).await.unwrap();
    assert_eq!(repo.next_tw_start_date().await.unwrap().as_deref(), Some("5"));
    db.drop_database().await;
}
