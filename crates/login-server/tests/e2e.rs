#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! The milestone 1 acceptance as a scripted suite: a fake H5 client and a fake game server
//! (both from `l2-testkit`) against a running login server and a throwaway database.
//!
//! Covers (docs/analysis/01-login-server.md, "Acceptance for milestone 1"):
//! - auth success, wrong password, banned account, auto-create (and auto-create off)
//! - IP ban after `LoginTryBeforeBan` failures
//! - account in use between two clients, and kicking the player from the game server
//! - game server registration and its `PlayerAuthRequest`
//! - GM-only server, `RequestServerLogin` with `PlayOk`
//! - a game server that goes away shows as down
//! - the capture comparison: the packets of a recorded session replayed against the login server
//!   (through an in-process `l2-proxy`) have the same types and fields. The recording here comes
//!   from the Rust login server itself; if `fixtures/captures/java-ls-login.jsonl` exists (a
//!   session recorded with the Java login server, see `docs/runbooks/m1-login-test.md`), that
//!   one is replayed too.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI's `database` job). Never point it at a
//! server with real data: each test creates and drops a database named `l2_test_e2e_*`.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use l2_db::{hash_password, string_to_hex};
use l2_protocol_login::enums::{AccountKickedReason, LoginFailReason, PlayFailReason};
use l2_protocol_login::link::{LoginServerFail, LsPacket, ServerStatus as StatusPacket};
use l2_protocol_login::server::{AccountKicked, LoginOk};
use l2_proxy::{CaptureWriter, Channel, Proxy, ProxyConfig, Record};
use l2_testkit::capture_compare::{
    CompareOptions, ReplayAccount, assert_same_shape, compare, drain, load_capture, login_connections, replay,
};
use l2_testkit::fake_game_server::{FakeGameServer, SessionKeys, auth_request};
use l2_testkit::fake_login_client::{FakeClient, login_fail, play_fail};
use l2_testkit::test_db::{SAMPLE_HEXID, TestDb};
use l2_testkit::vectors::workspace_root;
use l2_testkit::wait::until;
use login_server::state::{LoginState, ServerStatus};
use login_server::{Paths, Server};
use sqlx::AssertSqlSafe;
use tokio::sync::mpsc::unbounded_channel;

/// Three failed logins ban the address; accounts are created on first login; no flood
/// protection because the tests reconnect quickly.
const SERVER_INI: &str = "LoginserverHostname = 127.0.0.1\nLoginserverPort = 0\nLoginPort = 0\n\
     LoginTryBeforeBan = 3\nLoginBlockAfterBan = 600\nEnableFloodProtection = False\n\
     AutoCreateAccounts = True\nShowLicence = True\n";

/// A running login server on free ports with a throwaway database.
struct Rig {
    db: TestDb,
    _root: tempfile::TempDir,
    server: Server,
    state: Arc<LoginState>,
    /// Where clients connect.
    client_addr: SocketAddr,
    /// Where game servers connect.
    gs_addr: SocketAddr,
}

impl Rig {
    /// `None` (test skipped) without `L2_TEST_DATABASE_URL`.
    async fn start(server_ini: &str) -> Option<Self> {
        let db = TestDb::create("e2e").await?;
        let root = tempfile::tempdir().unwrap();
        db.write_server_folder(root.path(), server_ini);
        let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));
        let server = Server::start(&paths).await.unwrap();
        let state = Arc::clone(server.state());
        let client_addr = server.client_addr().unwrap();
        let gs_addr = state.game_servers.listen_addr().unwrap();
        Some(Self {
            db,
            _root: root,
            server,
            state,
            client_addr,
            gs_addr,
        })
    }

    /// Creates an account with this password (the SHA-1/Base64 hash, like Java).
    async fn account(&self, login: &str, password: &str, access_level: i32) {
        self.db
            .exec(&format!(
                "INSERT INTO accounts (login, password, accessLevel) VALUES ('{login}', '{}', {access_level})",
                hash_password(password)
            ))
            .await;
    }

    /// A game server that registered as the sample server (id 2) and reported its status the
    /// way the Java game server does.
    async fn game_server(&self, gm_only: bool) -> FakeGameServer {
        let mut gs = FakeGameServer::register_sample(self.gs_addr).await;
        gs.send_java_status(gm_only).await;
        let wanted = if gm_only {
            ServerStatus::GmOnly
        } else {
            ServerStatus::Auto
        };
        until(|| self.state.game_servers.registered(2).unwrap().status == wanted).await;
        gs
    }

    async fn finish(self) {
        self.server.shutdown(false).await;
        self.db.drop_db().await;
    }
}

