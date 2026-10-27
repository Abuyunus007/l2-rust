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
use super::enter::{LoadError, PlayerLoader};
use super::newchar::NewCharacterPlan;
use super::*;
use crate::runtime::{OutboxMessage, WorldRuntime};
use crate::world_data::{AccessLevels, DatapackData};
use l2_config::GameConfig;
use l2_util::WorldRng;
use l2_world::{IdFactory, ItemData, LsEvent, ObjectId, PersistCommand, PlayerData, ShortcutData, SkillData};

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

/// Loads the characters of [`FakeCharacters`] as level 20 characters with a wand, adena, two
/// skills and a skill shortcut.
struct FakeLoader;

impl PlayerLoader for FakeLoader {
    fn load_player<'a>(
        &'a self,
        account: &'a str,
        char_id: i32,
    ) -> BoxFuture<'a, Result<PlayerData, LoadError>> {
        let result =
            match ["Older", "Recent", "Middle"].get(usize::try_from(char_id - 0x1000_0000).unwrap_or(9)) {
                // "Older" cannot be loaded (a database error, say).
                Some(&"Older") => Err(LoadError::Database("boom".into())),
                Some(name) if account == ACCOUNT => Ok(test_player(char_id, name)),
                _ => Err(LoadError::NotFound(char_id)),
            };
        Box::pin(async move { result })
    }
}

