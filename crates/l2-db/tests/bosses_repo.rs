#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `BossRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from the four boss table files in `sql/game`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{
    BossRepository, GrandBossListRow, GrandBossStore, GrandBossUpdate, Pool, RaidBossRow, connect_with,
};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 4] = [
    include_str!("../sql/game/grandboss_data.sql"),
    include_str!("../sql/game/grandboss_list.sql"),
    include_str!("../sql/game/raidboss_spawnlist.sql"),
    include_str!("../sql/game/character_raid_points.sql"),
];

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

/// A throwaway database with the boss tables.
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
            "l2_test_bosses_{}_{}_{}",
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

/// `GrandBossManager.init`: the state rows, the decimals as numbers; `storeMe` writes whole rows
/// or the status only, and replaces `grandboss_list`.
#[tokio::test]
async fn grand_boss_rows_are_read_and_stored() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    sqlx::query(
        "INSERT INTO grandboss_data (boss_id,loc_x,loc_y,loc_z,heading,currentHP,currentMP) VALUES          (29001,-21610,181594,-5734,0,229898.48,667.776),(29020,116033,17447,10107,-25348,4068372,39960)",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let repo = BossRepository::new(db.pool.clone());
    let rows = repo.grand_bosses().await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].boss_id, 29001);
    assert_eq!(
        (rows[0].loc_x, rows[0].loc_y, rows[0].loc_z),
        (-21610, 181594, -5734)
    );
    assert!((rows[0].current_hp - 229_898.48).abs() < 1e-6);
    assert_eq!((rows[0].status, rows[0].respawn_time), (0, 0));
    assert_eq!(rows[1].heading, -25348);
    assert_eq!(repo.grand_boss(29020).await.unwrap().unwrap().boss_id, 29020);
    assert_eq!(repo.grand_boss(1).await.unwrap(), None);

    let full = GrandBossUpdate {
        boss_id: 29001,
        loc_x: 1,
        loc_y: 2,
        loc_z: 3,
        heading: 4,
        respawn_time: 1_792_000_000_123,
        current_hp: 100.5,
        current_mp: 50.25,
        status: 1,
    };
    repo.store_all(
        &[
            GrandBossListRow {
                player_id: 268435456,
                zone: 70050,
            },
            GrandBossListRow {
                player_id: 268435457,
                zone: 70050,
            },
        ],
        &[
            GrandBossStore::Full(full),
            GrandBossStore::StatusOnly {
                boss_id: 29020,
                status: 3,
            },
        ],
    )
    .await
    .unwrap();
    let rows = repo.grand_bosses().await.unwrap();
    assert_eq!(rows[0].status, 1);
    assert_eq!(rows[0].respawn_time, 1_792_000_000_123);
    assert_eq!((rows[0].loc_x, rows[0].heading), (1, 4));
    assert!((rows[0].current_mp - 50.25).abs() < 1e-9);
    assert_eq!(rows[1].status, 3);
    assert_eq!(rows[1].loc_x, 116033);
    assert_eq!(repo.grand_boss_list().await.unwrap().len(), 2);

    // A second store replaces the list.
    repo.store_all(
        &[GrandBossListRow {
            player_id: 268435458,
            zone: 70051,
        }],
        &[],
    )
    .await
    .unwrap();
    let list = repo.grand_boss_list().await.unwrap();
    assert_eq!(
        list,
        [GrandBossListRow {
            player_id: 268435458,
            zone: 70051
        }]
    );

    repo.update_grand_boss_status(29001, 2).await.unwrap();
    assert_eq!(repo.grand_boss(29001).await.unwrap().unwrap().status, 2);
    db.drop_database().await;
}

/// `RaidBossSpawnManager`: the spawn list reads with the delay defaults, a NULL HP as 0; the
/// insert, the update and the delete are Java's statements.
#[tokio::test]
async fn raid_boss_rows_are_read_inserted_updated_and_deleted() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    sqlx::query(
        "INSERT INTO raidboss_spawnlist (boss_id,loc_x,loc_y,loc_z,heading,currentHp,currentMp) VALUES          (25001,-54464,-17536,-3000,0,NULL,NULL)",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let repo = BossRepository::new(db.pool.clone());
    let rows = repo.raid_bosses().await.unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!((row.boss_id, row.amount), (25001, 1));
    assert_eq!((row.respawn_delay, row.respawn_random), (86400, 43200));
    assert_eq!((row.respawn_time, row.current_hp, row.current_mp), (0, 0.0, 0.0));

    repo.insert_raid_boss(&RaidBossRow {
        boss_id: 25002,
        amount: 1,
        loc_x: 10,
        loc_y: 20,
        loc_z: 30,
        heading: 40,
        respawn_delay: 0,
        respawn_random: 0,
        respawn_time: 0,
        current_hp: 1234.0,
        current_mp: 56.0,
    })
    .await
    .unwrap();
    repo.update_raid_boss(25002, 1_792_000_000_999, 999.0, 55.0)
        .await
        .unwrap();
    let rows = repo.raid_bosses().await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].respawn_time, 1_792_000_000_999);
    assert_eq!((rows[1].current_hp, rows[1].current_mp), (999.0, 55.0));
    // The insert leaves the delay columns to their defaults, like Java's statement.
    assert_eq!(rows[1].respawn_delay, 86400);

    repo.delete_raid_boss(25001).await.unwrap();
    let rows = repo.raid_bosses().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].boss_id, 25002);
    db.drop_database().await;
}

/// `RaidBossPointsManager`: `REPLACE` of a total, the rows of one character, the weekly delete.
#[tokio::test]
async fn raid_points_are_replaced_read_and_cleared() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = BossRepository::new(db.pool.clone());
    repo.replace_raid_points(268435456, 25001, 10).await.unwrap();
    repo.replace_raid_points(268435456, 25001, 17).await.unwrap();
    repo.replace_raid_points(268435456, 25002, 3).await.unwrap();
    repo.replace_raid_points(268435457, 25001, 5).await.unwrap();
    let all = repo.raid_points().await.unwrap();
    assert_eq!(all.len(), 3);
    let mine = repo.raid_points_of(268435456).await.unwrap();
    assert_eq!(mine.len(), 2);
    assert!(mine.iter().any(|r| r.boss_id == 25001 && r.points == 17));
    repo.delete_all_raid_points().await.unwrap();
    assert!(repo.raid_points().await.unwrap().is_empty());
    db.drop_database().await;
}
