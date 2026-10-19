#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! HP, MP and CP regeneration on the world (`status.rs`): the regeneration task on the timer
//! wheel, the status listeners and the `StatusUpdate` packets.
//!
//! Most tests use synthetic templates written inline, so they run in CI. The vector test reads the
//! datapack (`C:\l2-rust\l2jmobius\dist\game`) and `java-vectors/out/formulas_regen.json` and skips
//! with a message when either is missing.
// Java: entity/actor/status/{CreatureStatus,PlayerStatus}.java, mechanics/stats/Formulas.java
//       (calcHpRegen, calcMpRegen, calcCpRegen, getRegeneratePeriod)

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use l2_data::{
    ClassId, ExperienceTable, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions,
    SkillTemplates, SpawnData,
};
use l2_util::ObjectId;
use l2_world::*;
use serde_json::Value;

/// Java `StatusUpdate.MAX_HP` and friends (`attr` ids).
const MAX_HP: i32 = 0x0a;
const CUR_HP: i32 = 0x09;
const MAX_MP: i32 = 0x0c;
const CUR_MP: i32 = 0x0b;
const MAX_CP: i32 = 0x22;
const CUR_CP: i32 = 0x21;

/// `ServerPackets.STATUS_UPDATE`.
const STATUS_UPDATE: u8 = 0x18;

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

/// Packets sent to one connection, tagged with the player's object id.
struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

/// The static data the world reads: a stat table and nothing else.
struct Data(StatData);

impl StaticData for Data {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

/// A class template with one level row per level: HP `10 * level + 0.5`, MP `level + 0.25`,
/// CP `level + 0.75`, regeneration 2.0 / 0.9 / 1.5 per period (the shape of Java's
/// `playerTemplate.xsd`, with only the values the regeneration reads varied).
fn player_xml(class_id: i32, levels: usize) -> String {
    let mut rows = String::new();
    for l in 1..=levels {
        rows.push_str(&format!(
            "<level val=\"{l}\"><hp>{}.5</hp><mp>{l}.25</mp><cp>{l}.75</cp>\
             <hpRegen>2.0</hpRegen><mpRegen>0.9</mpRegen><cpRegen>1.5</cpRegen></level>",
            l * 10
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<classId>{class_id}</classId>
	<staticData>
		<baseINT>25</baseINT><baseSTR>41</baseSTR><baseCON>32</baseCON>
		<baseMEN>26</baseMEN><baseDEX>34</baseDEX><baseWIT>12</baseWIT>
		<creationPoints>
			<node x="28295" y="11063" z="-4224" />
		</creationPoints>
		<basePAtk>4</basePAtk><baseCritRate>5</baseCritRate><baseAtkType>FIST</baseAtkType>
		<basePAtkSpd>330</basePAtkSpd>
		<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
		<baseMAtk>6</baseMAtk>
		<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
		<baseAtkRange>20</baseAtkRange>
		<baseRndDam>10</baseRndDam>
		<baseMoveSpd><walk>85</walk><run>122</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
		<baseBreath>150</baseBreath>
		<baseSafeFall>350</baseSafeFall>
		<someFutureElement><x>1</x></someFutureElement>
		<collisionMale><radius>7.5</radius><height>24</height></collisionMale>
		<collisionFemale><radius>7.0</radius><height>23.5</height></collisionFemale>
	</staticData>
	<lvlUpgainData>{rows}</lvlUpgainData>
</list>"#
    )
}

/// Test monsters and folk: 1000 HP (5 per period), 200 MP (2 per period); 9003 has 3520 HP.
const NPC_XML: &str = r#"<?xml version="1.0"?>
<list>
	<npc id="9001" type="Monster" level="10" name="Test Monster">
		<stats><vitals hp="1000" hpRegen="5" mp="200" mpRegen="2" /></stats>
	</npc>
	<npc id="9002" type="Folk" level="10" name="Test Folk">
		<stats><vitals hp="1000" hpRegen="5" mp="200" mpRegen="2" /></stats>
	</npc>
	<npc id="9003" type="Monster" level="10" name="Test Big Monster">
		<stats><vitals hp="3520" hpRegen="5" mp="200" mpRegen="2" /></stats>
	</npc>
</list>"#;

const PLAYER: ObjectId = ObjectId(0x1000_0001);
const OTHER: ObjectId = ObjectId(0x1000_0002);
const THIRD: ObjectId = ObjectId(0x1000_0003);

/// A world with the synthetic templates, sinks per player and the id factory of the game server.
fn world() -> World {
    let players = PlayerTemplates::from_xml(&player_xml(31, 12), 85).unwrap();
    let npcs = NpcTemplates::from_xml(NPC_XML, &NpcLoadOptions::default()).unwrap();
    let mut world = World::new(WorldRng::from_seed(7), Box::new(NullOutbox));
    world.set_static_data(Box::new(Data(StatData {
        player_templates: Arc::new(players),
        ..StatData::default()
    })));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_npc_data(Arc::new(npcs), Arc::new(SpawnData::default()));
    world
}

/// A level 10 dark elf fighter (class 31) at (10000, 10000), with the vitals given.
fn player_data(object_id: ObjectId, name: &str, cur_hp: f64, cur_mp: f64, cur_cp: f64) -> PlayerData {
    PlayerData {
        object_id,
        account: "acc".into(),
        name: name.into(),
        title: String::new(),
        class_id: 31,
        base_class: 31,
        race: 2,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 10,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: 10_000,
        y: 10_000,
        z: -1_000,
        heading: 0,
        cur_hp,
        max_hp: 0.0,
        cur_mp,
        max_mp: 0.0,
        cur_cp,
        max_cp: 0.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn enter(world: &mut World, log: &Log, session: u64, data: PlayerData) {
    let id = data.object_id;
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(session),
        player: Box::new(data),
        sink: Box::new(Sink(id.0, log.clone())),
        ip: None,
    });
}

fn tick(world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        world.tick();
    }
}

/// The attributes of a `StatusUpdate` packet: its opcode and the `(id, value)` pairs.
fn status_update(bytes: &[u8]) -> (u8, Vec<(i32, i32)>) {
    let opcode = bytes[0];
    let count = i32::from_le_bytes(bytes[5..9].try_into().unwrap()) as usize;
    let pairs = (0..count)
        .map(|i| {
            let at = 9 + i * 8;
            (
                i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()),
                i32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()),
            )
        })
        .collect();
    (opcode, pairs)
}

