#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The topic table and the range table of the config editor against every `.ini` of the repo's
//! `dist/` (G-07).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use l2_config::defaults::{Side, default_for, java_defaults};
use l2_config::ini_edit::{IniDoc, IniEntry};
use l2_config::ranges::{RANGES, enum_options, is_color_key, is_list_key, parse_color_bgr, range_for};
use l2_config::topics::{RULES, StdTopic, Topic, rule_index, topic_of};

fn dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist")
}

struct DistKey {
    side: Side,
    rel: String,
    entry: IniEntry,
    /// The Java default (typed struct, else the file's `Default:` line, else the value in `dist/`).
    default: String,
}

fn ini_files(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            ini_files(&path, &format!("{prefix}{name}/"), out);
        } else if name.ends_with(".ini") {
            out.push((format!("{prefix}{name}"), path));
        }
    }
}

/// Every key of `dist/`; empty (and a message) when the folder is not there.
fn dist_keys() -> &'static [DistKey] {
    static KEYS: OnceLock<Vec<DistKey>> = OnceLock::new();
    KEYS.get_or_init(load_dist_keys)
}

fn load_dist_keys() -> Vec<DistKey> {
    let mut keys = Vec::new();
    for (side, dir) in [(Side::Login, "login/config"), (Side::Game, "game/config")] {
        let base = dist().join(dir);
        let mut files = Vec::new();
        ini_files(&base, "", &mut files);
        files.sort();
        for (rel, path) in files {
            let text = std::fs::read_to_string(&path).unwrap();
            let defaults = java_defaults(side, &rel);
            for entry in IniDoc::parse(&text).entries() {
                let default = default_for(&defaults, &entry.key, entry.comment_default.as_deref())
                    .unwrap_or_else(|| entry.value.clone());
                keys.push(DistKey {
                    side,
                    rel: rel.clone(),
                    entry,
                    default,
                });
            }
        }
    }
    if keys.is_empty() {
        eprintln!("skipped: no dist/ folder");
    }
    keys
}

