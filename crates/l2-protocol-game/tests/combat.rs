//! Combat, death and item packets against byte layouts worked out by hand from the Java
//! `writeImpl` / `readImpl` code, round trips, and the recorded Java game server sessions
//! (`fixtures/captures`, or `L2_CAPTURE_DIR`).
//!
//! The expected bytes are built with an independent little-endian builder (not the crate's
//! `PacketWriter`), so a mistake in the crate's encoder cannot hide behind the same mistake here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    AttackRequest, RequestActionUse, RequestDestroyItem, RequestDropItem, RequestItemList,
    RequestRestartPoint, RequestUnEquipItem, UseItem,
};
use l2_protocol_game::server::{
    Attack, AttackHit, AutoAttackStart, AutoAttackStop, ChangeMoveType, ChangeWaitType, Die, DropItem,
    ExUseSharedGroupItem, GetItem, HITFLAG_CRIT, HITFLAG_MISS, HITFLAG_SHLD, HITFLAG_USESS, Revive,
    SpawnItem,
};

/// Independent little-endian builder.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn byte(mut self, v: u8) -> Self {
        self.0.push(v);
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

const ATTACKER: i32 = 0x1000_0001;
const TARGET: i32 = 0x1000_0002;
const PLAYER: i32 = 0x1000_0020;
const ITEM: i32 = 0x1000_0010;

// ---------------------------------------------------------------------------------------------
// Server packets.
// ---------------------------------------------------------------------------------------------

#[test]
fn hit_flags_are_the_java_bits() {
    assert_eq!(HITFLAG_USESS, 0x10);
    assert_eq!(HITFLAG_CRIT, 0x20);
    assert_eq!(HITFLAG_SHLD, 0x40);
    assert_eq!(HITFLAG_MISS, 0x80);
}

#[test]
fn attack_with_one_hit_layout() {
    // Java: attacker id, the first hit (target, damage, flags byte), attacker position, the short
    // 0 (no more hits), target position.
    let attack = Attack {
        attacker_id: ATTACKER,
        attacker_x: 10,
        attacker_y: 20,
        attacker_z: 30,
        first_hit: AttackHit {
            target_id: TARGET,
            damage: 45,
            flags: HITFLAG_CRIT | HITFLAG_USESS | 3,
        },
        other_hits: Vec::new(),
        target_x: 40,
        target_y: 50,
        target_z: 60,
    };
    let expected = Bytes::default()
        .byte(0x33)
        .int(ATTACKER)
        .int(TARGET)
        .int(45)
        .byte(0x33)
        .int(10)
        .int(20)
        .int(30)
        .byte(0)
        .byte(0)
        .int(40)
        .int(50)
        .int(60)
        .done();
    check_round_trip(&attack, &expected, 1);
}

#[test]
fn attack_with_two_hits_writes_the_extra_count_then_the_hit() {
    // Dual weapons: a second hit after the short count of extra hits (here 1).
    let mut attack = Attack {
        attacker_id: ATTACKER,
        attacker_x: 1,
        attacker_y: 2,
        attacker_z: 3,
        first_hit: AttackHit {
            target_id: TARGET,
            damage: 30,
            flags: 0,
        },
        other_hits: Vec::new(),
        target_x: 4,
        target_y: 5,
        target_z: 6,
    };
    attack.add_hit(AttackHit {
        target_id: TARGET,
        damage: 0,
        flags: HITFLAG_MISS,
    });
    let expected = Bytes::default()
        .byte(0x33)
        .int(ATTACKER)
        .int(TARGET)
        .int(30)
        .byte(0)
        .int(1)
        .int(2)
        .int(3)
        .byte(1)
        .byte(0)
        .int(TARGET)
        .int(0)
        .byte(HITFLAG_MISS)
        .int(4)
        .int(5)
        .int(6)
        .done();
    check_round_trip(&attack, &expected, 1);
}

#[test]
fn auto_attack_start_and_stop_layout() {
    let start = AutoAttackStart { target_id: TARGET };
    check_round_trip(&start, &Bytes::default().byte(0x25).int(TARGET).done(), 1);
    let stop = AutoAttackStop { target_id: TARGET };
    check_round_trip(&stop, &Bytes::default().byte(0x26).int(TARGET).done(), 1);
}

