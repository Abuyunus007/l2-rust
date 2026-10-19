#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The formulas of `l2_world::formulas` against the vectors the real L2JMobius `Formulas` class
//! wrote (`java-vectors/out/formulas_*.json`, M4-JV-01).
//!
//! How a case is replayed:
//! - The creatures come from `formulas_creatures.json` (stats and probes of the stat engine); the
//!   numbers the formulas take as input are read from there, or from the `intermediate` block of
//!   the case where Java recorded them (accuracy, evasion, p.atk, p.def, ...). A `calcStat` whose
//!   base depends on the formula (defence critical rate, attack cancel, regeneration) is rebuilt
//!   from the creature's probe `stat@base` as `base * probe / base`; every such stat in the files
//!   is a plain multiplier, and a mismatch would show it if one were not.
//! - Every random draw is answered from the case's `rnd` list (`returned`), and the test checks
//!   that the formula made exactly these calls (`method`, `bound`) in this order.
//! - Files that are missing skip with a message (CI has all of them: they are committed).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use l2_data::{HitConditionBonus, NpcLoadOptions, NpcTemplates, PlayerTemplates};
use l2_world::formulas::{
    AtkBreakInput, CpRegenInput, CritInput, HitMissInput, HpRegenInput, MpRegenInput, PhysDamClassBalance,
    PhysDamInput, Position, RegenKind, RegenPosture, ReuseAttackType, ReuseInput, Rnd, ShieldInput,
    calc_atk_break, calc_atk_spd, calc_cp_regen, calc_crit, calc_hit_miss, calc_hp_regen, calc_lvl_bonus_mod,
    calc_mp_regen, calc_p_atk_spd, calc_phys_dam, calc_shld_use, calculate_reuse_time,
    calculate_time_between_attacks, condition_bonus,
};
use serde_json::Value;

// ------------------------------------------------------------------ loading

fn vector(name: &str) -> Option<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../java-vectors/out")
        .join(format!("{name}.json"));
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(
            serde_json::from_str(&exact_floats(&text)).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
        ),
        Err(_) => {
            eprintln!("skipped: {} is missing", path.display());
            None
        }
    }
}

