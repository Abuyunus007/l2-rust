#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! A fake game server and a fake client against a running login server and a throwaway
//! database: the character counts reach the server list, PlayOk, the game server's
//! `PlayerAuthRequest`, `PlayerInGame` making a second login fail, and the account changes
//! (access level, tracert, temp ban, password).
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI's `database` job). Never point it at a
//! server with real data: it creates and drops a database named `l2_test_gp_*`. The same
//! packets are covered without a database in `src/gs/mod.rs`.

use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use l2_crypto::login::decrypt_init;
use l2_crypto::rsa::encrypt_raw;
use l2_crypto::scramble::unscramble_modulus;
use l2_crypto::{LinkCipher, LoginClientCipher};
use l2_db::{hash_password, string_to_hex};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_login::client::{AuthGameGuard, RequestAuthLogin, RequestServerList, RequestServerLogin};
use l2_protocol_login::enums::LoginFailReason;
use l2_protocol_login::link::{
    BlowFishKey, ChangeAccessLevel, ChangePassword, GameServerAuth, LsPacket, PlayerAuthRequest,
    PlayerInGame, PlayerTracert, ReplyCharacters, RequestTempBan, ServerStatus as StatusPacket,
    decode_from_ls,
};
use l2_protocol_login::server::{Init, LoginFail, LoginOk, PlayOk, ServerList};
use login_server::state::ServerStatus;
use login_server::{Paths, Server};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

const SCHEMA: [&str; 4] = [
    include_str!("../../l2-db/sql/login/account_data.sql"),
    include_str!("../../l2-db/sql/login/accounts.sql"),
    include_str!("../../l2-db/sql/login/accounts_ipauth.sql"),
    include_str!("../../l2-db/sql/login/gameservers.sql"),
];

