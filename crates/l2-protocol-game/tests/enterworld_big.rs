//! `UserInfo` and `ItemList` against byte layouts worked out by hand from the Java `writeImpl`
//! code, and against recorded Java game server sessions when `L2_CAPTURE_DIR` is set.
//!
//! The expected bytes are built with a tiny writer of their own (not the crate's
//! `PacketWriter`), listing the fields in the order of the Java source, so a mistake in the
//! crate's encoder cannot hide behind the same mistake in the test.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader, PacketWriter};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::*;

/// Independent little-endian builder.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn byte(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn short(mut self, v: i16) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn int(mut self, v: i32) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn ints(mut self, vs: &[i32]) -> Self {
        for v in vs {
            self.0.extend(v.to_le_bytes());
        }
        self
    }
    fn long(mut self, v: i64) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn double(mut self, v: f64) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    /// Java `writeString`: UTF-16LE plus a 0x0000 terminator.
    fn text(mut self, s: &str) -> Self {
        for unit in s.encode_utf16() {
            self.0.extend(unit.to_le_bytes());
        }
        self.0.extend([0, 0]);
        self
    }
}

/// A value per slot: `base + slot`, so a swapped slot shows up.
fn per_slot(base: i32) -> [i32; paperdoll::TOTAL_SLOTS] {
    let mut out = [0; paperdoll::TOTAL_SLOTS];
    for (slot, value) in out.iter_mut().enumerate() {
        *value = base + slot as i32;
    }
    out
}

/// The wire order of the paperdoll slots as an independent list of slot numbers (the Java
/// `PAPERDOLL_ORDER`, with RHAND = 5 twice).
const WIRE: [usize; 26] = [
    0, 8, 9, 4, 13, 14, 1, 5, 7, 10, 6, 11, 12, 23, 5, 2, 3, 16, 15, 17, 18, 19, 20, 21, 22, 24,
];

fn paperdoll_block(values: &[i32; paperdoll::TOTAL_SLOTS]) -> Vec<i32> {
    WIRE.iter().map(|&slot| values[slot]).collect()
}

fn sample_user_info() -> UserInfo {
    UserInfo {
        x: -83_000,
        y: 243_000,
        z: -3_700,
        vehicle_id: 0,
        object_id: 268_480_000,
        name: "Ab".into(),
        race: 2,
        is_female: true,
        base_class: 31,
        level: 76,
        exp: 5_000_000_000,
        exp_percent: 0.25,
        str: 40,
        dex: 30,
        con: 43,
        int: 21,
        wit: 11,
        men: 25,
        max_hp: 1_000,
        current_hp: 900,
        max_mp: 800,
        current_mp: 700,
        sp: 123_456,
        current_load: 5_000,
        max_load: 69_000,
        weapon_equipped: true,
        paperdoll_object_ids: per_slot(1_000),
        paperdoll_item_ids: per_slot(2_000),
        paperdoll_augmentation_ids: per_slot(3_000),
        talisman_slots: 2,
        can_equip_cloak: true,
        p_atk: 301,
        p_atk_spd: 330,
        p_def: 202,
        evasion: 33,
        accuracy: 44,
        critical: 55,
        m_atk: 404,
        m_atk_spd: 333,
        m_def: 505,
        pvp_flag: 1,
        karma: 9,
        run_speed: 126,
        walk_speed: 80,
        swim_run_speed: 50,
        swim_walk_speed: 40,
        fly_run_speed: 11,
        fly_walk_speed: 12,
        move_multiplier: 1.1,
        attack_speed_multiplier: 1.5,
        collision_radius: 8.5,
        collision_height: 23.25,
        hair_style: 3,
        hair_color: 2,
        face: 1,
        is_gm: true,
        title: "Tt".into(),
        clan_id: 7,
        clan_crest_id: 8,
        ally_id: 9,
        ally_crest_id: 10,
        relation: 0x40,
        mount_type: 1,
        private_store_type: 3,
        has_dwarven_craft: true,
        pk_kills: 4,
        pvp_kills: 5,
        cubics: vec![1, 2],
        in_party_match_room: true,
        abnormal_visual_effects: 0x2000,
        water_or_flying: 2,
        clan_privileges: 0xFF,
        recom_left: 20,
        recom_have: 30,
        mount_npc_id: 1_012_345,
        inventory_limit: 80,
        class_id: 32,
        max_cp: 600,
        current_cp: 500,
        enchant_effect: 16,
        team: 2,
        clan_crest_large_id: 11,
        noble: true,
        hero: true,
        fishing: true,
        fish_x: 1,
        fish_y: 2,
        fish_z: 3,
        name_color: 0x00FF_FFFF,
        running: true,
        pledge_class: 5,
        pledge_type: -1,
        title_color: 0x00EC_9B2D,
        cursed_weapon_level: 3,
        transformation_display_id: 301,
        attack_element: -2,
        attack_element_value: 0,
        defense_fire: 1,
        defense_water: 2,
        defense_wind: 3,
        defense_earth: 4,
        defense_holy: 5,
        defense_dark: 6,
        agathion_id: 1_001,
        fame: 777,
        minimap_allowed: true,
        vitality_points: 17_000,
        abnormal_visual_effect_special: 4,
    }
}

