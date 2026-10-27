#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `CursedWeaponRepository` against a real MySQL/MariaDB server: Java's
//! cursed weapon statements (`CursedWeaponsManager.restore`, `controlPlayers`, `removeFromDb`,
//! `CursedWeapon.saveData` and the offline `endOfLife`) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{CursedWeaponRepository, CursedWeaponRow, Pool, connect_with};
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
            "l2_test_cursed_{}_{}_{}",
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
async fn cursed_weapon_rows_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = CursedWeaponRepository::new(db.pool.clone());
    let row = CursedWeaponRow {
        item_id: 8190,
        char_id: 268_500_001,
        player_karma: 12,
        player_pk_kills: 3,
        nb_kills: 25,
        end_time: 1_800_000_000_000,
    };

    // saveData: the old row goes, a held weapon is written.
    repo.save(8190, Some(&row)).await.unwrap();
    repo.save(8190, Some(&row)).await.unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM cursed_weapons").await, 1);
    // restore: every row.
    assert_eq!(repo.all().await.unwrap(), vec![row.clone()]);
    // A weapon that is not held leaves no row.
    repo.save(8190, None).await.unwrap();
    assert!(repo.all().await.unwrap().is_empty());

    // removeFromDb.
    repo.save(
        8689,
        Some(&CursedWeaponRow {
            item_id: 8689,
            ..row.clone()
        }),
    )
    .await
    .unwrap();
    repo.remove(8689).await.unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM cursed_weapons").await, 0);

    // controlPlayers and the offline endOfLife: the owner of the item, the delete, the karma.
    sqlx::query(AssertSqlSafe(
        "INSERT INTO characters (charId, account_name, char_name, karma, pkkills) VALUES (7, 'a', 'Holder', 9999999, 0)"
            .to_owned(),
    ))
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(AssertSqlSafe(
        "INSERT INTO items (owner_id, object_id, item_id, count, loc, loc_data) VALUES (7, 100, 8190, 1, 'INVENTORY', 0)"
            .to_owned(),
    ))
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(repo.item_owner(8190).await.unwrap(), Some(7));
    assert_eq!(repo.item_owner(8689).await.unwrap(), None);
    assert_eq!(repo.delete_owner_item(7, 8190).await.unwrap(), 1);
    assert_eq!(repo.delete_owner_item(7, 8190).await.unwrap(), 0);
    assert_eq!(repo.restore_karma(7, 12, 3).await.unwrap(), 1);
    assert_eq!(
        db.scalar("SELECT karma FROM characters WHERE charId = 7").await,
        12
    );
    assert_eq!(
        db.scalar("SELECT pkkills FROM characters WHERE charId = 7").await,
        3
    );
    assert_eq!(repo.restore_karma(99, 1, 1).await.unwrap(), 0);

    db.drop_database().await;
}
