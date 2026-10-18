#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! End to end through the proxy: a fake client, the proxy, and a fake login server built from
//! `l2-protocol-login` and `l2-crypto`. The fake login server is deliberately independent of the
//! proxy code: it only knows the wire protocol.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use l2_crypto::login::decrypt_init;
use l2_crypto::rsa::encrypt_raw;
use l2_crypto::scramble::unscramble_modulus;
use l2_crypto::{LoginCipher, LoginClientCipher, RsaKeyPair};
use l2_net::{
    CancellationToken, CipherError, ConnectionConfig, Control, Decode, Encode, FrameCipher, FrameHandler,
    PacketReader, listen,
};
use l2_protocol_login::server::{
    AccountKicked, GGAuth, Init, LoginFail, LoginOk, PlayOk, ServerCharInfo, ServerEntry, ServerList,
};
use l2_protocol_login::{
    AuthGameGuard, ClientPacket, ConnectionState, LoginFailReason, RequestAuthLogin, RequestServerList,
    RequestServerLogin, credentials_from_decrypted, decode_client,
};
use l2_proxy::{CaptureReader, CaptureWriter, Dir, Proxy, ProxyConfig, Record};
use rand::{Rng, RngExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver};

const USER: &str = "testuser";
const PASSWORD: &str = "testpass-1234";
const LOGIN_OK: (i32, i32) = (0x1111_1111, 0x2222_2222);
const PLAY_OK: (i32, i32) = (0x3333_3333, 0x4444_4444);

// ---------------------------------------------------------------------------------------------
// Fake login server
// ---------------------------------------------------------------------------------------------

struct LsCipher(LoginCipher);

impl FrameCipher for LsCipher {
    fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8> {
        self.0.encrypt(&payload, 0x1234_5678)
    }

    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
        self.0.decrypt(body).map_err(|e| CipherError(e.to_string()))
    }
}

/// What the fake login server saw, for assertions.
#[derive(Default)]
struct Seen {
    credentials: Vec<(String, String)>,
    game_guard_sessions: Vec<i32>,
}

struct LsShared {
    keys: RsaKeyPair,
    scrambled: [u8; 128],
    gs_addr: SocketAddr,
    seen: Mutex<Seen>,
}

struct FakeLs {
    shared: Arc<LsShared>,
    session_id: i32,
    blowfish_key: [u8; 16],
    state: ConnectionState,
}

impl FrameHandler for FakeLs {
    async fn on_connect(&mut self, ctl: &mut Control<'_>) {
        let init = Init::new(self.session_id, self.shared.scrambled, self.blowfish_key);
        ctl.send(init.to_bytes());
    }

    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        let mut reader = PacketReader::new(&body);
        let opcode = reader.u8().unwrap();
        let Some(packet) = decode_client(self.state, opcode, &mut reader).unwrap() else {
            return;
        };
        match packet {
            ClientPacket::AuthGameGuard(p) => {
                self.shared
                    .seen
                    .lock()
                    .unwrap()
                    .game_guard_sessions
                    .push(p.session_id);
                if p.session_id == self.session_id {
                    self.state = ConnectionState::AuthedGg;
                    ctl.send(
                        GGAuth {
                            response: p.session_id,
                        }
                        .to_bytes(),
                    );
                } else {
                    ctl.close();
                }
            }
            ClientPacket::RequestAuthLogin(p) => {
                let blocks = match *p {
                    RequestAuthLogin::Rsa128 { block } => vec![block],
                    RequestAuthLogin::Rsa256 { block1, block2 } => vec![block1, block2],
                };
                let mut plain = Vec::new();
                for block in blocks {
                    plain.extend(self.shared.keys.decrypt_raw(&block).unwrap());
                }
                let (user, password) = credentials_from_decrypted(&plain);
                let ok = user == USER && password == PASSWORD;
                self.shared
                    .seen
                    .lock()
                    .unwrap()
                    .credentials
                    .push((user, password));
                if ok {
                    self.state = ConnectionState::AuthedLogin;
                    ctl.send(
                        LoginOk {
                            login_ok1: LOGIN_OK.0,
                            login_ok2: LOGIN_OK.1,
                        }
                        .to_bytes(),
                    );
                } else {
                    ctl.send_and_close(
                        LoginFail {
                            reason: LoginFailReason::UserOrPassWrong,
                        }
                        .to_bytes(),
                    );
                }
            }
            ClientPacket::RequestServerList(p) => {
                if (p.login_ok1, p.login_ok2) == LOGIN_OK {
                    ctl.send(server_list(self.shared.gs_addr).to_bytes());
                } else {
                    ctl.close();
                }
            }
            ClientPacket::RequestServerLogin(p) => {
                if (p.login_ok1, p.login_ok2) == LOGIN_OK && p.server_id == 1 {
                    ctl.send(
                        PlayOk {
                            play_ok1: PLAY_OK.0,
                            play_ok2: PLAY_OK.1,
                        }
                        .to_bytes(),
                    );
                } else {
                    ctl.close();
                }
            }
        }
    }
}