fn test_player(char_id: i32, name: &str) -> PlayerData {
    PlayerData {
        object_id: ObjectId(char_id),
        account: ACCOUNT.into(),
        name: name.into(),
        title: String::new(),
        class_id: 10,
        base_class: 10,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 1,
        hair_color: 2,
        level: 20,
        exp: 100,
        sp: 5,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: -71_338,
        y: 258_271,
        z: -3_104,
        heading: 1_000,
        cur_hp: 90.0,
        max_hp: 100.0,
        cur_mp: 40.0,
        max_mp: 50.0,
        cur_cp: 10.0,
        max_cp: 20.0,
        online_time: 10,
        items: vec![
            ItemData {
                object_id: ObjectId(0x2000_0000 + char_id % 16),
                item_id: 6,
                count: 1,
                enchant_level: 3,
                loc: "PAPERDOLL".into(),
                loc_data: 5,
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: -1,
                ..Default::default()
            },
            ItemData {
                object_id: ObjectId(0x2000_0010 + char_id % 16),
                item_id: 57,
                count: 1_000,
                enchant_level: 0,
                loc: "INVENTORY".into(),
                loc_data: 0,
                custom_type1: 0,
                custom_type2: 0,
                mana_left: -1,
                time: -1,
                ..Default::default()
            },
        ],
        skills: vec![
            SkillData {
                skill_id: 1177,
                skill_level: 1,
            },
            SkillData {
                skill_id: 1216,
                skill_level: 1,
            },
        ],
        shortcuts: vec![ShortcutData {
            slot: 1,
            page: 0,
            kind: 2,
            shortcut_id: 1177,
            level: 1,
        }],
        profile: l2_world::PlayerProfile {
            recom_left: 20,
            ..l2_world::PlayerProfile::default()
        },
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
    /// What the world sent to the persistence side, in order.
    outbox: Arc<Mutex<Vec<OutboxMessage>>>,
    _world: WorldRuntime,
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
    // A real world thread; its outbox is collected for the assertions.
    let outbox: Arc<Mutex<Vec<OutboxMessage>>> = Arc::default();
    let static_data: Arc<dyn l2_world::StaticData> = Arc::new(DatapackData::new(
        Arc::clone(&datapack),
        l2_world::BaseStatBonus::default(),
        AccessLevels::default(),
    ));
    let settings = l2_world::WorldSettings::default();
    let left_acks = crate::persist::LeftAcks::default();
    let world = {
        let (data, settings) = (Arc::clone(&static_data), settings.clone());
        let collected = Arc::clone(&outbox);
        let acks = left_acks.clone();
        WorldRuntime::start_configured(
            WorldRng::from_seed(5),
            Duration::from_secs(900),
            move |world| {
                world.set_static_data(Box::new(data));
                world.set_settings(settings);
            },
            move |mut rx| {
                tokio::spawn(async move {
                    while let Some(message) = rx.recv().await {
                        // Like the persistence task: `PlayerLeft` is "stored" once it is seen.
                        if let crate::runtime::OutboxMessage::Persist(
                            l2_world::PersistCommand::PlayerLeft { object_id, .. },
                        ) = &message
                        {
                            acks.ack(*object_id);
                        }
                        collected.lock().unwrap().push(message);
                    }
                })
            },
        )
        .unwrap()
    };
    let ctx = Arc::new(ClientContext {
        gate: gate.clone(),
        characters: Arc::new(FakeCharacters),
        registry: Arc::default(),
        world: world.handle(),
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
        loader: Arc::new(FakeLoader),
        data: static_data,
        settings,
        left_acks,
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
        outbox,
        _world: world,
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

// ---- entering the world, chat, logout and restart ----------------------------------------------

use l2_protocol_game::client::{
    Appearing, EnterWorld, RequestDestroyItem, RequestItemList, RequestKeyMapping, RequestNetPing,
    RequestRestart, Say2,
};
use l2_protocol_game::server::enterworld::{ExUiSetting, SkillList};
use l2_protocol_game::server::{
    ChatType, CreatureSay, LeaveWorld, NetPing, RestartResponse, SayBody, SenderField,
};

/// The opcode name of a server packet body.
fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn enter_world_packet() -> EnterWorld {
    EnterWorld {
        unknown1: [0; 32],
        unknown2: [0; 4],
        unknown3: [0; 32],
        unknown4: 0,
        tracert: [[0; 4]; 5],
    }
}

/// The sequence Java sends after `EnterWorld` for a plain character (see `enter.rs` of
/// `l2-world` for what is left out and why).
const ENTER_SEQUENCE: [&str; 27] = [
    "USER_INFO",
    "EX_VOTE_SYSTEM_INFO",
    "MACRO_LIST",
    "ITEM_LIST",
    "EX_QUEST_ITEM_LIST",
    "EX_GET_BOOKMARK_INFO",
    "SHORT_CUT_INIT",
    "EX_BASIC_ACTION_LIST",
    "HENNA_INFO",
    "QUEST_LIST",
    "EX_ROTATION",
    "ETC_STATUS_UPDATE",
    "EX_STORAGE_MAX_COUNT",
    "L2_FRIEND_LIST",
    "SYSTEM_MESSAGE",
    // `SevenSigns.sendCurrentPeriodMsg` (M8-SS-01).
    "SYSTEM_MESSAGE",
    "SKILL_COOL_TIME",
    "EX_VOTE_SYSTEM_INFO",
    "EX_RECEIVE_SHOW_POST_FRIEND",
    "EX_NAVIT_ADVENT_POINT_INFO",
    "EX_NAVIT_ADVENT_TIME_CHANGE",
    "USER_INFO",
    "EX_BR_EXTRA_USER_INFO",
    "VALIDATE_LOCATION",
    "ACTION_FAIL",
    "SKILL_LIST",
    "<end>",
];

/// Selects the character of `slot` and reads SSQ_INFO, CHARACTER_SELECTED and SKILL_LIST.
async fn select(client: &mut FakeClient, slot: i32) {
    client.send(select_packet(slot).to_bytes()).await;
    assert_eq!(name_of(&client.recv().await.unwrap()), "SSQ_INFO");
    assert_eq!(name_of(&client.recv().await.unwrap()), "CHARACTER_SELECTED");
    assert_eq!(name_of(&client.recv().await.unwrap()), "SKILL_LIST");
}

/// Sends `EnterWorld` and reads the whole sequence, returning the packet names.
async fn enter_world(client: &mut FakeClient) -> Vec<String> {
    client.send(enter_world_packet().to_bytes()).await;
    let mut names = Vec::new();
    for _ in 0..ENTER_SEQUENCE.len() - 1 {
        names.push(name_of(&client.recv().await.unwrap()));
    }
    names
}

async fn in_world(h: &Harness) -> FakeClient {
    let mut client = logged_in(h).await;
    select(&mut client, 1).await;
    enter_world(&mut client).await;
    client
}

fn outbox_has(h: &Harness, wanted: impl Fn(&OutboxMessage) -> bool) -> bool {
    h.outbox.lock().unwrap().iter().any(wanted)
}

#[tokio::test]
async fn enter_world_sends_the_java_sequence_and_tells_the_login_server() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    select(&mut client, 1).await;

    // Between CharacterSelect and EnterWorld the stored key mapping is asked for.
    client.send(RequestKeyMapping.to_bytes()).await;
    assert_eq!(
        client.recv().await.unwrap(),
        ExUiSetting {
            ui_key_mapping: vec![]
        }
        .to_bytes()
    );

    let names = enter_world(&mut client).await;
    assert_eq!(names, ENTER_SEQUENCE[..ENTER_SEQUENCE.len() - 1]);

    // The registry knows the character is in the world; the selection is not undone on leave.
    assert!(
        h.ctx
            .registry
            .get(ACCOUNT)
            .unwrap()
            .in_world
            .load(Ordering::Acquire)
    );
    assert!(outbox_has(&h, |m| matches!(
        m,
        OutboxMessage::Ls(LsEvent::PlayerInGame { account }) if account == ACCOUNT
    )));
    drop(client);
    // The world stores the character, not the screen's online reset.
    wait_for(|| {
        outbox_has(&h, |m| {
            matches!(m, OutboxMessage::Persist(PersistCommand::PlayerLeft { .. }))
        })
    })
    .await;
    assert_eq!(h.store.online.lock().unwrap().as_slice(), [(0x1000_0001, 1)]);
}

#[tokio::test]
async fn the_skill_list_after_select_describes_the_loaded_skills() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    client.send(select_packet(1).to_bytes()).await;
    client.recv().await.unwrap();
    client.recv().await.unwrap();
    let body = client.recv().await.unwrap();
    let list = SkillList::decode(&mut l2_net::PacketReader::new(&body[1..])).unwrap();
    let ids: Vec<_> = list.skills.iter().map(|s| (s.id, s.level, s.passive)).collect();
    assert_eq!(ids, [(1177, 1, false), (1216, 1, false)]);
}

#[tokio::test]
async fn chat_goes_to_the_speaker_and_a_whisper_to_the_named_player() {
    let h = start(true).await;
    let mut client = in_world(&h).await;
    client
        .send(
            Say2 {
                text: "hello".into(),
                chat_type: 0,
                target: None,
            }
            .to_bytes(),
        )
        .await;
    let expected = CreatureSay {
        sender_object_id: 0x1000_0001,
        chat_type: ChatType::General,
        sender: SenderField::Name("Recent".into()),
        message_id: -1,
        body: SayBody::Text("hello".into()),
    };
    assert_eq!(client.recv().await.unwrap(), expected.to_bytes());

    // A whisper to oneself (the name is compared ignoring case): the receiver's copy shows the
    // sender, the sender's copy shows "->name".
    client
        .send(
            Say2 {
                text: "psst".into(),
                chat_type: 2,
                target: Some("RECENT".into()),
            }
            .to_bytes(),
        )
        .await;
    let to_receiver = CreatureSay {
        chat_type: ChatType::Whisper,
        body: SayBody::Text("psst".into()),
        ..expected.clone()
    };
    let to_sender = CreatureSay {
        sender: SenderField::Name("->Recent".into()),
        ..to_receiver.clone()
    };
    assert_eq!(client.recv().await.unwrap(), to_receiver.to_bytes());
    assert_eq!(client.recv().await.unwrap(), to_sender.to_bytes());

    // Nobody called Ghost: "That player is not online."
    client
        .send(
            Say2 {
                text: "boo".into(),
                chat_type: 2,
                target: Some("Ghost".into()),
            }
            .to_bytes(),
        )
        .await;
    assert_eq!(
        client.recv().await.unwrap(),
        SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline).to_bytes()
    );
}

#[tokio::test]
async fn unknown_in_game_packets_are_ignored_and_the_net_ping_is_answered() {
    let h = start(true).await;
    let mut client = in_world(&h).await;
    // `RequestGetItemFromPet` (a pet packet) is a valid IN_GAME packet without a struct yet; the
    // world gets it as `Other` and must not disconnect.
    client
        .send(vec![
            l2_protocol_game::ClientOpcode::RequestGetItemFromPet.id(),
            0,
            0,
            0,
            0,
        ])
        .await;
    // An opcode the table does not know at all.
    client.send(vec![0x02, 1, 2, 3]).await;
    client.send(RequestNetPing.to_bytes()).await;
    let body = client.recv().await.unwrap();
    assert_eq!(name_of(&body), "NET_PING");
    let ping = NetPing::decode(&mut l2_net::PacketReader::new(&body[1..])).unwrap();
    assert!((0..1440).contains(&ping.game_time));
}

#[tokio::test]
async fn the_inventory_packets_reach_the_world() {
    // M4-INV-01: RequestItemList is answered 250 ms later (ItemList, then ExQuestItemList);
    // RequestDestroyItem of an object the player does not have is answered with a system message.
    let h = start(true).await;
    let mut client = in_world(&h).await;
    client
        .send(
            RequestDestroyItem {
                object_id: 12345,
                count: 1,
            }
            .to_bytes(),
        )
        .await;
    assert_eq!(name_of(&client.recv().await.unwrap()), "SYSTEM_MESSAGE");
    client.send(RequestItemList.to_bytes()).await;
    assert_eq!(name_of(&client.recv().await.unwrap()), "ITEM_LIST");
    assert_eq!(name_of(&client.recv().await.unwrap()), "EX_QUEST_ITEM_LIST");
}

#[tokio::test]
async fn appearing_reaches_the_world_and_is_answered_with_the_user_info() {
    // Java Appearing.runImpl: not teleporting, so only `updateUserInfo()`.
    let h = start(true).await;
    let mut client = in_world(&h).await;
    client.send(Appearing.to_bytes()).await;
    assert_eq!(name_of(&client.recv().await.unwrap()), "USER_INFO");
    assert_eq!(name_of(&client.recv().await.unwrap()), "EX_BR_EXTRA_USER_INFO");
}

#[tokio::test]
async fn logout_stores_the_character_says_goodbye_and_closes() {
    let h = start(true).await;
    let mut client = in_world(&h).await;
    client.send(Logout.to_bytes()).await;
    assert_eq!(client.recv().await.unwrap(), LeaveWorld.to_bytes());
    client.expect_closed().await;
    wait_for(|| h.ctx.registry.is_empty()).await;

    let messages = h.outbox.lock().unwrap().clone();
    let stored = messages
        .iter()
        .position(
            |m| matches!(m, OutboxMessage::Persist(PersistCommand::StorePlayer(p)) if p.name == "Recent"),
        )
        .expect("the final store");
    let left = messages
        .iter()
        .position(|m| {
            matches!(
                m,
                OutboxMessage::Persist(PersistCommand::PlayerLeft { logout: true, .. })
            )
        })
        .expect("PlayerLeft");
    assert!(stored < left, "the store comes before PlayerLeft");
    // The persistence task tells the login server after the store; the session must not.
    assert!(h.gate.logouts.lock().unwrap().is_empty());
}

#[tokio::test]
async fn closing_the_connection_in_the_world_stores_the_character() {
    let h = start(true).await;
    let client = in_world(&h).await;
    drop(client);
    wait_for(|| {
        outbox_has(&h, |m| {
            matches!(
                m,
                OutboxMessage::Persist(PersistCommand::PlayerLeft { logout: true, .. })
            )
        })
    })
    .await;
    assert!(h.gate.logouts.lock().unwrap().is_empty());
}

#[tokio::test]
async fn restart_returns_to_the_character_screen_and_the_character_can_enter_again() {
    let h = start(true).await;
    let mut client = in_world(&h).await;
    client.send(RequestRestart.to_bytes()).await;
    assert_eq!(
        client.recv().await.unwrap(),
        RestartResponse { result: true }.to_bytes()
    );
    let list = decode_char_list(&client.recv().await.unwrap());
    assert_eq!(list.characters.len(), 3);

    // The character was stored, but the account stays on the server: no PlayerLogout.
    assert!(outbox_has(&h, |m| matches!(
        m,
        OutboxMessage::Persist(PersistCommand::PlayerLeft { logout: false, .. })
    )));
    assert!(
        !h.ctx
            .registry
            .get(ACCOUNT)
            .unwrap()
            .in_world
            .load(Ordering::Acquire)
    );
    assert!(h.gate.logouts.lock().unwrap().is_empty());

    // Back in AUTHENTICATED: select and enter again, then leave for good.
    select(&mut client, 1).await;
    assert_eq!(enter_world(&mut client).await.len(), ENTER_SEQUENCE.len() - 1);
    assert!(
        h.ctx
            .registry
            .get(ACCOUNT)
            .unwrap()
            .in_world
            .load(Ordering::Acquire)
    );
    client.send(Logout.to_bytes()).await;
    assert_eq!(client.recv().await.unwrap(), LeaveWorld.to_bytes());
    client.expect_closed().await;
}

#[tokio::test]
async fn a_restart_then_closing_tells_the_login_server_from_the_session() {
    let h = start(true).await;
    let mut client = in_world(&h).await;
    client.send(RequestRestart.to_bytes()).await;
    client.recv().await.unwrap();
    decode_char_list(&client.recv().await.unwrap());
    drop(client);
    wait_for(|| h.gate.logouts.lock().unwrap().as_slice() == [ACCOUNT]).await;
}

#[tokio::test]
async fn a_kick_of_a_character_in_the_world_goes_through_the_world() {
    let h = start(true).await;
    let mut client = in_world(&h).await;
    assert_eq!(h.ctx.registry.kick(ACCOUNT, &h.ctx.world), KickOutcome::WorldKick);
    assert_eq!(
        client.recv().await.unwrap(),
        SystemMessage::new(SystemMessageId::AnotherPersonHasLoggedInWithTheSameAccount).to_bytes()
    );
    client.expect_closed().await;
    wait_for(|| {
        outbox_has(&h, |m| {
            matches!(
                m,
                OutboxMessage::Persist(PersistCommand::PlayerLeft { logout: true, .. })
            )
        })
    })
    .await;
}

#[tokio::test]
async fn a_character_that_cannot_be_loaded_is_not_selected() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    // Java: `client.load` returns null, the packet ends, nothing is sent and nothing is marked.
    client.send(select_packet(0).to_bytes()).await;
    client.send(RequestGotoLobby.to_bytes()).await;
    decode_char_list(&client.recv().await.unwrap());
    assert!(h.store.online.lock().unwrap().is_empty());
}

#[tokio::test]
async fn enter_world_is_not_accepted_before_a_character_was_selected() {
    let h = start(true).await;
    let mut client = logged_in(&h).await;
    // EnterWorld is an ENTERING packet: in AUTHENTICATED it is dropped like any unknown packet.
    client.send(enter_world_packet().to_bytes()).await;
    client.send(RequestGotoLobby.to_bytes()).await;
    decode_char_list(&client.recv().await.unwrap());
    assert!(!outbox_has(&h, |_| true));
}
