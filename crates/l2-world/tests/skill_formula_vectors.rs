#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The skill formulas of `l2_world::formulas` against the vectors the real L2JMobius `Formulas`
//! class (and the real `Heal` effect) wrote (`java-vectors/out/skill_formulas_*.json`, M5-JV-03).
//!
//! How a case is replayed:
//! - The creatures come from `skill_formulas_creatures.json` (flags, stat probes, base stat
//!   bonuses, trait tables, elements) and the skills from `skill_formulas_skills.json`. A formula
//!   input that Java reads from a stat is the probe `STAT@init` of the creature; a `calcStat`
//!   whose base depends on the formula (`BLOW_RATE`, `DEFENCE_CRITICAL_RATE`, `HEAL_EFFECT`,
//!   `PHYSICAL_SKILL_POWER`) is rebuilt from the creature's extra stat functions (`mods`: the
//!   `mul` ones, then the `add` ones, like the calculator orders them), and checked against the
//!   probe; where the creature has other functions on that stat the probe's ratio is used.
//! - The numbers Java recorded in the `intermediate` block of a case (p.atk, m.def, skill power,
//!   trait and attribute bonus, ...) are fed to the composite formulas; the pieces that have
//!   their own files (trait bonus, attribute bonus) are also recomputed from the creature data
//!   and compared with the recorded ones.
//! - Every random draw is answered from the case's `rnd` list (`returned`), and the test checks
//!   that the formula made exactly these calls (`method`, `bound`) in this order. For
//!   `nextGaussian` the harness `Rnd` is not a real Gaussian: the recorded value is replayed.
//! - A missing file skips with a message (CI has all of them: they are committed).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use l2_data::skill::TraitType;
use l2_world::formulas::{
    AbnormalTimeInput, AbnormalTimeResists, BlowClassBalance, BlowInput, BlowSuccessInput,
    EffectSuccessInput, HealInput, HealWeaponGrade, MCriticalHitInput, MagicAffectedInput,
    MagicDamClassBalance, MagicDamInput, MagicFailure, MagicSuccessInput, ManaDamInput, Position, Rnd,
    SkillCastTimeInput, SkillCritInput, SkillMasteryInput, SkillPhysDamClassBalance, SkillPhysDamInput,
    SkillPower, SkillReuseInput, TraitInput, TraitKind, attribute_element, calc_atk_spd,
    calc_attack_trait_bonus, calc_attribute_bonus_skill, calc_backstab_damage, calc_blow_damage,
    calc_blow_success, calc_effect_abnormal_time, calc_effect_success, calc_general_trait_bonus,
    calc_heal_amount, calc_lvl_bonus_mod, calc_m_critical_hit, calc_magic_affected, calc_magic_dam,
    calc_magic_success, calc_mana_dam, calc_mcrit, calc_physical_skill_evasion, calc_skill_cast_time,
    calc_skill_crit, calc_skill_mastery, calc_skill_phys_dam, calc_skill_reuse_delay,
    calc_weapon_trait_bonus, calculate_skill_resurrect_restore_percent,
};
use serde_json::Value;

// ------------------------------------------------------------------ loading

fn vector(name: &str) -> Option<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../java-vectors/out")
        .join(format!("skill_formulas_{name}.json"));
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))),
        Err(_) => {
            eprintln!("skipped: {} is missing", path.display());
            None
        }
    }
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
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

fn cases(file: &Value) -> &Vec<Value> {
    file["cases"].as_array().unwrap()
}

/// Same bits (so a NaN and a signed zero are compared too).
fn assert_same(id: &str, what: &str, got: f64, want: f64) {
    assert!(
        got == want || (got.is_nan() && want.is_nan()),
        "{id}: {what}: got {got:?}, want {want:?}"
    );
}

// ------------------------------------------------------------------ the scripted RNG

struct Draw {
    method: String,
    bound: String,
    returned: String,
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
                returned: s(&d["returned"]).to_string(),
            })
            .collect();
        Self {
            expected,
            next: 0,
            calls: Vec::new(),
        }
    }

    fn answer(&mut self, method: &str, bound: String) -> String {
        self.calls.push((method.to_string(), bound));
        let value = self
            .expected
            .get(self.next)
            .map_or_else(|| "0".to_string(), |d| d.returned.clone());
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
        self.answer("get_int", bound.to_string()).parse().unwrap()
    }
    fn get_int_range(&mut self, origin: i32, bound: i32) -> i32 {
        if origin >= bound {
            return origin;
        }
        self.answer("get_int_range", format!("{origin},{bound}"))
            .parse()
            .unwrap()
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
        self.answer("nextGaussian", String::new()).parse().unwrap()
    }
}

// ------------------------------------------------------------------ creatures, skills, config

struct Cr {
    name: String,
    mods: Vec<String>,
    level: i32,
    is_player: bool,
    is_raid: bool,
    is_attackable: bool,
    attack_type: String,
    has_weapon: bool,
    probes: BTreeMap<String, f64>,
    bonus: BTreeMap<String, f64>,
    atk_traits: HashMap<String, (f32, i32)>,
    def_traits: HashMap<String, (f32, i32)>,
    invul: HashSet<String>,
    attack_power: Vec<i32>,
    defence_value: Vec<i32>,
    attack_element: i32,
    default_defence: i32,
}

fn trait_table(v: &Value) -> HashMap<String, (f32, i32)> {
    v.as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), (f(&v[0]) as f32, i(&v[1]))))
        .collect()
}

