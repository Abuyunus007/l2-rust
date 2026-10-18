#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! End-to-end client login against a throwaway database: Init, GameGuard, RSA credentials,
//! account checks, server list.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI's `database` job). Never point it at a
//! server with real data: it creates and drops a database named `l2_test_lc_*`.

use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use l2_crypto::LoginClientCipher;
use l2_crypto::login::decrypt_init;
use l2_crypto::rsa::encrypt_raw;
use l2_crypto::scramble::unscramble_modulus;
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_login::client::{AuthGameGuard, RequestAuthLogin, RequestServerList, RequestServerLogin};
use l2_protocol_login::enums::{AccountKickedReason, LoginFailReason, PlayFailReason};
use l2_protocol_login::server::{AccountKicked, Init, LoginFail, LoginOk, PlayFail, PlayOk, ServerList};
use login_server::state::ServerStatus;
use login_server::{Paths, Server};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const SCHEMA: [&str; 4] = [
    include_str!("../../l2-db/sql/login/account_data.sql"),
    include_str!("../../l2-db/sql/login/accounts.sql"),
    include_str!("../../l2-db/sql/login/accounts_ipauth.sql"),
    include_str!("../../l2-db/sql/login/gameservers.sql"),
];

/// A database with the login schema, dropped at the end of the test.
struct TestDb {
    url: String,
    name: String,
}

impl TestDb {
    async fn create() -> Option<Self> {
        let url = std::env::var("L2_TEST_DATABASE_URL")
            .ok()
            .filter(|u| !u.trim().is_empty());
        let Some(url) = url else {
            eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
            return None;
        };
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let name = format!("l2_test_lc_{}_{}", std::process::id(), nanos % 1_000_000_000);
        let mut admin = MySqlConnection::connect_with(&MySqlConnectOptions::from_str(&url).unwrap())
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!(
            "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        )))
        .execute(&mut admin)
        .await
        .unwrap();
        let mut setup = Self::connect_to(&url, &name).await;
        for file in SCHEMA {
            for statement in file.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(AssertSqlSafe(statement))
                    .execute(&mut setup)
                    .await
                    .unwrap();
            }
        }
        Some(Self { url, name })
    }

    async fn connect_to(url: &str, name: &str) -> MySqlConnection {
        MySqlConnection::connect_with(&MySqlConnectOptions::from_str(url).unwrap().database(name))
            .await
            .unwrap()
    }

    async fn exec(&self, sql: &str) {
        let mut conn = Self::connect_to(&self.url, &self.name).await;
        sqlx::query(AssertSqlSafe(sql.to_owned()))
            .execute(&mut conn)
            .await
            .unwrap();
    }

    async fn drop_db(self) {
        let mut admin = MySqlConnection::connect_with(&MySqlConnectOptions::from_str(&self.url).unwrap())
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE `{}`", self.name)))
            .execute(&mut admin)
            .await
            .unwrap();
    }

    /// `(user, password, host, port)` of the test URL.
    fn parts(&self) -> (String, String, String, u16) {
        let rest = self
            .url
            .strip_prefix("mysql://")
            .unwrap()
            .split('/')
            .next()
            .unwrap();
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

    fn write_server_folder(&self, root: &Path) {
        let (user, password, host, port) = self.parts();
        let config = root.join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(
            config.join("Database.ini"),
            format!(
                "URL = jdbc:mysql://{host}:{port}/{}?useSSL=false\nLogin = {user}\nPassword = {password}\n",
                self.name
            ),
        )
        .unwrap();
        std::fs::write(
            config.join("Server.ini"),
            "LoginserverHostname = 127.0.0.1\nLoginserverPort = 0\nLoginTryBeforeBan = 3\n\
             LoginBlockAfterBan = 600\nAutoCreateAccounts = True\nShowLicence = True\n",
        )
        .unwrap();
    }
}

/// A minimal H5 client: raw frames, the real ciphers, RSA credentials.
struct FakeClient {
    stream: TcpStream,
    cipher: LoginClientCipher,
    init: Init,
}

impl FakeClient {
    async fn connect(addr: std::net::SocketAddr) -> Self {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let mut body = read_frame(&mut stream).await.expect("Init");
        decrypt_init(&mut body).unwrap();
        assert_eq!(body[0], Init::OPCODE);
        let init = Init::decode(&mut PacketReader::new(&body[1..])).unwrap();
        let cipher = LoginClientCipher::after_init(&init.blowfish_key);
        Self { stream, cipher, init }
    }

    async fn send(&mut self, payload: &[u8]) {
        let body = self.cipher.encrypt(payload);
        let mut frame = ((body.len() + 2) as u16).to_le_bytes().to_vec();
        frame.extend(body);
        self.stream.write_all(&frame).await.unwrap();
    }

    /// Next server packet (decrypted), or `None` when the server closed the connection.
    async fn recv(&mut self) -> Option<Vec<u8>> {
        let mut body = read_frame(&mut self.stream).await?;
        self.cipher.decrypt(&mut body).unwrap();
        Some(body)
    }

