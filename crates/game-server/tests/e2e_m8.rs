#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End to end for the global cycles of M8: a GM and players log in through the in-process login
//! server and game server and play an instance (made, entered, destroyed), a raid boss kill, an
//! item auction (viewed and bid on), the Seven Signs (a join at a priest, a contribution of seal
//! stones, the status page) and a boat (boarded and left at the dock). The Olympiad match and the
//! boat leg are played on the bare world only (see below).
//!
//! Every scenario is written once against a `Harness` and runs like `e2e_m7.rs`:
//!
//! * `Net`: the real thing. The Rust login server and the Rust game server in one process, a
//!   throwaway MariaDB, a `FakeGameClient` on a socket. It runs only when `L2_TEST_DATABASE_URL`
//!   is set (CI's MariaDB job; never point it at a database with real data). The rows the Java
//!   game server would keep (the raid boss, the item auction, the Seven Signs status) are
//!   inserted before the game server starts (D-39). After a scenario the rows are read back and
//!   compared with what Java's statements write.
//! * `BareWorld`: the same datapack, configuration, geodata, zones and spawns loaded the way the
//!   game server loads them, but with the world driven directly (no sockets, no database, ticks
//!   instead of time). The persistence commands the world sends are recorded, the reads the
//!   world asks for are answered from a model of the rows, so the same row checks run here.
//!
//! Each scenario runs twice per half: with `CycleDatabaseWrites = False` (D-53: the global cycle
//! rows stay with the Java game server; the world reads the shared rows again, D-50) and with
//! `True` (the rows Java writes).
//!
//! A client action is an `Act`: the `ClientInput` for the bare world and the bytes of the packet
//! the real client sends for `Net`. `every_act_decodes_on_the_wire` checks that the bytes decode
//! as the packet each action stands for.
//!
//! Left to the bare world on purpose:
//!
//! * The Olympiad match: the competition runs on the wall clock of the day (Java
//!   `Olympiad.getInstance`, 18:00 + 6 h); the bare world takes a clock override that a real
//!   server cannot be given. The nobles register through `World::olympiad_register_noble` (the
//!   NPC dialog is covered by `l2-world/tests/olympiad.rs`).
//! * The boat leg: 25 minutes of game time.
//!
//! There is no datapack and no geodata in CI: `e2e/datapack.rs`, `e2e/m3_data.rs` to
//! `e2e/m6_data.rs` and `e2e/m8_data.rs` write them into a temporary folder.

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
#[path = "e2e/m8_data.rs"]
mod m8_data;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_server::world_data::{
    DatapackData, multisell_settings, regen_settings, skill_load_options, summon_settings, world_settings,
};
use game_server::{Paths, Server, geo, hexid, npcs};
use l2_config::GameConfig;
use l2_data::{BuyListOptions, BuyLists, Datapack, DatapackOptions, MerchantPriceConfigs};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::{
    ExItemAuctionInfo, GetOffVehicle, GetOnVehicle, LeaveWorld, NpcHtmlMessage, NpcInfo, SsqStatus,
    SystemMessage, SystemMessageParam, TeleportToLocation, VehicleInfo,
};
use l2_protocol_game::{SystemMessageId, client};
use l2_protocol_login::server::PlayOk;
use l2_testkit::fake_game_client::{
    FakeGameClient, Recv, character_create, enter_world_packet, names_of, packet_name,
};
use l2_testkit::fake_login_client::FakeClient as LoginClient;
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
use l2_world::sevensigns::{
    PERIOD_COMPETITION, SevenSignsLoad, SevenSignsStatus, SevenSignsWrite, SignsPlayer,
};
use l2_world::{
    BossLoad, BossWrite, ClientInput, CompetitionType, GameState, IdFactory, ItemAuctionInput,
    ItemAuctionLoad, ItemAuctionRecord, ItemAuctionWrite, ItemData, LeaveReason, LsEvent, ObjectId,
    OlympiadData, OlympiadLoad, OlympiadSettings, OlympiadWrite, Outbox, PacketSink, PersistCommand,
    PlayerData, PlayerProfile, RaidBossData, SessionId, SkillData, StaticData, VehicleInput, World,
    WorldCommand, WorldRng,
};
use login_server::state::{LoginState, ServerStatus as ListStatus};

use l2_db::Pool;

const ACCOUNT_GM: &str = "e2em8accg";
const PASSWORD: &str = "secret";
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

/// Everything in the game server's `config` folder except `Database.ini`. Like the M7 rig (see
/// `e2e_m4.rs` for the reasons), with a lot of starting adena, the admin rights, the item
/// auctions and boats switched on and `SideBySide.ini` with the switch of the scenario (D-53).
fn write_game_config(config_dir: &Path, ls_port: u16, cycle_writes: bool) {
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
        "MaxNpcAnimation = 0\nMinimumChatLevel = 0\nAllowManor = False\nAltItemAuctionEnabled = True\nAllowBoat = True\n",
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
        format!(
            "CycleDatabaseWrites = {}\n",
            if cycle_writes { "True" } else { "False" }
        ),
    )
    .unwrap();
    m8_data::write_admin_config(config_dir);
    hexid::save(config_dir, SAMPLE_SERVER_ID, SAMPLE_HEXID);
}

