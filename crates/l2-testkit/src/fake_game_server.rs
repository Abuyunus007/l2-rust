//! A fake game server on the LS<->GS link, behaving like the Java `LoginServerThread`.
//!
//! Java: `gameserver/LoginServerThread.java`. Order of a real game server: connect, read
//! `InitLS`, send `BlowFishKey` (the 40 byte session key under the login server's RSA key),
//! switch to that key, send `AuthRequest` (here [`GameServerAuth`]), read `AuthResponse`, then
//! send `ServerStatus` (see [`FakeGameServer::send_java_status`]) and `PlayerInGame` for the
//! players that are online.
//!
//! ```ignore
//! let mut gs = FakeGameServer::register_sample(state.game_servers.listen_addr().unwrap()).await;
//! gs.send_java_status(false).await;
//! ```

use std::net::SocketAddr;
use std::time::Duration;

use l2_crypto::LinkCipher;
use l2_crypto::rsa::encrypt_raw;
use l2_db::string_to_hex;
use l2_net::{Encode, PacketReader};
use l2_protocol_login::link::{
    AuthResponse, BlowFishKey, GameServerAuth, InitLS, LsPacket, PlayerAuthRequest, PlayerInGame,
    PlayerLogout, ReplyCharacters, ServerStatus, decode_from_ls,
};
use l2_protocol_login::server::{LoginOk, PlayOk};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::test_db::SAMPLE_HEXID;

/// How long a receive waits before the test fails.
pub const RECV_TIMEOUT: Duration = Duration::from_secs(10);

/// The four numbers a game server checks with `PlayerAuthRequest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionKeys {
    pub login_ok1: i32,
    pub login_ok2: i32,
    pub play_ok1: i32,
    pub play_ok2: i32,
}

impl SessionKeys {
    /// What the client received in `LoginOk` and `PlayOk`.
    pub fn new(login: &LoginOk, play: &PlayOk) -> Self {
        Self {
            login_ok1: login.login_ok1,
            login_ok2: login.login_ok2,
            play_ok1: play.play_ok1,
            play_ok2: play.play_ok2,
        }
    }
}

/// A `GameServerAuth` like a game server with default settings sends it.
pub fn auth_request(desired_id: u8, hex_id: Vec<u8>, accept_alternate: bool) -> GameServerAuth {
    GameServerAuth {
        desired_id,
        accept_alternate,
        reserve_host: false,
        port: 7777,
        max_players: 300,
        hex_id,
        subnets: vec!["0.0.0.0/0".to_owned()],
        hosts: vec!["127.0.0.1".to_owned()],
    }
}

/// The session key every fake game server uses: 40 bytes, first byte not zero (the login server
/// strips leading zeros of the decrypted block).
pub fn session_key() -> Vec<u8> {
    (1..=40u8).collect()
}

/// A game server on the link.
pub struct FakeGameServer {
    stream: TcpStream,
    cipher: LinkCipher,
}

impl FakeGameServer {
    /// Opens the connection. The cipher is the default link key, as before the key exchange.
    pub async fn connect(address: SocketAddr) -> Self {
        Self {
            stream: TcpStream::connect(address).await.unwrap(),
            cipher: LinkCipher::default_key().unwrap(),
        }
    }

    /// Connects, exchanges the Blowfish key and sends `request`. Returns the `AuthResponse`
    /// with the connection, which must stay open for the server to remain registered. A
    /// refusal (`LoginServerFail`) fails the test; use [`connect`](Self::connect) and
    /// [`handshake`](Self::handshake) to look at it.
    pub async fn register(address: SocketAddr, request: &GameServerAuth) -> (AuthResponse, Self) {
        let mut gs = Self::connect(address).await;
        gs.handshake(&session_key()).await;
        gs.send(request).await;
        match gs.recv().await {
            LsPacket::AuthResponse(response) => (response, gs),
            other => panic!("expected AuthResponse, got {other:?}"),
        }
    }

    /// Registers as the sample game server of `gameservers.sql` (id 2, port 7777, 300 players,
    /// host 127.0.0.1).
    pub async fn register_sample(address: SocketAddr) -> Self {
        let hex_id = string_to_hex(SAMPLE_HEXID).unwrap();
        let (response, gs) = Self::register(address, &auth_request(2, hex_id, false)).await;
        assert_eq!(response.server_id, 2);
        gs
    }

    /// Reads `InitLS`, sends the `BlowFishKey` and switches to the new key.
    pub async fn handshake(&mut self, session_key: &[u8]) -> InitLS {
        let init = self.read_init().await;
        let encrypted = encrypt_raw(&init.public_key, &[1, 0, 1], session_key).unwrap();
        self.send(&BlowFishKey {
            encrypted_key: encrypted,
        })
        .await;
        self.use_session_key(session_key);
        init
    }

    /// The plaintext of the next packet (padding and checksum slot included), or `None` when
    /// the login server closed the link. Waiting longer than [`RECV_TIMEOUT`] fails the test.
    pub async fn recv_raw(&mut self) -> Option<Vec<u8>> {
        let mut header = [0u8; 2];
        timeout(RECV_TIMEOUT, self.stream.read_exact(&mut header))
            .await
            .expect("timed out waiting for the login server")
            .ok()?;
        let mut body = vec![0u8; usize::from(u16::from_le_bytes(header)).checked_sub(2)?];
        timeout(RECV_TIMEOUT, self.stream.read_exact(&mut body))
            .await
            .expect("timed out waiting for the rest of a packet")
            .ok()?;
        Some(self.cipher.open(&mut body).unwrap().to_vec())
    }

