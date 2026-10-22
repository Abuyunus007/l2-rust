#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End to end: the real Rust login server and the real Rust game server in one process, a fake
//! H5 login client and a fake H5 game client (`l2_testkit`) playing the first minutes of a
//! player: log in at the login server, create a character, enter the world, chat, log out, log
//! in again and see what was stored.
//!
//! The tests with a database run only when `L2_TEST_DATABASE_URL` is set (CI sets it for a
//! throwaway MariaDB service, e.g. `mysql://root@127.0.0.1:3306/`). Never point the URL at a
//! server with real data: every test creates its own database named `l2_test_e2e*_*` and drops
//! only that one, but it needs the rights to do so. Both servers listen on port 0 on loopback,
//! so the real ports never open. The login server and the game server share the one database,
//! like the Java dist does by default.
//!
//! There is no datapack in CI. The game server reads a small one that the tests write into a
//! temporary folder (`e2e/datapack.rs`: two starting classes, their items and skills), so the
//! whole flow runs there too. The tests without a database (the packet names against the Java
//! captures, the config files the rig writes, the datapack itself) always run.
//!
//! Packet names and their order are compared with the recorded Java sessions in
//! `fixtures/captures/java-gs-*.jsonl`: everything the Rust server sends must also appear in
//! the Java conversation, in the same order (Java sends more: NPCs around the player, status
//! updates). Values are only compared where they are deterministic.

#[path = "e2e/datapack.rs"]
mod datapack;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use game_server::{Paths, Server, hexid};
use l2_config::GameConfig;
use l2_db::{CharacterRepository, CharacterRow, Pool};
use l2_net::Encode;
use l2_protocol_game::server::charscreen::CharSelectEntry;
use l2_protocol_game::server::{ChatType, CreatureSay, LeaveWorld, SayBody, SenderField};
use l2_protocol_login::server::PlayOk;
use l2_proxy::{Channel, Dir};
use l2_testkit::capture_compare::load_capture;
use l2_testkit::fake_game_client::{FakeGameClient, character_create};
use l2_testkit::fake_login_client::FakeClient as LoginClient;
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
use login_server::state::{LoginState, ServerStatus as ListStatus};

/// The account the login server creates on the first login (`AutoCreateAccounts`).
const ACCOUNT: &str = "e2eaccount";
const PASSWORD: &str = "secret";
const NAME: &str = "Newbie";
/// A protocol revision the game server accepts by default.
const PROTOCOL: i32 = 273;
/// `RUST_FIRST_OBJECT_ID` (D-39): ids of characters created by the Rust server start here.
const RUST_ID_START: i32 = 0x6000_0000;

/// The packets the Rust game server sends after `EnterWorld` (the closing `SKILL_LIST` is the
/// last). Java sends the same ones in this order, with others in between (see the module docs).
const ENTER_WORLD_PACKETS: [&str; 26] = [
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
    // Java: spawnMe -> revalidateZone (the zone the character stands in).
    "EX_SET_COMPASS_ZONE_CODE",
    "EX_ROTATION",
    "ETC_STATUS_UPDATE",
    "EX_STORAGE_MAX_COUNT",
    "L2_FRIEND_LIST",
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
];

/// Every table of the game schema (the files of `crates/l2-db/sql/game`), so the game server
/// finds whatever it reads or writes.
const GAME_SCHEMA: &[&str] = l2_db::GAME_SCHEMA;

// ------------------------------------------------------------------------------------------
// The game server's folder
// ------------------------------------------------------------------------------------------

/// `Server.ini` of the game server: port 0 on loopback, the login server's link port, packet
/// encryption on (the real client always uses it) and the test datapack.
fn game_server_ini(ls_port: u16) -> String {
    format!(
        "GameserverHostname = 127.0.0.1\nGameserverPort = 0\nLoginHost = 127.0.0.1\nLoginPort = {ls_port}\n\
         PacketEncryption = True\nDatapackRoot = ./datapack\nMaximumOnlineUsers = 10\n"
    )
}