/// `serde_json` without its `float_roundtrip` feature can be one ulp off when it parses a double,
/// and these vectors compare doubles exactly. So every number with a fraction or an exponent is
/// turned into the string `"f64:<text>"` before parsing, and [`f`] reads those with
/// `str::parse::<f64>`, which rounds correctly like Java's `Double.parseDouble`.
fn exact_floats(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '-' || c.is_ascii_digit() {
            let mut token = String::from(c);
            while let Some(&n) = chars.peek() {
                if n.is_ascii_digit() || matches!(n, '.' | 'e' | 'E' | '+' | '-') {
                    token.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            if token.contains(['.', 'e', 'E']) {
                out.push_str(&format!("\"f64:{token}\""));
            } else {
                out.push_str(&token);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn f(v: &Value) -> f64 {
    if let Some(text) = v.as_str().and_then(|t| t.strip_prefix("f64:")) {
        return text.parse().unwrap_or_else(|e| panic!("bad number {text}: {e}"));
    }
    // A whole number written without a fraction.
    v.as_i64()
        .map_or_else(|| panic!("not a number: {v}"), |n| n as f64)
}

fn i(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap_or_else(|| panic!("not an int: {v}"))).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn b(v: &Value) -> bool {
    v.as_bool().unwrap_or_else(|| panic!("not a bool: {v}"))
}

// ------------------------------------------------------------------ the scripted RNG

struct Draw {
    method: String,
    bound: String,
    returned: i64,
}

/// Answers the formula's draws from the recorded `rnd` list and remembers the calls it got.
struct Scripted {
    expected: Vec<Draw>,
    next: usize,
    calls: Vec<(String, String)>,
}

impl Scripted {
    fn from_case(case: &Value) -> Self {
        let expected = case["rnd"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| Draw {
                method: s(&d["method"]).to_string(),
                bound: s(&d["bound"]).to_string(),
                returned: s(&d["returned"]).parse().unwrap(),
            })
            .collect();
        Self {
            expected,
            next: 0,
            calls: Vec::new(),
        }
    }

    fn answer(&mut self, method: &str, bound: String) -> i64 {
        self.calls.push((method.to_string(), bound));
        let value = self.expected.get(self.next).map_or(0, |d| d.returned);
        self.next += 1;
        value
    }

    /// The formula made exactly the recorded calls, in order.
    fn finish(self, id: &str) {
        let wanted: Vec<(String, String)> = self
            .expected
            .iter()
            .map(|d| (d.method.clone(), d.bound.clone()))
            .collect();
        assert_eq!(self.calls, wanted, "{id}: the sequence of draws");
    }
}

impl Rnd for Scripted {
    fn next_boolean(&mut self) -> bool {
        panic!("next_boolean is not used by these formulas")
    }
    fn next_bytes(&mut self, _bytes: &mut [u8]) {
        panic!("next_bytes is not used by these formulas")
    }
    fn get_int(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0; // Java draws nothing here, and the vector does not record it
        }
        i32::try_from(self.answer("get_int", bound.to_string())).unwrap()
    }
    fn get_int_range(&mut self, origin: i32, bound: i32) -> i32 {
        if origin >= bound {
            return origin;
        }
        i32::try_from(self.answer("get_int_range", format!("{origin},{bound}"))).unwrap()
    }
    fn next_int(&mut self) -> i32 {
        panic!("next_int is not used by these formulas")
    }
    fn get_long(&mut self, _bound: i64) -> i64 {
        panic!("get_long is not used by these formulas")
    }
    fn get_long_range(&mut self, _origin: i64, _bound: i64) -> i64 {
        panic!("get_long_range is not used by these formulas")
    }
    fn next_long(&mut self) -> i64 {
        panic!("next_long is not used by these formulas")
    }
    fn next_float(&mut self) -> f32 {
        panic!("next_float is not used by these formulas")
    }
    fn get_double(&mut self, _bound: f64) -> f64 {
        panic!("get_double is not used by these formulas")
    }
    fn get_double_range(&mut self, _origin: f64, _bound: f64) -> f64 {
        panic!("get_double_range is not used by these formulas")
    }
    fn next_double(&mut self) -> f64 {
        panic!("next_double is not used by these formulas")
    }
    fn next_gaussian(&mut self) -> f64 {
        panic!("next_gaussian is not used by these formulas")
    }
}

// ------------------------------------------------------------------ creatures and config

struct Creature {
    name: String,
    player: bool,
    probes: BTreeMap<String, f64>,
    /// `Weapon.getItemType()` name of `getAttackType()`; `FIST` without a weapon type.
    attack_type: String,
    weapon_id: i64,
    weapon_reuse_delay: i32,
    is_raid: bool,
    dex_bonus: f64,
}

impl Creature {
    fn probe(&self, key: &str) -> f64 {
        *self.probes.get(key).unwrap_or_else(|| panic!("no probe {key}"))
    }

    fn has_weapon(&self) -> bool {
        self.weapon_id != 0
    }

    fn is_bow(&self) -> bool {
        self.attack_type == "BOW"
    }

    fn is_bow_or_crossbow(&self) -> bool {
        self.attack_type == "BOW" || self.attack_type == "CROSSBOW"
    }
}

/// `calcStat(stat, base)` rebuilt from the probe `stat@base`: a plain multiplier.
fn scaled(probe: f64, base: f64) -> impl Fn(f64) -> f64 {
    move |value| {
        if probe == base {
            value
        } else {
            value * (probe / base)
        }
    }
}

struct Config {
    hit_bonus: HitConditionBonus,
    alt_shield_blocks: bool,
    alt_perfect_shield_block: i32,
    randomize_auto_attack_damage: bool,
    cancel_cast: bool,
    cancel_bow: bool,
    min_npc_level_dmg_penalty: i32,
    npc_dmg_penalty: Vec<f32>,
    npc_crit_dmg_penalty: Vec<f32>,
    hp_regen: f64,
    mp_regen: f64,
    cp_regen: f64,
    raid_hp_regen: f64,
    raid_mp_regen: f64,
}

fn floats(v: &Value) -> Vec<f32> {
    v.as_array().unwrap().iter().map(|x| f(x) as f32).collect()
}

impl Config {
    fn from(v: &Value) -> Self {
        assert_eq!(
            v["ClassBalance_arrays_not_all_1"].as_array().unwrap().len(),
            0,
            "the vectors are only valid with the default ClassBalance"
        );
        Self {
            hit_bonus: HitConditionBonus {
                front: i(&v["hitConditionBonus_frontBonus"]),
                side: i(&v["hitConditionBonus_sideBonus"]),
                back: i(&v["hitConditionBonus_backBonus"]),
                high: i(&v["hitConditionBonus_highBonus"]),
                low: i(&v["hitConditionBonus_lowBonus"]),
                dark: i(&v["hitConditionBonus_darkBonus"]),
                rain: 0,
            },
            alt_shield_blocks: b(&v["AltShieldBlocks"]),
            alt_perfect_shield_block: i(&v["AltPerfectShieldBlockRate"]),
            randomize_auto_attack_damage: b(&v["RandomizeAutoAttackDamage"]),
            cancel_cast: b(&v["AltGameCancelByHit_cast"]),
            cancel_bow: b(&v["AltGameCancelByHit_bow"]),
            min_npc_level_dmg_penalty: i(&v["MinNPCLevelForDmgPenalty"]),
            npc_dmg_penalty: floats(&v["NpcDmgPenalty"]),
            npc_crit_dmg_penalty: floats(&v["NpcCritDmgPenalty"]),
            hp_regen: f(&v["HpRegenMultiplier"]),
            mp_regen: f(&v["MpRegenMultiplier"]),
            cp_regen: f(&v["CpRegenMultiplier"]),
            raid_hp_regen: f(&v["RaidHpRegenMultiplier"]),
            raid_mp_regen: f(&v["RaidMpRegenMultiplier"]),
        }
    }
}

fn probes_of(raw: &Value) -> BTreeMap<String, f64> {
    raw["probes"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), f(v)))
        .collect()
}

/// Everything the formula vectors share: the creatures, the config, the geometry, and the
/// per-pair numbers Java recorded in the `intermediate` blocks of the other files.
struct Ctx {
    config: Config,
    creatures: HashMap<String, Creature>,
}

impl Ctx {
    fn load() -> Option<Self> {
        let file = vector("formulas_creatures")?;
        let timing = vector("formulas_attack_timing")?;
        let regen = vector("formulas_regen")?;
        let config = Config::from(&file["config"]);

        let mut timing_by_name = HashMap::new();
        for c in timing["creatures"].as_array().unwrap() {
            timing_by_name.insert(s(&c["creature"]).to_string(), c.clone());
        }
        let mut raid = HashMap::new();
        for c in regen["cases"].as_array().unwrap() {
            raid.insert(s(&c["creature"]).to_string(), b(&c["is_raid"]));
        }

        let mut creatures = HashMap::new();
        for c in file["creatures"].as_array().unwrap() {
            let name = s(&c["name"]).to_string();
            let raw = c["expected"]["raw"].clone();
            let t = &timing_by_name[&name];
            creatures.insert(
                name.clone(),
                Creature {
                    name: name.clone(),
                    player: s(&c["input"]["kind"]) == "player",
                    probes: probes_of(&raw),
                    attack_type: s(&t["attack_type"]).to_string(),
                    weapon_id: t["weapon_id"].as_i64().unwrap(),
                    weapon_reuse_delay: i(&t["weapon_reuse_delay"]),
                    is_raid: raid[&name],
                    dex_bonus: f(&raw["base_stat_bonus"]["dex"]),
                },
            );
        }
        Some(Self { config, creatures })
    }

    fn get(&self, name: &str) -> &Creature {
        self.creatures
            .get(name)
            .unwrap_or_else(|| panic!("no creature {name}"))
    }
}

struct Geometry {
    target_x: i32,
    target_y: i32,
    target_heading: i32,
    attacker: HashMap<String, (i32, i32)>,
}

impl Geometry {
    fn from(file: &Value) -> Self {
        let g = &file["geometry"];
        let attacker = g["attacker_xy_by_relation"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let (x, y) = s(v).split_once(',').unwrap();
                (k.clone(), (x.parse().unwrap(), y.parse().unwrap()))
            })
            .collect();
        Self {
            target_x: i(&g["target"]["x"]),
            target_y: i(&g["target"]["y"]),
            target_heading: i(&g["target"]["heading"]),
            attacker,
        }
    }
}

fn position(placement: &Value) -> Position {
    match s(&placement["relation"]) {
        "front" => Position::Front,
        "side" => Position::Side,
        "back" => Position::Back,
        other => panic!("unknown relation {other}"),
    }
}

// ------------------------------------------------------------------ formula runners

fn run_hit_miss(
    ctx: &Ctx,
    accuracy: i32,
    evasion_rate: i32,
    placement: &Value,
    night: bool,
    rng: &mut Scripted,
) -> bool {
    calc_hit_miss(
        &HitMissInput {
            accuracy,
            evasion_rate,
            attacker_z: i(&placement["z_diff"]),
            target_z: 0,
            is_night: night,
            position: position(placement),
            hit_condition_bonus: &ctx.config.hit_bonus,
        },
        rng,
    )
}

fn run_crit(attacker: &Creature, target: &Creature, rate: f64, rng: &mut Scripted) -> bool {
    let defence = scaled(target.probe("defence_critical_rate@100"), 100.0);
    calc_crit(
        &CritInput {
            critical_rate: rate,
            attacker_is_playable: attacker.player,
            target_is_playable: target.player,
            target_is_attackable: !target.player,
            pvp_physical_attack_critical_chance_multiplier: 1.0,
            pve_physical_attack_critical_chance_multiplier: 1.0,
            defence_critical_rate: &defence,
            defence_critical_rate_add: target.probe("defence_critical_rate_add@0"),
        },
        rng,
    )
}

fn run_shield(
    ctx: &Ctx,
    geometry: &Geometry,
    attacker: &Creature,
    target: &Creature,
    placement: &Value,
    (has_shield, shield_rate): (bool, f64),
    rng: &mut Scripted,
) -> u8 {
    let (attacker_x, attacker_y) = geometry.attacker[s(&placement["relation"])];
    calc_shld_use(
        &ShieldInput {
            target_has_shield: has_shield,
            shield_rate,
            dex_bonus: target.dex_bonus,
            shield_defence_angle: target.probe("shield_defence_angle@0"),
            target_x: geometry.target_x,
            target_y: geometry.target_y,
            target_heading: geometry.target_heading,
            attacker_x,
            attacker_y,
            attacker_weapon_is_bow: attacker.is_bow(),
            alt_perfect_shld_block: ctx.config.alt_perfect_shield_block,
        },
        rng,
    )
}

/// `(attack, defence)` attribute values of the pair, the inputs of `calcAttributeBonus`. The vectors
/// do not record them; they come from the NPC templates (`data/stats/npcs`) and were checked
/// against Java's damage numbers: four monsters attack with `<attack type="DARK" value="20">`
/// against players without resistance, and the targets 18366 and 31033 have `default` element
/// defences of 50 and 150 against attackers without an element. Everything else is 0.
fn elements(attacker: &Creature, target: &Creature) -> (i32, i32) {
    let attack = match attacker.name.as_str() {
        "npc_20201" | "npc_20239" | "npc_21090" | "npc_21001" => 20,
        _ => 0,
    };
    let defence = match target.name.as_str() {
        "npc_18366" => 50,
        "npc_31033" => 150,
        _ => 0,
    };
    (attack, defence)
}

/// The numbers `intermediate` carries for a `calcPhysDam` case.
struct PhysDamNumbers {
    p_atk: f64,
    p_def: f64,
    shield_def: i32,
    attacker_level: i32,
    target_level: i32,
}

fn run_phys_dam(
    ctx: &Ctx,
    attacker: &Creature,
    target: &Creature,
    placement: &Value,
    n: &PhysDamNumbers,
    (shld, crit, ss): (u8, bool, bool),
    rng: &mut Scripted,
) -> f64 {
    let config = &ctx.config;
    let attributes = elements(attacker, target);
    calc_phys_dam(
        &PhysDamInput {
            attacker_is_playable: attacker.player,
            target_is_playable: target.player,
            target_is_attackable: !target.player,
            position: position(placement),
            p_atk: n.p_atk,
            p_def: n.p_def,
            shld_def: n.shield_def,
            pvp_physical_def: target.probe("pvp_physical_def@1"),
            critical_damage: attacker.probe("critical_damage@1"),
            critical_damage_pos: attacker.probe("critical_damage_pos@1"),
            defence_critical_damage: target.probe("defence_critical_damage@1"),
            critical_damage_add: attacker.probe("critical_damage_add@0"),
            defence_critical_damage_add: target.probe("defence_critical_damage_add@0"),
            // No trait is set on any creature of the vectors.
            attack_trait_bonus: 1.0,
            attacker_has_active_weapon: attacker.player || attacker.has_weapon(),
            random_damage: attacker.probe("random_damage@0") as i32,
            attacker_level: n.attacker_level,
            pvp_physical_dmg: attacker.probe("pvp_physical_dmg@1"),
            attack_attribute: attributes.0,
            defence_attribute: attributes.1,
            attacker_weapon_is_bow_or_crossbow: attacker.is_bow_or_crossbow(),
            pve_bow_dmg: attacker.probe("pve_bow_dmg@1"),
            pve_physical_dmg: attacker.probe("pve_physical_dmg@1"),
            target_is_raid: target.is_raid,
            target_is_raid_minion: false,
            target_level: n.target_level,
            attacker_player_level: attacker.player.then_some(n.attacker_level),
            class_balance: PhysDamClassBalance::default(),
            alt_game_shield_blocks: config.alt_shield_blocks,
            randomize_auto_attack_damage: config.randomize_auto_attack_damage,
            min_npc_level_dmg_penalty: config.min_npc_level_dmg_penalty,
            npc_dmg_penalty: &config.npc_dmg_penalty,
            npc_crit_dmg_penalty: &config.npc_crit_dmg_penalty,
        },
        shld,
        crit,
        ss,
        rng,
    )
}

// ------------------------------------------------------------------ the tests

#[test]
fn hit_miss_vectors() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("formulas_hit_miss")) else {
        return;
    };
    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 1080);
    for case in cases {
        let id = s(&case["id"]);
        let inter = &case["intermediate"];
        let placement = &case["placement"];
        // The condition bonus is its own function: check it against Java's number too.
        let bonus = condition_bonus(
            &ctx.config.hit_bonus,
            i(&placement["z_diff"]),
            0,
            b(&case["night"]),
            position(placement),
        );
        assert_eq!(bonus, f(&inter["condition_bonus"]), "{id}: condition bonus");

        let mut rng = Scripted::from_case(case);
        let missed = run_hit_miss(
            &ctx,
            i(&inter["accuracy"]),
            i(&inter["target_evasion"]),
            placement,
            b(&case["night"]),
            &mut rng,
        );
        assert_eq!(missed, b(&case["result"]), "{id}: result");
        rng.finish(id);
    }
    eprintln!("hit_miss: {} cases", cases.len());
}

