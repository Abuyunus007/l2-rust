//! Shape and consistency checks for the committed `java-vectors/out/stats_*.json` and `formulas_*.json`
//! files (M4-JV-01).
//!
//! These tests prove that the files parse, that the case lists have the promised coverage, and that the
//! numbers inside a file agree with each other where the Java source gives a closed form (UserInfo
//! rounding, attack timing, level bonus, the Rnd draw mapping). The stat engine and the formulas
//! (M4-STAT-01, M4-COMBAT-01, M4-REGEN-01) check the actual values against these files.
//!
//! A test skips with a message when its file is missing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeSet, HashMap};

use l2_testkit::vectors::{load, vector_path};
use serde::Deserialize;
use serde_json::Value;

/// Loads a vector file or returns `None` (and says so) when it is missing.
fn load_or_skip<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    if !vector_path(name).exists() {
        eprintln!("skipping: java-vectors/out/{name}.json is missing (run java-vectors/build.ps1)");
        return None;
    }
    Some(load(name))
}

// ---------------------------------------------------------------------------------------------
// stats_*.json
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct StatsFile {
    source: String,
    generated_by: String,
    note: String,
    config: HashMap<String, Value>,
    cases: Vec<StatCase>,
    skipped: Vec<Value>,
}

#[derive(Deserialize)]
struct StatCase {
    id: String,
    input: StatInput,
    expected: Expected,
}

#[derive(Deserialize)]
struct StatInput {
    kind: String,
    level: u32,
    // Players only.
    class_id: Option<u32>,
    female: Option<bool>,
    items: Option<Vec<ItemRef>>,
    skills: Option<Vec<SkillRef>>,
    derived_skills: Option<Vec<SkillRef>>,
    // NPCs only.
    npc_id: Option<u32>,
    npc_name: Option<String>,
    npc_type: Option<String>,
}

#[derive(Deserialize)]
struct ItemRef {
    item_id: u32,
    enchant: u32,
}

#[derive(Deserialize)]
struct SkillRef {
    id: u32,
    level: u32,
}

#[derive(Deserialize)]
struct Expected {
    user_info: UserInfo,
    raw: Raw,
}

#[derive(Deserialize)]
struct UserInfo {
    str: i32,
    dex: i32,
    con: i32,
    int: i32,
    wit: i32,
    men: i32,
    max_hp: i32,
    max_mp: i32,
    max_cp: Option<i32>,
    max_load: Option<i32>,
    p_atk: i32,
    p_atk_spd: i32,
    p_def: i32,
    evasion: i32,
    accuracy: i32,
    critical: i32,
    m_atk: i32,
    m_atk_spd: i32,
    m_def: i32,
    run_speed: i32,
    walk_speed: i32,
    swim_run_speed: i32,
    swim_walk_speed: i32,
    attack_range: i32,
}

#[derive(Deserialize)]
struct Raw {
    p_atk: f64,
    p_atk_spd: f64,
    p_def: f64,
    m_atk: f64,
    m_def: f64,
    run_speed: f64,
    walk_speed: f64,
    swim_run_speed: f64,
    swim_walk_speed: f64,
    movement_speed_multiplier: f64,
    attack_speed_multiplier: f64,
    m_critical: i32,
    shield_def: i32,
    level_mod: f64,
    level: i32,
    collision_radius: f64,
    collision_height: f64,
    base_stat_bonus: HashMap<String, f64>,
    probes: HashMap<String, f64>,
}