fn server_list(gs: SocketAddr) -> ServerList {
    let std::net::IpAddr::V4(ip) = gs.ip() else {
        panic!("test uses IPv4");
    };
    ServerList {
        last_server: 1,
        servers: vec![ServerEntry {
            server_id: 1,
            ip: ip.octets(),
            port: i32::from(gs.port()),
            age_limit: 0,
            pvp: true,
            current_players: 12,
            max_players: 500,
            up: true,
            server_type: 1,
            brackets: false,
        }],
        char_info: Some(vec![ServerCharInfo {
            server_id: 1,
            chars: 2,
            seconds_until_delete: vec![3600],
        }]),
    }
}

fn random_key() -> [u8; 16] {
    let mut key = [0u8; 16];
    rand::rng().fill_bytes(&mut key);
    key
}

fn random_session_id() -> i32 {
    rand::rng().random()
}

fn generate_ls_keys() -> (RsaKeyPair, [u8; 128]) {
    let keys = RsaKeyPair::generate(1024, &mut rand::rng()).unwrap();
    let scrambled = keys.scrambled_modulus().unwrap();
    (keys, scrambled)
}

/// Starts the fake login server and returns its address.
async fn start_fake_ls(gs_addr: SocketAddr) -> (SocketAddr, Arc<LsShared>) {
    let (keys, scrambled) = generate_ls_keys();
    let shared = Arc::new(LsShared {
        keys,
        scrambled,
        gs_addr,
        seen: Mutex::new(Seen::default()),
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let factory_shared = Arc::clone(&shared);
    tokio::spawn(listen(
        listener,
        Arc::new(ConnectionConfig::default()),
        CancellationToken::new(),
        move |_peer| {
            let blowfish_key = random_key();
            Some((
                FakeLs {
                    shared: Arc::clone(&factory_shared),
                    session_id: random_session_id(),
                    blowfish_key,
                    state: ConnectionState::Connected,
                },
                Box::new(LsCipher(LoginCipher::new(&blowfish_key))) as Box<dyn FrameCipher>,
            ))
        },
    ));
    (addr, shared)
}

/// A fake game server: echoes every byte.
async fn start_echo_gs() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 256];
                while let Ok(n) = stream.read(&mut buf).await {
                    if n == 0 || stream.write_all(&buf[..n]).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    addr
}

// ---------------------------------------------------------------------------------------------
// Fake client
// ---------------------------------------------------------------------------------------------

struct TestClient {
    stream: TcpStream,
    cipher: LoginClientCipher,
    modulus: [u8; 128],
    init: Init,
}

impl TestClient {
    async fn connect(addr: SocketAddr) -> Self {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        let mut body = read_frame(&mut stream).await.expect("the proxy sends Init");
        decrypt_init(&mut body).unwrap();
        assert_eq!(body[0], Init::OPCODE);
        let init = Init::decode(&mut PacketReader::new(&body[1..])).unwrap();
        Self {
            cipher: LoginClientCipher::after_init(&init.blowfish_key),
            modulus: unscramble_modulus(&init.scrambled_modulus),
            init,
            stream,
        }
    }

    async fn send(&mut self, packet: &impl Encode) {
        let body = self.cipher.encrypt(&packet.to_bytes());
        let total = u16::try_from(body.len() + 2).unwrap();
        self.stream.write_all(&total.to_le_bytes()).await.unwrap();
        self.stream.write_all(&body).await.unwrap();
    }

    /// The next server packet, decoded. Panics on a different opcode.
    async fn recv<P: Decode>(&mut self, opcode: u8) -> P {
        let mut body = read_frame(&mut self.stream)
            .await
            .expect("a packet from the proxy");
        self.cipher.decrypt(&mut body).unwrap();
        assert_eq!(body[0], opcode, "unexpected opcode");
        P::decode(&mut PacketReader::new(&body[1..])).unwrap()
    }

    /// True when the proxy closed the connection (possibly after the packets already read).
    async fn closed(&mut self) -> bool {
        read_frame(&mut self.stream).await.is_none()
    }

    fn auth_login(&self, user: &str, password: &str) -> RequestAuthLogin {
        // The retail client's layout: user at 0x4E, password at 0xDC, first byte of each block 0
        // so the value is smaller than any modulus.
        let mut plain = vec![0u8; 256];
        plain[0x4E..0x4E + user.len()].copy_from_slice(user.as_bytes());
        plain[0xDC..0xDC + password.len()].copy_from_slice(password.as_bytes());
        let block = |chunk: &[u8]| -> [u8; 128] {
            encrypt_raw(&self.modulus, &[1, 0, 1], chunk)
                .unwrap()
                .try_into()
                .unwrap()
        };
        RequestAuthLogin::Rsa256 {
            block1: block(&plain[..128]),
            block2: block(&plain[128..]),
        }
    }
}

/// Reads one frame body, or `None` when the connection ended.
async fn read_frame(stream: &mut TcpStream) -> Option<Vec<u8>> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut header = [0u8; 2];
        stream.read_exact(&mut header).await.ok()?;
        let total = usize::from(u16::from_le_bytes(header));
        let mut body = vec![0u8; total - 2];
        stream.read_exact(&mut body).await.ok()?;
        Some(body)
    })
    .await
    .expect("timed out waiting for a frame")
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

