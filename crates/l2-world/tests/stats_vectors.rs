#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The stat engine against the Java vectors of M4-JV-01 (`java-vectors/out/stats_*.json`): every
//! `UserInfo` / `NpcInfo` number, the raw doubles behind them, the 31 `calcStat` probes, the base
//! stat bonus tables and the function lists of `stats_func_trace.json`.
//!
//! The creatures are built from the real datapack (skipped when `dist/game`
//! or `L2JMOBIUS_GAME_DIR` is missing, like in CI) the way the Java harness built them: the gear
//! equipped in the listed order (the paperdoll slot as `Inventory.equipItem` picks it), then the
//! listed skills in order, then the weight and grade penalties.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use l2_data::item::BodyPart;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceTable, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillLoadOptions, SkillTemplates, SpawnData, Stat,
};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_world::stats::{FuncCalc, FuncOwner, StdFunc};
use l2_world::*;
use serde_json::Value;

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap_or(5),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: -1,
            weapon: t.is_weapon(),
        })
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

struct NoSink;

impl PacketSink for NoSink {
    fn send(&self, _bytes: Vec<u8>) {}
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Tables {
    stats: StatData,
    npcs: Arc<NpcTemplates>,
}

fn game_root() -> Option<PathBuf> {
    let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
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

/// The base stat bonus of the vectors (checked against the datapack in its own test).
fn bonus_from(vectors: &Value) -> BaseStatBonus {
    let mut bonus = BaseStatBonus::loaded();
    for (key, stat) in [
        ("str", BaseStat::Str),
        ("int", BaseStat::Int),
        ("dex", BaseStat::Dex),
        ("wit", BaseStat::Wit),
        ("con", BaseStat::Con),
        ("men", BaseStat::Men),
    ] {
        for (value, b) in vectors[key].as_array().unwrap().iter().enumerate() {
            assert!(bonus.set(stat, value, b.as_f64().unwrap()));
        }
    }
    bonus
}

/// The datapack as the shipped config loads it (custom items, NPCs and skills on).
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
        let bonus = bonus_from(&vectors("stats_base_bonus").unwrap());
        Tables {
            stats: StatData {
                items: Arc::new(ItemTemplates::load(root, true)),
                player_templates: Arc::new(PlayerTemplates::load(root, max_level)),
                armor_sets: Arc::new(ArmorSets::load(root)),
                enchant_hp_bonus: Arc::new(EnchantHpBonus::load(root)),
                hennas: Arc::new(Hennas::load(root)),
                base_stat_bonus: Arc::new(bonus),
                skills: Arc::new(skills),
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

fn new_world(t: &Tables) -> World {
    let mut world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
    world.set_static_data(Box::new(Data(t.stats.clone())));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_npc_data(Arc::clone(&t.npcs), Arc::new(SpawnData::default()));
    world
}

const ME: ObjectId = ObjectId(0x2000_0001);

/// `Inventory.equipItem`'s choice of slot, with the items it takes off.
fn equip(items: &mut Vec<ItemData>, t: &Tables, object_id: i32, item_id: i32, enchant: i32) {
    let template = t.stats.items.get(item_id).unwrap();
    let occupied = |items: &Vec<ItemData>, slot: usize| items.iter().any(|i| i.loc_data == slot as i32);
    let mut clear = Vec::new();
    let slot = match template.body_part {
        BodyPart::BothHands => {
            clear.push(paperdoll::LHAND);
            paperdoll::RHAND
        }
        BodyPart::RightHand => paperdoll::RHAND,
        BodyPart::LeftHand => {
            let rh_two_handed = items
                .iter()
                .find(|i| i.loc_data == paperdoll::RHAND as i32)
                .and_then(|i| t.stats.items.get(i.item_id))
                .is_some_and(|w| w.body_part == BodyPart::BothHands);
            if rh_two_handed {
                clear.push(paperdoll::RHAND);
            }
            paperdoll::LHAND
        }
        BodyPart::LeftEar | BodyPart::RightEar | BodyPart::BothEars => {
            if !occupied(items, paperdoll::LEAR) {
                paperdoll::LEAR
            } else if !occupied(items, paperdoll::REAR) {
                paperdoll::REAR
            } else {
                paperdoll::LEAR
            }
        }
        BodyPart::LeftFinger | BodyPart::RightFinger | BodyPart::BothFingers => {
            if !occupied(items, paperdoll::LFINGER) {
                paperdoll::LFINGER
            } else if !occupied(items, paperdoll::RFINGER) {
                paperdoll::RFINGER
            } else {
                paperdoll::LFINGER
            }
        }
        BodyPart::Neck => paperdoll::NECK,
        BodyPart::FullArmor => {
            clear.push(paperdoll::LEGS);
            paperdoll::CHEST
        }
        BodyPart::Chest => paperdoll::CHEST,
        BodyPart::Legs => paperdoll::LEGS,
        BodyPart::Feet => paperdoll::FEET,
        BodyPart::Gloves => paperdoll::GLOVES,
        BodyPart::Head => paperdoll::HEAD,
        BodyPart::Back => paperdoll::CLOAK,
        BodyPart::Underwear => paperdoll::UNDER,
        other => panic!("no slot for {other:?} in the vector gear"),
    };
    clear.push(slot);
    items.retain(|i| !clear.contains(&usize::try_from(i.loc_data).unwrap()));
    items.push(ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count: 1,
        enchant_level: enchant,
        loc: "PAPERDOLL".into(),
        loc_data: slot as i32,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    });
}

/// The creature of a vector input; returns its id and the world it lives in.
fn build(t: &Tables, input: &Value) -> (World, ObjectId) {
    let mut world = new_world(t);
    if input["kind"] == "npc" {
        let npc_id = i32::try_from(input["npc_id"].as_i64().unwrap()).unwrap();
        let id = world.spawn_npc_at(npc_id, 10_000, 10_000, -1_000, 0).unwrap();
        return (world, id);
    }
    let mut items = Vec::new();
    for (n, item) in input["items"].as_array().unwrap().iter().enumerate() {
        equip(
            &mut items,
            t,
            0x3000_0000 + i32::try_from(n).unwrap(),
            i32::try_from(item["item_id"].as_i64().unwrap()).unwrap(),
            i32::try_from(item["enchant"].as_i64().unwrap()).unwrap(),
        );
    }
    let skills = input["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| SkillData {
            skill_id: i32::try_from(s["id"].as_i64().unwrap()).unwrap(),
            skill_level: i32::try_from(s["level"].as_i64().unwrap()).unwrap(),
        })
        .collect();
    let class_id = i32::try_from(input["class_id"].as_i64().unwrap()).unwrap();
    let player = PlayerData {
        object_id: ME,
        account: "acc".into(),
        name: "Vector".into(),
        title: String::new(),
        class_id,
        base_class: class_id,
        // The character table stores the race of the class.
        race: i32::from(l2_data::ClassId(u16::try_from(class_id).unwrap()).race().unwrap() as u8),
        sex: i32::from(input["female"].as_bool().unwrap()),
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: i32::try_from(input["level"].as_i64().unwrap()).unwrap(),
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
        cur_hp: 1e9,
        max_hp: 1e9,
        cur_mp: 1e9,
        max_mp: 1e9,
        cur_cp: 1e9,
        max_cp: 1e9,
        online_time: 0,
        items,
        skills,
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    };
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(player),
        sink: Box::new(NoSink),
        ip: None,
    });
    // The harness never makes its players run (`Creature._isRunning` starts false; the game sets
    // it when the client enters), so they walk, which decides the movement multiplier.
    world.player_mut(ME).unwrap().movement.running = false;
    (world, ME)
}