impl Cr {
    fn from(c: &Value) -> Self {
        let raw = &c["expected"]["raw"];
        let mut probes: BTreeMap<String, f64> = BTreeMap::new();
        for source in [&raw["probes"], &c["skill_probes"]] {
            for (k, v) in source.as_object().unwrap() {
                probes.insert(k.clone(), f(v));
            }
        }
        let flags = &c["flags"];
        let elements = &c["elements"];
        let ints = |v: &Value| v.as_array().unwrap().iter().map(i).collect::<Vec<_>>();
        Cr {
            name: s(&c["name"]).to_string(),
            mods: c["mods"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| s(m).to_string())
                .collect(),
            level: i(&raw["level"]),
            is_player: b(&flags["is_player"]),
            is_raid: b(&flags["is_raid"]),
            is_attackable: b(&flags["is_attackable"]),
            attack_type: s(&flags["attack_type"]).to_string(),
            has_weapon: flags["active_weapon_item_id"].as_i64().unwrap() != 0,
            probes,
            bonus: raw["base_stat_bonus"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), f(v)))
                .collect(),
            atk_traits: trait_table(&c["traits"]["attack"]),
            def_traits: trait_table(&c["traits"]["defence"]),
            invul: c["traits"]["invul"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| s(t).to_string())
                .collect(),
            attack_power: ints(&elements["attack_power"]),
            defence_value: ints(&elements["defence_value"]),
            attack_element: i(&elements["attack_element"]),
            default_defence: i(&elements["default_defence_value"]),
        }
    }

    fn probe(&self, key: &str) -> f64 {
        *self
            .probes
            .get(key)
            .unwrap_or_else(|| panic!("{}: no probe {key}", self.name))
    }

    fn bonus(&self, stat: &str) -> f64 {
        self.bonus[stat]
    }

    fn random_damage(&self) -> i32 {
        self.probe("random_damage@0") as i32
    }

    fn player_level(&self) -> Option<i32> {
        self.is_player.then_some(self.level)
    }

    fn attack_element_value(&self, element: i32) -> i32 {
        usize::try_from(element)
            .ok()
            .filter(|e| *e < 6)
            .map_or(0, |e| self.attack_power[e])
    }

    fn defence_element_value(&self, element: i32) -> i32 {
        usize::try_from(element)
            .ok()
            .filter(|e| *e < 6)
            .map_or(self.default_defence, |e| self.defence_value[e])
    }

    fn bow_or_crossbow(&self) -> bool {
        self.attack_type == "BOW" || self.attack_type == "CROSSBOW"
    }

    /// The trait input of one trait for `self` attacking `target`.
    fn trait_input(&self, target: &Cr, name: &str) -> TraitInput {
        let attack = self.atk_traits.get(name).copied().unwrap_or((1.0, 0));
        let defence = target.def_traits.get(name).copied().unwrap_or((1.0, 0));
        TraitInput {
            target_invul: target.invul.contains(name),
            attacker_has_attack_trait: attack.1 > 0,
            target_has_defence_trait: defence.1 > 0,
            attack_trait: attack.0,
            defence_trait: defence.0,
        }
    }

    fn defence_trait(&self, name: &str) -> f32 {
        self.def_traits.get(name).map_or(1.0, |t| t.0)
    }
}

/// `calcStat(STAT, base)` for a stat whose base the formula decides: the creature's own `mul`
/// functions, then its `add` ones; or, when the creature has other functions on the stat, the
/// ratio of the probe `probe_key` (taken at `init`).
fn stat_fn(c: &Cr, stat: &str, probe_key: &str, init: f64) -> Box<dyn Fn(f64) -> f64> {
    let mut muls = Vec::new();
    let mut adds = Vec::new();
    for m in &c.mods {
        let parts: Vec<&str> = m.split(':').collect();
        if parts.len() == 3 && parts[1] == stat {
            let v: f64 = parts[2].parse().unwrap();
            match parts[0] {
                "mul" => muls.push(v),
                "add" => adds.push(v),
                _ => {}
            }
        }
    }
    let probe = c.probe(probe_key);
    let model = move |x: f64| {
        let mut v = x;
        for m in &muls {
            v *= m;
        }
        for a in &adds {
            v += a;
        }
        v
    };
    if model(init) == probe {
        Box::new(model)
    } else {
        Box::new(move |x| if probe == init { x } else { x * (probe / init) })
    }
}

struct Skill {
    key: String,
    is_magic: bool,
    is_physical: bool,
    is_static: bool,
    is_active: bool,
    is_passive: bool,
    is_toggle: bool,
    is_debuff: bool,
    has_negative_effect: bool,
    power: f64,
    pvp_power: f64,
    pve_power: f64,
    magic_level: i32,
    lvl_bonus_rate: i32,
    activate_rate: i32,
    min_chance: i32,
    max_chance: i32,
    basic_property: String,
    trait_name: String,
    element: i32,
    element_power: i32,
    base_crit_rate: i32,
    blow_chance: i32,
    hit_time: i32,
    cool_time: i32,
    reuse_delay: i32,
    static_reuse: bool,
    item_consume_count: i32,
    abnormal_time: i32,
    is_channeling: bool,
    channeling_skill_id: i32,
}

impl Skill {
    fn from(v: &Value) -> Self {
        Skill {
            key: s(&v["key"]).to_string(),
            is_magic: b(&v["is_magic"]),
            is_physical: b(&v["is_physical"]),
            is_static: b(&v["is_static"]),
            is_active: b(&v["is_active"]),
            is_passive: b(&v["is_passive"]),
            is_toggle: b(&v["is_toggle"]),
            is_debuff: b(&v["is_debuff"]),
            has_negative_effect: b(&v["has_negative_effect"]),
            power: f(&v["power"]),
            pvp_power: f(&v["pvp_power"]),
            pve_power: f(&v["pve_power"]),
            magic_level: i(&v["magic_level"]),
            lvl_bonus_rate: i(&v["lvl_bonus_rate"]),
            activate_rate: i(&v["activate_rate"]),
            min_chance: i(&v["min_chance"]),
            max_chance: i(&v["max_chance"]),
            basic_property: s(&v["basic_property"]).to_string(),
            trait_name: s(&v["trait"]).to_string(),
            element: i(&v["element"]),
            element_power: i(&v["element_power"]),
            base_crit_rate: i(&v["base_crit_rate"]),
            blow_chance: i(&v["blow_chance"]),
            hit_time: i(&v["hit_time"]),
            cool_time: i(&v["cool_time"]),
            reuse_delay: i(&v["reuse_delay"]),
            static_reuse: b(&v["static_reuse"]),
            item_consume_count: i(&v["item_consume_count"]),
            abnormal_time: i(&v["abnormal_time"]),
            is_channeling: b(&v["is_channeling"]),
            channeling_skill_id: i(&v["channeling_skill_id"]),
        }
    }

