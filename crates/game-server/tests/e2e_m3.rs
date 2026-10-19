#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End to end for the world of M3: two fake clients log in through the in-process login server
//! and game server, see each other, walk (against a synthetic geodata wall), talk, click a
//! gatekeeper, pay it and teleport away, chat out of each other's reach and log out.
//!
//! The scenario is written once (`play`) against a `Harness` and runs twice:
//!
//! * `Net`: the real thing. The Rust login server and the Rust game server in one process, a
//!   throwaway MariaDB, two `FakeGameClient`s on sockets. It runs only when
//!   `L2_TEST_DATABASE_URL` is set (CI's MariaDB job; never point it at a database with real
//!   data) and checks the rows the game server wrote. Both servers listen on port 0 on
//!   loopback, so the real ports never open.
//! * `BareWorld`: the same datapack, configuration, geodata, zones and spawns loaded the way the
//!   game server loads them, but with the world driven directly (no sockets, no database, ticks
//!   instead of time). It always runs, so the data of this test and the packets it expects are
//!   checked even where there is no database.
//!
//! There is no datapack and no geodata in CI: `e2e/datapack.rs` (the base classes and items,
//! shared with `e2e.rs`) and `e2e/m3_data.rs` (a gatekeeper with its pages and destinations,
//! zones, a map region and a one-tile geodata region with a wall) write them into a temporary
//! folder. `PathFinding` is 2, so the wall is real; the target of the walk is chosen so that
//! Java corrects the destination and does not start a path search (see `m3_data.rs`).
//!
//! Where the Java game server was recorded doing the same (`fixtures/captures/java-gs-*.jsonl`:
//! the first click on something, walking, entering the world) the names and the order of the
//! packets are compared with the recording. Java has no recording of a gatekeeper dialog, a
//! teleport, `CharInfo` or `Appearing`; those packets are checked against the Java sources by
//! the unit tests of the packets and the world.

#[path = "e2e/datapack.rs"]
mod datapack;
#[path = "e2e/m3_data.rs"]
mod m3_data;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_server::world_data::{DatapackData, world_settings};
use game_server::{Paths, Server, geo, hexid, npcs};
use l2_config::GameConfig;
use l2_data::Datapack;
use l2_db::{CharacterRepository, CharacterRow, Pool};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::compass_zone;
use l2_protocol_game::server::{
    CharInfo, ChatType, CreatureSay, DeleteObject, ExSetCompassZoneCode, InventoryUpdate, LeaveWorld,
    MoveToLocation, MyTargetSelected, NpcHtmlMessage, NpcInfo, SayBody, SenderField, SystemMessage,
    SystemMessageParam, TargetSelected, TeleportToLocation, UserInfo,
};
use l2_protocol_login::server::PlayOk;
use l2_proxy::{Channel, Dir};
use l2_testkit::capture_compare::load_capture;
use l2_testkit::fake_game_client::{
    FakeGameClient, character_create, enter_world_packet, names_of, packet_name,
};
use l2_testkit::fake_login_client::FakeClient as LoginClient;
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
use l2_world::{
    ClientInput, IdFactory, ItemData, LeaveReason, LsEvent, ObjectId, Outbox, PacketSink, PersistCommand,
    PlayerData, PlayerProfile, SessionId, StaticData, World, WorldCommand, WorldRng,
};
use login_server::state::{LoginState, ServerStatus as ListStatus};

const ACCOUNT_A: &str = "e2eacca";
const ACCOUNT_B: &str = "e2eaccb";
const PASSWORD: &str = "secret";
const NAME_A: &str = "Alice";
const NAME_B: &str = "Bobby";
/// A protocol revision the game server accepts by default.
const PROTOCOL: i32 = 273;
/// `StartingAdena` of the test configuration: what both characters begin with.
const START_ADENA: i64 = 1_000;
const ADENA_ID: i32 = 57;

/// Every table of the game schema (the files of `crates/l2-db/sql/game`), so the game server
/// finds whatever it reads or writes.
const GAME_SCHEMA: [&str; 26] = [
    include_str!("../../l2-db/sql/game/characters.sql"),
    include_str!("../../l2-db/sql/game/items.sql"),
    include_str!("../../l2-db/sql/game/item_attributes.sql"),
    include_str!("../../l2-db/sql/game/character_skills.sql"),
    include_str!("../../l2-db/sql/game/character_skills_save.sql"),
    include_str!("../../l2-db/sql/game/character_shortcuts.sql"),
    include_str!("../../l2-db/sql/game/character_macroses.sql"),
    include_str!("../../l2-db/sql/game/character_reco_bonus.sql"),
    include_str!("../../l2-db/sql/game/character_contacts.sql"),
    include_str!("../../l2-db/sql/game/character_friends.sql"),
    include_str!("../../l2-db/sql/game/character_hennas.sql"),
    include_str!("../../l2-db/sql/game/character_quests.sql"),
    include_str!("../../l2-db/sql/game/character_recipebook.sql"),
    include_str!("../../l2-db/sql/game/character_subclasses.sql"),
    include_str!("../../l2-db/sql/game/heroes.sql"),
    include_str!("../../l2-db/sql/game/olympiad_nobles.sql"),
    include_str!("../../l2-db/sql/game/seven_signs.sql"),
    include_str!("../../l2-db/sql/game/pets.sql"),
    include_str!("../../l2-db/sql/game/merchant_lease.sql"),
    include_str!("../../l2-db/sql/game/character_raid_points.sql"),
    include_str!("../../l2-db/sql/game/character_instance_time.sql"),
    include_str!("../../l2-db/sql/game/character_variables.sql"),
    include_str!("../../l2-db/sql/game/mods_wedding.sql"),
    include_str!("../../l2-db/sql/game/clan_data.sql"),
    include_str!("../../l2-db/sql/game/itemsonground.sql"),
    include_str!("../../l2-db/sql/game/messages.sql"),
];

// ------------------------------------------------------------------------------------------
// The game server's folder (shared by the real server and the bare world)
// ------------------------------------------------------------------------------------------