    async fn game_guard(&mut self) {
        self.send(
            &AuthGameGuard {
                session_id: self.init.session_id,
            }
            .to_bytes(),
        )
        .await;
        let reply = self.recv().await.unwrap();
        assert_eq!(reply[0], 0x0B, "GGAuth");
    }

    /// RequestAuthLogin with the H5 (256 byte) layout.
    async fn login(&mut self, user: &str, password: &str) -> Vec<u8> {
        let mut plain = [0u8; 256];
        plain[0x4E..0x4E + user.len()].copy_from_slice(user.as_bytes());
        plain[0xDC..0xDC + password.len()].copy_from_slice(password.as_bytes());
        let modulus = unscramble_modulus(&self.init.scrambled_modulus);
        let block = |half: &[u8]| -> [u8; 128] {
            let encrypted = encrypt_raw(&modulus, &[1, 0, 1], half).unwrap();
            let mut out = [0u8; 128];
            out[128 - encrypted.len()..].copy_from_slice(&encrypted);
            out
        };
        let request = RequestAuthLogin::Rsa256 {
            block1: block(&plain[..128]),
            block2: block(&plain[128..]),
        };
        self.send(&request.to_bytes()).await;
        self.recv().await.expect("a reply to RequestAuthLogin")
    }
}

async fn read_frame(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut header = [0u8; 2];
    tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut header))
        .await
        .ok()?
        .ok()?;
    let len = usize::from(u16::from_le_bytes(header)) - 2;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.ok()?;
    Some(body)
}

fn login_fail(reply: &[u8]) -> LoginFailReason {
    assert_eq!(
        reply[0],
        LoginFail::OPCODE,
        "expected LoginFail, got 0x{:02X}",
        reply[0]
    );
    LoginFail::decode(&mut PacketReader::new(&reply[1..]))
        .unwrap()
        .reason
}

#[tokio::test]
async fn client_login_flow() {
    let Some(db) = TestDb::create().await else { return };
    db.exec("INSERT INTO accounts (login, password, accessLevel) VALUES ('banned', 'x', -1)")
        .await;
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path());
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));
    let server = Server::start(&paths).await.unwrap();
    let addr = server.client_addr().unwrap();

    // 1. A new account is auto-created; LoginOk, then the server list with game server 2.
    let mut first = FakeClient::connect(addr).await;
    first.game_guard().await;
    let reply = first.login("Hero", "secret").await;
    assert_eq!(reply[0], LoginOk::OPCODE, "LoginOk");
    let ok = LoginOk::decode(&mut PacketReader::new(&reply[1..])).unwrap();
    first
        .send(
            &RequestServerList {
                login_ok1: ok.login_ok1,
                login_ok2: ok.login_ok2,
            }
            .to_bytes(),
        )
        .await;
    let reply = first.recv().await.unwrap();
    assert_eq!(reply[0], ServerList::OPCODE);
    let list = ServerList::decode(&mut PacketReader::new(&reply[1..])).unwrap();
    assert_eq!(list.servers.iter().map(|s| s.server_id).collect::<Vec<_>>(), [2]);
    assert!(!list.servers[0].up, "registered but not connected: down");
    // A wrong login pair is refused.
    first
        .send(
            &RequestServerList {
                login_ok1: ok.login_ok1 ^ 1,
                login_ok2: ok.login_ok2,
            }
            .to_bytes(),
        )
        .await;
    assert_eq!(
        login_fail(&first.recv().await.unwrap()),
        LoginFailReason::AccessFailed
    );

    // 2. Same account while the first client is still logged in: both get "account in use".
    let mut second = FakeClient::connect(addr).await;
    second.game_guard().await;
    assert_eq!(
        login_fail(&second.login("hero", "secret").await),
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
    let reply = banned.login("banned", "whatever").await;
    // The password check comes first in Java, so a banned account with a wrong password
    // just gets "wrong user or password".
    assert_eq!(login_fail(&reply), LoginFailReason::UserOrPassWrong);
    db.exec("UPDATE accounts SET password = 'qUqP5cyxm6YcTAhz05Hph5gvu9M=' WHERE login = 'banned'")
        .await; // "test"
    let reply = banned.login("banned", "test").await;
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
            login_fail(&guesser.login("hero", "nope").await),
            LoginFailReason::UserOrPassWrong
        );
    }
    assert!(!server.state().bans.is_banned("127.0.0.1"));
    assert_eq!(
        login_fail(&guesser.login("hero", "nope").await),
        LoginFailReason::UserOrPassWrong
    );
    assert!(server.state().bans.is_banned("127.0.0.1"));

    // 5. A banned address gets LoginFail(NOT_AUTHED) and is closed.
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut body = read_frame(&mut stream).await.expect("LoginFail");
    decrypt_init(&mut body).unwrap(); // first packet → static key
    assert_eq!(login_fail(&body), LoginFailReason::NotAuthed);
    assert!(read_frame(&mut stream).await.is_none());

    server.shutdown(false).await;
    db.drop_db().await;
}