    fn power(&self) -> SkillPower {
        SkillPower {
            power: self.power,
            pvp_power: self.pvp_power,
            pve_power: self.pve_power,
            hp_link: None,
        }
    }
}

struct Config {
    magic_failures: bool,
    randomize_magical: bool,
    randomize_physical: bool,
    alt_shield_blocks: bool,
    magic_success_by_skill_magic_level: bool,
    debuff_duration_uses_resists: bool,
    min_dmg_penalty: i32,
    min_magic_penalty: i32,
    skill_dmg_penalty: Vec<f32>,
    skill_chance_penalty: Vec<f32>,
}

fn floats(v: &Value) -> Vec<f32> {
    v.as_array().unwrap().iter().map(|x| f(x) as f32).collect()
}

struct Ctx {
    cfg: Config,
    creatures: HashMap<String, Cr>,
    skills: HashMap<String, Skill>,
}

impl Ctx {
    fn load() -> Option<Self> {
        let file = vector("creatures")?;
        let skills_file = vector("skills")?;
        let c = &file["config"];
        assert_eq!(
            c["ClassBalance_arrays_not_all_1"].as_array().unwrap().len(),
            0,
            "the vectors are only valid with the default ClassBalance"
        );
        let cfg = Config {
            magic_failures: b(&c["MagicFailures"]),
            randomize_magical: b(&c["RandomizeMagicalSkillDamage"]),
            randomize_physical: b(&c["RandomizePhysicalSkillDamage"]),
            alt_shield_blocks: b(&c["AltShieldBlocks"]),
            magic_success_by_skill_magic_level: b(&c["CalculateMagicSuccessBySkillMagicLevel"]),
            debuff_duration_uses_resists: b(&c["DebuffDurationUsesResists"]),
            min_dmg_penalty: i(&c["MinNPCLevelForDmgPenalty"]),
            min_magic_penalty: i(&c["MinNPCLevelForMagicPenalty"]),
            skill_dmg_penalty: floats(&c["NpcSkillDmgPenalty"]),
            skill_chance_penalty: floats(&c["NpcSkillChancePenalty"]),
        };
        let creatures = file["creatures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let cr = Cr::from(c);
                (cr.name.clone(), cr)
            })
            .collect();
        let skills = skills_file["skills"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let sk = Skill::from(v);
                (sk.key.clone(), sk)
            })
            .collect();
        Some(Self {
            cfg,
            creatures,
            skills,
        })
    }

    fn cr(&self, name: &str) -> &Cr {
        self.creatures
            .get(name)
            .unwrap_or_else(|| panic!("no creature {name}"))
    }

    fn skill(&self, key: &str) -> &Skill {
        self.skills.get(key).unwrap_or_else(|| panic!("no skill {key}"))
    }

    /// The attacker, target and skill named by a case.
    fn pair(&self, case: &Value) -> (&Cr, &Cr, &Skill) {
        (
            self.cr(s(&case["attacker"])),
            self.cr(s(&case["target"])),
            self.skill(s(&case["skill"])),
        )
    }

    /// `Formulas.calcGeneralTraitBonus(att, tgt, skill.getTraitType(), false)`.
    fn skill_trait_bonus(&self, att: &Cr, tgt: &Cr, sk: &Skill) -> f64 {
        let t = TraitType::from_name(&sk.trait_name).unwrap();
        calc_general_trait_bonus(
            TraitKind::of(t == TraitType::None, t.kind()),
            &att.trait_input(tgt, &sk.trait_name),
            false,
        )
    }

    /// `Formulas.calcAttributeBonus(att, tgt, skill)`; `skill` is `None` for no skill.
    fn attribute_bonus(&self, att: &Cr, tgt: &Cr, sk: Option<&Skill>) -> f64 {
        let skill_element = sk.map(|k| k.element as i8);
        let element = i32::from(attribute_element(skill_element, att.attack_element as i8));
        let power = sk.filter(|k| k.element != -1).map(|k| k.element_power);
        calc_attribute_bonus_skill(
            power,
            att.attack_element_value(element),
            tgt.defence_element_value(element),
        )
    }

    fn magic_success_input<'a>(&'a self, att: &Cr, tgt: &Cr, sk: &Skill) -> MagicSuccessInput<'a> {
        MagicSuccessInput {
            skill_power: sk.power,
            skill_activate_rate: sk.activate_rate,
            calculate_magic_success_by_skill_magic_level: self.cfg.magic_success_by_skill_magic_level,
            skill_magic_level: sk.magic_level,
            attacker_level: att.level,
            target_level: tgt.level,
            target_is_attackable: tgt.is_attackable,
            target_is_raid: tgt.is_raid,
            target_is_raid_minion: false,
            min_npc_level_magic_penalty: self.cfg.min_magic_penalty,
            attacker_player_level: att.player_level(),
            npc_skill_chance_penalty: &self.cfg.skill_chance_penalty,
            magic_success_res: tgt.probe("magic_success_res@1"),
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

fn shld(case: &Value) -> u8 {
    u8::try_from(i(&case["shld"])).unwrap()
}

// ------------------------------------------------------------------ the tests

#[test]
fn traits() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("traits")) else {
        return;
    };
    let mut counts = [0; 3];
    for case in cases(&file) {
        let id = s(&case["id"]);
        let att = ctx.cr(s(&case["attacker"]));
        let tgt = ctx.cr(s(&case["target"]));
        let want = f(&case["result"]);
        match s(&case["kind"]) {
            "general" => {
                let name = s(&case["trait"]);
                let kind = TraitKind::of(name == "NONE", i(&case["trait_type"]));
                // The datapack enum agrees with the vector about the type.
                let t = TraitType::from_name(name).unwrap();
                assert_eq!(t.kind(), i(&case["trait_type"]), "{id}: trait type");
                assert_eq!(t.ordinal() as i32, i(&case["ordinal"]), "{id}: ordinal");
                let got = calc_general_trait_bonus(
                    kind,
                    &att.trait_input(tgt, name),
                    b(&case["ignore_resistance"]),
                );
                assert_same(id, "general trait bonus", got, want);
                counts[0] += 1;
            }
            "weapon" => {
                assert_eq!(s(&case["attack_type"]), att.attack_type, "{id}");
                let got = calc_weapon_trait_bonus(tgt.defence_trait(s(&case["attack_type_trait"])));
                assert_same(id, "weapon trait bonus", got, want);
                counts[1] += 1;
            }
            "attack" => {
                let weaknesses: Vec<TraitInput> = TraitType::ALL
                    .iter()
                    .filter(|t| t.kind() == 2)
                    .map(|t| att.trait_input(tgt, t.name()))
                    .collect();
                let got = calc_attack_trait_bonus(tgt.defence_trait(&att.attack_type), &weaknesses);
                assert_same(id, "attack trait bonus", got, want);
                counts[2] += 1;
            }
            other => panic!("{id}: unknown kind {other}"),
        }
    }
    eprintln!(
        "traits: {} cases (general {}, weapon {}, attack {})",
        cases(&file).len(),
        counts[0],
        counts[1],
        counts[2]
    );
    assert_eq!(counts.iter().sum::<usize>(), cases(&file).len());
}