/// Logs in while a game server is connected: the game server is asked for the characters of the
/// account and answers.
async fn login_with_gs(
    client: &mut FakeClient,
    gs: &mut FakeGameServer,
    user: &str,
    password: &str,
    chars: u8,
) -> LoginOk {
    let ok = client.login_ok(user, password).await;
    gs.answer_characters(&user.to_lowercase(), chars).await;
    ok
}

#[tokio::test]
async fn auth_success_failure_and_auto_create() {
    let Some(rig) = Rig::start(SERVER_INI).await else {
        return;
    };
    rig.account("banned", "test", -1).await;

    // Auto-create: an unknown account is created with the hash of the password it logged in with.
    let mut first = FakeClient::connect(rig.client_addr).await;
    first.login_ok("newbie", "pw").await;
    let mut conn = rig.db.connection().await;
    let stored: String = sqlx::query_scalar(AssertSqlSafe(
        "SELECT password FROM accounts WHERE login = 'newbie'",
    ))
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(stored, hash_password("pw"));
    drop(conn);
    drop(first);
    until(|| rig.state.clients.get("newbie").is_none()).await;

    // A wrong password is refused; the right one then works and resets the failure counter.
    let mut wrong = FakeClient::connect(rig.client_addr).await;
    assert_eq!(
        login_fail(&wrong.login("newbie", "wrong").await),
        LoginFailReason::UserOrPassWrong
    );
    let mut right = FakeClient::connect(rig.client_addr).await;
    right.login_ok("newbie", "pw").await;
    assert_eq!(rig.state.failed_logins("127.0.0.1"), 0);

    // A banned account (access level below 0) is kicked for good and the connection is closed.
    let mut banned = FakeClient::connect(rig.client_addr).await;
    let reply = banned.login("banned", "test").await;
    assert_eq!(reply[0], AccountKicked::OPCODE);
    let kicked =
        <AccountKicked as l2_net::Decode>::decode(&mut l2_net::PacketReader::new(&reply[1..])).unwrap();
    assert_eq!(kicked.reason, AccountKickedReason::PermanentlyBanned);
    banned.expect_closed().await;

    rig.finish().await;
}

#[tokio::test]
async fn unknown_accounts_are_refused_when_auto_create_is_off() {
    let ini = SERVER_INI.replace("AutoCreateAccounts = True", "AutoCreateAccounts = False");
    let Some(rig) = Rig::start(&ini).await else { return };

    let mut client = FakeClient::connect(rig.client_addr).await;
    assert_eq!(
        login_fail(&client.login("nobody", "pw").await),
        LoginFailReason::UserOrPassWrong
    );
    let mut conn = rig.db.connection().await;
    let count: i64 = sqlx::query_scalar(AssertSqlSafe("SELECT COUNT(*) FROM accounts"))
        .fetch_one(&mut conn)
        .await
        .unwrap();
    assert_eq!(count, 0, "no account was created");
    drop(conn);

    rig.finish().await;
}

#[tokio::test]
async fn failed_logins_ban_the_address() {
    let Some(rig) = Rig::start(SERVER_INI).await else {
        return;
    };
    rig.account("victim", "test", 0).await;

    // LoginTryBeforeBan = 3: the third wrong password bans the address.
    let mut guesser = FakeClient::connect(rig.client_addr).await;
    guesser.game_guard().await;
    for attempt in 1..=3 {
        assert_eq!(
            login_fail(&guesser.request_login("victim", "nope").await),
            LoginFailReason::UserOrPassWrong
        );
        assert_eq!(
            rig.state.bans.is_banned("127.0.0.1"),
            attempt == 3,
            "attempt {attempt}"
        );
    }

    // A banned address gets LoginFail(NOT_AUTHED) instead of Init, and is disconnected.
    assert_eq!(
        FakeClient::connect_refused(rig.client_addr).await,
        LoginFailReason::NotAuthed
    );

    // When the ban is lifted the address can log in again.
    assert!(rig.state.bans.remove("127.0.0.1"));
    let mut again = FakeClient::connect(rig.client_addr).await;
    again.login_ok("victim", "test").await;

    rig.finish().await;
}

