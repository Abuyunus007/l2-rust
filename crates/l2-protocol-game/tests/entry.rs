//! `MacroList`, `ExQuestItemList`, `ExRotation`, `ExShowContactList`, `ActionFailed`,
//! `ExBrExtraUserInfo`, the Nevit packets and `RequestSkillCoolTime`: byte layouts worked out by
//! hand from the Java `writeImpl` code, and the recorded Java sessions in `fixtures/captures/`
//! (or `L2_CAPTURE_DIR`).
//!
//! The expected bytes are built with a tiny writer of their own, listing the fields in the order
//! of the Java source, so a mistake in the crate's encoder cannot hide behind the same mistake
//! here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::RequestSkillCoolTime;
use l2_protocol_game::server::{
    ActionFailed, ExBrExtraUserInfo, ExNevitAdventEffect, ExNevitAdventPointInfo, ExNevitAdventTimeChange,
    ExQuestItemList, ExRotation, ExShowContactList, ExVitalityPointInfo, InventoryBlock, ItemInfo,
    MacroCommandEntry, MacroEntry, MacroList,
};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

/// Independent little-endian builder.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn byte(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn short(mut self, v: u16) -> Self {
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
    /// Java `writeString`: UTF-16LE plus a 0x0000 terminator.
    fn text(mut self, s: &str) -> Self {
        for unit in s.encode_utf16() {
            self.0.extend(unit.to_le_bytes());
        }
        self.0.extend([0, 0]);
        self
    }
    fn done(self) -> Vec<u8> {
        self.0
    }
}

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

