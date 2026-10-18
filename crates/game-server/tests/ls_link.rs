#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The game server's link to the login server.
//!
//! Most tests talk to a small fake login server in this file (no database, port 0, loopback
//! only). The last one starts the real `login-server` crate in-process and runs only when
//! `L2_TEST_DATABASE_URL` is set (CI's throwaway MariaDB); it creates and drops a database named
//! `l2_test_gslink_*`. Neither opens the real login server ports.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use game_server::hexid::{self, HexId};
use game_server::ls_link::{
    CharacterCounts, IpConfig, KickRequest, LsLink, LsLinkParams, SessionKey, server_type_id,
};
use l2_config::GameConfig;
use l2_crypto::LinkCipher;
use l2_crypto::rsa::{RsaKeyPair, strip_leading_zeros};
use l2_db::{hex_to_string, string_to_hex};
use l2_net::{CancellationToken, Encode, PacketReader};
use l2_protocol_login::link::{
    AuthResponse, BlowFishKey, GameServerAuth, GsPacket, InitLS, KickPlayer, LinkState, LoginServerFail,
    PlayerAuthResponse, PlayerInGame, PlayerLogout, ReplyCharacters, RequestCharacters, ServerStatus,
    decode_from_gs,
};
use l2_testkit::wait::until;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(10);
/// Long enough that the link has seen a disconnect before it connects again.
const RECONNECT: Duration = Duration::from_millis(300);

// ------------------------------------------------------------------------------------------
// A fake login server
// ------------------------------------------------------------------------------------------

struct FakeLoginServer {
    listener: TcpListener,
    keys: RsaKeyPair,
}

impl FakeLoginServer {
    async fn start() -> Self {
        Self {
            listener: TcpListener::bind("127.0.0.1:0").await.unwrap(),
            keys: RsaKeyPair::generate(512, &mut rand::rng()).unwrap(),
        }
    }

    fn address(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }

    /// Accepts the game server and sends `InitLS` with `revision`.
    async fn accept_with_revision(&self, revision: i32) -> LsConn {
        let (stream, _) = timeout(WAIT, self.listener.accept())
            .await
            .expect("the game server should connect")
            .unwrap();
        let mut conn = LsConn {
            stream,
            cipher: LinkCipher::default_key().unwrap(),
        };
        conn.send(&InitLS {
            protocol_revision: revision,
            public_key: self.keys.java_to_byte_array_modulus(),
        })
        .await;
        conn
    }

    /// Accepts the game server and does the key exchange. Returns the `AuthRequest`.
    async fn accept_and_key_exchange(&self) -> (LsConn, GameServerAuth) {
        let mut conn = self
            .accept_with_revision(l2_protocol_login::link::PROTOCOL_REV)
            .await;
        let GsPacket::BlowFishKey(BlowFishKey { encrypted_key }) = conn.recv(LinkState::Connected).await
        else {
            panic!("expected BlowFishKey");
        };
        let block = self.keys.decrypt_raw(&encrypted_key).unwrap();
        let session_key = strip_leading_zeros(&block).to_vec();
        assert_eq!(session_key.len(), 40, "a 40 byte Blowfish key");
        conn.cipher = LinkCipher::new(&session_key).unwrap();
        let GsPacket::GameServerAuth(auth) = conn.recv(LinkState::BfConnected).await else {
            panic!("expected GameServerAuth");
        };
        (conn, auth)
    }

    /// Accepts, exchanges the key and registers the game server as `server_id`.
    async fn accept_and_register(&self, server_id: u8) -> (LsConn, GameServerAuth) {
        let (mut conn, auth) = self.accept_and_key_exchange().await;
        conn.send(&AuthResponse {
            server_id,
            server_name: "Test".to_owned(),
        })
        .await;
        (conn, auth)
    }
}

struct LsConn {
    stream: TcpStream,
    cipher: LinkCipher,
}