#[test]
fn crit_vectors() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("formulas_crit")) else {
        return;
    };
    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 390);
    for case in cases {
        let id = s(&case["id"]);
        let mut rng = Scripted::from_case(case);
        let crit = run_crit(
            ctx.get(s(&case["attacker"])),
            ctx.get(s(&case["target"])),
            f(&case["intermediate"]["rate"]),
            &mut rng,
        );
        assert_eq!(crit, b(&case["result"]), "{id}: result");
        rng.finish(id);
    }
    eprintln!("crit: {} cases", cases.len());
}

#[test]
fn shield_vectors() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("formulas_shield")) else {
        return;
    };
    let geometry = Geometry::from(&file);
    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 402);
    for case in cases {
        let id = s(&case["id"]);
        let mut rng = Scripted::from_case(case);
        let result = run_shield(
            &ctx,
            &geometry,
            ctx.get(s(&case["attacker"])),
            ctx.get(s(&case["target"])),
            &case["placement"],
            (b(&case["target_has_shield"]), f(&case["target_shield_rate"])),
            &mut rng,
        );
        assert_eq!(i32::from(result), i(&case["result"]), "{id}: result");
        rng.finish(id);
    }
    eprintln!("shield: {} cases", cases.len());
}

#[test]
fn phys_dam_vectors() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("formulas_phys_dam")) else {
        return;
    };
    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 1050);
    for case in cases {
        let id = s(&case["id"]);
        let inter = &case["intermediate"];
        let numbers = PhysDamNumbers {
            p_atk: f(&inter["p_atk"]),
            p_def: f(&inter["p_def"]),
            shield_def: i(&inter["shield_def"]),
            attacker_level: i(&inter["attacker_level"]),
            target_level: i(&inter["target_level"]),
        };
        let mut rng = Scripted::from_case(case);
        let damage = run_phys_dam(
            &ctx,
            ctx.get(s(&case["attacker"])),
            ctx.get(s(&case["target"])),
            &case["placement"],
            &numbers,
            (
                u8::try_from(i(&case["shld"])).unwrap(),
                b(&case["crit"]),
                b(&case["ss"]),
            ),
            &mut rng,
        );
        assert_eq!(damage, f(&case["result"]), "{id}: damage");
        rng.finish(id);
    }
    eprintln!("phys_dam: {} cases", cases.len());
}