/// Everything in the game server's `config` folder except `Database.ini`: `Server.ini`,
/// `Player.ini` (the default `ForbiddenNames` refuses every name, like Java), no select flood
/// protection (the tests select twice within seconds), `ipconfig.xml` (no network scan) and
/// `hexid.txt` (the registration of the sample game server 2 in `gameservers.sql`).
fn write_game_config(config_dir: &Path, ls_port: u16) {
    std::fs::create_dir_all(config_dir).unwrap();
    std::fs::write(config_dir.join("Server.ini"), game_server_ini(ls_port)).unwrap();
    std::fs::write(config_dir.join("Player.ini"), "ForbiddenNames = annoy\n").unwrap();
    // The shipped General.ini says 0; Java's code default (20) would refuse level-1 chat.
    std::fs::write(config_dir.join("General.ini"), "MinimumChatLevel = 0\n").unwrap();
    std::fs::write(
        config_dir.join("FloodProtector.ini"),
        "FloodProtectorCharacterSelectInterval = 0\n",
    )
    .unwrap();
    std::fs::write(
        config_dir.join("ipconfig.xml"),
        "<gameserver address=\"127.0.0.1\" />\n",
    )
    .unwrap();
    hexid::save(config_dir, SAMPLE_SERVER_ID, SAMPLE_HEXID);
}

// ------------------------------------------------------------------------------------------
// The rig: database + login server + game server
// ------------------------------------------------------------------------------------------

/// Waits for state that changes on another task. Longer than `l2_testkit::wait::until`: the
/// servers run unoptimised next to other tests.
async fn until_long(mut condition: impl FnMut() -> bool) {
    for _ in 0..1_200 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not met in 30 seconds");
}

struct Rig {
    db: TestDb,
    ls: login_server::Server,
    gs: Server,
    ls_state: Arc<LoginState>,
    ls_client: SocketAddr,
    gs_client: SocketAddr,
    /// The game server's pool, for looking at the rows it wrote.
    pool: Pool,
    /// Hold the server folders until the end.
    _roots: [tempfile::TempDir; 2],
}

impl Rig {
    /// A throwaway database with both schemas, the login server, and the game server registered
    /// with it. `None` (and a printed note) without `L2_TEST_DATABASE_URL`.
    async fn start(tag: &str) -> Option<Self> {
        let db = TestDb::create(tag).await?;
        for file in GAME_SCHEMA {
            let text: String = file
                .lines()
                .filter(|line| !line.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");
            for statement in text.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                db.exec(statement).await;
            }
        }

        let ls_root = tempfile::tempdir().unwrap();
        db.write_server_folder(ls_root.path(), DEFAULT_SERVER_INI);
        let ls = login_server::Server::start(&login_server::Paths::resolve(
            ls_root.path(),
            Path::new("./config"),
            Path::new("./data"),
        ))
        .await
        .unwrap();
        let ls_state = Arc::clone(ls.state());
        let ls_client = ls.client_addr().unwrap();
        let link_port = ls_state.game_servers.listen_addr().unwrap().port();

        let gs_root = tempfile::tempdir().unwrap();
        db.write_server_folder(gs_root.path(), &game_server_ini(link_port));
        write_game_config(&gs_root.path().join("config"), link_port);
        datapack::write_datapack(&gs_root.path().join("datapack"));
        let gs = Server::start(&Paths::resolve(gs_root.path(), Path::new("./config")))
            .await
            .unwrap();
        let gs_client = gs.client_addr().unwrap();
        let pool = gs.state().pool.clone();

        // The game server connects to the login server by itself; clients can log in once it
        // is registered and shows as up.
        until_long(|| {
            ls_state
                .game_servers
                .registered(SAMPLE_SERVER_ID)
                .is_some_and(|server| server.authed && server.status == ListStatus::Auto)
        })
        .await;
        Some(Self {
            db,
            ls,
            gs,
            ls_state,
            ls_client,
            gs_client,
            pool,
            _roots: [ls_root, gs_root],
        })
    }

    /// Does the login server count `account` as playing on a game server?
    fn in_game(&self, account: &str) -> bool {
        self.ls_state.game_servers.is_account_on_any_gs(account)
    }

    /// Waits until the login server no longer counts `account` as playing.
    async fn wait_until_out_of_game(&self, account: &str) {
        until_long(|| !self.in_game(account)).await;
    }