#[tokio::test]
async fn game_servers_register_and_are_refused() {
    let Some(rig) = Rig::start(SERVER_INI).await else {
        return;
    };

    // The wrong hex id for a known id is refused with reason 3, and the link is closed.
    let mut refused = FakeGameServer::connect(rig.gs_addr).await;
    refused
        .handshake(&l2_testkit::fake_game_server::session_key())
        .await;
    refused.send(&auth_request(2, vec![9, 9, 9], false)).await;
    assert_eq!(
        refused.recv().await,
        LsPacket::LoginServerFail(LoginServerFail {
            reason: LoginServerFail::REASON_WRONG_HEXID
        })
    );
    refused.expect_closed().await;
    assert!(!rig.state.game_servers.registered(2).unwrap().authed);

    // The right hex id registers: AuthResponse with the id and the name from servername.xml.
    let hex_id = string_to_hex(SAMPLE_HEXID).unwrap();
    let (response, mut gs) =
        FakeGameServer::register(rig.gs_addr, &auth_request(2, hex_id.clone(), false)).await;
    assert_eq!(
        (response.server_id, response.server_name.as_str()),
        (2, "Sieghardt")
    );
    assert!(rig.state.game_servers.registered(2).unwrap().authed);

    // A second link for the same id is refused with reason 7; the first one stays.
    let mut second = FakeGameServer::connect(rig.gs_addr).await;
    second
        .handshake(&l2_testkit::fake_game_server::session_key())
        .await;
    second.send(&auth_request(2, hex_id, false)).await;
    assert_eq!(
        second.recv().await,
        LsPacket::LoginServerFail(LoginServerFail {
            reason: LoginServerFail::REASON_ALREADY_LOGGED_IN
        })
    );
    assert!(rig.state.game_servers.registered(2).unwrap().authed);

    // Until the game server reports its status it is shown as down.
    gs.send_java_status(false).await;
    until(|| rig.state.game_servers.registered(2).unwrap().status == ServerStatus::Auto).await;

    rig.finish().await;
}