/// The ints in `user_info` are what the packet writes: `(int)` casts and `Math.round(speed / multiplier)`.
fn check_user_info_matches_raw(c: &StatCase) {
    let (ui, raw) = (&c.expected.user_info, &c.expected.raw);
    assert_eq!(ui.p_atk, raw.p_atk as i32, "{}: p_atk", c.id);
    assert_eq!(ui.p_atk_spd, raw.p_atk_spd as i32, "{}: p_atk_spd", c.id);
    assert_eq!(ui.p_def, raw.p_def as i32, "{}: p_def", c.id);
    assert_eq!(ui.m_atk, raw.m_atk as i32, "{}: m_atk", c.id);
    assert_eq!(ui.m_def, raw.m_def as i32, "{}: m_def", c.id);
    let m = raw.movement_speed_multiplier;
    // Java's Math.round is floor(x + 0.5).
    let round = |v: f64| (v / m + 0.5).floor() as i32;
    assert_eq!(ui.run_speed, round(raw.run_speed), "{}: run", c.id);
    assert_eq!(ui.walk_speed, round(raw.walk_speed), "{}: walk", c.id);
    assert_eq!(ui.swim_run_speed, round(raw.swim_run_speed), "{}: swim run", c.id);
    assert_eq!(
        ui.swim_walk_speed,
        round(raw.swim_walk_speed),
        "{}: swim walk",
        c.id
    );
}

fn check_common(c: &StatCase) {
    let (ui, raw) = (&c.expected.user_info, &c.expected.raw);
    assert!(ui.max_hp > 0 && ui.max_mp >= 0, "{}: hp/mp", c.id);
    for v in [ui.str, ui.dex, ui.con, ui.int, ui.wit, ui.men] {
        assert!((1..100).contains(&v), "{}: base stat {v}", c.id);
    }
    assert!(ui.accuracy >= 0 && ui.evasion >= 0 && ui.critical >= 0);
    assert!(ui.p_atk_spd > 0 && ui.m_atk_spd > 0 && ui.run_speed > 0);
    assert!(ui.attack_range >= 0);
    assert!(raw.collision_radius > 0.0 && raw.collision_height > 0.0);
    assert!(raw.attack_speed_multiplier > 0.0 && raw.level_mod > 0.0);
    assert!(raw.m_critical >= 0 && raw.shield_def >= 0 && raw.level >= 1);
    for key in ["str", "dex", "con", "int", "wit", "men"] {
        assert!(raw.base_stat_bonus.contains_key(key), "{}: bonus {key}", c.id);
    }
    assert!(raw.probes.contains_key("critical_damage@1"), "{}: probes", c.id);
    assert!(raw.probes.len() >= 25, "{}: probes", c.id);
    check_user_info_matches_raw(c);
}

fn check_header(f: &StatsFile) {
    assert_eq!(f.generated_by, "java-vectors");
    assert!(!f.source.is_empty() && !f.note.is_empty());
    assert!(f.config.contains_key("AutoLearnSkills"));
    assert!(f.skipped.is_empty(), "every case must have been built");
    let ids: BTreeSet<&str> = f.cases.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), f.cases.len(), "case ids must be unique");
}

#[test]
fn stats_players_naked_shape() {
    let Some(f) = load_or_skip::<StatsFile>("stats_players_naked") else {
        return;
    };
    check_header(&f);
    assert_eq!(f.cases.len(), 236);

    // All 11 starting classes at level 1, both sexes, with and without skills.
    for class in [0, 10, 18, 25, 31, 38, 44, 49, 53, 123, 124] {
        for sex in [false, true] {
            for skills in [false, true] {
                let n = f
                    .cases
                    .iter()
                    .filter(|c| {
                        let i = &c.input;
                        i.class_id == Some(class)
                            && i.female == Some(sex)
                            && i.level == 1
                            && (i.skills.as_ref().is_some_and(|s| !s.is_empty()) == skills)
                    })
                    .count();
                // A level 1 character may have no auto-get skill at all: "with skills" is then identical to "without".
                assert!(n >= 1, "class {class} female {sex} skills {skills}");
            }
        }
    }
    for level in [20, 40, 52, 76, 85] {
        assert!(
            f.cases.iter().filter(|c| c.input.level == level).count() >= 36,
            "level {level}"
        );
    }
    for c in &f.cases {
        assert_eq!(c.input.kind, "player");
        assert!(c.input.items.as_ref().unwrap().is_empty(), "{}: naked", c.id);
        assert!(c.expected.user_info.max_cp.is_some() && c.expected.user_info.max_load.is_some());
        check_common(c);
    }
    // Skills change numbers somewhere: a class that has auto-get skills differs between the two variants.
    let with_skills = f
        .cases
        .iter()
        .filter(|c| c.input.skills.as_ref().is_some_and(|s| !s.is_empty()))
        .count();
    assert!(with_skills > 100, "{with_skills}");
}