#[test]
fn die_writes_the_six_flags_in_java_order() {
    // Java: object id, canTeleport, hideout, castle, siege HQ, sweepable, fixed res, fortress.
    let die = Die {
        object_id: ATTACKER,
        can_teleport: true,
        to_clan_hall: true,
        to_castle: false,
        to_siege_hq: false,
        sweepable: true,
        fixed_res: false,
        to_fortress: true,
    };
    let expected = Bytes::default()
        .byte(0x00)
        .int(ATTACKER)
        .int(1)
        .int(1)
        .int(0)
        .int(0)
        .int(1)
        .int(0)
        .int(1)
        .done();
    check_round_trip(&die, &expected, 1);
}

#[test]
fn die_without_a_restart_point_only_shows_sweep_and_fixed_res() {
    let die = Die {
        object_id: ATTACKER,
        can_teleport: false,
        to_clan_hall: false,
        to_castle: false,
        to_siege_hq: false,
        sweepable: false,
        fixed_res: true,
        to_fortress: false,
    };
    let expected = Bytes::default()
        .byte(0x00)
        .int(ATTACKER)
        .int(0)
        .int(0)
        .int(0)
        .int(0)
        .int(0)
        .int(1)
        .int(0)
        .done();
    check_round_trip(&die, &expected, 1);
}

#[test]
fn revive_layout() {
    check_round_trip(
        &Revive { object_id: ATTACKER },
        &Bytes::default().byte(0x01).int(ATTACKER).done(),
        1,
    );
}

#[test]
fn change_move_type_writes_run_or_walk_and_a_zero() {
    let run = ChangeMoveType {
        object_id: 7,
        running: true,
    };
    check_round_trip(
        &run,
        &Bytes::default()
            .byte(0x28)
            .int(7)
            .int(ChangeMoveType::RUN)
            .int(0)
            .done(),
        1,
    );
    let walk = ChangeMoveType {
        object_id: 7,
        running: false,
    };
    check_round_trip(
        &walk,
        &Bytes::default()
            .byte(0x28)
            .int(7)
            .int(ChangeMoveType::WALK)
            .int(0)
            .done(),
        1,
    );
}

#[test]
fn change_wait_type_layout() {
    assert_eq!(ChangeWaitType::WT_SITTING, 0);
    assert_eq!(ChangeWaitType::WT_STANDING, 1);
    assert_eq!(ChangeWaitType::WT_START_FAKEDEATH, 2);
    assert_eq!(ChangeWaitType::WT_STOP_FAKEDEATH, 3);
    let sit = ChangeWaitType {
        object_id: 9,
        move_type: ChangeWaitType::WT_SITTING,
        x: 1,
        y: 2,
        z: 3,
    };
    let expected = Bytes::default()
        .byte(0x29)
        .int(9)
        .int(0)
        .int(1)
        .int(2)
        .int(3)
        .done();
    check_round_trip(&sit, &expected, 1);
}