#[test]
fn attribute() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("attribute")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let att = ctx.cr(s(&case["attacker"]));
        let tgt = ctx.cr(s(&case["target"]));
        // A JSON null is a normal attack (no skill).
        let skill = case["skill"].as_str().map(|key| ctx.skill(key));
        let element = i32::from(attribute_element(
            skill.map(|k| k.element as i8),
            att.attack_element as i8,
        ));
        assert_eq!(element, i(&case["element_used"]), "{id}: element");
        let power = skill.filter(|k| k.element != -1).map_or(0, |k| k.element_power);
        assert_eq!(
            att.attack_element_value(element) + power,
            i(&case["attack_value"]),
            "{id}: attack value"
        );
        assert_eq!(
            tgt.defence_element_value(element),
            i(&case["defence_value"]),
            "{id}: defence value"
        );
        assert_same(
            id,
            "attribute bonus",
            ctx.attribute_bonus(att, tgt, skill),
            f(&case["result"]),
        );
    }
    eprintln!("attribute: {} cases", cases(&file).len());
}

#[test]
fn resurrect() {
    let Some(file) = vector("resurrect") else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let got = calculate_skill_resurrect_restore_percent(f(&case["base"]), f(&case["wit_bonus"]));
        assert_same(id, "restore percent", got, f(&case["result"]));
    }
    eprintln!("resurrect: {} cases", cases(&file).len());
}

#[test]
fn mcrit() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("mcrit")) else {
        return;
    };
    let mut rates = 0;
    let mut calcs = 0;
    for case in cases(&file) {
        let id = s(&case["id"]);
        match s(&case["kind"]) {
            "rate" => {
                let (att, _tgt, _sk) = ctx.pair(case);
                // A skill and a target are given in every case; the class balance is 1.0.
                let got = calc_m_critical_hit(&MCriticalHitInput {
                    mcritical_rate: att.probe("mcritical_rate@1"),
                    class_balance_multiplier: att.is_player.then_some(1.0),
                    max_mcrit_rate: 200,
                });
                assert_eq!(
                    i64::from(got),
                    case["result"].as_i64().unwrap(),
                    "{id}: m critical hit"
                );
                rates += 1;
            }
            "calc" => {
                let mut rng = Scripted::from_case(case);
                let got = calc_mcrit(f(&case["rate"]), &mut rng);
                rng.finish(id);
                assert_eq!(got, b(&case["result"]), "{id}");
                calcs += 1;
            }
            other => panic!("{id}: unknown kind {other}"),
        }
    }
    eprintln!("mcrit: {} cases (rate {rates}, calc {calcs})", cases(&file).len());
    assert_eq!(rates + calcs, cases(&file).len());
}

#[test]
fn magic_success() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("magic_success")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        assert_eq!(att.level, i(&inter["attacker_level"]), "{id}");
        assert_eq!(tgt.level, i(&inter["target_level"]), "{id}");
        assert_eq!(tgt.is_attackable, b(&inter["target_is_attackable"]), "{id}");
        assert_eq!(tgt.is_raid, b(&inter["target_is_raid"]), "{id}");
        assert_same(
            id,
            "magic success res",
            tgt.probe("magic_success_res@1"),
            f(&inter["magic_success_res"]),
        );
        let mut rng = Scripted::from_case(case);
        let got = calc_magic_success(&ctx.magic_success_input(att, tgt, sk), &mut rng);
        rng.finish(id);
        assert_eq!(got, b(&case["result"]), "{id}");
    }
    eprintln!("magic_success: {} cases", cases(&file).len());
}

fn magic_dam_input<'a>(ctx: &'a Ctx, att: &Cr, tgt: &Cr, sk: &Skill, inter: &Value) -> MagicDamInput<'a> {
    MagicDamInput {
        attacker_is_playable: att.is_player,
        attacker_is_player: att.is_player,
        target_is_playable: tgt.is_player,
        target_is_player: tgt.is_player,
        target_is_attackable: tgt.is_attackable,
        m_def: f(&inter["m_def"]),
        shld_def: i(&inter["shield_def"]),
        m_atk: f(&inter["m_atk"]),
        skill_is_magic: sk.is_magic,
        skill_power: sk.power(),
        pvp_magical_def: tgt.probe("pvp_magical_def@1"),
        pvp_phys_skill_def: tgt.probe("pvp_phys_skill_def@1"),
        alt_game_magicfailures: ctx.cfg.magic_failures,
        magic_success: ctx.magic_success_input(att, tgt, sk),
        attacker_level: att.level,
        magic_crit_dmg: att.probe("magic_crit_dmg@1"),
        randomize_magical_skill_damage: ctx.cfg.randomize_magical,
        attacker_has_active_weapon: att.has_weapon,
        random_damage: att.random_damage(),
        pvp_magical_dmg: att.probe("pvp_magical_dmg@1"),
        pvp_phys_skill_dmg: att.probe("pvp_phys_skill_dmg@1"),
        attribute_bonus: ctx.attribute_bonus(att, tgt, Some(sk)),
        pve_magical_dmg: att.probe("pve_magical_dmg@1"),
        attacker_player_level: att.player_level(),
        target_is_raid: tgt.is_raid,
        target_is_raid_minion: false,
        target_level: tgt.level,
        min_npc_level_dmg_penalty: ctx.cfg.min_dmg_penalty,
        npc_skill_dmg_penalty: &ctx.cfg.skill_dmg_penalty,
        class_balance: MagicDamClassBalance::default(),
    }
}