#[test]
fn user_info_matches_the_java_layout() {
    let packet = sample_user_info();
    let expected = Bytes::default()
        .byte(0x32) // USER_INFO
        .ints(&[-83_000, 243_000, -3_700]) // x y z
        .int(0) // vehicle
        .int(268_480_000) // object id
        .text("Ab")
        .ints(&[2, 1, 31, 76]) // race, female, base class, level
        .long(5_000_000_000)
        .double(0.25)
        .ints(&[40, 30, 43, 21, 11, 25]) // STR DEX CON INT WIT MEN
        .ints(&[1_000, 900, 800, 700]) // max hp, hp, max mp, mp
        .ints(&[123_456, 5_000, 69_000]) // sp, load, max load
        .int(40) // weapon equipped
        .ints(&paperdoll_block(&per_slot(1_000)))
        .ints(&paperdoll_block(&per_slot(2_000)))
        .ints(&paperdoll_block(&per_slot(3_000)))
        .ints(&[2, 1]) // talisman slots, can equip cloak
        .ints(&[301, 330, 202, 33, 44, 55]) // patk, patk spd, pdef, evasion, accuracy, crit
        .ints(&[404, 333, 330, 505]) // matk, matk spd, patk spd again, mdef
        .ints(&[1, 9]) // pvp flag, karma
        .ints(&[126, 80, 50, 40]) // run walk swim-run swim-walk
        .ints(&[11, 12, 11, 12]) // fly run, fly walk, twice
        .double(1.1)
        .double(1.5)
        .double(8.5)
        .double(23.25)
        .ints(&[3, 2, 1]) // hair style, colour, face
        .int(1) // builder level
        .text("Tt")
        .ints(&[7, 8, 9, 10]) // clan, clan crest, ally, ally crest
        .int(0x40) // relation
        .byte(1) // mount type
        .byte(3) // private store
        .byte(1) // dwarven craft
        .ints(&[4, 5]) // pk, pvp
        .short(2)
        .short(1)
        .short(2) // cubics
        .byte(1) // party match room
        .int(0x2000)
        .byte(2) // water / flying
        .int(0xFF) // clan privileges
        .short(20)
        .short(30) // recommendations
        .int(1_012_345) // mount npc id
        .short(80) // inventory limit
        .int(32) // class id
        .int(0) // special effects
        .ints(&[600, 500]) // max cp, cp
        .byte(16) // enchant effect
        .byte(2) // team
        .int(11) // large crest
        .byte(1)
        .byte(1) // noble, hero
        .byte(1) // fishing
        .ints(&[1, 2, 3]) // fish x y z
        .int(0x00FF_FFFF) // name colour
        .byte(1) // running
        .ints(&[5, -1, 0x00EC_9B2D, 3]) // pledge class, type, title colour, cursed weapon
        .int(301) // transformation
        .short(-2)
        .short(0) // attack element + value
        .short(1)
        .short(2)
        .short(3)
        .short(4)
        .short(5)
        .short(6) // defence elements
        .int(1_001) // agathion
        .ints(&[777, 1, 17_000, 4]) // fame, minimap, vitality, special visual effect
        .0;
    let bytes = packet.to_bytes();
    assert_eq!(bytes.len(), expected.len());
    assert_eq!(bytes, expected);
    let decoded = UserInfo::decode(&mut PacketReader::new(&expected[1..])).unwrap();
    assert_eq!(decoded, packet);
}