/// The per-pair numbers of the other files, for the files that record none of their own.
#[test]
fn attack_hit_vectors() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("formulas_attack_hit")) else {
        return;
    };
    let (Some(hit_miss), Some(crit), Some(shield), Some(phys)) = (
        vector("formulas_hit_miss"),
        vector("formulas_crit"),
        vector("formulas_shield"),
        vector("formulas_phys_dam"),
    ) else {
        return;
    };
    let geometry = Geometry::from(&file);
    let pair = |case: &Value| format!("{}>{}", s(&case["attacker"]), s(&case["target"]));

    let mut accuracy_evasion = HashMap::new();
    for c in hit_miss["cases"].as_array().unwrap() {
        accuracy_evasion.insert(
            pair(c),
            (
                i(&c["intermediate"]["accuracy"]),
                i(&c["intermediate"]["target_evasion"]),
            ),
        );
    }
    let mut crit_rate = HashMap::new();
    for c in crit["cases"].as_array().unwrap() {
        crit_rate.insert(pair(c), f(&c["intermediate"]["rate"]));
    }
    let mut shield_numbers = HashMap::new();
    for c in shield["cases"].as_array().unwrap() {
        shield_numbers.insert(pair(c), (b(&c["target_has_shield"]), f(&c["target_shield_rate"])));
    }
    let mut damage_numbers = HashMap::new();
    for c in phys["cases"].as_array().unwrap() {
        let inter = &c["intermediate"];
        damage_numbers.insert(
            pair(c),
            (
                f(&inter["p_atk"]),
                f(&inter["p_def"]),
                i(&inter["shield_def"]),
                i(&inter["attacker_level"]),
                i(&inter["target_level"]),
            ),
        );
    }

    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 240);
    for case in cases {
        let id = s(&case["id"]);
        let key = pair(case);
        let attacker = ctx.get(s(&case["attacker"]));
        let target = ctx.get(s(&case["target"]));
        let placement = &case["placement"];
        let mut rng = Scripted::from_case(case);

        // The order of Creature.doAttackHitSimple.
        let (accuracy, evasion) = accuracy_evasion[&key];
        let miss = run_hit_miss(&ctx, accuracy, evasion, placement, false, &mut rng);
        let (mut shld, mut crit_hit, mut damage) = (0_u8, false, 0_i32);
        if !miss {
            shld = run_shield(
                &ctx,
                &geometry,
                attacker,
                target,
                placement,
                shield_numbers[&key],
                &mut rng,
            );
            crit_hit = run_crit(attacker, target, crit_rate[&key], &mut rng);
            let (p_atk, p_def, shield_def, attacker_level, target_level) = damage_numbers[&key];
            let numbers = PhysDamNumbers {
                p_atk,
                p_def,
                shield_def,
                attacker_level,
                target_level,
            };
            damage = run_phys_dam(
                &ctx,
                attacker,
                target,
                placement,
                &numbers,
                (shld, crit_hit, b(&case["ss"])),
                &mut rng,
            ) as i32;
        }

        let result = &case["result"];
        assert_eq!(miss, b(&result["miss"]), "{id}: miss");
        assert_eq!(i32::from(shld), i(&result["shld"]), "{id}: shield");
        assert_eq!(crit_hit, b(&result["crit"]), "{id}: crit");
        assert_eq!(damage, i(&result["damage"]), "{id}: damage");
        rng.finish(id);
    }
    eprintln!("attack_hit: {} cases", cases.len());
}

