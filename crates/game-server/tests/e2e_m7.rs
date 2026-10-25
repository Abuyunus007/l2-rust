#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End to end for the residences of M7: players log in through the in-process login server and
//! game server and play a GM-started castle siege (registration, headquarters, guards, a gate
//! broken, the artefact engraved, the owner changed), a fortress siege, a clan hall auction with a
//! hall function, and a purchase that pays the castle tax.
//!
//! Every scenario is written once (`play`) against a `Harness` and runs like `e2e_m6.rs`:
//!
//! * `Net`: the real thing. The Rust login server and the Rust game server in one process, a
//!   throwaway MariaDB, `FakeGameClient`s on sockets. It runs only when `L2_TEST_DATABASE_URL`
//!   is set (CI's MariaDB job; never point it at a database with real data). The castle, fortress,
//!   hall and auction rows are inserted before the game server starts; the clans (their ids come
//!   from the characters) after, so the world reads them at `CharacterSelect` (D-50, D-51). After a
//!   scenario the rows are read back and compared with what Java's statements write (D-39).
//! * `BareWorld`: the same datapack, configuration, geodata, zones and spawns loaded the way the
//!   game server loads them, but with the world driven directly (no sockets, no database, ticks
//!   instead of time). The persistence commands the world sends are applied to a model of the
//!   tables (`m7_db.rs`) that also answers the reads (D-50), so the same row checks run here.
//!
//! Each scenario runs twice per half: with `SiegeDatabaseWrites = False` (D-51: the siege-like
//! results change this world only, no global row changes) and with `True` (the rows Java writes).
//!
//! A client action is an `Act`: the `ClientInput` for the bare world and the bytes of the packet
//! the real client sends for `Net`. `every_act_decodes_on_the_wire` checks that the bytes decode
//! as the packet each action stands for.
//!
//! There is no datapack and no geodata in CI: `e2e/datapack.rs`, `e2e/m3_data.rs` to
//! `e2e/m6_data.rs` and `e2e/m7_data.rs` write them into a temporary folder.

#[path = "e2e/datapack.rs"]
mod datapack;
#[allow(dead_code)]
#[path = "e2e/m3_data.rs"]
mod m3_data;
#[allow(dead_code)]
#[path = "e2e/m4_data.rs"]
mod m4_data;
#[allow(dead_code)]
#[path = "e2e/m5_data.rs"]
mod m5_data;
#[allow(dead_code)]
#[path = "e2e/m6_data.rs"]
mod m6_data;
#[allow(dead_code)]
#[path = "e2e/m7_data.rs"]
mod m7_data;
#[allow(dead_code)]
#[path = "e2e/m7_db.rs"]
mod m7_db;
#[allow(dead_code)]
#[path = "e2e/m7_seed.rs"]
mod m7_seed;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_server::world_data::{
    DatapackData, multisell_settings, regen_settings, siege_settings, skill_load_options, summon_settings,
    world_settings,
};
use game_server::{
    Paths, Server, clan_hall_settings, clan_settings, fort_settings, geo, hexid, mail_settings, npcs,
    residence_npc_settings, residence_settings,
};
use l2_config::GameConfig;
use l2_data::{BuyListOptions, BuyLists, Datapack, DatapackOptions, MerchantPriceConfigs};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::{LeaveWorld, SystemMessage, SystemMessageParam};
use l2_protocol_game::{SystemMessageId, client};
use l2_protocol_login::server::PlayOk;
use l2_testkit::fake_game_client::{
    FakeGameClient, Recv, character_create, enter_world_packet, names_of, packet_name,
};
use l2_testkit::fake_login_client::FakeClient as LoginClient;
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
use l2_world::residence::{ResidenceReadKind, ResidenceRereadRows};
use l2_world::{
    ClientInput, IdFactory, ItemData, LeaveReason, LsEvent, ObjectId, Outbox, PacketSink, PersistCommand,
    PlayerClanState, PlayerData, PlayerProfile, SessionId, SkillData, StaticData, World, WorldCommand,
    WorldRng,
};
use login_server::state::{LoginState, ServerStatus as ListStatus};

use l2_db::Pool;

use m7_db::{Db, Rows};
use m7_seed::Seed;

const ACCOUNT_LORD: &str = "e2em7accl";
const ACCOUNT_RAIDER: &str = "e2em7accr";
const ACCOUNT_GM: &str = "e2em7accg";
const PASSWORD: &str = "secret";
const NAME_LORD: &str = "Lord";
const NAME_RAIDER: &str = "Raider";
const NAME_GM: &str = "Warden";
/// A protocol revision the game server accepts by default.
const PROTOCOL: i32 = 273;
const ADENA_ID: i32 = 57;
const SOULSHOT_ID: i32 = 1835;

/// Every table of the game schema (the files of `crates/l2-db/sql/game`), so the game server
/// finds whatever it reads or writes.
const GAME_SCHEMA: &[&str] = l2_db::GAME_SCHEMA;

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

/// The `Shanty` lines of `FortSiege.ini`: where the commanders and the combat flags stand.
fn fort_siege_ini() -> String {
    let commander = |i: i32, id: i32, (x, y, z): (i32, i32, i32)| format!("ShantyCommander{i} = {x},{y},{z},0,{id}\n");
    let flag = |i: i32, (x, y, z): (i32, i32, i32)| {
        format!("ShantyFlag{i} = {x},{y},{z},{}\n", m7_data::COMBAT_FLAG_ITEM)
    };
    format!(
        "SiegeClanMinLevel = 4\nSiegeLength = 60\nCountDownLength = 10\n{}{}{}{}",
        commander(1, m7_data::FORT_COMMANDER_1, m7_data::FORT_COMMANDER_1_POS),
        commander(2, m7_data::FORT_COMMANDER_2, m7_data::FORT_COMMANDER_2_POS),
        flag(1, m7_data::FORT_FLAG_POS),
        flag(2, (m7_data::FORT_FLAG_POS.0, m7_data::FORT_FLAG_POS.1 - 200, m7_data::FORT_FLAG_POS.2)),
    )
}

