#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End to end for the skills and consumables of M5: a caster and a watcher log in through the
//! in-process login server and game server. The caster buffs itself (the watcher sees the cast and
//! the new speed), nukes a target dummy, drinks a mana potion, charges the sword with a soulshot and
//! hits (then lets the automatic shots do it), logs out and in again with the buff and its reuse
//! timer still on, and logs out once more; the rows are what Java would write.
//!
//! The scenario is written once (`play`) against a `Harness` and runs twice, like `e2e_m4.rs`:
//!
//! * `Net`: the real thing. The Rust login server and the Rust game server in one process, a
//!   throwaway MariaDB, two `FakeGameClient`s on sockets. It runs only when
//!   `L2_TEST_DATABASE_URL` is set (CI's MariaDB job; never point it at a database with real
//!   data) and checks the rows the game server wrote (`character_skills_save`, `character_skills`,
//!   `items`). Both servers listen on port 0 on loopback. The game server seeds its world RNG from
//!   the system, so nothing here depends on a roll: the dummy has far more HP than the scenario can
//!   take, and what is checked about the shots does not depend on a hit or a miss.
//! * `BareWorld`: the same datapack, configuration, geodata, zones and spawns loaded the way the
//!   game server loads them, but with the world driven directly (no sockets, no database, ticks
//!   instead of time) and a seeded RNG. It always runs. A logout hands the stored buffs to the
//!   next login the way the database loader does (`effects_to_store` -> `effects_to_restore`).
//!
//! There is no datapack and no geodata in CI: `e2e/datapack.rs`, `e2e/m3_data.rs`, `e2e/m4_data.rs`
//! and `e2e/m5_data.rs` write them into a temporary folder. The skills and items carry the ids and
//! the shape of the shipped ones (see `m5_data.rs` for the one number that differs).
//!
//! Left out on purpose: summons and parties (M6).

#[path = "e2e/datapack.rs"]
mod datapack;
#[allow(dead_code)]
#[path = "e2e/m3_data.rs"]
mod m3_data;
#[allow(dead_code)]
#[path = "e2e/m4_data.rs"]
mod m4_data;
#[path = "e2e/m5_data.rs"]
mod m5_data;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_server::client::newchar::{CreateRequest, IdSource, ItemMaxMp, PlanContext, StatBonus, build_plan};
use game_server::world_data::{DatapackData, regen_settings, skill_load_options, world_settings};
use game_server::{Paths, Server, geo, hexid, npcs};
use l2_config::GameConfig;
use l2_data::{Datapack, DatapackOptions};
use l2_db::{ItemLocation, Pool};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{
    AbnormalStatusUpdate, Attack, CharInfo, DeleteObject, ExAutoSoulShot, HITFLAG_USESS, InventoryUpdate,
    ItemList, LeaveWorld, MagicSkillUse, NpcInfo, SkillCoolTime, SkillList, StatusUpdate, SystemMessage,
    UserInfo,
};
use l2_protocol_login::server::PlayOk;
use l2_testkit::fake_game_client::{
    FakeGameClient, Recv, character_create, enter_world_packet, names_of, packet_name,
};
use l2_testkit::fake_login_client::FakeClient as LoginClient;
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
use l2_world::{
    ClientInput, IdFactory, ItemData, LeaveReason, LsEvent, ObjectId, Outbox, PacketSink, PersistCommand,
    PlayerData, PlayerProfile, SessionId, SkillData, StaticData, World, WorldCommand, WorldRng,
};
use login_server::state::{LoginState, ServerStatus as ListStatus};

const ACCOUNT_A: &str = "e2em5acca";
const ACCOUNT_B: &str = "e2em5accb";
const PASSWORD: &str = "secret";
const NAME_A: &str = "Caster";
const NAME_B: &str = "Watcher";
/// A protocol revision the game server accepts by default.
const PROTOCOL: i32 = 273;
const START_ADENA: i64 = 1_000;
const ADENA_ID: i32 = 57;
const SOULSHOT_ID: i32 = 1835;
/// `StatusUpdate` attribute ids.
const CUR_HP: i32 = 0x09;
const CUR_MP: i32 = 0x0b;

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

