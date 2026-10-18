#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! The game relay end to end: a fake client, the proxy, and a fake game server that speaks the
//! game wire protocol (`KeyPacket`, then optionally encrypted frames). The client finds the game
//! port the way the real one does: through the rewritten `ServerList` of a (minimal) fake login
//! server. The fake game server is independent of the proxy code.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use l2_crypto::RsaKeyPair;
use l2_crypto::login::decrypt_init;
use l2_crypto::{GameCipher, LoginCipher, LoginClientCipher, game_key_from_random};
use l2_net::{
    CancellationToken, CipherError, ConnectionConfig, Control, Decode, Encode, FrameCipher, FrameHandler,
    PacketReader, listen,
};
use l2_protocol_login::server::{GGAuth, Init, ServerCharInfo, ServerEntry, ServerList};
use l2_protocol_login::{AuthGameGuard, RequestServerList};
use l2_proxy::{CaptureReader, CaptureWriter, Channel, Dir, Proxy, ProxyConfig, Record};
use rand::{Rng, RngExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver};

const KEY_RANDOM: [u8; 8] = [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x07, 0x18];
const SECRET_ACCOUNT: &str = "secretaccount";

// ---------------------------------------------------------------------------------------------
// Minimal fake login server: just enough to hand out a ServerList
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

struct FakeLs {
    scrambled: [u8; 128],
    gs_addr: SocketAddr,
    session_id: i32,
    blowfish_key: [u8; 16],
}

impl FrameHandler for FakeLs {
    async fn on_connect(&mut self, ctl: &mut Control<'_>) {
        ctl.send(Init::new(self.session_id, self.scrambled, self.blowfish_key).to_bytes());
    }

    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        match body.first().copied() {
            Some(AuthGameGuard::OPCODE) => ctl.send(
                GGAuth {
                    response: self.session_id,
                }
                .to_bytes(),
            ),
            Some(RequestServerList::OPCODE) => {
                let std::net::IpAddr::V4(ip) = self.gs_addr.ip() else {
                    panic!("test uses IPv4");
                };
                let list = ServerList {
                    last_server: 1,
                    servers: vec![ServerEntry {
                        server_id: 1,
                        ip: ip.octets(),
                        port: i32::from(self.gs_addr.port()),
                        age_limit: 0,
                        pvp: true,
                        current_players: 1,
                        max_players: 10,
                        up: true,
                        server_type: 1,
                        brackets: false,
                    }],
                    char_info: Some(vec![ServerCharInfo {
                        server_id: 1,
                        chars: 0,
                        seconds_until_delete: vec![],
                    }]),
                };
                ctl.send(list.to_bytes());
            }
            _ => {}
        }
    }
}

async fn start_fake_ls(gs_addr: SocketAddr) -> SocketAddr {
    let keys = RsaKeyPair::generate(1024, &mut rand::rng()).unwrap();
    let scrambled = keys.scrambled_modulus().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(listen(
        listener,
        Arc::new(ConnectionConfig::default()),
        CancellationToken::new(),
        move |_peer| {
            let mut blowfish_key = [0u8; 16];
            rand::rng().fill_bytes(&mut blowfish_key);
            Some((
                FakeLs {
                    scrambled,
                    gs_addr,
                    session_id: rand::rng().random(),
                    blowfish_key,
                },
                Box::new(LsCipher(LoginCipher::new(&blowfish_key))) as Box<dyn FrameCipher>,
            ))
        },
    ));
    addr
}

