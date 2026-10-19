//! Shape and consistency checks for the committed `java-vectors/out/skill_formulas_*.json` files
//! (M5-JV-03).
//!
//! These tests prove that the files parse, that the case lists have the promised coverage, and that
//! the numbers inside a file agree with each other where the Java source gives a closed form (the
//! `Rnd` draw mapping, the attribute bonus, the resurrection percent, the cast time of a skill, ...).
//! The skill formulas of M5-FORM-02 check the actual values against these files.
//!
//! A test skips with a message when its file is missing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeSet, HashMap};

use l2_testkit::vectors::{load, vector_path};
use serde_json::Value;

/// Loads a vector file or returns `None` (and says so) when it is missing.
fn load_or_skip(name: &str) -> Option<Value> {
    let file = format!("skill_formulas_{name}");
    if !vector_path(&file).exists() {
        eprintln!(
            "skipping: java-vectors/out/{file}.json is missing (run java-vectors/build.ps1 skill-formulas)"
        );
        return None;
    }
    Some(load(&file))
}

fn cases(f: &Value) -> &Vec<Value> {
    f["cases"].as_array().expect("cases is an array")
}

fn s(v: &Value) -> &str {
    v.as_str().expect("string")
}

fn n(v: &Value) -> f64 {
    v.as_f64().expect("number")
}

/// The common header of every file and unique case ids.
fn check_header(f: &Value, expected_cases: usize) {
    assert_eq!(s(&f["generated_by"]), "java-vectors");
    assert!(!s(&f["source"]).is_empty() && !s(&f["note"]).is_empty());
    let cs = cases(f);
    assert_eq!(cs.len(), expected_cases, "case count");
    let ids: BTreeSet<&str> = cs.iter().map(|c| s(&c["id"])).collect();
    assert_eq!(ids.len(), cs.len(), "case ids must be unique");
}

/// `script` and `rnd` of one case agree: one script value per recorded call, and every returned
/// value is what the harness `Rnd` makes of the script value (see the README).
fn check_draws(c: &Value) {
    let id = s(&c["id"]);
    let script = c["script"].as_array().expect("script");
    let rnd = c["rnd"].as_array().expect("rnd");
    assert_eq!(script.len(), rnd.len(), "{id}: one script value per draw");
    for (u, call) in script.iter().zip(rnd) {
        let u = n(u);
        assert!((0.0..1.0).contains(&u), "{id}: script value {u}");
        let bound = s(&call["bound"]);
        let returned = s(&call["returned"]);
        match s(&call["method"]) {
            "get_int" => {
                let b: f64 = bound.parse().unwrap();
                assert_eq!(returned.parse::<i64>().unwrap(), (u * b) as i64, "{id}");
            }
            "get_int_range" => {
                let (o, b) = bound.split_once(',').unwrap();
                let (o, b): (i64, i64) = (o.parse().unwrap(), b.parse().unwrap());
                let expected = o + (u * ((b - o) as f64 + 1.0)) as i64;
                assert_eq!(returned.parse::<i64>().unwrap(), expected, "{id}");
            }
            "nextGaussian" => {
                let r: f64 = returned.parse().unwrap();
                assert!((r - ((u * 2.0) - 1.0)).abs() < 1e-12, "{id}: gaussian");
            }
            other => panic!("{id}: unexpected Rnd method {other}"),
        }
    }
}

fn check_all_draws(f: &Value) {
    for c in cases(f) {
        check_draws(c);
    }
}

/// Names of the creatures and keys of the skills a file may refer to.
struct Refs {
    creatures: BTreeSet<String>,
    skills: BTreeSet<String>,
}

fn refs() -> Option<Refs> {
    let creatures = load_or_skip("creatures")?;
    let skills = load_or_skip("skills")?;
    Some(Refs {
        creatures: creatures["creatures"]
            .as_array()?
            .iter()
            .map(|c| s(&c["name"]).to_owned())
            .collect(),
        skills: skills["skills"]
            .as_array()?
            .iter()
            .map(|c| s(&c["key"]).to_owned())
            .collect(),
    })
}