/// The recorded `is_pvp`, `is_pve` and `skill_power` agree with what the inputs say.
fn check_pair_flags(id: &str, att: &Cr, tgt: &Cr, sk: &Skill, inter: &Value) {
    let is_pvp = att.is_player && tgt.is_player;
    let is_pve = att.is_player && tgt.is_attackable;
    assert_eq!(is_pvp, b(&inter["is_pvp"]), "{id}: is_pvp");
    assert_eq!(is_pve, b(&inter["is_pve"]), "{id}: is_pve");
    assert_same(
        id,
        "skill power",
        sk.power().get(is_pvp, is_pve),
        f(&inter["skill_power"]),
    );
}

#[test]
fn magic_dam() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("magic_dam")) else {
        return;
    };
    let mut failures = [0; 3];
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        check_pair_flags(id, att, tgt, sk, inter);
        assert_eq!(att.level, i(&inter["attacker_level"]), "{id}");
        assert_eq!(tgt.level, i(&inter["target_level"]), "{id}");
        let input = magic_dam_input(&ctx, att, tgt, sk, inter);
        let mut rng = Scripted::from_case(case);
        let got = calc_magic_dam(
            &input,
            shld(case),
            b(&case["sps"]),
            b(&case["bss"]),
            b(&case["mcrit"]),
            &mut rng,
        );
        rng.finish(id);
        assert_same(id, "magic damage", got.damage, f(&case["result"]));
        match got.failure {
            MagicFailure::None => failures[0] += 1,
            MagicFailure::Half => failures[1] += 1,
            MagicFailure::Resisted => failures[2] += 1,
        }
    }
    eprintln!(
        "magic_dam: {} cases (no failure {}, half {}, resisted {})",
        cases(&file).len(),
        failures[0],
        failures[1],
        failures[2]
    );
    // The MagicFailures paths are all covered by the vector.
    assert!(failures[1] > 0 && failures[2] > 0);
}

#[test]
fn mana_dam() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("mana_dam")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        check_pair_flags(id, att, tgt, sk, inter);
        let input = ManaDamInput {
            m_atk: f(&inter["m_atk"]),
            m_def: f(&inter["m_def"]),
            shld_def: i(&inter["shield_def"]),
            target_max_mp: i(&inter["target_max_mp"]),
            skill_power: sk.power(),
            attacker_is_playable: att.is_player,
            attacker_is_player: att.is_player,
            target_is_playable: tgt.is_player,
            target_is_attackable: tgt.is_attackable,
            general_trait_bonus: ctx.skill_trait_bonus(att, tgt, sk),
            pve_magical_dmg: att.probe("pve_magical_dmg@1"),
            attacker_player_level: att.player_level(),
            pve_magical_skill_damage: 1.0,
            target_is_raid: tgt.is_raid,
            target_is_raid_minion: false,
            target_level: tgt.level,
            min_npc_level_dmg_penalty: ctx.cfg.min_dmg_penalty,
            npc_skill_dmg_penalty: &ctx.cfg.skill_dmg_penalty,
            alt_game_magicfailures: ctx.cfg.magic_failures,
            magic_success: ctx.magic_success_input(att, tgt, sk),
        };
        let mut rng = Scripted::from_case(case);
        let got = calc_mana_dam(
            &input,
            shld(case),
            b(&case["sps"]),
            b(&case["bss"]),
            b(&case["mcrit"]),
            &mut rng,
        );
        rng.finish(id);
        assert_same(id, "mana damage", got.damage, f(&case["result"]));
    }
    eprintln!("mana_dam: {} cases", cases(&file).len());
}

#[test]
fn magic_affected() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("magic_affected")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (_att, tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        assert_eq!(sk.is_active, b(&inter["is_active"]), "{id}");
        assert_eq!(sk.has_negative_effect, b(&inter["has_negative_effect"]), "{id}");
        assert_eq!(sk.is_debuff, b(&inter["is_debuff"]), "{id}");
        let input = MagicAffectedInput {
            skill_is_active: sk.is_active,
            skill_has_negative_effect: sk.has_negative_effect,
            skill_is_debuff: sk.is_debuff,
            m_def: f(&inter["m_def"]),
            m_atk: f(&inter["m_atk"]),
            trait_bonus: f(&inter["trait_bonus"]),
            debuff_immunity: tgt.probe("debuff_immunity@0"),
        };
        let (att, tgt, sk) = ctx.pair(case);
        assert_same(
            id,
            "trait bonus",
            ctx.skill_trait_bonus(att, tgt, sk),
            input.trait_bonus,
        );
        let mut rng = Scripted::from_case(case);
        let got = calc_magic_affected(&input, &mut rng);
        rng.finish(id);
        assert_eq!(got, b(&case["result"]), "{id}");
    }
    eprintln!("magic_affected: {} cases", cases(&file).len());
}