/// Everything in the game server's `config` folder except `Database.ini`. Like the M6 rig (see
/// `e2e_m4.rs` for the reasons), with a lot of starting adena, the admin rights, `Siege.ini`,
/// `FortSiege.ini` and `SideBySide.ini` with the switch of the scenario (D-51).
fn write_game_config(config_dir: &Path, ls_port: u16, siege_writes: bool) {
    std::fs::create_dir_all(config_dir).unwrap();
    std::fs::write(config_dir.join("Server.ini"), game_server_ini(ls_port)).unwrap();
    std::fs::write(
        config_dir.join("Player.ini"),
        format!(
            "ForbiddenNames = annou\nStartingAdena = {}\nDisconnectAfterDeath = False\nEnableVitality = False\n",
            m6_data::START_ADENA
        ),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("NPC.ini"),
        format!("DefaultCorpseTime = {}\n", m4_data::CORPSE_SECONDS),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("General.ini"),
        "MaxNpcAnimation = 0\nMinimumChatLevel = 0\nAllowManor = False\n",
    )
    .unwrap();
    std::fs::write(
        config_dir.join("GeoEngine.ini"),
        "PathFinding = 2\nGeoDataPath = ./geodata/\nAvoidObstructedPathNodes = True\n",
    )
    .unwrap();
    std::fs::write(
        config_dir.join("FloodProtector.ini"),
        datapack::no_flood_protection_ini(),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("ipconfig.xml"),
        "<gameserver address=\"127.0.0.1\" />\n",
    )
    .unwrap();
    std::fs::write(
        config_dir.join("SideBySide.ini"),
        format!("SiegeDatabaseWrites = {}\n", if siege_writes { "True" } else { "False" }),
    )
    .unwrap();
    std::fs::write(config_dir.join("Siege.ini"), "SiegeClanMinLevel = 5\nSiegeLength = 120\n").unwrap();
    std::fs::write(config_dir.join("FortSiege.ini"), fort_siege_ini()).unwrap();
    m7_data::write_admin_config(config_dir);
    hexid::save(config_dir, SAMPLE_SERVER_ID, SAMPLE_HEXID);
}

/// The datapack of this test: the base classes and items, the world data of M3, the hunting data
/// of M4, the skills and items of M5, the social data of M6 and the residences of M7.
fn write_datapack(root: &Path) {
    datapack::write_datapack(root);
    m3_data::write_world_data(root);
    m4_data::write_hunting_data(root);
    m5_data::write_skill_data(root);
    m6_data::write_social_data(root);
    m7_data::write_residence_data(root);
}

// ------------------------------------------------------------------------------------------
// Looking at packets
// ------------------------------------------------------------------------------------------

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

fn names(batch: &[Vec<u8>]) -> Vec<String> {
    names_of(batch)
}

fn is_named(body: &[u8], name: &str) -> bool {
    packet_name(body) == name
}

