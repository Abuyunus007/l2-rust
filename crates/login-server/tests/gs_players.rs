#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! A fake game server and a fake client against a running login server and a throwaway
//! database: the character counts reach the server list, PlayOk, the game server's
//! `PlayerAuthRequest`, `PlayerInGame` making a second login fail, and the account changes
//! (access level, tracert, temp ban, password).
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI's `database` job). Never point it at a
//! server with real data: it creates and drops a database named `l2_test_gp_*`. The same
//! packets are covered without a database in `src/gs/mod.rs`.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::hash_password;
use l2_protocol_login::enums::LoginFailReason;
use l2_protocol_login::link::{ChangeAccessLevel, ChangePassword, LsPacket, PlayerTracert, RequestTempBan};
use l2_testkit::fake_game_server::{FakeGameServer, SessionKeys};
use l2_testkit::fake_login_client::{FakeClient, login_fail};
use l2_testkit::test_db::{DEFAULT_SERVER_INI, TestDb};
use l2_testkit::wait::until;
use login_server::state::ServerStatus;
use login_server::{Paths, Server};
use sqlx::AssertSqlSafe;

#[tokio::test]
async fn a_player_enters_the_game_through_the_login_server() {
    let Some(db) = TestDb::create("gp").await else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path(), DEFAULT_SERVER_INI);
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));
    let server = Server::start(&paths).await.unwrap();
    let state = std::sync::Arc::clone(server.state());
    let client_addr = server.client_addr().unwrap();
    let mut gs = FakeGameServer::register_sample(state.game_servers.listen_addr().unwrap()).await;

    // What the Java game server does right after AuthResponse: without it the server shows as
    // down (a registered server is DOWN until it reports its status).
    gs.send_java_status(false).await;
    until(|| state.game_servers.registered(2).unwrap().status == ServerStatus::Auto).await;

    // Login: the game server is asked for the characters and answers with 2 (one being deleted).
    let mut hero = FakeClient::connect(client_addr).await;
    let hero_ok = hero.login_ok("hero", "secret").await;
    let LsPacket::RequestCharacters(request) = gs.recv().await else {
        panic!("expected RequestCharacters");
    };
    assert_eq!(request.account, "hero");
    let deletes_at =
        i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() + 3_600_000).unwrap();
    gs.reply_characters("hero", 2, vec![deletes_at]).await;
    let list = hero.server_list(&hero_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert!(list.servers[0].up, "ServerStatus made the server show as up");
    assert_eq!(list.servers[0].max_players, 300);
    let chars = list.char_info.expect("the character counts");
    assert_eq!(chars.len(), 1);
    assert_eq!((chars[0].server_id, chars[0].chars), (2, 2));
    assert_eq!(chars[0].seconds_until_delete.len(), 1);

    // PlayOk, then the game server checks the keys.
    let play = hero.play_ok(&hero_ok, 2).await;
    let keys = SessionKeys::new(&hero_ok, &play);
    let wrong_play = SessionKeys {
        play_ok2: keys.play_ok2 ^ 1,
        ..keys
    };
    assert!(!gs.auth_player("hero", &wrong_play).await);
    let wrong_login = SessionKeys {
        login_ok1: keys.login_ok1 ^ 1,
        ..keys
    };
    assert!(!gs.auth_player("hero", &wrong_login).await);
    assert!(state.clients.get("hero").is_some(), "refusals keep the login");
    assert!(gs.auth_player("hero", &keys).await);
    assert!(
        state.clients.get("hero").is_none(),
        "accepted: the game server owns it now"
    );

    // The player is in the game: a second login is refused and the game server kicks the first.
    gs.players_in_game(&["hero"]).await;
    until(|| state.game_servers.is_account_on_any_gs("hero")).await;
    let mut again = FakeClient::connect(client_addr).await;
    assert_eq!(
        login_fail(&again.login("hero", "secret").await),
        LoginFailReason::AccountInUse
    );
    let LsPacket::KickPlayer(kick) = gs.recv().await else {
        panic!("expected KickPlayer");
    };
    assert_eq!(kick.account, "hero");

    // Account changes go to the database.
    gs.send(&ChangeAccessLevel {
        account: "hero".to_owned(),
        level: 3,
    })
    .await;
    gs.send(&PlayerTracert {
        account: "hero".to_owned(),
        pc_ip: "10.0.0.5".to_owned(),
        hop1: "10.0.0.1".to_owned(),
        hop2: "10.0.1.1".to_owned(),
        hop3: "10.0.2.1".to_owned(),
        hop4: "10.0.3.1".to_owned(),
    })
    .await;
    gs.send(&RequestTempBan {
        account: "hero".to_owned(),
        ip: "10.7.7.7".to_owned(),
        ban_time: 4_102_444_800_000,
        reason: None,
    })
    .await;
    gs.send(&ChangePassword {
        account: "hero".to_owned(),
        character_name: "Hero".to_owned(),
        old_pass: "secret".to_owned(),
        new_pass: "better".to_owned(),
    })
    .await;
    let LsPacket::ChangePasswordResponse(response) = gs.recv().await else {
        panic!("expected ChangePasswordResponse");
    };
    assert_eq!(response.character_name, "Hero");
    assert_eq!(response.message, "You have successfully changed your password!");
    gs.send(&ChangePassword {
        account: "hero".to_owned(),
        character_name: "Hero".to_owned(),
        old_pass: "secret".to_owned(),
        new_pass: "worse".to_owned(),
    })
    .await;
    let LsPacket::ChangePasswordResponse(response) = gs.recv().await else {
        panic!("expected ChangePasswordResponse");
    };
    assert_eq!(
        response.message,
        "The typed current password doesn't match with your current one."
    );

    // The packets were handled in order, so everything above is stored by now.
    let mut conn = db.connection().await;
    let (access, password, pc_ip, hop4): (i8, String, String, String) = sqlx::query_as(AssertSqlSafe(
        "SELECT accessLevel, password, pcIp, hop4 FROM accounts WHERE login = 'hero'",
    ))
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(access, 3);
    assert_eq!(password, hash_password("better"));
    assert_eq!((pc_ip.as_str(), hop4.as_str()), ("10.0.0.5", "10.0.3.1"));
    let ban: String = sqlx::query_scalar(AssertSqlSafe(
        "SELECT value FROM account_data WHERE account_name = 'hero' AND var = 'ban_temp'",
    ))
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(ban, "4102444800000");
    assert!(state.bans.is_banned("10.7.7.7"));

    // The player leaves: the account is free again for the login server.
    gs.player_logout("hero").await;
    until(|| !state.game_servers.is_account_on_any_gs("hero")).await;

    drop(conn);
    server.shutdown(false).await;
    db.drop_db().await;
}