#[tokio::test]
async fn gm_only_server_server_login_and_the_game_servers_auth_request() {
    let Some(rig) = Rig::start(SERVER_INI).await else {
        return;
    };
    rig.account("gm", "test", 1).await;
    let mut gs = rig.game_server(true).await;

    let mut player = FakeClient::connect(rig.client_addr).await;
    let player_ok = login_with_gs(&mut player, &mut gs, "hero", "secret", 2).await;
    let mut gm = FakeClient::connect(rig.client_addr).await;
    let gm_ok = login_with_gs(&mut gm, &mut gs, "gm", "test", 0).await;

    // GM only: hidden from the player, shown (up) to the GM.
    let list = player.server_list(&player_ok).await;
    assert!(list.servers.is_empty());
    let list = gm.server_list(&gm_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert_eq!(list.servers[0].server_id, 2);
    assert!(list.servers[0].up);
    assert_eq!(list.servers[0].max_players, 300);
    assert_eq!(
        play_fail(&player.server_login(&player_ok, 2).await),
        PlayFailReason::ServerOverloaded
    );

    // The GM gets PlayOk, and the game server accepts exactly the keys of that session.
    let gm_play = gm.play_ok(&gm_ok, 2).await;
    let gm_keys = SessionKeys::new(&gm_ok, &gm_play);
    assert!(
        !gs.auth_player(
            "gm",
            &SessionKeys {
                play_ok1: gm_keys.play_ok1 ^ 1,
                ..gm_keys
            }
        )
        .await
    );
    assert!(
        !gs.auth_player(
            "gm",
            &SessionKeys {
                login_ok2: gm_keys.login_ok2 ^ 1,
                ..gm_keys
            }
        )
        .await
    );
    assert!(rig.state.clients.get("gm").is_some(), "refusals keep the login");
    assert!(gs.auth_player("gm", &gm_keys).await);
    assert!(
        rig.state.clients.get("gm").is_none(),
        "accepted: the game server owns it now"
    );

    // The server opens for everybody: the player sees it (with the character count the game
    // server reported at login) and gets in.
    gs.send_status(StatusPacket::STATUS_AUTO).await;
    until(|| rig.state.game_servers.registered(2).unwrap().status == ServerStatus::Auto).await;
    let list = player.server_list(&player_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert!(list.servers[0].up);
    let chars = list.char_info.expect("the character counts");
    assert_eq!((chars[0].server_id, chars[0].chars), (2, 2));
    let player_play = player.play_ok(&player_ok, 2).await;
    assert!(
        gs.auth_player("hero", &SessionKeys::new(&player_ok, &player_play))
            .await
    );

    // A server that does not exist.
    let mut other = FakeClient::connect(rig.client_addr).await;
    let other_ok = login_with_gs(&mut other, &mut gs, "other", "pw", 0).await;
    assert_eq!(
        play_fail(&other.server_login(&other_ok, 9).await),
        PlayFailReason::ServerOverloaded
    );

    rig.finish().await;
}

#[tokio::test]
async fn account_in_use_on_the_login_server_and_on_a_game_server() {
    let Some(rig) = Rig::start(SERVER_INI).await else {
        return;
    };
    let mut gs = rig.game_server(false).await;

    // Logged in on the login server: the second login and the first client both get
    // "account in use", and nobody stays logged in.
    let mut first = FakeClient::connect(rig.client_addr).await;
    login_with_gs(&mut first, &mut gs, "hero", "secret", 1).await;
    let mut second = FakeClient::connect(rig.client_addr).await;
    assert_eq!(
        login_fail(&second.login("hero", "secret").await),
        LoginFailReason::AccountInUse
    );
    assert_eq!(login_fail(&first.expect().await), LoginFailReason::AccountInUse);
    assert!(rig.state.clients.get("hero").is_none());

    // The player gets into the game server ...
    let mut third = FakeClient::connect(rig.client_addr).await;
    let ok = login_with_gs(&mut third, &mut gs, "hero", "secret", 1).await;
    let play = third.play_ok(&ok, 2).await;
    assert!(gs.auth_player("hero", &SessionKeys::new(&ok, &play)).await);
    gs.players_in_game(&["hero"]).await;
    until(|| rig.state.game_servers.is_account_on_any_gs("hero")).await;

    // ... so a new login is refused and the game server is told to kick the player.
    let mut fourth = FakeClient::connect(rig.client_addr).await;
    assert_eq!(
        login_fail(&fourth.login("hero", "secret").await),
        LoginFailReason::AccountInUse
    );
    let LsPacket::KickPlayer(kick) = gs.recv().await else {
        panic!("expected KickPlayer");
    };
    assert_eq!(kick.account, "hero");

    // When the player leaves the game server, the account is free again.
    gs.player_logout("hero").await;
    until(|| !rig.state.game_servers.is_account_on_any_gs("hero")).await;
    let mut fifth = FakeClient::connect(rig.client_addr).await;
    login_with_gs(&mut fifth, &mut gs, "hero", "secret", 1).await;

    rig.finish().await;
}

#[tokio::test]
async fn a_game_server_that_goes_away_shows_as_down() {
    let Some(rig) = Rig::start(SERVER_INI).await else {
        return;
    };
    let mut gs = rig.game_server(false).await;

    let mut client = FakeClient::connect(rig.client_addr).await;
    let ok = login_with_gs(&mut client, &mut gs, "hero", "secret", 1).await;
    let list = client.server_list(&ok).await;
    assert_eq!(list.servers.len(), 1);
    assert!(list.servers[0].up, "the server is up while the link is");

    drop(gs);
    until(|| !rig.state.game_servers.registered(2).unwrap().authed).await;
    assert_eq!(
        rig.state.game_servers.registered(2).unwrap().status,
        ServerStatus::Down
    );

    // Still in the list (it is registered), but down, and nobody can enter it.
    let list = client.server_list(&ok).await;
    assert_eq!(list.servers.len(), 1);
    assert!(!list.servers[0].up);
    assert_eq!(
        play_fail(&client.server_login(&ok, 2).await),
        PlayFailReason::ServerOverloaded
    );

    // It can come back.
    let mut gs = rig.game_server(false).await;
    let mut again = FakeClient::connect(rig.client_addr).await;
    let ok = login_with_gs(&mut again, &mut gs, "again", "pw", 0).await;
    assert!(again.server_list(&ok).await.servers[0].up);

    rig.finish().await;
}

// ---------------------------------------------------------------------------------------------
// Capture comparison
// ---------------------------------------------------------------------------------------------

/// A proxy in front of the login server, as the GUI or `l2-proxy` runs it, with its records.
async fn proxy_in_front_of(rig: &Rig) -> (Proxy, SocketAddr, tokio::sync::mpsc::UnboundedReceiver<Record>) {
    let (tx, rx) = unbounded_channel();
    let mut config = ProxyConfig::new("127.0.0.1:0".parse().unwrap(), rig.client_addr);
    // Login only, like `l2-proxy --no-rewrite`.
    config.rewrite_server_list = false;
    config.rsa_pool = 1;
    let proxy = Proxy::start(config, tx).await.unwrap();
    let addr = proxy.local_addr();
    (proxy, addr, rx)
}

/// Plays every login connection of `expected` against `addr` and returns what the proxy
/// recorded for it.
async fn replay_recording(
    addr: SocketAddr,
    records: &mut tokio::sync::mpsc::UnboundedReceiver<Record>,
    expected: &[Record],
    account: &ReplayAccount,
) -> Vec<Record> {
    let connections = login_connections(expected);
    for conn in &connections {
        replay(addr, expected, *conn, account).await;
    }
    // The proxy records a packet before it forwards it, so everything the replay read is here.
    drain(records)
}

fn compat_account() -> ReplayAccount {
    ReplayAccount {
        user: "compat".to_owned(),
        password: "test".to_owned(),
        wrong_password: "nope".to_owned(),
    }
}

#[tokio::test]
async fn a_replayed_session_has_the_same_packets_as_its_recording() {
    let Some(rig) = Rig::start(SERVER_INI).await else {
        return;
    };
    rig.account("compat", "test", 0).await;
    let (proxy, proxy_addr, mut records) = proxy_in_front_of(&rig).await;

    // Record: a wrong password, then a full login with the server list and a refused
    // RequestServerLogin (no game server is connected).
    let mut wrong = FakeClient::connect(proxy_addr).await;
    assert_eq!(
        login_fail(&wrong.login("compat", "nope").await),
        LoginFailReason::UserOrPassWrong
    );
    let mut right = FakeClient::connect(proxy_addr).await;
    let ok = right.login_ok("compat", "test").await;
    let list = right.server_list(&ok).await;
    assert!(!list.servers[0].up);
    assert_eq!(
        play_fail(&right.server_login(&ok, 2).await),
        PlayFailReason::ServerOverloaded
    );
    let recorded = drain(&mut records);
    // Log the clients out, or the replay would find the account in use.
    drop(wrong);
    drop(right);
    until(|| rig.state.clients.get("compat").is_none()).await;

    // The recording goes through a JSONL file, like the Java one will.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recorded.jsonl");
    let mut writer = CaptureWriter::create(&path).unwrap();
    for record in &recorded {
        writer.write_record(record).unwrap();
    }
    drop(writer);
    let expected = load_capture(&path);
    assert_eq!(login_connections(&expected).len(), 2);
    let names: Vec<&str> = expected.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"LoginOk") && names.contains(&"LoginFail") && names.contains(&"ServerList"));
    for request in expected.iter().filter(|r| r.name == "RequestAuthLogin") {
        assert_eq!(request.raw.len(), 2, "only the opcode byte is recorded");
        assert_eq!(request.fields.as_ref().unwrap()["credentials"], "<redacted>");
    }

    // Replay it. Session ids, keys and the login/play pairs differ, nothing else does.
    let actual = replay_recording(proxy_addr, &mut records, &expected, &compat_account()).await;
    assert_same_shape(&expected, &actual, Channel::Login, &CompareOptions::default());

    // The comparison does notice a difference: here the server list says the server is up.
    let mut tampered = actual.clone();
    let list = tampered.iter_mut().find(|r| r.name == "ServerList").unwrap();
    list.fields.as_mut().unwrap()["servers"][0]["up"] = serde_json::json!(true);
    let differences = compare(&expected, &tampered, Channel::Login, &CompareOptions::default());
    assert_eq!(differences.len(), 1, "{differences:?}");
    assert!(differences[0].contains("ServerList"));

    proxy.stop().await;
    rig.finish().await;
}

/// The acceptance check against the real thing: a login session recorded with the Java login
/// server (`fixtures/captures/java-ls-login.jsonl`) replayed against the Rust one.
#[tokio::test]
async fn the_rust_login_server_answers_like_the_java_capture() {
    let path = workspace_root().join("fixtures/captures/java-ls-login.jsonl");
    if !path.exists() {
        eprintln!(
            "skipped: {} does not exist yet (record it as described in docs/runbooks/m1-login-test.md)",
            path.display()
        );
        return;
    }
    let expected = load_capture(&path);
    let Some(rig) = Rig::start(SERVER_INI).await else {
        return;
    };
    // The replay needs one account whose password it knows (a capture has no credentials).
    rig.account("compat", "test", 0).await;
    let (proxy, proxy_addr, mut records) = proxy_in_front_of(&rig).await;

    let actual = replay_recording(proxy_addr, &mut records, &expected, &compat_account()).await;
    // The server ids in the list depend on which game servers the Java database has registered.
    let options = CompareOptions::default().ignoring(&["id"]);
    assert_same_shape(&expected, &actual, Channel::Login, &options);

    proxy.stop().await;
    rig.finish().await;
}
