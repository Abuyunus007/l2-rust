#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `SiegeRepository` against a real MySQL/MariaDB server (M7-SIEGE-01): Java's
//! siege statements (`Siege.saveSiegeClan`, `removeSiegeClan`, `clearSiegeClan`,
//! `clearSiegeWaitingClan`, `saveSiegeDate`, `SiegeGuardManager`, the door and trap upgrades,
//! `Clan.updateBloodAllianceCountInDB`) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{NewSiegeGuard, Pool, SiegeClanRow, SiegeRepository, connect_with};
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
            "l2_test_sieges_{}_{}_{}",
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

    async fn text(&self, sql: &str) -> String {
        sqlx::query_scalar(AssertSqlSafe(sql.to_owned()))
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
async fn siege_clans_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SiegeRepository::new(db.pool.clone());
    repo.insert_siege_clan(600, 1, 1).await.unwrap();
    repo.insert_siege_clan(700, 1, 2).await.unwrap();
    repo.insert_siege_clan(700, 2, 1).await.unwrap();
    // A NULL type reads as 0, like Java's `getInt`.
    db.exec("INSERT INTO siege_clans (castle_id, clan_id, type, castle_owner) VALUES (3, 800, NULL, NULL)")
        .await;
    let mut rows = repo.siege_clans().await.unwrap();
    rows.sort_by_key(|r| (r.castle_id, r.clan_id));
    assert_eq!(
        rows,
        vec![
            SiegeClanRow {
                castle_id: 1,
                clan_id: 600,
                kind: 1
            },
            SiegeClanRow {
                castle_id: 1,
                clan_id: 700,
                kind: 2
            },
            SiegeClanRow {
                castle_id: 2,
                clan_id: 700,
                kind: 1
            },
            SiegeClanRow {
                castle_id: 3,
                clan_id: 800,
                kind: 0
            },
        ]
    );
    // `castle_owner` is always written 0.
    assert_eq!(
        db.scalar("SELECT castle_owner FROM siege_clans WHERE clan_id = 600")
            .await,
        0
    );
    // The primary key (clan_id, castle_id) refuses a second registration, as in Java.
    assert!(repo.insert_siege_clan(600, 1, 1).await.is_err());

    // Approval, the waiting list, one clan, a castle, an owner.
    repo.update_siege_clan(1, 700, 0).await.unwrap();
    assert_eq!(
        db.scalar("SELECT type FROM siege_clans WHERE castle_id = 1 AND clan_id = 700")
            .await,
        0
    );
    repo.insert_siege_clan(900, 1, 2).await.unwrap();
    repo.delete_waiting_siege_clans(1).await.unwrap();
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM siege_clans WHERE castle_id = 1")
            .await,
        2
    );
    repo.delete_siege_clan(1, 600).await.unwrap();
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM siege_clans WHERE castle_id = 1")
            .await,
        1
    );
    repo.delete_siege_clans_of_clan(700).await.unwrap();
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM siege_clans WHERE clan_id = 700")
            .await,
        0
    );
    repo.delete_siege_clans_of_castle(3).await.unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM siege_clans").await, 0);
    db.drop_database().await;
}

#[tokio::test]
async fn siege_date_guards_upgrades_and_blood_alliance() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SiegeRepository::new(db.pool.clone());
    db.exec("INSERT INTO castle VALUES (1,'Gludio',0,0,0,'true',0,'false',0)")
        .await;
    repo.update_castle_siege_date(1, 1_793_548_800_789, 1_792_672_496_789, false)
        .await
        .unwrap();
    assert_eq!(
        db.scalar("SELECT siegeDate FROM castle WHERE id = 1").await,
        1_793_548_800_789
    );
    assert_eq!(
        db.scalar("SELECT regTimeEnd FROM castle WHERE id = 1").await,
        1_792_672_496_789
    );
    assert_eq!(
        db.text("SELECT regTimeOver FROM castle WHERE id = 1").await,
        "false"
    );

    // Two guards of the datapack and a mercenary.
    db.exec("INSERT INTO castle_siege_guards (castleId,id,npcId,x,y,z,heading,respawnDelay,isHired) VALUES (1,1,35085,-18460,113107,-2759,15608,10800,0)").await;
    let merc = NewSiegeGuard {
        castle_id: 1,
        npc_id: 35010,
        x: -18000,
        y: 113000,
        z: -2700,
        heading: 100,
        respawn_delay: 0,
        is_hired: 1,
    };
    repo.insert_siege_guard(&merc).await.unwrap();
    let guards = repo.siege_guards().await.unwrap();
    assert_eq!(guards.len(), 2);
    assert_eq!(
        (guards[0].npc_id, guards[0].respawn_delay, guards[0].is_hired),
        (35085, 10800, 0)
    );
    assert_eq!(
        (guards[1].npc_id, guards[1].x, guards[1].is_hired),
        (35010, -18000, 1)
    );
    // `removeMerc` only takes a hired row at that spot; `removeMercs` every hired row.
    repo.delete_hired_guard(35085, -18460, 113107, -2759)
        .await
        .unwrap();
    assert_eq!(repo.siege_guards().await.unwrap().len(), 2);
    repo.delete_hired_guard(35010, -18000, 113000, -2700)
        .await
        .unwrap();
    assert_eq!(repo.siege_guards().await.unwrap().len(), 1);
    repo.insert_siege_guard(&merc).await.unwrap();
    repo.delete_hired_guards(1).await.unwrap();
    assert_eq!(repo.siege_guards().await.unwrap().len(), 1);

    // The upgrades (`REPLACE INTO` keeps one row per door / tower).
    repo.replace_door_upgrade(19210001, 2, 1).await.unwrap();
    repo.replace_door_upgrade(19210001, 3, 1).await.unwrap();
    repo.replace_trap_upgrade(1, 0, 2).await.unwrap();
    let doors = repo.door_upgrades().await.unwrap();
    assert_eq!(doors.len(), 1);
    assert_eq!(
        (doors[0].door_id, doors[0].ratio, doors[0].castle_id),
        (19210001, 3, 1)
    );
    let traps = repo.trap_upgrades().await.unwrap();
    assert_eq!(
        (traps[0].castle_id, traps[0].tower_index, traps[0].level),
        (1, 0, 2)
    );
    repo.delete_door_upgrades(1).await.unwrap();
    repo.delete_trap_upgrades(1).await.unwrap();
    assert!(repo.door_upgrades().await.unwrap().is_empty());
    assert!(repo.trap_upgrades().await.unwrap().is_empty());

    db.exec("INSERT INTO clan_data (clan_id, clan_name) VALUES (900, 'Lions')")
        .await;
    repo.update_blood_alliance_count(900, 3).await.unwrap();
    assert_eq!(
        db.scalar("SELECT blood_alliance_count FROM clan_data WHERE clan_id = 900")
            .await,
        3
    );
    db.drop_database().await;
}