#[test]
fn user_info_without_weapon_and_with_no_cubics() {
    let packet = UserInfo {
        weapon_equipped: false,
        cubics: Vec::new(),
        ..sample_user_info()
    };
    let bytes = packet.to_bytes();
    // After the three level ints and the long/double/stats the weapon int is 20.
    let weapon_at = 1 + 5 * 4 + (2 + 1) * 2 + 4 * 4 + 8 + 8 + 6 * 4 + 4 * 4 + 3 * 4;
    assert_eq!(bytes[weapon_at..weapon_at + 4], 20_i32.to_le_bytes());
    assert_eq!(
        UserInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
        packet
    );
}

#[test]
fn user_info_rejects_a_bad_weapon_flag() {
    let mut bytes = sample_user_info().to_bytes();
    let weapon_at = 1 + 5 * 4 + (2 + 1) * 2 + 4 * 4 + 8 + 8 + 6 * 4 + 4 * 4 + 3 * 4;
    bytes[weapon_at] = 41;
    assert!(UserInfo::decode(&mut PacketReader::new(&bytes[1..])).is_err());
}

#[test]
fn user_info_truncated_input_is_an_error() {
    let bytes = sample_user_info().to_bytes();
    for cut in [1, 10, 100, bytes.len() - 1] {
        assert!(
            UserInfo::decode(&mut PacketReader::new(&bytes[1..cut])).is_err(),
            "cut at {cut}"
        );
    }
}

fn sample_item(n: i32) -> ItemInfo {
    ItemInfo {
        object_id: 268_500_000 + n,
        item_id: 57,
        location: n,
        count: 5_000_000_000,
        type2: 4,
        custom_type1: 1,
        equipped: 1,
        body_part: 0x4000,
        enchant: 7,
        custom_type2: 2,
        augmentation: 12_345,
        mana: 120,
        time: 3_600,
        attack_element_type: 3,
        attack_element_power: 50,
        element_defense: [1, 2, 3, 4, 5, 6],
        enchant_options: [101, 102, 103],
    }
}

fn expected_item(n: i32) -> Bytes {
    Bytes::default()
        .int(268_500_000 + n) // object id
        .int(57) // display id
        .int(n) // location
        .long(5_000_000_000) // count
        .short(4) // type 2
        .short(1) // custom type 1
        .short(1) // equipped
        .int(0x4000) // body part mask
        .short(7) // enchant
        .short(2) // custom type 2
        .int(12_345) // augmentation
        .int(120) // mana
        .int(3_600) // time
        .short(3)
        .short(50) // attack element + power
        .short(1)
        .short(2)
        .short(3)
        .short(4)
        .short(5)
        .short(6) // defence elements
        .short(101)
        .short(102)
        .short(103) // enchant options
}

#[test]
fn item_list_matches_the_java_layout() {
    let packet = ItemList {
        show_window: true,
        items: vec![sample_item(1), sample_item(2)],
        block: None,
    };
    let mut expected = Bytes::default().byte(0x11).short(1).short(2);
    expected.0.extend(expected_item(1).0);
    expected.0.extend(expected_item(2).0);
    let expected = expected.short(0).0; // no inventory block
    assert_eq!(packet.to_bytes(), expected);
    assert_eq!(
        ItemList::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        packet
    );
}

#[test]
fn item_list_with_an_inventory_block() {
    let packet = ItemList {
        show_window: false,
        items: vec![],
        block: Some(InventoryBlock {
            mode: 1,
            items: vec![57, 1_060],
        }),
    };
    let expected = Bytes::default()
        .byte(0x11)
        .short(0) // no window
        .short(0) // no items
        .short(2) // block size
        .byte(1) // block mode
        .ints(&[57, 1_060])
        .0;
    assert_eq!(packet.to_bytes(), expected);
    assert_eq!(
        ItemList::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        packet
    );
}