/// Every attacker / target / actor / creature / caster / skill of the cases is in the shared files.
fn check_refs(f: &Value, r: &Refs) {
    for c in cases(f) {
        for key in ["attacker", "target", "actor", "creature", "caster"] {
            if let Some(name) = c.get(key).and_then(Value::as_str) {
                assert!(
                    r.creatures.contains(name),
                    "{}: unknown creature {name}",
                    s(&c["id"])
                );
            }
        }
        if let Some(skill) = c.get("skill").and_then(Value::as_str) {
            assert!(r.skills.contains(skill), "{}: unknown skill {skill}", s(&c["id"]));
        }
    }
}

fn is_finite_non_negative(v: &Value) -> bool {
    v.as_f64().is_some_and(|x| x.is_finite() && x >= 0.0)
}

// ---------------------------------------------------------------------------------------------
// creatures and skills
// ---------------------------------------------------------------------------------------------

#[test]
fn creatures_shape() {
    let Some(f) = load_or_skip("creatures") else {
        return;
    };
    assert_eq!(s(&f["generated_by"]), "java-vectors");
    let creatures = f["creatures"].as_array().unwrap();
    assert_eq!(creatures.len(), 32);
    let names: BTreeSet<&str> = creatures.iter().map(|c| s(&c["name"])).collect();
    assert_eq!(names.len(), creatures.len());
    // The 19 base creatures (17 of formulas_creatures.json + dagger user and healer) and 13 variants.
    let variants = creatures
        .iter()
        .filter(|c| !c["mods"].as_array().unwrap().is_empty())
        .count();
    assert_eq!(variants, 13);
    for c in creatures {
        let name = s(&c["name"]);
        let base = s(&c["base"]);
        assert!(names.contains(base), "{name}: base {base}");
        assert!(
            c["expected"]["user_info"]["max_hp"].as_i64().unwrap() > 0,
            "{name}"
        );
        assert!(
            c["traits"]["attack"].is_object() && c["traits"]["defence"].is_object(),
            "{name}"
        );
        assert!(c["traits"]["invul"].is_array(), "{name}");
        assert_eq!(
            c["elements"]["attack_power"].as_array().unwrap().len(),
            6,
            "{name}"
        );
        assert_eq!(
            c["elements"]["defence_value"].as_array().unwrap().len(),
            6,
            "{name}"
        );
        assert!(c["skill_probes"].as_object().unwrap().len() >= 30, "{name}");
        assert!(c["flags"]["max_recoverable_hp"].as_i64().unwrap() > 0, "{name}");
        // A variant has the same level / class as its base.
        let base_c = creatures.iter().find(|b| s(&b["name"]) == base).unwrap();
        assert_eq!(c["input"]["level"], base_c["input"]["level"], "{name}");
    }

    let cfg = &f["config"];
    assert_eq!(cfg["MagicFailures"], Value::Bool(true));
    assert_eq!(cfg["ClassBalance_arrays_not_all_1"].as_array().unwrap().len(), 0);
    assert!(!cfg["NpcSkillChancePenalty"].as_array().unwrap().is_empty());
    for needed in [
        "archmage76_elem",
        "npc_20677_res",
        "npc_21090_invul",
        "adventurer76_blow",
        "cardinal76",
    ] {
        assert!(names.contains(needed), "{needed}");
    }
    // Traits and invulnerabilities were applied.
    let res = creatures
        .iter()
        .find(|c| s(&c["name"]) == "npc_20677_res")
        .unwrap();
    assert!(res["traits"]["defence"]["SLEEP"].is_array());
    let inv = creatures
        .iter()
        .find(|c| s(&c["name"]) == "npc_21090_invul")
        .unwrap();
    assert_eq!(inv["traits"]["invul"].as_array().unwrap().len(), 3);
}