impl LsConn {
    async fn send<P: Encode>(&mut self, packet: &P) {
        let body = self.cipher.seal(&packet.to_bytes());
        let mut frame = u16::try_from(body.len() + 2).unwrap().to_le_bytes().to_vec();
        frame.extend_from_slice(&body);
        self.stream.write_all(&frame).await.unwrap();
    }

    /// The next decrypted frame, `None` when the game server closed the link or `wait` passed.
    async fn frame(&mut self, wait: Duration) -> Option<Vec<u8>> {
        let mut header = [0u8; 2];
        timeout(wait, self.stream.read_exact(&mut header))
            .await
            .ok()?
            .ok()?;
        let mut body = vec![0u8; usize::from(u16::from_le_bytes(header)).checked_sub(2)?];
        timeout(wait, self.stream.read_exact(&mut body))
            .await
            .ok()?
            .ok()?;
        Some(self.cipher.open(&mut body).unwrap().to_vec())
    }

    async fn recv_opt(&mut self, state: LinkState, wait: Duration) -> Option<GsPacket> {
        let plain = self.frame(wait).await?;
        Some(
            decode_from_gs(state, plain[0], &mut PacketReader::new(&plain[1..]))
                .unwrap()
                .expect("a packet"),
        )
    }

    async fn recv(&mut self, state: LinkState) -> GsPacket {
        self.recv_opt(state, WAIT)
            .await
            .expect("a packet from the game server")
    }

    /// The next packet of a registered game server.
    async fn next(&mut self) -> GsPacket {
        self.recv(LinkState::Authed).await
    }

    async fn expect_status(&mut self) -> Vec<(i32, i32)> {
        match self.next().await {
            GsPacket::ServerStatus(status) => status.attributes,
            other => panic!("expected ServerStatus, got {other:?}"),
        }
    }

    async fn expect_logout(&mut self, account: &str) {
        match self.next().await {
            GsPacket::PlayerLogout(PlayerLogout { account: got }) => assert_eq!(got, account),
            other => panic!("expected PlayerLogout of {account}, got {other:?}"),
        }
    }

    async fn expect_in_game(&mut self, accounts: &[&str]) {
        match self.next().await {
            GsPacket::PlayerInGame(PlayerInGame { accounts: got }) => assert_eq!(got, accounts),
            other => panic!("expected PlayerInGame {accounts:?}, got {other:?}"),
        }
    }

    /// True when nothing but the end of the connection arrives.
    async fn expect_closed(&mut self) {
        assert!(
            self.frame(WAIT).await.is_none(),
            "the game server should have closed the link"
        );
    }
}

// ------------------------------------------------------------------------------------------
// Setup
// ------------------------------------------------------------------------------------------

/// A `CharacterCounts` with fixed answers.
struct FakeCharacters {
    times: Result<Vec<u64>, String>,
    asked: AtomicUsize,
}

impl FakeCharacters {
    fn new(times: Result<Vec<u64>, String>) -> Arc<Self> {
        Arc::new(Self {
            times,
            asked: AtomicUsize::new(0),
        })
    }
}

impl CharacterCounts for FakeCharacters {
    fn delete_times<'a>(
        &'a self,
        _account: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u64>, String>> + Send + 'a>> {
        Box::pin(async move {
            self.asked.fetch_add(1, Ordering::SeqCst);
            self.times.clone()
        })
    }
}

fn params(address: SocketAddr, config_dir: &Path, hexid: Option<&HexId>) -> LsLinkParams {
    let mut params = LsLinkParams::from_config(
        &GameConfig::default(),
        server_type_id("Normal"),
        IpConfig::parse(
            r#"<gameserver address="203.0.113.7"><define subnet="10.0.0.0/8" address="10.1.1.1"/></gameserver>"#,
        ),
        hexid,
        config_dir.to_path_buf(),
    );
    params.host = "127.0.0.1".to_owned();
    params.port = address.port();
    params.reconnect_delay = RECONNECT;
    params
}

