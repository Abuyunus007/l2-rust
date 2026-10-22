#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `MessageRepository` (`messages`) against a real MySQL/MariaDB server.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{MessageRepository, MessageRow, Pool, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: &str = include_str!("../sql/game/messages.sql");

static COUNTER: AtomicU32 = AtomicU32::new(0);

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
            "l2_test_messages_{}_{}_{}",
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
        sqlx::query(AssertSqlSafe(SCHEMA.trim().trim_end_matches(';').to_owned()))
            .execute(&pool)
            .await
            .unwrap();
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

fn mail(message_id: i32, expiration: i64) -> MessageRow {
    MessageRow {
        message_id,
        sender_id: 268_500_001,
        receiver_id: 268_500_002,
        subject: "Hello".to_owned(),
        content: "A letter".to_owned(),
        expiration,
        req_adena: 5_000,
        has_attachments: true,
        is_unread: true,
        is_deleted_by_sender: false,
        is_deleted_by_receiver: false,
        send_by_system: 0,
        is_returned: false,
    }
}

#[tokio::test]
async fn a_mail_is_stored_loaded_and_changed_like_java_does() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = MessageRepository::new(db.pool.clone());
    assert!(repo.load_all().await.unwrap().is_empty());

    // The load is ordered by expiration, not by id.
    repo.insert(&mail(1_610_612_800, 2_000)).await.unwrap();
    repo.insert(&mail(1_610_612_801, 1_000)).await.unwrap();
    let rows = repo.load_all().await.unwrap();
    assert_eq!(
        rows.iter().map(|r| r.message_id).collect::<Vec<_>>(),
        vec![1_610_612_801, 1_610_612_800]
    );
    assert_eq!(rows[1], mail(1_610_612_800, 2_000));

    // The flags the packets change, one statement each.
    repo.mark_read(1_610_612_800).await.unwrap();
    repo.mark_deleted_by_sender(1_610_612_800).await.unwrap();
    repo.mark_deleted_by_receiver(1_610_612_800).await.unwrap();
    repo.remove_attachments(1_610_612_800).await.unwrap();
    let rows = repo.load_all().await.unwrap();
    let changed = rows.iter().find(|r| r.message_id == 1_610_612_800).unwrap();
    assert!(!changed.is_unread);
    assert!(changed.is_deleted_by_sender);
    assert!(changed.is_deleted_by_receiver);
    assert!(!changed.has_attachments);
    let untouched = rows.iter().find(|r| r.message_id == 1_610_612_801).unwrap();
    assert_eq!(untouched, &mail(1_610_612_801, 1_000));

    // A mail the system sent: sender -1, `sendBySystem` kept.
    let mut system = mail(1_610_612_802, 3_000);
    system.sender_id = -1;
    system.send_by_system = 2;
    system.is_deleted_by_sender = true;
    repo.insert(&system).await.unwrap();
    let rows = repo.load_all().await.unwrap();
    assert_eq!(rows.iter().find(|r| r.message_id == 1_610_612_802).unwrap(), &system);

    repo.delete(1_610_612_800).await.unwrap();
    assert_eq!(repo.load_all().await.unwrap().len(), 2);

    db.drop_database().await;
}