/// The `StatusUpdate` packets sent to `to`.
fn updates_to(sent: &Sent, to: ObjectId) -> Vec<Vec<(i32, i32)>> {
    sent.iter()
        .filter(|(conn, bytes)| *conn == to.0 && bytes[0] == STATUS_UPDATE)
        .map(|(_, bytes)| status_update(bytes).1)
        .collect()
}

/// Spawns an NPC of the synthetic table at the player's position and returns its id.
fn spawn(world: &mut World, npc_id: i32) -> ObjectId {
    world.spawn_npc_at(npc_id, 10_000, 10_000, -1_000, 0).unwrap()
}

/// The stored HP, MP and CP of a player or an NPC.
fn hp(world: &World, id: ObjectId) -> f64 {
    world.stats(id).unwrap().core().cur_hp
}

fn mp(world: &World, id: ObjectId) -> f64 {
    world.stats(id).unwrap().core().cur_mp
}

fn cp(world: &World, id: ObjectId) -> f64 {
    world.stats(id).unwrap().core().cur_cp
}

// ------------------------------------------------------------------ the task on the timer wheel

#[test]
fn a_hurt_player_regenerates_once_per_three_seconds() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 1.0, 10.0, 10.0));
    log.take();
    // Level 10: 2.0 * (1 + 89) / 100 * CON bonus 1.0 standing (x1.1), with no stat funcs.
    let per_period = 2.0 * 0.99 * 1.1;
    assert!(
        w.is_hp_mp_regenerating(PLAYER),
        "restore starts the task below the maximum"
    );
    tick(&mut w, 59);
    assert_eq!(hp(&w, PLAYER), 1.0, "nothing before the first period");
    tick(&mut w, 1);
    assert!(
        (hp(&w, PLAYER) - (1.0 + per_period)).abs() < 1e-9,
        "{}",
        hp(&w, PLAYER)
    );
    tick(&mut w, 60);
    assert!(
        (hp(&w, PLAYER) - (1.0 + 2.0 * per_period)).abs() < 1e-9,
        "{}",
        hp(&w, PLAYER)
    );
}

#[test]
fn a_task_stops_when_the_vitals_were_filled_without_the_setters() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 1.0, 10.0, 10.0));
    assert!(w.is_hp_mp_regenerating(PLAYER));
    // An admin heal writes the vitals directly (`admin/commands.rs`), so no setter runs.
    w.player_mut(PLAYER).unwrap().core.cur_hp = 100.0;
    log.take();
    tick(&mut w, 60);
    assert!(
        !w.is_hp_mp_regenerating(PLAYER),
        "nothing is below its maximum any more"
    );
    assert!(
        updates_to(&log.take(), PLAYER).is_empty(),
        "and no packet goes out for it"
    );
}