/// `Server.ini` of the game server: port 0 on loopback, the login server's link port, packet
/// encryption on (the real client always uses it) and the test datapack.
fn game_server_ini(ls_port: u16) -> String {
    format!(
        "GameserverHostname = 127.0.0.1\nGameserverPort = 0\nLoginHost = 127.0.0.1\nLoginPort = {ls_port}\n\
         PacketEncryption = True\nDatapackRoot = ./datapack\nMaximumOnlineUsers = 10\n"
    )
}

/// Everything in the game server's `config` folder except `Database.ini`.
///
/// Besides what `e2e.rs` writes (`Server.ini`, the forbidden names, no select flood protection,
/// `ipconfig.xml`, `hexid.txt`) the M3 rig sets:
/// * `Player.ini`: `StartingAdena` (something to pay the gatekeeper with) and
///   `MaxFreeTeleportLevel = 0` (the gatekeeper is free up to level 40 by default, and the
///   characters are level 1),
/// * `General.ini`: `MaxNpcAnimation = 0`, so the gatekeeper does not greet at random (a random
///   `SocialAction` would make the packet lists of the clicks vary), and `MinimumChatLevel = 0`
///   (the shipped value; the Java code default is 20, and a level 1 character may not use
///   general chat then),
/// * `GeoEngine.ini`: `PathFinding = 2` (the shipped value; the Java default of the key is 0).
fn write_game_config(config_dir: &Path, ls_port: u16) {
    std::fs::create_dir_all(config_dir).unwrap();
    std::fs::write(config_dir.join("Server.ini"), game_server_ini(ls_port)).unwrap();
    std::fs::write(
        config_dir.join("Player.ini"),
        format!("ForbiddenNames = annou\nStartingAdena = {START_ADENA}\nMaxFreeTeleportLevel = 0\n"),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("General.ini"),
        "MaxNpcAnimation = 0\nMinimumChatLevel = 0\n",
    )
    .unwrap();
    std::fs::write(
        config_dir.join("GeoEngine.ini"),
        "PathFinding = 2\nGeoDataPath = ./geodata/\nAvoidObstructedPathNodes = True\n",
    )
    .unwrap();
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

/// The datapack of this test: the base classes and items plus the world data of `m3_data.rs`.
fn write_datapack(root: &Path) {
    datapack::write_datapack(root);
    m3_data::write_world_data(root);
}

// ------------------------------------------------------------------------------------------
// Looking at packets
// ------------------------------------------------------------------------------------------

/// Decodes the fields after the opcode (the opcode and, for 0xFE, the ex id are skipped).
fn decode_any<P: Decode>(body: &[u8]) -> P {
    let skip = if body[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&body[skip..])).unwrap_or_else(|e| {
        panic!(
            "cannot decode {} as {}: {e:?}",
            packet_name(body),
            std::any::type_name::<P>()
        )
    })
}

/// Every packet of `batch` called `name`, decoded.
fn all<P: Decode>(batch: &[Vec<u8>], name: &str) -> Vec<P> {
    batch
        .iter()
        .filter(|body| packet_name(body) == name)
        .map(|body| decode_any(body))
        .collect()
}

/// The one packet of `batch` called `name`, decoded.
fn one<P: Decode>(batch: &[Vec<u8>], name: &str) -> P {
    let mut found = all::<P>(batch, name);
    assert_eq!(found.len(), 1, "exactly one {name} in {:?}", names_of(batch));
    found.remove(0)
}

fn names(batch: &[Vec<u8>]) -> Vec<String> {
    names_of(batch)
}

/// `wanted` appears in `names` in this order (other packets may be in between).
fn assert_in_order(names: &[String], wanted: &[&str]) {
    let mut rest = names.iter();
    for want in wanted {
        assert!(
            rest.any(|name| name == want),
            "`{want}` is missing or out of order in {names:?} (wanted {wanted:?})"
        );
    }
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

fn echo(char_id: i32, name: &str, text: &str) -> Vec<u8> {
    CreatureSay {
        sender_object_id: char_id,
        chat_type: ChatType::General,
        sender: SenderField::Name(name.to_owned()),
        message_id: -1,
        body: SayBody::Text(text.to_owned()),
    }
    .to_bytes()
}

// ------------------------------------------------------------------------------------------
// Comparing with the recorded Java conversations
// ------------------------------------------------------------------------------------------

/// `(direction, name)` of the first game connection of a Java capture that has all of `needles`
/// among the packets the server sent.
fn java_conversation(file: &str, needles: &[&str]) -> Vec<(Dir, String)> {
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
        let both: Vec<(Dir, String)> = records
            .iter()
            .filter(|r| r.channel == Channel::Game && r.conn == conn)
            .map(|r| (r.dir, r.name.clone()))
            .collect();
        let has = |needle: &&str| both.iter().any(|(dir, name)| *dir == Dir::S2c && name == needle);
        if needles.iter().all(has) {
            return both;
        }
    }
    panic!("no game connection of {file} has all of {needles:?}");
}

/// The names the server sent, in order.
fn server_names(conversation: &[(Dir, String)]) -> Vec<String> {
    conversation
        .iter()
        .filter(|(dir, _)| *dir == Dir::S2c)
        .map(|(_, name)| name.clone())
        .collect()
}

/// Every name of `ours` appears in `java`, in the same order (Java may send more in between).
fn assert_follows(ours: &[String], java: &[String]) {
    let mut recorded = java.iter();
    for (index, name) in ours.iter().enumerate() {
        assert!(
            recorded.any(|j| j == name),
            "packet {index} `{name}` of the Rust conversation is not in the Java capture after the \
             previous ones; Rust: {ours:?}"
        );
    }
}

/// `ours` is a run of consecutive packets somewhere in what Java sent.
fn assert_java_has_run(ours: &[String], java: &[String]) {
    assert!(
        java.windows(ours.len()).any(|window| window == ours),
        "Java never sent {ours:?} in a row"
    );
}

