#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! `rewards::drop::calculate_drops` against the vectors the real L2JMobius `NpcTemplate.calculateDrops`
//! wrote (`java-vectors/out/drops_*.json`, M4-JV-02).
//!
//! How a case is replayed:
//! - `drops_setup.json` has the 35 NPCs (real monsters and raid bosses) with the drop lists as Java's
//!   loader left them, the items they use (only `ex_immediate_effect` matters) and the rate profiles
//!   (the `RatesConfig` values). Templates are built from that data, so CI needs no datapack.
//! - A case has the killer (level and the four bonus multipliers `PlayerStat` returned), the victim
//!   comes from the NPC (`level`, `is_raid`), and `rnd`, the draws Java made. Every draw is answered
//!   from that list (`D<u>` = `Rnd.nextDouble()`, `L<origin>,<bound>=<value>` = `Rnd.get(long, long)`)
//!   and the test checks that Rust made exactly these calls in this order, then that the drop list
//!   (item id, count) is the same.
//! - A second test runs the same cases with the templates the Rust loaders read from the reference
//!   datapack (`L2_DATAPACK_ROOT`, default `dist/game`), which also checks the
//!   post-processing of the lists (items that do not exist dropped, the sorting by chance). It
//!   skips when the datapack is missing.
//! - Files that are missing skip with a message (CI has all of them: they are committed).
//!
//! Java: entity/actor/templates/NpcTemplate.java (calculateDrops, calculateGroupDrops,
//! calculateUngroupedDrops, calculateGroupDrop, calculateUngroupedDrop)

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use l2_data::npc::DropType;
use l2_data::{ItemTemplates, NpcLoadOptions, NpcTemplates};
use l2_world::formulas::Rnd;
use l2_world::rewards::drop::{DropKiller, DropSettings, DropVictim, calculate_drops};
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
/// turned into the string `"f64:<text>"` before parsing, and [`f`] / [`f32v`] read those with
/// `str::parse`, which rounds correctly like Java's `Double.parseDouble` / `Float.parseFloat`.
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
                let _ = write!(out, "\"f64:{token}\"");
            } else {
                out.push_str(&token);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The text of a number written with a fraction (see [`exact_floats`]) or a whole number.
fn number_text(v: &Value) -> String {
    if let Some(text) = v.as_str().and_then(|t| t.strip_prefix("f64:")) {
        return text.to_string();
    }
    v.as_i64()
        .map_or_else(|| panic!("not a number: {v}"), |n| n.to_string())
}

/// A Java `double`.
fn f(v: &Value) -> f64 {
    let text = number_text(v);
    text.parse().unwrap_or_else(|e| panic!("bad number {text}: {e}"))
}

/// A Java `float` (written with `Float.toString`, the shortest text that reads back as the same float).
fn f32v(v: &Value) -> f32 {
    let text = number_text(v);
    text.parse().unwrap_or_else(|e| panic!("bad float {text}: {e}"))
}

fn i(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap_or_else(|| panic!("not an int: {v}"))).unwrap()
}