#[test]
fn skills_shape() {
    let Some(f) = load_or_skip("skills") else {
        return;
    };
    assert_eq!(s(&f["generated_by"]), "java-vectors");
    let skills = f["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 41);
    let by_key: HashMap<&str, &Value> = skills.iter().map(|k| (s(&k["key"]), k)).collect();
    assert_eq!(by_key.len(), skills.len());
    for sk in skills {
        let key = s(&sk["key"]);
        assert_eq!(key, format!("{}:{}", sk["id"], sk["level"]));
        assert!(sk["effects"].is_array(), "{key}");
    }

    // The kinds of skill the card asks for.
    let nuke = by_key["1177:5"];
    assert_eq!(nuke["effects"][0], "MagicalDamage");
    assert_eq!(nuke["element"], 2);
    assert_eq!(by_key["1011:18"]["effects"][0], "Heal");
    assert_eq!(by_key["16:24"]["effects"][0], "FatalBlow");
    assert_eq!(by_key["30:37"]["effects"][0], "Backstab");
    assert_eq!(by_key["1069:42"]["is_debuff"], true);
    assert!(n(&by_key["1068:3"]["abnormal_time"]) > 0.0);
    assert_eq!(by_key["1108:1"]["magic_level"], 0, "a debuff with magic level 0");
    assert_eq!(by_key["1077:1"]["magic_level"], 0, "a buff with magic level 0");
    assert_eq!(by_key["2001:1"]["is_static"], true);
}

// ---------------------------------------------------------------------------------------------
// damage
// ---------------------------------------------------------------------------------------------

#[test]
fn magic_dam_shape() {
    let Some(f) = load_or_skip("magic_dam") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 1368);
    check_all_draws(&f);
    check_refs(&f, &r);
    let mut perfect_blocks = 0;
    let mut halved_or_resisted = 0;
    for c in cases(&f) {
        let id = s(&c["id"]);
        assert!(is_finite_non_negative(&c["result"]), "{id}");
        assert!(c["intermediate"]["m_critical"].is_number());
        if c["shld"] == 2 {
            perfect_blocks += 1;
            assert_eq!(n(&c["result"]), 1.0, "{id}: perfect block");
            assert!(c["rnd"].as_array().unwrap().is_empty(), "{id}: no draw");
        }
        if c["rnd"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| s(&d["method"]) == "get_int" && s(&d["bound"]) == "100" && s(&d["returned"]) == "99")
        {
            halved_or_resisted += 1;
        }
    }
    assert!(perfect_blocks > 100);
    assert!(halved_or_resisted > 100, "failure paths are covered");
}

#[test]
fn mcrit_shape() {
    let Some(f) = load_or_skip("mcrit") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 302);
    check_refs(&f, &r);
    let mut calc = 0;
    for c in cases(&f) {
        if s(&c["kind"]) == "calc" {
            calc += 1;
            check_draws(c);
            let rate = n(&c["rate"]) as i64;
            let drawn: i64 = s(&c["rnd"][0]["returned"]).parse().unwrap();
            assert_eq!(drawn, n(&c["draw"]) as i64);
            // Formulas.calcMCrit: rate > Rnd.get(1000).
            assert_eq!(c["result"].as_bool().unwrap(), rate > drawn, "{}", s(&c["id"]));
        } else {
            assert_eq!(s(&c["kind"]), "rate");
            assert!(c["result"].as_i64().unwrap() >= 0);
        }
    }
    assert_eq!(calc, 110);
}

#[test]
fn mana_dam_shape() {
    let Some(f) = load_or_skip("mana_dam") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 80);
    check_all_draws(&f);
    check_refs(&f, &r);
    for c in cases(&f) {
        assert!(is_finite_non_negative(&c["result"]), "{}", s(&c["id"]));
        assert!(c["intermediate"]["target_max_mp"].as_i64().unwrap() > 0);
        if c["shld"] == 2 {
            assert_eq!(n(&c["result"]), 1.0);
        }
    }
}