/// Java answered the client packet `request` with `answer` as the very next packet.
fn assert_java_answers(conversation: &[(Dir, String)], request: &str, answer: &str) {
    assert!(
        conversation.windows(2).any(|pair| pair[0].0 == Dir::C2s
            && pair[0].1 == request
            && pair[1].0 == Dir::S2c
            && pair[1].1 == answer),
        "Java never answered {request} with {answer} straight away"
    );
}

const JAVA_SESSION: &str = "java-gs-session.jsonl";
const JAVA_NEEDLES: [&str; 3] = ["SSQ_INFO", "USER_INFO", "RESTART_RESPONSE"];

// ------------------------------------------------------------------------------------------
// The scenario
// ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Who {
    A,
    B,
}

impl Who {
    fn index(self) -> usize {
        match self {
            Who::A => 0,
            Who::B => 1,
        }
    }
}

/// What the store of a character that left says, and what its client was told on the way out.
#[derive(Debug, PartialEq, Eq)]
struct Left {
    position: (i32, i32, i32),
    adena: i64,
    /// The objects the leaving player is told to forget (`DeleteObject`, Java
    /// `World.removeVisibleObject`), before `LeaveWorld`.
    forgot: Vec<i32>,
}

/// The packets of a logout: a `DeleteObject` for everything the player saw, then `LeaveWorld`.
/// Returns the ids of the deleted objects.
fn assert_goodbye(batch: &[Vec<u8>]) -> Vec<i32> {
    let (last, before) = batch.split_last().expect("the goodbye");
    assert_eq!(
        last,
        &LeaveWorld.to_bytes(),
        "the last packet is LeaveWorld: {:?}",
        names(batch)
    );
    before
        .iter()
        .map(|body| {
            assert_eq!(packet_name(body), "DELETE_OBJECT", "{:?}", names(batch));
            decode_any::<DeleteObject>(body).object_id
        })
        .collect()
}

/// The two ways to play the scenario. `collect` and `collect_quiet` return what the player's
/// client received since the last call; `collect` waits for the `times`-th packet called
/// `name` and then lets the rest of the event arrive.
trait Harness {
    fn object_id(&self, who: Who) -> i32;
    /// Logs in, selects and enters the world; returns every packet up to the closing `SKILL_LIST`.
    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>>;
    async fn move_to(&mut self, who: Who, target: (i32, i32, i32), origin: (i32, i32, i32));
    async fn validate_position(&mut self, who: Who, at: (i32, i32, i32), heading: i32);
    async fn click(&mut self, who: Who, object_id: i32);
    async fn bypass(&mut self, who: Who, command: &str);
    async fn appearing(&mut self, who: Who);
    async fn say(&mut self, who: Who, text: &str);
    async fn collect(&mut self, who: Who, name: &str, times: usize) -> Vec<Vec<u8>>;
    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>>;
    /// Logs out and returns what was stored.
    async fn leave(&mut self, who: Who) -> Left;
}

/// Both characters of the scenario and what they did.
struct Played {
    left_a: Left,
    left_b: Left,
    /// Where the gatekeeper's `TeleportToLocation` sent character A.
    teleported_to: (i32, i32, i32),
    npc_id: i32,
}