/// Runs the login steps a client needs to learn the proxy's game port, and returns it.
async fn game_port_via_login(proxy: &Proxy) -> u16 {
    let mut stream = TcpStream::connect(proxy.local_addr()).await.unwrap();
    let mut body = read_frame(&mut stream).await.unwrap();
    decrypt_init(&mut body).unwrap();
    let init = Init::decode(&mut PacketReader::new(&body[1..])).unwrap();
    let cipher = LoginClientCipher::after_init(&init.blowfish_key);

    for packet in [
        AuthGameGuard {
            session_id: init.session_id,
        }
        .to_bytes(),
        RequestServerList {
            login_ok1: 1,
            login_ok2: 2,
        }
        .to_bytes(),
    ] {
        let body = cipher.encrypt(&packet);
        let total = u16::try_from(body.len() + 2).unwrap();
        stream.write_all(&total.to_le_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
        // GGAuth for the first, ServerList for the second.
        read_frame(&mut stream).await.unwrap();
    }
    let relays = proxy.game_relays();
    assert_eq!(relays.len(), 1);
    relays[0].1
}

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
// Fake game server
// ---------------------------------------------------------------------------------------------

/// The exact bytes each side put on the wire, for the "forwarded unchanged" assertions.
#[derive(Default)]
struct Wire {
    server_sent: Vec<u8>,
    server_received: Vec<u8>,
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = u16::try_from(payload.len() + 2).unwrap().to_le_bytes().to_vec();
    out.extend_from_slice(payload);
    out
}

fn key_packet(encryption: bool) -> Vec<u8> {
    let mut p = vec![0x2E, 1]; // VERSION_CHECK, protocol ok
    p.extend_from_slice(&KEY_RANDOM);
    p.extend_from_slice(&i32::from(encryption).to_le_bytes());
    p.extend_from_slice(&1i32.to_le_bytes()); // server id
    p.push(1);
    p.extend_from_slice(&0i32.to_le_bytes()); // obfuscation key
    p
}

/// What the game server sends after the `KeyPacket`.
fn server_script() -> Vec<Vec<u8>> {
    vec![
        vec![0x09, 1, 2, 3, 4, 5, 6, 7],       // CHARACTER_SELECTION_INFO
        vec![0xFE, 0xE8, 0x00, 0xAA, 0xBB],    // EX_SAY2_FAIL (ex packet)
        vec![0x0B, 9, 9, 9, 9, 9, 9, 9, 9, 9], // CHARACTER_SELECTED
        vec![0x07, 0],                         // not in the table
    ]
}

/// What the client sends after the `KeyPacket`. The first is the game `AuthLogin`, carrying
/// an account name that must never reach the capture.
fn client_script() -> Vec<Vec<u8>> {
    let mut auth = vec![0x2B];
    auth.extend(SECRET_ACCOUNT.encode_utf16().flat_map(u16::to_le_bytes));
    auth.extend([0, 0]);
    auth.extend([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    vec![
        auth,
        vec![0xD0, 0x36, 0x00],             // ex: REQUEST_GOTO_LOBBY
        vec![0x12, 0, 0, 0, 0],             // CHARACTER_SELECT
        vec![0x11],                         // ENTER_WORLD
        vec![0xD0, 0x51, 0x00, 1, 0, 0, 0], // ex: EX_BOOKMARK_PACKET with sub id 1
    ]
}

/// A game server for one connection: reads `ProtocolVersion`, answers with the `KeyPacket`, sends
/// its script, reads the client's script and closes.
async fn start_fake_gs(encryption: bool, wire: Arc<Mutex<Wire>>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut cipher = GameCipher::new(game_key_from_random(KEY_RANDOM));

        let hello = read_frame(&mut stream).await.unwrap();
        wire.lock().unwrap().server_received.extend(frame(&hello));
        assert_eq!(hello[0], 0x0E, "ProtocolVersion first");

        // The KeyPacket goes out in clear; the first `encrypt` call only enables the cipher.
        let mut key = key_packet(encryption);
        cipher.encrypt(&mut key);
        let mut out = frame(&key);
        for mut payload in server_script() {
            if encryption {
                cipher.encrypt(&mut payload);
            }
            out.extend(frame(&payload));
        }
        stream.write_all(&out).await.unwrap();
        wire.lock().unwrap().server_sent.extend(&out);

        for _ in 0..client_script().len() {
            let mut body = read_frame(&mut stream).await.unwrap();
            wire.lock().unwrap().server_received.extend(frame(&body));
            if encryption {
                cipher.decrypt(&mut body);
            }
            assert!(!body.is_empty());
        }
        // Closing ends the connection through the proxy.
    });
    addr
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

async fn start_proxy(upstream: SocketAddr) -> (Proxy, UnboundedReceiver<Record>) {
    let mut config = ProxyConfig::new("127.0.0.1:0".parse().unwrap(), upstream);
    config.rsa_pool = 2;
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

/// A whole game session through the proxy. Returns what the capture shows.
async fn run_session(encryption: bool) -> (Arc<Mutex<Wire>>, Vec<u8>, String, Vec<Record>) {
    let wire = Arc::new(Mutex::new(Wire::default()));
    let gs = start_fake_gs(encryption, Arc::clone(&wire)).await;
    let ls = start_fake_ls(gs).await;
    let (proxy, mut rx) = start_proxy(ls).await;
    let port = game_port_via_login(&proxy).await;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut client_cipher = GameCipher::new(game_key_from_random(KEY_RANDOM));
    let mut client_sent: Vec<u8> = Vec::new();

    // ProtocolVersion: before the KeyPacket, so always plain.
    let hello = frame(&[0x0E, 0x21, 0xC6, 0, 0]);
    stream.write_all(&hello).await.unwrap();
    client_sent.extend(&hello);

    // The KeyPacket is plain. Read it, then answer like the real client.
    let key_body = read_frame(&mut stream).await.unwrap();
    assert_eq!(key_body, key_packet(encryption));
    client_cipher.encrypt(&mut []); // the client's cipher is on once it has the key
    let mut received = vec![frame(&key_body)];
    let mut out = Vec::new();
    for mut payload in client_script() {
        if encryption {
            client_cipher.encrypt(&mut payload);
        }
        out.extend(frame(&payload));
    }
    stream.write_all(&out).await.unwrap();
    client_sent.extend(&out);

    // Everything else the server sent, until it closes.
    let mut rest = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut rest))
        .await
        .unwrap()
        .unwrap();
    received.push(rest);
    let received_bytes: Vec<u8> = received.concat();

    // The client sent exactly what the server received, and the other way round.
    {
        let wire = wire.lock().unwrap();
        assert_eq!(
            wire.server_received, client_sent,
            "client -> server bytes changed"
        );
        assert_eq!(wire.server_sent, received_bytes, "server -> client bytes changed");
    }

    // Wait for the proxy to have shut the relay down cleanly, then collect the capture.
    let (text, records) = through_capture_file(&mut rx);
    let game: Vec<Record> = records
        .iter()
        .filter(|r| r.channel == Channel::Game)
        .cloned()
        .collect();
    proxy.stop().await;
    (wire, received_bytes, text, game)
}

type Row<'a> = (Dir, u8, Option<u16>, &'a str);