fn stat_named(name: &str) -> Stat {
    *Stat::ALL
        .iter()
        .find(|s| s.java_name().eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no stat {name}"))
}

/// The `UserInfo` / `NpcInfo` numbers and raw values of a creature, as the vectors name them.
/// `(name, value)` pairs: the `UserInfo` ints and the raw doubles.
type Observed = (Vec<(String, i64)>, Vec<(String, f64)>);

fn observe(world: &World, id: ObjectId, is_player: bool) -> Observed {
    let s = world.stats(id).unwrap();
    let mult = s.movement_speed_multiplier();
    let speed = |v: f64| ((v / mult) + 0.5).floor() as i64;
    let mut ints: Vec<(String, i64)> = vec![
        ("str".into(), s.str().into()),
        ("dex".into(), s.dex().into()),
        ("con".into(), s.con().into()),
        ("int".into(), s.int().into()),
        ("wit".into(), s.wit().into()),
        ("men".into(), s.men().into()),
        ("max_hp".into(), s.max_hp().into()),
        ("max_mp".into(), s.max_mp().into()),
        ("p_atk".into(), (s.p_atk(None) as i32).into()),
        ("p_atk_spd".into(), (s.p_atk_spd() as i32).into()),
        ("p_def".into(), (s.p_def(None) as i32).into()),
        ("evasion".into(), s.evasion_rate(None).into()),
        ("accuracy".into(), s.accuracy().into()),
        ("critical".into(), s.critical_hit(None, None).into()),
        ("m_atk".into(), (s.m_atk(None, None) as i32).into()),
        ("m_atk_spd".into(), s.m_atk_spd().into()),
        ("m_def".into(), (s.m_def(None, None) as i32).into()),
        ("run_speed".into(), speed(s.run_speed())),
        ("walk_speed".into(), speed(s.walk_speed())),
        ("swim_run_speed".into(), speed(s.swim_run_speed())),
        ("swim_walk_speed".into(), speed(s.swim_walk_speed())),
        ("attack_range".into(), s.physical_attack_range().into()),
    ];
    if is_player {
        ints.push(("max_cp".into(), s.max_cp().into()));
        ints.push(("max_load".into(), s.max_load().into()));
    }
    let mut raw: Vec<(String, f64)> = vec![
        ("p_atk".into(), s.p_atk(None)),
        ("p_atk_spd".into(), s.p_atk_spd()),
        ("p_def".into(), s.p_def(None)),
        ("m_atk".into(), s.m_atk(None, None)),
        ("m_def".into(), s.m_def(None, None)),
        ("run_speed".into(), s.run_speed()),
        ("walk_speed".into(), s.walk_speed()),
        ("swim_run_speed".into(), s.swim_run_speed()),
        ("swim_walk_speed".into(), s.swim_walk_speed()),
        ("movement_speed_multiplier".into(), mult),
        (
            "attack_speed_multiplier".into(),
            f64::from(s.attack_speed_multiplier()),
        ),
        ("m_critical".into(), s.m_critical_hit(None, None).into()),
        ("shield_def".into(), s.shld_def().into()),
        ("level_mod".into(), s.level_mod()),
        ("level".into(), s.level().into()),
        ("collision_radius".into(), f64::from(s.collision_radius())),
        ("collision_height".into(), f64::from(s.collision_height())),
    ];
    if is_player {
        raw.push(("inventory_limit".into(), s.inventory_limit().into()));
        raw.push(("current_load".into(), s.current_load().into()));
    }
    for (key, stat) in [
        ("str", BaseStat::Str),
        ("dex", BaseStat::Dex),
        ("con", BaseStat::Con),
        ("int", BaseStat::Int),
        ("wit", BaseStat::Wit),
        ("men", BaseStat::Men),
    ] {
        raw.push((format!("base_stat_bonus.{key}"), s.bonus(stat)));
    }
    (ints, raw)
}

/// Compares every case of a stats file; returns the mismatches.
fn check_file(name: &str) -> Option<(usize, Vec<String>)> {
    let root = game_root()?;
    let file = vectors(name)?;
    let t = tables(&root);
    let mut errors = Vec::new();
    let cases = file["cases"].as_array().unwrap();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let input = &case["input"];
        let is_player = input["kind"] == "player";
        let (world, me) = build(t, input);
        let (ints, raw) = observe(&world, me, is_player);
        let expected = &case["expected"];
        for (key, ours) in ints {
            let java = expected["user_info"][&key].as_i64().unwrap();
            if java != ours {
                errors.push(format!("{id}: user_info.{key} java {java} rust {ours}"));
            }
        }
        for (key, ours) in raw {
            let java = match key.split_once('.') {
                Some((a, b)) => expected["raw"][a][b].as_f64(),
                None => expected["raw"][&key].as_f64(),
            }
            .unwrap();
            // Java floats (the attack speed multiplier, the collision) are written as float text.
            let is_float = key == "attack_speed_multiplier" || key.starts_with("collision");
            let same = if is_float {
                java as f32 == ours as f32
            } else {
                java == ours
            };
            if !same {
                errors.push(format!("{id}: raw.{key} java {java} rust {ours}"));
            }
        }
        let s = world.stats(me).unwrap();
        for (key, value) in expected["raw"]["probes"].as_object().unwrap() {
            let (stat, init) = key.split_once('@').unwrap();
            let init: f64 = init.parse().unwrap();
            let ours = s.calc(stat_named(stat), init);
            let java = value.as_f64().unwrap();
            if java != ours {
                errors.push(format!("{id}: probe {key} java {java} rust {ours}"));
            }
        }
        if !is_player {
            // NPC_STD_CALCULATOR is shared until a template skill adds functions (copy on write).
            let calcs = &s.core().calculators;
            let own = Stat::ALL
                .iter()
                .any(|&st| calcs.funcs(st).iter().any(|f| f.owner != FuncOwner::Std));
            if calcs.is_shared() == own {
                errors.push(format!(
                    "{id}: shared calculators {} with {} functions",
                    calcs.is_shared(),
                    calcs.set().len()
                ));
            }
        }
        if is_player {
            // The skills the player knows on top of the listed ones (armor set, item, penalties).
            for skill in input["derived_skills"].as_array().unwrap() {
                let (sid, level) = (skill["id"].as_i64().unwrap(), skill["level"].as_i64().unwrap());
                let ours = i64::from(s.skill_level(i32::try_from(sid).unwrap()));
                if ours != level {
                    errors.push(format!(
                        "{id}: derived skill {sid} java level {level} rust {ours}"
                    ));
                }
            }
        }
    }
    Some((cases.len(), errors))
}