async fn play<H: Harness>(h: &mut H) -> Played {
    let (a, b) = (h.object_id(Who::A), h.object_id(Who::B));
    let java = java_conversation(JAVA_SESSION, &JAVA_NEEDLES);

    // ---- A enters alone: it sees the gatekeeper and stands in the peace zone ----------------
    let entered_a = h.enter(Who::A).await;
    let enter_names = names(&entered_a);
    assert_eq!(enter_names.last().map(String::as_str), Some("SKILL_LIST"));
    assert!(
        !enter_names.contains(&"CHAR_INFO".to_owned()),
        "nobody else is in the world yet: {enter_names:?}"
    );
    let npc: NpcInfo = one(&entered_a, "NPC_INFO");
    assert_eq!(
        (npc.x, npc.y, npc.z),
        m3_data::GATEKEEPER_POS,
        "the spawn of the datapack"
    );
    let compass: ExSetCompassZoneCode = one(&entered_a, "EX_SET_COMPASS_ZONE_CODE");
    assert_eq!(
        compass.zone_type,
        compass_zone::PEACEZONE,
        "the start is a peace zone"
    );
    // Java's EnterWorld, from the first UserInfo on (NPCs and the compass icon are in it too).
    let java_names = server_names(&java);
    let java_enter = &java_names[java_names.iter().position(|n| n == "USER_INFO").unwrap()..];
    assert_follows(&enter_names, java_enter);
    let npc_id = npc.object_id;

    // ---- B enters: they see each other ------------------------------------------------------
    let entered_b = h.enter(Who::B).await;
    let char_info_of_a: CharInfo = one(&entered_b, "CHAR_INFO");
    assert_eq!(
        (char_info_of_a.object_id, char_info_of_a.name.as_str()),
        (a, NAME_A)
    );
    assert_eq!(
        (char_info_of_a.x, char_info_of_a.y, char_info_of_a.z),
        m3_data::START
    );
    assert_eq!(one::<NpcInfo>(&entered_b, "NPC_INFO").object_id, npc_id);
    let seen_by_a = h.collect(Who::A, "CHAR_INFO", 1).await;
    let char_info_of_b: CharInfo = one(&seen_by_a, "CHAR_INFO");
    assert_eq!(
        (char_info_of_b.object_id, char_info_of_b.name.as_str()),
        (b, NAME_B)
    );
    assert_eq!(
        (char_info_of_b.x, char_info_of_b.y, char_info_of_b.z),
        m3_data::START
    );

    // ---- A tries to walk into the wall: refused, and B hears nothing ------------------------
    h.move_to(Who::A, m3_data::inside_the_wall(), m3_data::START)
        .await;
    assert_eq!(names(&h.collect(Who::A, "ACTION_FAIL", 1).await), ["ACTION_FAIL"]);
    assert!(
        h.collect_quiet(Who::B).await.is_empty(),
        "B sees nothing of a refused walk"
    );

    // ---- A walks east towards a spot behind the wall: it stops in front of the wall ---------
    h.move_to(Who::A, m3_data::behind_the_wall(), m3_data::START)
        .await;
    let moves_a = h.collect(Who::A, "MOVE_TO_LOCATION", 2).await;
    let moves_b = h.collect(Who::B, "MOVE_TO_LOCATION", 2).await;
    let stop = m3_data::last_free_spot();
    for (who_sees, batch) in [("A", &moves_a), ("B", &moves_b)] {
        let moves: Vec<MoveToLocation> = all(batch, "MOVE_TO_LOCATION");
        assert_eq!(
            moves.len(),
            2,
            "{who_sees}: the start and the arrival of the walk"
        );
        for step in &moves {
            assert_eq!(step.object_id, a);
            assert_eq!(
                (step.dest_x, step.dest_y, step.dest_z),
                stop,
                "{who_sees}: the destination is corrected to the last cell before the wall"
            );
        }
        assert_eq!(
            (moves[0].x, moves[0].y, moves[0].z),
            m3_data::START,
            "the walk starts at the start"
        );
        assert_eq!((moves[1].x, moves[1].y, moves[1].z), stop, "and ends at the wall");
    }
    // The player who walks and the one who watches get the same bytes.
    let walk_bytes = |batch: &[Vec<u8>]| -> Vec<Vec<u8>> {
        batch
            .iter()
            .filter(|body| packet_name(body) == "MOVE_TO_LOCATION")
            .cloned()
            .collect()
    };
    assert_eq!(walk_bytes(&moves_a), walk_bytes(&moves_b));
    // The client agrees with where the server put it: nothing is corrected, nobody hears more.
    h.validate_position(Who::A, stop, 0).await;
    assert!(h.collect_quiet(Who::A).await.is_empty());
    assert!(h.collect_quiet(Who::B).await.is_empty());
    assert_java_answers(&java, "MOVE_TO_LOCATION", "MOVE_TO_LOCATION");

    // ---- general chat reaches the neighbour --------------------------------------------------
    h.say(Who::A, "hello neighbour").await;
    let line = echo(a, NAME_A, "hello neighbour");
    assert_eq!(
        h.collect(Who::A, "SAY2", 1).await,
        std::slice::from_ref(&line),
        "the speaker gets the line back"
    );
    assert_eq!(
        h.collect(Who::B, "SAY2", 1).await,
        [line],
        "and so does B, 100 units away"
    );

    // ---- the gatekeeper: click to target, click to talk -------------------------------------
    h.click(Who::A, npc_id).await;
    let target_a = h.collect(Who::A, "ACTION_FAIL", 1).await;
    let target_names = [
        "VALIDATE_LOCATION",
        "MY_TARGET_SELECTED",
        "STATUS_UPDATE",
        "ACTION_FAIL",
    ];
    assert_eq!(names(&target_a), target_names);
    assert_eq!(
        one::<MyTargetSelected>(&target_a, "MY_TARGET_SELECTED").object_id,
        npc_id
    );
    // The same four packets are what Java sent for a first click.
    assert_java_has_run(&strings(&target_names), &java_names);
    assert_java_answers(&java, "ACTION", "VALIDATE_LOCATION");
    let target_b = h.collect(Who::B, "TARGET_SELECTED", 1).await;
    let seen: TargetSelected = one(&target_b, "TARGET_SELECTED");
    assert_eq!(
        (seen.object_id, seen.target_id),
        (a, npc_id),
        "B sees A pick the gatekeeper"
    );

    h.click(Who::A, npc_id).await;
    let talk = h.collect(Who::A, "ACTION_FAIL", 2).await;
    assert_eq!(
        names(&talk),
        ["MOVE_TO_PAWN", "NPC_HTML_MESSAGE", "ACTION_FAIL", "ACTION_FAIL"],
        "the second click turns to the NPC and opens its page"
    );
    let page: NpcHtmlMessage = one(&talk, "NPC_HTML_MESSAGE");
    assert_eq!(page.npc_obj_id, npc_id);
    assert!(
        page.html.contains("Gatekeeper Roxxy"),
        "%npcname% is replaced: {}",
        page.html
    );
    let show_teleports = format!("npc_{npc_id}_showTeleports");
    assert!(
        page.html.contains(&show_teleports),
        "%objectId% is replaced: {}",
        page.html
    );
    assert!(
        h.collect_quiet(Who::B).await.is_empty(),
        "B hears nothing of the dialog"
    );

    h.bypass(Who::A, &show_teleports).await;
    let list = h.collect(Who::A, "ACTION_FAIL", 1).await;
    assert_eq!(names(&list), ["NPC_HTML_MESSAGE", "ACTION_FAIL"]);
    let page: NpcHtmlMessage = one(&list, "NPC_HTML_MESSAGE");
    let teleport_first = format!("npc_{npc_id}_teleport NORMAL 0");
    assert!(
        page.html.contains(&teleport_first),
        "the destination links: {}",
        page.html
    );
    assert!(
        page.html
            .contains(&format!("Test Village - {} Adena", m3_data::TELEPORT_FEE)),
        "the price is in the list: {}",
        page.html
    );

    // ---- the teleport: the fee, TeleportToLocation, then Appearing at the new place ---------
    h.bypass(Who::A, &teleport_first).await;
    let leaving = h.collect(Who::A, "INVENTORY_UPDATE", 1).await;
    assert_in_order(
        &names(&leaving),
        &[
            "SYSTEM_MESSAGE",
            "TARGET_UNSELECTED",
            "DELETE_OBJECT",
            "TELEPORT_TO_LOCATION",
            "ACTION_FAIL",
            "INVENTORY_UPDATE",
        ],
    );
    let fee: SystemMessage = all(&leaving, "SYSTEM_MESSAGE").remove(0);
    assert_eq!(fee.id, SystemMessageId::S1AdenaDisappeared);
    assert_eq!(
        fee.params,
        [SystemMessageParam::LongNumber(m3_data::TELEPORT_FEE)]
    );
    let deleted: Vec<i32> = all::<DeleteObject>(&leaving, "DELETE_OBJECT")
        .iter()
        .map(|d| d.object_id)
        .collect();
    assert!(
        deleted.contains(&npc_id) && deleted.contains(&b),
        "A forgets the NPC and B: {deleted:?}"
    );
    let teleport: TeleportToLocation = one(&leaving, "TELEPORT_TO_LOCATION");
    assert_eq!(teleport.object_id, a);
    let destination = m3_data::DESTINATION;
    assert_eq!((teleport.x, teleport.y), (destination.0, destination.1));
    assert_eq!(
        teleport.z,
        m3_data::GROUND_Z + 5,
        "the ground under the destination, plus 5"
    );
    let teleported_to = (teleport.x, teleport.y, teleport.z);
    // The adena shows as a changed stack.
    let update: InventoryUpdate = one(&leaving, "INVENTORY_UPDATE");
    assert_eq!(update.updates.len(), 1);
    assert_eq!(
        (
            update.updates[0].change,
            update.updates[0].item.item_id,
            update.updates[0].item.count
        ),
        (
            InventoryUpdate::MODIFIED,
            ADENA_ID,
            START_ADENA - m3_data::TELEPORT_FEE
        )
    );

    // B watched A vanish: the target is dropped, A is deleted, and the teleport is announced.
    let gone = h.collect(Who::B, "TELEPORT_TO_LOCATION", 1).await;
    assert_in_order(
        &names(&gone),
        &["TARGET_UNSELECTED", "DELETE_OBJECT", "TELEPORT_TO_LOCATION"],
    );
    assert_eq!(one::<DeleteObject>(&gone, "DELETE_OBJECT").object_id, a);

    // The client has loaded the place and says so: the player is in the world there.
    h.appearing(Who::A).await;
    let arrived = h.collect(Who::A, "USER_INFO", 1).await;
    let info: UserInfo = all(&arrived, "USER_INFO").remove(0);
    assert_eq!(
        (info.x, info.y, info.z),
        teleported_to,
        "UserInfo shows the new place"
    );
    assert!(
        !names(&arrived).contains(&"NPC_INFO".to_owned()),
        "nobody is near the destination"
    );
    assert!(
        h.collect_quiet(Who::B).await.is_empty(),
        "B, 10000 units away, hears no more of A"
    );

    // ---- out of reach: chat does not cross the distance --------------------------------------
    h.say(Who::A, "is anybody there?").await;
    let line = echo(a, NAME_A, "is anybody there?");
    assert_eq!(h.collect(Who::A, "SAY2", 1).await, [line]);
    assert!(
        h.collect_quiet(Who::B).await.is_empty(),
        "B is beyond the 1250 of general chat"
    );
    h.say(Who::B, "hello?").await;
    assert_eq!(h.collect(Who::B, "SAY2", 1).await, [echo(b, NAME_B, "hello?")]);
    assert!(h.collect_quiet(Who::A).await.is_empty());

    // ---- both log out; what is stored is final ------------------------------------------------
    let left_a = h.leave(Who::A).await;
    let left_b = h.leave(Who::B).await;
    Played {
        left_a,
        left_b,
        teleported_to,
        npc_id,
    }
}

