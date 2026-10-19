//! Skill, buff and item-use packets against byte layouts worked out by hand from the Java
//! `writeImpl` / `readImpl` code, round trips, and the recorded Java game server sessions
//! (`fixtures/captures`, or `L2_CAPTURE_DIR`).
//!
//! The expected bytes are built with an independent little-endian builder (not the crate's
//! `PacketWriter`), so a mistake in the crate's encoder cannot hide behind the same mistake here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    RequestAcquireSkill, RequestAcquireSkillInfo, RequestAutoSoulShot, RequestDispel, RequestMagicSkillUse,
};
use l2_protocol_game::server::skills::{
    AbnormalStatusUpdate, AcquireSkillDone, AcquireSkillEntry, AcquireSkillInfo, AcquireSkillList,
    AcquireSkillRequirement, AcquireSkillType, EffectIcon, ExAutoSoulShot, MagicSkillCanceled,
    MagicSkillLaunched, MagicSkillUse, PartySpelled, ShortBuffStatusUpdate, SkillPosition,
};

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
    fn long(mut self, v: i64) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn done(self) -> Vec<u8> {
        self.0
    }
}

/// Encodes, checks the bytes, then decodes the bytes after `opcode_len` and expects the packet back.
fn check_round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(
    packet: &P,
    expected: &[u8],
    opcode_len: usize,
) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected, "encoded bytes");
    let decoded = P::decode(&mut PacketReader::new(&bytes[opcode_len..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
}

const CASTER: i32 = 0x1000_0001;
const TARGET: i32 = 0x1000_0002;
const SKILL: i32 = 1177;

fn pos(x: i32, y: i32, z: i32) -> SkillPosition {
    SkillPosition { x, y, z }
}

fn icon(display_id: i32, display_level: i32, time: i32) -> EffectIcon {
    EffectIcon {
        display_id,
        display_level,
        time,
        in_use: true,
    }
}

// ---------------------------------------------------------------------------------------------
// Server packets.
// ---------------------------------------------------------------------------------------------

#[test]
fn magic_skill_use_layout_without_a_ground_point() {
    // Java: caster id, target id, skill, level, hit time, reuse delay, caster position, the
    // ground-target short (0), the has-ground short (0), target position.
    let packet = MagicSkillUse {
        creature_id: CASTER,
        target_id: TARGET,
        skill_id: SKILL,
        skill_level: 1,
        hit_time: 1000,
        reuse_delay: 500,
        creature: pos(10, 20, 30),
        ground_location: None,
        target: pos(40, 50, 60),
    };
    let expected = Bytes::default()
        .byte(0x48)
        .int(CASTER)
        .int(TARGET)
        .int(SKILL)
        .int(1)
        .int(1000)
        .int(500)
        .int(10)
        .int(20)
        .int(30)
        .short(0)
        .short(0)
        .int(40)
        .int(50)
        .int(60)
        .done();
    // The same length as the recorded Java packets (53 bytes).
    assert_eq!(expected.len(), 53);
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn magic_skill_use_with_a_ground_point_writes_the_flag_and_the_point() {
    let packet = MagicSkillUse {
        creature_id: CASTER,
        target_id: TARGET,
        skill_id: SKILL,
        skill_level: 2,
        hit_time: 0,
        reuse_delay: 0,
        creature: pos(1, 2, 3),
        ground_location: Some(pos(7, 8, 9)),
        target: pos(4, 5, 6),
    };
    let expected = Bytes::default()
        .byte(0x48)
        .int(CASTER)
        .int(TARGET)
        .int(SKILL)
        .int(2)
        .int(0)
        .int(0)
        .int(1)
        .int(2)
        .int(3)
        .short(0)
        .short(1)
        .int(7)
        .int(8)
        .int(9)
        .int(4)
        .int(5)
        .int(6)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn magic_skill_launched_lists_the_target_ids_after_a_count() {
    let packet = MagicSkillLaunched {
        creature_id: CASTER,
        skill_id: SKILL,
        skill_level: 1,
        target_ids: vec![TARGET, CASTER],
    };
    let expected = Bytes::default()
        .byte(0x54)
        .int(CASTER)
        .int(SKILL)
        .int(1)
        .int(2)
        .int(TARGET)
        .int(CASTER)
        .done();
    check_round_trip(&packet, &expected, 1);
    // A cast with no target still sends the (zero) count.
    let none = MagicSkillLaunched {
        target_ids: Vec::new(),
        ..packet
    };
    check_round_trip(
        &none,
        &Bytes::default()
            .byte(0x54)
            .int(CASTER)
            .int(SKILL)
            .int(1)
            .int(0)
            .done(),
        1,
    );
}

#[test]
fn magic_skill_canceled_layout() {
    check_round_trip(
        &MagicSkillCanceled { object_id: CASTER },
        &Bytes::default().byte(0x49).int(CASTER).done(),
        1,
    );
}

#[test]
fn abnormal_status_update_writes_a_short_count_then_display_id_level_and_time() {
    let mut update = AbnormalStatusUpdate::default();
    update.add_skill(icon(1204, 2, 90), false);
    update.add_skill(icon(1331, 1, 12), false);
    let expected = Bytes::default()
        .byte(0x85)
        .short(2)
        .int(1204)
        .short(2)
        .int(90)
        .int(1331)
        .short(1)
        .int(12)
        .done();
    check_round_trip(&update, &expected, 1);
}

#[test]
fn abnormal_status_update_with_no_buffs_is_the_opcode_and_a_zero_count() {
    // The fixtures hold these: `85 00 00`.
    check_round_trip(
        &AbnormalStatusUpdate::default(),
        &Bytes::default().byte(0x85).short(0).done(),
        1,
    );
}

#[test]
fn healing_potion_skills_get_no_icon() {
    // Java `addSkill`: a healing potion is left out of the list.
    let mut update = AbnormalStatusUpdate::default();
    update.add_skill(icon(2031, 1, 0), true);
    update.add_skill(icon(1204, 2, 90), false);
    assert_eq!(update.effects, vec![icon(1204, 2, 90)]);
}

#[test]
fn abnormal_status_update_counts_every_icon_but_writes_only_the_ones_in_use() {
    // NOTE(java-quirk): Java's `writeShort(_effects.size())` counts the icon that is not in use,
    // while only the in-use icon is written. The expected bytes are that exact output.
    let hidden = EffectIcon {
        in_use: false,
        ..icon(1204, 2, 90)
    };
    let update = AbnormalStatusUpdate {
        effects: vec![hidden, icon(1331, 1, 12)],
    };
    let expected = Bytes::default()
        .byte(0x85)
        .short(2)
        .int(1331)
        .short(1)
        .int(12)
        .done();
    assert_eq!(update.to_bytes(), expected);
}

#[test]
fn short_buff_status_update_layout_and_the_reset_packet() {
    let short = ShortBuffStatusUpdate {
        skill_id: 1331,
        skill_level: 1,
        duration: 12,
    };
    let expected = Bytes::default().byte(0xFA).int(1331).int(1).int(12).done();
    check_round_trip(&short, &expected, 1);
    // Java RESET_SHORT_BUFF: all zeros.
    assert_eq!(
        ShortBuffStatusUpdate::RESET_SHORT_BUFF.to_bytes(),
        Bytes::default().byte(0xFA).int(0).int(0).int(0).done()
    );
}

#[test]
fn ex_auto_soulshot_uses_the_ex_id_0x0c() {
    let packet = ExAutoSoulShot {
        item_id: 1463,
        shot_type: ExAutoSoulShot::ON,
    };
    let expected = Bytes::default()
        .byte(0xFE)
        .byte(0x0C)
        .byte(0x00)
        .int(1463)
        .int(1)
        .done();
    check_round_trip(&packet, &expected, 3);
    assert_eq!(ExAutoSoulShot::OFF, 0);
}

#[test]
fn acquire_skill_list_layout_for_a_normal_type() {
    let mut list = AcquireSkillList::new(AcquireSkillType::Class);
    list.add_skill(1, 2, 3, 400, 0);
    let expected = Bytes::default()
        .byte(0x90)
        .int(0)
        .int(1)
        .int(1)
        .int(2)
        .int(3)
        .int(400)
        .int(0)
        .done();
    check_round_trip(&list, &expected, 1);
}

#[test]
fn acquire_skill_list_of_subpledge_skills_writes_a_zero_after_each_entry() {
    // Java: `if (_skillType == SUBPLEDGE) writeInt(0)`, after each skill.
    let list = AcquireSkillList {
        skill_type: AcquireSkillType::Subpledge,
        skills: vec![
            AcquireSkillEntry {
                id: 370,
                next_level: 1,
                max_level: 1,
                sp_cost: 0,
                requirements: 0,
            },
            AcquireSkillEntry {
                id: 371,
                next_level: 2,
                max_level: 3,
                sp_cost: 0,
                requirements: 0,
            },
        ],
    };
    let expected = Bytes::default()
        .byte(0x90)
        .int(3)
        .int(2)
        .int(370)
        .int(1)
        .int(1)
        .int(0)
        .int(0)
        .int(0)
        .int(371)
        .int(2)
        .int(3)
        .int(0)
        .int(0)
        .int(0)
        .done();
    check_round_trip(&list, &expected, 1);
}

#[test]
fn acquire_skill_list_with_no_skills_sends_nothing() {
    // Java returns before writing the opcode, so no bytes at all.
    assert!(
        AcquireSkillList::new(AcquireSkillType::Fishing)
            .to_bytes()
            .is_empty()
    );
}

#[test]
fn acquire_skill_info_layout_with_a_long_item_count() {
    let info = AcquireSkillInfo {
        skill_type: AcquireSkillType::Pledge,
        skill_id: 370,
        level: 1,
        sp_cost: 500,
        requirements: vec![AcquireSkillRequirement::item(9000, 5_000_000_000)],
    };
    // Java: id, level, SP, the type ordinal, the count, then per requirement: type 99, item id,
    // count (a long, so 8 bytes), unk 50.
    let expected = Bytes::default()
        .byte(0x91)
        .int(370)
        .int(1)
        .int(500)
        .int(2)
        .int(1)
        .int(99)
        .int(9000)
        .long(5_000_000_000)
        .int(50)
        .done();
    check_round_trip(&info, &expected, 1);
    assert_eq!(AcquireSkillRequirement::ITEM_TYPE, 99);
    assert_eq!(AcquireSkillRequirement::UNK, 50);
}

#[test]
fn acquire_skill_done_is_the_opcode_alone() {
    check_round_trip(&AcquireSkillDone, &[0x94], 1);
}

#[test]
fn party_spelled_layout_and_its_creature_types() {
    assert_eq!(PartySpelled::PLAYER, 0);
    assert_eq!(PartySpelled::PET, 1);
    assert_eq!(PartySpelled::SERVITOR, 2);
    let party = PartySpelled {
        creature_type: PartySpelled::SERVITOR,
        creature_id: TARGET,
        effects: vec![icon(1204, 2, 90)],
    };
    let expected = Bytes::default()
        .byte(0xF4)
        .int(2)
        .int(TARGET)
        .int(1)
        .int(1204)
        .short(2)
        .int(90)
        .done();
    check_round_trip(&party, &expected, 1);
}

#[test]
fn party_spelled_counts_hidden_icons_like_the_abnormal_status() {
    // The same Java quirk as ABNORMAL_STATUS_UPDATE, but here the count is an int.
    let hidden = EffectIcon {
        in_use: false,
        ..icon(1204, 2, 90)
    };
    let party = PartySpelled {
        creature_type: PartySpelled::PLAYER,
        creature_id: CASTER,
        effects: vec![hidden],
    };
    assert_eq!(
        party.to_bytes(),
        Bytes::default().byte(0xF4).int(0).int(CASTER).int(1).done()
    );
}

#[test]
fn acquire_skill_type_ids_are_the_java_ordinals() {
    let ids: Vec<i32> = AcquireSkillType::ALL.iter().map(|t| t.id()).collect();
    assert_eq!(ids, (0..8).collect::<Vec<i32>>());
    assert_eq!(AcquireSkillType::from_id(3), Some(AcquireSkillType::Subpledge));
    // Java `values()[id]` throws out of range, so these are not types at all here.
    assert_eq!(AcquireSkillType::from_id(8), None);
    assert_eq!(AcquireSkillType::from_id(-1), None);
}

// ---------------------------------------------------------------------------------------------
// Client packets.
// ---------------------------------------------------------------------------------------------

#[test]
fn request_magic_skill_use_layout_and_the_java_flag_rules() {
    let packet = RequestMagicSkillUse {
        magic_id: SKILL,
        ctrl_pressed: true,
        shift_pressed: true,
    };
    let expected = Bytes::default().byte(0x39).int(SKILL).int(1).byte(1).done();
    check_round_trip(&packet, &expected, 1);
    // Java reads the ctrl flag as `readInt() != 0` and the shift flag as `readByte() != 0`: any
    // non-zero value counts as pressed.
    let raw = Bytes::default().int(SKILL).int(7).byte(0xFF).done();
    let decoded = RequestMagicSkillUse::decode(&mut PacketReader::new(&raw)).unwrap();
    assert!(decoded.ctrl_pressed);
    assert!(decoded.shift_pressed);
}

#[test]
fn request_acquire_skill_sends_the_squad_only_for_subpledge() {
    let class = RequestAcquireSkill {
        id: 3,
        level: 2,
        skill_type: AcquireSkillType::Class,
        sub_type: 0,
    };
    check_round_trip(
        &class,
        &Bytes::default().byte(0x7C).int(3).int(2).int(0).done(),
        1,
    );
    let squad = RequestAcquireSkill {
        id: 3,
        level: 2,
        skill_type: AcquireSkillType::Subpledge,
        sub_type: 1001,
    };
    check_round_trip(
        &squad,
        &Bytes::default().byte(0x7C).int(3).int(2).int(3).int(1001).done(),
        1,
    );
}

#[test]
fn request_acquire_skill_rejects_an_unknown_type() {
    // Java `values()[id]` throws, the packet is dropped.
    let raw = Bytes::default().int(3).int(2).int(8).done();
    assert!(RequestAcquireSkill::decode(&mut PacketReader::new(&raw)).is_err());
}

#[test]
fn request_acquire_skill_info_layout() {
    let info = RequestAcquireSkillInfo {
        id: 4,
        level: 1,
        skill_type: AcquireSkillType::Transform,
    };
    check_round_trip(&info, &Bytes::default().byte(0x73).int(4).int(1).int(4).done(), 1);
}

#[test]
fn request_auto_soulshot_is_an_ex_packet_with_both_ints() {
    let shot = RequestAutoSoulShot {
        item_id: 1463,
        shot_type: RequestAutoSoulShot::OFF,
    };
    let expected = Bytes::default()
        .byte(0xD0)
        .byte(0x0D)
        .byte(0x00)
        .int(1463)
        .int(0)
        .done();
    check_round_trip(&shot, &expected, 3);
}

#[test]
fn request_dispel_is_an_ex_packet_with_three_ints() {
    let dispel = RequestDispel {
        object_id: CASTER,
        skill_id: 1204,
        skill_level: 2,
    };
    let expected = Bytes::default()
        .byte(0xD0)
        .byte(0x4B)
        .byte(0x00)
        .int(CASTER)
        .int(1204)
        .int(2)
        .done();
    check_round_trip(&dispel, &expected, 3);
}

// ---------------------------------------------------------------------------------------------
// Capture-driven check against the owner's Java sessions.
// ---------------------------------------------------------------------------------------------

/// Server packets of this file: the recorded bytes must re-encode exactly.
fn check_server(opcode: u8, raw: &[u8]) -> Option<bool> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0x48 => MagicSkillUse::decode(body).ok()?.to_bytes(),
        0x54 => MagicSkillLaunched::decode(body).ok()?.to_bytes(),
        0x49 => MagicSkillCanceled::decode(body).ok()?.to_bytes(),
        0x85 => AbnormalStatusUpdate::decode(body).ok()?.to_bytes(),
        0xFA => ShortBuffStatusUpdate::decode(body).ok()?.to_bytes(),
        0xF4 => PartySpelled::decode(body).ok()?.to_bytes(),
        0x90 => AcquireSkillList::decode(body).ok()?.to_bytes(),
        0x91 => AcquireSkillInfo::decode(body).ok()?.to_bytes(),
        _ => return None,
    };
    Some(encoded == raw)
}

/// Client packets of this file. The client may pad a packet, so only the recorded prefix that
/// we encode is compared.
fn check_client(opcode: u8, raw: &[u8]) -> Option<bool> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0x39 => RequestMagicSkillUse::decode(body).ok()?.to_bytes(),
        0x7C => RequestAcquireSkill::decode(body).ok()?.to_bytes(),
        0x73 => RequestAcquireSkillInfo::decode(body).ok()?.to_bytes(),
        _ => return None,
    };
    Some(raw.starts_with(&encoded))
}

