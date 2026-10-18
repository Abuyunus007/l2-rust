#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Starts the login server against a throwaway database and shuts it down again.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service,
//! e.g. `mysql://root@127.0.0.1:3306/`). Never point it at a server with real data: the test
//! only creates and drops a database named `l2_test_boot_*`, but it needs the rights to do so.

use std::path::Path;

use l2_db::string_to_hex;
use l2_testkit::test_db::{SAMPLE_HEXID, TestDb};
use login_server::bans::PERMANENT_BAN;
use login_server::state::{ExitReason, ServerStatus};
use login_server::{Paths, Server};

/// Port 0 for both listeners, so tests never fight over 2106 / 9013.
const SERVER_INI: &str =
    "LoginRestartSchedule = True\nLoginRestartTime = 6\nLoginserverPort = 0\nLoginPort = 0\n";

#[tokio::test]
async fn starts_reaches_ready_and_shuts_down_with_exit_code_0() {
    // A fresh database with the login schema and one registered game server (id 2).
    let Some(db) = TestDb::create("boot").await else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path(), SERVER_INI);
    std::fs::write(
        root.path().join("banned_ip.cfg"),
        "# test bans\n10.255.255.255\n10.254.0.0 3600000  # an hour\nnot-a-duration 12x\n",
    )
    .unwrap();
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));

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
        string_to_hex(SAMPLE_HEXID).unwrap()
    );
    assert_eq!(state.bans.len(), 2, "the invalid line is skipped");
    assert_eq!(state.bans.expiry("10.255.255.255"), Some(PERMANENT_BAN));
    assert!(state.bans.is_banned("10.254.7.7"));
    assert_eq!(state.status(), ServerStatus::Normal);
    assert!(state.random_client_key_pair().is_some());

    // The pool really talks to the database.
    let games = state.repository.game_servers().await.unwrap();
    assert_eq!(games.len(), 1);

    let result = server.shutdown(false).await;
    db.drop_db().await;

    assert_eq!(result, ExitReason::Shutdown);
    assert_eq!(result.code(), 0);
}