fn l(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not a long: {v}"))
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn arr(v: &Value) -> &Vec<Value> {
    v.as_array().unwrap_or_else(|| panic!("not an array: {v}"))
}

// ------------------------------------------------------------------ the scripted RNG

#[derive(Debug, Clone, PartialEq)]
enum Draw {
    /// `Rnd.nextDouble()`, the unit value it returned.
    Double(f64),
    /// `Rnd.get(long origin, long bound)`.
    Long { origin: i64, bound: i64, value: i64 },
}

fn parse_draw(text: &str) -> Draw {
    if let Some(rest) = text.strip_prefix('D') {
        return Draw::Double(rest.parse().unwrap_or_else(|e| panic!("bad draw {text}: {e}")));
    }
    if let Some(rest) = text.strip_prefix('L') {
        let (bounds, value) = rest.split_once('=').unwrap();
        let (origin, bound) = bounds.split_once(',').unwrap();
        return Draw::Long {
            origin: origin.parse().unwrap(),
            bound: bound.parse().unwrap(),
            value: value.parse().unwrap(),
        };
    }
    panic!("a draw the drop code never makes: {text}");
}

/// Answers the drop code's draws from the recorded list and remembers what it was asked.
struct Scripted {
    expected: Vec<Draw>,
    asked: Vec<Draw>,
}

impl Scripted {
    fn new(rnd: &Value) -> Self {
        Self {
            expected: arr(rnd).iter().map(|d| parse_draw(s(d))).collect(),
            asked: Vec::new(),
        }
    }

    /// What Java returned for the next call; a call the vector does not have is answered with a
    /// value that makes no sense (the final comparison reports it).
    fn next(&self) -> Option<&Draw> {
        self.expected.get(self.asked.len())
    }

    /// Rust made exactly the recorded calls, in order.
    fn finish(&self, id: &str) {
        assert_eq!(self.asked, self.expected, "{id}: the sequence of draws");
    }
}

impl Rnd for Scripted {
    fn next_double(&mut self) -> f64 {
        let value = match self.next() {
            Some(Draw::Double(u)) => *u,
            _ => 0.5,
        };
        self.asked.push(Draw::Double(value));
        value
    }

    fn get_long_range(&mut self, origin: i64, bound: i64) -> i64 {
        // `Rnd.get(long, long)` draws nothing when the range is a single number; Java records nothing.
        if origin >= bound {
            return origin;
        }
        let value = match self.next() {
            Some(Draw::Long { value, .. }) => *value,
            _ => origin,
        };
        self.asked.push(Draw::Long { origin, bound, value });
        value
    }

    fn next_boolean(&mut self) -> bool {
        panic!("next_boolean is not used by drops")
    }
    fn next_bytes(&mut self, _bytes: &mut [u8]) {
        panic!("next_bytes is not used by drops")
    }
    fn get_int(&mut self, _bound: i32) -> i32 {
        panic!("get_int is not used by drops")
    }
    fn get_int_range(&mut self, _origin: i32, _bound: i32) -> i32 {
        panic!("get_int_range is not used by drops")
    }
    fn next_int(&mut self) -> i32 {
        panic!("next_int is not used by drops")
    }
    fn get_long(&mut self, _bound: i64) -> i64 {
        panic!("get_long is not used by drops")
    }
    fn next_long(&mut self) -> i64 {
        panic!("next_long is not used by drops")
    }
    fn next_float(&mut self) -> f32 {
        panic!("next_float is not used by drops")
    }
    fn get_double(&mut self, _bound: f64) -> f64 {
        panic!("get_double is not used by drops")
    }
    fn get_double_range(&mut self, _origin: f64, _bound: f64) -> f64 {
        panic!("get_double_range is not used by drops")
    }
    fn next_gaussian(&mut self) -> f64 {
        panic!("next_gaussian is not used by drops")
    }
}

// ------------------------------------------------------------------ the setup file

struct Npc {
    level: i32,
    raid: bool,
}

struct Setup {
    npcs: HashMap<i32, Npc>,
    /// Rate profiles by name.
    settings: HashMap<String, DropSettings>,
    /// The NPC and item tables built from the vector data.
    npc_templates: NpcTemplates,
    items: ItemTemplates,
}

fn item_xml(item: &Value) -> String {
    let id = i(&item["id"]);
    let herb = item["ex_immediate_effect"].as_bool().unwrap();
    format!(
        r#"<item id="{id}" type="EtcItem" name="I{id}"><set name="ex_immediate_effect" val="{herb}" /></item>"#
    )
}

/// `<npc>` with the lists exactly as Java had them after loading.
fn npc_xml(npc: &Value) -> String {
    let id = i(&npc["id"]);
    let ty = s(&npc["type"]);
    let level = i(&npc["level"]);
    let entry = |e: &Value| {
        let e = arr(e);
        // The chance is a Java double: `Display` for f64 writes the shortest text that reads back as the same double.
        format!(
            r#"<item id="{}" min="{}" max="{}" chance="{}" />"#,
            e[0],
            e[1],
            e[2],
            f(&e[3])
        )
    };
    let mut drop = String::new();
    for group in arr(&npc["drop_groups"]) {
        let items: String = arr(&group["items"]).iter().map(entry).collect();
        let _ = write!(drop, r#"<group chance="{}">{items}</group>"#, f(&group["chance"]));
    }
    for e in arr(&npc["drops"]) {
        drop.push_str(&entry(e));
    }
    format!(
        r#"<npc id="{id}" type="{ty}" level="{level}" name="N{id}"><dropLists><drop>{drop}</drop></dropLists></npc>"#
    )
}

fn settings_of(raw: &Value) -> DropSettings {
    let by_id = |list: &Value| {
        arr(list)
            .iter()
            .map(|pair| (i(&arr(pair)[0]), f32v(&arr(pair)[1])))
            .collect()
    };
    DropSettings {
        rate_death_drop_amount_multiplier: f32v(&raw["RATE_DEATH_DROP_AMOUNT_MULTIPLIER"]),
        rate_spoil_drop_amount_multiplier: f32v(&raw["RATE_SPOIL_DROP_AMOUNT_MULTIPLIER"]),
        rate_herb_drop_amount_multiplier: f32v(&raw["RATE_HERB_DROP_AMOUNT_MULTIPLIER"]),
        rate_raid_drop_amount_multiplier: f32v(&raw["RATE_RAID_DROP_AMOUNT_MULTIPLIER"]),
        rate_death_drop_chance_multiplier: f32v(&raw["RATE_DEATH_DROP_CHANCE_MULTIPLIER"]),
        rate_spoil_drop_chance_multiplier: f32v(&raw["RATE_SPOIL_DROP_CHANCE_MULTIPLIER"]),
        rate_herb_drop_chance_multiplier: f32v(&raw["RATE_HERB_DROP_CHANCE_MULTIPLIER"]),
        rate_raid_drop_chance_multiplier: f32v(&raw["RATE_RAID_DROP_CHANCE_MULTIPLIER"]),
        rate_drop_amount_by_id: by_id(&raw["RATE_DROP_AMOUNT_BY_ID"]),
        rate_drop_chance_by_id: by_id(&raw["RATE_DROP_CHANCE_BY_ID"]),
        drop_max_occurrences_normal: i(&raw["DROP_MAX_OCCURRENCES_NORMAL"]),
        drop_max_occurrences_raidboss: i(&raw["DROP_MAX_OCCURRENCES_RAIDBOSS"]),
        drop_adena_min_level_difference: i(&raw["DROP_ADENA_MIN_LEVEL_DIFFERENCE"]),
        drop_adena_max_level_difference: i(&raw["DROP_ADENA_MAX_LEVEL_DIFFERENCE"]),
        drop_adena_min_level_gap_chance: f(&raw["DROP_ADENA_MIN_LEVEL_GAP_CHANCE"]),
        drop_item_min_level_difference: i(&raw["DROP_ITEM_MIN_LEVEL_DIFFERENCE"]),
        drop_item_max_level_difference: i(&raw["DROP_ITEM_MAX_LEVEL_DIFFERENCE"]),
        drop_item_min_level_gap_chance: f(&raw["DROP_ITEM_MIN_LEVEL_GAP_CHANCE"]),
        ..DropSettings::default()
    }
}

impl Setup {
    fn load() -> Option<Self> {
        let file = vector("drops_setup")?;
        let items_xml: String = arr(&file["items"]).iter().map(item_xml).collect();
        let npcs_xml: String = arr(&file["npcs"]).iter().map(npc_xml).collect();
        let items = ItemTemplates::from_xml(&format!("<list>{items_xml}</list>")).unwrap();
        let npc_templates =
            NpcTemplates::from_xml(&format!("<list>{npcs_xml}</list>"), &NpcLoadOptions::default()).unwrap();
        let npcs = arr(&file["npcs"])
            .iter()
            .map(|n| {
                (
                    i(&n["id"]),
                    Npc {
                        level: i(&n["level"]),
                        raid: n["is_raid"].as_bool().unwrap(),
                    },
                )
            })
            .collect();
        let settings = arr(&file["profiles"])
            .iter()
            .map(|p| (s(&p["name"]).to_string(), settings_of(&p["settings"])))
            .collect();
        Some(Self {
            npcs,
            settings,
            npc_templates,
            items,
        })
    }
}

// ------------------------------------------------------------------ replaying the cases

/// Replays every case of `file`; returns how many it ran.
fn replay(setup: &Setup, npc_templates: &NpcTemplates, items: &ItemTemplates, file: &str) -> Option<usize> {
    let vectors = vector(file)?;
    let cases = arr(&vectors["cases"]);
    for case in cases {
        let id = s(&case["id"]);
        let npc_id = i(&case["npc"]);
        let npc = &setup.npcs[&npc_id];
        let settings = &setup.settings[s(&case["profile"])];
        let killer = &case["killer"];
        let drop_killer = DropKiller {
            level: i(&killer["level"]),
            bonus_drop_rate: f(&killer["bonus_drop_rate"]),
            bonus_drop_amount: f(&killer["bonus_drop_amount"]),
            bonus_drop_adena: f(&killer["bonus_drop_adena"]),
            bonus_spoil_rate: f(&killer["bonus_spoil_rate"]),
        };
        assert_eq!(
            drop_killer.level - npc.level,
            i(&case["level_gap"]),
            "{id}: killer level minus victim level"
        );
        let victim = DropVictim {
            level: npc.level,
            raid: npc.raid,
            champion: false,
            seeded: false,
        };
        let template = npc_templates
            .get(npc_id)
            .unwrap_or_else(|| panic!("{id}: NPC {npc_id} is not in the template table"));

        let mut rng = Scripted::new(&case["rnd"]);
        let drops = calculate_drops(
            settings,
            items,
            template,
            DropType::Drop,
            &victim,
            &drop_killer,
            &mut rng,
        );
        rng.finish(id);

        let got: Vec<(i32, i64)> = drops.iter().map(|h| (h.id, h.count)).collect();
        let wanted: Vec<(i32, i64)> = arr(&case["result"])
            .iter()
            .map(|pair| (i(&arr(pair)[0]), l(&arr(pair)[1])))
            .collect();
        assert_eq!(got, wanted, "{id}: the drop list");
    }
    Some(cases.len())
}

const FILES: [(&str, usize); 3] = [
    ("drops_monsters", 400),
    ("drops_raids", 200),
    ("drops_rates", 600),
];

#[test]
fn every_vector_case_matches_java() {
    let Some(setup) = Setup::load() else { return };
    for (file, at_least) in FILES {
        let Some(count) = replay(&setup, &setup.npc_templates, &setup.items, file) else {
            continue;
        };
        // A file that lost its cases must not pass silently.
        assert!(count >= at_least, "{file} has only {count} cases");
        eprintln!("{file}: {count} cases match");
    }
}

/// The reference datapack, when it is on this machine.
fn datapack_root() -> Option<PathBuf> {
    let root = std::env::var_os("L2_DATAPACK_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
    root.join("data/stats/npcs").is_dir().then_some(root)
}

/// The same cases with the NPC and item tables of the Rust loaders: Java's post-processing of the
/// lists (`NpcData.parseDocument`) is part of what is checked.
#[test]
fn the_cases_match_with_the_templates_of_the_datapack_loaders() {
    let Some(setup) = Setup::load() else { return };
    let Some(root) = datapack_root() else {
        eprintln!("skipped: the reference datapack is missing");
        return;
    };
    let items = ItemTemplates::load(&root, false);
    let npcs = NpcTemplates::load(&root, &NpcLoadOptions::default());
    for (file, _) in FILES {
        if let Some(count) = replay(&setup, &npcs, &items, file) {
            eprintln!("{file}: {count} cases match with the loaded datapack");
        }
    }
}