fn count_named(batch: &[Vec<u8>], name: &str) -> usize {
    batch.iter().filter(|body| is_named(body, name)).count()
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

fn system_messages(batch: &[Vec<u8>]) -> Vec<SystemMessage> {
    all::<SystemMessage>(batch, "SYSTEM_MESSAGE")
}

fn message_ids(batch: &[Vec<u8>]) -> Vec<SystemMessageId> {
    system_messages(batch).into_iter().map(|m| m.id).collect()
}

/// The first message of `id` in `batch`.
fn message_of(batch: &[Vec<u8>], id: SystemMessageId) -> SystemMessage {
    system_messages(batch)
        .into_iter()
        .find(|m| m.id == id)
        .unwrap_or_else(|| panic!("no {id:?} in {:?} ({:?})", message_ids(batch), names(batch)))
}

fn has_message(batch: &[Vec<u8>], id: SystemMessageId) -> bool {
    message_ids(batch).contains(&id)
}

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

/// Wall-clock milliseconds, like the world's `unix_ms_now`.
fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

// ------------------------------------------------------------------------------------------
// Client actions
// ------------------------------------------------------------------------------------------

/// What a player does: the input the bare world takes and the packet a real client sends.
struct Act {
    input: ClientInput,
    wire: Vec<u8>,
}

impl Act {
    fn new<P: Encode>(input: ClientInput, packet: &P) -> Self {
        Self {
            input,
            wire: packet.to_bytes(),
        }
    }
}

/// One constructor per action the scenarios use.
mod act {
    use super::*;
    use l2_world::{FortInput, ShopInput, SiegeInput};

    pub fn click(object_id: i32, origin: (i32, i32, i32)) -> Act {
        Act::new(
            ClientInput::Action {
                object_id,
                action_id: 0,
            },
            &client::Action {
                object_id,
                origin_x: origin.0,
                origin_y: origin.1,
                origin_z: origin.2,
                action_id: client::Action::SIMPLE_CLICK,
            },
        )
    }

    /// A click on the ground: walk to `target` from `origin`.
    pub fn move_to(target: (i32, i32, i32), origin: (i32, i32, i32)) -> Act {
        Act::new(
            ClientInput::MoveTo {
                target_x: target.0,
                target_y: target.1,
                target_z: target.2,
                origin_x: origin.0,
                origin_y: origin.1,
                origin_z: origin.2,
                movement_mode: 1,
            },
            &client::MoveToLocation {
                target_x: target.0,
                target_y: target.1,
                target_z: target.2,
                origin_x: origin.0,
                origin_y: origin.1,
                origin_z: origin.2,
                movement_mode: 1,
            },
        )
    }

    pub fn use_skill(magic_id: i32) -> Act {
        Act::new(
            ClientInput::MagicSkillUse {
                magic_id,
                ctrl_pressed: false,
                shift_pressed: false,
            },
            &client::RequestMagicSkillUse {
                magic_id,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        )
    }

    pub fn use_item(object_id: i32) -> Act {
        Act::new(
            ClientInput::UseItem {
                object_id,
                ctrl_pressed: false,
            },
            &client::UseItem {
                object_id,
                ctrl_pressed: false,
            },
        )
    }

    pub fn bypass(command: &str) -> Act {
        Act::new(
            ClientInput::Bypass {
                command: command.to_owned(),
            },
            &client::RequestBypassToServer {
                command: command.to_owned(),
            },
        )
    }

    /// `//<command>`: the GM's chat command. The client sends `SendBypassBuildCmd` with the text
    /// after the two slashes; the server puts `admin_` in front (Java `SendBypassBuildCmd`).
    pub fn admin(command: &str) -> Act {
        Act::new(
            ClientInput::BuildCmd {
                command: command.to_owned(),
            },
            &client::SendBypassBuildCmd {
                command: command.to_owned(),
            },
        )
    }

    /// The client's answer to a teleport: `Appearing`.
    pub fn appearing() -> Act {
        Act::new(ClientInput::Appearing, &client::Appearing)
    }

    pub fn join_siege(castle_id: i32, is_attacker: i32, is_joining: i32) -> Act {
        Act::new(
            ClientInput::Siege(SiegeInput::JoinSiege {
                castle_id,
                is_attacker,
                is_joining,
            }),
            &client::RequestJoinSiege {
                castle_id,
                is_attacker,
                is_joining,
            },
        )
    }

    pub fn all_castle_info() -> Act {
        Act::new(
            ClientInput::Siege(SiegeInput::AllCastleInfo),
            &client::RequestAllCastleInfo,
        )
    }

    pub fn siege_attacker_list(castle_id: i32) -> Act {
        Act::new(
            ClientInput::Siege(SiegeInput::AttackerList { castle_id }),
            &client::RequestSiegeAttackerList { castle_id },
        )
    }

    pub fn all_fortress_info() -> Act {
        Act::new(
            ClientInput::Fort(FortInput::AllFortressInfo),
            &client::RequestAllFortressInfo,
        )
    }

    /// `(item id, count)` per product.
    pub fn buy_items(list_id: i32, items: &[(i32, i64)]) -> Act {
        let packet = client::RequestBuyItem {
            list_id,
            items: Some(
                items
                    .iter()
                    .map(|&(item_id, count)| client::BuyItemEntry { item_id, count })
                    .collect(),
            ),
        };
        Act::new(ClientInput::Shop(ShopInput::Buy(packet.clone())), &packet)
    }

    pub fn shop_ui_close() -> Act {
        Act::new(
            ClientInput::Shop(ShopInput::UiClose),
            &client::RequestBuySellUIClose,
        )
    }
}

// ------------------------------------------------------------------------------------------
// The scenarios' view of the world
// ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Who {
    /// The leader of the Lions, the clan that owns Gludio.
    Lord,
    /// The leader of the Wolves, the clan that attacks.
    Raider,
    /// A GM without a clan.
    Gm,
}

impl Who {
    const ALL: [Who; 3] = [Who::Lord, Who::Raider, Who::Gm];

    fn index(self) -> usize {
        match self {
            Who::Lord => 0,
            Who::Raider => 1,
            Who::Gm => 2,
        }
    }
}

/// One row of `items`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ItemRow {
    object_id: i32,
    item_id: i32,
    count: i64,
    loc: String,
}

/// What the store says about a character that left.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Left {
    level: i32,
    clan_id: i32,
    /// Every row of `items` owned by the character, sorted by object id.
    items: Vec<ItemRow>,
}

impl Left {
    /// The count of `item_id` in the inventory (worn or not).
    fn count_of(&self, item_id: i32) -> i64 {
        self.items
            .iter()
            .filter(|i| i.item_id == item_id && matches!(i.loc.as_str(), "INVENTORY" | "PAPERDOLL"))
            .map(|i| i.count)
            .sum()
    }
}

/// How a scenario starts: the switch of `SideBySide.ini`, where the three players stand and how
/// long the hall auction runs.
#[derive(Clone, Copy, Debug)]
struct Opts {
    /// `SiegeDatabaseWrites` (D-51).
    siege_writes: bool,
    /// Where the lord, the raider and the GM stand when they enter.
    positions: [(i32, i32, i32); 3],
    /// How long the auction of the hall runs, in milliseconds. The bare world needs a few
    /// seconds, the real servers more (they log in and start up on the clock).
    auction_ms: i64,
}

/// The two ways to play a scenario. `collect_until` returns everything the player's client got
/// since the last call, up to the packet `stop` accepts and a moment after it (the rest of that
/// event); `collect_quiet` returns what arrives in a quiet moment.
trait Harness {
    fn object_id(&self, who: Who) -> i32;
    /// Logs in, selects and enters the world; returns every packet up to the closing `SKILL_LIST`.
    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>>;
    async fn act(&mut self, who: Who, act: Act);
    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>>;
    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>>;
    /// Lets `seconds` of game time pass (the bare world ticks; the real server sleeps).
    async fn wait_seconds(&mut self, seconds: u64);
    /// Waits until the wall clock reaches `unix_ms` (the dates of the residences are wall-clock
    /// dates; the bare world ticks along in real time).
    async fn wait_until(&mut self, unix_ms: i64);
    /// Logs out and returns what was stored.
    async fn leave(&mut self, who: Who) -> Left;
    /// The residence tables as the store has them now.
    async fn rows(&mut self) -> Rows;
    /// Prints what the world sent to the store (debugging; the real servers print nothing).
    fn debug_commands(&self) {}
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

/// Everything the world sent to the persistence task, in order.
#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

struct RecordingOutbox(Persisted);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    fn commands(&self) -> Vec<PersistCommand> {
        self.0.lock().unwrap().clone()
    }
}

