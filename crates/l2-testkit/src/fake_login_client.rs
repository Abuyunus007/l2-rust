//! A minimal H5 login client: raw frames, the real ciphers, RSA credentials.
//!
//! It speaks the wire protocol the way the game client does (Init with the static key, then the
//! per-connection Blowfish key, `AuthGameGuard`, RSA-encrypted `RequestAuthLogin`, ...) but
//! knows nothing about the login server's code. Methods that expect a certain packet panic with
//! a clear message when something else arrives, because they only run inside tests.
//!
//! ```ignore
//! let mut client = FakeClient::connect(server.client_addr().unwrap()).await;
//! let ok = client.login_ok("hero", "secret").await;
//! let list = client.server_list(&ok).await;
//! ```

use std::net::SocketAddr;
use std::time::Duration;

use l2_crypto::LoginClientCipher;
use l2_crypto::login::decrypt_init;
use l2_crypto::rsa::encrypt_raw;
use l2_crypto::scramble::unscramble_modulus;
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_login::client::{AuthGameGuard, RequestAuthLogin, RequestServerList, RequestServerLogin};
use l2_protocol_login::enums::{LoginFailReason, PlayFailReason};
use l2_protocol_login::server::{GGAuth, Init, LoginFail, LoginOk, PlayFail, PlayOk, ServerList};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// How long [`FakeClient::recv`] waits for a packet.
pub const RECV_TIMEOUT: Duration = Duration::from_secs(10);

/// A connected client with the session cipher set up from `Init`.
pub struct FakeClient {
    stream: TcpStream,
    cipher: LoginClientCipher,
    /// The `Init` the server sent (session id, scrambled RSA modulus, Blowfish key).
    pub init: Init,
}

impl FakeClient {
    /// Connects and reads `Init`.
    pub async fn connect(addr: SocketAddr) -> Self {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let mut body = read_frame(&mut stream, RECV_TIMEOUT).await.expect("Init");
        decrypt_init(&mut body).unwrap();
        assert_eq!(body[0], Init::OPCODE, "the first packet is Init");
        let init = Init::decode(&mut PacketReader::new(&body[1..])).unwrap();
        let cipher = LoginClientCipher::after_init(&init.blowfish_key);
        Self { stream, cipher, init }
    }

    /// Connects to a server that refuses the address: the first packet is a `LoginFail` under
    /// the static key instead of `Init`, and the connection is closed. Returns the reason.
    pub async fn connect_refused(addr: SocketAddr) -> LoginFailReason {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let mut body = read_frame(&mut stream, RECV_TIMEOUT).await.expect("LoginFail");
        decrypt_init(&mut body).unwrap(); // the first packet always uses the static key
        let reason = login_fail(&body);
        assert!(
            read_frame(&mut stream, RECV_TIMEOUT).await.is_none(),
            "the server closes the connection after the refusal"
        );
        reason
    }

    /// Encrypts and sends one packet payload.
    pub async fn send(&mut self, payload: &[u8]) {
        let body = self.cipher.encrypt(payload);
        let mut frame = ((body.len() + 2) as u16).to_le_bytes().to_vec();
        frame.extend(body);
        self.stream.write_all(&frame).await.unwrap();
    }

    pub async fn send_packet<P: Encode>(&mut self, packet: &P) {
        self.send(&packet.to_bytes()).await;
    }