fn assert_file(name: &str) {
    let Some((count, errors)) = check_file(name) else {
        return;
    };
    assert!(count > 0);
    assert!(
        errors.is_empty(),
        "{} mismatches in {count} cases of {name}:\n{}",
        errors.len(),
        errors.iter().take(80).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn naked_players_match_java() {
    assert_file("stats_players_naked");
}

#[test]
fn players_with_gear_match_java() {
    assert_file("stats_players_gear");
}

#[test]
fn npcs_match_java() {
    assert_file("stats_npcs");
}

/// `statBonus.xml` read like Java reads it gives the tables of the vectors.
#[test]
fn base_stat_bonus_tables_match_java() {
    let (Some(root), Some(file)) = (game_root(), vectors("stats_base_bonus")) else {
        return;
    };
    let from_vectors = bonus_from(&file);
    let text = std::fs::read_to_string(root.join("data/stats/statBonus.xml")).unwrap();
    // A minimal reader of the file (the game server's loader is in `game-server`).
    let mut section = None;
    let mut ours = BaseStatBonus::loaded();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('<').and_then(|l| l.strip_suffix('>'))
            && let Some(stat) = BaseStat::from_section(name)
        {
            section = Some(stat);
        }
        if let (Some(stat), Some(rest)) = (section, line.strip_prefix("<stat value=\"")) {
            let (value, rest) = rest.split_once('"').unwrap();
            let bonus = rest.split("bonus=\"").nth(1).unwrap().split('"').next().unwrap();
            assert!(ours.set(stat, value.parse().unwrap(), bonus.parse().unwrap()));
        }
    }
    assert_eq!(ours, from_vectors);
}

/// Java's class name of a function.
fn class_of(calc: FuncCalc) -> String {
    match calc {
        FuncCalc::Template(kind) => format!("Func{}", kind.name()),
        FuncCalc::Std(f) => match f {
            StdFunc::Henna(_) => "FuncHenna".into(),
            StdFunc::ArmorSet(_) => "FuncArmorSet".into(),
            other => format!("Func{other:?}"),
        },
    }
}

/// Java's class name of a condition (`ConditionLogicNot`, `ConditionUsingItemType`, ...).
fn cond_class(cond: &l2_data::Condition) -> String {
    let debug = format!("{cond:?}");
    let name = debug
        .split(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or_default();
    match name {
        "And" | "Or" | "Not" => format!("ConditionLogic{name}"),
        other => format!("Condition{other}"),
    }
}

/// The function lists of `stats_func_trace.json`: class, order, value, owner, condition.
#[test]
fn function_lists_match_the_java_trace() {
    let (Some(root), Some(file)) = (game_root(), vectors("stats_func_trace")) else {
        return;
    };
    let t = tables(&root);
    let mut errors = Vec::new();
    for case in file["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let (world, me) = build(t, &case["input"]);
        let s = world.stats(me).unwrap();
        let items: Vec<ItemData> = world.player(me).map(|p| p.data.items.clone()).unwrap_or_default();
        let calcs = &s.core().calculators;
        for &stat in Stat::ALL {
            let ours: Vec<String> = calcs
                .funcs(stat)
                .iter()
                .map(|f| {
                    let owner = match f.owner {
                        FuncOwner::Std => "null".to_owned(),
                        FuncOwner::Item(oid) => format!(
                            "item:{}",
                            items.iter().find(|i| i.object_id == oid).unwrap().item_id
                        ),
                        FuncOwner::Skill { id, level } => format!("skill:{id}:{level}"),
                        FuncOwner::ArmorSet(chest) => format!("set:{chest}"),
                        // Items without augmentation or elements (the trace has none).
                        FuncOwner::ItemElement(_) | FuncOwner::AugmentationOption(_) => {
                            "item_attribute".to_owned()
                        }
                        // Java prints an effect owner as its skill and its handler name.
                        FuncOwner::Effect {
                            skill_id,
                            skill_level,
                            scope,
                            index,
                        } => {
                            let name = t
                                .stats
                                .skills
                                .get(skill_id, skill_level)
                                .and_then(|s| s.effects.get(scope).get(usize::from(index)))
                                .map_or("?", |e| e.name.as_str());
                            format!("skill:{skill_id}:{skill_level}/{name}")
                        }
                    };
                    let cond = f.cond.as_deref().map_or("null".to_owned(), cond_class);
                    format!("{} {} {} {owner} {cond}", class_of(f.calc), f.order, f.value)
                })
                .collect();
            let java: Vec<String> = case["calculators"][stat.java_name()]
                .as_array()
                .map(|list| {
                    list.iter()
                        .map(|f| {
                            let owner = f["owner"].as_str().unwrap();
                            format!(
                                "{} {} {} {owner} {}",
                                f["class"].as_str().unwrap(),
                                f["order"].as_i64().unwrap(),
                                f["value"].as_f64().unwrap(),
                                f["cond"].as_str().unwrap_or("null")
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            if ours != java {
                errors.push(format!(
                    "{id} {}:\n  java {java:?}\n  rust {ours:?}",
                    stat.java_name()
                ));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
