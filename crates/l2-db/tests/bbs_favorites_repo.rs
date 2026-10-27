#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `BbsFavoriteRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/bbs_favorite.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{BbsFavoriteRepository, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 1] = [include_str!("../sql/game/bbs_favorite.sql")];

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

/// A throwaway database with the favorites table.
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
            "l2_test_bbs_{}_{}_{}",
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

const ALICE: i32 = 268_500_001;
const BOB: i32 = 268_500_002;

#[tokio::test]
async fn favorites_are_added_listed_newest_first_counted_and_deleted_per_player() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = BbsFavoriteRepository::new(db.pool.clone());

    assert_eq!(repo.count_favorites(ALICE).await.unwrap(), 0);
    repo.add_favorite(ALICE, "Home", "_bbshome").await.unwrap();
    repo.add_favorite(ALICE, "Clan", "_bbsclan").await.unwrap();
    repo.add_favorite(BOB, "Region", "_bbsloc").await.unwrap();
    // Two rows added within one second: force the order the list sorts by.
    sqlx::query(
        "UPDATE bbs_favorites SET favAddDate = TIMESTAMPADD(MINUTE, -5, favAddDate) WHERE favTitle = 'Home'",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    let list = repo.favorites_of(ALICE).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(
        (list[0].title.as_str(), list[0].bypass.as_str()),
        ("Clan", "_bbsclan")
    );
    assert_eq!(
        (list[1].title.as_str(), list[1].bypass.as_str()),
        ("Home", "_bbshome")
    );
    assert!(list[0].added_epoch_seconds > list[1].added_epoch_seconds);
    assert_eq!(repo.count_favorites(ALICE).await.unwrap(), 2);
    assert_eq!(repo.count_favorites(BOB).await.unwrap(), 1);

    // A delete only touches the player's own rows.
    let bob_id = repo.favorites_of(BOB).await.unwrap()[0].fav_id;
    repo.delete_favorite(ALICE, i32::try_from(bob_id).unwrap())
        .await
        .unwrap();
    assert_eq!(repo.count_favorites(BOB).await.unwrap(), 1);
    repo.delete_favorite(BOB, i32::try_from(bob_id).unwrap())
        .await
        .unwrap();
    assert_eq!(repo.count_favorites(BOB).await.unwrap(), 0);

    db.drop_database().await;
}