#[test]
fn stats_players_gear_shape() {
    let Some(f) = load_or_skip::<StatsFile>("stats_players_gear") else {
        return;
    };
    check_header(&f);
    assert_eq!(f.cases.len(), 64);
    let enchants: BTreeSet<u32> = f
        .cases
        .iter()
        .flat_map(|c| c.input.items.as_ref().unwrap().iter().map(|i| i.enchant))
        .collect();
    assert_eq!(enchants.into_iter().collect::<Vec<_>>(), vec![0, 3, 6, 16]);
    for c in &f.cases {
        let items = c.input.items.as_ref().unwrap();
        assert!(items.len() >= 5, "{}: gear", c.id);
        assert!(
            items
                .iter()
                .all(|i| i.item_id > 0 && i.enchant == items[0].enchant),
            "{}",
            c.id
        );
        check_common(c);
    }
    // The heavy A-grade set (with its shield) triggers the armor set skills.
    let heavy = f
        .cases
        .iter()
        .find(|c| c.id.starts_with("gear_heavy_c88_l76_e0"))
        .unwrap();
    let derived = heavy.input.derived_skills.as_ref().unwrap();
    assert!(
        derived.iter().any(|s| s.id == 3530),
        "armor set skill 3530 missing"
    );
    assert!(derived.iter().any(|s| s.id == 3550), "shield skill 3550 missing");
    // Enchant 6+ adds the enchant-6 set skill, 0 does not.
    let e0 = f
        .cases
        .iter()
        .find(|c| c.id == "gear_heavy_c88_l76_e0_noskills")
        .unwrap();
    let e6 = f
        .cases
        .iter()
        .find(|c| c.id == "gear_heavy_c88_l76_e6_noskills")
        .unwrap();
    assert!(
        !e0.input
            .derived_skills
            .as_ref()
            .unwrap()
            .iter()
            .any(|s| s.id == 3620)
    );
    assert!(
        e6.input
            .derived_skills
            .as_ref()
            .unwrap()
            .iter()
            .any(|s| s.id == 3620)
    );
    // Skills of the weapon type come from the class: some case has a non-empty skills list.
    assert!(f.cases.iter().any(|c| {
        c.input
            .skills
            .as_ref()
            .is_some_and(|s| s.iter().any(|x| x.level > 0))
    }));
}

#[test]
fn stats_npcs_shape() {
    let Some(f) = load_or_skip::<StatsFile>("stats_npcs") else {
        return;
    };
    check_header(&f);
    assert!(f.cases.len() >= 30, "{}", f.cases.len());
    let types: BTreeSet<&str> = f
        .cases
        .iter()
        .map(|c| c.input.npc_type.as_deref().unwrap())
        .collect();
    for t in [
        "Monster",
        "RaidBoss",
        "GrandBoss",
        "Guard",
        "Teleporter",
        "Merchant",
        "Trainer",
    ] {
        assert!(types.contains(t), "type {t} missing");
    }
    assert!(f.cases.iter().any(|c| c.input.npc_id == Some(31033)), "NPC 31033");
    for c in &f.cases {
        assert_eq!(c.input.kind, "npc");
        assert!(c.input.npc_name.is_some() && c.input.level >= 1);
        assert!(
            c.expected.user_info.max_cp.is_none(),
            "{}: NPCs have no CP in the dump",
            c.id
        );
        check_common(c);
    }
}

#[test]
fn stats_base_bonus_shape() {
    let Some(v) = load_or_skip::<Value>("stats_base_bonus") else {
        return;
    };
    assert_eq!(v["generated_by"], "java-vectors");
    for key in ["str", "int", "dex", "wit", "con", "men"] {
        let table = v[key].as_array().unwrap_or_else(|| panic!("{key} missing"));
        assert_eq!(table.len(), 100, "{key}");
        let values: Vec<f64> = table.iter().map(|x| x.as_f64().unwrap()).collect();
        assert!(values.iter().all(|x| *x > 0.0), "{key}: positive");
        assert!(values.windows(2).all(|w| w[1] >= w[0]), "{key}: non-decreasing");
    }
}