#[test]
fn phys_dam_shape() {
    let Some(f) = load_or_skip("phys_dam") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 742);
    check_all_draws(&f);
    check_refs(&f, &r);
    assert!(f["geometry"]["attacker_xy_by_relation"]["back"].is_string());
    let mut relations = BTreeSet::new();
    let mut two_draws = 0;
    for c in cases(&f) {
        let id = s(&c["id"]);
        assert!(is_finite_non_negative(&c["result"]), "{id}");
        relations.insert(s(&c["placement"]["relation"]).to_owned());
        if c["shld"] == 2 {
            assert_eq!(n(&c["result"]), 1.0, "{id}");
            assert!(c["rnd"].as_array().unwrap().is_empty(), "{id}");
        } else {
            // RandomizePhysicalSkillDamage: the weapon random multiplier draws twice (Java quirk:
            // once for the config, once unconditionally). A creature whose weapon has no random
            // range (fists, RANDOM_DAMAGE 0) draws nothing: Rnd.get(0, 0).
            let rnd = c["rnd"].as_array().unwrap();
            assert!(rnd.len() == 2 || rnd.is_empty(), "{id}");
            two_draws += usize::from(rnd.len() == 2);
        }
    }
    assert!(two_draws > 400);
    assert_eq!(relations.len(), 3);
}

#[test]
fn blow_shape() {
    let Some(f) = load_or_skip("blow") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 640);
    check_all_draws(&f);
    check_refs(&f, &r);
    let (mut blow, mut backstab) = (0, 0);
    let mut one_draw = 0;
    for c in cases(&f) {
        let id = s(&c["id"]);
        match s(&c["kind"]) {
            "blow" => blow += 1,
            "backstab" => backstab += 1,
            other => panic!("{id}: kind {other}"),
        }
        // Both functions return at least 1 (1 for a perfect block).
        assert!(n(&c["result"]) >= 1.0, "{id}");
        if c["shld"] == 2 {
            assert_eq!(n(&c["result"]), 1.0, "{id}");
        } else {
            let rnd = c["rnd"].as_array().unwrap();
            assert!(rnd.len() <= 1, "{id}: at most one random multiplier");
            one_draw += usize::from(rnd.len() == 1);
        }
    }
    assert!(one_draw > 200);
    assert_eq!((blow, backstab), (320, 320));
}

#[test]
fn skill_crit_shape() {
    let Some(f) = load_or_skip("skill_crit") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 231);
    check_all_draws(&f);
    check_refs(&f, &r);
    for c in cases(&f) {
        let id = s(&c["id"]);
        let limit = n(&c["intermediate"]["limit"]);
        let drawn: i64 = s(&c["rnd"][0]["returned"]).parse().unwrap();
        // calcCrit: (defence rate) > Rnd.get(1000).
        assert_eq!(c["result"].as_bool().unwrap(), limit > drawn as f64, "{id}");
    }
}

// ---------------------------------------------------------------------------------------------
// chances
// ---------------------------------------------------------------------------------------------

/// The cases that compare `Rnd.get(100)` with a rate: the recorded draw is the `draw` field (or the
/// call drew nothing: `draw == -1`), and the result never flips back to true as the draw grows.
fn check_hundred_draw_cases(f: &Value) {
    let mut by_group: HashMap<String, Vec<(i64, bool)>> = HashMap::new();
    for c in cases(f) {
        let id = s(&c["id"]);
        let draw = c["draw"].as_i64().unwrap();
        let rnd = c["rnd"].as_array().unwrap();
        if draw == -1 {
            assert!(rnd.is_empty(), "{id}");
        } else {
            assert_eq!(rnd.len(), 1, "{id}");
            assert_eq!(s(&rnd[0]["method"]), "get_int");
            assert_eq!(s(&rnd[0]["bound"]), "100");
            assert_eq!(s(&rnd[0]["returned"]).parse::<i64>().unwrap(), draw, "{id}");
            let group = id.rsplit_once("_d").unwrap().0.to_owned();
            by_group
                .entry(group)
                .or_default()
                .push((draw, c["result"].as_bool().unwrap()));
        }
    }
    for (group, mut draws) in by_group {
        draws.sort();
        // The result is true while the draw is below the rate.
        let mut seen_false = false;
        for (d, result) in draws {
            if !result {
                seen_false = true;
            } else {
                assert!(!seen_false, "{group}: true after false at draw {d}");
            }
        }
    }
}

