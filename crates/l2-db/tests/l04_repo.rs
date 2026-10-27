#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `BookmarkRepository`, `ContactRepository`, `AccountDataRepository` and the
//! friend summary of `FriendRepository` against a real MySQL/MariaDB server (L-04).
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{
    AccountDataRepository, BookmarkRepository, BookmarkRow, ContactLink, ContactRepository, FriendRepository,
    Pool, VAR_PWD, VAR_WTE, connect_with, crypt_secondary_password,
};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 4] = [
    include_str!("../sql/game/account_gsdata.sql"),
    include_str!("../sql/game/character_contacts.sql"),
    include_str!("../sql/game/character_tpbookmark.sql"),
    include_str!("../sql/game/characters.sql"),
];

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

/// A throwaway database with the L-04 tables.
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
            "l2_test_l04_{}_{}_{}",
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

fn bookmark(id: i32, tag: Option<&str>) -> BookmarkRow {
    BookmarkRow {
        id,
        x: -71_338 + id,
        y: 258_271,
        z: -3_104,
        icon: 5,
        tag: tag.map(str::to_owned),
        name: format!("Place {id}"),
    }
}

/// Save, change and delete a bookmark with Java's statements; the other character's rows stay.
#[tokio::test]
async fn bookmarks_are_saved_changed_and_deleted() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = BookmarkRepository::new(db.pool.clone());
    repo.add_bookmark(ALICE, &bookmark(1, Some("home"))).await.unwrap();
    repo.add_bookmark(ALICE, &bookmark(2, None)).await.unwrap();
    repo.add_bookmark(BOB, &bookmark(1, Some("bob"))).await.unwrap();
    // The primary key is (charId, Id): the same slot twice is refused, as in Java (which logs it).
    assert!(repo.add_bookmark(ALICE, &bookmark(1, None)).await.is_err());

    let mut rows = repo.bookmarks_of(ALICE).await.unwrap();
    rows.sort_by_key(|r| r.id);
    assert_eq!(rows, [bookmark(1, Some("home")), bookmark(2, None)]);

    repo.update_bookmark(ALICE, 1, 9, Some("moved"), "Castle").await.unwrap();
    let mut rows = repo.bookmarks_of(ALICE).await.unwrap();
    rows.sort_by_key(|r| r.id);
    assert_eq!((rows[0].icon, rows[0].tag.as_deref(), rows[0].name.as_str()), (9, Some("moved"), "Castle"));
    // The location is not part of the UPDATE.
    assert_eq!((rows[0].x, rows[0].y, rows[0].z), (-71_337, 258_271, -3_104));
    assert_eq!(repo.bookmarks_of(BOB).await.unwrap(), [bookmark(1, Some("bob"))]);

    repo.delete_bookmark(ALICE, 1).await.unwrap();
    assert_eq!(repo.bookmarks_of(ALICE).await.unwrap(), [bookmark(2, None)]);
    // Deleting a slot that is not there is not an error.
    repo.delete_bookmark(ALICE, 7).await.unwrap();
    assert_eq!(repo.bookmarks_of(BOB).await.unwrap().len(), 1);

    db.drop_database().await;
}

/// `REPLACE` makes adding twice harmless; the read gives the name from `characters` (none for an
/// id no character has).
#[tokio::test]
async fn contacts_are_added_removed_and_read_with_their_names() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    // Every other column is nullable or has a default.
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, classid) VALUES \
         ('acc', ?, 'Bob', 1, 0)",
    )
    .bind(BOB)
    .execute(&db.pool)
    .await
    .unwrap();

    let repo = ContactRepository::new(db.pool.clone());
    repo.add_contact(ALICE, BOB).await.unwrap();
    repo.add_contact(ALICE, BOB).await.unwrap();
    repo.add_contact(ALICE, 999).await.unwrap();
    let mut rows = repo.contacts_of(ALICE).await.unwrap();
    rows.sort_by_key(|r| r.contact_id);
    assert_eq!(
        rows,
        [
            ContactLink {
                contact_id: 999,
                name: None
            },
            ContactLink {
                contact_id: BOB,
                name: Some("Bob".into())
            }
        ]
    );
    assert!(repo.contacts_of(BOB).await.unwrap().is_empty());

    repo.remove_contact(ALICE, BOB).await.unwrap();
    assert_eq!(repo.contacts_of(ALICE).await.unwrap().len(), 1);
    repo.remove_contact(ALICE, BOB).await.unwrap();

    db.drop_database().await;
}

/// The `secauth_*` rows: the password, then a changed one, and the upsert of the attempts.
#[tokio::test]
async fn the_secondary_password_rows_follow_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = AccountDataRepository::new(db.pool.clone());
    assert!(repo.secondary_auth_of("acc").await.unwrap().is_empty());

    let first = crypt_secondary_password("123457");
    repo.insert_password("acc", &first).await.unwrap();
    // `INSERT INTO account_gsdata VALUES (?, ?, ?)` twice hits the primary key.
    assert!(repo.insert_password("acc", &first).await.is_err());
    // Another account's rows and other vars are not `secauth_%` rows of this account.
    repo.insert_password("other", "x").await.unwrap();
    sqlx::query("INSERT INTO account_gsdata VALUES ('acc', 'something_else', '1')")
        .execute(&db.pool)
        .await
        .unwrap();

    repo.insert_wrong_attempt("acc", 2).await.unwrap();
    repo.insert_wrong_attempt("acc", 3).await.unwrap();
    let mut rows = repo.secondary_auth_of("acc").await.unwrap();
    rows.sort_by(|a, b| a.var.cmp(&b.var));
    assert_eq!(
        rows.iter().map(|r| (r.var.as_str(), r.value.as_str())).collect::<Vec<_>>(),
        [(VAR_PWD, first.as_str()), (VAR_WTE, "3")]
    );

    let second = crypt_secondary_password("765432");
    repo.update_password("acc", &second).await.unwrap();
    let rows = repo.secondary_auth_of("acc").await.unwrap();
    assert!(rows.iter().any(|r| r.var == VAR_PWD && r.value == second));
    assert_eq!(repo.secondary_auth_of("other").await.unwrap()[0].value, "x");

    db.drop_database().await;
}

/// `FriendListExtended`'s row: name, online flag, class and level; `None` for an unknown id.
#[tokio::test]
async fn the_friend_summary_reads_name_online_class_and_level() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    sqlx::query(
        "INSERT INTO characters (account_name, charId, char_name, level, classid, online) VALUES \
         ('acc', ?, 'Bob', 61, 18, 1)",
    )
    .bind(BOB)
    .execute(&db.pool)
    .await
    .unwrap();
    let repo = FriendRepository::new(db.pool.clone());
    let summary = repo.character_summary(BOB).await.unwrap().unwrap();
    assert_eq!(
        (summary.name.as_str(), summary.online, summary.class_id, summary.level),
        ("Bob", 1, 18, 61)
    );
    assert!(repo.character_summary(ALICE).await.unwrap().is_none());

    db.drop_database().await;
}
