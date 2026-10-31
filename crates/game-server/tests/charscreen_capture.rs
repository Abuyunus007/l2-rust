#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The character screen against recorded Java game server sessions.
//!
//! Needs two things that CI does not have, and skips with a printed note without them:
//! `L2_CAPTURE_DIR` (a folder with `*.jsonl` captures, format: `l2-proxy` capture.rs) and the
//! Java datapack (`L2_DATAPACK_ROOT`, default `dist/game`).
//!
//! What is checked:
//! * the answer order after each character screen packet,
//! * `NEW_CHARACTER_SUCCESS`, `CHARACTER_CREATE_SUCCESS`, `CHARACTER_DELETE_SUCCESS` and
//!   `SSQ_INFO` byte for byte,
//! * for each recorded `CHARACTER_CREATE` whose character was entered afterwards: the stats in
//!   the character list, the inventory, the skills and the shortcuts the Java server stored for
//!   the new character equal what [`build_plan`] makes of the same request.
//!
//! Failure messages name file, line and packet only, never names or bytes: captures contain
//! account and character data.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};

use game_server::client::charscreen::new_character_templates;
use game_server::client::newchar::{
    CreateRequest, IdSource, ItemMaxMp, PlanContext, StatBonus, build_plan, shortcut_type_ordinal,
};
use l2_config::GameConfig;
use l2_data::Datapack;
use l2_db::ItemLocation;
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::CharacterCreate;
use l2_protocol_game::server::enterworld::{ShortcutEntry, ShortcutInit, SkillList};
use l2_protocol_game::server::{
    CharCreateOk, CharDeleteSuccess, CharSelected, CharSelectionInfo, ItemList, SsqInfo,
};
use l2_util::WorldRng;

/// One game channel record of a capture.
struct Record {
    file: String,
    line: usize,
    conn: u64,
    c2s: bool,
    name: String,
    raw: Vec<u8>,
}

impl Record {
    fn label(&self) -> String {
        format!(
            "{} line {}: {} ({} bytes)",
            self.file,
            self.line,
            self.name,
            self.raw.len()
        )
    }

    /// The packet body after the opcode (or after the ex opcode).
    fn body(&self) -> &[u8] {
        let skip = if self.raw[0] == 0xFE || self.raw[0] == 0xD0 {
            3
        } else {
            1
        };
        &self.raw[skip..]
    }
}

fn records() -> Option<Vec<Record>> {
    let Some(dir) = std::env::var_os("L2_CAPTURE_DIR").map(PathBuf::from) else {
        eprintln!("L2_CAPTURE_DIR is not set: skipping the capture comparison");
        return None;
    };
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        for (i, line) in fs::read_to_string(&path).unwrap().lines().enumerate().skip(1) {
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if record["channel"] != "game" {
                continue;
            }
            let (Some(name), Some(raw), Some(conn)) = (
                record["name"].as_str(),
                record["raw"].as_str(),
                record["conn"].as_u64(),
            ) else {
                continue;
            };
            let Ok(raw) = hex::decode(raw) else { continue };
            if raw.is_empty() {
                continue;
            }
            out.push(Record {
                file: file.clone(),
                line: i + 1,
                conn,
                c2s: record["dir"] == "c2s",
                name: name.to_owned(),
                raw,
            });
        }
    }
    Some(out)
}

fn datapack_root() -> Option<PathBuf> {
    let root = std::env::var_os("L2_DATAPACK_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
    if root.join("data").is_dir() {
        Some(root)
    } else {
        eprintln!(
            "the Java datapack is not at {}: skipping the capture comparison",
            root.display()
        );
        None
    }
}

/// What the server answers right after each client packet, in order (from the capture).
fn expected_answers(client_packet: &str) -> Option<&'static [&'static str]> {
    Some(match client_packet {
        "NEW_CHARACTER" => &["NEW_CHARACTER_SUCCESS"],
        "CHARACTER_CREATE" => &["CHARACTER_CREATE_SUCCESS"],
        "REQUEST_GOTO_LOBBY" => &["CHARACTER_SELECTION_INFO"],
        "CHARACTER_SELECT" => &["SSQ_INFO", "CHARACTER_SELECTED"],
        "CHARACTER_DELETE" => &["CHARACTER_DELETE_SUCCESS", "CHARACTER_SELECTION_INFO"],
        "CHARACTER_RESTORE" => &["CHARACTER_SELECTION_INFO"],
        _ => return None,
    })
}

