#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `QuestRepository` and `GlobalVariableRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/character_quests.sql` and `global_variables.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{GlobalVariableRepository, Pool, QuestRepository, QuestStateRow, QuestVariableRow, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 2] = [
    include_str!("../sql/game/character_quests.sql"),
    include_str!("../sql/game/global_variables.sql"),
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

/// A throwaway database with the quest and global variable tables.
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
            "l2_test_quest_{}_{}_{}",
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

const ALICE: i32 = 268_700_001;
const BOB: i32 = 268_700_002;

fn state(name: &str, value: &str) -> QuestStateRow {
    QuestStateRow {
        name: name.to_owned(),
        value: Some(value.to_owned()),
    }
}

fn var(name: &str, var: &str, value: &str) -> QuestVariableRow {
    QuestVariableRow {
        name: name.to_owned(),
        var: var.to_owned(),
        value: Some(value.to_owned()),
    }
}

#[tokio::test]
async fn quest_rows_are_written_read_and_deleted_like_java() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = QuestRepository::new(db.pool.clone());
    const Q: &str = "Q00001_LettersOfLove";

    // `startQuest`: `set("cond", "1")` (a new variable), then the state row.
    repo.insert_variable(ALICE, Q, "cond", "1").await.unwrap();
    repo.insert_variable(ALICE, Q, "<state>", "Started").await.unwrap();
    repo.insert_variable(BOB, Q, "<state>", "Started").await.unwrap();
    // `setCond(2)`: an update of the variable.
    repo.update_variable(ALICE, Q, "cond", "2").await.unwrap();
    // `ON DUPLICATE KEY UPDATE value=?`: an insert of a known variable replaces it.
    repo.insert_variable(ALICE, Q, "memoState", "3").await.unwrap();
    repo.insert_variable(ALICE, Q, "memoState", "4").await.unwrap();

    assert_eq!(repo.quest_states_of(ALICE).await.unwrap(), [state(Q, "Started")]);
    let mut vars = repo.quest_variables_of(ALICE).await.unwrap();
    vars.sort_by(|a, b| a.var.cmp(&b.var));
    assert_eq!(vars, [var(Q, "cond", "2"), var(Q, "memoState", "4")]);

    // `unset`.
    repo.delete_variable(ALICE, Q, "memoState").await.unwrap();
    assert_eq!(repo.quest_variables_of(ALICE).await.unwrap(), [var(Q, "cond", "2")]);

    // `exitQuest(false)`: everything but the state row goes.
    repo.delete_quest(ALICE, Q, false).await.unwrap();
    assert!(repo.quest_variables_of(ALICE).await.unwrap().is_empty());
    assert_eq!(repo.quest_states_of(ALICE).await.unwrap(), [state(Q, "Started")]);
    // `exitQuest(true)`: the state row goes as well; Bob keeps his.
    repo.delete_quest(ALICE, Q, true).await.unwrap();
    assert!(repo.quest_states_of(ALICE).await.unwrap().is_empty());
    assert_eq!(repo.quest_states_of(BOB).await.unwrap(), [state(Q, "Started")]);

    db.drop_database().await;
}

#[tokio::test]
async fn global_variables_are_replaced_and_deleted_one_by_one() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = GlobalVariableRepository::new(db.pool.clone());
    repo.replace_variable("a", "1").await.unwrap();
    repo.replace_variable("b", "2").await.unwrap();
    // `REPLACE INTO`: a second write replaces the value.
    repo.replace_variable("b", "3").await.unwrap();
    let mut loaded = repo.load_all().await.unwrap();
    loaded.sort();
    assert_eq!(
        loaded,
        [("a".to_owned(), Some("1".to_owned())), ("b".to_owned(), Some("3".to_owned()))]
    );
    repo.delete_variable("a").await.unwrap();
    repo.delete_variable("missing").await.unwrap();
    assert_eq!(repo.load_all().await.unwrap(), [("b".to_owned(), Some("3".to_owned()))]);

    db.drop_database().await;
}