/// Set `L2_CAPTURE_DIR` to a folder with `*.jsonl` captures (format: `l2-proxy` capture.rs).
/// Failure messages name the file, line and packet only, never the bytes: captures contain
/// account and character data.
#[test]
fn skill_packets_match_recorded_java_sessions() {
    // The owner's own recordings (L2_CAPTURE_DIR), else the scrubbed fixtures in the repo.
    let from_env = std::env::var_os("L2_CAPTURE_DIR");
    let dir = from_env
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"));
    let mut compared: BTreeMap<String, usize> = BTreeMap::new();
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
            if record["channel"] != "game" {
                continue;
            }
            let (Some(opcode), Some(raw)) = (record["opcode"].as_u64(), record["raw"].as_str()) else {
                continue;
            };
            let Ok(raw) = hex::decode(raw) else { continue };
            if raw.is_empty() {
                continue;
            }
            // Ex packets (0xFE, 0xD0) are not in this file's set; skip them.
            if record.get("ex_opcode").is_some_and(|v| !v.is_null()) {
                continue;
            }
            let result = match record["dir"].as_str() {
                Some("s2c") => check_server(opcode as u8, &raw),
                Some("c2s") => check_client(opcode as u8, &raw),
                _ => None,
            };
            let name = record["name"].as_str().unwrap_or("?").to_owned();
            match result {
                Some(true) => *compared.entry(name).or_default() += 1,
                Some(false) => mismatches.push(format!(
                    "{} line {}: {} ({} bytes)",
                    path.file_name().unwrap().to_string_lossy(),
                    i + 1,
                    name,
                    raw.len()
                )),
                None => {}
            }
        }
    }
    let total: usize = compared.values().sum();
    eprintln!(
        "skill capture comparison: {total} packets identical, {} different",
        mismatches.len()
    );
    for (name, count) in &compared {
        eprintln!("  {name}: {count} identical");
    }
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
    // The committed fixtures hold recorded MagicSkillUse, MagicSkillLaunched,
    // AbnormalStatusUpdate and RequestMagicSkillUse packets.
    if from_env.is_none() {
        for name in [
            "MAGIC_SKILL_USE",
            "MAGIC_SKILL_LAUNCHED",
            "ABNORMAL_STATUS_UPDATE",
            "REQUEST_MAGIC_SKILL_USE",
        ] {
            assert!(compared.contains_key(name), "no {name} packets in the fixtures");
        }
    }
}