#[test]
fn answer_order_and_small_packets_match_the_recording() {
    let (Some(records), Some(_)) = (records(), datapack_root()) else {
        return;
    };
    let mut checked = 0;
    let mut dropped = 0;
    let mut problems = Vec::new();
    for (i, record) in records.iter().enumerate() {
        if !record.c2s {
            continue;
        }
        let Some(answers) = expected_answers(&record.name) else {
            continue;
        };
        let got: Vec<&str> = records[i + 1..]
            .iter()
            .take_while(|r| !r.c2s && r.conn == record.conn)
            .take(answers.len())
            .map(|r| r.name.as_str())
            .collect();
        let flood_protected =
            got.is_empty() && matches!(record.name.as_str(), "CHARACTER_SELECT" | "CHARACTER_RESTORE");
        if flood_protected {
            // Java drops a select or restore that comes too soon after the last one, silently.
            dropped += 1;
        } else if got != answers {
            problems.push(format!(
                "{}: answered with {} packets in another order",
                record.label(),
                got.len()
            ));
        } else {
            checked += 1;
        }
    }
    // The fixed packets, byte for byte.
    let fixed: [(&str, Vec<u8>); 3] = [
        ("CHARACTER_CREATE_SUCCESS", CharCreateOk.to_bytes()),
        ("CHARACTER_DELETE_SUCCESS", CharDeleteSuccess.to_bytes()),
        ("SSQ_INFO", SsqInfo { state: 0 }.to_bytes()),
    ];
    let mut compared = 0;
    for record in records.iter().filter(|r| !r.c2s) {
        if let Some((_, bytes)) = fixed.iter().find(|(name, _)| *name == record.name) {
            if &record.raw == bytes {
                compared += 1;
            } else {
                problems.push(format!("{}: differs from the encoded packet", record.label()));
            }
        }
    }
    eprintln!(
        "character screen: {checked} answer sequences and {compared} fixed packets identical, {dropped} selects dropped by the flood protector"
    );
    assert!(checked > 0, "no character screen packets in the capture");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn new_character_success_is_identical_to_the_recording() {
    let (Some(records), Some(root)) = (records(), datapack_root()) else {
        return;
    };
    let pack = Datapack::load(&root).unwrap();
    let ours = new_character_templates(&pack).to_bytes();
    let mut compared = 0;
    let mut problems = Vec::new();
    for record in records
        .iter()
        .filter(|r| !r.c2s && r.name == "NEW_CHARACTER_SUCCESS")
    {
        if record.raw == ours {
            compared += 1;
        } else {
            problems.push(format!("{}: differs from the encoded packet", record.label()));
        }
    }
    eprintln!(
        "NEW_CHARACTER_SUCCESS: {compared} identical, {} different",
        problems.len()
    );
    assert!(compared > 0, "no NEW_CHARACTER_SUCCESS in the capture");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The first record after `from` on the same connection with that name (server packets only).
fn next<'a>(records: &'a [Record], from: usize, name: &str) -> Option<&'a Record> {
    let conn = records[from].conn;
    records[from + 1..]
        .iter()
        .take_while(|r| r.conn == conn)
        .find(|r| !r.c2s && r.name == name)
}

