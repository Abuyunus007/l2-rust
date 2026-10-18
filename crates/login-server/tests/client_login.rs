#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! End-to-end client login against a throwaway database: Init, GameGuard, RSA credentials,
//! account checks, server list.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI's `database` job). Never point it at a
//! server with real data: it creates and drops a database named `l2_test_lc_*`.

use std::path::Path;
use std::sync::Arc;

use l2_net::{Decode, PacketReader};
use l2_protocol_login::enums::{AccountKickedReason, LoginFailReason, PlayFailReason};
use l2_protocol_login::server::{AccountKicked, LoginOk, PlayOk};
use l2_testkit::fake_login_client::{FakeClient, login_fail, play_fail};
use l2_testkit::test_db::TestDb;
use login_server::state::ServerStatus;
use login_server::{Paths, Server};
use sqlx::AssertSqlSafe;

/// Three failed logins ban the address; accounts are created on first login.
const SERVER_INI: &str = "LoginserverHostname = 127.0.0.1\nLoginserverPort = 0\nLoginPort = 0\n\
     LoginTryBeforeBan = 3\nLoginBlockAfterBan = 600\nAutoCreateAccounts = True\nShowLicence = True\n";

#[tokio::test]
async fn client_login_flow() {
    let Some(db) = TestDb::create("lc").await else {
        return;
    };
    db.exec("INSERT INTO accounts (login, password, accessLevel) VALUES ('banned', 'x', -1)")
        .await;
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path(), SERVER_INI);
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));
    let server = Server::start(&paths).await.unwrap();
    let addr = server.client_addr().unwrap();

    // 1. A new account is auto-created; LoginOk, then the server list with game server 2.
    let mut first = FakeClient::connect(addr).await;
    first.game_guard().await;
    let reply = first.request_login("Hero", "secret").await;
    assert_eq!(reply[0], LoginOk::OPCODE, "LoginOk");
    let ok = LoginOk::decode(&mut PacketReader::new(&reply[1..])).unwrap();
    let list = first.server_list(&ok).await;
    assert_eq!(list.servers.iter().map(|s| s.server_id).collect::<Vec<_>>(), [2]);
    assert!(!list.servers[0].up, "registered but not connected: down");
    // A wrong login pair is refused.
    let wrong = LoginOk {
        login_ok1: ok.login_ok1 ^ 1,
        login_ok2: ok.login_ok2,
    };
    assert_eq!(
        login_fail(&first.request_server_list(&wrong).await),
        LoginFailReason::AccessFailed
    );

    // 2. Same account while the first client is still logged in: both get "account in use".
    let mut second = FakeClient::connect(addr).await;
    second.game_guard().await;
    assert_eq!(
        login_fail(&second.request_login("hero", "secret").await),
        LoginFailReason::AccountInUse
    );
    assert_eq!(
        login_fail(&first.recv().await.unwrap()),
        LoginFailReason::AccountInUse
    );
    assert!(server.state().clients.get("hero").is_none());

    // 3. Banned account (accessLevel < 0).
    let mut banned = FakeClient::connect(addr).await;
    banned.game_guard().await;
    let reply = banned.request_login("banned", "whatever").await;
    // The password check comes first in Java, so a banned account with a wrong password
    // just gets "wrong user or password".
    assert_eq!(login_fail(&reply), LoginFailReason::UserOrPassWrong);
    db.exec("UPDATE accounts SET password = 'qUqP5cyxm6YcTAhz05Hph5gvu9M=' WHERE login = 'banned'")
        .await; // "test"
    let reply = banned.request_login("banned", "test").await;
    assert_eq!(reply[0], AccountKicked::OPCODE);
    let kicked = AccountKicked::decode(&mut PacketReader::new(&reply[1..])).unwrap();
    assert_eq!(kicked.reason, AccountKickedReason::PermanentlyBanned);
    assert!(
        banned.recv().await.is_none(),
        "AccountKicked closes the connection"
    );

    // 4. LoginTryBeforeBan = 3: the third wrong password bans the address.
    let mut guesser = FakeClient::connect(addr).await;
    guesser.game_guard().await;
    // (The successful "banned" login above reset the counter for this address.)
    for _ in 0..2 {
        assert_eq!(
            login_fail(&guesser.request_login("hero", "nope").await),
            LoginFailReason::UserOrPassWrong
        );
    }
    assert!(!server.state().bans.is_banned("127.0.0.1"));
    assert_eq!(
        login_fail(&guesser.request_login("hero", "nope").await),
        LoginFailReason::UserOrPassWrong
    );
    assert!(server.state().bans.is_banned("127.0.0.1"));

    // 5. A banned address gets LoginFail(NOT_AUTHED) and is closed.
    assert_eq!(
        FakeClient::connect_refused(addr).await,
        LoginFailReason::NotAuthed
    );

    server.shutdown(false).await;
    db.drop_db().await;
}