#[test]
fn macro_list_without_a_macro() {
    // Java `sendUpdate` of an empty list: revision, 0, count 0, "has macro" 0.
    let packet = MacroList {
        revision: 2,
        count: 0,
        macro_entry: None,
    };
    let expected = Bytes::default().byte(0xE8).int(2).byte(0).byte(0).byte(0).done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn macro_list_with_a_macro() {
    let packet = MacroList {
        revision: 5,
        count: 3,
        macro_entry: Some(MacroEntry {
            id: 1001,
            name: "Hello".into(),
            descr: String::new(),
            acronym: "HI".into(),
            icon: 7,
            commands: vec![
                MacroCommandEntry {
                    kind: 1,
                    d1: 1177,
                    d2: 2,
                    cmd: "".into(),
                },
                MacroCommandEntry {
                    kind: 3,
                    d1: 0,
                    d2: 0,
                    cmd: "/say hi".into(),
                },
            ],
        }),
    };
    let expected = Bytes::default()
        .byte(0xE8)
        .int(5)
        .byte(0)
        .byte(3)
        .byte(1)
        .int(1001)
        .text("Hello")
        .text("")
        .text("HI")
        .byte(7)
        .byte(2)
        // Commands are numbered from 1: number, type ordinal, d1, d2 (one byte), text.
        .byte(1)
        .byte(1)
        .int(1177)
        .byte(2)
        .text("")
        .byte(2)
        .byte(3)
        .int(0)
        .byte(0)
        .text("/say hi")
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn ex_quest_item_list_layout() {
    let item = ItemInfo::plain(0x1000_0001, 5588, 1);
    let packet = ExQuestItemList {
        items: vec![item.clone()],
        block: Some(InventoryBlock {
            mode: 1,
            items: vec![57],
        }),
    };
    // 0xFE, ex id 0x00C6; u16 count; the item; the inventory block (count, mode, ids).
    let expected = Bytes::default()
        .byte(0xFE)
        .short(0xC6)
        .short(1)
        .int(0x1000_0001)
        .int(5588)
        .int(0)
        .long(1)
        .short(0)
        .short(0)
        .short(0)
        .int(0)
        .short(0)
        .short(0)
        .int(0)
        .int(-1)
        .int(-9999)
        .short(0xFFFE)
        .short(0)
        .short(0)
        .short(0)
        .short(0)
        .short(0)
        .short(0)
        .short(0)
        .short(0)
        .short(0)
        .short(0)
        .short(1)
        .byte(1)
        .int(57)
        .done();
    check_round_trip(&packet, &expected, 3);
    assert_eq!(
        ExQuestItemList::default().to_bytes(),
        Bytes::default().byte(0xFE).short(0xC6).short(0).short(0).done()
    );
}

#[test]
fn ex_rotation_layout() {
    let packet = ExRotation {
        char_id: 0x1000_00AE,
        heading: 20_000,
    };
    let expected = Bytes::default()
        .byte(0xFE)
        .short(0xC1)
        .int(0x1000_00AE)
        .int(20_000)
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn contact_list_layout() {
    let packet = ExShowContactList {
        contacts: vec!["Ann".into(), "Bo".into()],
    };
    let expected = Bytes::default()
        .byte(0xFE)
        .short(0xD3)
        .int(2)
        .text("Ann")
        .text("Bo")
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn action_failed_is_one_byte() {
    check_round_trip(&ActionFailed, &[0x1F], 1);
}

#[test]
fn ex_br_extra_user_info_layout() {
    let packet = ExBrExtraUserInfo {
        object_id: 7,
        abnormal_visual_effects_event: 0,
        lecture_mark: 1,
    };
    let expected = Bytes::default()
        .byte(0xFE)
        .short(0xDA)
        .int(7)
        .int(0)
        .byte(1)
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn nevit_packets_layout() {
    check_round_trip(
        &ExNevitAdventPointInfo { points: 2000 },
        &Bytes::default().byte(0xFE).short(0xDF).int(2000).done(),
        3,
    );
    // Java: the byte is `!paused`, the time is capped at 14400.
    check_round_trip(
        &ExNevitAdventTimeChange::new(30, true),
        &Bytes::default().byte(0xFE).short(0xE1).byte(0).int(30).done(),
        3,
    );
    assert_eq!(ExNevitAdventTimeChange::new(99_999, false).time, 14_400);
    check_round_trip(
        &ExNevitAdventTimeChange::new(99_999, false),
        &Bytes::default().byte(0xFE).short(0xE1).byte(1).int(14_400).done(),
        3,
    );
}

#[test]
fn nevit_effect_and_vitality_packets_layout() {
    // Java ExNevitAdventEffect: EX_NAVIT_ADVENT_EFFECT (0xFE 0xE0), the seconds left.
    check_round_trip(
        &ExNevitAdventEffect { time_left: 180 },
        &Bytes::default().byte(0xFE).short(0xE0).int(180).done(),
        3,
    );
    check_round_trip(
        &ExNevitAdventEffect { time_left: 0 },
        &Bytes::default().byte(0xFE).short(0xE0).int(0).done(),
        3,
    );
    // Java ExVitalityPointInfo: EX_VITALITY_POINT_INFO (0xFE 0xA0), the points.
    check_round_trip(
        &ExVitalityPointInfo { points: 20_000 },
        &Bytes::default().byte(0xFE).short(0xA0).int(20_000).done(),
        3,
    );
}

#[test]
fn request_skill_cool_time_is_accepted_in_game_only() {
    for &state in ConnectionState::ALL {
        let decoded = decode_client(state, 0xA6, &mut PacketReader::new(&[])).unwrap();
        if state == ConnectionState::InGame {
            assert_eq!(
                decoded,
                Some(ClientPacket::RequestSkillCoolTime(RequestSkillCoolTime))
            );
        } else {
            assert_eq!(decoded, None, "{state:?}");
        }
    }
    assert_eq!(RequestSkillCoolTime.to_bytes(), [0xA6]);
}

// ---------------------------------------------------------------------------------------------
// Capture-driven check against the Java sessions.
// ---------------------------------------------------------------------------------------------

fn check_server(opcode: u8, raw: &[u8]) -> Option<Result<bool, ()>> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0xE8 => MacroList::decode(body).map(|p| p.to_bytes()),
        0x1F => ActionFailed::decode(body).map(|p| p.to_bytes()),
        _ => return None,
    };
    Some(encoded.map(|bytes| bytes == raw).map_err(|_| ()))
}

fn check_server_ex(ex: u16, raw: &[u8]) -> Option<Result<bool, ()>> {
    let body = &mut PacketReader::new(&raw[3..]);
    let encoded = match ex {
        0xC6 => ExQuestItemList::decode(body).map(|p| p.to_bytes()),
        0xC1 => ExRotation::decode(body).map(|p| p.to_bytes()),
        0xD3 => ExShowContactList::decode(body).map(|p| p.to_bytes()),
        0xDA => ExBrExtraUserInfo::decode(body).map(|p| p.to_bytes()),
        0xDF => ExNevitAdventPointInfo::decode(body).map(|p| p.to_bytes()),
        0xE1 => ExNevitAdventTimeChange::decode(body).map(|p| p.to_bytes()),
        _ => return None,
    };
    Some(encoded.map(|bytes| bytes == raw).map_err(|_| ()))
}

/// Failure messages name the file, line and packet only, never the bytes: captures contain
/// account and character data.
#[test]
fn packets_match_recorded_java_sessions() {
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
            let (Some(opcode), Some(raw)) = (record["opcode"].as_u64(), record["raw"].as_str()) else {
                continue;
            };
            let Ok(raw) = hex::decode(raw) else { continue };
            let ex = record["ex_opcode"].as_u64();
            let result = match ex {
                None if raw.first() == Some(&(opcode as u8)) => check_server(opcode as u8, &raw),
                Some(ex) if raw.len() >= 3 && raw[0] == 0xFE => check_server_ex(ex as u16, &raw),
                _ => None,
            };
            match result {
                Some(Ok(true)) => compared += 1,
                Some(Ok(false)) | Some(Err(())) => mismatches.push(format!(
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
    assert!(compared > 0, "no matching packets found in {}", dir.display());
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
}
