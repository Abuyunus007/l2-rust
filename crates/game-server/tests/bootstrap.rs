#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Starts the game server against a throwaway database and shuts it down again.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service,
//! e.g. `mysql://root@127.0.0.1:3306/`). Never point it at a server with real data: the test
//! only creates and drops a database named `l2_test_gsboot_*`, but it needs the rights to do so.
//! Both listeners use port 0, so the real game port never opens.

use std::path::Path;
use std::time::Duration;

use game_server::state::ExitReason;
use game_server::{Paths, Server, StartupError};
use l2_admin_api::{AdminClient, Request, Response};
use l2_testkit::test_db::{SAMPLE_HEXID, TestDb};
use l2_world::{ObjectId, RUST_FIRST_OBJECT_ID};
use tokio::net::TcpStream;

/// The id tables the startup scan reads (Java `DatabaseIdManager`).
const ID_TABLES: [&str; 5] = [
    include_str!("../../l2-db/sql/game/characters.sql"),
    include_str!("../../l2-db/sql/game/items.sql"),
    include_str!("../../l2-db/sql/game/clan_data.sql"),
    include_str!("../../l2-db/sql/game/itemsonground.sql"),
    include_str!("../../l2-db/sql/game/messages.sql"),
];

/// Port 0, loopback only, and a datapack folder relative to the server root.
const SERVER_INI: &str =
    "GameserverHostname = 127.0.0.1\nGameserverPort = 0\nDatapackRoot = ./datapack\nMaximumOnlineUsers = 5\n";

/// A database with the login schema (from `TestDb`) plus the id tables of the game schema, and
/// a server folder pointing at it.
async fn setup(tag: &str, root: &Path, admin_ini: Option<&str>) -> Option<TestDb> {
    let db = TestDb::create(tag).await?;
    for file in ID_TABLES {
        for statement in file.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            db.exec(statement).await;
        }
    }
    db.write_server_folder(root, SERVER_INI);
    std::fs::create_dir_all(root.join("datapack").join("data")).unwrap();
    if let Some(admin) = admin_ini {
        std::fs::write(root.join("config").join("Admin.ini"), admin).unwrap();
    }
    Some(db)
}

fn paths(root: &Path) -> Paths {
    Paths::resolve(root, Path::new("./config"))
}

#[tokio::test]
async fn starts_reaches_ready_and_shuts_down_with_exit_code_0() {
    let root = tempfile::tempdir().unwrap();
    let Some(db) = setup("gsboot", root.path(), None).await else {
        return;
    };
    // Ids in the Rust range (two tables, one id twice), one of Java's range, and the registration.
    db.exec("INSERT INTO messages (messageId) VALUES (1610612741)")
        .await;
    db.exec("INSERT INTO itemsonground (object_id) VALUES (1610612741), (1610612742)")
        .await;
    db.exec("INSERT INTO itemsonground (object_id) VALUES (268435500)")
        .await;
    std::fs::write(
        root.path().join("config").join("hexid.txt"),
        format!("HexID={SAMPLE_HEXID}\nServerID=7\n"),
    )
    .unwrap();

    let server = Server::start(&paths(root.path())).await.unwrap();
    let state = server.state();

    // The listener really is bound (port 0 -> a free port) and accepts connections.
    let addr = server.client_addr().unwrap();
    assert!(addr.ip().is_loopback());
    assert_ne!(addr.port(), 0);
    TcpStream::connect(addr).await.unwrap();

    // Ids: only the two Rust-range ids count, Java's is left alone.
    assert_eq!(state.ids.used_count(), 2);
    assert!(state.ids.is_allocated(ObjectId(RUST_FIRST_OBJECT_ID + 5)));
    assert!(!state.ids.is_allocated(ObjectId(268_435_500)));
    assert_eq!(state.ids.available_count(), 0x2000_0000 - 2);

    assert!(state.datapack.class_list.is_empty());
    assert_eq!(state.config.server.maximum_online_users, 5);
    assert!(state.paths.config_dir.ends_with("config"));

    let hexid = state.hexid.as_ref().unwrap();
    assert_eq!(hexid.server_id, 7);
    assert_eq!(l2_db::hex_to_string(&hexid.hex_id).unwrap(), SAMPLE_HEXID);

    // The pool really talks to the database, and the startup did not change anything in it:
    // D-39, no cleanup queries.
    let rows: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM itemsonground")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(rows.0, 3);

    let result = server.shutdown(false).await;
    db.drop_db().await;

    assert_eq!(result, ExitReason::Shutdown);
    assert_eq!(result.code(), 0);
}

#[tokio::test]
async fn missing_hexid_file_still_starts() {
    let root = tempfile::tempdir().unwrap();
    let Some(db) = setup("gsnohex", root.path(), None).await else {
        return;
    };
    let server = Server::start(&paths(root.path())).await.unwrap();
    assert!(server.state().hexid.is_none());
    assert_eq!(server.state().ids.used_count(), 0);
    server.shutdown(false).await;
    db.drop_db().await;
}

#[tokio::test]
async fn admin_shutdown_request_ends_the_server_with_exit_code_2() {
    let root = tempfile::tempdir().unwrap();
    let Some(db) = setup(
        "gsadmin",
        root.path(),
        Some("Enabled = True\nBind = 127.0.0.1:0\nToken = test-token\n"),
    )
    .await
    else {
        return;
    };
    let server = Server::start(&paths(root.path())).await.unwrap();
    let admin = server.admin_addr().unwrap();

    let (client, _events) = AdminClient::connect(&format!("ws://{admin}"), "test-token")
        .await
        .unwrap();
    let reply = client.request(Request::Shutdown { restart: true }).await.unwrap();
    assert_eq!(reply, Response::Ok);

    let reason = tokio::time::timeout(
        Duration::from_secs(30),
        server.run_until(std::future::pending::<()>()),
    )
    .await
    .unwrap();
    db.drop_db().await;
    assert_eq!(reason, ExitReason::Restart);
    assert_eq!(reason.code(), 2);
}

#[tokio::test]
async fn a_missing_datapack_folder_is_a_startup_error() {
    let root = tempfile::tempdir().unwrap();
    let Some(db) = setup("gsnodata", root.path(), None).await else {
        return;
    };
    std::fs::remove_dir_all(root.path().join("datapack")).unwrap();
    let error = Server::start(&paths(root.path())).await.err().unwrap();
    db.drop_db().await;
    assert!(matches!(error, StartupError::Datapack(_)), "{error}");
}
