//! A fake game client against real sessions on a port-0 listener, with a stub login server
//! link and a fake character list: ProtocolVersion -> KeyPacket -> AuthLogin -> CharSelectionInfo.

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Duration;

use l2_crypto::{GameCipher, game_key_from_random};
use l2_net::{Decode, Encode};
use l2_protocol_game::client::{
    AuthLogin, CharacterCreate, CharacterDelete, CharacterRestore, CharacterSelect, Logout, NewCharacter,
    ProtocolVersion, RequestGotoLobby,
};
use l2_protocol_game::server::charscreen::CharSelectEntry;
use l2_protocol_game::server::{
    CharCreateFail, CharCreateOk, CharDeleteFail, CharDeleteSuccess, CharSelected, CharSelectionInfo,
    KeyPacket, LoginFail, NewCharacterSuccess, ServerClose, SsqInfo, SystemMessage,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::charlist::CharacterInfo;
use super::charscreen::{CharScreen, CharScreenStore, DeleteOutcome, StoreError};
use super::newchar::NewCharacterPlan;
use super::*;
use crate::state::WorldHandle;
use l2_config::GameConfig;
use l2_world::IdFactory;

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
                    access_level: if *name == "Middle" { -1 } else { 0 },
                    title: format!("title of {name}"),
                })
                .collect()
        } else {
            Vec::new()
        };
        Box::pin(async move { list })
    }
}

/// A character screen store that records what was asked and answers from its fields.
struct FakeStore {
    account_characters: Mutex<i64>,
    names: Mutex<HashSet<String>>,
    inserted: Mutex<Vec<NewCharacterPlan>>,
    discarded: Mutex<Vec<i32>>,
    fail_insert: AtomicBool,
    marks: Mutex<Vec<(i32, i32, bool)>>,
    outcome: Mutex<DeleteOutcome>,
    restored: Mutex<Vec<i32>>,
    online: Mutex<Vec<(i32, i32)>>,
}

impl Default for FakeStore {
    fn default() -> Self {
        Self {
            account_characters: Mutex::new(3),
            names: Mutex::default(),
            inserted: Mutex::default(),
            discarded: Mutex::default(),
            fail_insert: AtomicBool::new(false),
            marks: Mutex::default(),
            outcome: Mutex::new(DeleteOutcome::Done),
            restored: Mutex::default(),
            online: Mutex::default(),
        }
    }
}

impl CharScreenStore for FakeStore {
    fn account_character_count<'a>(&'a self, _account: &'a str) -> BoxFuture<'a, i64> {
        Box::pin(async move { *self.account_characters.lock().unwrap() })
    }

    fn name_exists<'a>(&'a self, name: &'a str) -> BoxFuture<'a, bool> {
        Box::pin(async move { self.names.lock().unwrap().contains(name) })
    }

    fn insert_character<'a>(&'a self, plan: &'a NewCharacterPlan) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            if self.fail_insert.load(Ordering::Relaxed) {
                return Err(StoreError("boom".into()));
            }
            self.inserted.lock().unwrap().push(plan.clone());
            Ok(())
        })
    }

    fn discard_character<'a>(&'a self, char_id: i32) -> BoxFuture<'a, ()> {
        Box::pin(async move { self.discarded.lock().unwrap().push(char_id) })
    }

    fn mark_to_delete<'a>(
        &'a self,
        char_id: i32,
        delete_days: i32,
        delete_wedding: bool,
        _now_ms: i64,
    ) -> BoxFuture<'a, Result<DeleteOutcome, StoreError>> {
        Box::pin(async move {
            self.marks
                .lock()
                .unwrap()
                .push((char_id, delete_days, delete_wedding));
            Ok(*self.outcome.lock().unwrap())
        })
    }

    fn restore<'a>(&'a self, char_id: i32) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            self.restored.lock().unwrap().push(char_id);
            Ok(())
        })
    }

    fn set_online<'a>(
        &'a self,
        char_id: i32,
        online: i32,
        _now_ms: i64,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            self.online.lock().unwrap().push((char_id, online));
            Ok(())
        })
    }
}

struct Harness {
    addr: SocketAddr,
    gate: Arc<FakeGate>,
    store: Arc<FakeStore>,
    ids: Arc<IdFactory>,
    _datapack: tempfile::TempDir,
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
    start_with(packet_encryption, |_| {}, 0).await
}