async fn start_proxy(
    upstream: SocketAddr,
    tweak: impl FnOnce(&mut ProxyConfig),
) -> (Proxy, UnboundedReceiver<Record>) {
    let mut config = ProxyConfig::new("127.0.0.1:0".parse().unwrap(), upstream);
    config.rsa_pool = 2;
    tweak(&mut config);
    let (tx, rx) = mpsc::unbounded_channel();
    (Proxy::start(config, tx).await.unwrap(), rx)
}

/// Writes the records through the real capture writer and reads them back.
fn through_capture_file(rx: &mut UnboundedReceiver<Record>) -> (String, Vec<Record>) {
    let mut writer = CaptureWriter::new(Vec::new()).unwrap();
    while let Ok(record) = rx.try_recv() {
        writer.write_record(&record).unwrap();
    }
    let bytes = writer.into_inner();
    let text = String::from_utf8(bytes.clone()).unwrap();
    let records = CaptureReader::new(bytes.as_slice()).unwrap().read_all().unwrap();
    (text, records)
}

fn names(records: &[Record]) -> Vec<(Dir, &str)> {
    records.iter().map(|r| (r.dir, r.name.as_str())).collect()
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn login_succeeds_through_the_proxy_and_the_capture_is_clean() {
    let gs = start_echo_gs().await;
    let (ls_addr, ls) = start_fake_ls(gs).await;
    let (proxy, mut rx) = start_proxy(ls_addr, |_| {}).await;

    let mut client = TestClient::connect(proxy.local_addr()).await;
    let session_id = client.init.session_id;

    // GG auth: the proxy's Init carries the real session id, so the real server accepts it.
    client.send(&AuthGameGuard { session_id }).await;
    let gg: GGAuth = client.recv(GGAuth::OPCODE).await;
    assert_eq!(gg.response, session_id);

    // The credentials reach the real server intact, re-encrypted with its own key.
    let auth = client.auth_login(USER, PASSWORD);
    client.send(&auth).await;
    let login_ok: LoginOk = client.recv(LoginOk::OPCODE).await;
    assert_eq!((login_ok.login_ok1, login_ok.login_ok2), LOGIN_OK);
    assert_eq!(
        ls.seen.lock().unwrap().credentials,
        vec![(USER.to_owned(), PASSWORD.to_owned())]
    );
    assert_eq!(ls.seen.lock().unwrap().game_guard_sessions, vec![session_id]);

    // The server list now points at the proxy's game relay.
    client
        .send(&RequestServerList {
            login_ok1: LOGIN_OK.0,
            login_ok2: LOGIN_OK.1,
        })
        .await;
    let list: ServerList = client.recv(ServerList::OPCODE).await;
    let relays = proxy.game_relays();
    assert_eq!(relays.len(), 1);
    let (id, relay_port, real) = relays[0];
    assert_eq!((id, real), (1, gs));
    assert_ne!(relay_port, gs.port());
    assert_eq!(list.servers.len(), 1);
    assert_eq!(list.servers[0].ip, [127, 0, 0, 1]);
    assert_eq!(list.servers[0].port, i32::from(relay_port));
    // Everything but the address is untouched.
    let mut expected = server_list(gs);
    expected.servers[0].port = i32::from(relay_port);
    assert_eq!(list, expected);

    client
        .send(&RequestServerLogin {
            login_ok1: LOGIN_OK.0,
            login_ok2: LOGIN_OK.1,
            server_id: 1,
        })
        .await;
    let play_ok: PlayOk = client.recv(PlayOk::OPCODE).await;
    assert_eq!((play_ok.play_ok1, play_ok.play_ok2), PLAY_OK);

    // The game connection works through the relay.
    let mut game = TcpStream::connect(("127.0.0.1", relay_port)).await.unwrap();
    game.write_all(b"hello game").await.unwrap();
    let mut echoed = [0u8; 10];
    tokio::time::timeout(Duration::from_secs(10), game.read_exact(&mut echoed))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&echoed, b"hello game");

    // The capture.
    let (text, records) = through_capture_file(&mut rx);
    assert_eq!(
        names(&records),
        vec![
            (Dir::S2c, "Init"),
            (Dir::C2s, "AuthGameGuard"),
            (Dir::S2c, "GGAuth"),
            (Dir::C2s, "RequestAuthLogin"),
            (Dir::S2c, "LoginOk"),
            (Dir::C2s, "RequestServerList"),
            (Dir::S2c, "ServerList"),
            (Dir::C2s, "RequestServerLogin"),
            (Dir::S2c, "PlayOk"),
        ]
    );
    assert!(
        records.iter().all(|r| r.conn == records[0].conn),
        "one connection"
    );
    assert!(records.iter().all(|r| r.channel == l2_proxy::Channel::Login));

    let field = |i: usize| records[i].fields.as_ref().unwrap();
    // Init is the real server's: its session id is the one the client used.
    assert_eq!(field(0)["session_id"], session_id);
    assert_eq!(field(0)["protocol"], 0xc621);
    assert_eq!(field(1)["session_id"], session_id);
    assert_eq!(field(3), &serde_json::json!({ "credentials": "<redacted>" }));
    assert_eq!(records[3].raw, "00", "no RSA blocks in the capture");
    assert_eq!(field(4)["login_ok1"], LOGIN_OK.0);
    // ServerList: original and rewritten addresses side by side.
    let server = &field(6)["servers"][0];
    assert_eq!(server["ip"], "127.0.0.1");
    assert_eq!(server["port"], gs.port());
    assert_eq!(server["proxy_ip"], "127.0.0.1");
    assert_eq!(server["proxy_port"], relay_port);
    assert_eq!(server["players"], 12);
    assert_eq!(field(6)["characters"][0]["chars"], 2);
    assert_eq!(field(7)["server_id"], 1);
    assert_eq!(field(8)["play_ok2"], PLAY_OK.1);
    // `raw` of the ServerList is the original (not rewritten) payload.
    assert_eq!(records[6].raw_bytes().unwrap(), server_list(gs).to_bytes());

    // Credentials never appear, in any form (the account name is only logged on request).
    assert!(!text.contains(USER), "account name leaked");
    assert!(!text.contains(PASSWORD), "password leaked");
    assert!(!text.contains(&hex::encode(USER)), "account name leaked as hex");
    assert!(!text.contains(&hex::encode(PASSWORD)), "password leaked as hex");

    proxy.stop().await;
}