#[test]
fn stats_func_trace_shape() {
    let Some(v) = load_or_skip::<Value>("stats_func_trace") else {
        return;
    };
    assert_eq!(v["generated_by"], "java-vectors");
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 9);
    for c in cases {
        let calculators = c["calculators"].as_object().unwrap();
        // Every creature has the standard formula functions (order 1) on these stats.
        for stat in [
            "POWER_ATTACK",
            "POWER_DEFENCE",
            "MAX_HP",
            "ACCURACY_COMBAT",
            "MOVE_SPEED",
        ] {
            let funcs = calculators[stat]
                .as_array()
                .unwrap_or_else(|| panic!("{}: {stat}", s(c, "id")));
            let orders: Vec<i64> = funcs.iter().map(|x| x["order"].as_i64().unwrap()).collect();
            let mut sorted = orders.clone();
            sorted.sort_unstable();
            assert_eq!(orders, sorted, "{}: {stat} must be sorted by order", s(c, "id"));
        }
    }
    // The heavy +6 case: the weapon's item function comes first on P.Atk, then the standard P.Atk formula.
    let heavy = cases
        .iter()
        .find(|c| s(c, "id") == "trace_heavy_c88_l76_e6_skills")
        .unwrap();
    let p_atk = heavy["calculators"]["POWER_ATTACK"].as_array().unwrap();
    assert_eq!(s(&p_atk[0], "class"), "FuncSet");
    assert_eq!(s(&p_atk[0], "owner"), "item:148");
    assert!(p_atk.iter().any(|x| s(x, "class") == "FuncPAtkMod"));
}

// ---------------------------------------------------------------------------------------------
// formulas_*.json
// ---------------------------------------------------------------------------------------------

fn f(v: &Value, key: &str) -> f64 {
    v[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is not a number in {v}"))
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} is not a string in {v}"))
}

fn creatures_config() -> Value {
    let v: Value = load("formulas_creatures");
    v["config"].clone()
}

fn randomize_auto_attack_damage() -> bool {
    creatures_config()["RandomizeAutoAttackDamage"] == true
}