#[test]
fn a_full_player_starts_no_task_until_it_is_hurt() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 100.0, 10.0, 10.0));
    assert!(!w.is_hp_mp_regenerating(PLAYER));
    w.set_current_hp(PLAYER, 50.0, true);
    assert!(w.is_hp_mp_regenerating(PLAYER));
    // Back to the maximum: the last flag clears and the task stops.
    assert!(w.set_current_hp(PLAYER, 100.0, true));
    assert!(!w.is_hp_mp_regenerating(PLAYER));
}

#[test]
fn stop_cancels_the_pending_tick() {
    let mut w = world();
    let npc = spawn(&mut w, 9001);
    let before = w.timers().len();
    w.set_current_hp(npc, 500.0, false);
    assert!(w.is_hp_mp_regenerating(npc));
    assert_eq!(w.timers().len(), before + 1, "one timer for the task");
    w.stop_hp_mp_regeneration(npc);
    assert!(!w.is_hp_mp_regenerating(npc));
    assert_eq!(w.timers().len(), before, "the timer is cancelled");
    tick(&mut w, 120);
    assert_eq!(hp(&w, npc), 500.0, "a stopped task never adds HP");
}

#[test]
fn starting_twice_keeps_one_task() {
    let mut w = world();
    let npc = spawn(&mut w, 9001);
    w.set_current_hp(npc, 500.0, false);
    w.start_hp_mp_regeneration(npc);
    tick(&mut w, 60);
    assert_eq!(hp(&w, npc), 505.0, "one period, not two");
    tick(&mut w, 60);
    assert_eq!(hp(&w, npc), 510.0, "the fixed rate goes on");
}

#[test]
fn the_task_stops_by_itself_when_the_last_vital_is_full() {
    let mut w = world();
    let npc = spawn(&mut w, 9001);
    // 999 HP: one period (+5) fills it, and the task stops with the last flag.
    w.set_current_hp_mp(npc, 999.0, 200.0);
    assert!(w.is_hp_mp_regenerating(npc));
    tick(&mut w, 60);
    assert_eq!(hp(&w, npc), 1000.0);
    assert!(!w.is_hp_mp_regenerating(npc));
    let timers = w.timers().len();
    tick(&mut w, 120);
    assert_eq!(w.timers().len(), timers, "no tick is left behind");
}

#[test]
fn the_dead_neither_regenerate_nor_change() {
    let mut w = world();
    let npc = spawn(&mut w, 9001);
    w.set_dead(npc, true);
    assert!(
        !w.set_current_hp(npc, 500.0, true),
        "vitals do not change while dead"
    );
    assert_eq!(hp(&w, npc), 1000.0);
    w.start_hp_mp_regeneration(npc);
    assert!(!w.is_hp_mp_regenerating(npc));
    w.set_dead(npc, false);
    w.set_current_hp(npc, 500.0, false);
    assert!(w.is_hp_mp_regenerating(npc));
}

#[test]
fn a_hurt_player_restored_at_zero_hp_is_dead_and_does_not_regenerate() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 0.0, 10.0, 10.0));
    assert!(
        !w.is_hp_mp_regenerating(PLAYER),
        "Java: restore sets dead and stops the task"
    );
    tick(&mut w, 120);
    assert_eq!(hp(&w, PLAYER), 0.0);
}

#[test]
fn an_attackable_starts_regenerating_when_its_region_activates() {
    let mut w = world();
    let monster = spawn(&mut w, 9001);
    let folk = spawn(&mut w, 9002);
    assert!(!w.is_hp_mp_regenerating(monster), "nothing is active yet");
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 100.0, 10.0, 10.0));
    // Java `WorldRegion.setActive(true)` -> `switchAI(true)`: attackables start their task.
    assert!(w.is_hp_mp_regenerating(monster));
    assert!(!w.is_hp_mp_regenerating(folk), "a folk NPC is not attackable");
    tick(&mut w, 60);
    assert!(
        !w.is_hp_mp_regenerating(monster),
        "full, so it stops at the first tick"
    );
}

// ------------------------------------------------------------------ status listeners and packets

