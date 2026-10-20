#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End to end for the combat of M4: a hunter and a watcher log in through the in-process login
//! server and game server. The hunter kills a monster (and so gets exp, SP, adena and items, and a
//! level), picks the loot up, walks into the aggro range of a monster it cannot beat, dies, and
//! restarts in the town; the watcher sees all of it. The monster that died comes back after its
//! respawn delay, and the rows of the hunter are what Java would write.
//!
//! The scenario is written once (`play`) against a `Harness` and runs twice, like `e2e_m3.rs`:
//!
//! * `Net`: the real thing. The Rust login server and the Rust game server in one process, a
//!   throwaway MariaDB, two `FakeGameClient`s on sockets. It runs only when
//!   `L2_TEST_DATABASE_URL` is set (CI's MariaDB job; never point it at a database with real
//!   data) and checks the rows the game server wrote. Both servers listen on port 0 on loopback.
//!   The game server seeds its world RNG from the system, so nothing here depends on a roll: the
//!   monsters' drops are certain, the damage only has to be enough, and the numbers that matter
//!   (exp, SP, the exp a death costs) do not depend on the damage dealt.
//! * `BareWorld`: the same datapack, configuration, geodata, zones and spawns loaded the way the
//!   game server loads them, but with the world driven directly (no sockets, no database, ticks
//!   instead of time) and a seeded RNG. It always runs.
//!
//! There is no datapack and no geodata in CI: `e2e/datapack.rs`, `e2e/m3_data.rs` and
//! `e2e/m4_data.rs` write them into a temporary folder. The expected numbers are worked out from
//! the Java formulas (see `expected_after_the_rat` and `expected_after_the_death`).
//!
//! Left out on purpose: skills (M5), parties, vitality and Nevit (their own cards: if one changes
//! the exp of a kill, the numbers below move with it).

#[path = "e2e/datapack.rs"]
mod datapack;
#[allow(dead_code)]
#[path = "e2e/m3_data.rs"]
mod m3_data;
#[path = "e2e/m4_data.rs"]
mod m4_data;

use std::cell::Cell;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_server::world_data::{DatapackData, regen_settings, skill_load_options, world_settings};
use game_server::{Paths, Server, geo, hexid, npcs};
use l2_config::GameConfig;
use l2_data::{Datapack, DatapackOptions};
use l2_db::{CharacterRepository, CharacterRow, Pool};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{
    Attack, CharInfo, DeleteObject, Die, DropItem, GetItem, HITFLAG_MISS, InventoryUpdate, LeaveWorld,
    NpcInfo, Revive, SocialAction, SystemMessage, SystemMessageParam, TeleportToLocation, UserInfo,
};
use l2_protocol_login::server::PlayOk;
use l2_testkit::fake_game_client::{
    FakeGameClient, Recv, character_create, enter_world_packet, names_of, packet_name,
};
use l2_testkit::fake_login_client::FakeClient as LoginClient;
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
use l2_world::{
    ClientInput, IdFactory, ItemData, LeaveReason, LsEvent, ObjectId, Outbox, PacketSink, PersistCommand,
    PlayerData, PlayerProfile, RUST_FIRST_OBJECT_ID, SessionId, StaticData, World, WorldCommand, WorldRng,
};
use login_server::state::{LoginState, ServerStatus as ListStatus};

const ACCOUNT_A: &str = "e2em4acca";
const ACCOUNT_B: &str = "e2em4accb";
const PASSWORD: &str = "secret";
const NAME_A: &str = "Hunter";
const NAME_B: &str = "Watcher";
/// A protocol revision the game server accepts by default.
const PROTOCOL: i32 = 273;
/// `StartingAdena` of the test configuration: what both characters begin with.
const START_ADENA: i64 = 1_000;
const ADENA_ID: i32 = 57;
/// `Player.java`: the level up animation (`SocialAction.LEVEL_UP`).
const LEVEL_UP_ACTION: i32 = 2122;
/// `RequestRestartPoint`: the death window's "To Village".
const TO_VILLAGE: i32 = 0;

