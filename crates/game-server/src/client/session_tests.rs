//! A fake game client against real sessions on a port-0 listener, with a stub login server
//! link and a fake character list: ProtocolVersion -> KeyPacket -> AuthLogin -> CharSelectionInfo.

use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use l2_crypto::{GameCipher, game_key_from_random};
use l2_net::{Decode, Encode};
use l2_protocol_game::client::{AuthLogin, Logout, ProtocolVersion};
use l2_protocol_game::server::charscreen::CharSelectEntry;
use l2_protocol_game::server::{CharSelectionInfo, KeyPacket, SystemMessage};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::charlist::CharacterInfo;
use super::*;
use crate::state::WorldHandle;

const ACCOUNT: &str = "acc";
const KEY: SessionKey = SessionKey {
    login_ok1: 11,
    login_ok2: 22,
    play_ok1: 33,
    play_ok2: 44,
};

#[derive(Default)]
struct FakeGate {
    logouts: Mutex<Vec<String>>,
}

impl LoginGate for FakeGate {
    fn auth_player<'a>(&'a self, account: &'a str, key: SessionKey) -> BoxFuture<'a, bool> {
        Box::pin(async move { account == ACCOUNT && key == KEY })
    }

    fn player_logout<'a>(&'a self, account: &'a str) -> BoxFuture<'a, ()> {
        self.logouts.lock().unwrap().push(account.to_string());
        Box::pin(async {})
    }

    fn server_id(&self) -> i32 {
        7
    }
}

struct FakeCharacters;

impl CharacterSource for FakeCharacters {
    fn character_list<'a>(&'a self, account: &'a str) -> BoxFuture<'a, Vec<CharacterInfo>> {
        let list = if account == ACCOUNT {
            [("Older", 100), ("Recent", 300), ("Middle", 200)]
                .iter()
                .enumerate()
                .map(|(i, (name, last_access))| CharacterInfo {
                    entry: CharSelectEntry {
                        name: (*name).to_string(),
                        object_id: 0x1000_0000 + i as i32,
                        level: 20,
                        ..CharSelectEntry::default()
                    },
                    last_access: *last_access,
                    delete_time_ms: 0,
                    access_level: 0,
                })
                .collect()
        } else {
            Vec::new()
        };
        Box::pin(async move { list })
    }
}