#[test]
fn attack_timing_vectors() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("formulas_attack_timing")) else {
        return;
    };
    let creatures = file["creatures"].as_array().unwrap();
    assert_eq!(creatures.len(), 17);
    for c in creatures {
        let name = s(&c["creature"]);
        let p_atk_spd = f(&c["p_atk_spd"]);
        let time_atk = calculate_time_between_attacks(p_atk_spd);
        assert_eq!(time_atk, i(&c["time_atk"]), "{name}: time_atk");
        assert_eq!(time_atk / 2, i(&c["time_to_hit"]), "{name}: time_to_hit");
        let creature = ctx.get(name);
        let reuse = calculate_reuse_time(&ReuseInput {
            p_atk_spd,
            weapon_reuse_delay: creature.has_weapon().then_some(creature.weapon_reuse_delay),
            is_transformed: false,
            attack_type: match creature.attack_type.as_str() {
                "BOW" => ReuseAttackType::Bow,
                "CROSSBOW" => ReuseAttackType::Crossbow,
                _ => ReuseAttackType::Other,
            },
            weapon_reuse_modifier: creature.probe("atk_reuse@1"),
        });
        assert_eq!(reuse, i(&c["reuse"]), "{name}: reuse");
    }

    let rates = file["calc_p_atk_spd"].as_array().unwrap();
    assert_eq!(rates.len(), 15);
    for c in rates {
        assert_eq!(
            calc_p_atk_spd(f(&c["rate"])),
            i(&c["result"]),
            "rate {}",
            c["rate"]
        );
    }

    let skills = file["calc_atk_spd"].as_array().unwrap();
    assert_eq!(skills.len(), 32);
    for c in skills {
        let result = calc_atk_spd(
            b(&c["skill_is_magic"]),
            f(&c["skill_time"]),
            i(&c["m_atk_spd"]),
            f(&c["p_atk_spd"]),
        );
        assert_eq!(
            result,
            i(&c["result"]),
            "{} skill {}",
            c["creature"],
            c["skill_id"]
        );
    }
    eprintln!(
        "attack_timing: {} creatures, {} rates, {} skill times",
        creatures.len(),
        rates.len(),
        skills.len()
    );
}

