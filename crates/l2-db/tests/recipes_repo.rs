#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `RecipeRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/character_recipebook.sql` and
//! `sql/game/character_recipeshoplist.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{Pool, RecipeBookRow, RecipeRepository, RecipeShopRow, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 2] = [
    include_str!("../sql/game/character_recipebook.sql"),
    include_str!("../sql/game/character_recipeshoplist.sql"),
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

/// A throwaway database with the crafting tables.
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
            "l2_test_recipe_{}_{}_{}",
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

const OWNER: i32 = 268_700_001;

/// `Player.insertNewRecipeData` / `deleteRecipeData` / `restoreRecipeBook`: a dwarven recipe is
/// stored under the class index (type 1), a common one under 0; the delete names the class index.
#[tokio::test]
async fn recipe_book_rows_follow_java() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = RecipeRepository::new(db.pool.clone());
    repo.insert_recipe(OWNER, 20, 1, true).await.unwrap();
    repo.insert_recipe(OWNER, 5, 0, false).await.unwrap();
    repo.insert_recipe(OWNER, 21, 0, true).await.unwrap();
    // Another character's recipe is not read.
    repo.insert_recipe(OWNER + 1, 99, 0, false).await.unwrap();
    let mut rows = repo.recipes_of(OWNER).await.unwrap();
    rows.sort_by_key(|r| r.recipe_id);
    let row = |recipe_id, kind, class_index| RecipeBookRow {
        recipe_id,
        kind,
        class_index,
    };
    assert_eq!(rows, [row(5, 0, 0), row(20, 1, 1), row(21, 1, 0)]);
    // The delete matches the class index too.
    repo.delete_recipe(OWNER, 20, 0).await.unwrap();
    assert_eq!(repo.recipes_of(OWNER).await.unwrap().len(), 3);
    repo.delete_recipe(OWNER, 20, 1).await.unwrap();
    assert_eq!(repo.recipes_of(OWNER).await.unwrap().len(), 2);
    // The key is (id, charId, classIndex): the same row twice is refused (Java logs it).
    assert!(repo.insert_recipe(OWNER, 5, 0, false).await.is_err());
    db.drop_database().await;
}

/// `storeRecipeShopList` / `restoreRecipeShopList`: the old rows go, the new ones get a 1-based
/// `index`, and they come back in that order.
#[tokio::test]
async fn recipe_shop_rows_are_replaced_in_order() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = RecipeRepository::new(db.pool.clone());
    let shop = |recipe_id, price| RecipeShopRow { recipe_id, price };
    repo.store_shop(OWNER, &[shop(9, 100), shop(3, 5_000_000_000), shop(7, 0)])
        .await
        .unwrap();
    assert_eq!(
        repo.shop_of(OWNER).await.unwrap(),
        [shop(9, 100), shop(3, 5_000_000_000), shop(7, 0)]
    );
    let indexes: Vec<(i64,)> = sqlx::query_as(
        "SELECT CAST(`index` AS SIGNED) FROM character_recipeshoplist WHERE charId = ? ORDER BY `index`",
    )
    .bind(OWNER)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(indexes, [(1,), (2,), (3,)]);
    repo.store_shop(OWNER, &[shop(4, 1)]).await.unwrap();
    assert_eq!(repo.shop_of(OWNER).await.unwrap(), [shop(4, 1)]);
    db.drop_database().await;
}