fn number(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches(['f', 'F', 'd', 'D']);
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[test]
fn every_dist_key_has_a_topic() {
    let keys = dist_keys();
    let mut missing = Vec::new();
    let mut per_topic: BTreeMap<Topic, usize> = BTreeMap::new();
    for k in keys {
        match topic_of(k.side, &k.rel, &k.entry.group, &k.entry.key) {
            Some(t) => *per_topic.entry(t).or_default() += 1,
            None => missing.push(format!("{:?} {} {}", k.side, k.rel, k.entry.key)),
        }
    }
    assert!(missing.is_empty(), "keys without a topic: {missing:#?}");
    if keys.is_empty() {
        return;
    }
    // All the fixed topics have keys, and every Custom file is its own topic.
    for t in StdTopic::ALL {
        assert!(per_topic.contains_key(&Topic::Std(t)), "no key in {}", t.label());
    }
    let customs: BTreeSet<&str> = keys
        .iter()
        .filter_map(|k| k.rel.strip_prefix("Custom/"))
        .collect();
    assert_eq!(per_topic.keys().filter(|t| t.is_custom()).count(), customs.len());
    assert!(customs.len() > 40);
    for (t, n) in &per_topic {
        eprintln!("{:>4}  {}", n, t.label());
    }
}

#[test]
fn every_rule_is_used() {
    let keys = dist_keys();
    if keys.is_empty() {
        return;
    }
    let mut used = BTreeSet::new();
    for k in keys {
        if let Some(i) = rule_index(k.side, &k.rel, &k.entry.group, &k.entry.key) {
            used.insert(i);
        }
    }
    // GeoOverride.ini is Rust-only and not part of dist/ (D-26, D-36).
    let unused: Vec<String> = RULES
        .iter()
        .enumerate()
        .filter(|(i, r)| !used.contains(i) && r.file != "GeoOverride.ini")
        .map(|(_, r)| format!("{:?} {} / group '{}' / key '{}'", r.side, r.file, r.group, r.key))
        .collect();
    assert!(unused.is_empty(), "rules no key of dist/ reaches: {unused:#?}");
}

#[test]
fn range_entries_are_valid() {
    let keys = dist_keys();
    for r in RANGES {
        assert!(r.min < r.max, "{} {}: min must be below max", r.file, r.pattern);
        assert!(
            r.step > 0.0 && r.step <= r.max - r.min,
            "{} {}: bad step",
            r.file,
            r.pattern
        );
        assert!(r.file.is_empty() || r.file.ends_with(".ini"), "{}", r.file);
    }
    if keys.is_empty() {
        return;
    }
    let mut wins: BTreeMap<usize, usize> = BTreeMap::new();
    let mut problems = Vec::new();
    for k in keys {
        if is_list_key(&k.entry.key) {
            continue;
        }
        let Some(range) = range_for(&k.rel, &k.entry.key) else {
            continue;
        };
        let Some(v) = number(&k.default) else {
            continue; // a list or a word: the editor shows a text field, no slider
        };
        let index = RANGES.iter().position(|r| std::ptr::eq(r, range)).unwrap();
        *wins.entry(index).or_default() += 1;
        if !range.contains(v) {
            problems.push(format!(
                "{} {}: default {} is outside {}..{}",
                k.rel, k.entry.key, k.default, range.min, range.max
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "defaults outside the slider range: {problems:#?}"
    );
    // Each entry must name a key that exists; one that no key reaches is a typo or is hidden by
    // an earlier entry.
    let dead: Vec<String> = RANGES
        .iter()
        .enumerate()
        .filter(|(i, _)| !wins.contains_key(i))
        .map(|(_, r)| format!("{} {}", r.file, r.pattern))
        .collect();
    assert!(
        dead.is_empty(),
        "range entries that no numeric key of dist/ uses: {dead:#?}"
    );
}

#[test]
fn the_values_in_dist_are_inside_the_ranges_too() {
    // Not a rule (a slider may park at its end), but a hint that a range is too narrow for the
    // values the owner's own dist uses.
    let mut outside = Vec::new();
    for k in dist_keys() {
        if is_list_key(&k.entry.key) {
            continue;
        }
        if let (Some(r), Some(v)) = (range_for(&k.rel, &k.entry.key), number(&k.entry.value))
            && !r.contains(v)
        {
            outside.push(format!("{} {} = {}", k.rel, k.entry.key, k.entry.value));
        }
    }
    assert!(outside.is_empty(), "{outside:#?}");
}

fn options_of(rel: &str, key: &str) -> Option<Vec<String>> {
    let k = dist_keys().iter().find(|k| k.rel == rel && k.entry.key == key)?;
    enum_options(key, &k.entry.help).map(|o| o.into_iter().map(|o| o.value).collect())
}

#[test]
fn enum_keys_of_dist_are_detected() {
    if dist_keys().is_empty() {
        return;
    }
    assert_eq!(
        options_of("GeoEngine.ini", "PathFinding").unwrap(),
        ["0", "1", "2"]
    );
    assert_eq!(
        options_of("General.ini", "PeaceZoneMode").unwrap(),
        ["0", "1", "2"]
    );
    assert_eq!(
        options_of("General.ini", "GlobalChat").unwrap(),
        ["ON", "OFF", "GM", "GLOBAL"]
    );
    assert_eq!(options_of("General.ini", "TradeChat").unwrap().len(), 4);
    assert_eq!(
        options_of("General.ini", "DefaultPunish").unwrap(),
        ["BROADCAST", "KICK", "KICKBAN", "JAIL"]
    );
    assert_eq!(
        options_of("Player.ini", "OverEnchantPunishment").unwrap(),
        ["NONE", "BROADCAST", "KICK", "KICKBAN", "JAIL"]
    );
    assert_eq!(
        options_of("Player.ini", "AltGameCancelByHit").unwrap(),
        ["bow", "cast", "all"]
    );
    assert_eq!(
        options_of("Player.ini", "PartyXpCutoffMethod").unwrap(),
        ["highfive", "auto", "level", "percentage", "none"]
    );
    assert_eq!(
        options_of("Olympiad.ini", "OlympiadPeriod").unwrap(),
        ["MONTH", "WEEK", "DAY"]
    );
    assert_eq!(
        options_of("Server.ini", "ServerListAge").unwrap(),
        ["0", "15", "18"]
    );
    assert_eq!(options_of("Server.ini", "ServerListType").unwrap().len(), 10);
    assert_eq!(
        options_of("Custom/Captcha.ini", "Punishment").unwrap(),
        ["0", "1", "2", "3"]
    );
    assert_eq!(
        options_of("Custom/Rebirth.ini", "RebirthSkillRefundMode").unwrap(),
        ["FULL", "MID", "NONE"]
    );
    assert_eq!(
        options_of("Custom/Rebirth.ini", "RebirthScreenMessagePosition")
            .unwrap()
            .len(),
        8
    );
    assert_eq!(
        options_of("FloodProtector.ini", "FloodProtectorUseItemPunishmentType").unwrap(),
        ["none", "kick", "ban", "jail"]
    );
}

#[test]
fn plain_keys_are_not_taken_for_enums() {
    let mut wrong = Vec::new();
    let known: BTreeSet<(&str, &str)> = [
        ("GeoEngine.ini", "PathFinding"),
        ("General.ini", "PeaceZoneMode"),
        ("General.ini", "GlobalChat"),
        ("General.ini", "TradeChat"),
        ("General.ini", "DefaultPunish"),
        ("Player.ini", "OverEnchantPunishment"),
        ("Player.ini", "AltGameCancelByHit"),
        ("Player.ini", "PartyXpCutoffMethod"),
        ("Olympiad.ini", "OlympiadPeriod"),
        ("Server.ini", "ServerListAge"),
        ("Server.ini", "ServerListType"),
        ("Custom/Captcha.ini", "Punishment"),
        ("Custom/Rebirth.ini", "RebirthSkillRefundMode"),
        ("Custom/Rebirth.ini", "RebirthScreenMessagePosition"),
    ]
    .into_iter()
    .collect();
    for k in dist_keys() {
        let is_flood_type = k.rel == "FloodProtector.ini" && k.entry.key.ends_with("PunishmentType");
        if enum_options(&k.entry.key, &k.entry.help).is_some()
            && !is_flood_type
            && !known.contains(&(k.rel.as_str(), k.entry.key.as_str()))
        {
            wrong.push(format!("{} {}", k.rel, k.entry.key));
        }
    }
    assert!(
        wrong.is_empty(),
        "unexpected enum-like keys (add them above if right): {wrong:#?}"
    );
}

#[test]
fn colour_keys_of_dist_hold_hex_colours() {
    let mut seen = 0;
    for k in dist_keys() {
        if is_color_key(&k.entry.key) && parse_color_bgr(&k.entry.value).is_some() {
            seen += 1;
        } else if is_color_key(&k.entry.key) {
            // The only other keys with "Color" in the name are switches.
            assert!(
                k.entry.value.eq_ignore_ascii_case("true") || k.entry.value.eq_ignore_ascii_case("false"),
                "{} {} = {}",
                k.rel,
                k.entry.key,
                k.entry.value
            );
        }
    }
    if !dist_keys().is_empty() {
        assert!(seen >= 8, "{seen}");
    }
}