#[test]
fn a_listener_gets_the_hp_bar_of_an_npc_and_others_do_not() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 100.0, 10.0, 10.0));
    enter(&mut w, &log, 2, player_data(OTHER, "Other", 100.0, 10.0, 10.0));
    enter(&mut w, &log, 3, player_data(THIRD, "Third", 100.0, 10.0, 10.0));
    let npc = spawn(&mut w, 9001);
    w.add_status_listener(npc, PLAYER);
    log.take();
    assert!(w.set_current_hp(npc, 500.0, true));
    let sent = log.take();
    assert_eq!(
        updates_to(&sent, PLAYER),
        vec![vec![(MAX_HP, 1000), (CUR_HP, 500)]]
    );
    assert!(updates_to(&sent, OTHER).is_empty());
    assert!(updates_to(&sent, THIRD).is_empty());
}

#[test]
fn removing_a_listener_stops_its_updates() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 100.0, 10.0, 10.0));
    let npc = spawn(&mut w, 9001);
    w.add_status_listener(npc, PLAYER);
    w.remove_status_listener(npc, PLAYER);
    log.take();
    w.set_current_hp(npc, 500.0, true);
    assert!(updates_to(&log.take(), PLAYER).is_empty());
}

#[test]
fn a_small_change_of_an_npc_is_not_sent_to_listeners() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 100.0, 10.0, 10.0));
    // 3520 HP: the update windows are 10 HP wide (Java `needHpUpdate`, `_hpUpdateInterval`).
    let npc = spawn(&mut w, 9003);
    w.add_status_listener(npc, PLAYER);
    w.set_current_hp(npc, 2000.0, true);
    log.take();
    // Inside the window [2000, 2010): no update.
    w.set_current_hp(npc, 2005.0, true);
    assert!(updates_to(&log.take(), PLAYER).is_empty());
    // Past the window's top: an update, with the new value.
    w.set_current_hp(npc, 2010.0, true);
    assert_eq!(
        updates_to(&log.take(), PLAYER),
        vec![vec![(MAX_HP, 3520), (CUR_HP, 2010)]]
    );
}

#[test]
fn players_send_their_vitals_to_themselves_one_tick_after_the_regeneration() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 1.0, 10.0, 10.0));
    log.take();
    tick(&mut w, 60);
    // Java `PlayerStatus.doRegeneration` ends with `broadcastStatusUpdate`, which is a 50 ms task.
    assert!(updates_to(&log.take(), PLAYER).is_empty());
    tick(&mut w, 1);
    let sent = log.take();
    let updates = updates_to(&sent, PLAYER);
    assert_eq!(updates.len(), 1, "{updates:?}");
    let cur_hp = hp(&w, PLAYER) as i32;
    assert_eq!(
        updates[0],
        vec![
            (MAX_HP, 100),
            (CUR_HP, cur_hp),
            (MAX_MP, 10),
            (CUR_MP, 10),
            (MAX_CP, 10),
            (CUR_CP, 10),
        ]
    );
}

#[test]
fn a_players_listeners_are_not_sent_its_hp_java_quirk() {
    let mut w = world();
    let log = Log::default();
    enter(&mut w, &log, 1, player_data(PLAYER, "Hero", 100.0, 10.0, 10.0));
    enter(&mut w, &log, 2, player_data(OTHER, "Other", 100.0, 10.0, 10.0));
    w.add_status_listener(PLAYER, OTHER);
    log.take();
    w.set_current_hp(PLAYER, 50.0, true);
    tick(&mut w, 1);
    let sent = log.take();
    // Java's `Player.broadcastStatusUpdate` overrides the NPC version and never reaches listeners.
    assert!(updates_to(&sent, OTHER).is_empty());
    assert_eq!(updates_to(&sent, PLAYER).len(), 1);
}

// ------------------------------------------------------------------ datapack vectors

/// The datapack as the shipped config loads it (the same root as `stats_vectors.rs`).
fn game_root() -> Option<PathBuf> {
    let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    if root.join("data/stats/skills").is_dir() {
        Some(root)
    } else {
        eprintln!("skipped: {} has no datapack", root.display());
        None
    }
}

fn vectors(name: &str) -> Option<Value> {
    let path = l2_testkit::vectors::vector_path(name);
    if !path.is_file() {
        eprintln!("skipped: {} is missing", path.display());
        return None;
    }
    Some(l2_testkit::vectors::load(name))
}

struct Tables {
    stats: StatData,
    npcs: Arc<NpcTemplates>,
}