    /// The next server packet (decrypted, trailing padding and checksum included), or `None`
    /// when the server closed the connection or nothing came within [`RECV_TIMEOUT`].
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.recv_within(RECV_TIMEOUT).await
    }

    /// Like [`recv`](Self::recv) with another timeout (for "nothing is sent" checks).
    pub async fn recv_within(&mut self, timeout: Duration) -> Option<Vec<u8>> {
        let mut body = read_frame(&mut self.stream, timeout).await?;
        self.cipher.decrypt(&mut body).unwrap();
        Some(body)
    }

    /// Like [`recv`](Self::recv), but a missing packet fails the test.
    pub async fn expect(&mut self) -> Vec<u8> {
        self.recv().await.expect("a packet from the login server")
    }

    /// The server closes the connection without sending anything else.
    pub async fn expect_closed(&mut self) {
        assert!(
            self.recv().await.is_none(),
            "the server should have closed the connection"
        );
    }

    /// `AuthGameGuard` with the session id of `Init`; checks for `GGAuth`.
    pub async fn game_guard(&mut self) {
        let session_id = self.init.session_id;
        self.send_packet(&AuthGameGuard { session_id }).await;
        let reply = self.expect().await;
        assert_eq!(reply[0], GGAuth::OPCODE, "GGAuth");
    }

    /// `RequestAuthLogin` with the H5 (256 byte) layout, without waiting for the reply.
    pub async fn send_login_request(&mut self, user: &str, password: &str) {
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
        self.send_packet(&request).await;
    }

    /// `RequestAuthLogin`; returns the raw reply.
    pub async fn request_login(&mut self, user: &str, password: &str) -> Vec<u8> {
        self.send_login_request(user, password).await;
        self.recv().await.expect("a reply to RequestAuthLogin")
    }

    /// GameGuard, then `RequestAuthLogin`; returns the raw reply (`LoginOk`, `LoginFail`,
    /// `AccountKicked`, ...).
    pub async fn login(&mut self, user: &str, password: &str) -> Vec<u8> {
        self.game_guard().await;
        self.request_login(user, password).await
    }

    /// [`login`](Self::login) that must end in `LoginOk`.
    pub async fn login_ok(&mut self, user: &str, password: &str) -> LoginOk {
        let reply = self.login(user, password).await;
        assert_eq!(reply[0], LoginOk::OPCODE, "LoginOk for {user}");
        LoginOk::decode(&mut PacketReader::new(&reply[1..])).unwrap()
    }

    /// `RequestServerList`; returns the raw reply.
    pub async fn request_server_list(&mut self, ok: &LoginOk) -> Vec<u8> {
        self.send_packet(&RequestServerList {
            login_ok1: ok.login_ok1,
            login_ok2: ok.login_ok2,
        })
        .await;
        self.expect().await
    }

    /// `RequestServerList` that must be answered with a `ServerList`.
    pub async fn server_list(&mut self, ok: &LoginOk) -> ServerList {
        let reply = self.request_server_list(ok).await;
        assert_eq!(reply[0], ServerList::OPCODE, "ServerList");
        ServerList::decode(&mut PacketReader::new(&reply[1..])).unwrap()
    }

    /// `RequestServerLogin`; returns the raw reply (`PlayOk`, `PlayFail`, `LoginFail`).
    pub async fn server_login(&mut self, ok: &LoginOk, server_id: u8) -> Vec<u8> {
        self.send_packet(&RequestServerLogin {
            login_ok1: ok.login_ok1,
            login_ok2: ok.login_ok2,
            server_id,
        })
        .await;
        self.expect().await
    }

    /// `RequestServerLogin` that must be answered with `PlayOk`.
    pub async fn play_ok(&mut self, ok: &LoginOk, server_id: u8) -> PlayOk {
        let reply = self.server_login(ok, server_id).await;
        assert_eq!(reply[0], PlayOk::OPCODE, "PlayOk");
        PlayOk::decode(&mut PacketReader::new(&reply[1..])).unwrap()
    }
}

/// Reads one length-prefixed frame (the length includes its own two bytes). `None` on timeout,
/// EOF or a broken header.
async fn read_frame(stream: &mut TcpStream, timeout: Duration) -> Option<Vec<u8>> {
    let mut header = [0u8; 2];
    tokio::time::timeout(timeout, stream.read_exact(&mut header))
        .await
        .ok()?
        .ok()?;
    let len = usize::from(u16::from_le_bytes(header)).checked_sub(2)?;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.ok()?;
    Some(body)
}

/// The reason of a `LoginFail` reply; fails the test for any other packet.
pub fn login_fail(reply: &[u8]) -> LoginFailReason {
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

/// The reason of a `PlayFail` reply; fails the test for any other packet.
pub fn play_fail(reply: &[u8]) -> PlayFailReason {
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