/// Everything in the game server's `config` folder except `Database.ini`. Like the M4 rig (see
/// `e2e_m4.rs` for the reasons), and `StoreSkillCooltime` is left at its default (true): the buffs
/// and their reuse timers are stored at the logout.
fn write_game_config(config_dir: &Path, ls_port: u16) {
    std::fs::create_dir_all(config_dir).unwrap();
    std::fs::write(config_dir.join("Server.ini"), game_server_ini(ls_port)).unwrap();
    std::fs::write(
        config_dir.join("Player.ini"),
        format!(
            "ForbiddenNames = annou\nStartingAdena = {START_ADENA}\nDisconnectAfterDeath = False\nEnableVitality = False\n"
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
        datapack::no_flood_protection_ini(),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("ipconfig.xml"),
        "<gameserver address=\"127.0.0.1\" />\n",
    )
    .unwrap();
    hexid::save(config_dir, SAMPLE_SERVER_ID, SAMPLE_HEXID);
}

/// The datapack of this test: the base classes and items, the world data of M3, the hunting data
/// of M4 (the fighter's numbers, the exp tables) and the skills, items and dummy of M5.
fn write_datapack(root: &Path) {
    datapack::write_datapack(root);
    m3_data::write_world_data(root);
    m4_data::write_hunting_data(root);
    m5_data::write_skill_data(root);
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

/// The value the last `StatusUpdate` of `object_id` in `batch` gave for `attribute`.
fn last_status(batch: &[Vec<u8>], object_id: i32, attribute: i32) -> Option<i32> {
    all::<StatusUpdate>(batch, "STATUS_UPDATE")
        .iter()
        .filter(|u| u.object_id == object_id)
        .flat_map(|u| u.attributes.iter())
        .filter(|(id, _)| *id == attribute)
        .map(|(_, value)| *value)
        .next_back()
}

fn message_ids(batch: &[Vec<u8>]) -> Vec<SystemMessageId> {
    all::<SystemMessage>(batch, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|m| m.id)
        .collect()
}

/// The `MagicSkillUse` packets in `batch` as `(caster, target, skill id)`.
fn skill_uses(batch: &[Vec<u8>]) -> Vec<(i32, i32, i32)> {
    all::<MagicSkillUse>(batch, "MAGIC_SKILL_USE")
        .into_iter()
        .map(|m| (m.creature_id, m.target_id, m.skill_id))
        .collect()
}

/// The count of item `item_id` in the last `InventoryUpdate` of `batch` that mentions it.
fn counted_in_updates(batch: &[Vec<u8>], item_id: i32) -> Option<(i16, i64)> {
    all::<InventoryUpdate>(batch, "INVENTORY_UPDATE")
        .into_iter()
        .flat_map(|update| update.updates)
        .filter(|u| u.item.item_id == item_id)
        .map(|u| (u.change, u.item.count))
        .next_back()
}

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

/// One row of `character_skills_save` (or the `SavedEffect` the world gave the store).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stored {
    skill_id: i32,
    skill_level: i32,
    remaining_time: i32,
    reuse_delay: i64,
    /// The wall clock millisecond at which the reuse timer ends.
    systime: i64,
    restore_type: i32,
}

/// What the store of a character that left says.
#[derive(Debug, PartialEq, Eq)]
struct Left {
    /// `(skill id, level)` of the skills the character knows, sorted.
    skills: Vec<(i32, i32)>,
    /// Counts of the stacks the scenario uses: soulshots and mana potions.
    soulshots: i64,
    potions: i64,
    /// The saved buffs and reuse timers, in the order of the table.
    effects: Vec<Stored>,
}

/// The two ways to play the scenario. `collect_until` returns everything the player's client got
/// since the last call, up to the packet `stop` accepts and a moment after it (the rest of that
/// event); `collect_quiet` returns what arrives in a quiet moment.
trait Harness {
    fn object_id(&self, who: Who) -> i32;
    /// Logs in, selects and enters the world; returns every packet up to the closing `SKILL_LIST`.
    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>>;
    async fn click(&mut self, who: Who, object_id: i32);
    async fn use_skill(&mut self, who: Who, skill_id: i32);
    async fn use_item(&mut self, who: Who, object_id: i32);
    async fn auto_shot(&mut self, who: Who, item_id: i32, on: bool);
    async fn skill_cool_time(&mut self, who: Who);
    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>>;
    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>>;
    /// Logs out and returns what was stored.
    async fn leave(&mut self, who: Who) -> Left;
}

/// What the scenario saw, for the checks after it.
struct Played {
    /// What the caster's store said after the first logout and after the second.
    first_left: Left,
    second_left: Left,
    left_b: Left,
    /// The soulshots the client was last told it has (the stack the store must show).
    soulshots_seen: i64,
}

/// How fast the caster runs, as the client is told: the speed fields of `UserInfo` and `CharInfo`
/// are the base speeds (Java divides by `getMovementSpeedMultiplier()`), and the multiplier next
/// to them carries the buffs.
fn speed_in_user_info(batch: &[Vec<u8>]) -> f64 {
    all::<UserInfo>(batch, "USER_INFO")
        .last()
        .expect("a UserInfo")
        .move_multiplier
}

fn speed_of_a_in_char_info(batch: &[Vec<u8>], a: i32) -> f64 {
    all::<CharInfo>(batch, "CHAR_INFO")
        .iter()
        .rfind(|c| c.object_id == a)
        .expect("a CharInfo of the caster")
        .move_multiplier
}

async fn play<H: Harness>(h: &mut H) -> Played {
    let a = h.object_id(Who::A);

    // ---- both enter; the caster knows the skills and sees the dummy -------------------------------
    let entered_a = h.enter(Who::A).await;
    let skill_list: SkillList = all(&entered_a, "SKILL_LIST").remove(0);
    let known: Vec<(i32, i32)> = skill_list.skills.iter().map(|s| (s.id, s.level)).collect();
    for wanted in [m5_data::WIND_STRIKE, m5_data::WIND_WALK] {
        assert!(known.contains(&(wanted, 1)), "{wanted} is learned: {known:?}");
    }
    let dummy = all::<NpcInfo>(&entered_a, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == m5_data::DUMMY_ID)
        .expect("the dummy is in view")
        .object_id;
    let items: ItemList = all(&entered_a, "ITEM_LIST").remove(0);
    let object_of = |item_id: i32| {
        items
            .items
            .iter()
            .find(|i| i.item_id == item_id)
            .unwrap_or_else(|| panic!("item {item_id} in the pack: {items:?}"))
    };
    let (shots, potions) = (
        object_of(SOULSHOT_ID).object_id,
        object_of(m5_data::MANA_POTION).object_id,
    );
    assert_eq!(object_of(SOULSHOT_ID).count, m5_data::SOULSHOTS_AT_START);
    assert_eq!(object_of(m5_data::MANA_POTION).count, m5_data::POTIONS_AT_START);
    let speed_before = speed_in_user_info(&entered_a);
    // The stat engine makes the maximum MP more than the 30 of the template (the MEN bonus); the
    // bare character starts with 30, a created one with the maximum.
    let info = all::<UserInfo>(&entered_a, "USER_INFO").remove(0);
    let (max_mp, start_mp) = (info.max_mp, info.current_mp);
    assert!(start_mp >= 30 && start_mp <= max_mp, "{start_mp} of {max_mp}");

    let entered_b = h.enter(Who::B).await;
    let seen_by_b = speed_of_a_in_char_info(&entered_b, a);
    assert_eq!(seen_by_b, speed_before, "B sees the speed A has");
    h.collect_until(Who::A, &|body| is_named(body, "CHAR_INFO")).await;

    // ---- the buff: select oneself, cast Wind Walk, the icon and the speed ------------------------
    h.click(Who::A, a).await;
    let selecting = h
        .collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED"))
        .await;
    assert_in_order(&names(&selecting), &["MY_TARGET_SELECTED"]);
    h.use_skill(Who::A, m5_data::WIND_WALK).await;
    let buffing = h
        .collect_until(Who::A, &|body| is_named(body, "ABNORMAL_STATUS_UPDATE"))
        .await;
    let buffing_names = names(&buffing);
    assert_in_order(
        &buffing_names,
        &[
            "MAGIC_SKILL_USE",
            "MAGIC_SKILL_LAUNCHED",
            "ABNORMAL_STATUS_UPDATE",
        ],
    );
    assert_eq!(
        skill_uses(&buffing),
        [(a, a, m5_data::WIND_WALK)],
        "the cast is announced once: {buffing_names:?}"
    );
    assert!(
        message_ids(&buffing).contains(&SystemMessageId::UseS1),
        "You use Wind Walk."
    );
    let icons: AbnormalStatusUpdate = all(&buffing, "ABNORMAL_STATUS_UPDATE").remove(0);
    assert_eq!(icons.effects.len(), 1, "one icon: {icons:?}");
    let icon = icons.effects[0];
    assert_eq!((icon.display_id, icon.display_level), (m5_data::WIND_WALK, 1));
    assert!(
        (m5_data::WIND_WALK_SECONDS - 10..=m5_data::WIND_WALK_SECONDS).contains(&icon.time),
        "the icon shows the abnormal time: {}",
        icon.time
    );
    let boosted = speed_in_user_info(&buffing);
    assert!(
        boosted > speed_before,
        "{boosted} > {speed_before}: Wind Walk adds speed"
    );
    // MP: the initial 4 at the start and the 16 at the hit are paid (20 of the maximum); the regeneration
    // during the 4 s cast gives a little back.
    let mp_after_buff = last_status(&buffing, a, CUR_MP).expect("the MP changed");
    assert!(
        (start_mp - 20..=start_mp - 15).contains(&mp_after_buff),
        "MP after Wind Walk: {mp_after_buff}"
    );

    // The watcher sees the cast and then the caster's new speed.
    let watched = h
        .collect_until(Who::B, &|body| {
            is_named(body, "CHAR_INFO") && decode_any::<CharInfo>(body).move_multiplier > speed_before
        })
        .await;
    assert!(
        skill_uses(&watched).contains(&(a, a, m5_data::WIND_WALK)),
        "B saw the cast: {:?}",
        names(&watched)
    );
    assert_in_order(&names(&watched), &["MAGIC_SKILL_USE", "CHAR_INFO"]);
    assert_eq!(
        speed_of_a_in_char_info(&watched, a),
        boosted,
        "B sees the speed A has now"
    );
    let _ = h.collect_quiet(Who::A).await;

    // ---- the nuke: select the dummy, Wind Strike ------------------------------------------------------
    h.click(Who::A, dummy).await;
    let targeting = h
        .collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED"))
        .await;
    assert_in_order(&names(&targeting), &["MY_TARGET_SELECTED"]);
    // Selecting a monster tells its HP (the stat engine makes the maximum more than the template's).
    let hp_before = last_status(&targeting, dummy, CUR_HP).expect("the HP of the target");
    assert!(hp_before >= m5_data::DUMMY_HP, "{hp_before}");
    h.use_skill(Who::A, m5_data::WIND_STRIKE).await;
    let nuke = h
        .collect_until(Who::A, &|body| {
            is_named(body, "STATUS_UPDATE")
                && decode_any::<StatusUpdate>(body).object_id == dummy
                && decode_any::<StatusUpdate>(body)
                    .attributes
                    .iter()
                    .any(|(id, _)| *id == CUR_HP)
        })
        .await;
    let nuke_names = names(&nuke);
    assert_in_order(
        &nuke_names,
        &[
            "MAGIC_SKILL_USE",
            "MAGIC_SKILL_LAUNCHED",
            "SYSTEM_MESSAGE",
            "STATUS_UPDATE",
        ],
    );
    assert!(
        skill_uses(&nuke).contains(&(a, dummy, m5_data::WIND_STRIKE)),
        "the nuke goes at the dummy: {nuke_names:?}"
    );
    let dummy_hp = last_status(&nuke, dummy, CUR_HP).expect("the dummy's HP");
    assert!(
        dummy_hp < hp_before,
        "the nuke hurt the dummy: {dummy_hp} of {hp_before}"
    );
    // 2 MP at the start and 7 at the hit are paid (9), less what the regeneration gave back during
    // the cast.
    let mp_after_nuke = last_status(&nuke, a, CUR_MP).expect("the MP changed");
    assert!(
        (mp_after_buff - 9..=mp_after_buff - 4).contains(&mp_after_nuke),
        "MP after Wind Strike: {mp_after_nuke}, before: {mp_after_buff}"
    );
    let _ = h.collect_quiet(Who::B).await;

    // ---- the potion: the mana comes back, one potion less --------------------------------------------------
    h.use_item(Who::A, potions).await;
    let drinking = h
        .collect_until(Who::A, &|body| is_named(body, "INVENTORY_UPDATE"))
        .await;
    let drinking_names = names(&drinking);
    assert_eq!(
        counted_in_updates(&drinking, m5_data::MANA_POTION),
        Some((InventoryUpdate::MODIFIED, m5_data::POTIONS_AT_START - 1)),
        "one potion is gone: {drinking_names:?}"
    );
    assert_eq!(
        last_status(&drinking, a, CUR_MP),
        Some(max_mp),
        "the potion fills the MP: {drinking_names:?}"
    );
    assert!(
        message_ids(&drinking).contains(&SystemMessageId::S1MpHasBeenRestored),
        "{drinking_names:?}"
    );

    // ---- the soulshot: the sword is charged, one shot is used ---------------------------------------------
    h.use_item(Who::A, shots).await;
    let charging = h
        .collect_until(Who::A, &|body| is_named(body, "INVENTORY_UPDATE"))
        .await;
    let charging_names = names(&charging);
    assert!(
        message_ids(&charging).contains(&SystemMessageId::YourSoulshotsAreEnabled),
        "{charging_names:?}"
    );
    assert!(
        skill_uses(&charging).contains(&(a, a, m5_data::SOULSHOT_SKILL)),
        "the shot has its animation: {charging_names:?}"
    );
    let shots_after_manual = m5_data::SOULSHOTS_AT_START - m5_data::SOULSHOTS_PER_SHOT;
    assert_eq!(
        counted_in_updates(&charging, SOULSHOT_ID),
        Some((InventoryUpdate::MODIFIED, shots_after_manual))
    );
    // B sees the glow.
    let glow = h.collect_quiet(Who::B).await;
    assert!(
        skill_uses(&glow).contains(&(a, a, m5_data::SOULSHOT_SKILL)),
        "B saw the shot: {:?}",
        names(&glow)
    );

    // A nuke does not start the melee (the skill has no next action): the caster clicks the target
    // again, walks up to it and swings. The first blow after the charge uses it up.
    let glow_at = charging
        .iter()
        .position(|body| {
            is_named(body, "MAGIC_SKILL_USE")
                && decode_any::<MagicSkillUse>(body).skill_id == m5_data::SOULSHOT_SKILL
        })
        .expect("the glow is in the batch");
    let blows_by = |batch: &[Vec<u8>]| -> Vec<Attack> {
        all::<Attack>(batch, "ATTACK")
            .into_iter()
            .filter(|x| x.attacker_id == a)
            .collect()
    };
    h.click(Who::A, dummy).await;
    let mut after_charge = charging[glow_at..].to_vec();
    after_charge.extend(
        h.collect_until(Who::A, &|body| {
            is_named(body, "ATTACK") && decode_any::<Attack>(body).attacker_id == a
        })
        .await,
    );
    assert_in_order(&names(&after_charge), &["MOVE_TO_PAWN", "ATTACK"]);
    let first_blow = blows_by(&after_charge).remove(0);
    assert_ne!(
        first_blow.first_hit.flags & HITFLAG_USESS,
        0,
        "the first blow after the charge is a soulshot blow: {first_blow:?}"
    );

    // Automatic soulshots: on (the client is told, the sword is charged at once), every blow after
    // that is a soulshot blow; off (the client is told again).
    h.auto_shot(Who::A, SOULSHOT_ID, true).await;
    let blows_seen = std::cell::Cell::new(0);
    let marker_seen = std::cell::Cell::new(false);
    let automatic = h
        .collect_until(Who::A, &|body| {
            if is_named(body, "EX_AUTO_SOULSHOT") {
                marker_seen.set(true);
            }
            if marker_seen.get() && is_named(body, "ATTACK") && decode_any::<Attack>(body).attacker_id == a {
                blows_seen.set(blows_seen.get() + 1);
            }
            blows_seen.get() >= 3
        })
        .await;
    let on_packets: Vec<ExAutoSoulShot> = all(&automatic, "EX_AUTO_SOULSHOT");
    assert_eq!(
        on_packets
            .iter()
            .map(|p| (p.item_id, p.shot_type))
            .collect::<Vec<_>>(),
        [(SOULSHOT_ID, 1)],
        "the client is told the automatic use is on"
    );
    let marker = automatic
        .iter()
        .position(|body| is_named(body, "EX_AUTO_SOULSHOT"))
        .unwrap();
    let blows_after = blows_by(&automatic[marker..]);
    assert!(blows_after.len() >= 3, "{blows_after:?}");
    assert!(
        blows_after.iter().all(|x| x.first_hit.flags & HITFLAG_USESS != 0),
        "every blow after the automatic charge is a soulshot blow: {blows_after:?}"
    );
    h.auto_shot(Who::A, SOULSHOT_ID, false).await;
    let off = h.collect_quiet(Who::A).await;
    assert_eq!(
        all::<ExAutoSoulShot>(&off, "EX_AUTO_SOULSHOT")
            .iter()
            .map(|p| (p.item_id, p.shot_type))
            .collect::<Vec<_>>(),
        [(SOULSHOT_ID, 0)],
        "and told it is off"
    );
    let mut every = Vec::new();
    every.extend(charging);
    every.extend(after_charge);
    every.extend(automatic);
    every.extend(off);
    let (_, soulshots_seen) = counted_in_updates(&every, SOULSHOT_ID).expect("the soulshot stack changed");
    assert!(
        soulshots_seen < shots_after_manual,
        "the automatic shots used soulshots too: {soulshots_seen}"
    );

    // ---- the first logout: the buff and its reuse are stored ------------------------------------------------
    let first_left = h.leave(Who::A).await;
    let departed = h
        .collect_until(Who::B, &|body| {
            is_named(body, "DELETE_OBJECT") && decode_any::<DeleteObject>(body).object_id == a
        })
        .await;
    assert!(!departed.is_empty(), "B saw the caster go");

    // ---- the login: the buff is back with the time it had left, and so is the reuse ----------------------------
    let entered_again = h.enter(Who::A).await;
    // The buff is applied before the UserInfo of the enter is built, so it already shows the speed.
    assert_eq!(
        speed_in_user_info(&entered_again),
        boosted,
        "the restored buff is in the first UserInfo"
    );
    let restored = h
        .collect_until(Who::A, &|body| is_named(body, "ABNORMAL_STATUS_UPDATE"))
        .await;
    let icons: AbnormalStatusUpdate = all(&restored, "ABNORMAL_STATUS_UPDATE").remove(0);
    assert_eq!(icons.effects.len(), 1, "{icons:?}");
    let back = icons.effects[0];
    assert_eq!((back.display_id, back.display_level), (m5_data::WIND_WALK, 1));
    let stored_buff = first_left
        .effects
        .iter()
        .find(|e| e.skill_id == m5_data::WIND_WALK)
        .expect("the buff was stored");
    assert!(
        back.time <= stored_buff.remaining_time && back.time > stored_buff.remaining_time - 30,
        "the buff has the time it had left: {} of {}",
        back.time,
        stored_buff.remaining_time
    );
    // The speed shows again, to the caster and to the watcher.
    let _ = h.collect_quiet(Who::A).await;
    let seen_again = h
        .collect_until(Who::B, &|body| {
            is_named(body, "CHAR_INFO")
                && decode_any::<CharInfo>(body).object_id == a
                && decode_any::<CharInfo>(body).move_multiplier > speed_before
        })
        .await;
    assert!(
        seen_again.iter().any(|body| is_named(body, "CHAR_INFO")),
        "B sees the buffed caster again"
    );

    // The reuse timer survived too: Wind Walk is listed and cannot be cast again yet.
    h.skill_cool_time(Who::A).await;
    let cool = h
        .collect_until(Who::A, &|body| is_named(body, "SKILL_COOL_TIME"))
        .await;
    let cool: SkillCoolTime = all(&cool, "SKILL_COOL_TIME").remove(0);
    let entry = cool
        .entries
        .iter()
        .find(|e| e.skill_id == m5_data::WIND_WALK)
        .unwrap_or_else(|| panic!("Wind Walk is under reuse: {cool:?}"));
    assert_eq!(entry.reuse_seconds, (m5_data::WIND_WALK_REUSE_MS / 1000) as i32);
    assert!(
        (1..=entry.reuse_seconds).contains(&entry.remaining_seconds),
        "{entry:?}"
    );
    h.click(Who::A, a).await;
    h.collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED"))
        .await;
    h.use_skill(Who::A, m5_data::WIND_WALK).await;
    let refused = h.collect_quiet(Who::A).await;
    // Three minutes of reuse, a few seconds to a minute gone: the minutes variant of the message.
    assert!(
        message_ids(&refused).contains(&SystemMessageId::ThereAreS2MinuteSS3SecondSRemainingInS1SReUseTime),
        "{:?}",
        names(&refused)
    );
    assert!(skill_uses(&refused).is_empty(), "nothing was cast");

    // ---- both log out; what is stored is final -----------------------------------------------------------------------
    let second_left = h.leave(Who::A).await;
    let left_b = h.leave(Who::B).await;
    Played {
        first_left,
        second_left,
        left_b,
        soulshots_seen,
    }
}

fn assert_played(played: &Played) {
    let known = [(m5_data::WIND_STRIKE, 1), (m5_data::WIND_WALK, 1)];
    for left in [&played.first_left, &played.second_left] {
        for skill in &known {
            assert!(
                left.skills.contains(skill),
                "{skill:?} is stored: {:?}",
                left.skills
            );
        }
        // The stack the client was told about is the stack in the store.
        assert_eq!(left.soulshots, played.soulshots_seen);
        assert_eq!(left.potions, m5_data::POTIONS_AT_START - 1);
        // One row: the buff, with its reuse timer. Wind Strike's reuse (1.2 s) and the potion's
        // (0.5 s) are over, and a toggle-free character has nothing else on it.
        assert_eq!(left.effects.len(), 1, "{:?}", left.effects);
        let row = &left.effects[0];
        assert_eq!(
            (row.skill_id, row.skill_level, row.restore_type, row.reuse_delay),
            (m5_data::WIND_WALK, 1, 0, m5_data::WIND_WALK_REUSE_MS)
        );
        assert!(
            (1..m5_data::WIND_WALK_SECONDS).contains(&row.remaining_time),
            "the time the buff had left: {}",
            row.remaining_time
        );
    }
    let (first, second) = (&played.first_left.effects[0], &played.second_left.effects[0]);
    assert!(
        second.remaining_time <= first.remaining_time,
        "the buff went on counting down: {} then {}",
        first.remaining_time,
        second.remaining_time
    );
    assert!(
        second.systime >= first.systime - 5_000 && second.systime <= first.systime + 5_000,
        "it is the same reuse timer, not a new one: {} then {}",
        first.systime,
        second.systime
    );

    let b = &played.left_b;
    assert_eq!(b.soulshots, m5_data::SOULSHOTS_AT_START, "B used nothing");
    assert_eq!(b.potions, m5_data::POTIONS_AT_START);
    assert!(b.effects.is_empty(), "B has no buffs: {:?}", b.effects);
    assert!(b.skills.contains(&(m5_data::WIND_WALK, 1)));
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
struct StoredPlayers(Arc<Mutex<Vec<PlayerData>>>);

struct RecordingOutbox(StoredPlayers);

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
    stored: StoredPlayers,
    /// What the next login of each player loads (the database, in the real server).
    players: [PlayerData; 2],
    /// Keeps the folder of the datapack (nothing reads it after the start).
    _root: tempfile::TempDir,
}

/// Far above the ids the factory hands out to the NPCs (it counts up from `RUST_FIRST_OBJECT_ID`).
const BARE_IDS: [i32; 2] = [0x7000_0001, 0x7000_0002];
/// The longest the bare world waits for an event: 60 seconds of world time (50 ms ticks).
const BARE_LIMIT_TICKS: usize = 1_200;
/// The moment after an event that is collected with it: 600 ms.
const BARE_SETTLE_TICKS: usize = 12;

fn bare_player(index: usize) -> PlayerData {
    let (name, account) = [(NAME_A, ACCOUNT_A), (NAME_B, ACCOUNT_B)][index];
    let item = |object_id: i32, item_id: i32, count: i64, loc: &str, loc_data: i32| ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        enchant_level: 0,
        loc: loc.to_owned(),
        loc_data,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    };
    let slot = i32::try_from(index).unwrap();
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
        // What a new fighter owns that matters here: adena, the squire's sword in the hand, the
        // potions and the soulshots of `initialEquipment.xml`.
        items: vec![
            item(1_000 + slot, ADENA_ID, START_ADENA, "INVENTORY", 0),
            item(
                2_000 + slot,
                2369,
                1,
                "PAPERDOLL",
                i32::try_from(paperdoll::RHAND).unwrap(),
            ),
            item(
                3_000 + slot,
                m5_data::MANA_POTION,
                m5_data::POTIONS_AT_START,
                "INVENTORY",
                0,
            ),
            item(
                4_000 + slot,
                SOULSHOT_ID,
                m5_data::SOULSHOTS_AT_START,
                "INVENTORY",
                0,
            ),
        ],
        skills: [m5_data::WIND_STRIKE, m5_data::WIND_WALK]
            .into_iter()
            .map(|skill_id| SkillData {
                skill_id,
                skill_level: 1,
            })
            .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn count_of(data: &PlayerData, item_id: i32) -> i64 {
    data.items
        .iter()
        .filter(|i| i.item_id == item_id)
        .map(|i| i.count)
        .sum()
}

impl BareWorld {
    /// Loads the configuration, datapack, geodata, zones, html and spawns the way
    /// `Server::start_with` does, without the database and the sockets.
    fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        write_game_config(&config_dir, 4242);
        let data_root = root.path().join("datapack");
        write_datapack(&data_root);

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

        let stored = StoredPlayers::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(RecordingOutbox(stored.clone())));
        world.set_static_data(Box::new(static_data));
        world.set_settings(settings);
        world.set_regen_settings(regen_settings(&config));
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

    async fn click(&mut self, who: Who, object_id: i32) {
        self.input(
            who,
            ClientInput::Action {
                object_id,
                action_id: 0,
            },
        );
    }

    async fn use_skill(&mut self, who: Who, skill_id: i32) {
        self.input(
            who,
            ClientInput::MagicSkillUse {
                magic_id: skill_id,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        );
    }

    async fn use_item(&mut self, who: Who, object_id: i32) {
        self.input(
            who,
            ClientInput::UseItem {
                object_id,
                ctrl_pressed: false,
            },
        );
    }

    async fn auto_shot(&mut self, who: Who, item_id: i32, on: bool) {
        self.input(
            who,
            ClientInput::AutoSoulShot {
                item_id,
                shot_type: i32::from(on),
            },
        );
    }

    async fn skill_cool_time(&mut self, who: Who) {
        self.input(who, ClientInput::SkillCoolTime);
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
            self.world.tick();
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
            .stored
            .0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|data| data.object_id == ObjectId(id))
            .cloned()
            .expect("the character was stored");
        let effects: Vec<Stored> = data
            .profile
            .effects_to_store
            .clone()
            .expect("StoreSkillCooltime is on: the buffs are stored")
            .iter()
            .map(|e| Stored {
                skill_id: e.skill_id,
                skill_level: e.skill_level,
                remaining_time: e.remaining_time,
                reuse_delay: e.reuse_delay,
                systime: e.systime,
                restore_type: e.restore_type,
            })
            .collect();
        let mut skills: Vec<(i32, i32)> = data.skills.iter().map(|s| (s.skill_id, s.skill_level)).collect();
        skills.sort_unstable();
        let left = Left {
            skills,
            soulshots: count_of(&data, SOULSHOT_ID),
            potions: count_of(&data, m5_data::MANA_POTION),
            effects,
        };
        // The next login loads what the store wrote, with the buffs of the last logout to apply
        // (`restoreEffects` reads them from the table; the loader hands them over as these).
        let mut next = data;
        next.profile.effects_to_restore = next.profile.effects_to_store.take().unwrap_or_default();
        self.players[who.index()] = next;
        left
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

/// `skill_id, skill_level, remaining_time, reuse_delay, systime, restore_type, class_index,
/// buff_index` of `character_skills_save`.
type SkillSaveColumns = (i32, i32, i32, i32, i64, i32, i32, i32);

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

    /// `(item id, count)` of the character's stacks of `item_id`, summed.
    async fn count_of(&self, char_id: i32, item_id: i32) -> i64 {
        let rows: Vec<(u64,)> =
            sqlx::query_as("SELECT `count` FROM items WHERE owner_id = ? AND item_id = ?")
                .bind(char_id)
                .bind(item_id)
                .fetch_all(&self.pool)
                .await
                .unwrap();
        rows.iter().map(|(count,)| i64::try_from(*count).unwrap()).sum()
    }

    /// The rows of `character_skills_save` of the character, in the order of `buff_index`, with
    /// the index and class index that go with them.
    async fn skill_save_rows(&self, char_id: i32) -> Vec<(Stored, i32, i32)> {
        let rows: Vec<SkillSaveColumns> = sqlx::query_as(
            "SELECT skill_id, skill_level, remaining_time, reuse_delay, CAST(systime AS SIGNED), \
             restore_type, class_index, buff_index FROM character_skills_save WHERE charId = ? \
             ORDER BY buff_index",
        )
        .bind(char_id)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        rows.into_iter()
            .map(
                |(
                    skill_id,
                    skill_level,
                    remaining_time,
                    reuse_delay,
                    systime,
                    restore_type,
                    class_index,
                    buff_index,
                )| {
                    (
                        Stored {
                            skill_id,
                            skill_level,
                            remaining_time,
                            reuse_delay: i64::from(reuse_delay),
                            systime,
                            restore_type,
                        },
                        class_index,
                        buff_index,
                    )
                },
            )
            .collect()
    }

    /// `(skill id, level)` of the rows of `character_skills` of the active class, sorted.
    async fn known_skills(&self, char_id: i32) -> Vec<(i32, i32)> {
        sqlx::query_as(
            "SELECT skill_id, skill_level FROM character_skills WHERE charId = ? AND class_index = 0 \
             ORDER BY skill_id",
        )
        .bind(char_id)
        .fetch_all(&self.pool)
        .await
        .unwrap()
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
        let rig = Rig::start("e2em5").await?;
        let a = rig.create_character_and_leave(ACCOUNT_A, NAME_A).await;
        let b = rig.create_character_and_leave(ACCOUNT_B, NAME_B).await;
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

    async fn click(&mut self, who: Who, object_id: i32) {
        self.client(who).click(object_id, m3_data::START).await;
    }

    async fn use_skill(&mut self, who: Who, skill_id: i32) {
        self.client(who).use_skill(skill_id, false).await;
    }

    async fn use_item(&mut self, who: Who, object_id: i32) {
        self.client(who).use_item(object_id).await;
    }

    async fn auto_shot(&mut self, who: Who, item_id: i32, on: bool) {
        self.client(who).auto_soulshot(item_id, on).await;
    }

    async fn skill_cool_time(&mut self, who: Who) {
        self.client(who).skill_cool_time().await;
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
        let id = self.ids[who.index()];
        let rows = self.rig.skill_save_rows(id).await;
        for (index, (_, class_index, buff_index)) in rows.iter().enumerate() {
            assert_eq!(*class_index, 0, "the base class");
            assert_eq!(
                usize::try_from(*buff_index).unwrap(),
                index + 1,
                "Java numbers the rows from 1 in the order it saves them"
            );
        }
        Left {
            skills: self.rig.known_skills(id).await,
            soulshots: self.rig.count_of(id, SOULSHOT_ID).await,
            potions: self.rig.count_of(id, m5_data::MANA_POTION).await,
            effects: rows.into_iter().map(|(row, ..)| row).collect(),
        }
    }
}

// ------------------------------------------------------------------------------------------
// The test with a database
// ------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_caster_buffs_nukes_drinks_shoots_and_relogs_while_a_watcher_sees_it() {
    let Some(mut net) = Net::start().await else {
        return;
    };
    let a = net.ids[0];
    let played = play(&mut net).await;
    assert_played(&played);

    // The rows the game server wrote for the caster, as Java writes them: the skills the creation
    // gave, the buff with the reuse timer of the cast, which is in the future.
    let skills = net.rig.known_skills(a).await;
    assert!(skills.contains(&(m5_data::WIND_STRIKE, 1)) && skills.contains(&(m5_data::WIND_WALK, 1)));
    let rows = net.rig.skill_save_rows(a).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let now_ms = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    assert!(
        rows[0].0.systime > now_ms && rows[0].0.systime <= now_ms + m5_data::WIND_WALK_REUSE_MS,
        "the reuse timer ends in the future: {} (now {now_ms})",
        rows[0].0.systime
    );
    // The soulshots the store shows are the ones the client was told about.
    assert_eq!(net.rig.count_of(a, SOULSHOT_ID).await, played.soulshots_seen);

    // The character list still shows the character.
    let (mut again, list, _) = net.rig.login(ACCOUNT_A, 1).await;
    assert_eq!(list.characters[0].object_id, a);
    assert!(again.logout().await.is_empty());
    net.rig.wait_until_out_of_game(ACCOUNT_A).await;

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
fn the_skill_data_loads_as_the_game_server_reads_it() {
    let root = tempfile::tempdir().unwrap();
    write_datapack(root.path());
    let pack = Datapack::load(root.path()).unwrap();

    for (id, name) in [
        (m5_data::WIND_STRIKE, "Wind Strike"),
        (m5_data::WIND_WALK, "Wind Walk"),
        (m5_data::SOULSHOT_SKILL, "Soulshot: No Grade"),
        (m5_data::MANA_POTION_SKILL, "Mana Potion"),
    ] {
        let skill = pack.skills.get(id, 1).unwrap_or_else(|| panic!("skill {id}"));
        assert_eq!(skill.name, name);
    }
    let walk = pack.skills.get(m5_data::WIND_WALK, 1).unwrap();
    assert_eq!(i64::from(walk.reuse_delay), m5_data::WIND_WALK_REUSE_MS);
    assert_eq!(walk.abnormal_time, m5_data::WIND_WALK_SECONDS);

    let potion = pack.items.get(m5_data::MANA_POTION).expect("the mana potion");
    assert!(potion.stackable && potion.immediate_effect);
    let shot = pack.items.get(SOULSHOT_ID).expect("the soulshot");
    assert!(shot.stackable);
    let dummy = pack.npcs.get(m5_data::DUMMY_ID).expect("the dummy");
    assert_eq!(dummy.npc_type.as_str(), "Monster");
    assert_eq!(pack.spawns.spawns_of(m5_data::DUMMY_ID).count(), 1);
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
    assert_eq!(config.player.starting_adena, START_ADENA);
    assert!(config.player.store_skill_cooltime, "the buffs are stored");
    assert!(!config.player.enable_vitality);
}

/// What the database variant relies on and cannot show without a database: the character the game
/// server creates for the account (the plan it writes to `characters`, `items` and
/// `character_skills`) knows both skills and owns the potions and soulshots of the scenario.
#[test]
fn a_new_fighter_is_created_with_the_skills_and_items_of_the_scenario() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    write_game_config(&config_dir, 4242);
    let data_root = dir.path().join("datapack");
    write_datapack(&data_root);
    let config = GameConfig::load(&config_dir).unwrap();
    let pack = Datapack::load(&data_root).unwrap();
    let bonus = StatBonus::load(&data_root);
    let max_mp = ItemMaxMp::load(&data_root, &ItemMaxMp::wanted_ids(&pack), false);
    let ctx = PlanContext {
        datapack: &pack,
        config: &config,
        stat_bonus: &bonus,
        item_max_mp: &max_mp,
        master_access_level: 0,
    };
    let next_id = std::sync::atomic::AtomicI32::new(0x6000_0001);
    let allocate = || Some(next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let mut ids = IdSource::new(&allocate);
    let request = CreateRequest {
        account: ACCOUNT_A,
        name: NAME_A,
        is_female: false,
        hair_style: 1,
        hair_color: 1,
        face: 1,
    };
    let plan = build_plan(
        &ctx,
        &request,
        pack.player_templates.get_by_id(0).unwrap(),
        0x6000_0000,
        &mut ids,
        &mut WorldRng::from_seed(3),
        0,
    )
    .unwrap();

    let mut skills: Vec<(i32, i32)> = plan.skills.iter().map(|s| (s.skill_id, s.skill_level)).collect();
    skills.sort_unstable();
    for wanted in [(m5_data::WIND_STRIKE, 1), (m5_data::WIND_WALK, 1)] {
        assert!(skills.contains(&wanted), "{wanted:?} in {skills:?}");
    }
    let stack = |item_id: i32| -> Vec<(i64, ItemLocation)> {
        plan.items
            .iter()
            .filter(|i| i.item_id == item_id)
            .map(|i| (i.count, i.loc))
            .collect()
    };
    assert_eq!(
        stack(SOULSHOT_ID),
        [(m5_data::SOULSHOTS_AT_START, ItemLocation::Inventory)]
    );
    assert_eq!(
        stack(m5_data::MANA_POTION),
        [(m5_data::POTIONS_AT_START, ItemLocation::Inventory)]
    );
    assert_eq!(
        stack(2369),
        [(1, ItemLocation::Paperdoll)],
        "the sword is in the hand"
    );
    // A created character starts with the maximum MP (more than the 30 of the bare character).
    assert!(plan.update.cur_mp >= 30.0 && plan.update.cur_mp == f64::from(plan.update.max_mp));
}