#[test]
fn phys_dam() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("phys_dam")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        check_pair_flags(id, att, tgt, sk, inter);
        assert_same(
            id,
            "attribute bonus",
            ctx.attribute_bonus(att, tgt, Some(sk)),
            f(&inter["attribute_bonus"]),
        );
        let physical_skill_power = stat_fn(att, "PHYSICAL_SKILL_POWER", "physical_skill_power@100", 100.0);
        let input = SkillPhysDamInput {
            attacker_is_playable: att.is_player,
            target_is_playable: tgt.is_player,
            target_is_attackable: tgt.is_attackable,
            position: position(&case["placement"]),
            p_atk: f(&inter["p_atk"]),
            p_def: f(&inter["p_def"]),
            shld_def: i(&inter["shield_def"]),
            pvp_phys_skill_def: tgt.probe("pvp_phys_skill_def@1"),
            skill_power: sk.power(),
            critical_damage: att.probe("critical_damage@1"),
            critical_damage_pos: att.probe("critical_damage_pos@1"),
            defence_critical_damage: tgt.probe("defence_critical_damage@1"),
            critical_damage_add: att.probe("critical_damage_add@0"),
            defence_critical_damage_add: tgt.probe("defence_critical_damage_add@0"),
            attack_trait_bonus: f(&inter["attack_trait_bonus"]),
            randomize_physical_skill_damage: ctx.cfg.randomize_physical,
            attacker_has_active_weapon: att.has_weapon,
            random_damage: att.random_damage(),
            attacker_level: att.level,
            pvp_phys_skill_dmg: att.probe("pvp_phys_skill_dmg@1"),
            physical_skill_power: &*physical_skill_power,
            attribute_bonus: f(&inter["attribute_bonus"]),
            attacker_weapon_is_bow_or_crossbow: att.bow_or_crossbow(),
            pve_bow_skill_dmg: att.probe("pve_bow_skill_dmg@1"),
            pve_physical_dmg: att.probe("pve_physical_dmg@1"),
            target_is_raid: tgt.is_raid,
            target_is_raid_minion: false,
            target_level: tgt.level,
            attacker_player_level: att.player_level(),
            class_balance: SkillPhysDamClassBalance::default(),
            alt_game_shield_blocks: ctx.cfg.alt_shield_blocks,
            min_npc_level_dmg_penalty: ctx.cfg.min_dmg_penalty,
            npc_skill_dmg_penalty: &ctx.cfg.skill_dmg_penalty,
        };
        let mut rng = Scripted::from_case(case);
        let got = calc_skill_phys_dam(&input, shld(case), b(&case["crit"]), b(&case["ss"]), &mut rng);
        rng.finish(id);
        assert_same(id, "skill damage", got, f(&case["result"]));
    }
    eprintln!("phys_dam: {} cases", cases(&file).len());
}

#[test]
fn blow() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("blow")) else {
        return;
    };
    let mut kinds = [0; 2];
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        check_pair_flags(id, att, tgt, sk, inter);
        assert_same(
            id,
            "attribute bonus",
            ctx.attribute_bonus(att, tgt, Some(sk)),
            f(&inter["attribute_bonus"]),
        );
        assert_same(
            id,
            "general trait bonus",
            ctx.skill_trait_bonus(att, tgt, sk),
            f(&inter["general_trait_bonus"]),
        );
        let blow = s(&case["kind"]) == "blow";
        let input = BlowInput {
            attacker_is_playable: att.is_player,
            attacker_is_attackable: att.is_attackable,
            target_is_playable: tgt.is_player,
            target_is_player: tgt.is_player,
            target_is_attackable: tgt.is_attackable,
            position: position(&case["placement"]),
            p_atk: f(&inter["p_atk"]),
            p_def: f(&inter["p_def"]),
            shld_def: i(&inter["shield_def"]),
            skill_power: sk.power(),
            pvp_phys_skill_dmg: att.probe("pvp_phys_skill_dmg@1"),
            pvp_phys_skill_def: tgt.probe("pvp_phys_skill_def@1"),
            class_balance: BlowClassBalance::default(),
            critical_damage: att.probe("critical_damage@1"),
            critical_damage_pos: att.probe("critical_damage_pos@1"),
            defence_critical_damage: tgt.probe("defence_critical_damage@1"),
            critical_damage_add: att.probe("critical_damage_add@0"),
            defence_critical_damage_add: tgt.probe("defence_critical_damage_add@0"),
            weapon_trait_bonus: if blow {
                f(&inter["weapon_trait_bonus"])
            } else {
                1.0
            },
            general_trait_bonus: f(&inter["general_trait_bonus"]),
            attribute_bonus: f(&inter["attribute_bonus"]),
            randomize_physical_skill_damage: ctx.cfg.randomize_physical,
            attacker_has_active_weapon: att.has_weapon,
            random_damage: att.random_damage(),
            attacker_level: att.level,
            target_is_raid: tgt.is_raid,
            target_is_raid_minion: false,
            target_level: tgt.level,
            attacker_player_level: att.player_level(),
            min_npc_level_dmg_penalty: ctx.cfg.min_dmg_penalty,
            npc_skill_dmg_penalty: &ctx.cfg.skill_dmg_penalty,
        };
        let mut rng = Scripted::from_case(case);
        let got = if blow {
            kinds[0] += 1;
            calc_blow_damage(&input, shld(case), b(&case["ss"]), &mut rng)
        } else {
            kinds[1] += 1;
            calc_backstab_damage(&input, shld(case), b(&case["ss"]), &mut rng)
        };
        rng.finish(id);
        assert_same(id, "blow damage", got, f(&case["result"]));
    }
    eprintln!(
        "blow: {} cases (blow {}, backstab {})",
        cases(&file).len(),
        kinds[0],
        kinds[1]
    );
}

#[test]
fn skill_crit() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("skill_crit")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, tgt, sk) = ctx.pair(case);
        let defence_critical_rate = stat_fn(tgt, "DEFENCE_CRITICAL_RATE", "defence_critical_rate@100", 100.0);
        // The recorded rate is `baseCritRate * 10 * STR bonus`; the limit is what it is compared with.
        let rate = f64::from(sk.base_crit_rate * 10) * att.bonus("str");
        assert_same(id, "rate", rate, f(&case["intermediate"]["rate"]));
        assert_same(
            id,
            "limit",
            defence_critical_rate(rate) + tgt.probe("defence_critical_rate_add@0"),
            f(&case["intermediate"]["limit"]),
        );
        let input = SkillCritInput {
            base_crit_rate: sk.base_crit_rate,
            str_bonus: att.bonus("str"),
            attacker_is_playable: att.is_player,
            target_is_playable: tgt.is_player,
            target_is_attackable: tgt.is_attackable,
            pvp_physical_skill_critical_chance_multiplier: 1.0,
            pve_physical_skill_critical_chance_multiplier: 1.0,
            defence_critical_rate: &*defence_critical_rate,
            defence_critical_rate_add: tgt.probe("defence_critical_rate_add@0"),
        };
        let mut rng = Scripted::from_case(case);
        let got = calc_skill_crit(&input, &mut rng);
        rng.finish(id);
        assert_eq!(got, b(&case["result"]), "{id}");
    }
    eprintln!("skill_crit: {} cases", cases(&file).len());
}

