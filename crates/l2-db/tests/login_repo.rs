#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration tests of `LoginRepository` against a real MySQL/MariaDB server.
//!
//! They run only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the tests only create and drop databases named `l2_test_*`, but they need the rights
//! to do so. Each test gets a fresh database built from `sql/login/*.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_config::{DatabaseConfig, IniFile};
use l2_db::{
    ChangePasswordResult, IpAuthKind, LoginRepository, Pool, TracertHops, connect, connect_with,
    hash_password, hex_to_string, string_to_hex,
};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 4] = [
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/login/account_data.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/login/accounts.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/login/accounts_ipauth.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/login/gameservers.sql"
    )),
];

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A throwaway database with the login schema.
struct TestDb {
    admin: MySqlConnection,
    options: MySqlConnectOptions,
    url: String,
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
            "l2_test_{}_{}_{}",
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

        let options = admin_options.database(&name);
        let pool = connect_with(options.clone(), 10).await.unwrap();
        for file in SCHEMA {
            for statement in file.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(AssertSqlSafe(statement))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        Some(Self {
            admin,
            options,
            url,
            name,
            pool,
        })
    }

    fn repo(&self) -> LoginRepository {
        LoginRepository::new(self.pool.clone())
    }

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
}

const NOW: i64 = 1_700_000_000_000;

#[tokio::test]
async fn account_create_fetch_and_updates() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    let hash = hash_password("secret");

    assert_eq!(repo.account_info("alice", NOW).await.unwrap(), None);

    repo.create_account("alice", &hash, NOW, 0, "10.0.0.1")
        .await
        .unwrap();
    let info = repo.account_info("alice", NOW).await.unwrap().unwrap();
    assert_eq!(info.login, "alice");
    assert_eq!(info.password.as_deref(), Some(hash.as_str()));
    assert_eq!(info.access_level, 0);
    assert_eq!(info.last_server_or_zero(), 1, "column default");

    // A second insert with the same login hits the primary key.
    assert!(
        repo.create_account("alice", &hash, NOW, 0, "10.0.0.1")
            .await
            .is_err()
    );

    repo.update_last_active("alice", NOW + 5, "10.0.0.2")
        .await
        .unwrap();
    let (lastactive, last_ip): (u64, String) =
        sqlx::query_as("SELECT lastactive, lastIP FROM accounts WHERE login = 'alice'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(lastactive, (NOW + 5) as u64);
    assert_eq!(last_ip, "10.0.0.2");

    repo.update_last_server("alice", 7).await.unwrap();
    repo.update_access_level("alice", 100).await.unwrap();
    let info = repo.account_info("alice", NOW).await.unwrap().unwrap();
    assert_eq!(info.last_server, Some(7));
    assert_eq!(info.access_level, 100);

    repo.update_access_level("alice", -100).await.unwrap();
    let info = repo.account_info("alice", NOW).await.unwrap().unwrap();
    assert_eq!(info.access_level, -100);

    let hops = TracertHops {
        pc_ip: Some("192.168.0.2".into()),
        hops: [
            Some("192.168.0.1".into()),
            Some("10.1.1.1".into()),
            None,
            Some("8.8.8.8".into()),
        ],
    };
    repo.update_tracert("alice", &hops).await.unwrap();
    // NULL shows up as "-" so a single text column can carry all five.
    let row: String = sqlx::query_scalar(
        "SELECT CONCAT_WS(',', IFNULL(pcIp, '-'), IFNULL(hop1, '-'), IFNULL(hop2, '-'), \
         IFNULL(hop3, '-'), IFNULL(hop4, '-')) FROM accounts WHERE login = 'alice'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(row, "192.168.0.2,192.168.0.1,10.1.1.1,-,8.8.8.8");

    db.drop_database().await;
}

#[tokio::test]
async fn temp_ban_overrides_access_level_until_it_expires() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    repo.create_account("bob", &hash_password("x"), NOW, 50, "10.0.0.1")
        .await
        .unwrap();
    assert_eq!(
        repo.account_info("bob", NOW).await.unwrap().unwrap().access_level,
        50
    );

    // Ban ends in the future: effective level -1, the stored level is untouched.
    let future = NOW + 3_600_000;
    repo.temp_ban("bob", future).await.unwrap();
    assert_eq!(
        repo.account_info("bob", NOW).await.unwrap().unwrap().access_level,
        -1
    );
    let stored: i8 = sqlx::query_scalar("SELECT accessLevel FROM accounts WHERE login = 'bob'")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(stored, 50);

    // Once `now` is past the ban time the account is back to its own level.
    assert_eq!(
        repo.account_info("bob", future + 1)
            .await
            .unwrap()
            .unwrap()
            .access_level,
        50
    );

    // A new ban replaces the old row (ON DUPLICATE KEY UPDATE).
    repo.temp_ban("bob", NOW + 10).await.unwrap();
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT account_name, var, value FROM account_data")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![("bob".into(), "ban_temp".into(), Some((NOW + 10).to_string()))]
    );
    assert_eq!(
        repo.account_info("bob", NOW + 11)
            .await
            .unwrap()
            .unwrap()
            .access_level,
        50
    );

    // Other accounts are not affected by bob's ban.
    repo.create_account("carol", &hash_password("x"), NOW, 0, "10.0.0.1")
        .await
        .unwrap();
    assert_eq!(
        repo.account_info("carol", NOW)
            .await
            .unwrap()
            .unwrap()
            .access_level,
        0
    );

    db.drop_database().await;
}

