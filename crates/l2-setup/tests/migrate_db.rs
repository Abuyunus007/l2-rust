#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The migration on a real database: runs only when `L2_TEST_DATABASE_URL` is set (CI's
//! throwaway MariaDB). It works on its own `l2_test_setupmig_*` database and drops it.

use std::path::Path;

use l2_db_installer::installer::{Schema as Side, load_scripts};
use l2_setup::migrate;
use l2_testkit::test_db::TestDb;
use sqlx::Row;

#[tokio::test]
async fn an_old_database_gets_what_it_lacks_and_keeps_its_data() {
    let Some(db) = TestDb::create("setupmig").await else {
        return;
    };
    let sql = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/db_installer/sql");
    let mut scripts = load_scripts(&sql, Side::Login).unwrap();
    scripts.extend(load_scripts(&sql, Side::Game).unwrap());

    // An "old" game database: the characters table without `vitality_points`, one character,
    // and an account_premium row (its script DROPs the table).
    let characters = scripts.iter().find(|s| s.name == "characters.sql").unwrap();
    for statement in &characters.statements {
        if statement.to_ascii_uppercase().starts_with("CREATE TABLE") {
            db.exec(statement).await;
        }
    }
    db.exec("ALTER TABLE `characters` DROP COLUMN `vitality_points`")
        .await;
    db.exec(
        "INSERT INTO `characters` (`account_name`, `charId`, `char_name`) VALUES ('acc', 268435457, 'Hero')",
    )
    .await;
    let premium = scripts.iter().find(|s| s.name == "account_premium.sql").unwrap();
    for statement in &premium.statements {
        if statement.to_ascii_uppercase().starts_with("CREATE TABLE") {
            db.exec(statement).await;
        }
    }
    db.exec("INSERT INTO `account_premium` (`account_name`, `enddate`) VALUES ('acc', 123)")
        .await;

    let mut conn = db.connection().await;
    let existing = migrate::existing_schema(&mut conn, db.name()).await.unwrap();
    assert!(migrate::has_login_tables(&existing) && migrate::has_game_tables(&existing));
    let tables: Vec<String> = scripts
        .iter()
        .filter_map(migrate::script_table)
        .filter(|t| existing.contains_key(t))
        .collect();
    let with_rows = migrate::tables_with_rows(&mut conn, tables).await.unwrap();
    assert!(with_rows.contains("characters") && with_rows.contains("gameservers"));
    let plan = migrate::plan(&scripts, &existing, &with_rows);
    assert!(
        plan.new_columns
            .contains(&("characters".to_owned(), "vitality_points".to_owned()))
    );
    assert!(
        !plan
            .new_tables
            .iter()
            .any(|t| t == "characters" || t == "account_premium" || t == "accounts")
    );
    let applied = migrate::apply(&mut conn, &plan, |sql, error| panic!("{sql}: {error}"))
        .await
        .unwrap();
    assert!(applied.complete());
    // The column came back at its place: right after `vitality_points`' neighbour in the script.
    let order: Vec<String> = sqlx::query_scalar(
        "SELECT COLUMN_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = 'characters' ORDER BY ORDINAL_POSITION",
    )
    .bind(db.name())
    .fetch_all(&mut conn)
    .await
    .unwrap();
    let def = scripts
        .iter()
        .find(|s| s.name == "characters.sql")
        .and_then(|s| s.statements.iter().find_map(|q| migrate::parse_create_table(q)))
        .unwrap();
    let script_order: Vec<String> = def.columns.iter().map(|(c, _)| c.clone()).collect();
    assert_eq!(order, script_order);

    // Everything the scripts define is there now.
    let after = migrate::existing_schema(&mut conn, db.name()).await.unwrap();
    let empty = std::collections::BTreeSet::new();
    assert!(migrate::plan(&scripts, &after, &empty).new_tables.is_empty());
    assert!(migrate::plan(&scripts, &after, &empty).new_columns.is_empty());
    // The data stayed: the character, the premium row, the sample game server.
    let row =
        sqlx::query("SELECT `char_name`, `vitality_points` FROM `characters` WHERE `charId` = 268435457")
            .fetch_one(&mut conn)
            .await
            .unwrap();
    assert_eq!(row.get::<String, _>(0), "Hero");
    let premium: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `account_premium`")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(premium, 1);
    let servers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM `gameservers`")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(servers, 1);

    // A second run changes nothing but the INSERT IGNORE rows.
    let again = migrate::plan(&scripts, &after, &empty);
    assert!(again.new_tables.is_empty() && again.new_columns.is_empty());
    drop(conn);
    db.drop_db().await;
}