struct BareWorld {
    world: World,
    log: Log,
    persisted: Persisted,
    seed: Seed,
    /// How many of the persistence commands were looked at for reads that need an answer.
    answered: usize,
    players: [PlayerData; 3],
    /// Keeps the folder of the datapack (nothing reads it after the start).
    _root: tempfile::TempDir,
}

/// Far above the ids the factory hands out to the NPCs (it counts up from `RUST_FIRST_OBJECT_ID`).
const BARE_IDS: [i32; 3] = [0x7000_0001, 0x7000_0002, 0x7000_0003];
/// The longest the bare world waits for an event: 60 seconds of world time (50 ms ticks).
const BARE_LIMIT_TICKS: usize = 1_200;
/// The moment after an event that is collected with it: 600 ms.
const BARE_SETTLE_TICKS: usize = 12;

/// What a new fighter owns: `(item id, count, paperdoll slot if worn)`. The same list as the
/// `initialEquipment.xml` of `m6_data.rs` (class 0).
fn start_items() -> Vec<(i32, i64, Option<usize>)> {
    use l2_protocol_game::server::charscreen::paperdoll;
    vec![
        (2369, 1, Some(paperdoll::RHAND)),
        (1146, 1, Some(paperdoll::CHEST)),
        (1147, 1, Some(paperdoll::LEGS)),
        (5588, 1, None),
        (m5_data::MANA_POTION, m5_data::POTIONS_AT_START, None),
        (SOULSHOT_ID, m5_data::SOULSHOTS_AT_START, None),
        (m6_data::TEST_BLADE, m6_data::BLADES_AT_START, None),
        (m6_data::ENCHANT_SCROLL_D, m6_data::SCROLLS_AT_START, None),
        (m6_data::GEM, m6_data::GEMS_AT_START, None),
        (m6_data::ORE, m6_data::ORE_AT_START, None),
        (m6_data::BLOOD_MARK, m6_data::BLOOD_MARKS_AT_START, None),
    ]
}