fn assert_played(played: &Played) {
    assert_eq!(
        played.left_a,
        Left {
            position: played.teleported_to,
            adena: START_ADENA - m3_data::TELEPORT_FEE,
            forgot: vec![],
        },
        "A is stored where the gatekeeper sent it, with the fee paid; nothing is near it to forget"
    );
    assert_eq!(
        played.left_b,
        Left {
            position: m3_data::START,
            adena: START_ADENA,
            forgot: vec![played.npc_id],
        },
        "B never moved and paid nothing; it forgets the gatekeeper on its way out"
    );
}

// ------------------------------------------------------------------------------------------
// The bare world: the same datapack and configuration, driven without sockets or database
// ------------------------------------------------------------------------------------------

/// Packets sent to the players: `(object id, bytes)`.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

/// `(object id of the receiver, bytes)`.
type Sent = Vec<(i32, Vec<u8>)>;

impl Log {
    fn peek(&self, id: i32) -> Vec<Vec<u8>> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(to, _)| *to == id)
            .map(|(_, bytes)| bytes.clone())
            .collect()
    }

    fn take(&self, id: i32) -> Vec<Vec<u8>> {
        let mut all = self.0.lock().unwrap();
        let (mine, others): (Vec<_>, Vec<_>) = all.drain(..).partition(|(to, _)| *to == id);
        *all = others;
        mine.into_iter().map(|(_, bytes)| bytes).collect()
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

/// What the persistence task would get.
#[derive(Clone, Default)]
struct Stored(Arc<Mutex<Vec<PlayerData>>>);

struct RecordingOutbox(Stored);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        if let PersistCommand::StorePlayer(data) = command {
            self.0.0.lock().unwrap().push(*data);
        }
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

struct BareWorld {
    world: World,
    log: Log,
    stored: Stored,
    players: [PlayerData; 2],
    /// Keeps the folder of the datapack (nothing reads it after the start).
    _root: tempfile::TempDir,
}

const BARE_IDS: [i32; 2] = [0x6000_0001, 0x6000_0002];

fn bare_player(index: usize) -> PlayerData {
    let (name, account) = [(NAME_A, ACCOUNT_A), (NAME_B, ACCOUNT_B)][index];
    PlayerData {
        object_id: ObjectId(BARE_IDS[index]),
        account: account.to_owned(),
        name: name.to_owned(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 1,
        hair_style: 1,
        hair_color: 1,
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: m3_data::START.0,
        y: m3_data::START.1,
        z: m3_data::START.2,
        heading: 0,
        cur_hp: 80.0,
        max_hp: 80.0,
        cur_mp: 30.0,
        max_mp: 30.0,
        cur_cp: 32.0,
        max_cp: 32.0,
        online_time: 0,
        items: vec![ItemData {
            object_id: ObjectId(1_000 + i32::try_from(index).unwrap()),
            item_id: ADENA_ID,
            count: START_ADENA,
            enchant_level: 0,
            loc: "INVENTORY".to_owned(),
            loc_data: 0,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: -1,
        }],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

impl BareWorld {
    /// Loads the configuration, datapack, geodata, zones, html, teleporter lists and spawns the
    /// way `Server::start` does, without the database and the sockets.
    fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        write_game_config(&config_dir, 4242);
        let data_root = root.path().join("datapack");
        write_datapack(&data_root);

        let config = GameConfig::load(&config_dir).unwrap();
        let datapack = Arc::new(Datapack::load(&data_root).unwrap());
        let static_data: Arc<dyn StaticData> =
            Arc::new(DatapackData::load(Arc::clone(&datapack), &config, &config_dir));
        let settings = world_settings(&config, &config_dir);
        let geo_settings = geo::GeoSettings::load(&config, &config_dir, &data_root);
        let engine = geo::load_engine(&geo_settings);
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());

        let stored = Stored::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(RecordingOutbox(stored.clone())));
        world.set_static_data(Box::new(static_data));
        world.set_settings(settings);
        world.set_geo(engine);
        world.set_zone_data(Arc::clone(&datapack.zones), Arc::clone(&datapack.map_regions));
        world.set_html_cache(Arc::clone(&datapack.html));
        world.set_teleporter_data(Arc::clone(&datapack.teleporters));
        world.spawn_door_data(&datapack, &ids).unwrap();
        npcs::spawn_npcs(&mut world, &datapack, ids, &config, &config_dir);
        Self {
            world,
            log: Log::default(),
            stored,
            players: [bare_player(0), bare_player(1)],
            _root: root,
        }
    }

    fn input(&mut self, who: Who, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.index() + 1).unwrap()),
            input,
        });
    }

    fn ticks(&mut self, count: usize) {
        for _ in 0..count {
            self.world.tick();
        }
    }

    fn count(&self, who: Who, name: &str) -> usize {
        self.log
            .peek(BARE_IDS[who.index()])
            .iter()
            .filter(|body| packet_name(body) == name)
            .count()
    }
}