// Java binds the time as text and compares it with a VARCHAR column, so this is the real
// (quirky) behaviour: a ban time with fewer digits compares "greater" than any later time
// that starts with a smaller digit.
#[tokio::test]
async fn ban_temp_comparison_is_textual_like_java() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    repo.create_account("dave", &hash_password("x"), NOW, 0, "10.0.0.1")
        .await
        .unwrap();
    // "9" > "1700000000000" as text, so a ban that "ended" in 1970 still bans.
    repo.temp_ban("dave", 9).await.unwrap();
    assert_eq!(
        repo.account_info("dave", NOW)
            .await
            .unwrap()
            .unwrap()
            .access_level,
        -1
    );
    db.drop_database().await;
}

#[tokio::test]
async fn ip_auth_rows() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    assert!(repo.ip_auth("erin").await.unwrap().is_empty());

    sqlx::query(
        "INSERT INTO accounts_ipauth (login, ip, type) VALUES \
         ('erin', '1.2.3.4', 'allow'), ('erin', '5.6.7.8', 'deny'), ('erin', '9.9.9.9', DEFAULT), \
         ('frank', '1.1.1.1', 'deny')",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    let mut rows = repo.ip_auth("erin").await.unwrap();
    rows.sort_by(|a, b| a.ip.cmp(&b.ip));
    let summary: Vec<_> = rows.iter().map(|r| (r.ip.as_str(), r.kind())).collect();
    assert_eq!(
        summary,
        vec![
            ("1.2.3.4", Some(IpAuthKind::Allow)),
            ("5.6.7.8", Some(IpAuthKind::Deny)),
            ("9.9.9.9", Some(IpAuthKind::Allow)), // column default
        ]
    );
    assert!(rows.iter().all(|r| r.login == "erin"));

    db.drop_database().await;
}

#[tokio::test]
async fn game_servers_register_and_list() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();

    // `gameservers.sql` ships one row.
    let rows = repo.game_servers().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].server_id, 2);
    assert_eq!(rows[0].hexid, "-2ad66b3f483c22be097019f55c8abdf0");
    assert_eq!(rows[0].host, "");

    let hex_bytes = vec![0x8f, 0x01, 0x02, 0x03];
    let hexid = hex_to_string(&hex_bytes).unwrap();
    repo.register_game_server(3, &hexid, "play.example.org")
        .await
        .unwrap();

    // Same id again: primary key violation, nothing overwritten.
    assert!(repo.register_game_server(3, "1", "other").await.is_err());

    let mut rows = repo.game_servers().await.unwrap();
    rows.sort_by_key(|r| r.server_id);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].server_id, 3);
    assert_eq!(rows[1].host, "play.example.org");
    assert_eq!(string_to_hex(&rows[1].hexid).unwrap(), hex_bytes);

    db.drop_database().await;
}