/// The datapack of this test: the base classes and items, the world data of M3, the hunting data
/// of M4, the skills and items of M5, the social data of M6 and the data of M8.
fn write_datapack(root: &Path) {
    datapack::write_datapack(root);
    m3_data::write_world_data(root);
    m4_data::write_hunting_data(root);
    m5_data::write_skill_data(root);
    m6_data::write_social_data(root);
    m8_data::write_m8_data(root);
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

fn system_messages(batch: &[Vec<u8>]) -> Vec<SystemMessage> {
    all::<SystemMessage>(batch, "SYSTEM_MESSAGE")
}

fn message_ids(batch: &[Vec<u8>]) -> Vec<SystemMessageId> {
    system_messages(batch).into_iter().map(|m| m.id).collect()
}

fn message_of(batch: &[Vec<u8>], id: SystemMessageId) -> SystemMessage {
    system_messages(batch)
        .into_iter()
        .find(|m| m.id == id)
        .unwrap_or_else(|| panic!("no {id:?} in {:?} ({:?})", message_ids(batch), names(batch)))
}

fn has_message(batch: &[Vec<u8>], id: SystemMessageId) -> bool {
    message_ids(batch).contains(&id)
}

/// The texts of the system messages with one string (what `sendMessage` and the admin words
/// send).
fn texts(batch: &[Vec<u8>]) -> Vec<String> {
    system_messages(batch)
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn pages(batch: &[Vec<u8>]) -> Vec<String> {
    all::<NpcHtmlMessage>(batch, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|p| p.html)
        .collect()
}

/// The object id of the NPC called `npc_id` among the `NpcInfo` of `batch`.
fn npc_object(batch: &[Vec<u8>], npc_id: i32) -> i32 {
    all::<NpcInfo>(batch, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == npc_id)
        .unwrap_or_else(|| panic!("NPC {npc_id} is not in view: {:?}", names(batch)))
        .object_id
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

    pub fn ssq_status(page: i8) -> Act {
        Act::new(
            ClientInput::SsqStatus { page },
            &client::RequestSsqStatus { page },
        )
    }

    pub fn auction_info(instance_id: i32) -> Act {
        Act::new(
            ClientInput::ItemAuction(ItemAuctionInput::Info { instance_id }),
            &client::RequestInfoItemAuction { instance_id },
        )
    }

    pub fn auction_bid(instance_id: i32, bid: i64) -> Act {
        Act::new(
            ClientInput::ItemAuction(ItemAuctionInput::Bid { instance_id, bid }),
            &client::RequestBidItemAuction { instance_id, bid },
        )
    }

    pub fn get_on(boat_id: i32, (x, y, z): (i32, i32, i32)) -> Act {
        Act::new(
            ClientInput::Vehicle(VehicleInput::GetOn { boat_id, x, y, z }),
            &client::RequestGetOnVehicle { boat_id, x, y, z },
        )
    }

    pub fn get_off(boat_id: i32, (x, y, z): (i32, i32, i32)) -> Act {
        Act::new(
            ClientInput::Vehicle(VehicleInput::GetOff { boat_id, x, y, z }),
            &client::RequestGetOffVehicle { boat_id, x, y, z },
        )
    }
}

// ------------------------------------------------------------------------------------------
// The scenarios' view of the world
// ------------------------------------------------------------------------------------------

/// A number the rows or the persistence commands say; `Harness::fact` gives it for both halves.
#[derive(Clone, Copy, Debug)]
enum Fact {
    /// `raidboss_spawnlist.respawn_time` of the raid boss (0: the row of the seed, untouched).
    RaidRespawnTime,
    /// The raid points of the GM (`character_raid_points`).
    RaidPoints,
    /// `item_auction` rows besides the one of the seed.
    ItemAuctionNewRows,
    /// `item_auction_bid` rows.
    ItemAuctionBids,
    /// 1 when `seven_signs` has the GM as a member of the Dawn for the Seal of Avarice, else 0.
    SsJoined,
    /// `seven_signs.contribution_score` of the GM.
    SsContribution,
}

/// How a scenario starts: the switch of `SideBySide.ini` and where the GM stands.
#[derive(Clone, Copy, Debug)]
struct Opts {
    /// `CycleDatabaseWrites` (D-53).
    cycle_writes: bool,
    position: (i32, i32, i32),
    /// Whether the GM is a Human Warrior (a class that went through the first class transfer).
    warrior: bool,
    /// Bare world only: the Olympiad runs with the clock of the tests (see `olympiad_match`).
    olympiad: bool,
}

impl Opts {
    fn at_start(cycle_writes: bool) -> Self {
        Self {
            cycle_writes,
            position: m3_data::START,
            warrior: true,
            olympiad: false,
        }
    }

    fn with_olympiad(cycle_writes: bool) -> Self {
        Self {
            olympiad: true,
            ..Self::at_start(cycle_writes)
        }
    }

    fn at_dock(cycle_writes: bool) -> Self {
        Self {
            cycle_writes,
            position: (
                m8_data::TALKING_DOCK.0 + 50,
                m8_data::TALKING_DOCK.1,
                m8_data::TALKING_DOCK.2,
            ),
            warrior: true,
            olympiad: false,
        }
    }
}

/// The two ways to play a scenario with one player (the GM). `collect_until` returns everything
/// the player's client got since the last call, up to the packet `stop` accepts and a moment
/// after it (the rest of that event); `collect_quiet` returns what arrives in a quiet moment.
trait Harness {
    fn object_id(&self) -> i32;
    fn start_position(&self) -> (i32, i32, i32);
    /// Logs in, selects and enters the world; returns every packet up to the closing `SKILL_LIST`.
    async fn enter(&mut self) -> Vec<Vec<u8>>;
    async fn act(&mut self, act: Act);
    async fn collect_until(&mut self, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>>;
    async fn collect_quiet(&mut self) -> Vec<Vec<u8>>;
    /// Logs out (the character is stored) and returns what the world said last.
    async fn leave(&mut self);
    /// A number of the store as it is now (the persistence task has caught up).
    async fn fact(&mut self, fact: Fact) -> i64;
}

// ------------------------------------------------------------------------------------------
// What the Java game server keeps (the seed)
// ------------------------------------------------------------------------------------------

struct Seed {
    now: i64,
}

impl Seed {
    fn new() -> Self {
        Self { now: now_ms() }
    }

    fn raid_row(&self) -> RaidBossData {
        RaidBossData {
            boss_id: m8_data::RAID_ID,
            amount: 1,
            loc_x: m8_data::RAID_POS.0,
            loc_y: m8_data::RAID_POS.1,
            loc_z: m8_data::RAID_POS.2,
            heading: 0,
            respawn_delay: m8_data::RAID_RESPAWN.0,
            respawn_random: m8_data::RAID_RESPAWN.1,
            respawn_time: 0,
            current_hp: 3000.0,
            current_mp: 400.0,
        }
    }

    /// The auction the Java game server started a minute ago and that runs for an hour.
    fn auction_record(&self) -> ItemAuctionRecord {
        ItemAuctionRecord {
            auction_id: m8_data::AUCTION_ID,
            instance_id: m8_data::AUCTION_INSTANCE,
            auction_item_id: m8_data::AUCTION_ITEM_ID,
            starting_time: self.now - 60_000,
            ending_time: self.now + 3_600_000,
            state_id: 1,
            bids: Vec::new(),
        }
    }

    /// The `seven_signs_status` row: cycle 1, the competition period, saved just now.
    fn ss_status(&self) -> SevenSignsStatus {
        SevenSignsStatus {
            current_cycle: 1,
            active_period: PERIOD_COMPETITION,
            last_save: self.now,
            ..SevenSignsStatus::default()
        }
    }

    /// The statements of the Java game server before this one starts.
    fn startup_sql(&self) -> Vec<String> {
        let raid = self.raid_row();
        let auction = self.auction_record();
        vec![
            format!(
                "INSERT INTO raidboss_spawnlist (boss_id, amount, loc_x, loc_y, loc_z, heading, respawn_delay, respawn_random, respawn_time, currentHp, currentMp) \
                 VALUES ({}, 1, {}, {}, {}, 0, {}, {}, 0, 3000, 400)",
                raid.boss_id, raid.loc_x, raid.loc_y, raid.loc_z, raid.respawn_delay, raid.respawn_random
            ),
            format!(
                "INSERT INTO item_auction (auctionId, instanceId, auctionItemId, startingTime, endingTime, auctionStateId) \
                 VALUES ({}, {}, {}, {}, {}, {})",
                auction.auction_id,
                auction.instance_id,
                auction.auction_item_id,
                auction.starting_time,
                auction.ending_time,
                auction.state_id
            ),
            format!(
                "UPDATE seven_signs_status SET current_cycle = 1, active_period = {PERIOD_COMPETITION}, `date` = {} WHERE id = 0",
                self.now
            ),
        ]
    }
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

/// Far above the ids the factory hands out to the NPCs (it counts up from `RUST_FIRST_OBJECT_ID`).
const BARE_FIRST_ID: i32 = 0x7000_0001;
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

/// A bare level 10 character with the starting items: the GM (index 0, a Human Warrior) or a noble
/// (the others, Duelists by their base class).
fn bare_player(index: usize, position: (i32, i32, i32), warrior: bool, extra: &[(i32, i64)]) -> PlayerData {
    let id = BARE_FIRST_ID + i32::try_from(index).unwrap();
    let base = 0x7100_0000 + i32::try_from(index * 100).unwrap();
    let mut items = vec![ItemData {
        object_id: ObjectId(base + 99),
        item_id: ADENA_ID,
        count: m6_data::START_ADENA,
        loc: "INVENTORY".to_owned(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }];
    for (position, (item_id, count, slot)) in start_items().into_iter().enumerate() {
        items.push(ItemData {
            object_id: ObjectId(base + i32::try_from(position).unwrap()),
            item_id,
            count,
            loc: if slot.is_some() { "PAPERDOLL" } else { "INVENTORY" }.to_owned(),
            loc_data: slot.map_or(0, |s| i32::try_from(s).unwrap()),
            mana_left: -1,
            time: -1,
            ..Default::default()
        });
    }
    for (position, (item_id, count)) in extra.iter().enumerate() {
        items.push(ItemData {
            object_id: ObjectId(base + 50 + i32::try_from(position).unwrap()),
            item_id: *item_id,
            count: *count,
            loc: "INVENTORY".to_owned(),
            mana_left: -1,
            time: -1,
            ..Default::default()
        });
    }
    let gm = index == 0;
    let (class_id, base_class) = if gm && warrior {
        (m8_data::CLASS_WARRIOR, m8_data::CLASS_WARRIOR)
    } else if gm {
        (0, 0)
    } else {
        (0, m8_data::DUELIST)
    };
    let mut data = PlayerData {
        object_id: ObjectId(id),
        account: format!("e2em8acc{index}"),
        name: if gm {
            NAME_GM.to_owned()
        } else {
            format!("Noble{index}")
        },
        title: String::new(),
        class_id,
        base_class,
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
        access_level: if gm { m8_data::GM_LEVEL } else { 0 },
        x: position.0 + i32::try_from(index * 20).unwrap(),
        y: position.1,
        z: position.2,
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
    data.profile.noble = !gm;
    data
}

/// A moment inside a competition day, in UTC (the zone offset of the settings is 0).
const OLYMPIAD_BASE: i64 = 1_793_212_200_000;
const DAY_MS: i64 = 86_400_000;

/// The Olympiad of the config (`game_server::olympiad_settings`) with the clock of the tests: two
/// players start a match, a short wait before the teleport, Adena as the reward.
fn olympiad_test_settings(config: &GameConfig, cycle_writes: bool) -> OlympiadSettings {
    OlympiadSettings {
        classed: 2,
        non_classed: 2,
        wait_time: 3,
        classed_reward: vec![[ADENA_ID, 50]],
        non_classed_reward: vec![[ADENA_ID, 40]],
        clock_override_unix_ms: Some(OLYMPIAD_BASE),
        zone_offset_override_ms: Some(0),
        ..game_server::olympiad_settings(config, cycle_writes)
    }
}

struct BareWorld {
    world: World,
    log: Log,
    persisted: Persisted,
    seed: Seed,
    /// How many of the persistence commands were looked at for reads that need an answer.
    answered: usize,
    players: Vec<PlayerData>,
    opts: Opts,
    /// Keeps the folder of the datapack (nothing reads it after the start).
    _root: tempfile::TempDir,
}

impl BareWorld {
    /// Loads the configuration, datapack, geodata, zones, html and spawns the way
    /// `Server::start_with` does, without the database and the sockets, then the global cycles of
    /// M8 with the rows the seed describes. `players` is how many characters are made (the first
    /// is the GM); `extra_items` is what the GM carries besides the starting items.
    fn start(opts: &Opts, players: usize, extra_items: &[(i32, i64)]) -> Self {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        write_game_config(&config_dir, 4242, opts.cycle_writes);
        let data_root = root.path().join("datapack");
        write_datapack(&data_root);
        let seed = Seed::new();

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
        // Java: InstanceManager (the names and templates; M8-INST-01).
        world.set_instance_settings(game_server::world_data::instance_settings(&config, &config_dir));
        world.set_instance_data(Arc::clone(&datapack.instances));
        npcs::spawn_npcs(&mut world, &datapack, ids, &config, &config_dir);

        // Java: RaidBossPointsManager, GrandBossManager and RaidBossSpawnManager (M8-BOSS-01).
        world.set_boss_settings(game_server::world_data::boss_settings(&config, &config_dir));
        world.load_bosses(BossLoad::default());
        world.spawn_raid_bosses(vec![seed.raid_row()]);
        // Java: SevenSigns (after the castles), then the Mammon spawns (M8-SS-01).
        world.set_seven_signs_settings(game_server::seven_signs_settings(&config, &config_dir));
        world.load_seven_signs(SevenSignsLoad {
            status: Some(seed.ss_status()),
            ..SevenSignsLoad::default()
        });
        world.load_auto_spawns(Vec::new());
        world.spawn_seven_signs_npc();
        // Java: Olympiad.getInstance (GameServer; M8-OLY-01). The clock is the tests': a real
        // server has no clock override, so this half is bare only.
        if opts.olympiad {
            world.set_olympiad_settings(olympiad_test_settings(&config, opts.cycle_writes));
            world.load_olympiad(OlympiadLoad {
                data: Some(OlympiadData {
                    current_cycle: 1,
                    period: 0,
                    olympiad_end: OLYMPIAD_BASE + 10 * DAY_MS,
                    validation_end: 0,
                    next_weekly_change: OLYMPIAD_BASE + 5 * DAY_MS,
                }),
                ..OlympiadLoad::default()
            });
        }
        // Java: the scripts of the boats (M8-VEHICLE-01).
        world.set_vehicle_settings(game_server::vehicle_settings(&config));
        world.spawn_boats();
        // Java: ItemAuctionManager (after the managers; L-05).
        let item_auction_settings = game_server::item_auctions::item_auction_settings(&config, &config_dir);
        let xml =
            std::fs::read_to_string(data_root.join(game_server::item_auctions::ITEM_AUCTIONS_FILE)).unwrap();
        let instances = game_server::item_auctions::instances_from_xml(&xml, &|item_id| {
            datapack.items.get(item_id).is_some()
        });
        world.set_item_auction_settings(item_auction_settings);
        world.load_item_auctions(ItemAuctionLoad {
            next_auction_id: seed.auction_record().auction_id + 1,
            instances,
            auctions: vec![seed.auction_record()],
        });
        let buy_options = BuyListOptions {
            custom_buylist_load: config.general.custom_buylist_load,
            correct_prices: config.general.correct_prices,
        };
        world.set_buy_lists(
            Arc::new(BuyLists::load(&datapack.root, &datapack.items, &buy_options)),
            Arc::new(MerchantPriceConfigs::load(&datapack.root)),
        );
        world.load_crests(Vec::new());

        let players: Vec<PlayerData> = (0..players)
            .map(|index| {
                bare_player(
                    index,
                    opts.position,
                    opts.warrior,
                    if index == 0 { extra_items } else { &[] },
                )
            })
            .collect();
        Self {
            world,
            log: Log::default(),
            persisted,
            seed,
            answered: 0,
            players,
            opts: *opts,
            _root: root,
        }
    }

    fn id_of(index: usize) -> i32 {
        BARE_FIRST_ID + i32::try_from(index).unwrap()
    }

    fn input_of(&mut self, index: usize, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(index + 1).unwrap()),
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

    /// Enters player `index` (the character as the world stored it when it left, the first time
    /// the new one).
    fn enter_player(&mut self, index: usize) -> Vec<Vec<u8>> {
        let id = Self::id_of(index);
        let player = self
            .persisted
            .commands()
            .into_iter()
            .rev()
            .find_map(|c| match c {
                PersistCommand::StorePlayer(data) if data.object_id == ObjectId(id) => Some(*data),
                _ => None,
            })
            .unwrap_or_else(|| self.players[index].clone());
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(index + 1).unwrap()),
            player: Box::new(player),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.answer_reads();
        self.log.take(id)
    }

    /// The Seven Signs members the persistence task would read: every row the world wrote.
    fn ss_members(&self) -> Vec<SignsPlayer> {
        let mut members: Vec<SignsPlayer> = Vec::new();
        for command in self.persisted.commands() {
            let update = match command {
                PersistCommand::SevenSigns(SevenSignsWrite::InsertPlayer { char_id, cabal, seal }) => {
                    Some(SignsPlayer {
                        char_id,
                        cabal,
                        seal,
                        ..SignsPlayer::default()
                    })
                }
                PersistCommand::SevenSigns(SevenSignsWrite::UpdatePlayer(player)) => Some(player),
                // `Player.store` carries the row of the member (`saveSevenSignsData(objectId)`).
                PersistCommand::StorePlayer(data) => data.profile.seven_signs_to_store,
                _ => None,
            };
            if let Some(player) = update {
                members.retain(|m| m.char_id != player.char_id);
                members.push(player);
            }
        }
        members
    }

    /// What the persistence task does for the commands that ask a question (D-50): it reads the
    /// rows and answers on the world's channel. Here the rows come from the model of the tables.
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
                    PersistCommand::ItemAuctionRequest { instance_id, then } => {
                        self.world.handle(WorldCommand::ItemAuctionReread {
                            instance_id,
                            result: Ok(vec![self.seed.auction_record()]),
                            then,
                        });
                    }
                    PersistCommand::SevenSignsReadRequest { then } => {
                        let load = SevenSignsLoad {
                            players: self.ss_members(),
                            status: Some(self.seed.ss_status()),
                            ..SevenSignsLoad::default()
                        };
                        self.world.handle(WorldCommand::SevenSignsLoaded {
                            result: Ok(load),
                            then,
                        });
                    }
                    PersistCommand::CrestIdsRequest { ticket } => {
                        self.world.handle(WorldCommand::CrestIdsLoaded {
                            ticket,
                            result: Ok(Vec::new()),
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    fn collect_until_of(&mut self, index: usize, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        let id = Self::id_of(index);
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
            "the event did not happen in 60 s of world time; got {:?}",
            names(&self.log.peek(id))
        );
    }
}

impl Harness for BareWorld {
    fn object_id(&self) -> i32 {
        Self::id_of(0)
    }

    fn start_position(&self) -> (i32, i32, i32) {
        self.opts.position
    }

    async fn enter(&mut self) -> Vec<Vec<u8>> {
        let packets = self.enter_player(0);
        assert_eq!(names(&packets).last().map(String::as_str), Some("SKILL_LIST"));
        packets
    }

    async fn act(&mut self, act: Act) {
        self.input_of(0, act.input);
    }

    async fn collect_until(&mut self, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        self.collect_until_of(0, stop)
    }

    async fn collect_quiet(&mut self) -> Vec<Vec<u8>> {
        self.ticks(10);
        self.log.take(Self::id_of(0))
    }

    async fn leave(&mut self) {
        self.input_of(0, ClientInput::Logout);
        self.ticks(2);
        let goodbye = self.log.take(Self::id_of(0));
        assert_eq!(
            goodbye.last(),
            Some(&LeaveWorld.to_bytes()),
            "{:?}",
            names(&goodbye)
        );
    }

    async fn fact(&mut self, fact: Fact) -> i64 {
        let commands = self.persisted.commands();
        let me = self.object_id();
        match fact {
            Fact::RaidRespawnTime => commands
                .iter()
                .rev()
                .find_map(|c| match c {
                    PersistCommand::Boss(BossWrite::RaidBossUpdate(rows)) => rows
                        .iter()
                        .find(|r| r.boss_id == m8_data::RAID_ID)
                        .map(|r| r.respawn_time),
                    _ => None,
                })
                .unwrap_or(0),
            Fact::RaidPoints => commands
                .iter()
                .rev()
                .find_map(|c| match c {
                    PersistCommand::Boss(BossWrite::RaidPoints { char_id, points, .. }) if *char_id == me => {
                        Some(i64::from(*points))
                    }
                    _ => None,
                })
                .unwrap_or(0),
            Fact::ItemAuctionNewRows => {
                let mut ids: Vec<i32> = commands
                    .iter()
                    .filter_map(|c| match c {
                        PersistCommand::ItemAuction(ItemAuctionWrite::Store { auction_id, .. })
                            if *auction_id != m8_data::AUCTION_ID =>
                        {
                            Some(*auction_id)
                        }
                        _ => None,
                    })
                    .collect();
                ids.sort_unstable();
                ids.dedup();
                i64::try_from(ids.len()).unwrap()
            }
            Fact::ItemAuctionBids => {
                let mut bids: Vec<(i32, i32)> = commands
                    .iter()
                    .filter_map(|c| match c {
                        PersistCommand::ItemAuction(ItemAuctionWrite::Bid {
                            auction_id,
                            player_obj_id,
                            ..
                        }) => Some((*auction_id, *player_obj_id)),
                        _ => None,
                    })
                    .collect();
                bids.sort_unstable();
                bids.dedup();
                i64::try_from(bids.len()).unwrap()
            }
            Fact::SsJoined => i64::from(
                self.ss_members()
                    .iter()
                    .any(|m| m.char_id == me && m.cabal == "dawn" && m.seal == 1),
            ),
            Fact::SsContribution => self
                .ss_members()
                .iter()
                .find(|m| m.char_id == me)
                .map_or(0, |m| m.contribution_score),
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
    /// A throwaway database with both schemas and the rows of `seed`, the login server, and the
    /// game server registered with it. `None` (and a printed note) without `L2_TEST_DATABASE_URL`.
    async fn start(tag: &str, seed: &Seed, opts: &Opts) -> Option<Self> {
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
        // The rows the Java game server keeps in the shared database (D-39).
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
        write_game_config(&gs_root.path().join("config"), link_port, opts.cycle_writes);
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

    fn in_game(&self, account: &str) -> bool {
        self.ls_state.game_servers.is_account_on_any_gs(account)
    }

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

    async fn count(&self, query: &'static str, bind: i32) -> i64 {
        let (n,): (i64,) = sqlx::query_as(query)
            .bind(bind)
            .fetch_one(&self.pool)
            .await
            .unwrap();
        n
    }

    async fn fact(&self, fact: Fact, char_id: i32) -> i64 {
        self.settle().await;
        match fact {
            Fact::RaidRespawnTime => {
                self.count(
                    "SELECT CAST(respawn_time AS SIGNED) FROM raidboss_spawnlist WHERE boss_id = ?",
                    m8_data::RAID_ID,
                )
                .await
            }
            Fact::RaidPoints => {
                self.count(
                    "SELECT CAST(IFNULL(SUM(points), 0) AS SIGNED) FROM character_raid_points WHERE charId = ?",
                    char_id,
                )
                .await
            }
            Fact::ItemAuctionNewRows => {
                self.count(
                    "SELECT COUNT(*) FROM item_auction WHERE auctionId <> ?",
                    m8_data::AUCTION_ID,
                )
                .await
            }
            Fact::ItemAuctionBids => self.count("SELECT COUNT(*) FROM item_auction_bid WHERE auctionId = ?", m8_data::AUCTION_ID).await,
            Fact::SsJoined => {
                self.count(
                    "SELECT COUNT(*) FROM seven_signs WHERE charId = ? AND cabal = 'dawn' AND seal = 1",
                    char_id,
                )
                .await
            }
            Fact::SsContribution => {
                self.count(
                    "SELECT CAST(IFNULL(MAX(contribution_score), 0) AS SIGNED) FROM seven_signs WHERE charId = ?",
                    char_id,
                )
                .await
            }
        }
    }
}

/// One rig at a time: every real-server test starts a login server, a game server and a
/// database, and the tests of this file run side by side.
static ONE_RIG_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The scenario against the real servers.
struct Net {
    rig: Rig,
    id: i32,
    opts: Opts,
    client: Option<FakeGameClient>,
    _turn: tokio::sync::MutexGuard<'static, ()>,
}

impl Net {
    async fn start(tag: &str, opts: &Opts) -> Option<Self> {
        let turn = ONE_RIG_AT_A_TIME.lock().await;
        let seed = Seed::new();
        let rig = Rig::start(tag, &seed, opts).await?;
        let id = rig.create_character_and_leave(ACCOUNT_GM, NAME_GM).await;
        // A GM of level 10 who stands where the scenario wants him (with the HP, MP and CP of the
        // level: the numbers of `m6_data.rs`), in the class of the scenario.
        let class = if opts.warrior { m8_data::CLASS_WARRIOR } else { 0 };
        sqlx::query(
            "UPDATE characters SET level = ?, exp = ?, x = ?, y = ?, z = ?, classid = ?, base_class = ?, accesslevel = ?, \
             maxHp = 192, curHp = 192, maxMp = 82, curMp = 82, maxCp = 77, curCp = 77 WHERE charId = ?",
        )
        .bind(m6_data::LEADER_LEVEL)
        .bind(m6_data::LEADER_EXP)
        .bind(opts.position.0)
        .bind(opts.position.1)
        .bind(opts.position.2)
        .bind(class)
        .bind(class)
        .bind(m8_data::GM_LEVEL)
        .bind(id)
        .execute(&rig.pool)
        .await
        .unwrap();
        Some(Self {
            rig,
            id,
            opts: *opts,
            client: None,
            _turn: turn,
        })
    }

    fn client(&mut self) -> &mut FakeGameClient {
        self.client.as_mut().expect("the client is in the world")
    }

    async fn finish(self) {
        self.rig.gs.shutdown(false).await;
        self.rig.ls.shutdown(false).await;
        self.rig.db.drop_db().await;
    }
}

impl Harness for Net {
    fn object_id(&self) -> i32 {
        self.id
    }

    fn start_position(&self) -> (i32, i32, i32) {
        self.opts.position
    }

    async fn enter(&mut self) -> Vec<Vec<u8>> {
        let (mut game, list, play) = self.rig.login(ACCOUNT_GM, 1).await;
        assert_eq!(list.characters[0].object_id, self.id);
        let selected = game.select_character(0).await;
        assert_eq!(selected.session_id, play.play_ok1);
        game.send_packet(&enter_world_packet()).await;
        let packets = game.read_until("SKILL_LIST").await;
        self.client = Some(game);
        packets
    }

    async fn act(&mut self, act: Act) {
        self.client().send(act.wire).await;
    }

    async fn collect_until(&mut self, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        let client = self.client();
        let mut batch = Vec::new();
        loop {
            match client.recv_within(Duration::from_secs(40)).await {
                Recv::Packet(body) => {
                    let done = stop(&body);
                    batch.push(body);
                    if done {
                        break;
                    }
                }
                Recv::Closed => panic!("the game server closed the connection"),
                Recv::Silent => panic!("the event did not happen in 40 s; got {:?}", names(&batch)),
            }
        }
        batch.extend(client.read_quiet(Duration::from_millis(600)).await);
        batch
    }

    async fn collect_quiet(&mut self) -> Vec<Vec<u8>> {
        self.client().read_quiet(Duration::from_millis(500)).await
    }

    async fn leave(&mut self) {
        let mut game = self.client.take().expect("the client is in the world");
        let goodbye = game.logout().await;
        assert_eq!(
            goodbye.last(),
            Some(&LeaveWorld.to_bytes()),
            "{:?}",
            names(&goodbye)
        );
        // The login server hears about the logout only after the character is stored: when it
        // no longer counts the account, the rows are already final.
        self.rig.wait_until_out_of_game(ACCOUNT_GM).await;
    }

    async fn fact(&mut self, fact: Fact) -> i64 {
        self.rig.fact(fact, self.id).await
    }
}

// ------------------------------------------------------------------------------------------
// The scenarios
// ------------------------------------------------------------------------------------------

fn is_named(body: &[u8], name: &str) -> bool {
    packet_name(body) == name
}

/// An instance made from a template, entered with `//setinstance` and destroyed: the player is
/// put back at the exit point of the template.
async fn instance<H: Harness>(h: &mut H) {
    h.enter().await;
    let me = h.object_id();
    let at = h.start_position();
    let (id, template) = (m8_data::INSTANCE_ID, m8_data::INSTANCE_TEMPLATE);

    // A word without the right numbers is explained; a template makes the instance.
    h.act(act::admin("createinstance 1")).await;
    assert_eq!(
        texts(&h.collect_quiet().await),
        ["Example: //createinstance <id> <templateId> - ids => 300000 are reserved for dynamic instances"]
    );
    h.act(act::admin(&format!("createinstance {id} {template}")))
        .await;
    assert_eq!(texts(&h.collect_quiet().await), ["Instance created."]);

    // `//setinstance` moves the target: the GM targets himself first.
    h.act(act::click(me, at)).await;
    h.collect_quiet().await;
    h.act(act::admin(&format!("setinstance {id}"))).await;
    let moved = h.collect_quiet().await;
    assert_eq!(
        texts(&moved),
        [
            format!("Admin set your instance to:{id}"),
            format!("Moved {NAME_GM} to instance {id}.")
        ]
    );
    let teleport = all::<TeleportToLocation>(&moved, "TELEPORT_TO_LOCATION");
    assert_eq!(teleport.len(), 1, "{:?}", names(&moved));
    assert_eq!(
        (teleport[0].object_id, teleport[0].x, teleport[0].y),
        (me, at.0, at.1)
    );
    h.act(act::appearing()).await;
    h.collect_quiet().await;

    // Destroying the instance puts everybody inside out at the exit point.
    h.act(act::admin(&format!("destroyinstance {id}"))).await;
    let destroyed = h.collect_quiet().await;
    assert_eq!(texts(&destroyed), ["Instance destroyed"]);
    let out = all::<TeleportToLocation>(&destroyed, "TELEPORT_TO_LOCATION");
    assert_eq!(out.len(), 1, "{:?}", names(&destroyed));
    let exit = m8_data::INSTANCE_EXIT;
    // The exit is a teleport with a random offset (Java `teleToLocation(.., randomOffset)`).
    assert_eq!(out[0].object_id, me);
    assert!(
        (out[0].x - exit.0).abs() <= 100 && (out[0].y - exit.1).abs() <= 100,
        "{:?} is not near the exit {exit:?}",
        (out[0].x, out[0].y)
    );
    h.act(act::appearing()).await;
    h.collect_quiet().await;

    h.act(act::admin(&format!("setinstance {id}"))).await;
    assert_eq!(
        texts(&h.collect_quiet().await),
        [format!("Instance {id} does not exist.")]
    );
    h.leave().await;
}

/// The GM kills the raid boss: the kill message, the raid points of the killer (written whatever
/// the switch says), and the respawn time, a global row that only `CycleDatabaseWrites` writes.
async fn raid_boss<H: Harness>(h: &mut H, cycle_writes: bool) {
    let entered = h.enter().await;
    let at = h.start_position();
    let boss = npc_object(&entered, m8_data::RAID_ID);
    assert_eq!(
        h.fact(Fact::RaidRespawnTime).await,
        0,
        "the boss of the seed is alive"
    );

    h.act(act::click(boss, at)).await;
    h.collect_quiet().await;
    let before = now_ms();
    h.act(act::admin("kill")).await;
    let killed = h.collect_quiet().await;
    assert!(
        has_message(&killed, SystemMessageId::CongratulationsYourRaidWasSuccessful),
        "{:?} {:?}",
        message_ids(&killed),
        names(&killed)
    );
    assert!(names(&killed).contains(&"DIE".to_owned()), "{:?}", names(&killed));

    // `RaidBossPointsManager.addPoints`: level / 2 + Rnd.get(-5, 5) (35 at level 70), written
    // for the killer in both modes.
    let points = h.fact(Fact::RaidPoints).await;
    assert!((30..=40).contains(&points), "{points}");
    // `RaidBossSpawnManager.updateStatus`: the respawn time is between a half and one and a
    // half days from now, in the row only with the switch on (D-53).
    let respawn = h.fact(Fact::RaidRespawnTime).await;
    if cycle_writes {
        assert!(
            respawn >= before + 43_200_000 && respawn <= now_ms() + 129_600_000,
            "{respawn}"
        );
    } else {
        assert_eq!(respawn, 0, "the Java game server owns the row");
    }
    h.leave().await;
}

/// An item auction: the GM views it, then bids. The auction of the seed is the Java game
/// server's; with the switch off the world reads the rows before it shows them and refuses a bid
/// (D-50, D-53).
async fn item_auction<H: Harness>(h: &mut H, cycle_writes: bool) {
    h.enter().await;
    let instance = m8_data::AUCTION_INSTANCE;

    h.act(act::auction_info(instance)).await;
    let shown = h
        .collect_until(&|body| is_named(body, "EX_ITEM_AUCTION_INFO"))
        .await;
    let info = &all::<ExItemAuctionInfo>(&shown, "EX_ITEM_AUCTION_INFO")[0];
    assert_eq!(info.instance_id, instance);
    // Nobody bid: the shown bid is the initial one; an hour long, a minute gone.
    assert_eq!(info.bid, m8_data::AUCTION_INIT_BID);
    assert!(
        (3_500..=3_600).contains(&info.time_remaining),
        "{}",
        info.time_remaining
    );
    assert_eq!(info.item.item_id, m8_data::AUCTION_ITEM);
    assert_eq!(info.item.count, 1);

    // With the switch on the world makes the next auction itself (a global row); off it makes none.
    let new_rows = h.fact(Fact::ItemAuctionNewRows).await;
    assert_eq!(new_rows, i64::from(cycle_writes));

    h.act(act::auction_bid(instance, m8_data::AUCTION_INIT_BID + 50))
        .await;
    let bid = h.collect_quiet().await;
    if cycle_writes {
        assert!(
            has_message(&bid, SystemMessageId::YouHaveSubmittedABidForTheAuctionOfS1),
            "{:?}",
            message_ids(&bid)
        );
        assert_eq!(h.fact(Fact::ItemAuctionBids).await, 1);
    } else {
        assert_eq!(texts(&bid), ["This feature is disabled."]);
        assert_eq!(h.fact(Fact::ItemAuctionBids).await, 0);
    }
    h.leave().await;
}

/// The Seven Signs: a Warrior joins the Lords of Dawn at the priest (the row of the member is a
/// per-player write, so it is written in both modes), pays in seal stones and reads the status
/// page. The contribution is stored with the character (`AltSevenSignsLazyUpdate`).
async fn seven_signs<H: Harness>(h: &mut H) {
    let entered = h.enter().await;
    let at = h.start_position();
    let priest = npc_object(&entered, m8_data::PRIEST_ID);

    // The stones the GM creates for himself first (the page of the create word would replace the
    // priest's page, and a bypass is only taken from the page the player saw): ten red and two
    // blue are 10 * 10 + 2 * 3 = 106 points.
    h.act(act::admin(&format!(
        "create_item {} {}",
        m8_data::RED_STONE,
        m8_data::RED_COUNT
    )))
    .await;
    h.collect_quiet().await;
    h.act(act::admin(&format!(
        "create_item {} {}",
        m8_data::BLUE_STONE,
        m8_data::BLUE_COUNT
    )))
    .await;
    h.collect_quiet().await;

    // Two clicks on the priest: select, then talk. The first page is the one for a competition
    // period without a cabal.
    for _ in 0..2 {
        h.act(act::click(priest, at)).await;
    }
    let talk = h.collect_quiet().await;
    assert!(
        pages(&talk).iter().any(|p| p.contains("dawn_priest_1a")),
        "{:?} {:?}",
        pages(&talk),
        names(&talk)
    );

    h.act(act::bypass(&format!("npc_{priest}_SevenSigns 4 2 1")))
        .await;
    let joined = h.collect_quiet().await;
    let ids = message_ids(&joined);
    let at_dawn = ids
        .iter()
        .position(|m| *m == SystemMessageId::YouWillParticipateInTheSevenSignsAsAMemberOfTheLordsOfDawn)
        .unwrap_or_else(|| panic!("{ids:?} {:?}", pages(&joined)));
    assert_eq!(
        ids[at_dawn + 1],
        SystemMessageId::YouVeChosenToFightForTheSealOfAvariceDuringThisQuestEventPeriod
    );
    assert!(pages(&joined).iter().any(|p| p.contains("signs_4_dawn")));
    assert_eq!(h.fact(Fact::SsJoined).await, 1);

    // The contribution: every stone that fits under the limit, red first.
    h.act(act::bypass(&format!("npc_{priest}_SevenSigns 6 4"))).await;
    let paid = h.collect_quiet().await;
    let gained = message_of(&paid, SystemMessageId::YourContributionScoreHasIncreasedByS1);
    assert_eq!(
        gained.params,
        vec![SystemMessageParam::LongNumber(m8_data::CONTRIBUTION)]
    );
    assert!(
        pages(&paid).iter().any(|p| p.contains("signs_6_dawn")),
        "{:?}",
        pages(&paid)
    );

    // The status page (D-50: with the switch off it is read from the rows first).
    h.act(act::ssq_status(1)).await;
    let status = h.collect_until(&|body| is_named(body, "SSQ_STATUS")).await;
    let page = &all::<SsqStatus>(&status, "SSQ_STATUS")[0];
    assert_eq!((page.page, i32::from(page.period)), (1, PERIOD_COMPETITION));

    // With the lazy update the member's row is written when the character is stored.
    h.leave().await;
    assert_eq!(h.fact(Fact::SsContribution).await, m8_data::CONTRIBUTION);
}

/// A boat: the GM stands at the Talking Island dock, boards the ferry and gets off again.
async fn boat<H: Harness>(h: &mut H) {
    let entered = h.enter().await;
    let me = h.object_id();
    let boat = all::<VehicleInfo>(&entered, "VEHICLE_INFO")
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no boat in view: {:?}", names(&entered)))
        .object_id;

    h.act(act::get_on(boat, (10, 20, 30))).await;
    let on = h.collect_quiet().await;
    let got_on = &all::<GetOnVehicle>(&on, "GET_ON_VEHICLE")[0];
    assert_eq!(
        (
            got_on.object_id,
            got_on.boat_object_id,
            got_on.x,
            got_on.y,
            got_on.z
        ),
        (me, boat, 10, 20, 30)
    );

    let quay = (
        m8_data::TALKING_DOCK.0 + 100,
        m8_data::TALKING_DOCK.1,
        m8_data::TALKING_DOCK.2,
    );
    h.act(act::get_off(boat, quay)).await;
    let off = h.collect_quiet().await;
    let got_off = &all::<GetOffVehicle>(&off, "GET_OFF_VEHICLE")[0];
    assert_eq!(
        (got_off.object_id, got_off.boat_object_id, got_off.x),
        (me, boat, quay.0)
    );
    h.leave().await;
}

// ------------------------------------------------------------------------------------------
// The tests
// ------------------------------------------------------------------------------------------

/// Every action's bytes are a packet the server decodes in game, to the packet the action stands
/// for and without bytes left over. (The bare world never sees the bytes and the real servers run
/// only with a database, so this is what keeps the two halves of an `Act` together.)
#[test]
fn every_act_decodes_on_the_wire() {
    use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

    let at = m3_data::START;
    let cases: Vec<(Act, &str)> = vec![
        (act::click(1, at), "Action"),
        (act::bypass("npc_1_list"), "RequestBypassToServer"),
        (act::admin("kill"), "SendBypassBuildCmd"),
        (act::appearing(), "Appearing"),
        (act::ssq_status(1), "RequestSsqStatus"),
        (act::auction_info(1), "RequestInfoItemAuction"),
        (act::auction_bid(1, 5), "RequestBidItemAuction"),
        (act::get_on(1, at), "RequestGetOnVehicle"),
        (act::get_off(1, at), "RequestGetOffVehicle"),
    ];
    for (act, expected) in cases {
        let mut reader = PacketReader::new(&act.wire[1..]);
        let packet = decode_client(ConnectionState::InGame, act.wire[0], &mut reader)
            .unwrap_or_else(|e| panic!("{expected}: {e:?}"))
            .unwrap_or_else(|| panic!("{expected} is not accepted in game"));
        assert_eq!(reader.remaining(), 0, "{expected} leaves bytes unread");
        assert!(
            !matches!(packet, ClientPacket::Unported(_) | ClientPacket::UnportedEx(_)),
            "{expected} is not ported: {packet:?}"
        );
        let debug = format!("{packet:?}");
        assert!(
            debug.starts_with(&format!("{expected}(")) || debug == expected,
            "{expected}: {debug}"
        );
    }
}

/// What the real-server variants rely on and cannot show without a database: the statements of
/// the seed are made for the tables of the game schema (every column exists).
#[test]
fn the_seed_statements_name_columns_the_schema_has() {
    let seed = Seed::new();
    let schema: String = GAME_SCHEMA.concat();
    for statement in seed.startup_sql() {
        let table = statement
            .split_whitespace()
            .skip(1)
            .find(|word| {
                let word = word.trim_matches('`');
                word != "INTO" && word != "SET" && !word.is_empty()
            })
            .map(|w| w.trim_matches('`'))
            .unwrap();
        assert!(
            schema.contains(&format!("CREATE TABLE IF NOT EXISTS `{table}`")),
            "no table {table} for {statement}"
        );
        if statement.starts_with("INSERT") {
            let list = statement.split('(').nth(1).unwrap().split(')').next().unwrap();
            for column in list.split(',').map(|c| c.trim().trim_matches('`')) {
                assert!(
                    schema.contains(&format!("`{column}`")),
                    "no column {column} in {table} (statement {statement})"
                );
            }
        }
    }
}

/// Ticks until the stadium's task is in `wanted` (at most `max_secs`), sending the client's
/// `Appearing` after every teleport so the zones see the players arrive.
fn run_until(w: &mut BareWorld, wanted: GameState, max_secs: usize, players: &[usize]) {
    for _ in 0..max_secs * 20 {
        if w.world.olympiad().stadiums()[0].task.state == wanted {
            return;
        }
        w.ticks(1);
        for &index in players {
            if w.world.is_teleporting(ObjectId(BareWorld::id_of(index))) {
                w.input_of(index, ClientInput::Appearing);
            }
        }
    }
    panic!(
        "the task did not reach {wanted:?}, it is in {:?}",
        w.world.olympiad().stadiums()[0].task.state
    );
}

fn olympiad_writes(w: &BareWorld) -> Vec<OlympiadWrite> {
    w.persisted
        .commands()
        .into_iter()
        .filter_map(|c| match c {
            PersistCommand::Olympiad(write) => Some(write),
            _ => None,
        })
        .collect()
}

fn adena_of(w: &BareWorld, id: i32) -> i64 {
    w.world
        .player(ObjectId(id))
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == ADENA_ID && i.loc == "INVENTORY")
        .map(|i| i.count)
        .sum()
}

/// A classed Olympiad match from the registration to the points: two nobles register, the game
/// manager takes them to the stadium's instance, one falls, the points and the reward go to the
/// nobles, and the match row is written. The rows of the global cycle (the nobles, the status)
/// are written only with `CycleDatabaseWrites` (D-53). Bare only: see the module comment.
fn olympiad_match(cycle_writes: bool) {
    let mut w = BareWorld::start(&Opts::with_olympiad(cycle_writes), 3, &[]);
    let (a, b) = (BareWorld::id_of(1), BareWorld::id_of(2));
    w.enter_player(1);
    w.enter_player(2);
    w.ticks(2);
    w.log.take(a);
    w.log.take(b);
    let home = w.world.position_of(ObjectId(a)).unwrap();

    assert!(
        w.world
            .olympiad_register_noble(ObjectId(a), CompetitionType::Classed)
    );
    assert!(
        w.world
            .olympiad_register_noble(ObjectId(b), CompetitionType::Classed)
    );
    assert_eq!(
        message_ids(&w.log.take(a)),
        [SystemMessageId::YouHaveBeenRegisteredForTheGrandOlympiadWaitingListForAClassSpecificMatch]
    );
    assert_eq!(w.world.olympiad().registered_classed()[&m8_data::DUELIST], [a, b]);

    // The game manager runs 30 s after the start: the two leave the list for stadium 0.
    w.ticks(31 * 20);
    assert!(w.world.olympiad().registered_classed()[&m8_data::DUELIST].is_empty());
    let game = w.world.olympiad().stadiums()[0].task.game.clone().unwrap();
    assert_eq!(game.kind, CompetitionType::Classed);
    // Both are drawn at random (the world RNG) for side 1 and side 2.
    let (one, two) = (game.team_one[0].object_id, game.team_two[0].object_id);
    assert_eq!([one.min(two), one.max(two)], [a, b]);

    // The countdown, then the teleport into the stadium's instance.
    run_until(&mut w, GameState::BattleCountdownFirst, 10, &[1, 2]);
    let instance = w.world.olympiad().stadiums()[0].instance_id;
    for who in [one, two] {
        assert_eq!(w.world.instance_id_of(ObjectId(who)), instance);
        assert!(w.world.is_in_olympiad_mode(ObjectId(who)));
    }

    // The battle starts after the 60 + 10 s countdown; the side that stays alive wins.
    run_until(&mut w, GameState::BattleInProgress, 80, &[1, 2]);
    assert!(w.world.is_olympiad_start(ObjectId(one)));
    w.log.take(one);
    w.world
        .reduce_current_hp(ObjectId(two), 1_000_000.0, Some(ObjectId(one)));
    assert!(w.world.is_dead(ObjectId(two)));
    run_until(&mut w, GameState::TeleportToTown, 3, &[1, 2]);
    let ids = message_ids(&w.log.take(one));
    assert!(
        ids.contains(&SystemMessageId::CongratulationsC1YouWinTheMatch),
        "{ids:?}"
    );
    assert!(ids.contains(&SystemMessageId::YouHaveEarnedS2S1S), "{ids:?}");

    // `pointDiff = min(10, 10) / OlympiadDividerClassed (5)`.
    let winner = w.world.olympiad().noble(one).unwrap().clone();
    let loser = w.world.olympiad().noble(two).unwrap().clone();
    assert_eq!((winner.points, winner.comp_done, winner.comp_won), (12, 1, 1));
    assert_eq!((loser.points, loser.comp_done, loser.comp_lost), (8, 1, 1));
    assert_eq!(adena_of(&w, one), m6_data::START_ADENA + 50);
    assert_eq!(adena_of(&w, two), m6_data::START_ADENA);

    // The match row is the match of this server's players: written whatever the switch says.
    let fights: Vec<_> = olympiad_writes(&w)
        .into_iter()
        .filter_map(|write| match write {
            OlympiadWrite::Fight(fight) => Some(fight),
            _ => None,
        })
        .collect();
    assert_eq!(fights.len(), 1);
    assert_eq!((fights[0].char_one_id, fights[0].char_two_id), (one, two));
    assert_eq!(
        (
            fights[0].char_one_class,
            fights[0].char_two_class,
            fights[0].classed
        ),
        (m8_data::DUELIST, m8_data::DUELIST, true)
    );
    // The nobles and the cycle are global rows: nothing yet, and only with the switch on after
    // the save of the status (`Olympiad.saveOlympiadStatus`, also the shutdown's).
    assert_eq!(olympiad_writes(&w).len(), 1);
    w.world.save_olympiad_status();
    let saved: Vec<_> = olympiad_writes(&w)
        .into_iter()
        .filter(|write| !matches!(write, OlympiadWrite::Fight(_)))
        .collect();
    if cycle_writes {
        match saved.as_slice() {
            [OlympiadWrite::SaveNobles(nobles), OlympiadWrite::SaveData(data)] => {
                assert_eq!(nobles.len(), 2);
                assert_eq!((data.current_cycle, data.period), (1, 0));
            }
            other => panic!("{other:?}"),
        }
    } else {
        assert!(saved.is_empty(), "{saved:?}");
    }

    // 40 s later both are back where they stood, out of the olympiad mode.
    run_until(&mut w, GameState::Idle, 45, &[1, 2]);
    for who in [a, b] {
        assert_eq!(w.world.instance_id_of(ObjectId(who)), 0);
        assert!(!w.world.is_in_olympiad_mode(ObjectId(who)));
    }
    let (x, y, _) = w.world.position_of(ObjectId(a)).unwrap();
    assert_eq!((x, y), (home.0, home.1));
}

#[test]
fn bare_olympiad_match_with_cycle_writes_off() {
    olympiad_match(false);
}

#[test]
fn bare_olympiad_match_with_cycle_writes_on() {
    olympiad_match(true);
}

/// A whole leg of the ferry from Talking Island to Gludin: the ticket is paid when the boat
/// leaves, the passenger sails with the boat, and gets off at the other quay. 25 minutes of
/// game time, so the bare world only.
#[test]
fn bare_boat_leg() {
    let mut w = BareWorld::start(&Opts::at_dock(false), 1, &[(m8_data::BOAT_TICKET, 1)]);
    let me = ObjectId(BareWorld::id_of(0));
    let entered = w.enter_player(0);
    let boat = all::<VehicleInfo>(&entered, "VEHICLE_INFO")[0].object_id;
    let tickets = |w: &BareWorld| -> i64 {
        w.world
            .player(me)
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == m8_data::BOAT_TICKET)
            .map(|i| i.count)
            .sum()
    };
    w.input_of(0, act::get_on(boat, (1, 2, 3)).input);
    assert_eq!(w.world.vehicle_of(me), Some(ObjectId(boat)));
    w.log.take(me.0);

    // 3 minutes, then 5 + 40 s + 20 s of announcements: the boat leaves after 480 s (9600 ticks).
    w.ticks(9_590);
    assert_eq!(tickets(&w), 1);
    w.ticks(30);
    // `payForRide(1074, 1, ...)`: the ticket is taken and the passenger rides.
    assert_eq!(tickets(&w), 0);
    assert_eq!(w.world.vehicle_of(me), Some(ObjectId(boat)));
    let started = w.log.take(me.0);
    assert!(
        names(&started).contains(&"VEHICLE_DEPARTURE".to_owned()),
        "{:?}",
        names(&started)
    );

    // The trip takes 919 s; the Gludin dock is free, the boat comes to rest there.
    w.ticks(22_000);
    let dock = (-95_686, 150_514, -3_610);
    let ship = w.world.vehicles().vehicle(ObjectId(boat)).unwrap();
    assert_eq!((ship.x, ship.y, ship.z), dock);
    assert!(!ship.is_moving());
    assert_eq!(w.world.position_of(me), Some(dock));

    // Get off at the quay.
    let quay = (-95_686, 150_900, -3_610);
    w.input_of(0, act::get_off(boat, quay).input);
    assert_eq!(w.world.vehicle_of(me), None);
    assert_eq!(w.world.position_of(me), Some(quay));
}

macro_rules! scenario_tests {
    ($($name:ident, $net:ident, $opts:ident, |$h:ident, $cycle:ident| $body:expr;)*) => {
        $(
            #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
            async fn $name() {
                for $cycle in [false, true] {
                    let mut $h = BareWorld::start(&Opts::$opts($cycle), 1, &[]);
                    $body;
                }
            }

            #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
            async fn $net() {
                for $cycle in [false, true] {
                    let tag = format!("e2em8_{}_{}", stringify!($name), u8::from($cycle));
                    let Some(mut $h) = Net::start(&tag, &Opts::$opts($cycle)).await else {
                        println!("L2_TEST_DATABASE_URL is not set: skipping {}", stringify!($net));
                        return;
                    };
                    $body;
                    $h.finish().await;
                }
            }
        )*
    };
}

scenario_tests! {
    bare_instance, net_instance, at_start, |h, _cycle| instance(&mut h).await;
    bare_raid_boss, net_raid_boss, at_start, |h, cycle| raid_boss(&mut h, cycle).await;
    bare_item_auction, net_item_auction, at_start, |h, cycle| item_auction(&mut h, cycle).await;
    bare_seven_signs, net_seven_signs, at_start, |h, _cycle| seven_signs(&mut h).await;
    bare_boat, net_boat, at_dock, |h, _cycle| boat(&mut h).await;
}