struct Harness {
    addr: SocketAddr,
    gate: Arc<FakeGate>,
    ctx: Arc<ClientContext>,
    _world_rx: std::sync::mpsc::Receiver<l2_world::WorldCommand>,
    cancel: l2_net::CancellationToken,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

async fn start(packet_encryption: bool) -> Harness {
    let gate = Arc::new(FakeGate::default());
    let (world_tx, world_rx) = std::sync::mpsc::channel();
    let ctx = Arc::new(ClientContext {
        gate: gate.clone(),
        characters: Arc::new(FakeCharacters),
        registry: Arc::default(),
        world: WorldHandle::new(world_tx),
        protocol_list: vec![267, 268, 271, 273],
        packet_encryption,
        max_characters: 7,
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let cancel = l2_net::CancellationToken::new();
    let next_id = AtomicU64::new(1);
    let factory_ctx = Arc::clone(&ctx);
    let factory = move |peer: SocketAddr| {
        Some(factory_ctx.session(SessionId(next_id.fetch_add(1, Ordering::Relaxed)), peer))
    };
    tokio::spawn(l2_net::listen(
        listener,
        Arc::new(ConnectionConfig::default()),
        cancel.clone(),
        factory,
    ));
    Harness {
        addr,
        gate,
        ctx,
        _world_rx: world_rx,
        cancel,
    }
}

/// The client side of one connection.
struct FakeClient {
    stream: TcpStream,
    cipher: Option<GameCipher>,
}

impl FakeClient {
    async fn connect(addr: SocketAddr) -> Self {
        Self {
            stream: TcpStream::connect(addr).await.unwrap(),
            cipher: None,
        }
    }

    async fn send(&mut self, mut body: Vec<u8>) {
        if let Some(cipher) = &mut self.cipher {
            cipher.encrypt(&mut body);
        }
        let mut frame = ((body.len() + 2) as u16).to_le_bytes().to_vec();
        frame.extend(body);
        self.stream.write_all(&frame).await.unwrap();
    }

    /// The next frame body, or `None` when the server closed the connection.
    async fn recv(&mut self) -> Option<Vec<u8>> {
        let mut header = [0u8; 2];
        let read = tokio::time::timeout(Duration::from_secs(5), self.stream.read_exact(&mut header))
            .await
            .expect("timed out waiting for a packet");
        if read.is_err() {
            return None;
        }
        let mut body = vec![0u8; usize::from(u16::from_le_bytes(header)) - 2];
        self.stream.read_exact(&mut body).await.ok()?;
        if let Some(cipher) = &mut self.cipher {
            cipher.decrypt(&mut body);
        }
        Some(body)
    }

    /// ProtocolVersion, then the KeyPacket (in clear). Switches the client cipher on when the
    /// server says so, like the real client.
    async fn handshake(&mut self, version: i32) -> KeyPacket {
        self.send(ProtocolVersion { version }.to_bytes()).await;
        let body = self.recv().await.expect("KeyPacket");
        assert_eq!(body[0], ServerOpcode::VersionCheck.id());
        let key = KeyPacket::decode(&mut l2_net::PacketReader::new(&body[1..])).unwrap();
        if key.use_encryption {
            let mut cipher = GameCipher::new(game_key_from_random(key.key));
            cipher.encrypt(&mut []);
            self.cipher = Some(cipher);
        }
        key
    }

    async fn auth(&mut self, account: &str, key: SessionKey) {
        self.send(
            AuthLogin {
                login_name: account.to_string(),
                play_key2: key.play_ok2,
                play_key1: key.play_ok1,
                login_key1: key.login_ok1,
                login_key2: key.login_ok2,
            }
            .to_bytes(),
        )
        .await;
    }

    async fn expect_closed(&mut self) {
        while self.recv().await.is_some() {}
    }
}

fn decode_char_list(body: &[u8]) -> CharSelectionInfo {
    assert_eq!(body[0], ServerOpcode::CharacterSelectionInfo.id());
    CharSelectionInfo::decode(&mut l2_net::PacketReader::new(&body[1..])).unwrap()
}

async fn wait_for(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition not reached");
}

#[tokio::test]
async fn protocol_key_and_auth_give_the_character_list_encrypted() {
    let h = start(true).await;
    let mut client = FakeClient::connect(h.addr).await;
    let key = client.handshake(273).await;
    assert_eq!(key.result, KeyPacket::RESULT_OK);
    assert!(key.use_encryption);
    assert_eq!(key.server_id, 7);

    client.auth("ACC", KEY).await; // AuthLogin lower-cases the name, like Java
    let list = decode_char_list(&client.recv().await.unwrap());
    assert_eq!(list.account_name, ACCOUNT);
    assert_eq!(list.session_id, KEY.play_ok1);
    assert_eq!(list.max_characters, 7);
    let names: Vec<_> = list.characters.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Older", "Recent", "Middle"]);
    // The most recently used character is highlighted.
    let selected: Vec<_> = list.characters.iter().map(|c| c.selected).collect();
    assert_eq!(selected, [false, true, false]);
    assert!(h.ctx.registry.get(ACCOUNT).is_some());

    // Leaving from the character screen tells the login server right away.
    drop(client);
    wait_for(|| h.gate.logouts.lock().unwrap().as_slice() == [ACCOUNT]).await;
    assert!(h.ctx.registry.is_empty());
}

#[tokio::test]
async fn without_packet_encryption_everything_stays_in_clear() {
    let h = start(false).await;
    let mut client = FakeClient::connect(h.addr).await;
    let key = client.handshake(267).await;
    assert!(!key.use_encryption);
    client.auth(ACCOUNT, KEY).await;
    let list = decode_char_list(&client.recv().await.unwrap());
    assert_eq!(list.characters.len(), 3);
}

#[tokio::test]
async fn a_wrong_protocol_gets_result_0_and_is_closed() {
    let h = start(true).await;
    let mut client = FakeClient::connect(h.addr).await;
    let key = client.handshake(999).await;
    assert_eq!(key.result, KeyPacket::RESULT_WRONG_PROTOCOL);
    client.expect_closed().await;
}

#[tokio::test]
async fn the_c2_ping_is_closed_without_an_answer() {
    let h = start(true).await;
    let mut client = FakeClient::connect(h.addr).await;
    client.send(ProtocolVersion { version: -2 }.to_bytes()).await;
    client.expect_closed().await;
}

#[tokio::test]
async fn auth_before_the_protocol_check_closes() {
    let h = start(false).await;
    let mut client = FakeClient::connect(h.addr).await;
    // AuthLogin is accepted in CONNECTED, but without a good ProtocolVersion first
    // `protocol_ok` is false and Java closes.
    client.auth(ACCOUNT, KEY).await;
    client.expect_closed().await;
    assert!(h.gate.logouts.lock().unwrap().is_empty(), "never authenticated");
}

#[tokio::test]
async fn a_refused_session_key_closes_without_a_list() {
    let h = start(true).await;
    let mut client = FakeClient::connect(h.addr).await;
    client.handshake(273).await;
    let wrong = SessionKey { play_ok1: 1, ..KEY };
    client.auth(ACCOUNT, wrong).await;
    client.expect_closed().await;
    assert!(h.ctx.registry.is_empty());
}

#[tokio::test]
async fn logout_at_the_character_screen_disconnects() {
    let h = start(true).await;
    let mut client = FakeClient::connect(h.addr).await;
    client.handshake(273).await;
    client.auth(ACCOUNT, KEY).await;
    decode_char_list(&client.recv().await.unwrap());
    client.send(Logout.to_bytes()).await;
    client.expect_closed().await;
    wait_for(|| h.gate.logouts.lock().unwrap().len() == 1).await;
}

#[tokio::test]
async fn a_kick_at_the_character_screen_sends_the_message_and_closes() {
    let h = start(true).await;
    let mut client = FakeClient::connect(h.addr).await;
    client.handshake(273).await;
    client.auth(ACCOUNT, KEY).await;
    decode_char_list(&client.recv().await.unwrap());

    let outcome = h.ctx.registry.kick(ACCOUNT, &h.ctx.world);
    assert_eq!(outcome, KickOutcome::Closed);
    let body = client.recv().await.unwrap();
    assert_eq!(
        body,
        SystemMessage::new(SystemMessageId::AnotherPersonHasLoggedInWithTheSameAccount).to_bytes()
    );
    client.expect_closed().await;
    wait_for(|| h.gate.logouts.lock().unwrap().as_slice() == [ACCOUNT]).await;
    assert_eq!(
        h.ctx.registry.kick(ACCOUNT, &h.ctx.world),
        KickOutcome::NotConnected
    );
}
