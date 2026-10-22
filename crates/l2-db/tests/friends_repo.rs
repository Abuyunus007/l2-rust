#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `FriendRepository` against a real MySQL/MariaDB server: the statements of
//! `RequestAnswerFriendInvite`, `RequestFriendDel`, `Player.restoreFriendList` and
//! `BlockList` (`loadList`, `updateInDB`) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{FriendLink, FriendRepository, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 2] = [
    include_str!("../sql/game/characters.sql"),
    include_str!("../sql/game/character_friends.sql"),
];

const ALICE: i32 = 0x1000_0001;
const BOB: i32 = 0x1000_0002;
const CAROL: i32 = 0x1000_0003;

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
            "l2_test_friends_{}_{}_{}",
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
        for file in SCHEMA {
            for statement in statements(file) {
                sqlx::query(AssertSqlSafe(statement))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        let db = Self { admin, name, pool };
        // Three characters: the lists only carry names for ids that have a row.
        for (id, name) in [(ALICE, "Alice"), (BOB, "Bob"), (CAROL, "Carol")] {
            db.exec(&format!(
                "INSERT INTO characters (charId, char_name) VALUES ({id}, '{name}')"
            ))
            .await;
        }
        Some(db)
    }

    async fn exec(&self, sql: &str) {
        sqlx::query(AssertSqlSafe(sql.to_owned()))
            .execute(&self.pool)
            .await
            .unwrap();
    }

    /// The number of rows of `character_friends` matching `where_clause`.
    async fn rows(&self, where_clause: &str) -> i64 {
        sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM character_friends WHERE {where_clause}"
        )))
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

fn ids(links: &[FriendLink]) -> Vec<(i32, Option<&str>)> {
    links.iter().map(|l| (l.friend_id, l.name.as_deref())).collect()
}

#[tokio::test]
async fn friendships_are_two_rows_and_end_in_both_directions() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = FriendRepository::new(db.pool.clone());

    // RequestAnswerFriendInvite: one INSERT with both rows, relation 0.
    repo.add_friends(ALICE, BOB).await.unwrap();
    assert_eq!(
        db.rows(&format!("charId={ALICE} AND friendId={BOB} AND relation=0"))
            .await,
        1
    );
    assert_eq!(
        db.rows(&format!("charId={BOB} AND friendId={ALICE} AND relation=0"))
            .await,
        1
    );

    // Restore: each side sees the other with its name.
    assert_eq!(
        ids(&repo.friends_of(ALICE).await.unwrap()),
        vec![(BOB, Some("Bob"))]
    );
    assert_eq!(
        ids(&repo.friends_of(BOB).await.unwrap()),
        vec![(ALICE, Some("Alice"))]
    );
    assert!(repo.block_list_of(ALICE).await.unwrap().is_empty());

    // RequestFriendDel: both rows go, whatever their relation.
    repo.remove_friends(BOB, ALICE).await.unwrap();
    assert_eq!(db.rows(&format!("charId IN ({ALICE},{BOB})")).await, 0);
    assert!(repo.friends_of(ALICE).await.unwrap().is_empty());
    db.drop_database().await;
}

#[tokio::test]
async fn the_block_list_is_relation_one_and_separate_from_friends() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = FriendRepository::new(db.pool.clone());

    // BlockList.updateInDB(add): one row, relation 1, nothing the other way.
    repo.add_block(ALICE, CAROL).await.unwrap();
    assert_eq!(
        db.rows(&format!("charId={ALICE} AND friendId={CAROL} AND relation=1"))
            .await,
        1
    );
    assert_eq!(db.rows(&format!("charId={CAROL} AND friendId={ALICE}")).await, 0);
    assert_eq!(
        ids(&repo.block_list_of(ALICE).await.unwrap()),
        vec![(CAROL, Some("Carol"))]
    );
    // The friend list of Alice does not show the blocked character.
    assert!(repo.friends_of(ALICE).await.unwrap().is_empty());

    // BlockList.updateInDB(remove) only deletes a block row (relation 1). A friend and a block
    // row of the same pair cannot both exist (primary key charId, friendId), so the friend row is
    // another character's.
    repo.remove_block(ALICE, CAROL).await.unwrap();
    assert!(repo.block_list_of(ALICE).await.unwrap().is_empty());
    repo.add_friends(ALICE, BOB).await.unwrap();
    repo.remove_block(ALICE, BOB).await.unwrap();
    assert_eq!(
        ids(&repo.friends_of(ALICE).await.unwrap()),
        vec![(BOB, Some("Bob"))],
        "a friend row survives the removal of a block"
    );
    db.drop_database().await;
}

#[tokio::test]
async fn a_row_with_the_own_id_and_an_unknown_character_are_handled_like_java() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = FriendRepository::new(db.pool.clone());

    // Java skips friendId == charId when it loads the lists.
    db.exec(&format!(
        "INSERT INTO character_friends (charId, friendId, relation) VALUES ({ALICE}, {ALICE}, 0)"
    ))
    .await;
    // An id without a character row still loads, with no name (Java's CharInfoTable has none).
    db.exec(&format!(
        "INSERT INTO character_friends (charId, friendId, relation) VALUES ({ALICE}, 999999, 0)"
    ))
    .await;
    assert_eq!(ids(&repo.friends_of(ALICE).await.unwrap()), vec![(999_999, None)]);
    db.drop_database().await;
}