    /// The whole way in: log in at the login server, look at the server list, ask for the game
    /// server, connect to it, handshake, `AuthLogin`, and read the character list. `chars` is
    /// the number of characters the game server must have reported to the login server (0: not
    /// checked, the list may leave them out).
    async fn login(
        &self,
        account: &str,
        chars: u8,
    ) -> (
        FakeGameClient,
        l2_protocol_game::server::CharSelectionInfo,
        PlayOk,
    ) {
        let server_id = u8::try_from(SAMPLE_SERVER_ID).unwrap();
        let mut login = LoginClient::connect(self.ls_client).await;
        let ok = login.login_ok(account, PASSWORD).await;
        // The login server asks the game server for the characters of the account (the game
        // server answers from the database) before it shows the list.
        until_long(|| {
            self.ls_state
                .clients
                .get(account)
                .is_some_and(|c| c.characters.snapshot().is_complete())
        })
        .await;
        let list = login.server_list(&ok).await;
        assert_eq!(list.servers.len(), 1, "one game server is registered");
        assert!(list.servers[0].up, "and it is up");
        if chars > 0 {
            let counts = list.char_info.expect("the character counts");
            let ours = counts
                .iter()
                .find(|c| c.server_id == server_id)
                .expect("our server");
            assert_eq!(ours.chars, chars, "the characters the game server reported");
        }
        let play = login.play_ok(&ok, server_id).await;

        let mut game = FakeGameClient::connect(self.gs_client).await;
        let key = game.handshake(PROTOCOL).await;
        assert!(key.use_encryption, "PacketEncryption is on");
        assert_eq!(key.server_id, SAMPLE_SERVER_ID, "the id the login server gave");
        game.auth(account, &ok, &play).await;
        let list = game.char_selection_info().await;
        // The account name and session id are written inside every entry (Java), so an empty list
        // carries neither.
        if !list.characters.is_empty() {
            assert_eq!(list.account_name, account);
            assert_eq!(list.session_id, play.play_ok1);
        }
        assert_eq!(list.max_characters, 7, "CharMaxNumber default");
        // The game server told the login server the account is on it. The login connection
        // stayed open until now, like the real client's does while it connects.
        until_long(|| self.in_game(account)).await;
        drop(login);
        (game, list, play)
    }

    /// The `characters` row, read the way the game server reads it.
    async fn row(&self, char_id: i32) -> CharacterRow {
        CharacterRepository::new(self.pool.clone())
            .restore_character(char_id)
            .await
            .unwrap()
            .expect("the character row")
    }

    async fn item_count(&self, char_id: i32) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE owner_id = ?")
            .bind(char_id)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// Creates the character [`NAME`] (a human fighter) on a fresh connection and leaves
    /// through the character screen. Returns its object id.
    async fn create_character_and_leave(&self) -> i32 {
        let (mut game, list, _) = self.login(ACCOUNT, 0).await;
        assert!(list.characters.is_empty());
        game.create_character(&character_create(NAME, 0)).await.unwrap();
        let list = game.goto_lobby().await;
        let char_id = list.characters[0].object_id;
        // Java: Logout at the character screen disconnects.
        assert!(game.logout().await.is_empty());
        self.wait_until_out_of_game(ACCOUNT).await;
        char_id
    }

    /// Stops both servers and drops the database.
    async fn finish(self) {
        self.gs.shutdown(false).await;
        self.ls.shutdown(false).await;
        self.db.drop_db().await;
    }
}

// ------------------------------------------------------------------------------------------
// Comparing with the recorded Java conversations
// ------------------------------------------------------------------------------------------

/// The server packet names of the first game connection of a Java capture that has all of
/// `needles` among them.
fn java_conversation(file: &str, needles: &[&str]) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/captures")
        .join(file);
    let records = load_capture(&path);
    let mut connections: Vec<u64> = Vec::new();
    for record in records.iter().filter(|r| r.channel == Channel::Game) {
        if !connections.contains(&record.conn) {
            connections.push(record.conn);
        }
    }
    for conn in connections {
        let names: Vec<String> = records
            .iter()
            .filter(|r| r.channel == Channel::Game && r.conn == conn && r.dir == Dir::S2c)
            .map(|r| r.name.clone())
            .collect();
        if needles.iter().all(|needle| names.iter().any(|n| n == needle)) {
            return names;
        }
    }
    panic!("no game connection of {file} has all of {needles:?}");
}