/// Logs in and returns the `LoginOk` pair.
async fn login_ok(client: &mut FakeClient, user: &str, password: &str) -> LoginOk {
    client.game_guard().await;
    let reply = client.login(user, password).await;
    assert_eq!(reply[0], LoginOk::OPCODE, "LoginOk for {user}");
    LoginOk::decode(&mut PacketReader::new(&reply[1..])).unwrap()
}

async fn request_list(client: &mut FakeClient, ok: &LoginOk) -> ServerList {
    client
        .send(
            &RequestServerList {
                login_ok1: ok.login_ok1,
                login_ok2: ok.login_ok2,
            }
            .to_bytes(),
        )
        .await;
    let reply = client.recv().await.unwrap();
    assert_eq!(reply[0], ServerList::OPCODE);
    ServerList::decode(&mut PacketReader::new(&reply[1..])).unwrap()
}

async fn request_play(client: &mut FakeClient, ok: &LoginOk, server_id: u8) -> Vec<u8> {
    client
        .send(
            &RequestServerLogin {
                login_ok1: ok.login_ok1,
                login_ok2: ok.login_ok2,
                server_id,
            }
            .to_bytes(),
        )
        .await;
    client.recv().await.unwrap()
}

fn play_fail(reply: &[u8]) -> PlayFailReason {
    assert_eq!(
        reply[0],
        PlayFail::OPCODE,
        "expected PlayFail, got 0x{:02X}",
        reply[0]
    );
    PlayFail::decode(&mut PacketReader::new(&reply[1..]))
        .unwrap()
        .reason
}

#[tokio::test]
async fn server_list_and_server_login_flow() {
    let Some(db) = TestDb::create().await else { return };
    // "test" hashed; access level 1 makes a GM.
    db.exec(
        "INSERT INTO accounts (login, password, accessLevel, lastServer) \
         VALUES ('gm', 'qUqP5cyxm6YcTAhz05Hph5gvu9M=', 1, 1)",
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path());
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));
    let server = Server::start(&paths).await.unwrap();
    let addr = server.client_addr().unwrap();
    let state = std::sync::Arc::clone(server.state());

    // Game server 2 (from the gameservers table) connects, GM only for now. There is no link
    // yet (M1-LS-04), so the table is set directly.
    assert!(state.game_servers.update_registered(2, |gs| {
        gs.authed = true;
        gs.status = ServerStatus::GmOnly;
    }));

    let mut player = FakeClient::connect(addr).await;
    let player_ok = login_ok(&mut player, "hero", "secret").await;
    let mut gm = FakeClient::connect(addr).await;
    let gm_ok = login_ok(&mut gm, "gm", "test").await;

    // GM-only: hidden from the player, shown (up) to the GM, who also gets lastServer = 1.
    assert!(request_list(&mut player, &player_ok).await.servers.is_empty());
    let list = request_list(&mut gm, &gm_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert_eq!(list.servers[0].server_id, 2);
    assert!(list.servers[0].up);
    assert_eq!(list.last_server, 1);
    assert_eq!(list.char_info, None, "no ReplyCharacters, no counts");
    assert_eq!(
        play_fail(&request_play(&mut player, &player_ok, 2).await),
        PlayFailReason::ServerOverloaded
    );

    // Normal, but without room (max players is 0 until the link reports it): only a GM gets in.
    assert!(
        state
            .game_servers
            .update_registered(2, |gs| gs.status = ServerStatus::Normal)
    );
    let list = request_list(&mut player, &player_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert_eq!(
        play_fail(&request_play(&mut player, &player_ok, 2).await),
        PlayFailReason::ServerOverloaded
    );
    // A server that does not exist.
    assert_eq!(
        play_fail(&request_play(&mut gm, &gm_ok, 9).await),
        PlayFailReason::ServerOverloaded
    );
    // A wrong login pair is refused before anything else.
    let wrong = LoginOk {
        login_ok1: gm_ok.login_ok1 ^ 1,
        login_ok2: gm_ok.login_ok2,
    };
    assert_eq!(
        login_fail(&request_play(&mut gm, &wrong, 2).await),
        LoginFailReason::AccessFailed
    );

    // The GM gets PlayOk with the play pair of the session, and lastServer changes.
    let key = state.clients.session_key("gm").unwrap();
    let reply = request_play(&mut gm, &gm_ok, 2).await;
    assert_eq!(reply[0], PlayOk::OPCODE, "PlayOk");
    let play = PlayOk::decode(&mut PacketReader::new(&reply[1..])).unwrap();
    assert_eq!((play.play_ok1, play.play_ok2), (key.play_ok1, key.play_ok2));
    let mut conn = TestDb::connect_to(&db.url, &db.name).await;
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
        login_fail(&request_play(&mut gm, &gm_ok, 2).await),
        LoginFailReason::AccessFailed
    );

    server.shutdown(false).await;
    db.drop_db().await;
}