fn creature_names() -> Vec<String> {
    let v: Value = load("formulas_creatures");
    v["creatures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| s(c, "name").to_string())
        .collect()
}

/// Every case with a script: the `rnd` calls line up with the consumed script values and the returned
/// values follow the mapping documented in the harness `Rnd`.
fn check_draws(case: &Value) {
    let id = s(case, "id");
    let script: Vec<f64> = case["script"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect();
    let rnd = case["rnd"].as_array().unwrap();
    assert_eq!(script.len(), rnd.len(), "{id}: one script value per draw");
    for (u, call) in script.iter().zip(rnd) {
        assert!((0.0..1.0).contains(u), "{id}: u {u}");
        let method = s(call, "method");
        let bound = s(call, "bound");
        let returned = s(call, "returned");
        match method {
            "get_int" => {
                let b: i32 = bound.parse().unwrap();
                let r: i32 = returned.parse().unwrap();
                assert_eq!(r, if b <= 0 { 0 } else { (u * f64::from(b)) as i32 }, "{id}");
            }
            "get_int_range" => {
                let (o, b) = bound.split_once(',').unwrap();
                let (o, b): (i64, i64) = (o.parse().unwrap(), b.parse().unwrap());
                let r: i64 = returned.parse().unwrap();
                let expected = if o >= b {
                    o
                } else {
                    o + (u * ((b - o + 1) as f64)) as i64
                };
                assert_eq!(r, expected, "{id}");
            }
            other => panic!("{id}: unexpected draw method {other}"),
        }
    }
}

/// Loads a formula file, checks the common header and that every case names known creatures, and returns its cases.
fn formula_cases(name: &str, expected_min: usize) -> Option<Vec<Value>> {
    let v: Value = load_or_skip(name)?;
    assert_eq!(v["generated_by"], "java-vectors");
    assert!(!s(&v, "source").is_empty() && !s(&v, "note").is_empty());
    let cases = v["cases"].as_array().unwrap().clone();
    assert!(cases.len() >= expected_min, "{name}: {} cases", cases.len());
    let names = creature_names();
    let mut ids = BTreeSet::new();
    for c in &cases {
        assert!(
            ids.insert(s(c, "id").to_string()),
            "{name}: duplicate id {}",
            s(c, "id")
        );
        for key in ["attacker", "target"] {
            if let Some(n) = c[key].as_str() {
                assert!(names.iter().any(|x| x == n), "{name}: unknown creature {n}");
            }
        }
    }
    Some(cases)
}

#[test]
fn formulas_creatures_shape() {
    let Some(v) = load_or_skip::<Value>("formulas_creatures") else {
        return;
    };
    assert_eq!(v["generated_by"], "java-vectors");
    let creatures = v["creatures"].as_array().unwrap();
    assert!(creatures.len() >= 15);
    for c in creatures {
        assert!(c["input"]["kind"] == "player" || c["input"]["kind"] == "npc");
        assert!(c["expected"]["user_info"]["max_hp"].as_i64().unwrap() > 0);
    }
    let names = creature_names();
    let pairs = v["pairs"].as_array().unwrap();
    assert!(pairs.len() >= 25);
    for p in pairs {
        for n in p.as_array().unwrap() {
            assert!(
                names.iter().any(|x| Some(x.as_str()) == n.as_str()),
                "pair member {n}"
            );
        }
    }
    let config = &v["config"];
    for key in [
        "AltShieldBlocks",
        "AltPerfectShieldBlockRate",
        "RandomizeAutoAttackDamage",
        "NpcDmgPenalty",
        "NpcCritDmgPenalty",
        "NpcSkillDmgPenalty",
        "MinNPCLevelForDmgPenalty",
        "HpRegenMultiplier",
        "ClassBalance_arrays_not_all_1",
    ] {
        assert!(config.get(key).is_some(), "config {key}");
    }
    // The vectors are only valid for a ClassBalance.ini without custom multipliers.
    assert!(
        config["ClassBalance_arrays_not_all_1"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn formulas_hit_miss_shape() {
    let Some(cases) = formula_cases("formulas_hit_miss", 1000) else {
        return;
    };
    let mut misses = 0;
    for c in &cases {
        check_draws(c);
        assert_eq!(c["rnd"].as_array().unwrap().len(), 1);
        let miss = c["result"].as_bool().unwrap();
        misses += usize::from(miss);
        // The draw decides: miss <=> chance < draw. With the generator's own chance copy the boundary draws must agree.
        let chance = c["intermediate"]["chance_estimate"].as_i64().unwrap();
        assert!((200..=980).contains(&chance), "{}: chance {chance}", s(c, "id"));
        let draw: i64 = s(&c["rnd"][0], "returned").parse().unwrap();
        assert_eq!(miss, chance < draw, "{}: chance {chance} draw {draw}", s(c, "id"));
    }
    assert!(misses > 100 && misses < cases.len() - 100, "misses {misses}");
    assert!(cases.iter().any(|c| c["night"] == true) && cases.iter().any(|c| c["night"] == false));
}

#[test]
fn formulas_crit_shape() {
    let Some(cases) = formula_cases("formulas_crit", 300) else {
        return;
    };
    for c in &cases {
        check_draws(c);
        let limit = f(&c["intermediate"], "limit");
        let draw: f64 = s(&c["rnd"][0], "returned").parse().unwrap();
        // Java: (defence-adjusted rate) > Rnd.get(1000)
        assert_eq!(c["result"].as_bool().unwrap(), limit > draw, "{}", s(c, "id"));
    }
}

#[test]
fn formulas_shield_shape() {
    let Some(cases) = formula_cases("formulas_shield", 300) else {
        return;
    };
    let mut seen = BTreeSet::new();
    for c in &cases {
        check_draws(c);
        let result = c["result"].as_i64().unwrap();
        assert!((0..=2).contains(&result));
        seen.insert(result);
        if c["target_has_shield"] == false {
            assert_eq!(result, 0, "{}: no shield, no block", s(c, "id"));
            assert!(c["rnd"].as_array().unwrap().is_empty());
        }
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        vec![0, 1, 2],
        "all three outcomes appear"
    );
}

#[test]
fn formulas_phys_dam_shape() {
    let Some(cases) = formula_cases("formulas_phys_dam", 900) else {
        return;
    };
    for c in &cases {
        check_draws(c);
        let damage = f(c, "result");
        assert!(damage >= 0.0, "{}", s(c, "id"));
        if c["shld"] == 2 {
            assert_eq!(damage, 1.0, "perfect block");
            assert!(c["rnd"].as_array().unwrap().is_empty());
        } else {
            // getRandomDamageMultiplier is applied once when RandomizeAutoAttackDamage is on and once more
            // unconditionally (Java quirk), each time one Rnd.get(-random, random) draw.
            // A creature whose random damage range is 0 (Rnd.get(0, 0)) draws nothing.
            let expected = 1 + usize::from(randomize_auto_attack_damage());
            assert!(c["rnd"].as_array().unwrap().len() <= expected, "{}", s(c, "id"));
            assert!(
                c["rnd"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|d| s(d, "method") == "get_int_range")
            );
        }
    }
    // Soulshot doubles P.Atk before the formula: with the same draws the damage is higher.
    let plain = cases
        .iter()
        .find(|c| s(c, "id") == "pdam_duelist76>npc_20677_front_s0_nocrit_noss_r1")
        .unwrap();
    let ss = cases
        .iter()
        .find(|c| s(c, "id") == "pdam_duelist76>npc_20677_front_s0_nocrit_ss_r1")
        .unwrap();
    assert!(f(ss, "result") > f(plain, "result") * 1.9);
}

#[test]
fn formulas_attack_hit_shape() {
    let Some(cases) = formula_cases("formulas_attack_hit", 200) else {
        return;
    };
    for c in &cases {
        check_draws(c);
        let r = &c["result"];
        let miss = r["miss"].as_bool().unwrap();
        let n = c["rnd"].as_array().unwrap().len();
        if miss {
            assert_eq!(n, 1, "{}: a miss draws once", s(c, "id"));
            assert_eq!(r["damage"], 0);
        } else {
            // hit chance, shield (0..2 draws), crit, then 2 damage randoms (none after a perfect block)
            assert!((2..=6).contains(&n), "{}: {n} draws", s(c, "id"));
            assert!(r["damage"].as_i64().unwrap() >= 0);
        }
    }
    assert!(
        cases.iter().any(|c| c["result"]["miss"] == true)
            && cases.iter().any(|c| c["result"]["crit"] == true)
    );
}

#[test]
fn formulas_attack_timing_shape() {
    let Some(v) = load_or_skip::<Value>("formulas_attack_timing") else {
        return;
    };
    assert_eq!(v["generated_by"], "java-vectors");
    for c in v["creatures"].as_array().unwrap() {
        let spd = f(c, "p_atk_spd");
        // Creature.calculateTimeBetweenAttacks: (int) (500000 / pAtkSpd)
        assert_eq!(
            c["time_atk"].as_i64().unwrap(),
            (500_000.0 / spd) as i64,
            "{}",
            s(c, "creature")
        );
        assert_eq!(
            c["time_to_hit"].as_i64().unwrap(),
            c["time_atk"].as_i64().unwrap() / 2
        );
        let delay = c["weapon_reuse_delay"].as_i64().unwrap();
        // Creature.calculateReuseTime: weapon reuse delay 0 gives 0, else (int) (delay * 333 / pAtkSpd)
        let reuse = if delay == 0 {
            0
        } else {
            (delay as f64 * 333.0 / spd) as i64
        };
        assert_eq!(c["reuse"].as_i64().unwrap(), reuse, "{}", s(c, "creature"));
    }
    for c in v["calc_p_atk_spd"].as_array().unwrap() {
        let rate = f(c, "rate");
        // Formulas.calcPAtkSpd
        let expected = if rate < 2.0 {
            2700
        } else {
            (470_000.0 / rate) as i64
        };
        assert_eq!(c["result"].as_i64().unwrap(), expected, "rate {rate}");
    }
    for c in v["calc_atk_spd"].as_array().unwrap() {
        // Formulas.calcAtkSpd: magic (skillTime / mAtkSpd) * 333, physical (skillTime / pAtkSpd) * 300
        let time = f(c, "skill_time");
        let expected = if c["skill_is_magic"] == true {
            ((time / f(c, "m_atk_spd")) * 333.0) as i64
        } else {
            ((time / f(c, "p_atk_spd")) * 300.0) as i64
        };
        assert_eq!(c["result"].as_i64().unwrap(), expected, "{}", s(c, "creature"));
    }
}

#[test]
fn formulas_atk_break_shape() {
    let Some(cases) = formula_cases("formulas_atk_break", 150) else {
        return;
    };
    let mut broke = 0;
    for c in &cases {
        check_draws(c);
        let state = s(c, "state");
        let result = c["result"].as_bool().unwrap();
        broke += usize::from(result);
        let draws = c["rnd"].as_array().unwrap().len();
        if state == "idle" || c["intermediate"]["is_raid"] == true || c["intermediate"]["is_invul"] == true {
            assert!(!result, "{}: nothing to break", s(c, "id"));
            assert_eq!(draws, 0);
        } else {
            // Only the configured trigger (AltGameCancelByHit: cast, bow, all) gives a non-zero base rate, hence a draw.
            let config = &creatures_config();
            let enabled = if state == "cast" {
                &config["AltGameCancelByHit_cast"]
            } else {
                &config["AltGameCancelByHit_bow"]
            };
            assert_eq!(draws, usize::from(enabled == true), "{}", s(c, "id"));
        }
    }
    assert!(broke > 0 && broke < cases.len(), "broke {broke}");
}

#[test]
fn formulas_regen_shape() {
    let Some(v) = load_or_skip::<Value>("formulas_regen") else {
        return;
    };
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 300);
    let by_id: HashMap<&str, &Value> = cases.iter().map(|c| (s(c, "id"), c)).collect();
    assert_eq!(by_id.len(), cases.len(), "unique ids");
    for c in cases {
        assert!(f(c, "hp") > 0.0 && f(c, "mp") > 0.0, "{}", s(c, "id"));
    }
    // Movement state factors (Formulas.calcHpRegen): sitting x1.5, standing x1.1, running x0.7, walking x1.
    let stand = f(by_id["duelist76_stand"], "hp");
    let sit = f(by_id["duelist76_sit"], "hp");
    let run = f(by_id["duelist76_move_run"], "hp");
    let walk = f(by_id["duelist76_move_walk"], "hp");
    assert!((sit / stand - 1.5 / 1.1).abs() < 1e-9);
    assert!((run / stand - 0.7 / 1.1).abs() < 1e-9);
    assert!((walk / stand - 1.0 / 1.1).abs() < 1e-9);
    assert!(by_id["duelist76_stand"].get("cp").is_some());
    assert!(by_id["npc_20677_stand"].get("cp").is_none());
}

#[test]
fn formulas_lvl_bonus_shape() {
    let Some(v) = load_or_skip::<Value>("formulas_lvl_bonus") else {
        return;
    };
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 250);
    for c in cases {
        // Formulas.calcLvlBonusMod
        let magic = c["skill_magic_level"].as_i64().unwrap();
        let attacker = if magic > 0 {
            magic
        } else {
            c["attacker_level"].as_i64().unwrap()
        };
        let rate_mod = 1.0 + f(c, "skill_lvl_bonus_rate") / 100.0;
        let lvl_mod = 1.0 + (attacker - c["target_level"].as_i64().unwrap()) as f64 / 100.0;
        assert!(
            (f(c, "result") - rate_mod * lvl_mod).abs() < 1e-12,
            "{}",
            s(c, "id")
        );
    }
    assert!(
        cases.iter().any(|c| c["skill_magic_level"] == 0),
        "a skill without magic level"
    );
}