/// Every name of `ours` appears in `java`, in the same order (Java may send more in between).
fn assert_follows(ours: &[String], java: &[String]) {
    let mut recorded = java.iter();
    for (index, name) in ours.iter().enumerate() {
        assert!(
            recorded.any(|j| j == name),
            "packet {index} `{name}` of the Rust conversation is not in the Java capture after \
             the previous ones; Rust: {ours:?}"
        );
    }
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

/// The identity of a character as the list shows it: what must not change by playing.
fn assert_same_character(actual: &CharSelectEntry, expected: &CharSelectEntry) {
    assert_eq!(actual.name, expected.name);
    assert_eq!(actual.object_id, expected.object_id);
    assert_eq!(
        (actual.class_id, actual.base_class_id, actual.race, actual.sex),
        (
            expected.class_id,
            expected.base_class_id,
            expected.race,
            expected.sex
        )
    );
    assert_eq!(
        (actual.hair_style, actual.hair_color, actual.face),
        (expected.hair_style, expected.hair_color, expected.face)
    );
    assert_eq!(
        (actual.level, actual.exp, actual.sp),
        (expected.level, expected.exp, expected.sp)
    );
    assert_eq!(
        (actual.x, actual.y, actual.z),
        (expected.x, expected.y, expected.z),
        "the position"
    );
    assert_eq!(actual.paperdoll, expected.paperdoll, "what the character wears");
}

fn echo(char_id: i32, text: &str) -> Vec<u8> {
    CreatureSay {
        sender_object_id: char_id,
        chat_type: ChatType::General,
        sender: SenderField::Name(NAME.to_owned()),
        message_id: -1,
        body: SayBody::Text(text.to_owned()),
    }
    .to_bytes()
}

// ------------------------------------------------------------------------------------------
// The tests with a database
// ------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_new_player_creates_a_character_enters_the_world_chats_and_logs_out() {
    let Some(rig) = Rig::start("e2emain").await else {
        return;
    };

    // Login server: account created, game server listed. Game server: empty character screen.
    let (mut game, list, play) = rig.login(ACCOUNT, 0).await;
    assert!(list.characters.is_empty(), "a new account has no characters");

    // The creation screen offers the starting classes of the datapack.
    let templates = game.new_character().await;
    let classes: Vec<i32> = templates.templates.iter().map(|t| t.class_id).collect();
    assert_eq!(classes, [0, 10]);

    // A name the server refuses (it contains a ForbiddenNames entry), then the real one.
    let refused = game
        .create_character(&character_create("Annoying", 0))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        l2_protocol_game::server::CharCreateFail::REASON_INCORRECT_NAME
    );
    game.create_character(&character_create(NAME, 0)).await.unwrap();

    let list = game.goto_lobby().await;
    assert_eq!(list.characters.len(), 1);
    let created = list.characters[0].clone();
    assert_eq!(created.name, NAME);
    assert!(
        created.object_id >= RUST_ID_START,
        "ids come from the Rust range (D-39)"
    );
    assert_eq!(
        (created.level, created.exp, created.class_id, created.race),
        (1, 0, 0, 0)
    );
    assert_eq!((created.hair_style, created.hair_color, created.face), (1, 1, 1));
    assert!(created.selected, "the only character is highlighted");
    assert!(
        created.paperdoll.iter().any(|&item| item != 0),
        "the starting equipment is worn"
    );
    let char_id = created.object_id;

    // What the creation wrote.
    let row = rig.row(char_id).await;
    assert_eq!(row.account_name.as_deref(), Some(ACCOUNT));
    assert_eq!(row.online, Some(0));
    let items = rig.item_count(char_id).await;
    assert!(items >= 4, "starting items were stored, got {items}");
    let created_at = row.last_access;

    // Select: the character is marked online and the world has not got it yet.
    let selected = game.select_character(0).await;
    assert_eq!((selected.name.as_str(), selected.object_id), (NAME, char_id));
    assert_eq!(selected.session_id, play.play_ok1);
    assert_eq!(rig.row(char_id).await.online, Some(1));

    // Enter the world: the packets of Java's EnterWorld, in its order.
    let sent = game.enter_world().await;
    assert_eq!(sent, ENTER_WORLD_PACKETS);
    assert!(rig.in_game(ACCOUNT));
    // Everything so far must be in the Java capture of the same steps, in the same order.
    // (The refused name above is not in the capture; Java's player picked a good one.)
    let until_enter: Vec<String> = game
        .received_names()
        .into_iter()
        .filter(|name| name != "CHARACTER_CREATE_FAIL")
        .collect();
    assert_follows(
        &until_enter,
        &java_conversation(
            "java-gs-charscreen.jsonl",
            &["CHARACTER_CREATE_SUCCESS", "USER_INFO", "RESTART_RESPONSE"],
        ),
    );

    // Chat: the speaker gets the line back.
    game.say(0, "hello from the e2e test", None).await;
    assert_eq!(game.expect().await, echo(char_id, "hello from the e2e test"));

    // Logout: LeaveWorld, then the connection closes.
    let goodbye = game.logout().await;
    assert_eq!(goodbye, [LeaveWorld.to_bytes()]);

    // The login server hears about the logout only after the character is stored: when it no
    // longer counts the account, the row is already final.
    rig.wait_until_out_of_game(ACCOUNT).await;
    let stored = rig.row(char_id).await;
    assert_eq!(stored.online, Some(0), "offline again");
    assert!(stored.last_access >= created_at);
    assert_eq!(stored.level, Some(1));
    assert_eq!(
        (stored.x, stored.y, stored.z),
        (Some(created.x), Some(created.y), Some(created.z))
    );
    assert_eq!(
        rig.item_count(char_id).await,
        items,
        "no item was lost or duplicated"
    );

    // Log in again: the character is there as it was left, and the login server counts it.
    let (mut again, list, _) = rig.login(ACCOUNT, 1).await;
    assert_eq!(list.characters.len(), 1);
    assert_same_character(&list.characters[0], &created);
    assert!(list.characters[0].selected);
    assert!(again.logout().await.is_empty());
    rig.wait_until_out_of_game(ACCOUNT).await;

    rig.finish().await;
}