fn spawn_link(
    params: LsLinkParams,
    characters: Arc<dyn CharacterCounts>,
) -> (LsLink, tokio::task::JoinHandle<()>) {
    let (link, task) = LsLink::new(params, characters, CancellationToken::new());
    (link, tokio::spawn(task))
}

/// A link that is registered at a fake login server.
struct Rig {
    server: FakeLoginServer,
    conn: LsConn,
    link: LsLink,
    task: tokio::task::JoinHandle<()>,
    /// Holds `hexid.txt`; must outlive the link.
    _config_dir: tempfile::TempDir,
    auth: GameServerAuth,
}

impl Rig {
    async fn new() -> Self {
        Self::with_characters(FakeCharacters::new(Ok(Vec::new()))).await
    }

    async fn with_characters(characters: Arc<dyn CharacterCounts>) -> Self {
        let server = FakeLoginServer::start().await;
        let config_dir = tempfile::tempdir().unwrap();
        let (link, task) = spawn_link(params(server.address(), config_dir.path(), None), characters);
        let (mut conn, auth) = server.accept_and_register(3).await;
        // What the Java game server sends right after `AuthResponse`.
        conn.expect_status().await;
        until(|| link.status().registered).await;
        Self {
            server,
            conn,
            link,
            task,
            _config_dir: config_dir,
            auth,
        }
    }
}

const KEY: SessionKey = SessionKey {
    login_ok1: 11,
    login_ok2: 12,
    play_ok1: 13,
    play_ok2: 14,
};

// ------------------------------------------------------------------------------------------
// Registration
// ------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_new_game_server_registers_with_a_generated_secret_and_saves_hexid_txt() {
    let server = FakeLoginServer::start().await;
    let config_dir = tempfile::tempdir().unwrap();
    let (link, _task) = spawn_link(
        params(server.address(), config_dir.path(), None),
        FakeCharacters::new(Ok(Vec::new())),
    );
    let (mut conn, auth) = server.accept_and_key_exchange().await;

    let defaults = GameConfig::default().server;
    assert_eq!(i32::from(auth.desired_id), defaults.request_id);
    assert_eq!(auth.accept_alternate, defaults.accept_alternate_id);
    assert!(!auth.reserve_host);
    assert_eq!(auth.max_players, defaults.maximum_online_users);
    assert_eq!(auth.hex_id.len(), 16);
    assert!(auth.hex_id.iter().all(|&byte| byte != 0));
    assert_eq!(auth.subnets, ["10.0.0.0/8", "0.0.0.0/0"]);
    assert_eq!(auth.hosts, ["10.1.1.1", "203.0.113.7"]);
    assert!(!link.status().registered, "not registered before AuthResponse");

    conn.send(&AuthResponse {
        server_id: 3,
        server_name: "Kain".to_owned(),
    })
    .await;
    // Java: no brackets, the configured type, status auto, age (as attribute 5, see the module).
    assert_eq!(
        conn.expect_status().await,
        vec![
            (ServerStatus::SERVER_LIST_SQUARE_BRACKET, ServerStatus::OFF),
            (ServerStatus::SERVER_TYPE, ServerStatus::SERVER_NORMAL),
            (ServerStatus::SERVER_LIST_STATUS, ServerStatus::STATUS_AUTO),
            (ServerStatus::TEST_SERVER, ServerStatus::SERVER_AGE_ALL),
        ]
    );
    until(|| link.status().registered).await;
    let status = link.status();
    assert_eq!((status.server_id, status.server_name.as_str()), (Some(3), "Kain"));

    // `hexid.txt` now holds the assigned id and the secret that was sent.
    let saved = hexid::load(config_dir.path()).expect("hexid.txt was written");
    assert_eq!(saved.server_id, 3);
    assert_eq!(saved.hex_id, auth.hex_id);
    assert_eq!(
        hex_to_string(&saved.hex_id).unwrap(),
        hex_to_string(&auth.hex_id).unwrap()
    );
}