#[test]
fn blow_success() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("blow_success")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, _tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        assert_eq!(sk.blow_chance, i(&inter["blow_chance"]), "{id}");
        assert_same(id, "dex bonus", att.bonus("dex"), f(&inter["dex_bonus"]));
        let blow_rate = stat_fn(att, "BLOW_RATE", "blow_rate@50", 50.0);
        let input = BlowSuccessInput {
            blow_chance: sk.blow_chance,
            dex_bonus: att.bonus("dex"),
            position: position(&case["placement"]),
            blow_rate: &*blow_rate,
        };
        let mut rng = Scripted::from_case(case);
        let got = calc_blow_success(&input, &mut rng);
        rng.finish(id);
        assert_eq!(got, b(&case["result"]), "{id}");
    }
    eprintln!("blow_success: {} cases", cases(&file).len());
}

#[test]
fn phys_evasion() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("phys_evasion")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (_att, tgt, sk) = ctx.pair(case);
        let evasion = tgt.probe("p_skill_evasion@0");
        assert_same(
            id,
            "p skill evasion",
            evasion,
            f(&case["intermediate"]["p_skill_evasion"]),
        );
        let mut rng = Scripted::from_case(case);
        let got = calc_physical_skill_evasion(sk.is_magic, sk.is_debuff, evasion, &mut rng);
        rng.finish(id);
        assert_eq!(got, b(&case["result"]), "{id}");
    }
    eprintln!("phys_evasion: {} cases", cases(&file).len());
}

#[test]
fn effect_success() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("effect_success")) else {
        return;
    };
    let mut drawless = 0;
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        assert_eq!(tgt.level, i(&inter["target_level"]), "{id}");
        let element_mod = ctx.attribute_bonus(att, tgt, Some(sk));
        let trait_mod = ctx.skill_trait_bonus(att, tgt, sk);
        assert_same(id, "element bonus", element_mod, f(&inter["element_bonus"]));
        assert_same(id, "trait bonus", trait_mod, f(&inter["trait_bonus"]));
        let target_base_stat = match sk.basic_property.as_str() {
            "NONE" => None,
            stat => Some(i(&inter[format!("target_{}", stat.to_lowercase())])),
        };
        let input = EffectSuccessInput {
            skill_is_debuff: sk.is_debuff,
            debuff_immunity: f(&inter["debuff_immunity"]),
            activate_rate: sk.activate_rate,
            target_base_stat,
            skill_magic_level: sk.magic_level,
            skill_lvl_bonus_rate: sk.lvl_bonus_rate,
            target_level: tgt.level,
            element_mod,
            trait_mod,
            buff_debuff_vuln: tgt.probe(if sk.is_debuff {
                "debuff_vuln@1"
            } else {
                "buff_vuln@1"
            }),
            skill_is_magic: sk.is_magic,
            attacker_m_atk: f(&inter["attacker_m_atk"]),
            attacker_blessed_spiritshot_charged: b(&case["shots"]["bss"]),
            target_m_def: f(&inter["target_m_def"]),
            min_chance: sk.min_chance,
            max_chance: sk.max_chance,
        };
        assert_same(
            id,
            "debuff immunity",
            tgt.probe("debuff_immunity@0"),
            input.debuff_immunity,
        );
        let mut rng = Scripted::from_case(case);
        let got = calc_effect_success(&input, &mut rng);
        if rng.expected.is_empty() {
            drawless += 1;
        }
        rng.finish(id);
        assert_eq!(got, b(&case["result"]), "{id}");
    }
    eprintln!(
        "effect_success: {} cases ({drawless} without a draw)",
        cases(&file).len()
    );
}

fn mastery_input(att: &Cr, sk: &Skill) -> SkillMasteryInput {
    SkillMasteryInput {
        actor_is_player: att.is_player,
        skill_is_static: sk.is_static,
        skill_mastery: att.probe("skill_mastery@0"),
        str_bonus: att.bonus("str"),
        int_bonus: att.bonus("int"),
        skill_mastery_rate: att.probe("skill_mastery_rate@1"),
        class_balance_multiplier: 1.0,
    }
}

#[test]
fn mastery() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("mastery")) else {
        return;
    };
    let mut counts = [0, 0];
    for case in cases(&file) {
        let id = s(&case["id"]);
        let inter = &case["intermediate"];
        match s(&case["kind"]) {
            "mastery" => {
                let att = ctx.cr(s(&case["actor"]));
                let sk = ctx.skill(s(&case["skill"]));
                let input = mastery_input(att, sk);
                assert_same(
                    id,
                    "skill mastery",
                    input.skill_mastery,
                    f(&inter["skill_mastery"]),
                );
                assert_same(
                    id,
                    "skill mastery rate",
                    input.skill_mastery_rate,
                    f(&inter["skill_mastery_rate"]),
                );
                assert_same(id, "str bonus", input.str_bonus, f(&inter["str_bonus"]));
                assert_same(id, "int bonus", input.int_bonus, f(&inter["int_bonus"]));
                assert_eq!(sk.is_static, b(&inter["is_static"]), "{id}");
                let mut rng = Scripted::from_case(case);
                let got = calc_skill_mastery(&input, &mut rng);
                rng.finish(id);
                assert_eq!(got, b(&case["result"]), "{id}");
                counts[0] += 1;
            }
            "abnormal_time" => {
                let (att, tgt, sk) = ctx.pair(case);
                assert_eq!(sk.abnormal_time, i(&inter["skill_abnormal_time"]), "{id}");
                let stat_mod = if sk.basic_property == "NONE" {
                    assert!(inter["stat_mod_bonus"].is_null(), "{id}");
                    1.0
                } else {
                    f(&inter["stat_mod_bonus"])
                };
                let res_mod = ctx.skill_trait_bonus(att, tgt, sk);
                let lvl_bonus_mod =
                    calc_lvl_bonus_mod(sk.magic_level, sk.lvl_bonus_rate, att.level, tgt.level);
                let element_mod = ctx.attribute_bonus(att, tgt, Some(sk));
                assert_same(id, "trait bonus", res_mod, f(&inter["trait_bonus"]));
                assert_same(id, "lvl bonus mod", lvl_bonus_mod, f(&inter["lvl_bonus_mod"]));
                assert_same(id, "attribute bonus", element_mod, f(&inter["attribute_bonus"]));
                let input = AbnormalTimeInput {
                    skill_is_passive_or_toggle: sk.is_passive || sk.is_toggle,
                    skill_abnormal_time: sk.abnormal_time,
                    // Servitor targets are not in the vectors.
                    halve_for_servitor: false,
                    mastery: mastery_input(att, sk),
                    resists: (sk.is_debuff && ctx.cfg.debuff_duration_uses_resists).then_some(
                        AbnormalTimeResists {
                            stat_mod,
                            res_mod,
                            lvl_bonus_mod,
                            element_mod,
                        },
                    ),
                };
                let mut rng = Scripted::from_case(case);
                let got = calc_effect_abnormal_time(&input, &mut rng);
                rng.finish(id);
                assert_eq!(i64::from(got), case["result"].as_i64().unwrap(), "{id}");
                counts[1] += 1;
            }
            other => panic!("{id}: unknown kind {other}"),
        }
    }
    eprintln!(
        "mastery: {} cases (mastery {}, abnormal time {})",
        cases(&file).len(),
        counts[0],
        counts[1]
    );
    assert_eq!(counts[0] + counts[1], cases(&file).len());
}