impl Harness for BareWorld {
    fn object_id(&self, who: Who) -> i32 {
        BARE_IDS[who.index()]
    }

    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>> {
        let index = who.index();
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(index + 1).unwrap()),
            player: Box::new(self.players[index].clone()),
            sink: Box::new(Sink(BARE_IDS[index], self.log.clone())),
            ip: None,
        });
        let packets = self.log.take(BARE_IDS[index]);
        assert_eq!(names(&packets).last().map(String::as_str), Some("SKILL_LIST"));
        packets
    }

    async fn move_to(&mut self, who: Who, target: (i32, i32, i32), origin: (i32, i32, i32)) {
        self.input(
            who,
            ClientInput::MoveTo {
                target_x: target.0,
                target_y: target.1,
                target_z: target.2,
                origin_x: origin.0,
                origin_y: origin.1,
                origin_z: origin.2,
                movement_mode: 1,
            },
        );
    }

    async fn validate_position(&mut self, who: Who, at: (i32, i32, i32), heading: i32) {
        self.input(
            who,
            ClientInput::ValidatePosition {
                x: at.0,
                y: at.1,
                z: at.2,
                heading,
            },
        );
    }

    async fn click(&mut self, who: Who, object_id: i32) {
        self.input(
            who,
            ClientInput::Action {
                object_id,
                action_id: 0,
            },
        );
    }

    async fn bypass(&mut self, who: Who, command: &str) {
        self.input(
            who,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
    }

    async fn appearing(&mut self, who: Who) {
        self.input(who, ClientInput::Appearing);
    }

    async fn say(&mut self, who: Who, text: &str) {
        self.input(
            who,
            ClientInput::Say {
                chat_type: 0,
                text: text.to_owned(),
                target: None,
            },
        );
    }

    async fn collect(&mut self, who: Who, name: &str, times: usize) -> Vec<Vec<u8>> {
        // At most 40 seconds of world time (800 ticks of 50 ms).
        let mut waited = 0;
        while self.count(who, name) < times {
            assert!(
                waited < 800,
                "no {name} x{times} for {who:?} in 40 s of world time; got {:?}",
                names(&self.log.peek(BARE_IDS[who.index()]))
            );
            self.world.tick();
            waited += 1;
        }
        // Let the rest of the event arrive, like the quiet period of a socket does.
        self.ticks(4);
        self.log.take(BARE_IDS[who.index()])
    }

    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>> {
        self.ticks(10);
        self.log.take(BARE_IDS[who.index()])
    }

    async fn leave(&mut self, who: Who) -> Left {
        let id = BARE_IDS[who.index()];
        self.input(who, ClientInput::Logout);
        self.ticks(2);
        let forgot = assert_goodbye(&self.log.take(id));
        let stored = self.stored.0.lock().unwrap();
        let data = stored
            .iter()
            .rev()
            .find(|data| data.object_id == ObjectId(id))
            .expect("the character was stored");
        Left {
            position: (data.x, data.y, data.z),
            adena: data
                .items
                .iter()
                .find(|item| item.item_id == ADENA_ID)
                .map_or(0, |item| item.count),
            forgot,
        }
    }
}