#[tokio::test]
async fn an_existing_hexid_decides_the_id_and_the_secret() {
    let server = FakeLoginServer::start().await;
    let config_dir = tempfile::tempdir().unwrap();
    let hexid = HexId {
        server_id: 2,
        hex_id: string_to_hex("-2ad66b3f483c22be097019f55c8abdf0").unwrap(),
    };
    let (_link, _task) = spawn_link(
        params(server.address(), config_dir.path(), Some(&hexid)),
        FakeCharacters::new(Ok(Vec::new())),
    );
    let (_conn, auth) = server.accept_and_key_exchange().await;
    assert_eq!(auth.desired_id, 2);
    assert_eq!(auth.hex_id, hexid.hex_id);
}

#[tokio::test]
async fn a_refusal_is_not_fatal_and_the_link_tries_again() {
    let server = FakeLoginServer::start().await;
    let config_dir = tempfile::tempdir().unwrap();
    let (link, _task) = spawn_link(
        params(server.address(), config_dir.path(), None),
        FakeCharacters::new(Ok(Vec::new())),
    );
    let (mut conn, first) = server.accept_and_key_exchange().await;
    conn.send(&LoginServerFail {
        reason: LoginServerFail::REASON_WRONG_HEXID,
    })
    .await;
    drop(conn);
    // Nothing is saved for a refused registration, and the same secret is offered again.
    let (_conn, second) = server.accept_and_key_exchange().await;
    assert_eq!(second.hex_id, first.hex_id);
    assert!(!link.status().registered);
    assert!(hexid::load(config_dir.path()).is_none());
}

#[tokio::test]
async fn a_protocol_revision_mismatch_leaves_the_link_silent() {
    let server = FakeLoginServer::start().await;
    let config_dir = tempfile::tempdir().unwrap();
    let (link, _task) = spawn_link(
        params(server.address(), config_dir.path(), None),
        FakeCharacters::new(Ok(Vec::new())),
    );
    let mut conn = server.accept_with_revision(0x0107).await;
    assert!(
        conn.recv_opt(LinkState::Connected, Duration::from_millis(500))
            .await
            .is_none(),
        "Java only warns and sends nothing"
    );
    assert!(!link.status().registered);
}

#[tokio::test]
async fn without_a_login_server_players_are_refused_and_the_link_keeps_trying() {
    // A port nobody listens on: bind, note the address, close.
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let config_dir = tempfile::tempdir().unwrap();
    let (link, _task) = spawn_link(
        params(closed, config_dir.path(), None),
        FakeCharacters::new(Ok(Vec::new())),
    );
    assert!(!link.auth_player("hero", KEY).await);
    // Logging out and the status calls are harmless.
    link.player_logout("hero").await;
    link.set_server_status(ServerStatus::STATUS_FULL).await.unwrap();
    until(|| link.status().server_status == ServerStatus::STATUS_FULL).await;
}

// ------------------------------------------------------------------------------------------
// Players
// ------------------------------------------------------------------------------------------