#[tokio::test]
async fn wrong_password_passes_the_failure_through_and_log_accounts_adds_only_the_name() {
    let gs = start_echo_gs().await;
    let (ls_addr, ls) = start_fake_ls(gs).await;
    let (proxy, mut rx) = start_proxy(ls_addr, |c| c.log_accounts = true).await;

    let mut client = TestClient::connect(proxy.local_addr()).await;
    let session_id = client.init.session_id;
    client.send(&AuthGameGuard { session_id }).await;
    let _: GGAuth = client.recv(GGAuth::OPCODE).await;

    let auth = client.auth_login(USER, "wrong-password");
    client.send(&auth).await;
    let fail: LoginFail = client.recv(LoginFail::OPCODE).await;
    assert_eq!(fail.reason, LoginFailReason::UserOrPassWrong);
    assert!(
        client.closed().await,
        "the failure closes the connection on both sides"
    );
    assert_eq!(ls.seen.lock().unwrap().credentials[0].1, "wrong-password");

    let (text, records) = through_capture_file(&mut rx);
    assert_eq!(records.last().unwrap().name, "LoginFail");
    assert_eq!(
        records.last().unwrap().fields.as_ref().unwrap()["reason_name"],
        "REASON_USER_OR_PASS_WRONG"
    );
    let auth_record = records.iter().find(|r| r.name == "RequestAuthLogin").unwrap();
    let fields = auth_record.fields.as_ref().unwrap();
    assert_eq!(fields["credentials"], "<redacted>");
    assert_eq!(fields["account"], USER);
    assert!(!text.contains("wrong-password"), "password leaked");
    assert!(
        !text.contains(&hex::encode("wrong-password")),
        "password leaked as hex"
    );

    proxy.stop().await;
}