// ------------------------------------------------------------------------------------------
// The real servers: login server, game server, database, sockets
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
        write_datapack(&gs_root.path().join("datapack"));
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

    /// The whole way in: log in at the login server, ask for the game server, connect to it,
    /// handshake, `AuthLogin`, and read the character list. `chars` is the number of characters
    /// the game server must have reported to the login server (0: not checked).
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
        game.auth(account, &ok, &play).await;
        let list = game.char_selection_info().await;
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

    /// The adena the character has stored (0 without an adena row).
    async fn adena(&self, char_id: i32) -> i64 {
        let count: Option<u64> =
            sqlx::query_scalar("SELECT `count` FROM items WHERE owner_id = ? AND item_id = ?")
                .bind(char_id)
                .bind(ADENA_ID)
                .fetch_optional(&self.pool)
                .await
                .unwrap();
        i64::try_from(count.unwrap_or(0)).unwrap()
    }

    /// Creates a human fighter on a fresh connection and leaves through the character screen.
    /// Returns its object id.
    async fn create_character_and_leave(&self, account: &str, name: &str) -> i32 {
        let (mut game, list, _) = self.login(account, 0).await;
        assert!(list.characters.is_empty());
        game.create_character(&character_create(name, 0)).await.unwrap();
        let list = game.goto_lobby().await;
        let created = &list.characters[0];
        assert_eq!(
            (created.x, created.y, created.z),
            m3_data::START,
            "the creation point of the datapack"
        );
        let char_id = created.object_id;
        // Java: Logout at the character screen disconnects.
        assert!(game.logout().await.is_empty());
        self.wait_until_out_of_game(account).await;
        char_id
    }
}

/// The scenario against the real servers.
struct Net {
    rig: Rig,
    ids: [i32; 2],
    clients: [Option<FakeGameClient>; 2],
}

impl Net {
    const ACCOUNTS: [&'static str; 2] = [ACCOUNT_A, ACCOUNT_B];

    async fn start() -> Option<Self> {
        let rig = Rig::start("e2em3").await?;
        let a = rig.create_character_and_leave(ACCOUNT_A, NAME_A).await;
        let b = rig.create_character_and_leave(ACCOUNT_B, NAME_B).await;
        assert_eq!(
            rig.adena(a).await,
            START_ADENA,
            "the character was created with StartingAdena"
        );
        Some(Self {
            rig,
            ids: [a, b],
            clients: [None, None],
        })
    }

    fn client(&mut self, who: Who) -> &mut FakeGameClient {
        self.clients[who.index()]
            .as_mut()
            .expect("the client is in the world")
    }
}

impl Harness for Net {
    fn object_id(&self, who: Who) -> i32 {
        self.ids[who.index()]
    }

    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>> {
        let (mut game, list, play) = self.rig.login(Self::ACCOUNTS[who.index()], 1).await;
        assert_eq!(list.characters[0].object_id, self.ids[who.index()]);
        let selected = game.select_character(0).await;
        assert_eq!(selected.session_id, play.play_ok1);
        game.send_packet(&enter_world_packet()).await;
        let packets = game.read_until("SKILL_LIST").await;
        self.clients[who.index()] = Some(game);
        packets
    }

    async fn move_to(&mut self, who: Who, target: (i32, i32, i32), origin: (i32, i32, i32)) {
        self.client(who).move_to(target, origin).await;
    }

    async fn validate_position(&mut self, who: Who, at: (i32, i32, i32), heading: i32) {
        self.client(who)
            .validate_position(at.0, at.1, at.2, heading)
            .await;
    }

    async fn click(&mut self, who: Who, object_id: i32) {
        self.client(who).click(object_id, m3_data::START).await;
    }

    async fn bypass(&mut self, who: Who, command: &str) {
        self.client(who).bypass(command).await;
    }

    async fn appearing(&mut self, who: Who) {
        self.client(who).appearing().await;
    }

    async fn say(&mut self, who: Who, text: &str) {
        self.client(who).say(0, text, None).await;
    }

    async fn collect(&mut self, who: Who, name: &str, times: usize) -> Vec<Vec<u8>> {
        let client = self.client(who);
        let mut batch = Vec::new();
        for _ in 0..times {
            batch.extend(client.read_until(name).await);
        }
        batch.extend(client.read_quiet(Duration::from_millis(250)).await);
        batch
    }

    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>> {
        self.client(who).read_quiet(Duration::from_millis(500)).await
    }

    async fn leave(&mut self, who: Who) -> Left {
        let mut game = self.clients[who.index()]
            .take()
            .expect("the client is in the world");
        let forgot = assert_goodbye(&game.logout().await);
        // The login server hears about the logout only after the character is stored: when it
        // no longer counts the account, the rows are already final.
        self.rig.wait_until_out_of_game(Self::ACCOUNTS[who.index()]).await;
        let id = self.ids[who.index()];
        let row = self.rig.row(id).await;
        assert_eq!(row.online, Some(0), "offline again");
        Left {
            position: (row.x.unwrap(), row.y.unwrap(), row.z.unwrap()),
            adena: self.rig.adena(id).await,
            forgot,
        }
    }
}

// ------------------------------------------------------------------------------------------
// The tests with a database
// ------------------------------------------------------------------------------------------

#[tokio::test]
async fn two_players_see_each_other_walk_talk_to_a_gatekeeper_teleport_chat_and_log_out() {
    let Some(mut net) = Net::start().await else {
        return;
    };
    let items_a = net.rig.item_count(net.ids[0]).await;
    let played = play(&mut net).await;
    assert_played(&played);

    // What the game server wrote: A is where the gatekeeper sent it, the fee is paid, and no
    // item was lost or duplicated (the adena stack was only reduced).
    let (a, b) = (net.ids[0], net.ids[1]);
    let row = net.rig.row(a).await;
    assert_eq!(
        (row.x, row.y, row.z),
        (
            Some(played.teleported_to.0),
            Some(played.teleported_to.1),
            Some(played.teleported_to.2)
        )
    );
    assert_eq!(net.rig.item_count(a).await, items_a);
    assert_eq!(net.rig.adena(b).await, START_ADENA);

    // Log in again: the character list shows A at the destination, B at the start.
    let (mut again, list, _) = net.rig.login(ACCOUNT_A, 1).await;
    let entry = &list.characters[0];
    assert_eq!((entry.x, entry.y, entry.z), played.teleported_to);
    assert!(again.logout().await.is_empty());
    net.rig.wait_until_out_of_game(ACCOUNT_A).await;
    let (mut again, list, _) = net.rig.login(ACCOUNT_B, 1).await;
    let entry = &list.characters[0];
    assert_eq!((entry.x, entry.y, entry.z), m3_data::START);
    assert!(again.logout().await.is_empty());
    net.rig.wait_until_out_of_game(ACCOUNT_B).await;

    net.rig.gs.shutdown(false).await;
    net.rig.ls.shutdown(false).await;
    net.rig.db.drop_db().await;
}

// ------------------------------------------------------------------------------------------
// The tests without a database (they always run)
// ------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_scenario_in_a_bare_world_with_the_same_data() {
    let mut world = BareWorld::start();
    let played = play(&mut world).await;
    assert_played(&played);
}