#[tokio::test]
async fn what_the_character_had_when_it_entered_is_what_the_next_login_sees() {
    let Some(rig) = Rig::start("e2estate").await else {
        return;
    };
    let char_id = rig.create_character_and_leave().await;

    // The character was played elsewhere (the Java game server, say): level 3, some exp and sp,
    // a title, another place. `lastAccess = 1` shows later that the logout wrote it.
    rig.db
        .exec(&format!(
            "UPDATE characters SET level = 3, exp = 500, sp = 77, x = -70000, y = 250000, z = -3000, \
             heading = 12345, title = 'Tester', lastAccess = 1 WHERE charId = {char_id}"
        ))
        .await;
    let items = rig.item_count(char_id).await;

    // The list shows the stored state; selecting loads all of it.
    let (mut game, list, play) = rig.login(ACCOUNT, 1).await;
    let seen = list.characters[0].clone();
    assert_eq!((seen.level, seen.exp, seen.sp), (3, 500, 77));
    assert_eq!((seen.x, seen.y, seen.z), (-70_000, 250_000, -3_000));
    let selected = game.select_character(0).await;
    assert_eq!(selected.title, "Tester");
    assert_eq!(selected.session_id, play.play_ok1);
    assert_eq!((selected.level, selected.exp, selected.sp), (3, 500, 77));
    assert_eq!(rig.row(char_id).await.online, Some(1));

    assert_eq!(game.enter_world().await, ENTER_WORLD_PACKETS);
    game.say(0, "still here", None).await;
    assert_eq!(game.expect().await, echo(char_id, "still here"));
    // The login, select and enter of the Java session capture (the last packet here is our own
    // chat echo, which the capture has no counterpart of).
    let mut names = game.received_names();
    names.pop();
    assert_follows(
        &names,
        &java_conversation(
            "java-gs-session.jsonl",
            &["SSQ_INFO", "USER_INFO", "RESTART_RESPONSE"],
        ),
    );
    assert_eq!(game.logout().await, [LeaveWorld.to_bytes()]);

    // The store came first: the row has the same state, is offline, and was written (lastAccess).
    rig.wait_until_out_of_game(ACCOUNT).await;
    let stored = rig.row(char_id).await;
    assert_eq!(stored.online, Some(0));
    assert!(stored.last_access > 1, "the logout wrote lastAccess");
    assert_eq!((stored.level, stored.exp, stored.sp), (Some(3), Some(500), 77));
    assert_eq!(
        (stored.x, stored.y, stored.z),
        (Some(-70_000), Some(250_000), Some(-3_000))
    );
    assert_eq!(stored.heading, Some(12_345));
    assert_eq!(stored.title.as_deref(), Some("Tester"));
    assert_eq!(rig.item_count(char_id).await, items);

    // And a third login sees exactly that.
    let (mut third, list, _) = rig.login(ACCOUNT, 1).await;
    assert_same_character(&list.characters[0], &seen);
    assert_eq!(third.select_character(0).await.title, "Tester");
    // Leaving after the select, without entering, clears the online flag.
    drop(third);
    rig.wait_until_out_of_game(ACCOUNT).await;
    until_long_row_offline(&rig, char_id).await;

    rig.finish().await;
}