#[tokio::test]
async fn without_rewriting_the_server_list_is_untouched_and_no_relay_opens() {
    let gs = start_echo_gs().await;
    let (ls_addr, _ls) = start_fake_ls(gs).await;
    let (proxy, mut rx) = start_proxy(ls_addr, |c| c.rewrite_server_list = false).await;

    let mut client = TestClient::connect(proxy.local_addr()).await;
    let session_id = client.init.session_id;
    client.send(&AuthGameGuard { session_id }).await;
    let _: GGAuth = client.recv(GGAuth::OPCODE).await;
    let auth = client.auth_login(USER, PASSWORD);
    client.send(&auth).await;
    let _: LoginOk = client.recv(LoginOk::OPCODE).await;
    client
        .send(&RequestServerList {
            login_ok1: LOGIN_OK.0,
            login_ok2: LOGIN_OK.1,
        })
        .await;
    let list: ServerList = client.recv(ServerList::OPCODE).await;
    assert_eq!(list, server_list(gs));
    assert!(proxy.game_relays().is_empty());

    let (_, records) = through_capture_file(&mut rx);
    let list_record = records.iter().find(|r| r.name == "ServerList").unwrap();
    assert!(
        list_record.fields.as_ref().unwrap()["servers"][0]
            .get("proxy_ip")
            .is_none()
    );

    proxy.stop().await;
}

#[tokio::test]
async fn a_second_login_reuses_the_same_game_port() {
    let gs = start_echo_gs().await;
    let (ls_addr, _ls) = start_fake_ls(gs).await;
    let (proxy, _rx) = start_proxy(ls_addr, |_| {}).await;

    let mut ports = Vec::new();
    for _ in 0..2 {
        let mut client = TestClient::connect(proxy.local_addr()).await;
        let session_id = client.init.session_id;
        client.send(&AuthGameGuard { session_id }).await;
        let _: GGAuth = client.recv(GGAuth::OPCODE).await;
        let auth = client.auth_login(USER, PASSWORD);
        client.send(&auth).await;
        let _: LoginOk = client.recv(LoginOk::OPCODE).await;
        client
            .send(&RequestServerList {
                login_ok1: LOGIN_OK.0,
                login_ok2: LOGIN_OK.1,
            })
            .await;
        let list: ServerList = client.recv(ServerList::OPCODE).await;
        ports.push(list.servers[0].port);
    }
    assert_eq!(ports[0], ports[1]);
    assert_eq!(proxy.game_relays().len(), 1);
    proxy.stop().await;
}

#[tokio::test]
async fn an_unreachable_login_server_closes_the_client() {
    // A port nobody listens on.
    let dead = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let (proxy, _rx) = start_proxy(dead, |_| {}).await;
    let mut stream = TcpStream::connect(proxy.local_addr()).await.unwrap();
    assert!(read_frame(&mut stream).await.is_none());
    proxy.stop().await;
}

#[tokio::test]
async fn a_corrupt_client_packet_closes_both_sides() {
    let gs = start_echo_gs().await;
    let (ls_addr, _ls) = start_fake_ls(gs).await;
    let (proxy, _rx) = start_proxy(ls_addr, |_| {}).await;

    let mut client = TestClient::connect(proxy.local_addr()).await;
    // Eight bytes that cannot pass the checksum.
    client
        .stream
        .write_all(&[10, 0, 1, 2, 3, 4, 5, 6, 7, 8])
        .await
        .unwrap();
    assert!(client.closed().await);
    proxy.stop().await;
}

#[tokio::test]
async fn config_errors_are_reported() {
    let upstream: SocketAddr = "127.0.0.1:1".parse().unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    // 0.0.0.0 gives the client nothing to connect to.
    let config = ProxyConfig::new("0.0.0.0:0".parse().unwrap(), upstream);
    assert!(Proxy::start(config, tx.clone()).await.is_err());
    let mut config = ProxyConfig::new("127.0.0.1:0".parse().unwrap(), upstream);
    config.rsa_pool = 0;
    assert!(Proxy::start(config, tx).await.is_err());
}

// Keeps the otherwise unused import honest: the fake LS never kicks accounts.
#[test]
fn account_kicked_is_a_known_packet() {
    assert_eq!(AccountKicked::OPCODE, 0x02);
}
