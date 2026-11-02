#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The load test tool against the real Rust login server and game server in one process, both on
//! port 0 on loopback (the real ports never open), with 20 bots for a few seconds.
//!
//! Both servers need a database, so this runs only when `L2_TEST_DATABASE_URL` is set (CI's
//! throwaway MariaDB, as in `crates/game-server/tests/e2e.rs`) and is skipped with a printed
//! message otherwise. It creates and drops its own database named `l2_test_loadtest_*`.
//! The test datapack is the e2e one.

#[path = "../../game-server/tests/e2e/datapack.rs"]
mod datapack;

use std::path::Path;
use std::sync::Arc;

use game_server::{Paths, Server, hexid};
use l2_loadtest::options::Options;
use l2_loadtest::report::{self, Verdict};
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
use login_server::state::ServerStatus;

use clap::Parser;

const GAME_SCHEMA: &[&str] = l2_db::GAME_SCHEMA;
const ADMIN_TOKEN: &str = "loadtest-token";

fn game_server_ini(ls_port: u16) -> String {
    format!(
        "GameserverHostname = 127.0.0.1\nGameserverPort = 0\nLoginHost = 127.0.0.1\nLoginPort = {ls_port}\n\
         PacketEncryption = True\nDatapackRoot = ./datapack\nMaximumOnlineUsers = 100\n"
    )
}

fn write_game_config(config_dir: &Path, ls_port: u16) {
    std::fs::create_dir_all(config_dir).unwrap();
    std::fs::write(config_dir.join("Server.ini"), game_server_ini(ls_port)).unwrap();
    std::fs::write(config_dir.join("Player.ini"), "ForbiddenNames = annoy\n").unwrap();
    std::fs::write(config_dir.join("General.ini"), "MinimumChatLevel = 0\n").unwrap();
    std::fs::write(
        config_dir.join("FloodProtector.ini"),
        datapack::no_flood_protection_ini(),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("ipconfig.xml"),
        "<gameserver address=\"127.0.0.1\" />\n",
    )
    .unwrap();
    // The admin endpoint the tool samples, on a free port of loopback.
    std::fs::write(
        config_dir.join("Admin.ini"),
        format!("Enabled = True\nBind = 127.0.0.1:0\nToken = {ADMIN_TOKEN}\n"),
    )
    .unwrap();
    hexid::save(config_dir, SAMPLE_SERVER_ID, SAMPLE_HEXID);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn twenty_bots_play_for_a_few_seconds() {
    let Some(db) = TestDb::create("loadtest").await else {
        return;
    };
    for file in GAME_SCHEMA {
        let text: String = file
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        for statement in text.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            db.exec(statement).await;
        }
    }

    let ls_root = tempfile::tempdir().unwrap();
    db.write_server_folder(ls_root.path(), DEFAULT_SERVER_INI);
    let ls = login_server::Server::start(&login_server::Paths::resolve(
        ls_root.path(),
        Path::new("./config"),
        Path::new("./data"),
    ))
    .await
    .unwrap();
    let ls_state = Arc::clone(ls.state());
    let ls_client = ls.client_addr().unwrap();
    let link_port = ls_state.game_servers.listen_addr().unwrap().port();

    let gs_root = tempfile::tempdir().unwrap();
    db.write_server_folder(gs_root.path(), &game_server_ini(link_port));
    write_game_config(&gs_root.path().join("config"), link_port);
    datapack::write_datapack(&gs_root.path().join("datapack"));
    let gs = Server::start(&Paths::resolve(gs_root.path(), Path::new("./config")))
        .await
        .unwrap();
    let admin_addr = gs.admin_addr().unwrap();

    // The game server registers by itself; the bots can log in once it shows as up.
    for _ in 0..1_200 {
        if ls_state
            .game_servers
            .registered(SAMPLE_SERVER_ID)
            .is_some_and(|server| server.authed && server.status == ServerStatus::Auto)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    let options = Options::parse_from([
        "l2-loadtest",
        "--login",
        &ls_client.to_string(),
        "--count",
        "20",
        "--ramp",
        "20",
        "--duration",
        "8",
        "--admin",
        &admin_addr.to_string(),
        "--admin-token",
        ADMIN_TOKEN,
        "--sample-interval",
        "1",
    ]);
    let outcome = l2_loadtest::run::run(&options).await.unwrap();
    println!("{}", report::summary_table(&outcome));

    assert_eq!(outcome.error_table, [], "no bot failed");
    assert_eq!(outcome.disconnects, 0);
    assert_eq!(outcome.started, 20);
    assert_eq!(outcome.logged_in, 20);
    assert!(outcome.entered >= 20, "every bot entered the world");
    assert!(outcome.packets_rx > 0);
    assert!(outcome.timings.login.is_some() && outcome.timings.enter.is_some());
    // The admin endpoint reported the measured tick time.
    assert!(!outcome.samples.is_empty());
    assert!(
        outcome
            .samples
            .iter()
            .filter_map(|s| s.admin.as_ref())
            .any(|a| a.tick_samples > 0 && a.online > 0),
        "{:?}",
        outcome.samples
    );
    assert_ne!(outcome.verdict(), Verdict::Unchecked);

    gs.shutdown(false).await;
    ls.shutdown(false).await;
    db.drop_db().await;
}