/// Waits until the row says the character is offline (the session clears it when the client
/// leaves between select and enter).
async fn until_long_row_offline(rig: &Rig, char_id: i32) {
    for _ in 0..1_200 {
        if rig.row(char_id).await.online == Some(0) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the character is still marked online");
}

#[tokio::test]
async fn restart_goes_back_to_the_character_screen_and_the_character_can_enter_again() {
    let Some(rig) = Rig::start("e2erestart").await else {
        return;
    };
    let (mut game, _, _) = rig.login(ACCOUNT, 0).await;
    game.new_character().await;
    game.create_character(&character_create(NAME, 0)).await.unwrap();
    let list = game.goto_lobby().await;
    let created = list.characters[0].clone();
    let char_id = created.object_id;
    game.select_character(0).await;
    assert_eq!(game.enter_world().await, ENTER_WORLD_PACKETS);
    let until_enter = game.received_names().len();
    game.say(0, "before the restart", None).await;
    assert_eq!(game.expect().await, echo(char_id, "before the restart"));

    // Restart: RestartResponse, then the character screen with the stored character.
    let list = game.restart().await;
    assert_eq!(list.characters.len(), 1);
    assert_same_character(&list.characters[0], &created);
    // The Java capture of the same steps (its chat echo is not in it, so leave ours out).
    let mut names = game.received_names();
    names.remove(until_enter);
    assert_follows(
        &names,
        &java_conversation(
            "java-gs-charscreen.jsonl",
            &["CHARACTER_CREATE_SUCCESS", "USER_INFO", "RESTART_RESPONSE"],
        ),
    );

    // The character was stored and is offline; the account is still on the game server (no
    // PlayerLogout: the connection lives on).
    //
    // The test waits for the store before it selects again. The world answers RestartResponse
    // at once and hands the store to the persistence task; a client that selects within
    // milliseconds could have its `online = 1` overwritten by the late `online = 0` of the
    // restart (reported in the card report; a human needs seconds).
    until_long_row_offline(&rig, char_id).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(rig.in_game(ACCOUNT), "a restart does not leave the game server");

    // Select and enter again, chat, and leave for good.
    let selected = game.select_character(0).await;
    assert_eq!(selected.object_id, char_id);
    assert_eq!(game.enter_world().await, ENTER_WORLD_PACKETS);
    game.say(0, "after the restart", None).await;
    assert_eq!(game.expect().await, echo(char_id, "after the restart"));
    assert_eq!(game.logout().await, [LeaveWorld.to_bytes()]);
    rig.wait_until_out_of_game(ACCOUNT).await;
    assert_eq!(rig.row(char_id).await.online, Some(0));

    rig.finish().await;
}

#[tokio::test]
async fn closing_the_connection_in_the_world_stores_the_character_before_the_account_is_freed() {
    let Some(rig) = Rig::start("e2eclose").await else {
        return;
    };
    let char_id = rig.create_character_and_leave().await;
    rig.db
        .exec(&format!(
            "UPDATE characters SET level = 3, exp = 500, lastAccess = 1 WHERE charId = {char_id}"
        ))
        .await;

    let (mut game, _, _) = rig.login(ACCOUNT, 1).await;
    game.select_character(0).await;
    assert_eq!(game.enter_world().await, ENTER_WORLD_PACKETS);
    assert_eq!(rig.row(char_id).await.online, Some(1));
    // The client just goes away (no Logout packet).
    drop(game);

    rig.wait_until_out_of_game(ACCOUNT).await;
    let stored = rig.row(char_id).await;
    assert_eq!(stored.online, Some(0));
    assert!(stored.last_access > 1);
    assert_eq!((stored.level, stored.exp), (Some(3), Some(500)));

    // The account can log in again at once.
    let (mut again, list, _) = rig.login(ACCOUNT, 1).await;
    assert_eq!((list.characters[0].level, list.characters[0].exp), (3, 500));
    assert!(again.logout().await.is_empty());
    rig.wait_until_out_of_game(ACCOUNT).await;

    rig.finish().await;
}

// ------------------------------------------------------------------------------------------
// The tests without a database (they always run)
// ------------------------------------------------------------------------------------------

#[test]
fn the_enter_world_sequence_is_what_the_java_captures_show() {
    let expected = strings(&ENTER_WORLD_PACKETS);
    for file in ["java-gs-session.jsonl", "java-gs-charscreen.jsonl"] {
        let java = java_conversation(file, &["USER_INFO", "RESTART_RESPONSE"]);
        // Skip the list, select and skill list that come before: the enter sequence starts at
        // the first `USER_INFO`.
        let start = java.iter().position(|n| n == "USER_INFO").unwrap();
        assert_follows(&expected, &java[start..]);
    }
}

#[test]
fn the_character_screen_part_of_the_conversation_matches_the_java_capture() {
    // What the Rust game server sends up to the closing SKILL_LIST, for create + select + enter.
    let mut rust = strings(&[
        "VERSION_CHECK",
        "CHARACTER_SELECTION_INFO",
        "NEW_CHARACTER_SUCCESS",
        "CHARACTER_CREATE_SUCCESS",
        "CHARACTER_SELECTION_INFO",
        "SSQ_INFO",
        "CHARACTER_SELECTED",
        "SKILL_LIST",
    ]);
    rust.extend(strings(&ENTER_WORLD_PACKETS));
    rust.extend(strings(&["RESTART_RESPONSE", "CHARACTER_SELECTION_INFO"]));
    assert_follows(
        &rust,
        &java_conversation(
            "java-gs-charscreen.jsonl",
            &["CHARACTER_CREATE_SUCCESS", "USER_INFO", "RESTART_RESPONSE"],
        ),
    );
}

#[test]
fn the_test_datapack_loads_and_has_the_starting_classes() {
    let root = tempfile::tempdir().unwrap();
    datapack::write_datapack(root.path());
    let pack = l2_data::Datapack::load(root.path()).unwrap();
    assert!(pack.player_templates.get_by_id(0).is_some(), "human fighter");
    assert!(pack.player_templates.get_by_id(10).is_some(), "human mystic");
    assert!(!pack.items.is_empty());
    assert!(!pack.initial_equipment.is_empty());
}

#[test]
fn the_config_the_rig_writes_says_what_it_should() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    write_game_config(&config_dir, 4242);
    let config = GameConfig::load(&config_dir).unwrap();
    assert_eq!(
        config.server.port_game, 0,
        "port 0: the real game port never opens"
    );
    assert_eq!(config.server.gameserver_hostname, "127.0.0.1");
    assert_eq!(config.server.game_server_login_port, 4242);
    assert_eq!(config.server.game_server_login_host, "127.0.0.1");
    assert!(config.server.packet_encryption);
    assert_eq!(config.server.max_characters_number_per_account, 7);
    assert_eq!(config.manual.player.forbidden_names, ["annoy"]);
    assert!(config.manual.server.protocol_list.contains(&PROTOCOL));
    let saved = hexid::load(&config_dir).expect("hexid.txt");
    assert_eq!(saved.server_id, SAMPLE_SERVER_ID);
    assert_eq!(l2_db::hex_to_string(&saved.hex_id).unwrap(), SAMPLE_HEXID);
}