#[test]
fn plain_item_uses_the_java_defaults() {
    let item = ItemInfo::plain(1, 57, 10);
    let mut w = PacketWriter::new();
    write_item(&mut w, &item);
    let bytes = w.into_bytes();
    // 3 ints + long + 3 shorts + int + 2 shorts + 3 ints + 2 shorts + 6 shorts + 3 shorts.
    assert_eq!(bytes.len(), 12 + 8 + 6 + 4 + 4 + 12 + 4 + 12 + 6);
    let mana_at = 12 + 8 + 6 + 4 + 4 + 4;
    assert_eq!(bytes[mana_at..mana_at + 4], (-1_i32).to_le_bytes());
    assert_eq!(bytes[mana_at + 4..mana_at + 8], (-9999_i32).to_le_bytes());
    assert_eq!(bytes[mana_at + 8..mana_at + 10], (-2_i16).to_le_bytes());
    assert_eq!(read_item(&mut PacketReader::new(&bytes)).unwrap(), item);
}

#[test]
fn item_updates_write_change_before_each_item() {
    let updates = vec![
        ItemUpdate {
            change: 1,
            item: sample_item(1),
        },
        ItemUpdate {
            change: 3,
            item: sample_item(2),
        },
    ];
    let mut w = PacketWriter::new();
    write_item_updates(&mut w, &updates);
    let mut expected = Bytes::default().short(2).short(1);
    expected.0.extend(expected_item(1).0);
    let mut expected = expected.short(3);
    expected.0.extend(expected_item(2).0);
    let bytes = w.into_bytes();
    assert_eq!(bytes, expected.0);
    assert_eq!(
        read_item_updates(&mut PacketReader::new(&bytes)).unwrap(),
        updates
    );
}

#[test]
fn item_list_truncated_input_is_an_error() {
    let bytes = ItemList {
        show_window: true,
        items: vec![sample_item(1)],
        block: None,
    }
    .to_bytes();
    for cut in [1, 3, 10, bytes.len() - 1] {
        assert!(
            ItemList::decode(&mut PacketReader::new(&bytes[1..cut])).is_err(),
            "cut at {cut}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Capture-driven check against the owner's Java sessions.
// ---------------------------------------------------------------------------------------------

/// Re-encodes a recorded `UserInfo` / `ItemList`; `None` for other opcodes or a decode failure.
fn check_server(opcode: u8, raw: &[u8]) -> Option<bool> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0x32 => UserInfo::decode(body).ok()?.to_bytes(),
        0x11 => ItemList::decode(body).ok()?.to_bytes(),
        _ => return None,
    };
    Some(encoded == raw)
}

/// Set `L2_CAPTURE_DIR` to a folder with `*.jsonl` captures (format: `l2-proxy` capture.rs).
/// Failure messages name the file, line and packet only, never the bytes: captures contain
/// account and character data.
#[test]
fn user_info_and_item_list_match_recorded_java_sessions() {
    // The owner's own recordings (L2_CAPTURE_DIR), else the scrubbed fixtures in the repo
    // (M2-CAP-01), so CI compares against real Java packets too.
    let dir = std::env::var_os("L2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"));
    let mut compared = 0;
    let mut mismatches = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        for (i, line) in text.lines().enumerate().skip(1) {
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if record["channel"] != "game" || record["dir"] != "s2c" {
                continue;
            }
            if record.get("ex_opcode").is_some_and(|v| !v.is_null()) {
                continue;
            }
            let (Some(opcode), Some(raw)) = (record["opcode"].as_u64(), record["raw"].as_str()) else {
                continue;
            };
            let Ok(raw) = hex::decode(raw) else { continue };
            if raw.is_empty() {
                continue;
            }
            // A packet of these two kinds that does not decode counts as a mismatch too.
            let result = match opcode {
                0x32 | 0x11 => Some(check_server(opcode as u8, &raw).unwrap_or(false)),
                _ => None,
            };
            match result {
                Some(true) => compared += 1,
                Some(false) => mismatches.push(format!(
                    "{} line {}: {} ({} bytes)",
                    path.file_name().unwrap().to_string_lossy(),
                    i + 1,
                    record["name"].as_str().unwrap_or("?"),
                    raw.len()
                )),
                None => {}
            }
        }
    }
    eprintln!(
        "capture comparison: {compared} packets identical, {} different",
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
}