#[test]
fn atk_break_vectors() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("formulas_atk_break")) else {
        return;
    };
    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 162);
    for case in cases {
        let id = s(&case["id"]);
        let inter = &case["intermediate"];
        let state = s(&case["state"]);
        let attack_cancel = scaled(f(&inter["attack_cancel_probe"]), 50.0);
        let mut rng = Scripted::from_case(case);
        let result = calc_atk_break(
            &AtkBreakInput {
                is_channeling: false,
                is_casting_now: state == "cast",
                is_attacking_now: state == "bow_attack",
                active_weapon_is_bow: state == "bow_attack",
                is_raid: b(&inter["is_raid"]),
                is_invul: b(&inter["is_invul"]),
                men_bonus: f(&inter["men_bonus"]),
                attack_cancel: &attack_cancel,
                alt_game_cancel_cast: ctx.config.cancel_cast,
                alt_game_cancel_bow: ctx.config.cancel_bow,
            },
            f(&case["damage"]),
            &mut rng,
        );
        assert_eq!(result, b(&case["result"]), "{id}: result");
        rng.finish(id);
    }
    eprintln!("atk_break: {} cases", cases.len());
}

#[test]
fn lvl_bonus_vectors() {
    let Some(file) = vector("formulas_lvl_bonus") else {
        return;
    };
    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 270);
    for case in cases {
        let result = calc_lvl_bonus_mod(
            i(&case["skill_magic_level"]),
            i(&case["skill_lvl_bonus_rate"]),
            i(&case["attacker_level"]),
            i(&case["target_level"]),
        );
        assert_eq!(result, f(&case["result"]), "{}", case["id"]);
    }
    eprintln!("lvl_bonus: {} cases", cases.len());
}

// ------------------------------------------------------------------ regeneration

/// A creature of `formulas_regen.json`: a formula creature or a `stats_*.json` case.
struct RegenCreature {
    player: bool,
    class_id: i32,
    npc_id: i32,
    level: i32,
    level_mod: f64,
    con_bonus: f64,
    men_bonus: f64,
    probes: BTreeMap<String, f64>,
}