#[test]
fn skill_timing() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("skill_timing")) else {
        return;
    };
    for case in cases(&file) {
        let id = s(&case["id"]);
        let att = ctx.cr(s(&case["creature"]));
        let sk = ctx.skill(s(&case["skill"]));
        let shots = &case["shots"];
        assert_eq!(sk.hit_time + sk.cool_time, i(&case["hit_plus_cool"]), "{id}");
        let m_atk_spd = i(&case["m_atk_spd"]);
        let p_atk_spd = f(&case["p_atk_spd"]);
        assert_eq!(
            calc_atk_spd(
                sk.is_magic,
                f64::from(sk.hit_time + sk.cool_time),
                m_atk_spd,
                p_atk_spd
            ),
            i(&case["calc_atk_spd"]),
            "{id}: calcAtkSpd"
        );
        let cast = calc_skill_cast_time(&SkillCastTimeInput {
            hit_time: sk.hit_time,
            cool_time: sk.cool_time,
            skill_is_magic: sk.is_magic,
            skill_is_static: sk.is_static,
            skill_is_channeling: sk.is_channeling,
            channeling_skill_id: sk.channeling_skill_id,
            spiritshot_charged: b(&shots["sps"]) || b(&shots["bss"]),
            m_atk_spd,
            p_atk_spd,
        });
        assert_eq!(cast, i(&case["cast_time"]), "{id}: cast time");
        let reuse = calc_skill_reuse_delay(&SkillReuseInput {
            reuse_delay: sk.reuse_delay,
            static_reuse: sk.static_reuse,
            skill_is_static: sk.is_static,
            skill_is_magic: sk.is_magic,
            skill_is_physical: sk.is_physical,
            magic_reuse_rate: f(&case["magic_reuse_rate"]),
            p_reuse: f(&case["p_reuse"]),
            dance_reuse: f(&case["dance_reuse"]),
            skill_reuse_multiplier: att.is_player.then_some(1.0),
        });
        assert_eq!(reuse, i(&case["reuse_delay"]), "{id}: reuse delay");
    }
    eprintln!("skill_timing: {} cases", cases(&file).len());
}

#[test]
fn heal() {
    let (Some(ctx), Some(file)) = (Ctx::load(), vector("heal")) else {
        return;
    };
    let mut statics = 0;
    for case in cases(&file) {
        let id = s(&case["id"]);
        let (att, tgt, sk) = ctx.pair(case);
        let inter = &case["intermediate"];
        let shots = &case["shots"];
        let heal_effect = stat_fn(tgt, "HEAL_EFFECT", "heal_effect@100", 100.0);
        assert_same(
            id,
            "heal effect probe",
            heal_effect(100.0),
            f(&inter["heal_effect_probe"]),
        );
        let weapon_grade = match inter["effector_weapon_crystal"].as_str() {
            None => HealWeaponGrade::NoWeapon,
            Some("S84") => HealWeaponGrade::S84,
            Some("S80") => HealWeaponGrade::S80,
            Some(_) => HealWeaponGrade::Other,
        };
        let hp_before = f(&inter["hp_before"]);
        let input = HealInput {
            power: f(&inter["effect_power"]),
            skill_is_magic: sk.is_magic,
            skill_is_static: sk.is_static,
            skill_mp_consume: i(&inter["skill_mp_consume"]),
            skill_item_consume_count: sk.item_consume_count,
            effector_spiritshot_charged: b(&shots["sps"]),
            effector_blessed_spiritshot_charged: b(&shots["bss"]),
            effector_is_mage_player: att.is_player && b(&inter["effector_is_mage_class"]),
            effector_is_summon: false,
            effector_is_npc: !att.is_player,
            effector_weapon_grade: weapon_grade,
            effector_m_atk: f(&inter["effector_m_atk"]),
            heal_effect: &*heal_effect,
            effector_is_playable: att.is_player,
            player_healing_skill_multiplier: 1.0,
            effector_m_critical_hit: i(&inter["effector_m_critical"]),
            effected_max_recoverable_hp: i(&inter["max_recoverable_hp"]),
            effected_current_hp: hp_before,
        };
        if sk.is_static {
            statics += 1;
        }
        let mut rng = Scripted::from_case(case);
        let amount = calc_heal_amount(&input, &mut rng);
        rng.finish(id);
        // The vector is the HP after minus the HP before.
        let got = if amount != 0.0 {
            (amount + hp_before) - hp_before
        } else {
            0.0
        };
        assert_same(id, "healed amount", got, f(&case["result"]));
    }
    eprintln!("heal: {} cases ({statics} static)", cases(&file).len());
}