#[tokio::test]
async fn server_list_and_server_login_flow() {
    let Some(db) = TestDb::create("lc").await else {
        return;
    };
    // "test" hashed; access level 1 makes a GM.
    db.exec(
        "INSERT INTO accounts (login, password, accessLevel, lastServer) \
         VALUES ('gm', 'qUqP5cyxm6YcTAhz05Hph5gvu9M=', 1, 1)",
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path(), SERVER_INI);
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));
    let server = Server::start(&paths).await.unwrap();
    let addr = server.client_addr().unwrap();
    let state = Arc::clone(server.state());

    // Game server 2 (from the gameservers table) connects, GM only for now. There is no link
    // yet (M1-LS-04), so the table is set directly.
    assert!(state.game_servers.update_registered(2, |gs| {
        gs.authed = true;
        gs.status = ServerStatus::GmOnly;
    }));

    let mut player = FakeClient::connect(addr).await;
    let player_ok = player.login_ok("hero", "secret").await;
    let mut gm = FakeClient::connect(addr).await;
    let gm_ok = gm.login_ok("gm", "test").await;

    // GM-only: hidden from the player, shown (up) to the GM, who also gets lastServer = 1.
    assert!(player.server_list(&player_ok).await.servers.is_empty());
    let list = gm.server_list(&gm_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert_eq!(list.servers[0].server_id, 2);
    assert!(list.servers[0].up);
    assert_eq!(list.last_server, 1);
    assert_eq!(list.char_info, None, "no ReplyCharacters, no counts");
    assert_eq!(
        play_fail(&player.server_login(&player_ok, 2).await),
        PlayFailReason::ServerOverloaded
    );

    // Normal, but without room (max players is 0 until the link reports it): only a GM gets in.
    assert!(
        state
            .game_servers
            .update_registered(2, |gs| gs.status = ServerStatus::Normal)
    );
    let list = player.server_list(&player_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert_eq!(
        play_fail(&player.server_login(&player_ok, 2).await),
        PlayFailReason::ServerOverloaded
    );
    // A server that does not exist.
    assert_eq!(
        play_fail(&gm.server_login(&gm_ok, 9).await),
        PlayFailReason::ServerOverloaded
    );
    // A wrong login pair is refused before anything else.
    let wrong = LoginOk {
        login_ok1: gm_ok.login_ok1 ^ 1,
        login_ok2: gm_ok.login_ok2,
    };
    assert_eq!(
        login_fail(&gm.server_login(&wrong, 2).await),
        LoginFailReason::AccessFailed
    );

    // The GM gets PlayOk with the play pair of the session, and lastServer changes.
    let key = state.clients.session_key("gm").unwrap();
    let reply = gm.server_login(&gm_ok, 2).await;
    assert_eq!(reply[0], PlayOk::OPCODE, "PlayOk");
    let play = PlayOk::decode(&mut PacketReader::new(&reply[1..])).unwrap();
    assert_eq!((play.play_ok1, play.play_ok2), (key.play_ok1, key.play_ok2));
    let mut conn = db.connection().await;
    let last_server: i8 = sqlx::query_scalar(AssertSqlSafe(
        "SELECT lastServer FROM accounts WHERE login = 'gm'",
    ))
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(last_server, 2);

    // Login server DOWN: nobody gets to a game server.
    state.set_status(ServerStatus::Down);
    assert_eq!(
        login_fail(&gm.server_login(&gm_ok, 2).await),
        LoginFailReason::AccessFailed
    );

    server.shutdown(false).await;
    db.drop_db().await;
}