#[test]
fn ex_use_shared_group_item_sends_seconds() {
    // Java: remaining time (a long, in ms) is cast to int after the division, so only the low 32
    // bits of the seconds reach the wire.
    let packet = ExUseSharedGroupItem::new(1061, 32, (1i64 << 32) * 1000 + 7000, 60_000);
    assert_eq!(packet.remaining_secs, 7);
    assert_eq!(packet.total_secs, 60);
    let expected = Bytes::default()
        .byte(0xFE)
        .byte(0x4A)
        .byte(0x00)
        .int(1061)
        .int(32)
        .int(7)
        .int(60)
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn spawn_item_layout_has_count_and_two_zero_ints() {
    let spawn = SpawnItem {
        object_id: ITEM,
        display_id: 57,
        x: 100,
        y: -200,
        z: 300,
        stackable: true,
        count: 5_000_000_000,
    };
    let expected = Bytes::default()
        .byte(0x05)
        .int(ITEM)
        .int(57)
        .int(100)
        .int(-200)
        .int(300)
        .int(1)
        .long(5_000_000_000)
        .int(0)
        .int(0)
        .done();
    assert_eq!(expected.len(), 41);
    check_round_trip(&spawn, &expected, 1);
}

#[test]
fn drop_item_layout_has_the_dropping_player_first() {
    let drop = DropItem {
        player_id: PLAYER,
        object_id: ITEM,
        display_id: 57,
        x: 1,
        y: 2,
        z: 3,
        stackable: false,
        count: 1,
    };
    let expected = Bytes::default()
        .byte(0x16)
        .int(PLAYER)
        .int(ITEM)
        .int(57)
        .int(1)
        .int(2)
        .int(3)
        .int(0)
        .long(1)
        .int(1)
        .done();
    assert_eq!(expected.len(), 41);
    check_round_trip(&drop, &expected, 1);
}

#[test]
fn get_item_layout() {
    let get = GetItem {
        player_id: PLAYER,
        object_id: ITEM,
        x: -1,
        y: 2,
        z: 3,
    };
    let expected = Bytes::default()
        .byte(0x17)
        .int(PLAYER)
        .int(ITEM)
        .int(-1)
        .int(2)
        .int(3)
        .done();
    check_round_trip(&get, &expected, 1);
}

// ---------------------------------------------------------------------------------------------
// Client packets.
// ---------------------------------------------------------------------------------------------

#[test]
fn attack_request_uses_the_two_java_ids() {
    // Java registers AttackRequest for ATTACK (0x01) and ATTACK_REQUEST (0x32).
    assert_eq!(AttackRequest::OPCODE.id(), 0x01);
    assert_eq!(AttackRequest::OPCODE_REQUEST.id(), 0x32);
    let shift = AttackRequest {
        object_id: TARGET,
        origin_x: 1,
        origin_y: 2,
        origin_z: 3,
        attack_id: AttackRequest::SHIFT_CLICK,
    };
    let expected = Bytes::default()
        .byte(0x01)
        .int(TARGET)
        .int(1)
        .int(2)
        .int(3)
        .byte(1)
        .done();
    check_round_trip(&shift, &expected, 1);
}

#[test]
fn attack_request_keeps_the_signed_attack_byte() {
    // Java `readByte()` is signed: 0xFF reads as -1, which is no click kind but still a packet.
    let odd = AttackRequest {
        object_id: TARGET,
        origin_x: 0,
        origin_y: 0,
        origin_z: 0,
        attack_id: -1,
    };
    let expected = Bytes::default()
        .byte(0x01)
        .int(TARGET)
        .int(0)
        .int(0)
        .int(0)
        .byte(0xFF)
        .done();
    check_round_trip(&odd, &expected, 1);
}

#[test]
fn request_action_use_layout_and_java_flag_rule() {
    let action = RequestActionUse {
        action_id: 15,
        ctrl_pressed: true,
        shift_pressed: false,
    };
    let expected = Bytes::default().byte(0x56).int(15).int(1).byte(0).done();
    check_round_trip(&action, &expected, 1);

    // Java reads both flags as `== 1`: a 2 in the ctrl int or the shift byte is "not pressed".
    let raw = Bytes::default().int(15).int(2).byte(1).done();
    let decoded = RequestActionUse::decode(&mut PacketReader::new(&raw)).unwrap();
    assert!(!decoded.ctrl_pressed);
    assert!(decoded.shift_pressed);
}

#[test]
fn request_restart_point_layout() {
    let restart = RequestRestartPoint {
        requested_point_type: 5,
    };
    check_round_trip(&restart, &Bytes::default().byte(0x7D).int(5).done(), 1);
}

#[test]
fn use_item_layout_and_java_ctrl_rule() {
    let use_item = UseItem {
        object_id: ITEM,
        ctrl_pressed: true,
    };
    check_round_trip(&use_item, &Bytes::default().byte(0x19).int(ITEM).int(1).done(), 1);

    // Java `!= 0` for the ctrl int, so any non-zero value means ctrl was pressed.
    let raw = Bytes::default().int(ITEM).int(2).done();
    let decoded = UseItem::decode(&mut PacketReader::new(&raw)).unwrap();
    assert!(decoded.ctrl_pressed);
}

#[test]
fn request_un_equip_item_layout() {
    let unequip = RequestUnEquipItem { paperdoll_slot: 7 };
    check_round_trip(&unequip, &Bytes::default().byte(0x16).int(7).done(), 1);
}

#[test]
fn request_drop_item_layout_keeps_the_eight_byte_count() {
    let drop = RequestDropItem {
        object_id: ITEM,
        count: 0x1_0000_0002,
        x: 1,
        y: 2,
        z: 3,
    };
    let expected = Bytes::default()
        .byte(0x17)
        .int(ITEM)
        .long(0x1_0000_0002)
        .int(1)
        .int(2)
        .int(3)
        .done();
    check_round_trip(&drop, &expected, 1);
}

#[test]
fn request_destroy_item_layout() {
    let destroy = RequestDestroyItem {
        object_id: ITEM,
        count: 3,
    };
    check_round_trip(&destroy, &Bytes::default().byte(0x60).int(ITEM).long(3).done(), 1);
}

#[test]
fn request_item_list_is_the_opcode_alone() {
    assert_eq!(RequestItemList.to_bytes(), [0x14]);
    let decoded = RequestItemList::decode(&mut PacketReader::new(&[])).unwrap();
    assert_eq!(decoded, RequestItemList);
}

#[test]
fn short_bodies_are_errors_and_padding_is_ignored() {
    // A body shorter than the fields is an error (the dispatcher drops the packet).
    assert!(AttackRequest::decode(&mut PacketReader::new(&[0; 16])).is_err());
    assert!(RequestDropItem::decode(&mut PacketReader::new(&[0; 20])).is_err());
    // The client may pad a packet: trailing bytes are not read.
    let padded = Bytes::default().int(ITEM).long(3).byte(0xAA).done();
    assert_eq!(
        RequestDestroyItem::decode(&mut PacketReader::new(&padded)).unwrap(),
        RequestDestroyItem {
            object_id: ITEM,
            count: 3
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Capture-driven check against the owner's Java sessions.
// ---------------------------------------------------------------------------------------------

/// Server packets of this file: the recorded bytes must re-encode exactly.
fn check_server(opcode: u8, raw: &[u8]) -> Option<bool> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0x33 => Attack::decode(body).ok()?.to_bytes(),
        0x25 => AutoAttackStart::decode(body).ok()?.to_bytes(),
        0x26 => AutoAttackStop::decode(body).ok()?.to_bytes(),
        0x00 => Die::decode(body).ok()?.to_bytes(),
        0x01 => Revive::decode(body).ok()?.to_bytes(),
        0x28 => ChangeMoveType::decode(body).ok()?.to_bytes(),
        0x29 => ChangeWaitType::decode(body).ok()?.to_bytes(),
        0x05 => SpawnItem::decode(body).ok()?.to_bytes(),
        0x16 => DropItem::decode(body).ok()?.to_bytes(),
        0x17 => GetItem::decode(body).ok()?.to_bytes(),
        _ => return None,
    };
    Some(encoded == raw)
}

/// Client packets of this file. The opcode is not compared (`0x01` and `0x32` both decode to
/// `AttackRequest`, which encodes `0x01`), only the body; the client may pad, so the recorded body
/// may be longer than ours.
fn check_client(opcode: u8, raw: &[u8]) -> Option<bool> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0x01 | 0x32 => AttackRequest::decode(body).ok()?.to_bytes(),
        0x56 => RequestActionUse::decode(body).ok()?.to_bytes(),
        0x7D => RequestRestartPoint::decode(body).ok()?.to_bytes(),
        0x19 => UseItem::decode(body).ok()?.to_bytes(),
        0x16 => RequestUnEquipItem::decode(body).ok()?.to_bytes(),
        0x17 => RequestDropItem::decode(body).ok()?.to_bytes(),
        0x60 => RequestDestroyItem::decode(body).ok()?.to_bytes(),
        0x14 => RequestItemList::decode(body).ok()?.to_bytes(),
        _ => return None,
    };
    Some(raw[1..].starts_with(&encoded[1..]))
}

/// Set `L2_CAPTURE_DIR` to a folder with `*.jsonl` captures (format: `l2-proxy` capture.rs).
/// Failure messages name the file, line and packet only, never the bytes: captures contain
/// account and character data.
#[test]
fn combat_packets_match_recorded_java_sessions() {
    // The owner's own recordings (L2_CAPTURE_DIR), else the scrubbed fixtures in the repo.
    let from_env = std::env::var_os("L2_CAPTURE_DIR");
    let dir = from_env
        .clone()
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
        "combat capture comparison: {compared} packets identical, {} different",
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
    // The committed fixtures hold recorded Attack, Die, item and move packets.
    if from_env.is_none() {
        assert!(compared > 0, "no combat packets found in the fixtures");
    }
}