#[test]
fn blow_success_shape() {
    let Some(f) = load_or_skip("blow_success") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 367);
    check_all_draws(&f);
    check_refs(&f, &r);
    check_hundred_draw_cases(&f);
}

#[test]
fn phys_evasion_shape() {
    let Some(f) = load_or_skip("phys_evasion") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 66);
    check_all_draws(&f);
    check_refs(&f, &r);
    check_hundred_draw_cases(&f);
    // Magic skills and debuffs never evade and draw nothing.
    for c in cases(&f) {
        if matches!(s(&c["skill"]), "1177:5" | "223:49") {
            assert_eq!(c["draw"], -1);
            assert_eq!(c["result"], false);
        }
    }
}

#[test]
fn effect_success_shape() {
    let Some(f) = load_or_skip("effect_success") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 1621);
    check_all_draws(&f);
    check_refs(&f, &r);
    check_hundred_draw_cases(&f);
    let (mut yes, mut no, mut bss) = (0, 0, 0);
    for c in cases(&f) {
        if c["result"].as_bool().unwrap() {
            yes += 1;
        } else {
            no += 1;
        }
        if c["shots"]["bss"] == true {
            bss += 1;
        }
    }
    assert!(yes > 300 && no > 300 && bss > 100, "{yes} {no} {bss}");
}

#[test]
fn magic_success_shape() {
    let Some(f) = load_or_skip("magic_success") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 438);
    check_all_draws(&f);
    check_refs(&f, &r);
    check_hundred_draw_cases(&f);
}

#[test]
fn magic_affected_shape() {
    let Some(f) = load_or_skip("magic_affected") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 374);
    check_all_draws(&f);
    check_refs(&f, &r);
    // The result never goes from true to false as the gaussian grows.
    let mut groups: HashMap<String, Vec<(f64, bool)>> = HashMap::new();
    for c in cases(&f) {
        let id = s(&c["id"]);
        let (group, u) = id.rsplit_once("_u").unwrap();
        let rnd = c["rnd"].as_array().unwrap();
        assert!(rnd.len() <= 1, "{id}");
        groups
            .entry(group.to_owned())
            .or_default()
            .push((u.parse().unwrap(), c["result"].as_bool().unwrap()));
    }
    for (group, mut draws) in groups {
        draws.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut seen_true = false;
        for (u, result) in draws {
            assert!(result || !seen_true, "{group}: false after true at u={u}");
            seen_true |= result;
        }
    }
}

#[test]
fn mastery_shape() {
    let Some(f) = load_or_skip("mastery") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 299);
    check_all_draws(&f);
    check_refs(&f, &r);
    let mut times = 0;
    for c in cases(&f) {
        let id = s(&c["id"]);
        match s(&c["kind"]) {
            "mastery" => {
                let draw = c["draw"].as_i64().unwrap();
                if draw == -1 {
                    assert_eq!(c["result"], false, "{id}: nothing to draw means no mastery");
                } else {
                    assert_eq!(s(&c["rnd"][0]["returned"]).parse::<i64>().unwrap(), draw);
                }
            }
            "abnormal_time" => {
                times += 1;
                assert!(c["result"].as_i64().is_some(), "{id}");
            }
            other => panic!("{id}: kind {other}"),
        }
    }
    assert_eq!(times, 9 * 13 * 2);
}

// ---------------------------------------------------------------------------------------------
// numbers without draws
// ---------------------------------------------------------------------------------------------

#[test]
fn skill_timing_shape() {
    let Some(f) = load_or_skip("skill_timing") else {
        return;
    };
    let Some(skills) = load_or_skip("skills") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 210);
    check_refs(&f, &r);
    let magic: HashMap<&str, bool> = skills["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| (s(&k["key"]), k["is_magic"].as_bool().unwrap()))
        .collect();
    for c in cases(&f) {
        let id = s(&c["id"]);
        let base = n(&c["hit_plus_cool"]);
        // Formulas.calcAtkSpd: (int)((time / speed) * 333 | 300).
        let expected = if magic[s(&c["skill"])] {
            ((base / n(&c["m_atk_spd"])) * 333.0) as i64
        } else {
            ((base / n(&c["p_atk_spd"])) * 300.0) as i64
        };
        assert_eq!(c["calc_atk_spd"].as_i64().unwrap(), expected, "{id}");
        assert!(
            c["cast_time"].as_i64().unwrap() >= 0 && c["reuse_delay"].as_i64().unwrap() >= 0,
            "{id}"
        );
    }
}