#[test]
fn the_world_data_of_the_test_loads_as_the_game_server_reads_it() {
    let root = tempfile::tempdir().unwrap();
    write_datapack(root.path());
    let pack = Datapack::load(root.path()).unwrap();

    let npc = pack
        .npcs
        .get(m3_data::GATEKEEPER_ID)
        .expect("the gatekeeper template");
    assert_eq!(
        (npc.npc_type.as_str(), npc.name.as_str()),
        ("Teleporter", m3_data::GATEKEEPER_NAME)
    );
    let spawns: Vec<_> = pack.spawns.spawns_of(m3_data::GATEKEEPER_ID).collect();
    assert_eq!(spawns.len(), 1, "one spawn");

    let list = pack
        .teleporters
        .get_holder(m3_data::GATEKEEPER_ID, "NORMAL")
        .expect("the NORMAL list");
    assert_eq!(list.locations().len(), 2);
    let first = list.location(0).unwrap();
    assert_eq!((first.x, first.y, first.z), m3_data::DESTINATION);
    assert_eq!(first.fee_count, m3_data::TELEPORT_FEE);

    for page in ["teleporter/30006.htm", "teleporter/teleports.htm"] {
        assert!(
            pack.html.contains(&format!("data/html/{page}")),
            "{page} is cached"
        );
    }

    let (x, y, z) = m3_data::START;
    assert!(pack.zones.is_inside_zone_type(x, y, z, "PeaceZone"));
    assert!(pack.zones.is_inside_zone_type(x, y, z, "TownZone"));
    assert!(
        !pack
            .zones
            .is_inside_zone_type(m3_data::DESTINATION.0, m3_data::DESTINATION.1, z, "PeaceZone"),
        "the destination is outside the zones"
    );
    let region = pack
        .map_regions
        .region_at(x, y)
        .expect("the map region of the start");
    assert_eq!(region.name, "talking_island_town");
}

#[test]
fn the_synthetic_geodata_has_one_wall_and_nothing_else() {
    use l2_geo::coords::{geo_x, geo_y};
    use l2_geo::{GeoEngine, GeoEngineOptions, GeoLoadSpec, Location};

    let dir = tempfile::tempdir().unwrap();
    m3_data::write_geodata(dir.path());
    let (engine, report) = GeoEngine::load(&GeoLoadSpec::new(dir.path()), GeoEngineOptions::shipped());
    assert_eq!(report.loaded, 1, "one tile: {report:?}");
    assert!(report.failed.is_empty());

    let (x, y, z) = m3_data::START;
    assert!(engine.has_geo(x, y));
    assert_eq!(engine.height(x, y, z), m3_data::GROUND_Z);
    let (wall_x, row) = m3_data::wall_cell();
    assert!(engine.is_completely_blocked(wall_x, row, z));
    // The wall is the whole column of the block: 8 cells long, one cell thick.
    for blocked in [(wall_x, row - 1), (wall_x, row + 1)] {
        assert!(
            engine.is_completely_blocked(blocked.0, blocked.1, z),
            "{blocked:?} is wall"
        );
    }
    for free in [
        (wall_x - 1, row),
        (wall_x + 1, row),
        (wall_x, row - 6),
        (wall_x, row + 3),
    ] {
        assert!(
            !engine.is_completely_blocked(free.0, free.1, z),
            "{free:?} is open"
        );
    }
    assert_eq!(geo_x(m3_data::inside_the_wall().0), wall_x);
    assert_eq!(geo_y(m3_data::inside_the_wall().1), row);

    // A walk from the start to behind the wall ends in front of it; to a spot on the other side
    // (not behind the wall) the way is free.
    let from = Location::new(x, y, z);
    let behind = m3_data::behind_the_wall();
    let stop = engine.valid_location(from, Location::new(behind.0, behind.1, behind.2), 0);
    assert_eq!((stop.x, stop.y, stop.z), m3_data::last_free_spot());
    let free_spot = Location::new(x - 60, y + 40, z);
    let reached = engine.valid_location(from, free_spot, 0);
    assert_eq!((reached.x, reached.y), (free_spot.x, free_spot.y));
    // The path of the walk is short enough that Java does not look for another way
    // (`original distance - corrected distance <= 30`).
    let corrected = f64::from(stop.x - x).hypot(f64::from(stop.y - y));
    let wanted = f64::from(behind.0 - x).hypot(f64::from(behind.1 - y));
    assert!(wanted - corrected <= 30.0, "{wanted} - {corrected}");
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
    assert_eq!(config.server.game_server_login_port, 4242);
    assert_eq!(config.player.starting_adena, START_ADENA);
    assert_eq!(config.player.max_free_teleport_level, 0);
    assert_eq!(config.general.max_npc_animation, 0);
    assert_eq!(config.general.minimum_chat_level, 0);
    assert_eq!(config.geo_engine.pathfinding, 2);
}

#[test]
fn what_java_recorded_has_the_packets_the_scenario_compares_with() {
    let java = java_conversation(JAVA_SESSION, &JAVA_NEEDLES);
    assert_java_answers(&java, "MOVE_TO_LOCATION", "MOVE_TO_LOCATION");
    assert_java_answers(&java, "ACTION", "VALIDATE_LOCATION");
    assert_java_has_run(
        &strings(&[
            "VALIDATE_LOCATION",
            "MY_TARGET_SELECTED",
            "STATUS_UPDATE",
            "ACTION_FAIL",
        ]),
        &server_names(&java),
    );
}