fn regen_creatures() -> Option<HashMap<String, RegenCreature>> {
    let formulas = vector("formulas_creatures")?;
    let mut out = HashMap::new();
    let mut add = |name: String, input: &Value, raw: &Value| {
        let player = s(&input["kind"]) == "player";
        out.insert(
            name,
            RegenCreature {
                player,
                class_id: if player { i(&input["class_id"]) } else { 0 },
                npc_id: if player { 0 } else { i(&input["npc_id"]) },
                level: i(&raw["level"]),
                level_mod: f(&raw["level_mod"]),
                con_bonus: f(&raw["base_stat_bonus"]["con"]),
                men_bonus: f(&raw["base_stat_bonus"]["men"]),
                probes: probes_of(raw),
            },
        );
    };
    for c in formulas["creatures"].as_array().unwrap() {
        add(s(&c["name"]).to_string(), &c["input"], &c["expected"]["raw"]);
    }
    for file in ["stats_players_naked", "stats_players_gear", "stats_npcs"] {
        let v = vector(file)?;
        for c in v["cases"].as_array().unwrap() {
            add(
                format!("stat_{}", s(&c["id"])),
                &c["input"],
                &c["expected"]["raw"],
            );
        }
    }
    Some(out)
}

fn posture(state: &str) -> RegenPosture {
    match state {
        "stand" => RegenPosture::Standing,
        "sit" => RegenPosture::Sitting,
        "move_run" => RegenPosture::Running,
        "move_walk" => RegenPosture::Walking,
        other => panic!("unknown state {other}"),
    }
}

/// How a creature's stat engine folds a base value into `REGENERATE_*_RATE`. The vectors hold one
/// probe per stat, `calcStat(stat, 100)`, which a plain multiplier (`x * probe / 100`) and a plain
/// addition (`x + probe - 100`) both explain; the tests try both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatModel {
    Mul,
    Add,
}

fn model_fn(model: StatModel, probe: f64) -> impl Fn(f64) -> f64 {
    move |value| {
        if probe == 100.0 {
            value
        } else {
            match model {
                StatModel::Mul => value * (probe / 100.0),
                StatModel::Add => value + (probe - 100.0),
            }
        }
    }
}

/// Java's numbers match to the bit except where a stat is rebuilt from a probe.
fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= expected.abs() * 1e-12 + 1e-15
}

/// Runs the three regeneration formulas for one case and compares them with Java's rates.
fn check_regen_case(
    config: &Config,
    c: &RegenCreature,
    case: &Value,
    base: (f64, f64, f64),
    level_mod: f64,
    (con_bonus, men_bonus): (f64, f64),
    models: [StatModel; 3],
) -> Result<(), String> {
    let id = s(&case["id"]);
    let state = s(&case["state"]);
    let is_raid = b(&case["is_raid"]);
    let hp_rate = model_fn(models[0], c.probes["regenerate_hp_rate@100"]);
    let mp_rate = model_fn(models[1], c.probes["regenerate_mp_rate@100"]);
    let cp_rate = model_fn(models[2], c.probes["regenerate_cp_rate@100"]);
    let (kind_hp, kind_mp) = if c.player {
        (
            RegenKind::Player {
                base_regen: base.0,
                level_mod,
                stat_bonus: con_bonus,
                posture: posture(state),
            },
            RegenKind::Player {
                base_regen: base.1,
                level_mod,
                stat_bonus: men_bonus,
                posture: posture(state),
            },
        )
    } else {
        (
            RegenKind::Npc {
                base_reg: base.0 as f32,
            },
            RegenKind::Npc {
                base_reg: base.1 as f32,
            },
        )
    };
    let hp = calc_hp_regen(&HpRegenInput {
        kind: kind_hp,
        is_raid,
        hp_regen_multiplier: config.hp_regen,
        raid_hp_regen_multiplier: config.raid_hp_regen,
        champion_hp_regen: None,
        multipliers: &[],
        mother_tree_hp_regen_bonus: 0,
        regenerate_hp_rate: &hp_rate,
    });
    if !close(hp, f(&case["hp"])) {
        return Err(format!("{id}: hp {hp} vs Java {}", f(&case["hp"])));
    }
    let mp = calc_mp_regen(&MpRegenInput {
        kind: kind_mp,
        is_raid,
        mp_regen_multiplier: config.mp_regen,
        raid_mp_regen_multiplier: config.raid_mp_regen,
        multipliers: &[],
        mother_tree_mp_regen_bonus: 0,
        regenerate_mp_rate: &mp_rate,
    });
    if !close(mp, f(&case["mp"])) {
        return Err(format!("{id}: mp {mp} vs Java {}", f(&case["mp"])));
    }
    if c.player {
        let cp = calc_cp_regen(&CpRegenInput {
            base_cp_regen: base.2,
            level_mod,
            con_bonus,
            posture: posture(state),
            cp_regen_multiplier: config.cp_regen,
            regenerate_cp_rate: &cp_rate,
        });
        if !close(cp, f(&case["cp"])) {
            return Err(format!("{id}: cp {cp} vs Java {}", f(&case["cp"])));
        }
    } else if case.get("cp").is_some() {
        return Err(format!("{id}: only players have CP"));
    }
    Ok(())
}