#[test]
fn resurrect_shape() {
    let Some(f) = load_or_skip("resurrect") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 448);
    check_refs(&f, &r);
    for c in cases(&f) {
        let id = s(&c["id"]);
        let base = n(&c["base"]);
        let result = n(&c["result"]);
        if base == 0.0 || base == 100.0 {
            assert_eq!(result, base, "{id}");
            continue;
        }
        // calculateSkillResurrectRestorePercent.
        let mut percent = base * n(&c["wit_bonus"]);
        if percent - base > 20.0 {
            percent += 20.0;
        }
        let expected = percent.max(base).min(90.0);
        assert!((result - expected).abs() < 1e-9, "{id}: {result} vs {expected}");
    }
}

#[test]
fn heal_shape() {
    let Some(f) = load_or_skip("heal") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 464);
    check_all_draws(&f);
    check_refs(&f, &r);
    let mut healed = 0;
    for c in cases(&f) {
        let id = s(&c["id"]);
        let i = &c["intermediate"];
        let result = n(&c["result"]);
        // Never overheals, never negative.
        assert!(result >= 0.0, "{id}");
        assert!(
            result <= n(&i["max_recoverable_hp"]) - n(&i["hp_before"]) + 1e-9,
            "{id}"
        );
        if result > 0.0 {
            healed += 1;
        }
        assert!(n(&i["effect_power"]) > 0.0, "{id}");
    }
    assert!(healed > 150);
}

#[test]
fn traits_shape() {
    let Some(f) = load_or_skip("traits") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 602);
    check_refs(&f, &r);
    let mut general = 0;
    for c in cases(&f) {
        let id = s(&c["id"]);
        let result = n(&c["result"]);
        match s(&c["kind"]) {
            "general" => {
                general += 1;
                if s(&c["trait"]) == "NONE" {
                    assert_eq!(result, 1.0, "{id}");
                }
                // 0 (invulnerable), 1 (no effect) or a clamped value.
                assert!(
                    result == 0.0 || result == 1.0 || (0.05..=2.0).contains(&result),
                    "{id}"
                );
                if c["trait_type"] == 1 || c["trait_type"] == 0 {
                    assert!(
                        result == 1.0 || result == 0.0,
                        "{id}: weapon traits have no general bonus"
                    );
                }
            }
            "weapon" | "attack" => assert!(result >= 0.0, "{id}"),
            other => panic!("{id}: kind {other}"),
        }
    }
    assert_eq!(general, 7 * 42 * 2);
}

#[test]
fn attribute_shape() {
    let Some(f) = load_or_skip("attribute") else {
        return;
    };
    let Some(r) = refs() else { return };
    check_header(&f, 209);
    check_refs(&f, &r);
    let (mut up, mut down, mut even) = (0, 0, 0);
    for c in cases(&f) {
        let id = s(&c["id"]);
        let diff = n(&c["attack_value"]) - n(&c["defence_value"]);
        // Formulas.calcAttributeBonus.
        let expected = if diff > 0.0 {
            up += 1;
            (1.025 + ((diff.powi(3) / 2.0).sqrt() * 0.0001)).min(1.25)
        } else if diff < 0.0 {
            down += 1;
            (0.975 - (((-diff).powi(3) / 2.0).sqrt() * 0.0001)).max(0.75)
        } else {
            even += 1;
            1.0
        };
        assert!((n(&c["result"]) - expected).abs() < 1e-12, "{id}");
    }
    assert!(up > 20 && down > 20 && even > 5, "{up} {down} {even}");
}