#[test]
fn a_new_character_is_what_the_java_server_stored() {
    let (Some(records), Some(root)) = (records(), datapack_root()) else {
        return;
    };
    let pack = Datapack::load(&root).unwrap();
    let config = GameConfig::default();
    let bonus = StatBonus::load(&root);
    let max_mp = ItemMaxMp::load(&root, &ItemMaxMp::wanted_ids(&pack), false);
    let ctx = PlanContext {
        datapack: &pack,
        config: &config,
        stat_bonus: &bonus,
        item_max_mp: &max_mp,
        master_access_level: 0,
    };
    let mut verified = 0;
    let mut problems = Vec::new();
    for (i, record) in records.iter().enumerate() {
        if !(record.c2s && record.name == "CHARACTER_CREATE") {
            continue;
        }
        let create = CharacterCreate::decode(&mut PacketReader::new(record.body())).unwrap();
        // Create, lobby (list), select, enter world, then the lists of the entered character.
        let Some(lobby) = records[i + 1..]
            .iter()
            .take_while(|r| r.conn == record.conn)
            .position(|r| r.c2s && r.name == "REQUEST_GOTO_LOBBY")
            .map(|p| p + i + 1)
        else {
            continue;
        };
        let (Some(list), Some(select)) = (
            next(&records, lobby, "CHARACTER_SELECTION_INFO"),
            records[lobby + 1..]
                .iter()
                .take_while(|r| r.conn == record.conn)
                .position(|r| r.c2s && r.name == "CHARACTER_SELECT")
                .map(|p| p + lobby + 1),
        ) else {
            continue;
        };
        let list = CharSelectionInfo::decode(&mut PacketReader::new(list.body())).unwrap();
        let select_slot = {
            let mut r = PacketReader::new(records[select].body());
            r.i32().unwrap()
        };
        let Some(entry) = usize::try_from(select_slot)
            .ok()
            .and_then(|s| list.characters.get(s))
        else {
            continue;
        };
        // Only a character that was created in this very request is compared.
        if entry.name != create.name {
            continue;
        }
        let (Some(selected), Some(skills), Some(items), Some(bar)) = (
            next(&records, select, "CHARACTER_SELECTED"),
            next(&records, select, "SKILL_LIST"),
            next(&records, select, "ITEM_LIST"),
            next(&records, select, "SHORT_CUT_INIT"),
        ) else {
            continue;
        };
        let label = record.label();
        let selected = CharSelected::decode(&mut PacketReader::new(selected.body())).unwrap();
        let skills = SkillList::decode(&mut PacketReader::new(skills.body())).unwrap();
        let items = ItemList::decode(&mut PacketReader::new(items.body())).unwrap();
        let bar = ShortcutInit::decode(&mut PacketReader::new(bar.body())).unwrap();

        // Our plan for the same request.
        let template = pack.player_templates.get_by_id(create.class_id).unwrap();
        let next_id = AtomicI32::new(0x6000_0001);
        let allocate = || Some(next_id.fetch_add(1, Ordering::Relaxed));
        let mut ids = IdSource::new(&allocate);
        let request = CreateRequest {
            account: "acc",
            name: "Name",
            is_female: create.is_female,
            hair_style: create.hair_style,
            hair_color: create.hair_color,
            face: create.face,
        };
        let plan = build_plan(
            &ctx,
            &request,
            template,
            0x6000_0000,
            &mut ids,
            &mut WorldRng::from_seed(3),
            0,
        )
        .unwrap();

        let mut wrong = Vec::new();
        let mut check = |what: &str, ok: bool| {
            if !ok {
                wrong.push(what.to_owned());
            }
        };
        // The list entry.
        check("max hp", entry.max_hp == f64::from(plan.update.max_hp));
        check("max mp", entry.max_mp == f64::from(plan.update.max_mp));
        check("current hp", entry.current_hp == plan.update.cur_hp);
        check("current mp", entry.current_mp == plan.update.cur_mp);
        check(
            "level exp sp",
            (entry.level, entry.exp, entry.sp) == (plan.update.level, plan.update.exp, plan.update.sp as i32),
        );
        check(
            "class race sex",
            (entry.class_id, entry.base_class_id, entry.race, entry.sex)
                == (
                    plan.update.class_id,
                    plan.update.base_class,
                    plan.update.race,
                    plan.update.sex,
                ),
        );
        check(
            "look",
            (entry.hair_style, entry.hair_color, entry.face)
                == (plan.update.hair_style, plan.update.hair_color, plan.update.face),
        );
        check("vitality", entry.vitality_points == plan.update.vitality_points);
        check(
            "start point",
            template
                .creation_points
                .iter()
                .any(|p| (p.x, p.y, p.z) == (entry.x, entry.y, entry.z)),
        );
        let mut paperdoll = [0; 25];
        for item in plan.items.iter().filter(|i| i.loc == ItemLocation::Paperdoll) {
            let display = pack
                .items
                .get(item.item_id)
                .map_or(item.item_id, |t| t.display_id);
            paperdoll[usize::try_from(item.loc_data).unwrap()] = display;
        }
        check("paperdoll", entry.paperdoll == paperdoll);
        // CharSelected.
        check(
            "selected stats",
            (
                selected.str,
                selected.dex,
                selected.con,
                selected.int,
                selected.wit,
                selected.men,
            ) == (
                template.base_str,
                template.base_dex,
                template.base_con,
                template.base_int,
                template.base_wit,
                template.base_men,
            ),
        );
        check("selected title", selected.title.is_empty());
        check(
            "selected hp mp level",
            (selected.current_hp, selected.current_mp, selected.level)
                == (plan.update.cur_hp, plan.update.cur_mp, plan.update.level),
        );
        // Inventory: id, count, equipped slot (the object ids differ by design).
        let mut java_items: Vec<_> = items
            .items
            .iter()
            .map(|i| {
                (
                    i.item_id,
                    i.count,
                    i.equipped,
                    if i.equipped == 1 { i.location } else { 0 },
                )
            })
            .collect();
        let mut our_items: Vec<_> = plan
            .items
            .iter()
            .map(|i| {
                let equipped = i16::from(i.loc == ItemLocation::Paperdoll);
                (
                    i.item_id,
                    i.count,
                    equipped,
                    if equipped == 1 { i.loc_data } else { 0 },
                )
            })
            .collect();
        java_items.sort();
        our_items.sort();
        check("items", java_items == our_items);
        check(
            "item mana and time",
            items.items.iter().all(|i| i.mana == -1 && i.time == -9999)
                && plan.items.iter().all(|i| i.mana_left == -1 && i.time == -1),
        );
        // Skills.
        let mut java_skills: Vec<_> = skills.skills.iter().map(|s| (s.id, s.level)).collect();
        let mut our_skills: Vec<_> = plan.skills.iter().map(|s| (s.skill_id, s.skill_level)).collect();
        java_skills.sort();
        our_skills.sort();
        check("skills", java_skills == our_skills);
        // Shortcuts: an item shortcut points at an object id, compare the item behind it.
        let java_item_of: BTreeMap<i32, i32> = items.items.iter().map(|i| (i.object_id, i.item_id)).collect();
        let our_item_of: BTreeMap<i32, i32> = plan.items.iter().map(|i| (i.object_id, i.item_id)).collect();
        let mut java_bar: Vec<_> = bar
            .shortcuts
            .iter()
            .map(|s| match *s {
                ShortcutEntry::Item { slot, id, .. } => {
                    (slot, 1, java_item_of.get(&id).copied().unwrap_or(-1), 0)
                }
                ShortcutEntry::Skill { slot, id, level } => (slot, 2, id, level),
                ShortcutEntry::Other { kind, slot, id } => (slot, kind as i32, id, 0),
                ShortcutEntry::Empty { slot } => (slot, 0, 0, 0),
            })
            .collect();
        let mut our_bar: Vec<_> = plan
            .shortcuts
            .iter()
            .map(|s| {
                let id = if s.kind == shortcut_type_ordinal(l2_data::ShortcutType::Item) {
                    our_item_of.get(&s.shortcut_id).copied().unwrap_or(-1)
                } else {
                    s.shortcut_id
                };
                (s.slot + s.page * 12, s.kind, id, s.level)
            })
            .collect();
        java_bar.sort();
        our_bar.sort();
        check("shortcuts", java_bar == our_bar);

        if wrong.is_empty() {
            verified += 1;
        } else {
            problems.push(format!("{label}: differs in {}", wrong.join(", ")));
        }
    }
    eprintln!(
        "new characters: {verified} identical to the Java server, {} different",
        problems.len()
    );
    assert!(verified > 0, "no created and entered character in the capture");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
