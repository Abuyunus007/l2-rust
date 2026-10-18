#![allow(clippy::unwrap_used, clippy::expect_used)]
//! A fake game server registers with a running login server and the `gameservers` table.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service,
//! e.g. `mysql://root@127.0.0.1:3306/`). Never point it at a server with real data: the test
//! only creates and drops a database named `l2_test_gs_*`, but it needs the rights to do so.
//! The handshake itself is covered without a database in `src/gs/mod.rs`.

use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use l2_crypto::LinkCipher;
use l2_crypto::rsa::encrypt_raw;
use l2_db::string_to_hex;
use l2_net::{Encode, PacketReader};
use l2_protocol_login::link::{AuthResponse, BlowFishKey, GameServerAuth, LsPacket, decode_from_ls};
use login_server::state::ExitReason;
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

/// The hex id of the sample row in `gameservers.sql`.
const SAMPLE_HEXID: &str = "-2ad66b3f483c22be097019f55c8abdf0";

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
    // LoginPort 0: a free port. The flood protection would refuse the quick reconnects below.
    std::fs::write(
        config.join("Server.ini"),
        "LoginPort = 0\nEnableFloodProtection = False\n",
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
}

/// A game server on the link, like the Java `LoginServerThread`.
struct FakeGs {
    stream: TcpStream,
    cipher: LinkCipher,
}

impl FakeGs {
    async fn connect(address: SocketAddr) -> Self {
        Self {
            stream: TcpStream::connect(address).await.unwrap(),
            cipher: LinkCipher::default_key().unwrap(),
        }
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

    /// Registers and returns the `AuthResponse` with the connection, which must stay open for
    /// the server to remain registered.
    async fn register(address: SocketAddr, request: &GameServerAuth) -> (AuthResponse, Self) {
        let mut gs = Self::connect(address).await;
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
        gs.send(request).await;
        match gs.recv().await {
            LsPacket::AuthResponse(response) => (response, gs),
            other => panic!("expected AuthResponse, got {other:?}"),
        }
    }
}

fn request(desired_id: u8, hex_id: Vec<u8>, accept_alternate: bool) -> GameServerAuth {
    GameServerAuth {
        desired_id,
        accept_alternate,
        reserve_host: false,
        port: 7777,
        max_players: 300,
        hex_id,
        subnets: vec!["0.0.0.0/0".to_owned()],
        hosts: vec!["gs.example.org".to_owned()],
    }
}

#[tokio::test]
async fn game_servers_register_against_the_database() {
    let Ok(url) = std::env::var("L2_TEST_DATABASE_URL") else {
        eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
        return;
    };
    if url.trim().is_empty() {
        eprintln!("skipped: L2_TEST_DATABASE_URL is empty");
        return;
    }

    // A fresh database with the login schema and the sample game server (id 2).
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let database = format!("l2_test_gs_{}_{}", std::process::id(), nanos % 1_000_000_000);
    assert!(database.starts_with("l2_test_gs_"));
    let mut admin = MySqlConnection::connect_with(&MySqlConnectOptions::from_str(&url).unwrap())
        .await
        .unwrap();
    sqlx::query(AssertSqlSafe(format!(
        "CREATE DATABASE `{database}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    )))
    .execute(&mut admin)
    .await
    .unwrap();
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
        let address = state.game_servers.listen_addr().unwrap();

        // The sample server, with the hex id as Java reads it from the table.
        let sample = string_to_hex(SAMPLE_HEXID).unwrap();
        let (response, _first) = FakeGs::register(address, &request(2, sample, false)).await;
        assert_eq!(
            (response.server_id, response.server_name.as_str()),
            (2, "Sieghardt")
        );
        assert!(state.game_servers.registered(2).unwrap().authed);
        assert_eq!(
            state.repository.game_servers().await.unwrap().len(),
            1,
            "nothing new saved"
        );

        // A new server is saved with the BigInteger text of its hex id and its external host.
        let (response, _second) = FakeGs::register(address, &request(3, vec![0x2A, 0xD6, 0x6B], false)).await;
        assert_eq!((response.server_id, response.server_name.as_str()), (3, "Kain"));
        let rows = state.repository.game_servers().await.unwrap();
        let row = rows.iter().find(|row| row.server_id == 3).unwrap();
        assert_eq!(row.hexid, "2ad66b");
        assert_eq!(row.host, "gs.example.org");

        // A wrong hex id with an alternative gets the first free id (1) and is saved too.
        let (response, _third) = FakeGs::register(address, &request(2, vec![9, 9, 9], true)).await;
        assert_eq!((response.server_id, response.server_name.as_str()), (1, "Bartz"));
        assert_eq!(state.repository.game_servers().await.unwrap().len(), 3);

        server.shutdown(false).await
    }
    .await;

    sqlx::query(AssertSqlSafe(format!("DROP DATABASE `{database}`")))
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(result, ExitReason::Shutdown);
}