fn tables(root: &Path) -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let max_level = ExperienceTable::load(root)
            .max_level()
            .unwrap_or(l2_data::DEFAULT_MAX_PLAYER_LEVEL);
        let skills = SkillTemplates::load(
            root,
            &SkillLoadOptions {
                custom_skills_load: true,
                ..SkillLoadOptions::default()
            },
        );
        let mut bonus = BaseStatBonus::loaded();
        let base = vectors("stats_base_bonus").unwrap();
        for (key, stat) in [
            ("str", BaseStat::Str),
            ("int", BaseStat::Int),
            ("dex", BaseStat::Dex),
            ("wit", BaseStat::Wit),
            ("con", BaseStat::Con),
            ("men", BaseStat::Men),
        ] {
            for (value, b) in base[key].as_array().unwrap().iter().enumerate() {
                assert!(bonus.set(stat, value, b.as_f64().unwrap()));
            }
        }
        Tables {
            stats: StatData {
                player_templates: Arc::new(PlayerTemplates::load(root, max_level)),
                base_stat_bonus: Arc::new(bonus),
                skills: Arc::new(skills),
                ..StatData::default()
            },
            npcs: Arc::new(NpcTemplates::load(
                root,
                &NpcLoadOptions {
                    custom_npc_data: true,
                    ..NpcLoadOptions::default()
                },
            )),
        }
    })
}

/// `calcHpRegen` and friends compare to the bit, except that a stat rebuilt from a probe may differ
/// in the last digits.
fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= expected.abs() * 1e-12 + 1e-15
}

/// Java's regeneration rates for the standing creatures of `formulas_regen.json` (no gear, no
/// skills, multipliers 1.0 as in the vector config): one period of the task, measured on the
/// world. Each case starts from 1 HP / 1 MP / 1 CP and runs 60 ticks (one period).
#[test]
fn regeneration_matches_the_java_vectors() {
    let Some(root) = game_root() else {
        return;
    };
    let Some(regen) = vectors("formulas_regen") else {
        return;
    };
    let Some(creatures) = vectors("formulas_creatures") else {
        return;
    };
    // The vector config is Java's default: every multiplier 1.0, which is what the world defaults to.
    for key in [
        "HpRegenMultiplier",
        "MpRegenMultiplier",
        "CpRegenMultiplier",
        "RaidHpRegenMultiplier",
        "RaidMpRegenMultiplier",
    ] {
        assert_eq!(creatures["config"][key].as_f64(), Some(1.0), "{key}");
    }
    let t = tables(&root);
    let mut checked = 0;
    for case in regen["cases"].as_array().unwrap() {
        let name = case["creature"].as_str().unwrap();
        if case["state"] != "stand" || !(name == "newbie1" || name.starts_with("npc_")) {
            continue;
        }
        let log = Log::default();
        let mut w = World::new(WorldRng::from_seed(1), Box::new(NullOutbox));
        w.set_static_data(Box::new(Data(t.stats.clone())));
        w.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        w.set_npc_data(Arc::clone(&t.npcs), Arc::new(SpawnData::default()));
        let creature = creatures["creatures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap();
        let input = &creature["input"];
        let id = if input["kind"] == "npc" {
            let npc_id = input["npc_id"].as_i64().unwrap() as i32;
            let id = w.spawn_npc_at(npc_id, 10_000, 10_000, -1_000, 0).unwrap();
            w.set_current_hp_mp(id, 1.0, 1.0);
            id
        } else {
            let class_id = input["class_id"].as_i64().unwrap() as i32;
            let mut data = player_data(PLAYER, "Vector", 1.0, 1.0, 1.0);
            data.class_id = class_id;
            data.base_class = class_id;
            data.level = input["level"].as_i64().unwrap() as i32;
            data.race = i32::from(ClassId(u16::try_from(class_id).unwrap()).race().unwrap() as u8);
            enter(&mut w, &log, 1, data);
            PLAYER
        };
        tick(&mut w, 60);
        let gained = (hp(&w, id) - 1.0, mp(&w, id) - 1.0, cp(&w, id) - 1.0);
        let (want_hp, want_mp) = (case["hp"].as_f64().unwrap(), case["mp"].as_f64().unwrap());
        assert!(
            close(gained.0, want_hp),
            "{name}: hp {} vs Java {want_hp}",
            gained.0
        );
        assert!(
            close(gained.1, want_mp),
            "{name}: mp {} vs Java {want_mp}",
            gained.1
        );
        if input["kind"] != "npc" {
            let want_cp = case["cp"].as_f64().unwrap();
            assert!(
                close(gained.2, want_cp),
                "{name}: cp {} vs Java {want_cp}",
                gained.2
            );
        }
        checked += 1;
    }
    eprintln!("regeneration: {checked} vector cases");
    assert!(checked > 0);
}
