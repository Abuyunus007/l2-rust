#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Starts the login server against a throwaway database and shuts it down again.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service,
//! e.g. `mysql://root@127.0.0.1:3306/`). Never point it at a server with real data: the test
//! only creates and drops a database named `l2_test_ls_*`, but it needs the rights to do so.

use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::string_to_hex;
use login_server::bans::PERMANENT_BAN;
use login_server::state::{ExitReason, ServerStatus};
use login_server::{Paths, Server};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 4] = [
    include_str!("../../l2-db/sql/login/account_data.sql"),
    include_str!("../../l2-db/sql/login/accounts.sql"),
    include_str!("../../l2-db/sql/login/accounts_ipauth.sql"),
    include_str!("../../l2-db/sql/login/gameservers.sql"),
];

/// `(user, password, host, port)` of a `mysql://user[:password]@host[:port]/` URL.
fn parse_url(url: &str) -> (String, String, String, u16) {
    let rest = url.strip_prefix("mysql://").expect("mysql:// URL");
    let rest = rest.split('/').next().unwrap();
    let (credentials, host_port) = rest.rsplit_once('@').unwrap_or(("root", rest));
    let (user, password) = credentials.split_once(':').unwrap_or((credentials, ""));
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, "3306"));
    (
        user.to_owned(),
        password.to_owned(),
        host.to_owned(),
        port.parse().unwrap(),
    )
}

/// A server folder like `dist/login`, with the files this test needs.
fn write_server_folder(root: &Path, database: &str, url: &str) {
    let (user, password, host, port) = parse_url(url);
    let config = root.join("config");
    let data = root.join("data");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(
        config.join("Database.ini"),
        format!(
            "URL = jdbc:mysql://{host}:{port}/{database}?useSSL=false\nLogin = {user}\nPassword = {password}\nMaximumDatabaseConnections = 4\n"
        ),
    )
    .unwrap();
    std::fs::write(
        config.join("Server.ini"),
        "LoginRestartSchedule = True\nLoginRestartTime = 6\nLoginserverPort = 0\n",
    )
    .unwrap();
    std::fs::write(
        data.join("servername.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<servers_list>
	<server id="1" name="Bartz" />
	<server id="2" name="Sieghardt" />
	<server id="3" name="Kain" />
</servers_list>"#,
    )
    .unwrap();
    std::fs::write(
        root.join("banned_ip.cfg"),
        "# test bans\n10.255.255.255\n10.254.0.0 3600000  # an hour\nnot-a-duration 12x\n",
    )
    .unwrap();
}

#[tokio::test]
async fn starts_reaches_ready_and_shuts_down_with_exit_code_0() {
    let Ok(url) = std::env::var("L2_TEST_DATABASE_URL") else {
        eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
        return;
    };
    if url.trim().is_empty() {
        eprintln!("skipped: L2_TEST_DATABASE_URL is empty");
        return;
    }

    // A fresh database with the login schema and one registered game server.
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let database = format!("l2_test_ls_{}_{}", std::process::id(), nanos % 1_000_000_000);
    assert!(database.starts_with("l2_test_ls_"));
    let mut admin = MySqlConnection::connect_with(&MySqlConnectOptions::from_str(&url).unwrap())
        .await
        .unwrap();
    sqlx::query(AssertSqlSafe(format!(
        "CREATE DATABASE `{database}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    )))
    .execute(&mut admin)
    .await
    .unwrap();
    // The schema files also insert the sample game server (id 2).
    let mut setup =
        MySqlConnection::connect_with(&MySqlConnectOptions::from_str(&url).unwrap().database(&database))
            .await
            .unwrap();
    for file in SCHEMA {
        for statement in file.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            sqlx::query(AssertSqlSafe(statement))
                .execute(&mut setup)
                .await
                .unwrap();
        }
    }
    setup.close().await.unwrap();

    let root = tempfile::tempdir().unwrap();
    write_server_folder(root.path(), &database, &url);
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));

    let result = async {
        let server = Server::start(&paths).await.unwrap();
        let state = server.state();

        assert_eq!(state.client_keys.pairs.len(), 10);
        assert_eq!(state.client_keys.blowfish.len(), 20);
        assert_eq!(state.game_servers.key_pairs().len(), 10);
        assert_eq!(state.game_servers.server_names().len(), 3);
        assert_eq!(state.game_servers.server_name(2), Some("Sieghardt"));
        assert_eq!(state.game_servers.registered_ids(), [2]);
        assert_eq!(
            state.game_servers.registered(2).unwrap().hex_id,
            string_to_hex("-2ad66b3f483c22be097019f55c8abdf0").unwrap()
        );
        assert_eq!(state.bans.len(), 2, "the invalid line is skipped");
        assert_eq!(state.bans.expiry("10.255.255.255"), Some(PERMANENT_BAN));
        assert!(state.bans.is_banned("10.254.7.7"));
        assert_eq!(state.status(), ServerStatus::Normal);
        assert!(state.random_client_key_pair().is_some());

        // The pool really talks to the database.
        let games = state.repository.game_servers().await.unwrap();
        assert_eq!(games.len(), 1);

        server.shutdown(false).await
    }
    .await;

    sqlx::query(AssertSqlSafe(format!("DROP DATABASE `{database}`")))
        .execute(&mut admin)
        .await
        .unwrap();

    assert_eq!(result, ExitReason::Shutdown);
    assert_eq!(result.code(), 0);
}