/// Every table of the game schema (the files of `crates/l2-db/sql/game`), so the game server
/// finds whatever it reads or writes.
const GAME_SCHEMA: [&str; 27] = [
    include_str!("../../l2-db/sql/game/characters.sql"),
    include_str!("../../l2-db/sql/game/items.sql"),
    include_str!("../../l2-db/sql/game/item_attributes.sql"),
    include_str!("../../l2-db/sql/game/character_skills.sql"),
    include_str!("../../l2-db/sql/game/character_skills_save.sql"),
    include_str!("../../l2-db/sql/game/character_item_reuse_save.sql"),
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

/// Everything in the game server's `config` folder except `Database.ini`. Besides what the M3
/// rig writes (`e2e_m3.rs`), the M4 rig sets:
/// * `Player.ini`: `DisconnectAfterDeath = False` (the shipped value; the Java code default is
///   true, and a dead player would be kicked after an hour). Everything else a death and a kill
///   read (`Delevel`, `RespawnRestoreHP`, `MaxOffsetOnTeleport`, `AutoLoot`) is left at the Java
///   default, which is also the shipped value,
/// * `NPC.ini`: `DefaultCorpseTime` of 2 seconds (shipped: 7), so the test does not wait for long
///   for a corpse to decay and a monster to come back.
fn write_game_config(config_dir: &Path, ls_port: u16) {
    std::fs::create_dir_all(config_dir).unwrap();
    std::fs::write(config_dir.join("Server.ini"), game_server_ini(ls_port)).unwrap();
    // EnableVitality = False: a new character has full vitality (Java answers lastAccess 0 with
    // the maximum), which triples the exp of a kill; M4-VIT-01's own tests cover vitality.
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

/// The datapack of this test: the base classes and items, the world data of M3 (zones, the map
/// region with the town's restart point, geodata), and the hunting data of M4.
fn write_datapack(root: &Path) {
    datapack::write_datapack(root);
    m3_data::write_world_data(root);
    m4_data::write_hunting_data(root);
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

/// Drops the `StatusUpdate` that tells a character about its own HP, MP and CP (the regeneration
/// task of Java sends it every period while one of them is not full), see `e2e_m3.rs`. Fights
/// change HP all the time, so the packets of the scenario are compared without it.
fn without_own_status(me: i32, batch: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    batch
        .into_iter()
        .filter(|body| {
            let own = body
                .get(1..5)
                .is_some_and(|id| i32::from_le_bytes(id.try_into().unwrap()) == me);
            !(own && packet_name(body) == "STATUS_UPDATE")
        })
        .collect()
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

fn is_named(body: &[u8], name: &str) -> bool {
    packet_name(body) == name
}

/// `Die` of this object.
fn is_die_of(object_id: i32) -> impl Fn(&[u8]) -> bool {
    move |body| is_named(body, "DIE") && decode_any::<Die>(body).object_id == object_id
}

/// `GetItem` of this ground item.
fn is_get_item(object_id: i32) -> impl Fn(&[u8]) -> bool {
    move |body| is_named(body, "GET_ITEM") && decode_any::<GetItem>(body).object_id == object_id
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

/// What the store of a character that left says.
#[derive(Debug, PartialEq, Eq)]
struct Left {
    position: (i32, i32, i32),
    level: i32,
    exp: i64,
    exp_before_death: i64,
    sp: i64,
    cur_hp: i64,
    max_hp: i64,
    /// `(item id, count)` of the stored items the monsters drop: adena, soulshots, rat tails.
    loot: Vec<(i32, i64)>,
    /// The object ids of the stored loot that is not adena, sorted: the adena joins a stack that
    /// was already there, the other items keep the ids they had on the ground.
    loot_object_ids: Vec<i32>,
}

/// The two ways to play the scenario. `collect_until` returns everything the player's client got
/// since the last call, up to the packet `stop` accepts and a moment after it (the rest of that
/// event); `collect_quiet` returns what arrives in a quiet moment.
trait Harness {
    fn object_id(&self, who: Who) -> i32;
    /// Logs in, selects and enters the world; returns every packet up to the closing `SKILL_LIST`.
    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>>;
    async fn move_to(&mut self, who: Who, target: (i32, i32, i32), origin: (i32, i32, i32));
    async fn click(&mut self, who: Who, object_id: i32);
    async fn restart_point(&mut self, who: Who, point_type: i32);
    async fn appearing(&mut self, who: Who);
    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>>;
    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>>;
    /// Logs out and returns what was stored.
    async fn leave(&mut self, who: Who) -> Left;
}

/// What the scenario saw, for the checks after it.
struct Played {
    left_a: Left,
    left_b: Left,
    /// Where the restart point sent the hunter.
    restarted_at: (i32, i32, i32),
    /// `(item id, object id)` of the three items the rat dropped.
    loot_ids: Vec<(i32, i32)>,
}

/// The hunter's exp and SP after the rat, from Java: `Attackable.calculateRewards` gives the only
/// damage dealer the whole `(long) (exp * RateXp) * damage / totalDamage = 100` and
/// `(int) (sp * RateSp) = 12`; `PlayerStat.addExpAndSp` adds them without bonuses; 100 is above
/// the 68 of level 2 and below the 363 of level 3.
fn expected_after_the_rat() -> (i32, i64, i64) {
    (2, m4_data::RAT_EXP, m4_data::RAT_SP)
}

/// The exp after the wolf killed the hunter, from Java `Player.calculateDeathExpPenalty`:
/// level 2 pays `experienceLoss.xml` 9.875% of the level (`363 - 68 = 295` exp), which is 29.13
/// and so `Math.round` 29; the cap is a tenth of the level, `295 / 10 = 29`. The exp before the
/// death (100) is kept in `expBeforeDeath`.
fn expected_after_the_death() -> i64 {
    let (_, exp, _) = expected_after_the_rat();
    let level_exp = 363 - 68;
    let lost = ((level_exp as f64 * m4_data::EXP_LOSS[1]) / 100.0).round() as i64;
    exp - lost.min(level_exp / 10)
}

async fn play<H: Harness>(h: &mut H) -> Played {
    let (a, b) = (h.object_id(Who::A), h.object_id(Who::B));

    // ---- both enter; the hunter sees the monsters, the watcher sees the hunter -------------------
    let entered_a = h.enter(Who::A).await;

    let npcs_seen: Vec<NpcInfo> = all(&entered_a, "NPC_INFO");
    let find = |template: i32| {
        npcs_seen
            .iter()
            .find(|n| n.display_id == template)
            .unwrap_or_else(|| panic!("NPC {template} is not in view: {npcs_seen:?}"))
            .clone()
    };
    let rat = find(m4_data::RAT_ID);
    let wolf = find(m4_data::WOLF_ID);
    assert_eq!(
        (rat.x, rat.y, rat.z),
        m4_data::RAT_POS,
        "the spawn of the datapack"
    );
    assert_eq!((wolf.x, wolf.y, wolf.z), m4_data::WOLF_POS);
    assert!(rat.attackable, "a monster is attackable");
    let entered_b = h.enter(Who::B).await;
    assert!(
        names(&entered_b).contains(&"CHAR_INFO".to_owned()),
        "the watcher sees the hunter"
    );
    h.collect_until(Who::A, &|body| is_named(body, "CHAR_INFO")).await;

    // ---- the hunter clicks the rat twice: target, then attack ---------------------------------
    h.click(Who::A, rat.object_id).await;
    let targeting = without_own_status(
        a,
        h.collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED"))
            .await,
    );
    assert_in_order(&names(&targeting), &["MY_TARGET_SELECTED"]);

    // ---- the fight: the hunter runs up to the rat and hits it until it dies ---------------------
    h.click(Who::A, rat.object_id).await;
    let fight = without_own_status(a, h.collect_until(Who::A, &is_die_of(rat.object_id)).await);
    let fight_names = names(&fight);

    // The hunter's blows are `Attack` packets of the hunter against the rat, and the first thing
    // the second click does is to walk to it (the rat is 450 units away).
    let attacks: Vec<Attack> = all(&fight, "ATTACK");
    let blows: Vec<&Attack> = attacks.iter().filter(|x| x.attacker_id == a).collect();
    assert!(!blows.is_empty(), "the hunter hit the rat: {fight_names:?}");
    assert!(blows.iter().all(|x| x.first_hit.target_id == rat.object_id));
    assert_in_order(&fight_names, &["MOVE_TO_PAWN", "ATTACK"]);
    // The hits that landed add up to at least the 30 HP of the template (a miss deals nothing; the
    // stat engine makes the rat's maximum HP a little more, so there are two or three blows).
    let dealt: i32 = blows
        .iter()
        .flat_map(|x| std::iter::once(&x.first_hit).chain(x.other_hits.iter()))
        .filter(|hit| hit.flags & HITFLAG_MISS == 0)
        .map(|hit| hit.damage)
        .sum();
    assert!(dealt >= 30, "{dealt} damage killed a rat of 30 base HP");

    // The kill, in the order of Java's `Creature.doDie`: the loot is dropped, the exp is given, and
    // only then the AI is told and the `Die` packet goes out (`calculateRewards` comes first).
    let drops: Vec<DropItem> = all(&fight, "DROP_ITEM");
    assert_eq!(drops.len(), 3, "adena, soulshots and a rat tail: {fight_names:?}");
    assert_in_order(
        &fight_names,
        &["DROP_ITEM", "DROP_ITEM", "DROP_ITEM", "SOCIAL_ACTION", "DIE"],
    );
    // `rewardSkills` sends the skill list 300 ms after the level up.
    assert_in_order(&fight_names, &["SOCIAL_ACTION", "SKILL_LIST"]);
    let mut dropped: Vec<(i32, i64)> = drops.iter().map(|d| (d.display_id, d.count)).collect();
    dropped.sort_unstable();
    let mut wanted_drops = vec![
        (ADENA_ID, m4_data::RAT_ADENA),
        (m4_data::SOULSHOT_ID, m4_data::RAT_SOULSHOTS),
        (m4_data::RAT_TAIL_ID, 1),
    ];
    wanted_drops.sort_unstable();
    assert_eq!(dropped, wanted_drops, "certain drops, exact counts");
    assert!(
        drops.iter().all(|d| d.player_id == rat.object_id),
        "the rat is the dropper"
    );
    let loot_ids: Vec<(i32, i32)> = drops.iter().map(|d| (d.display_id, d.object_id)).collect();
    assert!(
        loot_ids.iter().all(|(_, id)| *id >= RUST_FIRST_OBJECT_ID),
        "the loot has ids of the Rust range (D-39): {loot_ids:?}"
    );

    // Exp and SP: the message, the level up (animation, message), the new level in `UserInfo`.
    let (level, exp, sp) = expected_after_the_rat();
    let earned: Vec<SystemMessage> = all(&fight, "SYSTEM_MESSAGE");
    let exp_message = earned
        .iter()
        .find(|m| m.id == SystemMessageId::YouHaveEarnedS1ExperienceAndS2Sp)
        .unwrap_or_else(|| panic!("no exp message: {earned:?}"));
    assert_eq!(
        exp_message.params,
        [
            SystemMessageParam::LongNumber(exp),
            SystemMessageParam::LongNumber(sp)
        ]
    );
    assert!(
        earned
            .iter()
            .any(|m| m.id == SystemMessageId::YourLevelHasIncreased),
        "the level went up"
    );
    // Only the hunter's own animations: NPCs in view may play their random animation
    // (RandomAnimationTaskManager, M4-AI-02) at any time.
    let animation = all::<SocialAction>(&fight, "SOCIAL_ACTION");
    assert_eq!(
        animation
            .iter()
            .filter(|s| s.object_id == a)
            .map(|s| (s.object_id, s.action_id))
            .collect::<Vec<_>>(),
        [(a, LEVEL_UP_ACTION)]
    );
    let info: UserInfo = all(&fight, "USER_INFO")
        .pop()
        .expect("UserInfo after the level up");
    assert_eq!((info.level, info.exp), (level, exp), "{fight_names:?}");

    // ---- the loot: the hunter walks to each item and picks it up -------------------------------
    // The adena joins the stack in the inventory, the soulshots are a new stack, the tail a new
    // item. `InventoryUpdate` follows `GetItem` by 300 ms.
    let mut pickups = Vec::new();
    for drop in &drops {
        h.click(Who::A, drop.object_id).await;
        let got = without_own_status(a, h.collect_until(Who::A, &is_get_item(drop.object_id)).await);
        let get: GetItem = all(&got, "GET_ITEM").remove(0);
        assert_eq!((get.player_id, get.object_id), (a, drop.object_id));
        pickups.push(got);
    }
    let change_of = |item_id: i32| -> (i16, i64) {
        pickups
            .iter()
            .flat_map(|batch| all::<InventoryUpdate>(batch, "INVENTORY_UPDATE"))
            .flat_map(|update| update.updates)
            .find(|u| u.item.item_id == item_id)
            .map(|u| (u.change, u.item.count))
            .unwrap_or_else(|| panic!("no inventory update for {item_id}"))
    };
    assert_eq!(
        change_of(ADENA_ID),
        (InventoryUpdate::MODIFIED, START_ADENA + m4_data::RAT_ADENA),
        "the adena joins the stack"
    );
    assert_eq!(
        change_of(m4_data::SOULSHOT_ID),
        (InventoryUpdate::ADDED, m4_data::RAT_SOULSHOTS)
    );
    assert_eq!(change_of(m4_data::RAT_TAIL_ID), (InventoryUpdate::ADDED, 1));

    // ---- the hunter walks into the wolf's aggro range and is killed ----------------------------
    h.move_to(Who::A, m4_data::NEAR_THE_WOLF, m4_data::RAT_POS).await;
    let death = without_own_status(a, h.collect_until(Who::A, &is_die_of(a)).await);
    let death_names = names(&death);
    let bites: Vec<Attack> = all(&death, "ATTACK");
    assert!(
        bites
            .iter()
            .any(|x| x.attacker_id == wolf.object_id && x.first_hit.target_id == a),
        "the wolf hit the hunter: {death_names:?}"
    );
    let dies: Die = all(&death, "DIE").remove(0);
    assert_eq!(
        dies,
        Die {
            object_id: a,
            // The window offers the town (nothing is pending) and nothing else: no clan, no
            // self-resurrection item, no sweep.
            can_teleport: true,
            to_clan_hall: false,
            to_castle: false,
            to_siege_hq: false,
            sweepable: false,
            fixed_res: false,
            to_fortress: false,
        }
    );

    // ---- "To village": revive first, then the teleport, then the client says it has arrived ----
    h.restart_point(Who::A, TO_VILLAGE).await;
    let leaving = without_own_status(
        a,
        h.collect_until(Who::A, &|b| is_named(b, "TELEPORT_TO_LOCATION"))
            .await,
    );
    assert_in_order(&names(&leaving), &["REVIVE", "TELEPORT_TO_LOCATION"]);
    assert_eq!(all::<Revive>(&leaving, "REVIVE")[0].object_id, a);
    let teleport: TeleportToLocation = all(&leaving, "TELEPORT_TO_LOCATION").remove(0);
    assert_eq!(teleport.object_id, a);
    let restarted_at = (teleport.x, teleport.y, teleport.z);
    // The town's restart point (`respawnPoint` of the map region) with `MaxOffsetOnTeleport` 50.
    let town = (m3_data::START.0, m3_data::START.1 - 300);
    assert!(
        (teleport.x - town.0).abs() <= 50 && (teleport.y - town.1).abs() <= 50,
        "the restart point of the town: {restarted_at:?}"
    );
    h.appearing(Who::A).await;
    let arrived = without_own_status(a, h.collect_until(Who::A, &|b| is_named(b, "USER_INFO")).await);
    let info: UserInfo = all(&arrived, "USER_INFO").remove(0);
    assert_eq!((info.x, info.y, info.z), restarted_at, "UserInfo shows the town");
    assert_eq!(info.exp, expected_after_the_death(), "the death cost exp");

    // ---- what the watcher saw: the whole thing, in order, then the rat's return -----------------
    // The rat decays 2 s after its death (`DefaultCorpseTime`) and, after its `respawnDelay` of
    // 5 s, the same object comes back. The packets of B have piled up since it entered; the wait
    // ends when both the rat's return and the hunter's return to the town have been seen (which
    // comes first depends on how long the walk to the wolf took).
    let rat_deleted = Cell::new(false);
    let rat_back = Cell::new(false);
    let hunter_teleported = Cell::new(false);
    let hunter_back = Cell::new(false);
    let rat_id = rat.object_id;
    let seen_b = h
        .collect_until(Who::B, &|body| {
            match packet_name(body).as_str() {
                "DELETE_OBJECT" if decode_any::<DeleteObject>(body).object_id == rat_id => {
                    rat_deleted.set(true);
                }
                "NPC_INFO" if rat_deleted.get() && decode_any::<NpcInfo>(body).object_id == rat_id => {
                    rat_back.set(true);
                }
                "TELEPORT_TO_LOCATION" if decode_any::<TeleportToLocation>(body).object_id == a => {
                    hunter_teleported.set(true);
                }
                "CHAR_INFO" if hunter_teleported.get() && decode_any::<CharInfo>(body).object_id == a => {
                    hunter_back.set(true);
                }
                _ => {}
            }
            rat_back.get() && hunter_back.get()
        })
        .await;
    let seen_b = without_own_status(b, seen_b);
    let seen_names = names(&seen_b);
    // The hunter hits the rat.
    let attack_a = |body: &Vec<u8>| is_named(body, "ATTACK") && decode_any::<Attack>(body).attacker_id == a;
    assert!(
        seen_b.iter().any(attack_a),
        "B saw the hunter fight: {seen_names:?}"
    );
    // Position in B's list of the first packet that satisfies each condition, in the order the
    // events happened.
    let position = |what: &str, test: &dyn Fn(&Vec<u8>) -> bool| -> usize {
        seen_b
            .iter()
            .position(test)
            .unwrap_or_else(|| panic!("B never saw {what}: {seen_names:?}"))
    };
    let rat_dies = position("the rat die", &|p| is_die_of(rat_id)(p));
    let first_drop = position("the loot drop", &|p| is_named(p, "DROP_ITEM"));
    let level_up = position("the level up", &|p| {
        is_named(p, "SOCIAL_ACTION") && decode_any::<SocialAction>(p).action_id == LEVEL_UP_ACTION
    });
    let picked_up = position("the pick up", &|p| {
        is_named(p, "GET_ITEM") && decode_any::<GetItem>(p).player_id == a
    });
    let wolf_bites = position("the wolf bite", &|p| {
        is_named(p, "ATTACK") && decode_any::<Attack>(p).attacker_id == wolf.object_id
    });
    let hunter_dies = position("the hunter die", &|p| is_die_of(a)(p));
    let revived = position("the revive", &|p| {
        is_named(p, "REVIVE") && decode_any::<Revive>(p).object_id == a
    });
    let teleports = position("the teleport", &|p| {
        is_named(p, "TELEPORT_TO_LOCATION") && decode_any::<TeleportToLocation>(p).object_id == a
    });
    assert!(
        first_drop < rat_dies,
        "the drop comes before the death: {seen_names:?}"
    );
    assert!(level_up < rat_dies, "and so does the exp");
    assert!(rat_dies < picked_up, "the loot is picked up after the kill");
    assert!(picked_up < wolf_bites && wolf_bites < hunter_dies && hunter_dies < revived);
    assert!(revived < teleports, "revived, then teleported");
    let three_items_taken = seen_b
        .iter()
        .filter(|p| is_named(p, "GET_ITEM") && decode_any::<GetItem>(p).player_id == a)
        .count();
    assert_eq!(three_items_taken, 3, "B saw each pick up");
    assert!(
        seen_b.iter().any(|p| is_named(p, "CHAR_INFO")),
        "after the teleport the watcher sees the hunter in the town again: {seen_names:?}"
    );
    let _ = h.collect_quiet(Who::A).await;

    // ---- both log out; what is stored is final ------------------------------------------------
    let left_a = h.leave(Who::A).await;
    let left_b = h.leave(Who::B).await;
    Played {
        left_a,
        left_b,
        restarted_at,
        loot_ids,
    }
}

fn assert_played(played: &Played) {
    let (level, _, sp) = expected_after_the_rat();
    let a = &played.left_a;
    assert_eq!(
        a.level, level,
        "the level the rat gave is kept: dying does not cost a level here"
    );
    assert_eq!(a.exp, expected_after_the_death());
    assert_eq!(
        a.exp_before_death,
        m4_data::RAT_EXP,
        "Java stores the exp before the death in expBeforeDeath"
    );
    assert_eq!(a.sp, sp);
    assert_eq!(
        a.position, played.restarted_at,
        "stored where the restart point put the hunter"
    );
    // `RespawnRestoreHP` 65: the revive gives back that share of the maximum, and the regeneration
    // since then can only add to it.
    let at_least = a.max_hp * 65 / 100 - 1;
    assert!(
        (at_least..=a.max_hp).contains(&a.cur_hp),
        "HP after the revive: {} of {}",
        a.cur_hp,
        a.max_hp
    );
    assert_eq!(
        a.loot,
        [
            (ADENA_ID, START_ADENA + m4_data::RAT_ADENA),
            (m4_data::SOULSHOT_ID, m4_data::RAT_SOULSHOTS),
            (m4_data::RAT_TAIL_ID, 1),
        ]
    );
    let mut dropped_ids: Vec<i32> = played
        .loot_ids
        .iter()
        .filter(|(item_id, _)| *item_id != ADENA_ID)
        .map(|(_, object_id)| *object_id)
        .collect();
    dropped_ids.sort_unstable();
    assert_eq!(
        a.loot_object_ids, dropped_ids,
        "the picked up items are the ground items the monster dropped, with their ids (D-39)"
    );

    let b = &played.left_b;
    assert_eq!(
        (b.level, b.exp, b.sp, b.exp_before_death),
        (1, 0, 0, 0),
        "B did nothing"
    );
    assert_eq!(b.position, m3_data::START);
    assert_eq!(b.loot, [(ADENA_ID, START_ADENA)]);
    assert!(b.loot_object_ids.is_empty());
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

/// Far above the ids the factory hands out to the NPCs (it counts up from `RUST_FIRST_OBJECT_ID`).
const BARE_IDS: [i32; 2] = [0x7000_0001, 0x7000_0002];
/// The longest the bare world waits for an event: 60 seconds of world time (50 ms ticks).
const BARE_LIMIT_TICKS: usize = 1_200;
/// The moment after an event that is collected with it: 600 ms, longer than the 300 ms delay of
/// the `InventoryUpdate` that follows a pick up.
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
        // What a new fighter owns that matters here: adena and the squire's sword in the hand.
        items: vec![
            item(1_000 + slot, ADENA_ID, START_ADENA, "INVENTORY", 0),
            item(
                2_000 + slot,
                2369,
                1,
                "PAPERDOLL",
                i32::try_from(paperdoll::RHAND).unwrap(),
            ),
        ],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

/// The items of `data` that the monsters drop, as `(item id, count)`, sorted by item id.
fn loot_of(data: &PlayerData) -> (Vec<(i32, i64)>, Vec<i32>) {
    let rows: Vec<&ItemData> = data
        .items
        .iter()
        .filter(|i| [ADENA_ID, m4_data::SOULSHOT_ID, m4_data::RAT_TAIL_ID].contains(&i.item_id))
        .collect();
    let mut loot: Vec<(i32, i64)> = rows.iter().map(|i| (i.item_id, i.count)).collect();
    loot.sort_unstable();
    let mut ids: Vec<i32> = rows
        .iter()
        .filter(|i| i.item_id != ADENA_ID)
        .map(|i| i.object_id.0)
        .collect();
    ids.sort_unstable();
    (loot, ids)
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

        let stored = Stored::default();
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

    async fn click(&mut self, who: Who, object_id: i32) {
        self.input(
            who,
            ClientInput::Action {
                object_id,
                action_id: 0,
            },
        );
    }

    async fn restart_point(&mut self, who: Who, point_type: i32) {
        self.input(who, ClientInput::RestartPoint { point_type });
    }

    async fn appearing(&mut self, who: Who) {
        self.input(who, ClientInput::Appearing);
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
        let stored = self.stored.0.lock().unwrap();
        let data = stored
            .iter()
            .rev()
            .find(|data| data.object_id == ObjectId(id))
            .expect("the character was stored");
        let (loot, loot_object_ids) = loot_of(data);
        Left {
            position: (data.x, data.y, data.z),
            level: data.level,
            exp: data.exp,
            exp_before_death: data.profile.exp_before_death,
            sp: data.sp,
            cur_hp: data.cur_hp as i64,
            max_hp: data.max_hp as i64,
            loot,
            loot_object_ids,
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

    /// `(object id, item id, count, loc, loc_data)` of every row of the character's items.
    async fn items(&self, char_id: i32) -> Vec<(i32, i32, u64, String, Option<i32>)> {
        sqlx::query_as(
            "SELECT object_id, item_id, `count`, loc, loc_data FROM items WHERE owner_id = ? ORDER BY item_id, object_id",
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
        let rig = Rig::start("e2em4").await?;
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

    async fn move_to(&mut self, who: Who, target: (i32, i32, i32), origin: (i32, i32, i32)) {
        self.client(who).move_to(target, origin).await;
    }

    async fn click(&mut self, who: Who, object_id: i32) {
        self.client(who).click(object_id, m3_data::START).await;
    }

    async fn restart_point(&mut self, who: Who, point_type: i32) {
        self.client(who).restart_point(point_type).await;
    }

    async fn appearing(&mut self, who: Who) {
        self.client(who).appearing().await;
    }

    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        let client = self.client(who);
        let mut batch = Vec::new();
        loop {
            // A fight takes a few seconds and a respawn longer: wait up to 40 s for the next packet.
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
        // Longer than the 300 ms delay of the `InventoryUpdate` that follows a pick up.
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
        let row = self.rig.row(id).await;
        assert_eq!(row.online, Some(0), "offline again");
        let rows: Vec<_> = self
            .rig
            .items(id)
            .await
            .into_iter()
            .filter(|(_, item_id, ..)| {
                [ADENA_ID, m4_data::SOULSHOT_ID, m4_data::RAT_TAIL_ID].contains(item_id)
            })
            .collect();
        let mut loot: Vec<(i32, i64)> = rows
            .iter()
            .map(|(_, item_id, count, ..)| (*item_id, i64::try_from(*count).unwrap()))
            .collect();
        loot.sort_unstable();
        let mut loot_object_ids: Vec<i32> = rows
            .iter()
            .filter(|(_, item_id, ..)| *item_id != ADENA_ID)
            .map(|(object_id, ..)| *object_id)
            .collect();
        loot_object_ids.sort_unstable();
        // A column that was never written reads as NULL; Java reads that as 0.
        Left {
            position: (row.x.unwrap(), row.y.unwrap(), row.z.unwrap()),
            level: i32::from(row.level.unwrap_or(0)),
            exp: i64::try_from(row.exp.unwrap_or(0)).unwrap(),
            exp_before_death: i64::try_from(row.exp_before_death.unwrap_or(0)).unwrap(),
            sp: i64::try_from(row.sp).unwrap(),
            cur_hp: i64::from(row.cur_hp.unwrap_or(0)),
            max_hp: i64::from(row.max_hp.unwrap_or(0)),
            loot,
            loot_object_ids,
        }
    }
}

// ------------------------------------------------------------------------------------------
// The test with a database
// ------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_hunter_kills_loots_levels_dies_and_restarts_while_a_watcher_sees_it() {
    let Some(mut net) = Net::start().await else {
        return;
    };
    let (a, b) = (net.ids[0], net.ids[1]);
    let played = play(&mut net).await;
    assert_played(&played);

    // The rows the game server wrote for the hunter, as Java writes them.
    let row = net.rig.row(a).await;
    assert_eq!(
        (row.level, row.exp, row.exp_before_death, row.sp),
        (Some(2), Some(71), Some(100), 12),
        "characters row of the hunter"
    );
    let items = net.rig.items(a).await;
    for (item_id, count) in [
        (ADENA_ID, START_ADENA + m4_data::RAT_ADENA),
        (m4_data::SOULSHOT_ID, m4_data::RAT_SOULSHOTS),
        (m4_data::RAT_TAIL_ID, 1),
    ] {
        let rows: Vec<_> = items.iter().filter(|r| r.1 == item_id).collect();
        assert_eq!(rows.len(), 1, "one stack of {item_id}: {items:?}");
        let (object_id, _, stored, loc, loc_data) = rows[0];
        assert_eq!(i64::try_from(*stored).unwrap(), count, "count of {item_id}");
        assert_eq!(loc, "INVENTORY");
        assert_eq!(
            *loc_data,
            Some(0),
            "Java: setItemLocation(INVENTORY) has loc_data 0"
        );
        if item_id != ADENA_ID {
            // The item the monster dropped has an id of the Rust range (D-39), and it is one of
            // the ground items of the kill.
            assert!(*object_id >= RUST_FIRST_OBJECT_ID, "{object_id}");
            assert!(
                played.loot_ids.contains(&(item_id, *object_id)),
                "{object_id} in {:?}",
                played.loot_ids
            );
        }
    }
    // B got none of the loot.
    assert!(
        net.rig
            .items(b)
            .await
            .iter()
            .all(|r| ![m4_data::SOULSHOT_ID, m4_data::RAT_TAIL_ID].contains(&r.1))
    );

    // The character list shows the level and where the hunter stands.
    let (mut again, list, _) = net.rig.login(ACCOUNT_A, 1).await;
    let entry = &list.characters[0];
    assert_eq!((entry.level, entry.exp), (2, 71));
    assert_eq!((entry.x, entry.y, entry.z), played.restarted_at);
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
fn the_hunting_data_loads_as_the_game_server_reads_it() {
    let root = tempfile::tempdir().unwrap();
    write_datapack(root.path());
    let pack = Datapack::load(root.path()).unwrap();

    let rat = pack.npcs.get(m4_data::RAT_ID).expect("the rat template");
    assert_eq!((rat.npc_type.as_str(), rat.level), ("Monster", 1));
    assert_eq!(
        (rat.exp, rat.sp),
        (m4_data::RAT_EXP as f64, m4_data::RAT_SP as f64)
    );
    assert!(!rat.is_aggressive);
    assert_eq!(rat.drops.len(), 3, "adena, soulshots and the tail");
    assert!(rat.drops.iter().all(|d| d.chance == 100.0 && d.min == d.max));
    let wolf = pack.npcs.get(m4_data::WOLF_ID).expect("the wolf template");
    assert!(wolf.is_aggressive);
    assert_eq!(wolf.aggro_range, m4_data::WOLF_AGGRO_RANGE);

    for (id, name) in [(m4_data::RAT_ID, "rat"), (m4_data::WOLF_ID, "wolf")] {
        assert_eq!(pack.spawns.spawns_of(id).count(), 1, "one {name} spawn");
    }
    let spawn = pack.spawns.spawns_of(m4_data::RAT_ID).next().unwrap();
    assert_eq!(spawn.respawn_min_ms, m4_data::RESPAWN_SECONDS * 1000);

    // The numbers the expectations are built from.
    assert_eq!(pack.experience.exp_for_level(2), Some(68));
    assert_eq!(pack.experience.exp_for_level(3), Some(363));
    assert_eq!(pack.experience_loss.percent_lost(2), 9.875);
    assert_eq!(expected_after_the_death(), 71);
    assert!(pack.items.get(m4_data::SOULSHOT_ID).is_some_and(|i| i.stackable));
    assert!(pack.items.get(m4_data::RAT_TAIL_ID).is_some_and(|i| !i.stackable));

    // The rat stands in the town, where only fights between players are stopped; the wolf stands
    // outside the town zones.
    let (x, y, z) = m4_data::RAT_POS;
    assert!(pack.zones.is_inside_zone_type(x, y, z, "PeaceZone"));
    let (x, y, z) = m4_data::WOLF_POS;
    assert!(!pack.zones.is_inside_zone_type(x, y, z, "PeaceZone"));
    let (x, y, z) = m4_data::NEAR_THE_WOLF;
    assert!(!pack.zones.is_inside_zone_type(x, y, z, "PeaceZone"));
    let distance = f64::from(m4_data::WOLF_POS.1 - m4_data::NEAR_THE_WOLF.1);
    assert!(distance < f64::from(m4_data::WOLF_AGGRO_RANGE));
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
    assert!(!config.player.disconnect_after_death);
    assert_eq!(config.npc.default_corpse_time, m4_data::CORPSE_SECONDS);
    // Left at the Java defaults, which are the shipped values.
    assert!(config.player.player_delevel);
    assert_eq!(config.player.max_offset_on_teleport, 50);
    assert!(!config.player.auto_loot);
}