#[tokio::test]
async fn change_password_outcomes() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();
    let old = hash_password("old");
    let new = hash_password("new");
    repo.create_account("gina", &old, NOW, 0, "10.0.0.1")
        .await
        .unwrap();

    // Wrong current password: nothing changes.
    assert_eq!(
        repo.change_password("gina", &new, &new).await.unwrap(),
        ChangePasswordResult::CurrentMismatch
    );
    // Unknown account.
    assert_eq!(
        repo.change_password("nobody", &old, &new).await.unwrap(),
        ChangePasswordResult::CurrentMismatch
    );
    assert_eq!(
        repo.account_info("gina", NOW).await.unwrap().unwrap().password,
        Some(old.clone())
    );

    assert_eq!(
        repo.change_password("gina", &old, &new).await.unwrap(),
        ChangePasswordResult::Changed
    );
    assert_eq!(
        repo.account_info("gina", NOW).await.unwrap().unwrap().password,
        Some(new.clone())
    );

    // Setting the same password again still matches a row, like Java (found-rows semantics).
    assert_eq!(
        repo.change_password("gina", &new, &new).await.unwrap(),
        ChangePasswordResult::Changed
    );

    db.drop_database().await;
}

#[tokio::test]
async fn connect_from_database_config() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    // Rebuild a `Database.ini` for the throwaway database from the test URL's parts.
    let ini = format!(
        "URL = jdbc:mysql://{}:{}/{}?useSSL=false\nLogin = {}\nPassword = {}\nMaximumDatabaseConnections = 5\n",
        db.options.get_host(),
        db.options.get_port(),
        db.name,
        db.options.get_username(),
        password_of(&db.url),
    );
    let config = DatabaseConfig::from_ini(&IniFile::parse("Database.ini", &ini).unwrap());
    let pool = connect(&config).await.unwrap();
    assert_eq!(pool.options().get_max_connections(), 5);
    let one: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&pool).await.unwrap();
    assert_eq!(one, 1);
    pool.close().await;

    db.drop_database().await;
}

#[tokio::test]
async fn account_admin_queries() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = db.repo();

    for (login, level) in [("ann", 0), ("anna", 100), ("a_b", 0), ("bob", -1)] {
        repo.insert_account(login, &hash_password("pw"), level)
            .await
            .unwrap();
    }
    assert!(
        repo.insert_account("ann", "x", 0)
            .await
            .unwrap_err()
            .is_duplicate_key()
    );
    assert!(repo.account_exists("bob").await.unwrap());
    assert!(!repo.account_exists("zed").await.unwrap());
    assert_eq!(repo.access_level("anna").await.unwrap(), Some(100));
    assert_eq!(repo.access_level("zed").await.unwrap(), None);

    // Paging and prefix (the underscore is literal, not "any character").
    let (all, total) = repo.search_accounts("", 2, 0).await.unwrap();
    assert_eq!((all.len(), total), (2, 4));
    assert_eq!(all[0].login, "a_b");
    let (rest, _) = repo.search_accounts("", 2, 2).await.unwrap();
    assert_eq!(
        rest.iter().map(|r| r.login.as_str()).collect::<Vec<_>>(),
        ["anna", "bob"]
    );
    let (ann, total) = repo.search_accounts("an", 50, 0).await.unwrap();
    assert_eq!((ann.len(), total), (2, 2));
    let (under, total) = repo.search_accounts("a_", 50, 0).await.unwrap();
    assert_eq!((under.len(), total), (1, 1));
    assert_eq!(ann[0].last_server, Some(1), "column default");
    assert_eq!(ann[0].lastactive, 0);

    repo.set_password("ann", &hash_password("new")).await.unwrap();
    let info = repo.account_info("ann", NOW).await.unwrap().unwrap();
    assert_eq!(info.password.as_deref(), Some(hash_password("new").as_str()));

    assert_eq!(repo.last_ip("ann").await.unwrap(), Some(None));
    repo.update_last_active("ann", NOW, "10.0.0.9").await.unwrap();
    assert_eq!(repo.last_ip("ann").await.unwrap(), Some(Some("10.0.0.9".into())));
    assert_eq!(repo.last_ip("zed").await.unwrap(), None);

    repo.delete_account("ann").await.unwrap();
    assert!(!repo.account_exists("ann").await.unwrap());

    db.drop_database().await;
}

/// Password part of `mysql://user:password@host...` (empty when there is none).
fn password_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let credentials = rest.rsplit_once('@').map_or("", |(c, _)| c);
    credentials.split_once(':').map_or("", |(_, p)| p).to_owned()
}