#[tokio::test]
async fn auth_round_trip_accepts_and_refuses_like_java() {
    let mut rig = Rig::new().await;

    // Accepted: PlayerAuthRequest goes out, PlayerInGame follows the answer.
    let link = rig.link.clone();
    let hero = tokio::spawn(async move { link.auth_player("hero", KEY).await });
    let GsPacket::PlayerAuthRequest(request) = rig.conn.next().await else {
        panic!("expected PlayerAuthRequest");
    };
    assert_eq!(request.account, "hero");
    assert_eq!(
        (
            request.login_ok1,
            request.login_ok2,
            request.play_ok1,
            request.play_ok2
        ),
        (11, 12, 13, 14)
    );
    rig.conn
        .send(&PlayerAuthResponse {
            account: "hero".to_owned(),
            authed: true,
        })
        .await;
    assert!(hero.await.unwrap());
    rig.conn.expect_in_game(&["hero"]).await;

    // The same account again is a duplicate login: refused without asking the login server.
    assert!(!rig.link.auth_player("hero", KEY).await);

    // Refused keys: the client is closed and the login server hears PlayerLogout.
    let link = rig.link.clone();
    let villain = tokio::spawn(async move { link.auth_player("villain", KEY).await });
    let GsPacket::PlayerAuthRequest(request) = rig.conn.next().await else {
        panic!("expected PlayerAuthRequest");
    };
    assert_eq!(request.account, "villain");
    rig.conn
        .send(&PlayerAuthResponse {
            account: "villain".to_owned(),
            authed: false,
        })
        .await;
    assert!(!villain.await.unwrap());
    rig.conn.expect_logout("villain").await;

    // After a logout the account may come back.
    rig.link.player_logout("hero").await;
    rig.conn.expect_logout("hero").await;
    let link = rig.link.clone();
    let again = tokio::spawn(async move { link.auth_player("hero", KEY).await });
    let GsPacket::PlayerAuthRequest(request) = rig.conn.next().await else {
        panic!("expected PlayerAuthRequest");
    };
    assert_eq!(request.account, "hero");
    rig.conn
        .send(&PlayerAuthResponse {
            account: "hero".to_owned(),
            authed: true,
        })
        .await;
    assert!(again.await.unwrap());
}

#[tokio::test]
async fn player_in_game_and_logout_reach_the_login_server_in_order() {
    let mut rig = Rig::new().await;
    rig.link.player_in_game("anna").await;
    rig.link.player_logout("anna").await;
    rig.link.player_logout("never-seen").await;
    rig.conn.expect_in_game(&["anna"]).await;
    rig.conn.expect_logout("anna").await;
    rig.conn.expect_logout("never-seen").await;
}

#[tokio::test]
async fn a_kick_goes_to_the_session_registry_and_waits_for_the_logout() {
    let mut rig = Rig::new().await;
    let mut kicks = rig.link.take_kicks().expect("the first taker gets the receiver");
    assert!(rig.link.take_kicks().is_none(), "only one registry");

    let link = rig.link.clone();
    let hero = tokio::spawn(async move { link.auth_player("hero", KEY).await });
    rig.conn.next().await; // PlayerAuthRequest
    rig.conn
        .send(&PlayerAuthResponse {
            account: "hero".to_owned(),
            authed: true,
        })
        .await;
    assert!(hero.await.unwrap());
    rig.conn.expect_in_game(&["hero"]).await;

    rig.conn
        .send(&KickPlayer {
            account: "hero".to_owned(),
        })
        .await;
    let kick = timeout(WAIT, kicks.recv()).await.unwrap().unwrap();
    assert_eq!(
        kick,
        KickRequest {
            account: "hero".to_owned()
        }
    );
    // Nothing is sent until the registry says the player is gone (the character is stored).
    rig.link.player_logout("hero").await;
    rig.conn.expect_logout("hero").await;

    // An account this game server does not have is released at once, as Java does.
    rig.conn
        .send(&KickPlayer {
            account: "ghost".to_owned(),
        })
        .await;
    rig.conn.expect_logout("ghost").await;
    assert!(kicks.try_recv().is_err());
}

#[tokio::test]
async fn without_a_session_registry_a_kick_is_answered_with_a_logout() {
    let mut rig = Rig::new().await;
    let link = rig.link.clone();
    let hero = tokio::spawn(async move { link.auth_player("hero", KEY).await });
    rig.conn.next().await;
    rig.conn
        .send(&PlayerAuthResponse {
            account: "hero".to_owned(),
            authed: true,
        })
        .await;
    assert!(hero.await.unwrap());
    rig.conn.expect_in_game(&["hero"]).await;
    rig.conn
        .send(&KickPlayer {
            account: "hero".to_owned(),
        })
        .await;
    rig.conn.expect_logout("hero").await;
}