async fn start_with(
    packet_encryption: bool,
    tweak: impl FnOnce(&mut GameConfig),
    select_interval_ticks: i32,
) -> Harness {
    let gate = Arc::new(FakeGate::default());
    let store = Arc::new(FakeStore::default());
    let (datapack_dir, datapack) = super::testdata::datapack();
    let mut config = GameConfig::default();
    // The default list is [""], which refuses every name (see check_request).
    config.manual.player.forbidden_names = vec!["annou".to_owned()];
    tweak(&mut config);
    let ids = Arc::new(IdFactory::new([]).unwrap());
    let (world_tx, world_rx) = std::sync::mpsc::channel();
    let ctx = Arc::new(ClientContext {
        gate: gate.clone(),
        characters: Arc::new(FakeCharacters),
        registry: Arc::default(),
        world: WorldHandle::new(world_tx),
        protocol_list: vec![267, 268, 271, 273],
        packet_encryption,
        max_characters: 7,
        charscreen: CharScreen::new(
            store.clone(),
            datapack,
            Arc::new(config),
            Arc::clone(&ids),
            select_interval_ticks,
        ),
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
        store,
        ids,
        _datapack: datapack_dir,
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

// ---- the character screen ---------------------------------------------------------------------

/// A connected, authenticated client that has read its first character list.
async fn logged_in(h: &Harness) -> FakeClient {
    let mut client = FakeClient::connect(h.addr).await;
    client.handshake(273).await;
    client.auth(ACCOUNT, KEY).await;
    decode_char_list(&client.recv().await.unwrap());
    client
}

fn create_packet(name: &str, class_id: i32) -> CharacterCreate {
    CharacterCreate {
        name: name.to_owned(),
        race: 0,
        is_female: false,
        class_id,
        int: 0,
        str: 0,
        con: 0,
        men: 0,
        dex: 0,
        wit: 0,
        hair_style: 1,
        hair_color: 1,
        face: 1,
    }
}

async fn create_reason(client: &mut FakeClient, packet: CharacterCreate) -> Option<i32> {
    client.send(packet.to_bytes()).await;
    let body = client.recv().await.unwrap();
    if body == CharCreateOk.to_bytes() {
        return None;
    }
    assert_eq!(body[0], ServerOpcode::CharacterCreateFail.id());
    Some(
        CharCreateFail::decode(&mut l2_net::PacketReader::new(&body[1..]))
            .unwrap()
            .reason,
    )
}

#[tokio::test]
async fn the_creation_screen_gets_the_starting_templates() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    client.send(NewCharacter.to_bytes()).await;
    let body = client.recv().await.unwrap();
    assert_eq!(body[0], ServerOpcode::NewCharacterSuccess.id());
    let packet = NewCharacterSuccess::decode(&mut l2_net::PacketReader::new(&body[1..])).unwrap();
    // Only the classes of the test datapack exist: human fighter, mystic and dark fighter.
    let classes: Vec<_> = packet
        .templates
        .iter()
        .map(|t| (t.class_id, t.race, t.base_con))
        .collect();
    assert_eq!(classes, [(0, 0, 43), (10, 0, 24), (31, 2, 32)]);
}

#[tokio::test]
async fn a_valid_character_is_stored_and_confirmed() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    assert_eq!(create_reason(&mut client, create_packet("Newbie", 0)).await, None);
    {
        let inserted = h.store.inserted.lock().unwrap();
        assert_eq!(inserted.len(), 1);
        let plan = &inserted[0];
        assert_eq!(plan.insert.char_name, "Newbie");
        assert_eq!(plan.insert.account_name, ACCOUNT);
        assert_eq!(
            (plan.insert.hair_style, plan.insert.hair_color, plan.insert.face),
            (1, 1, 1)
        );
        assert_eq!(plan.insert.class_id, 0);
        assert_eq!(plan.object_ids.len(), 7);
        // The ids come from the Rust range and stay allocated.
        assert!(plan.object_ids.iter().all(|&id| h.ids.is_allocated(id.into())));
        assert!(plan.char_id >= 0x6000_0000);
    }
    // The lobby request shows the list again.
    client.send(RequestGotoLobby.to_bytes()).await;
    decode_char_list(&client.recv().await.unwrap());
}

#[tokio::test]
async fn every_java_validation_has_its_reason() {
    let h = start(true).await;
    h.store.names.lock().unwrap().insert("Taken".to_owned());
    let mut client = logged_in(&h).await;
    let mut female = create_packet("Girl", 0);
    female.is_female = true;
    female.hair_style = 7;
    let mut male_style = create_packet("Boy", 0);
    male_style.hair_style = 5;
    let mut negative_style = create_packet("Neg", 0);
    negative_style.hair_style = -1;
    let mut face = create_packet("Face", 0);
    face.face = 3;
    let mut color = create_packet("Color", 0);
    color.hair_color = 4;
    let cases = [
        (create_packet("", 0), CharCreateFail::REASON_16_ENG_CHARS),
        (
            create_packet("Seventeen_chars__", 0),
            CharCreateFail::REASON_16_ENG_CHARS,
        ),
        (
            create_packet("xxAnnOUxx", 0),
            CharCreateFail::REASON_INCORRECT_NAME,
        ),
        (
            create_packet("Bad Name", 0),
            CharCreateFail::REASON_INCORRECT_NAME,
        ),
        (
            create_packet("Bad-Name", 0),
            CharCreateFail::REASON_INCORRECT_NAME,
        ),
        (face, CharCreateFail::REASON_CREATION_FAILED),
        (color, CharCreateFail::REASON_CREATION_FAILED),
        (male_style, CharCreateFail::REASON_CREATION_FAILED),
        (female, CharCreateFail::REASON_CREATION_FAILED),
        (negative_style, CharCreateFail::REASON_CREATION_FAILED),
        (
            create_packet("Taken", 0),
            CharCreateFail::REASON_NAME_ALREADY_EXISTS,
        ),
        (
            create_packet("Nobody", 999),
            CharCreateFail::REASON_CREATION_FAILED,
        ),
        // A second class has a template but is no starting class.
        (
            create_packet("Warrior", 1),
            CharCreateFail::REASON_CREATION_FAILED,
        ),
    ];
    for (packet, reason) in cases {
        let name = packet.name.clone();
        assert_eq!(create_reason(&mut client, packet).await, Some(reason), "{name}");
    }
    // 16 characters and a girl's hair style 6 are the limits that still work.
    let mut limit = create_packet("Sixteenchars1234", 0);
    limit.is_female = true;
    limit.hair_style = 6;
    assert_eq!(create_reason(&mut client, limit).await, None);
    assert_eq!(h.store.inserted.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn too_many_characters_and_disallowed_races_are_refused() {
    let h = start_with(true, |c| c.allowed_player_races.allow_human = false, 0).await;
    let mut client = logged_in(&h).await;
    assert_eq!(
        create_reason(&mut client, create_packet("Human", 0)).await,
        Some(CharCreateFail::REASON_CREATION_FAILED)
    );
    *h.store.account_characters.lock().unwrap() = 7;
    assert_eq!(
        create_reason(&mut client, create_packet("Another", 0)).await,
        Some(CharCreateFail::REASON_TOO_MANY_CHARACTERS)
    );
    assert!(h.store.inserted.lock().unwrap().is_empty());

    // CharMaxNumber = 0 means no limit.
    let h = start_with(true, |c| c.server.max_characters_number_per_account = 0, 0).await;
    *h.store.account_characters.lock().unwrap() = 100;
    let mut client = logged_in(&h).await;
    assert_eq!(create_reason(&mut client, create_packet("Many", 0)).await, None);
}

#[tokio::test]
async fn a_failed_insert_cleans_up_and_gives_the_ids_back() {
    let h = start(true).await;
    h.store.fail_insert.store(true, Ordering::Relaxed);
    let mut client = logged_in(&h).await;
    assert_eq!(
        create_reason(&mut client, create_packet("Doomed", 0)).await,
        Some(CharCreateFail::REASON_CREATION_FAILED)
    );
    assert_eq!(h.store.discarded.lock().unwrap().len(), 1);
    assert_eq!(h.ids.used_count(), 0);
}

#[tokio::test]
async fn delete_answers_then_sends_the_list_with_slot_zero_highlighted() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    client.send(CharacterDelete { char_slot: 1 }.to_bytes()).await;
    assert_eq!(client.recv().await.unwrap(), CharDeleteSuccess.to_bytes());
    let list = decode_char_list(&client.recv().await.unwrap());
    let selected: Vec<_> = list.characters.iter().map(|c| c.selected).collect();
    assert_eq!(selected, [true, false, false]);
    // DeleteCharAfterDays = 1 by default; no wedding.
    assert_eq!(
        h.store.marks.lock().unwrap().as_slice(),
        [(0x1000_0001, 1, false)]
    );

    // A clan member, a clan leader, and a slot without a character.
    *h.store.outcome.lock().unwrap() = DeleteOutcome::ClanMember;
    client.send(CharacterDelete { char_slot: 0 }.to_bytes()).await;
    let body = client.recv().await.unwrap();
    assert_eq!(
        body,
        CharDeleteFail {
            reason: CharDeleteFail::REASON_YOU_MAY_NOT_DELETE_CLAN_MEMBER
        }
        .to_bytes()
    );
    decode_char_list(&client.recv().await.unwrap());
    *h.store.outcome.lock().unwrap() = DeleteOutcome::ClanLeader;
    client.send(CharacterDelete { char_slot: 2 }.to_bytes()).await;
    let body = client.recv().await.unwrap();
    assert_eq!(
        body,
        CharDeleteFail {
            reason: CharDeleteFail::REASON_CLAN_LEADERS_MAY_NOT_BE_DELETED
        }
        .to_bytes()
    );
    decode_char_list(&client.recv().await.unwrap());
    client.send(CharacterDelete { char_slot: 9 }.to_bytes()).await;
    decode_char_list(&client.recv().await.unwrap());
    assert_eq!(h.store.marks.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn restore_answers_with_the_list_only() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    client.send(CharacterRestore { char_slot: 2 }.to_bytes()).await;
    let list = decode_char_list(&client.recv().await.unwrap());
    assert!(list.characters[0].selected);
    assert_eq!(h.store.restored.lock().unwrap().as_slice(), [0x1000_0002]);
}

#[tokio::test]
async fn select_and_restore_are_rate_limited() {
    // 600 ticks of 100 ms: the second request inside a minute is dropped silently.
    let h = start_with(true, |_| {}, 600).await;
    let mut client = logged_in(&h).await;
    client.send(CharacterRestore { char_slot: 0 }.to_bytes()).await;
    decode_char_list(&client.recv().await.unwrap());
    client.send(CharacterRestore { char_slot: 1 }.to_bytes()).await;
    // A later packet that is answered proves the ignored one produced nothing.
    client.send(RequestGotoLobby.to_bytes()).await;
    decode_char_list(&client.recv().await.unwrap());
    assert_eq!(h.store.restored.lock().unwrap().as_slice(), [0x1000_0000]);
}

fn select_packet(slot: i32) -> CharacterSelect {
    CharacterSelect {
        char_slot: slot,
        unk1: 0,
        unk2: 0,
        unk3: 0,
        unk4: 0,
    }
}

#[tokio::test]
async fn select_sends_ssq_info_then_char_selected_and_marks_the_character_online() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    client.send(select_packet(1).to_bytes()).await;
    assert_eq!(client.recv().await.unwrap(), SsqInfo { state: 0 }.to_bytes());
    let body = client.recv().await.unwrap();
    assert_eq!(body[0], ServerOpcode::CharacterSelected.id());
    let selected = CharSelected::decode(&mut l2_net::PacketReader::new(&body[1..])).unwrap();
    assert_eq!(selected.name, "Recent");
    assert_eq!(selected.title, "title of Recent");
    assert_eq!(selected.object_id, 0x1000_0001);
    assert_eq!(selected.session_id, KEY.play_ok1);
    assert_eq!(h.store.online.lock().unwrap().as_slice(), [(0x1000_0001, 1)]);

    // A second CharacterSelect is not a valid packet in the ENTERING state.
    client.send(select_packet(0).to_bytes()).await;
    drop(client);
    // Leaving before the world took the character clears the online flag again.
    wait_for(|| h.store.online.lock().unwrap().len() == 2).await;
    assert_eq!(h.store.online.lock().unwrap()[1], (0x1000_0001, 0));
    wait_for(|| h.gate.logouts.lock().unwrap().len() == 1).await;
}

#[tokio::test]
async fn selecting_a_banned_character_closes_the_connection() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    // "Middle" has access level -1 in the fake list.
    client.send(select_packet(2).to_bytes()).await;
    assert_eq!(client.recv().await.unwrap(), ServerClose.to_bytes());
    client.expect_closed().await;
    assert!(h.store.online.lock().unwrap().is_empty());
}

#[tokio::test]
async fn selecting_an_empty_slot_does_nothing() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    client.send(select_packet(7).to_bytes()).await;
    client.send(RequestGotoLobby.to_bytes()).await;
    decode_char_list(&client.recv().await.unwrap());
    assert!(h.store.online.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_refused_session_key_gets_login_fail() {
    let h = start(true).await;
    let mut client = FakeClient::connect(h.addr).await;
    client.handshake(273).await;
    client.auth(ACCOUNT, SessionKey { play_ok1: 1, ..KEY }).await;
    let body = client.recv().await.unwrap();
    assert_eq!(
        body,
        LoginFail {
            reason: LoginFail::SYSTEM_ERROR_LOGIN_LATER
        }
        .to_bytes()
    );
    client.expect_closed().await;
}