    /// The next decoded packet, or `None` when the link was closed.
    pub async fn recv_opt(&mut self) -> Option<LsPacket> {
        let plain = self.recv_raw().await?;
        Some(decode_from_ls(plain[0], &mut PacketReader::new(&plain[1..])).unwrap())
    }

    /// The next decoded packet; a closed link fails the test.
    pub async fn recv(&mut self) -> LsPacket {
        self.recv_opt()
            .await
            .expect("a packet from the login server (the link was closed or timed out)")
    }

    /// True when the login server closed the link (a packet that arrives instead is consumed).
    pub async fn is_closed(&mut self) -> bool {
        self.recv_raw().await.is_none()
    }

    /// The login server closes the link without sending anything else.
    pub async fn expect_closed(&mut self) {
        assert!(
            self.is_closed().await,
            "the login server should have closed the link"
        );
    }

    /// The `InitLS` the login server sends first.
    pub async fn read_init(&mut self) -> InitLS {
        match self.recv_opt().await {
            Some(LsPacket::InitLS(init)) => init,
            other => panic!("expected InitLS, got {other:?}"),
        }
    }

    /// Switches to the session key without sending a `BlowFishKey` (for tests of a broken key
    /// exchange).
    pub fn use_session_key(&mut self, session_key: &[u8]) {
        self.cipher = LinkCipher::new(session_key).unwrap();
    }

    /// Writes bytes to the socket as they are: no framing, no encryption (for malformed frames).
    pub async fn send_wire(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).await.unwrap();
    }

    /// Sends a payload that is already encoded (also for deliberately wrong packets).
    pub async fn send_raw(&mut self, payload: &[u8]) {
        let body = self.cipher.seal(payload);
        let mut frame = u16::try_from(body.len() + 2).unwrap().to_le_bytes().to_vec();
        frame.extend_from_slice(&body);
        self.stream.write_all(&frame).await.unwrap();
    }

    pub async fn send<P: Encode>(&mut self, packet: &P) {
        self.send_raw(&packet.to_bytes()).await;
    }

    /// What the Java game server sends right after `AuthResponse`: no brackets, type normal,
    /// status auto (or GM only), age for all. Without it the server shows as down.
    // Java: LoginServerThread case 0x02
    pub async fn send_java_status(&mut self, gm_only: bool) {
        let status = if gm_only {
            ServerStatus::STATUS_GM_ONLY
        } else {
            ServerStatus::STATUS_AUTO
        };
        self.send(&ServerStatus {
            attributes: vec![
                (ServerStatus::SERVER_LIST_SQUARE_BRACKET, ServerStatus::OFF),
                (ServerStatus::SERVER_TYPE, ServerStatus::SERVER_NORMAL),
                (ServerStatus::SERVER_LIST_STATUS, status),
                (ServerStatus::SERVER_AGE, ServerStatus::SERVER_AGE_ALL),
            ],
        })
        .await;
    }

    /// Changes only the list status (`ServerStatus::STATUS_*`).
    // Java: LoginServerThread.setServerStatus
    pub async fn send_status(&mut self, status: i32) {
        self.send(&ServerStatus {
            attributes: vec![(ServerStatus::SERVER_LIST_STATUS, status)],
        })
        .await;
    }

    /// Answers a `RequestCharacters` with `chars` characters, `delete_times` being the
    /// deletion times (epoch ms) of those waiting to be deleted.
    pub async fn reply_characters(&mut self, account: &str, chars: u8, delete_times: Vec<i64>) {
        self.send(&ReplyCharacters {
            account: account.to_owned(),
            chars,
            delete_times,
        })
        .await;
    }

    /// Waits for the `RequestCharacters` the login server sends when `account` logs in, and
    /// answers it with `chars` characters (none of them being deleted).
    // Java: LoginServerThread case 0x05
    pub async fn answer_characters(&mut self, account: &str, chars: u8) {
        match self.recv().await {
            LsPacket::RequestCharacters(request) => assert_eq!(request.account, account),
            other => panic!("expected RequestCharacters, got {other:?}"),
        }
        self.reply_characters(account, chars, Vec::new()).await;
    }

    /// `PlayerAuthRequest`; returns whether the login server accepted the keys.
    // Java: LoginServerThread.addWaitingClientAndSendRequest
    pub async fn auth_player(&mut self, account: &str, keys: &SessionKeys) -> bool {
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

    /// `PlayerInGame`: these accounts are now on this game server.
    pub async fn players_in_game(&mut self, accounts: &[&str]) {
        self.send(&PlayerInGame {
            accounts: accounts.iter().map(|a| (*a).to_owned()).collect(),
        })
        .await;
    }

    /// `PlayerLogout`.
    pub async fn player_logout(&mut self, account: &str) {
        self.send(&PlayerLogout {
            account: account.to_owned(),
        })
        .await;
    }
}