#[tokio::test]
async fn request_characters_is_answered_from_the_character_source() {
    let mut rig = Rig::with_characters(FakeCharacters::new(Ok(vec![0, 1_234, 0, 5_678]))).await;
    rig.conn
        .send(&RequestCharacters {
            account: "hero".to_owned(),
        })
        .await;
    // Java: every row counts, only those with a delete time are listed.
    assert_eq!(
        rig.conn.next().await,
        GsPacket::ReplyCharacters(ReplyCharacters {
            account: "hero".to_owned(),
            chars: 4,
            delete_times: vec![1_234, 5_678],
        })
    );

    // And by hand.
    rig.link.reply_characters("anna", 2, vec![99]).await;
    assert_eq!(
        rig.conn.next().await,
        GsPacket::ReplyCharacters(ReplyCharacters {
            account: "anna".to_owned(),
            chars: 2,
            delete_times: vec![99],
        })
    );
}

#[tokio::test]
async fn a_failing_character_source_gives_an_empty_answer() {
    let characters = FakeCharacters::new(Err("boom".to_owned()));
    let mut rig = Rig::with_characters(Arc::clone(&characters) as Arc<dyn CharacterCounts>).await;
    rig.conn
        .send(&RequestCharacters {
            account: "hero".to_owned(),
        })
        .await;
    assert_eq!(
        rig.conn.next().await,
        GsPacket::ReplyCharacters(ReplyCharacters {
            account: "hero".to_owned(),
            chars: 0,
            delete_times: vec![],
        })
    );
    assert_eq!(characters.asked.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn status_calls_reach_the_login_server() {
    let mut rig = Rig::new().await;
    rig.link
        .set_server_status(ServerStatus::STATUS_FULL)
        .await
        .unwrap();
    assert_eq!(
        rig.conn.expect_status().await,
        vec![(ServerStatus::SERVER_LIST_STATUS, ServerStatus::STATUS_FULL)]
    );
    assert!(rig.link.set_server_status(42).await.is_err());
    rig.link.set_max_player(50).await;
    assert_eq!(
        rig.conn.expect_status().await,
        vec![(ServerStatus::MAX_PLAYERS, 50)]
    );
    rig.link
        .send_server_status(ServerStatus::SERVER_TYPE, ServerStatus::SERVER_FREE)
        .await;
    assert_eq!(
        rig.conn.expect_status().await,
        vec![(ServerStatus::SERVER_TYPE, ServerStatus::SERVER_FREE)]
    );
    let status = rig.link.status();
    assert_eq!(
        (status.server_status, status.max_players),
        (ServerStatus::STATUS_FULL, 50)
    );
}

// ------------------------------------------------------------------------------------------
// Reconnect and shutdown
// ------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_link_reconnects_resends_the_online_accounts_and_fails_waiting_clients() {
    let mut rig = Rig::new().await;

    // One account is in the game, one is waiting for its answer when the link breaks.
    let link = rig.link.clone();
    let hero = tokio::spawn(async move { link.auth_player("hero", KEY).await });
    rig.conn.next().await;
    rig.conn
        .send(&PlayerAuthResponse {
            account: "hero".to_owned(),
            authed: true,
        })
        .await;
    assert!(hero.await.unwrap());
    rig.conn.expect_in_game(&["hero"]).await;
    let link = rig.link.clone();
    let late = tokio::spawn(async move { link.auth_player("late", KEY).await });
    let GsPacket::PlayerAuthRequest(request) = rig.conn.next().await else {
        panic!("expected PlayerAuthRequest");
    };
    assert_eq!(request.account, "late");

    let Rig {
        server,
        conn,
        link,
        auth: first_auth,
        ..
    } = rig;
    drop(conn);
    assert!(!late.await.unwrap(), "the answer can no longer come");
    until(|| !link.status().registered).await;
    assert!(!link.auth_player("other", KEY).await, "no link, no session");

    // After the delay the link connects again, asks for the same id and secret (Java keeps
    // both), and tells the login server who is online.
    let (mut conn, second_auth) = server.accept_and_register(3).await;
    assert_eq!(second_auth, first_auth);
    conn.expect_status().await;
    conn.expect_in_game(&["hero"]).await;
    until(|| link.status().registered).await;

    // `late` never got in: it can try again.
    let again = tokio::spawn({
        let link = link.clone();
        async move { link.auth_player("late", KEY).await }
    });
    let GsPacket::PlayerAuthRequest(request) = conn.next().await else {
        panic!("expected PlayerAuthRequest");
    };
    assert_eq!(request.account, "late");
    conn.send(&PlayerAuthResponse {
        account: "late".to_owned(),
        authed: true,
    })
    .await;
    assert!(again.await.unwrap());
}

#[tokio::test]
async fn an_account_that_left_while_the_link_was_down_is_not_resent() {
    let mut rig = Rig::new().await;
    rig.link.player_in_game("anna").await;
    rig.link.player_in_game("bob").await;
    rig.conn.expect_in_game(&["anna"]).await;
    rig.conn.expect_in_game(&["bob"]).await;

    let Rig {
        server, conn, link, ..
    } = rig;
    drop(conn);
    until(|| !link.status().registered).await;
    link.player_logout("anna").await;

    let (mut conn, _) = server.accept_and_register(3).await;
    conn.expect_status().await;
    conn.expect_in_game(&["bob"]).await;
}

#[tokio::test]
async fn shutdown_reports_the_server_down_and_closes_the_link() {
    let mut rig = Rig::new().await;
    rig.link.shutdown().await;
    assert_eq!(
        rig.conn.expect_status().await,
        vec![(ServerStatus::SERVER_LIST_STATUS, ServerStatus::STATUS_DOWN)]
    );
    rig.conn.expect_closed().await;
    timeout(WAIT, &mut rig.task).await.unwrap().unwrap();
    // The link is gone: calls are refused or ignored, never hang.
    assert!(!rig.link.auth_player("hero", KEY).await);
    rig.link.player_logout("hero").await;
    rig.link.shutdown().await;
}

// ------------------------------------------------------------------------------------------
// The real login server (database)
// ------------------------------------------------------------------------------------------

fn gs_config_dir_with_hexid(server_id: i32, hex: &str) -> (tempfile::TempDir, PathBuf, HexId) {
    let dir = tempfile::tempdir().unwrap();
    hexid::save(dir.path(), server_id, hex);
    let path = dir.path().to_path_buf();
    let loaded = hexid::load(&path).expect("hexid.txt");
    (dir, path, loaded)
}

#[tokio::test]
async fn the_game_server_works_with_the_real_login_server() {
    use l2_protocol_login::enums::LoginFailReason;
    use l2_testkit::fake_login_client::{FakeClient, login_fail};
    use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
    use login_server::state::ServerStatus as ListStatus;

    let Some(db) = TestDb::create("gslink").await else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path(), DEFAULT_SERVER_INI);
    let paths = login_server::Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));
    let server = login_server::Server::start(&paths).await.unwrap();
    let state = Arc::clone(server.state());
    let client_addr = server.client_addr().unwrap();
    let ls_address = state.game_servers.listen_addr().unwrap();

    // The game server is the sample server of `gameservers.sql` (id 2), with its hexid.txt.
    let (_dir, config_dir, hexid) = gs_config_dir_with_hexid(SAMPLE_SERVER_ID, SAMPLE_HEXID);
    let deletes_at =
        i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() + 3_600_000).unwrap();
    let characters = FakeCharacters::new(Ok(vec![0, u64::try_from(deletes_at).unwrap()]));
    let (link, _task) = spawn_link(params(ls_address, &config_dir, Some(&hexid)), characters);
    let mut kicks = link.take_kicks().unwrap();

    // Registered, and the status made the server show as up.
    until(|| {
        state
            .game_servers
            .registered(SAMPLE_SERVER_ID)
            .is_some_and(|gs| gs.authed && gs.status == ListStatus::Auto)
    })
    .await;
    assert_eq!(link.status().server_id, Some(2));

    // Wrong keys are refused. Java then sends PlayerLogout, which also drops the login the
    // login server still holds for the account.
    let mut villain = FakeClient::connect(client_addr).await;
    let villain_ok = villain.login_ok("villain", "secret").await;
    let _ = villain.server_list(&villain_ok).await;
    let villain_play = villain.play_ok(&villain_ok, 2).await;
    assert!(
        !link
            .auth_player(
                "villain",
                SessionKey {
                    login_ok1: villain_ok.login_ok1,
                    login_ok2: villain_ok.login_ok2,
                    play_ok1: villain_play.play_ok1,
                    play_ok2: villain_play.play_ok2 ^ 1,
                }
            )
            .await
    );
    until(|| state.clients.get("villain").is_none()).await;
    assert!(!state.game_servers.is_account_on_any_gs("villain"));

    // A player logs in: the login server asks for the characters and our link answers.
    let mut hero = FakeClient::connect(client_addr).await;
    let hero_ok = hero.login_ok("hero", "secret").await;
    until(|| {
        state
            .clients
            .get("hero")
            .is_some_and(|client| client.characters.snapshot().is_complete())
    })
    .await;
    let list = hero.server_list(&hero_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert!(list.servers[0].up);
    let counts = list.char_info.expect("the character counts");
    assert_eq!((counts[0].server_id, counts[0].chars), (2, 2));
    assert_eq!(counts[0].seconds_until_delete.len(), 1);

    // PlayOk, then the game server checks the keys with the login server.
    let play = hero.play_ok(&hero_ok, 2).await;
    let keys = SessionKey {
        login_ok1: hero_ok.login_ok1,
        login_ok2: hero_ok.login_ok2,
        play_ok1: play.play_ok1,
        play_ok2: play.play_ok2,
    };
    assert!(link.auth_player("hero", keys).await);
    until(|| state.game_servers.is_account_on_any_gs("hero")).await;
    assert!(
        state.clients.get("hero").is_none(),
        "accepted: the game server owns the account now"
    );

    // A second login for the account is refused and the game server is told to kick it.
    let mut again = FakeClient::connect(client_addr).await;
    assert_eq!(
        login_fail(&again.login("hero", "secret").await),
        LoginFailReason::AccountInUse
    );
    let kick = timeout(WAIT, kicks.recv()).await.unwrap().unwrap();
    assert_eq!(kick.account, "hero");
    link.player_logout("hero").await;
    until(|| !state.game_servers.is_account_on_any_gs("hero")).await;

    // A player in the game, then the login server drops the link: the game server comes back
    // and tells the login server again.
    let mut villager = FakeClient::connect(client_addr).await;
    let villager_ok = villager.login_ok("villager", "secret").await;
    let _ = villager.server_list(&villager_ok).await;
    let play = villager.play_ok(&villager_ok, 2).await;
    assert!(
        link.auth_player(
            "villager",
            SessionKey {
                login_ok1: villager_ok.login_ok1,
                login_ok2: villager_ok.login_ok2,
                play_ok1: play.play_ok1,
                play_ok2: play.play_ok2,
            }
        )
        .await
    );
    until(|| state.game_servers.is_account_on_any_gs("villager")).await;
    state
        .game_servers
        .link(SAMPLE_SERVER_ID)
        .expect("the link")
        .close();
    until(|| !link.status().registered).await;
    until(|| link.status().registered).await;
    until(|| {
        state
            .game_servers
            .registered(SAMPLE_SERVER_ID)
            .is_some_and(|gs| gs.authed && gs.status == ListStatus::Auto)
    })
    .await;
    until(|| state.game_servers.is_account_on_any_gs("villager")).await;

    // The game server stops: the login server shows it as down.
    link.shutdown().await;
    until(|| {
        state
            .game_servers
            .registered(SAMPLE_SERVER_ID)
            .is_some_and(|gs| gs.status == ListStatus::Down)
    })
    .await;

    drop((hero, villain, again, villager));
    server.shutdown(false).await;
    db.drop_db().await;
}