fn rows(records: &[Record]) -> Vec<Row<'_>> {
    records
        .iter()
        .map(|r| (r.dir, r.opcode, r.ex_opcode, r.name.as_str()))
        .collect()
}

fn expected_rows() -> Vec<Row<'static>> {
    vec![
        (Dir::C2s, 0x0E, None, "PROTOCOL_VERSION"),
        (Dir::S2c, 0x2E, None, "VERSION_CHECK"),
        (Dir::S2c, 0x09, None, "CHARACTER_SELECTION_INFO"),
        (Dir::S2c, 0xFE, Some(0xE8), "EX_SAY2_FAIL"),
        (Dir::S2c, 0x0B, None, "CHARACTER_SELECTED"),
        (Dir::S2c, 0x07, None, "unknown"),
        (Dir::C2s, 0x2B, None, "AUTH_LOGIN"),
        (Dir::C2s, 0xD0, Some(0x36), "REQUEST_GOTO_LOBBY"),
        (Dir::C2s, 0x12, None, "CHARACTER_SELECT"),
        (Dir::C2s, 0x11, None, "ENTER_WORLD"),
        (Dir::C2s, 0xD0, Some(0x51), "EX_BOOKMARK_PACKET"),
    ]
}

/// The two directions are recorded by independent tasks, so only the order within one
/// direction is defined. Sorts into (direction, arrival) groups for comparison.
fn by_direction(mut rows: Vec<Row<'_>>) -> Vec<Row<'_>> {
    rows.sort_by_key(|r| r.0 == Dir::S2c);
    rows
}

fn check_capture(records: &[Record], text: &str, encryption: bool) {
    assert_eq!(by_direction(rows(records)), by_direction(expected_rows()));
    assert!(
        records.iter().all(|r| r.conn == records[0].conn),
        "one connection"
    );
    assert!(
        records
            .iter()
            .all(|r| r.fields.is_none() || r.name == "AUTH_LOGIN")
    );

    // The raw plaintext payloads, decrypted.
    let find = |name: &str| records.iter().find(|r| r.name == name).unwrap();
    assert_eq!(find("PROTOCOL_VERSION").raw, "0e21c60000");
    assert_eq!(find("VERSION_CHECK").raw_bytes().unwrap(), key_packet(encryption));
    assert_eq!(find("EX_SAY2_FAIL").raw, "fee800aabb");
    assert_eq!(find("CHARACTER_SELECT").raw, "1200000000");
    assert_eq!(find("ENTER_WORLD").raw, "11");
    assert_eq!(find("EX_BOOKMARK_PACKET").raw, "d0510001000000");

    // The game AuthLogin is redacted down to its opcode.
    let auth = find("AUTH_LOGIN");
    assert_eq!(auth.raw, "2b");
    assert_eq!(
        auth.fields,
        Some(serde_json::json!({ "credentials": "<redacted>" }))
    );
    assert!(!text.contains(SECRET_ACCOUNT), "account name leaked");
    assert!(
        !text.contains(&hex::encode(SECRET_ACCOUNT)),
        "account name leaked as hex"
    );
    assert!(
        !text.contains(&hex::encode(
            SECRET_ACCOUNT
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        )),
        "account name leaked as UTF-16 hex"
    );
    // ex_opcode is only written where it exists.
    assert!(text.contains("\"ex_opcode\":232"));
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn plain_game_traffic_is_forwarded_unchanged_and_named() {
    let (_wire, _received, text, game) = run_session(false).await;
    check_capture(&game, &text, false);
}

#[tokio::test]
async fn encrypted_game_traffic_is_forwarded_unchanged_and_decrypted_for_the_capture() {
    let (wire, received, text, game) = run_session(true).await;
    check_capture(&game, &text, true);

    // The wire really was encrypted: the capture shows the plaintext, the wire does not.
    let wire = wire.lock().unwrap();
    let plain_ex = [0xFE, 0xE8, 0x00, 0xAA, 0xBB];
    assert!(
        !wire.server_sent.windows(plain_ex.len()).any(|w| w == plain_ex),
        "the fake server encrypted its packets"
    );
    assert_eq!(received, wire.server_sent);
}

#[tokio::test]
async fn an_unreachable_game_server_closes_the_client() {
    // Nothing listens on the "game server" address.
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap()
    };
    let ls = start_fake_ls(dead).await;
    let (proxy, _rx) = start_proxy(ls).await;
    let port = game_port_via_login(&proxy).await;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut rest = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(15), stream.read_to_end(&mut rest))
        .await
        .expect("the proxy closes the connection");
    // A reset is as good as a clean close.
    let _ = read;
    assert!(rest.is_empty());
    proxy.stop().await;
}