/// The bare character `who`: a level 10 fighter with the starting items and the clan the seed
/// gives him. His clan, and the residences of the clan, come to the world the way the session
/// hands them over at `CharacterSelect`.
fn bare_player(who: Who, opts: &Opts) -> PlayerData {
    let index = who.index();
    let account = [ACCOUNT_LORD, ACCOUNT_RAIDER, ACCOUNT_GM][index];
    let mut items = vec![ItemData {
        object_id: ObjectId(0x7100_0000 + i32::try_from(index * 100 + 99).unwrap()),
        item_id: ADENA_ID,
        count: m6_data::START_ADENA,
        loc: "INVENTORY".to_owned(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }];
    for (position, (item_id, count, slot)) in start_items().into_iter().enumerate() {
        items.push(ItemData {
            object_id: ObjectId(0x7100_0000 + i32::try_from(index * 100 + position).unwrap()),
            item_id,
            count,
            loc: if slot.is_some() { "PAPERDOLL" } else { "INVENTORY" }.to_owned(),
            loc_data: slot.map_or(0, |s| i32::try_from(s).unwrap()),
            mana_left: -1,
            time: -1,
            ..Default::default()
        });
    }
    if who == Who::Lord {
        items.push(Seed::lord_ticket());
    }
    let (x, y, z) = opts.positions[index];
    let mut data = PlayerData {
        object_id: ObjectId(BARE_IDS[index]),
        account: account.to_owned(),
        name: Seed::name_of(who).to_owned(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 1,
        hair_style: 1,
        hair_color: 1,
        level: m6_data::LEADER_LEVEL,
        exp: m6_data::LEADER_EXP,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: if who == Who::Gm { m7_data::GM_LEVEL } else { 0 },
        x,
        y,
        z,
        heading: 0,
        cur_hp: 191.8,
        max_hp: 191.8,
        cur_mp: 81.5,
        max_mp: 81.5,
        cur_cp: 76.7,
        max_cp: 76.7,
        online_time: 0,
        items,
        skills: [m5_data::WIND_STRIKE, m5_data::WIND_WALK]
            .into_iter()
            .map(|skill_id| SkillData {
                skill_id,
                skill_level: 1,
            })
            .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    };
    if let Some(clan) = Seed::clan_of(who) {
        data.profile.clan = Some(PlayerClanState {
            clan_id: clan,
            privileges: l2_world::clan::ClanPrivilegesRef::Own(0x00FF_FFFF),
            power_grade: 1,
            ..PlayerClanState::default()
        });
    }
    data
}

impl BareWorld {
    /// Loads the configuration, datapack, geodata, zones, html and spawns the way
    /// `Server::start_with` does, without the database and the sockets.
    fn start(opts: &Opts) -> Self {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        write_game_config(&config_dir, 4242, opts.siege_writes);
        let data_root = root.path().join("datapack");
        write_datapack(&data_root);
        let seed = Seed::new(now_ms(), opts.auction_ms);

        let config = GameConfig::load(&config_dir).unwrap();
        // The datapack options of the config (`options_from_config` of the server, which is
        // private): the NPC values this test sets, the rest are the defaults.
        let options = DatapackOptions {
            skills: skill_load_options(&config, &config_dir),
            max_aggro_range: config.npc.max_aggro_range,
            default_corpse_time: config.npc.default_corpse_time,
            ..DatapackOptions::default()
        };
        let datapack = Arc::new(Datapack::load_with(&data_root, &options).unwrap());
        let static_data: Arc<dyn StaticData> =
            Arc::new(DatapackData::load(Arc::clone(&datapack), &config_dir));
        let settings = world_settings(&config, &config_dir);
        let geo_settings = geo::GeoSettings::load(&config, &config_dir, &data_root);
        let engine = geo::load_engine(&geo_settings);
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());

        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(7),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        world.set_static_data(Box::new(static_data));
        world.set_settings(settings);
        world.set_regen_settings(regen_settings(&config));
        world.set_summon_settings(summon_settings(&config));
        world.set_geo(engine);
        world.set_zone_data(Arc::clone(&datapack.zones), Arc::clone(&datapack.map_regions));
        world.set_html_cache(Arc::clone(&datapack.html));
        world.set_teleporter_data(Arc::clone(&datapack.teleporters));
        world.set_hit_condition_bonus(Arc::clone(&datapack.hit_condition_bonus));
        world.set_exp_data(
            Arc::clone(&datapack.experience),
            Arc::clone(&datapack.experience_loss),
            Arc::clone(&datapack.karma_loss),
            Arc::clone(&datapack.skill_trees),
        );
        world.set_skill_learn_data(Arc::clone(&datapack.skill_learn));
        world.set_pet_skill_data(Arc::clone(&datapack.pet_skills));
        world.set_pet_data(Arc::clone(&datapack.pets));
        world.set_cubic_data(Arc::clone(&datapack.cubics));
        world.set_multisell_data(Arc::clone(&datapack.multisell), multisell_settings(&config));
        world.set_enchant_data(
            Arc::clone(&datapack.enchant_items),
            Arc::clone(&datapack.enchant_groups),
            Arc::clone(&datapack.elemental_attributes),
        );
        world.spawn_door_data(&datapack, &ids).unwrap();
        npcs::spawn_npcs(&mut world, &datapack, ids, &config, &config_dir);
        // The tables Java loads from the database at its start: no clans yet, the residences the
        // seed describes.
        world.set_clan_settings(clan_settings(&config, &config_dir));
        world.load_clans(Vec::new(), Vec::new());
        world.set_residence_settings(residence_settings(&config, &config_dir));
        world.set_fort_settings(fort_settings(&config, &config_dir));
        world.set_siege_schedule(Arc::clone(&datapack.siege_schedule));
        let residences = seed.residences();
        let castle_names: Vec<(i32, String)> = residences
            .castles
            .iter()
            .map(|c| (c.id, c.name.clone()))
            .collect();
        world.load_residences(residences);
        world.set_siege_settings(siege_settings(&config, &config_dir, &castle_names));
        world.load_sieges(seed.sieges());
        world.load_fort_data(seed.fort_sieges());
        world.set_clan_hall_settings(clan_hall_settings(&config));
        world.set_residence_npc_settings(residence_npc_settings(&config));
        world.load_clan_hall_auctions(seed.auctions());
        world.set_mail_settings(mail_settings(&config, &config_dir));
        world.load_mail(l2_world::MailLoad::default());
        let buy_options = BuyListOptions {
            custom_buylist_load: config.general.custom_buylist_load,
            correct_prices: config.general.correct_prices,
        };
        world.set_buy_lists(
            Arc::new(BuyLists::load(&datapack.root, &datapack.items, &buy_options)),
            Arc::new(MerchantPriceConfigs::load(&datapack.root)),
        );
        world.load_crests(Vec::new());
        let players = Who::ALL.map(|who| bare_player(who, opts));
        Self {
            world,
            log: Log::default(),
            persisted,
            seed,
            answered: 0,
            players,
            _root: root,
        }
    }

    fn db(&self) -> Db {
        Db::replay(&self.seed, &self.persisted.commands())
    }

    fn input(&mut self, who: Who, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.index() + 1).unwrap()),
            input,
        });
        self.answer_reads();
    }

    fn ticks(&mut self, count: usize) {
        for _ in 0..count {
            self.world.tick();
            self.answer_reads();
        }
    }

    /// What the persistence task does for the commands that ask a question (M6-SYNC-01, D-50): it
    /// reads the rows and answers on the world's channel. Here the rows come from the model of the
    /// tables.
    fn answer_reads(&mut self) {
        loop {
            let commands = self.persisted.commands();
            if self.answered >= commands.len() {
                break;
            }
            let new = commands[self.answered..].to_vec();
            self.answered = commands.len();
            for command in new {
                match command {
                    PersistCommand::ResidenceRereadRequest {
                        residence_id,
                        kind,
                        then,
                    } => {
                        // The model of the tables as it is when the question is asked: the
                        // commands before this one.
                        let db = Db::replay(&self.seed, &commands[..self.answered - 1]);
                        let rows: ResidenceRereadRows = db.reread(kind, residence_id);
                        let _: ResidenceReadKind = kind;
                        self.world.handle(WorldCommand::ResidenceReread {
                            result: Ok(rows),
                            then,
                        });
                    }
                    PersistCommand::ClanWarehouseRequest { player, clan_id } => {
                        let db = Db::replay(&self.seed, &commands[..self.answered - 1]);
                        self.world.handle(WorldCommand::ClanWarehouseLoaded {
                            player,
                            clan_id,
                            result: Ok(db.clan_warehouse(clan_id)),
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Harness for BareWorld {
    fn object_id(&self, who: Who) -> i32 {
        BARE_IDS[who.index()]
    }

    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>> {
        let index = who.index();
        // The session loads the character from the store: what the world stored when he left (the
        // first time the new character), and at `CharacterSelect` his clan and its residences as
        // the tables have them now (D-50, D-51).
        let id = BARE_IDS[index];
        let mut player = self
            .persisted
            .commands()
            .into_iter()
            .rev()
            .find_map(|c| match c {
                PersistCommand::StorePlayer(data) if data.object_id == ObjectId(id) => Some(*data),
                _ => None,
            })
            .unwrap_or_else(|| self.players[index].clone());
        if let Some(clan) = Seed::clan_of(who) {
            let db = self.db();
            player.profile.clan_record = Some(Box::new(db.clan_record(clan, BARE_IDS)));
            player.profile.residence_refresh = Some(Box::new(db.refresh(clan)));
        }
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(index + 1).unwrap()),
            player: Box::new(player),
            sink: Box::new(Sink(BARE_IDS[index], self.log.clone())),
            ip: None,
        });
        self.answer_reads();
        let packets = self.log.take(BARE_IDS[index]);
        assert_eq!(names(&packets).last().map(String::as_str), Some("SKILL_LIST"));
        packets
    }

    async fn act(&mut self, who: Who, act: Act) {
        self.input(who, act.input);
    }

    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        let id = BARE_IDS[who.index()];
        let mut examined = 0;
        for _ in 0..BARE_LIMIT_TICKS {
            let mine = self.log.peek(id);
            if mine[examined..].iter().any(|body| stop(body)) {
                // Let the rest of the event arrive, like the quiet period of a socket does.
                self.ticks(BARE_SETTLE_TICKS);
                return self.log.take(id);
            }
            examined = mine.len();
            self.ticks(1);
        }
        panic!(
            "the event did not happen for {who:?} in 60 s of world time; got {:?}",
            names(&self.log.peek(id))
        );
    }

    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>> {
        self.ticks(10);
        self.log.take(BARE_IDS[who.index()])
    }

    async fn wait_seconds(&mut self, seconds: u64) {
        self.ticks(usize::try_from(seconds).unwrap() * 20);
    }

    async fn wait_until(&mut self, unix_ms: i64) {
        // Real time passes with the ticks: the dates the world compares are wall-clock dates.
        while now_ms() <= unix_ms {
            self.ticks(5);
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        self.ticks(40);
    }

    async fn leave(&mut self, who: Who) -> Left {
        let id = BARE_IDS[who.index()];
        self.input(who, ClientInput::Logout);
        self.ticks(2);
        let goodbye = self.log.take(id);
        assert_eq!(
            goodbye.last(),
            Some(&LeaveWorld.to_bytes()),
            "{:?}",
            names(&goodbye)
        );
        let data = self
            .persisted
            .commands()
            .into_iter()
            .rev()
            .find_map(|c| match c {
                PersistCommand::StorePlayer(data) if data.object_id == ObjectId(id) => Some(data),
                _ => None,
            })
            .expect("the character was stored");
        let mut items: Vec<ItemRow> = self
            .db()
            .items_of(id)
            .into_iter()
            .map(|item| ItemRow {
                object_id: item.object_id.0,
                item_id: item.item_id,
                count: item.count,
                loc: item.loc,
            })
            .collect();
        items.sort_by_key(|i| i.object_id);
        Left {
            level: data.level,
            clan_id: data.profile.clan.as_ref().map_or(0, |c| c.clan_id),
            items,
        }
    }

    async fn rows(&mut self) -> Rows {
        self.db().rows()
    }

    fn debug_commands(&self) {
        for command in self.persisted.commands() {
            let text = format!("{command:?}");
            eprintln!("persist {}", &text[..text.len().min(220)]);
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

/// `object_id, item_id, count, loc` of `items`.
type ItemColumns = (i32, Option<i32>, u64, Option<String>);

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
    /// A throwaway database with both schemas and the residence rows of `seed`, the login server,
    /// and the game server registered with it. `None` (and a printed note) without
    /// `L2_TEST_DATABASE_URL`.
    async fn start(tag: &str, seed: &Seed, siege_writes: bool) -> Option<Self> {
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
        // The residences Sieghardt (the Java game server) keeps in the shared database (D-39).
        for statement in seed.startup_sql() {
            db.exec(&statement).await;
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
        write_game_config(&gs_root.path().join("config"), link_port, siege_writes);
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

    /// The persistence task writes after the world sent the command, in order; a third of a
    /// second is far more than a statement takes.
    async fn settle(&self) {
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    /// The residence tables, sorted like `Db::rows`.
    async fn rows(&self) -> Rows {
        use m7_db::{
            AuctionRow, BidRow, CastleRow, ClanAdena, ClanDbRow, FortRow, FunctionRow, GuardRow, HallRow,
        };
        let pool = &self.pool;
        let castles: Vec<(i64, i64, i64, i64, String, i64, String, i64)> = sqlx::query_as(
            "SELECT CAST(id AS SIGNED), CAST(taxPercent AS SIGNED), CAST(treasury AS SIGNED), \
             CAST(siegeDate AS SIGNED), regTimeOver, CAST(regTimeEnd AS SIGNED), showNpcCrest, \
             CAST(ticketBuyCount AS SIGNED) FROM castle ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let clans: Vec<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT CAST(clan_id AS SIGNED), CAST(IFNULL(hasCastle, 0) AS SIGNED), \
             CAST(reputation_score AS SIGNED), CAST(blood_alliance_count AS SIGNED), \
             CAST(blood_oath_count AS SIGNED), CAST(auction_bid_at AS SIGNED) FROM clan_data ORDER BY clan_id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let siege_clans: Vec<(i64, i64, i64)> = sqlx::query_as(
            "SELECT CAST(castle_id AS SIGNED), CAST(clan_id AS SIGNED), CAST(IFNULL(type, 0) AS SIGNED) \
             FROM siege_clans ORDER BY castle_id, clan_id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let guards: Vec<(i64, i64, i64)> = sqlx::query_as(
            "SELECT CAST(castleId AS SIGNED), CAST(npcId AS SIGNED), CAST(isHired AS SIGNED) \
             FROM castle_siege_guards ORDER BY castleId, npcId, isHired",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let forts: Vec<(i64, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT CAST(id AS SIGNED), CAST(siegeDate AS SIGNED), CAST(lastOwnedTime AS SIGNED), \
             CAST(owner AS SIGNED), CAST(state AS SIGNED), CAST(castleId AS SIGNED), \
             CAST(supplyLvL AS SIGNED) FROM fort ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let fort_clans: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT CAST(fort_id AS SIGNED), CAST(clan_id AS SIGNED) FROM fortsiege_clans ORDER BY fort_id, clan_id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let halls: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT CAST(id AS SIGNED), CAST(ownerId AS SIGNED), CAST(paidUntil AS SIGNED), \
             CAST(paid AS SIGNED) FROM clanhall ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let mut functions: Vec<FunctionRow> = Vec::new();
        for (query, label) in [
            (
                "SELECT CAST(castle_id AS SIGNED), CAST(`type` AS SIGNED), CAST(lvl AS SIGNED), \
                 CAST(lease AS SIGNED), CAST(rate AS SIGNED), CAST(endTime AS SIGNED) \
                 FROM castle_functions ORDER BY castle_id, `type`",
                "castle",
            ),
            (
                "SELECT CAST(fort_id AS SIGNED), CAST(`type` AS SIGNED), CAST(lvl AS SIGNED), \
                 CAST(lease AS SIGNED), CAST(rate AS SIGNED), CAST(endTime AS SIGNED) \
                 FROM fort_functions ORDER BY fort_id, `type`",
                "fort",
            ),
            (
                "SELECT CAST(hall_id AS SIGNED), CAST(`type` AS SIGNED), CAST(lvl AS SIGNED), \
                 CAST(lease AS SIGNED), CAST(rate AS SIGNED), CAST(endTime AS SIGNED) \
                 FROM clanhall_functions ORDER BY hall_id, `type`",
                "clanhall",
            ),
        ] {
            let rows: Vec<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(query)
                .fetch_all(pool)
                .await
                .unwrap();
            functions.extend(rows.into_iter().map(|(id, kind, lvl, lease, rate, end_time)| FunctionRow {
                table: label,
                residence_id: i32::try_from(id).unwrap(),
                function_type: i32::try_from(kind).unwrap(),
                lvl: i32::try_from(lvl).unwrap(),
                lease: i32::try_from(lease).unwrap(),
                rate,
                end_time,
            }));
        }
        let auctions: Vec<(i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT CAST(id AS SIGNED), CAST(sellerId AS SIGNED), CAST(startingBid AS SIGNED), \
             CAST(currentBid AS SIGNED), CAST(endDate AS SIGNED) FROM auction ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let bids: Vec<(i64, i64, String, String, i64)> = sqlx::query_as(
            "SELECT CAST(auctionId AS SIGNED), CAST(bidderId AS SIGNED), bidderName, clan_name, \
             CAST(maxBid AS SIGNED) FROM auction_bid ORDER BY auctionId, bidderId",
        )
        .fetch_all(pool)
        .await
        .unwrap();
        let mut clan_adena = Vec::new();
        for clan in &clans {
            let (adena,): (i64,) = sqlx::query_as(
                "SELECT CAST(IFNULL(SUM(`count`), 0) AS SIGNED) FROM items \
                 WHERE owner_id = ? AND loc = 'CLANWH' AND item_id = ?",
            )
            .bind(clan.0)
            .bind(ADENA_ID)
            .fetch_one(pool)
            .await
            .unwrap();
            clan_adena.push(ClanAdena {
                clan_id: i32::try_from(clan.0).unwrap(),
                adena,
            });
        }
        let i = |v: i64| i32::try_from(v).unwrap();
        Rows {
            castles: castles
                .into_iter()
                .map(|(id, tax, treasury, date, over, end, crest, tickets)| CastleRow {
                    id: i(id),
                    tax_percent: i(tax),
                    treasury,
                    siege_date: date,
                    reg_time_over: over == "true",
                    reg_time_end: end,
                    show_npc_crest: crest == "true",
                    ticket_buy_count: i(tickets),
                })
                .collect(),
            clans: clans
                .into_iter()
                .map(|(id, castle, reputation, alliance, oath, bid_at)| ClanDbRow {
                    clan_id: i(id),
                    has_castle: i(castle),
                    reputation: i(reputation),
                    blood_alliance_count: i(alliance),
                    blood_oath_count: i(oath),
                    auction_bid_at: i(bid_at),
                })
                .collect(),
            siege_clans: siege_clans
                .into_iter()
                .map(|(castle, clan, kind)| (i(castle), i(clan), i(kind)))
                .collect(),
            guards: guards
                .into_iter()
                .map(|(castle, npc, hired)| GuardRow {
                    castle_id: i(castle),
                    npc_id: i(npc),
                    hired: hired != 0,
                })
                .collect(),
            forts: forts
                .into_iter()
                .map(|(id, date, owned, owner, state, castle, supply)| FortRow {
                    id: i(id),
                    siege_date: date,
                    last_owned_time: owned,
                    owner: i(owner),
                    state: i(state),
                    castle_id: i(castle),
                    supply_level: i(supply),
                })
                .collect(),
            fort_clans: fort_clans.into_iter().map(|(fort, clan)| (i(fort), i(clan))).collect(),
            halls: halls
                .into_iter()
                .map(|(id, owner, paid_until, paid)| HallRow {
                    id: i(id),
                    owner_id: i(owner),
                    paid_until,
                    paid: paid != 0,
                })
                .collect(),
            functions,
            auctions: auctions
                .into_iter()
                .map(|(id, seller, starting, current, end)| AuctionRow {
                    id: i(id),
                    seller_id: i(seller),
                    starting_bid: starting,
                    current_bid: current,
                    end_date: end,
                })
                .collect(),
            bids: bids
                .into_iter()
                .map(|(auction, bidder, bidder_name, clan_name, max_bid)| BidRow {
                    auction_id: i(auction),
                    bidder_id: i(bidder),
                    bidder_name,
                    clan_name,
                    max_bid,
                })
                .collect(),
            clan_adena,
        }
    }

    /// The rows the scenarios compare for a character: its clan and every row of `items` it owns.
    async fn left_of(&self, char_id: i32) -> Left {
        let (level, clan_id): (Option<u8>, Option<u32>) =
            sqlx::query_as("SELECT level, clanid FROM characters WHERE charId = ?")
                .bind(char_id)
                .fetch_one(&self.pool)
                .await
                .unwrap();
        let rows: Vec<ItemColumns> = sqlx::query_as(
            "SELECT object_id, item_id, `count`, loc FROM items WHERE owner_id = ? ORDER BY object_id",
        )
        .bind(char_id)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        Left {
            level: i32::from(level.unwrap_or(0)),
            clan_id: clan_id.map_or(0, |c| i32::try_from(c).unwrap()),
            items: rows
                .into_iter()
                .map(|(object_id, item_id, count, loc)| ItemRow {
                    object_id,
                    item_id: item_id.unwrap_or(0),
                    count: i64::try_from(count).unwrap(),
                    loc: loc.unwrap_or_default(),
                })
                .collect(),
        }
    }
}

/// One rig at a time: every real-server test starts a login server, a game server and a
/// database, and the tests of this file run side by side.
static ONE_RIG_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The scenario against the real servers.
struct Net {
    rig: Rig,
    ids: [i32; 3],
    clients: [Option<FakeGameClient>; 3],
    _turn: tokio::sync::MutexGuard<'static, ()>,
}

impl Net {
    const ACCOUNTS: [&'static str; 3] = [ACCOUNT_LORD, ACCOUNT_RAIDER, ACCOUNT_GM];

    async fn start(tag: &str, opts: &Opts) -> Option<Self> {
        let turn = ONE_RIG_AT_A_TIME.lock().await;
        let seed = Seed::new(now_ms(), opts.auction_ms);
        let rig = Rig::start(tag, &seed, opts.siege_writes).await?;
        let mut ids = [0; 3];
        for who in Who::ALL {
            let i = who.index();
            ids[i] = rig
                .create_character_and_leave(Self::ACCOUNTS[i], Seed::name_of(who))
                .await;
        }
        // The clans come after the characters (their ids are made by the game server): the world
        // reads them at `CharacterSelect`.
        for statement in seed.late_sql(ids) {
            rig.db.exec(&statement).await;
        }
        for who in Who::ALL {
            let (x, y, z) = opts.positions[who.index()];
            // Level 10 with the HP, MP and CP of the level (the numbers of `m6_data.rs`); the casts
            // of the siege skills need the MP.
            sqlx::query(
                "UPDATE characters SET level = ?, exp = ?, x = ?, y = ?, z = ?, \
                 maxHp = 192, curHp = 192, maxMp = 82, curMp = 82, maxCp = 77, curCp = 77 WHERE charId = ?",
            )
            .bind(m6_data::LEADER_LEVEL)
            .bind(m6_data::LEADER_EXP)
            .bind(x)
            .bind(y)
            .bind(z)
            .bind(ids[who.index()])
            .execute(&rig.pool)
            .await
            .unwrap();
        }
        Some(Self {
            rig,
            ids,
            clients: [None, None, None],
            _turn: turn,
        })
    }

    fn client(&mut self, who: Who) -> &mut FakeGameClient {
        self.clients[who.index()]
            .as_mut()
            .expect("the client is in the world")
    }

    async fn finish(self) {
        self.rig.gs.shutdown(false).await;
        self.rig.ls.shutdown(false).await;
        self.rig.db.drop_db().await;
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

    async fn act(&mut self, who: Who, act: Act) {
        self.client(who).send(act.wire).await;
    }

    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        let client = self.client(who);
        let mut batch = Vec::new();
        loop {
            // A cast takes a few seconds: wait up to 40 s for the next packet.
            match client.recv_within(Duration::from_secs(40)).await {
                Recv::Packet(body) => {
                    let done = stop(&body);
                    batch.push(body);
                    if done {
                        break;
                    }
                }
                Recv::Closed => panic!("the game server closed the connection of {who:?}"),
                Recv::Silent => panic!(
                    "the event did not happen for {who:?} in 40 s; got {:?}",
                    names(&batch)
                ),
            }
        }
        // Longer than the 300 ms delay of the `InventoryUpdate` that follows an item use.
        batch.extend(client.read_quiet(Duration::from_millis(600)).await);
        batch
    }

    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>> {
        self.client(who).read_quiet(Duration::from_millis(500)).await
    }

    async fn wait_seconds(&mut self, seconds: u64) {
        tokio::time::sleep(Duration::from_secs(seconds)).await;
    }

    async fn wait_until(&mut self, unix_ms: i64) {
        let wait = unix_ms - now_ms();
        if wait > 0 {
            tokio::time::sleep(Duration::from_millis(u64::try_from(wait).unwrap())).await;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    async fn leave(&mut self, who: Who) -> Left {
        let mut game = self.clients[who.index()]
            .take()
            .expect("the client is in the world");
        let goodbye = game.logout().await;
        assert_eq!(
            goodbye.last(),
            Some(&LeaveWorld.to_bytes()),
            "{:?}",
            names(&goodbye)
        );
        // The login server hears about the logout only after the character is stored: when it
        // no longer counts the account, the rows are already final.
        self.rig.wait_until_out_of_game(Self::ACCOUNTS[who.index()]).await;
        self.rig.left_of(self.ids[who.index()]).await
    }

    async fn rows(&mut self) -> Rows {
        self.rig.settle().await;
        self.rig.rows().await
    }
}

// ------------------------------------------------------------------------------------------
// Scenarios
// ------------------------------------------------------------------------------------------

#[path = "e2e/m7_castle.rs"]
mod castle;

#[path = "e2e/m7_checks.rs"]
mod checks;