/// The hex id of the sample row in `gameservers.sql` (game server 2).
const SAMPLE_HEXID: &str = "-2ad66b3f483c22be097019f55c8abdf0";

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
        let name = format!("l2_test_gp_{}_{}", std::process::id(), nanos % 1_000_000_000);
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

    async fn connection(&self) -> MySqlConnection {
        Self::connect_to(&self.url, &self.name).await
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
        // Port 0 for both listeners; ShowLicence = True is the default, spelled out because the
        // session key comparison depends on it.
        std::fs::write(
            config.join("Server.ini"),
            "LoginserverHostname = 127.0.0.1\nLoginserverPort = 0\nLoginPort = 0\n\
             EnableFloodProtection = False\nAutoCreateAccounts = True\nShowLicence = True\n",
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
    async fn connect(addr: SocketAddr) -> Self {
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

    async fn recv(&mut self) -> Option<Vec<u8>> {
        let mut body = read_frame(&mut self.stream).await?;
        self.cipher.decrypt(&mut body).unwrap();
        Some(body)
    }

    /// GameGuard and `RequestAuthLogin`; the reply to the login.
    async fn login(&mut self, user: &str, password: &str) -> Vec<u8> {
        self.send(
            &AuthGameGuard {
                session_id: self.init.session_id,
            }
            .to_bytes(),
        )
        .await;
        assert_eq!(self.recv().await.unwrap()[0], 0x0B, "GGAuth");

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

    async fn login_ok(&mut self, user: &str, password: &str) -> LoginOk {
        let reply = self.login(user, password).await;
        assert_eq!(reply[0], LoginOk::OPCODE, "LoginOk for {user}");
        LoginOk::decode(&mut PacketReader::new(&reply[1..])).unwrap()
    }

    async fn server_list(&mut self, ok: &LoginOk) -> ServerList {
        self.send(
            &RequestServerList {
                login_ok1: ok.login_ok1,
                login_ok2: ok.login_ok2,
            }
            .to_bytes(),
        )
        .await;
        let reply = self.recv().await.unwrap();
        assert_eq!(reply[0], ServerList::OPCODE);
        ServerList::decode(&mut PacketReader::new(&reply[1..])).unwrap()
    }

    async fn server_login(&mut self, ok: &LoginOk, server_id: u8) -> Vec<u8> {
        self.send(
            &RequestServerLogin {
                login_ok1: ok.login_ok1,
                login_ok2: ok.login_ok2,
                server_id,
            }
            .to_bytes(),
        )
        .await;
        self.recv().await.unwrap()
    }
}

async fn read_frame(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut header = [0u8; 2];
    timeout(Duration::from_secs(10), stream.read_exact(&mut header))
        .await
        .ok()?
        .ok()?;
    let len = usize::from(u16::from_le_bytes(header)) - 2;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.ok()?;
    Some(body)
}

/// A game server on the link, like the Java `LoginServerThread`.
struct FakeGs {
    stream: TcpStream,
    cipher: LinkCipher,
}

impl FakeGs {
    /// Connects, exchanges the Blowfish key and registers as the sample server (id 2).
    async fn register(address: SocketAddr) -> Self {
        let mut gs = Self {
            stream: TcpStream::connect(address).await.unwrap(),
            cipher: LinkCipher::default_key().unwrap(),
        };
        let LsPacket::InitLS(init) = gs.recv().await else {
            panic!("expected InitLS");
        };
        let session_key: Vec<u8> = (1..=40u8).collect();
        let encrypted = encrypt_raw(&init.public_key, &[1, 0, 1], &session_key).unwrap();
        gs.send(&BlowFishKey {
            encrypted_key: encrypted,
        })
        .await;
        gs.cipher = LinkCipher::new(&session_key).unwrap();
        gs.send(&GameServerAuth {
            desired_id: 2,
            accept_alternate: false,
            reserve_host: false,
            port: 7777,
            max_players: 300,
            hex_id: string_to_hex(SAMPLE_HEXID).unwrap(),
            subnets: vec!["0.0.0.0/0".to_owned()],
            hosts: vec!["127.0.0.1".to_owned()],
        })
        .await;
        assert!(matches!(gs.recv().await, LsPacket::AuthResponse(_)));
        gs
    }

    async fn recv(&mut self) -> LsPacket {
        let mut header = [0u8; 2];
        timeout(Duration::from_secs(10), self.stream.read_exact(&mut header))
            .await
            .unwrap()
            .unwrap();
        let mut body = vec![0u8; usize::from(u16::from_le_bytes(header)) - 2];
        timeout(Duration::from_secs(10), self.stream.read_exact(&mut body))
            .await
            .unwrap()
            .unwrap();
        let plain = self.cipher.open(&mut body).unwrap();
        decode_from_ls(plain[0], &mut PacketReader::new(&plain[1..])).unwrap()
    }

    async fn send<P: Encode>(&mut self, packet: &P) {
        let body = self.cipher.seal(&packet.to_bytes());
        let mut frame = u16::try_from(body.len() + 2).unwrap().to_le_bytes().to_vec();
        frame.extend_from_slice(&body);
        self.stream.write_all(&frame).await.unwrap();
    }

    async fn auth_response(&mut self, account: &str, keys: &KeyParts) -> bool {
        self.send(&PlayerAuthRequest {
            account: account.to_owned(),
            play_ok1: keys.play_ok1,
            play_ok2: keys.play_ok2,
            login_ok1: keys.login_ok1,
            login_ok2: keys.login_ok2,
        })
        .await;
        match self.recv().await {
            LsPacket::PlayerAuthResponse(response) => {
                assert_eq!(response.account, account);
                response.authed
            }
            other => panic!("expected PlayerAuthResponse, got {other:?}"),
        }
    }
}

struct KeyParts {
    login_ok1: i32,
    login_ok2: i32,
    play_ok1: i32,
    play_ok2: i32,
}

async fn until(mut condition: impl FnMut() -> bool) {
    for _ in 0..200 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not met in time");
}

#[tokio::test]
async fn a_player_enters_the_game_through_the_login_server() {
    let Some(db) = TestDb::create().await else { return };
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path());
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));
    let server = Server::start(&paths).await.unwrap();
    let state = std::sync::Arc::clone(server.state());
    let client_addr = server.client_addr().unwrap();
    let mut gs = FakeGs::register(state.game_servers.listen_addr().unwrap()).await;

    // What the Java game server does right after AuthResponse: without it the server shows as
    // down (a registered server is DOWN until it reports its status).
    gs.send(&StatusPacket {
        attributes: vec![
            (StatusPacket::SERVER_LIST_SQUARE_BRACKET, StatusPacket::OFF),
            (StatusPacket::SERVER_TYPE, StatusPacket::SERVER_NORMAL),
            (StatusPacket::SERVER_LIST_STATUS, StatusPacket::STATUS_AUTO),
        ],
    })
    .await;
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
    gs.send(&ReplyCharacters {
        account: "hero".to_owned(),
        chars: 2,
        delete_times: vec![deletes_at],
    })
    .await;
    let list = hero.server_list(&hero_ok).await;
    assert_eq!(list.servers.len(), 1);
    assert!(list.servers[0].up, "ServerStatus made the server show as up");
    assert_eq!(list.servers[0].max_players, 300);
    let chars = list.char_info.expect("the character counts");
    assert_eq!(chars.len(), 1);
    assert_eq!((chars[0].server_id, chars[0].chars), (2, 2));
    assert_eq!(chars[0].seconds_until_delete.len(), 1);

    // PlayOk, then the game server checks the keys.
    let reply = hero.server_login(&hero_ok, 2).await;
    assert_eq!(reply[0], PlayOk::OPCODE, "PlayOk");
    let play = PlayOk::decode(&mut PacketReader::new(&reply[1..])).unwrap();
    let keys = KeyParts {
        login_ok1: hero_ok.login_ok1,
        login_ok2: hero_ok.login_ok2,
        play_ok1: play.play_ok1,
        play_ok2: play.play_ok2,
    };
    let wrong_play = KeyParts {
        play_ok2: keys.play_ok2 ^ 1,
        ..keys
    };
    assert!(!gs.auth_response("hero", &wrong_play).await);
    let wrong_login = KeyParts {
        login_ok1: keys.login_ok1 ^ 1,
        ..keys
    };
    assert!(!gs.auth_response("hero", &wrong_login).await);
    assert!(state.clients.get("hero").is_some(), "refusals keep the login");
    assert!(gs.auth_response("hero", &keys).await);
    assert!(
        state.clients.get("hero").is_none(),
        "accepted: the game server owns it now"
    );

    // The player is in the game: a second login is refused and the game server kicks the first.
    gs.send(&PlayerInGame {
        accounts: vec!["hero".to_owned()],
    })
    .await;
    until(|| state.game_servers.is_account_on_any_gs("hero")).await;
    let mut again = FakeClient::connect(client_addr).await;
    let reply = again.login("hero", "secret").await;
    assert_eq!(reply[0], LoginFail::OPCODE);
    assert_eq!(
        LoginFail::decode(&mut PacketReader::new(&reply[1..]))
            .unwrap()
            .reason,
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
    gs.send(&l2_protocol_login::link::PlayerLogout {
        account: "hero".to_owned(),
    })
    .await;
    until(|| !state.game_servers.is_account_on_any_gs("hero")).await;

    drop(conn);
    server.shutdown(false).await;
    db.drop_db().await;
}
