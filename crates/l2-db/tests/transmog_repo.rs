#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of the transmog rows (`character_transmogs`, the `transmogId` of
//! `item_variables`) against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux). It creates and drops a database named `l2_test_transmog_*` only.

use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{TransmogRepository, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 2] = [
    l2_db::game_sql!("character_transmogs"),
    l2_db::game_sql!("item_variables"),
];

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

#[tokio::test]
async fn the_collection_and_the_item_transmogs_round_trip() {
    let Ok(url) = std::env::var("L2_TEST_DATABASE_URL") else {
        eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
        return;
    };
    if url.trim().is_empty() {
        eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
        return;
    }
    let admin_options = MySqlConnectOptions::from_str(&url).unwrap();
    let mut admin = MySqlConnection::connect_with(&admin_options).await.unwrap();
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let name = format!(
        "l2_test_transmog_{}_{}",
        std::process::id(),
        nanos % 1_000_000_000
    );
    assert!(name.starts_with("l2_test_"));
    sqlx::query(AssertSqlSafe(format!(
        "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    )))
    .execute(&mut admin)
    .await
    .unwrap();
    let pool = connect_with(admin_options.database(&name), 5).await.unwrap();
    for file in SCHEMA {
        for statement in statements(file) {
            sqlx::query(AssertSqlSafe(statement))
                .execute(&pool)
                .await
                .unwrap();
        }
    }

    let repo = TransmogRepository::new(pool.clone());
    // The collection of an account (the owner is text): saved, loaded in ascending order, and
    // saving it again keeps one row per item.
    assert!(repo.load_collection("acc").await.unwrap().is_empty());
    repo.save_collection("acc", &[6408, 57, 2375]).await.unwrap();
    repo.save_collection("acc", &[57]).await.unwrap();
    repo.save_collection("268500001", &[1]).await.unwrap();
    assert_eq!(repo.load_collection("acc").await.unwrap(), [57, 2375, 6408]);
    assert_eq!(repo.load_collection("268500001").await.unwrap(), [1]);

    // The transmog id of an item: set, read, changed, removed.
    assert!(
        repo.load_item_transmogs(&[268_600_001, 268_600_002])
            .await
            .unwrap()
            .is_empty()
    );
    repo.set_item_transmog(268_600_001, 2375).await.unwrap();
    repo.set_item_transmog(268_600_002, 6408).await.unwrap();
    repo.set_item_transmog(268_600_001, 57).await.unwrap();
    assert_eq!(
        repo.load_item_transmogs(&[268_600_001, 268_600_002, 268_600_003])
            .await
            .unwrap(),
        [(268_600_001, 57), (268_600_002, 6408)]
    );
    repo.remove_item_transmog(268_600_001).await.unwrap();
    assert_eq!(
        repo.load_item_transmogs(&[268_600_001, 268_600_002])
            .await
            .unwrap(),
        [(268_600_002, 6408)]
    );

    pool.close().await;
    sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{name}`")))
        .execute(&mut admin)
        .await
        .unwrap();
}