fn by_creature(cases: &[Value]) -> BTreeMap<&str, Vec<&Value>> {
    let mut groups: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for case in cases {
        groups.entry(s(&case["creature"])).or_default().push(case);
    }
    groups
}

/// The real thing: base regeneration from the class and NPC templates of the datapack. All the
/// states of a creature have to match with one choice of stat model per stat.
#[test]
fn regen_vectors_with_the_datapack() {
    let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"), PathBuf::from);
    if !root.join("data/stats/players/templates").is_dir() || !root.join("data/stats/npcs").is_dir() {
        eprintln!("skipped: {} has no datapack", root.display());
        return;
    }
    let (Some(ctx), Some(file), Some(creatures)) = (Ctx::load(), vector("formulas_regen"), regen_creatures())
    else {
        return;
    };
    let players = PlayerTemplates::load(&root, l2_data::DEFAULT_MAX_PLAYER_LEVEL);
    let npcs = NpcTemplates::load(&root, &NpcLoadOptions::default());

    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 393);
    let mut additive = 0;
    for (name, group) in by_creature(cases) {
        let c = &creatures[name];
        let level = usize::try_from(c.level).unwrap();
        let base = if c.player {
            let t = players.get_by_id(c.class_id).unwrap();
            (
                t.base_hp_regen(level).unwrap(),
                t.base_mp_regen(level).unwrap(),
                t.base_cp_regen(level).unwrap(),
            )
        } else {
            let t = npcs.get(c.npc_id).unwrap();
            (f64::from(t.base_hp_reg), f64::from(t.base_mp_reg), 0.0)
        };
        let mut last_error = String::new();
        let mut matched = None;
        for hp in [StatModel::Mul, StatModel::Add] {
            for mp in [StatModel::Mul, StatModel::Add] {
                for cp in [StatModel::Mul, StatModel::Add] {
                    let models = [hp, mp, cp];
                    let result = group.iter().try_for_each(|case| {
                        assert_eq!(c.level, i(&case["level"]));
                        check_regen_case(
                            &ctx.config,
                            c,
                            case,
                            base,
                            c.level_mod,
                            (c.con_bonus, c.men_bonus),
                            models,
                        )
                    });
                    match result {
                        Ok(()) if matched.is_none() => matched = Some(models),
                        Ok(()) => {}
                        Err(e) => last_error = e,
                    }
                }
            }
        }
        let models = matched.unwrap_or_else(|| panic!("{name}: no stat model matches ({last_error})"));
        additive += models.iter().filter(|m| **m == StatModel::Add).count();
    }
    eprintln!(
        "regen (datapack): {} cases, {additive} stats explained by an addition",
        cases.len()
    );
}

/// No datapack needed: the stand case of each creature fixes its base regeneration (divided by
/// the stat multiplier and the standing bonus); the sit / run / walk cases of the same creature
/// then have to follow from the posture factors alone.
#[test]
fn regen_vectors_postures() {
    let (Some(ctx), Some(file), Some(creatures)) = (Ctx::load(), vector("formulas_regen"), regen_creatures())
    else {
        return;
    };
    let cases = file["cases"].as_array().unwrap();
    let mut checked = 0;
    for (name, group) in by_creature(cases) {
        let c = &creatures[name];
        if !c.player {
            // A monster has no posture: every state returns the same rate.
            for case in &group {
                assert_eq!(
                    f(&case["hp"]),
                    f(&group[0]["hp"]),
                    "{name}: monster hp does not depend on the state"
                );
            }
            continue;
        }
        let stand = group.iter().find(|c| s(&c["state"]) == "stand").unwrap();
        let multiplier = |config: f64, probe: &str| c.probes[probe] / 100.0 * config * 1.1;
        // base values that reproduce the stand rates (level mod and stat bonus folded into them)
        let hp = f(&stand["hp"]) / multiplier(ctx.config.hp_regen, "regenerate_hp_rate@100");
        let mp = f(&stand["mp"]) / multiplier(ctx.config.mp_regen, "regenerate_mp_rate@100");
        let cp = f(&stand["cp"]) / multiplier(ctx.config.cp_regen, "regenerate_cp_rate@100");
        for case in group {
            // The base was derived with a multiplier, so the multiplier model is the one to use.
            check_regen_case(
                &ctx.config,
                c,
                case,
                (hp, mp, cp),
                1.0,
                (1.0, 1.0),
                [StatModel::Mul; 3],
            )
            .unwrap_or_else(|e| panic!("{e}"));
            checked += 1;
        }
    }
    assert!(checked > 0);
    eprintln!("regen (postures): {checked} player cases");
}
