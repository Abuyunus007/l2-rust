//! The fishing packets (M6-FU-08) against byte layouts worked out by hand from the Java
//! `readImpl` / `writeImpl` code, plus round trips and the dispatcher.
//!
//! No recorded Java session holds a cast yet, so the layouts below are the spec. The expected
//! bytes are built with an independent little-endian builder (not the crate's `PacketWriter`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::RequestExFishRanking;
use l2_protocol_game::server::{ExFishingEnd, ExFishingHpRegen, ExFishingStart, ExFishingStartCombat};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

/// Java `writeInt`.
fn int(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// Encodes, compares with `expected`, decodes the bytes after the 3 opcode bytes and expects the
/// same packet back.
fn check<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P, expected: &[u8]) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected, "encoded bytes");
    let decoded = P::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
}

/// `0xFE` and the u16 ex id of `ServerPackets` (`EX_FISHING_*`).
fn ex(id: u16) -> Vec<u8> {
    let mut out = vec![0xFE];
    out.extend(id.to_le_bytes());
    out
}

#[test]
fn ex_fishing_start() {
    // EX_FISHING_START (0xFE, 0x1E): objectId, fishType, x, y, z, nightLure, 0.
    let packet = ExFishingStart {
        object_id: 0x6000_0001,
        fish_type: 5,
        x: -83_000,
        y: 150_000,
        z: -3_100,
        night_lure: true,
    };
    let mut expected = ex(0x1E);
    for v in [0x6000_0001, 5, -83_000, 150_000, -3_100] {
        expected.extend(int(v));
    }
    expected.extend([1, 0]);
    check(&packet, &expected);

    // A day lure and a fish that will not bite (`setFishGroup(-1)`).
    let packet = ExFishingStart {
        night_lure: false,
        fish_type: -1,
        ..packet
    };
    let mut expected = ex(0x1E);
    for v in [0x6000_0001, -1, -83_000, 150_000, -3_100] {
        expected.extend(int(v));
    }
    expected.extend([0, 0]);
    check(&packet, &expected);
}

#[test]
fn ex_fishing_start_combat() {
    // EX_FISHING_START_COMBAT (0xFE, 0x27): objectId, time, hp, mode, lureType, deceptiveMode.
    let packet = ExFishingStartCombat {
        object_id: 0x6000_0001,
        time: 24,
        hp: 100,
        mode: 1,
        lure_type: 2,
        deceptive_mode: 1,
    };
    let mut expected = ex(0x27);
    for v in [0x6000_0001, 24, 100] {
        expected.extend(int(v));
    }
    expected.extend([1, 2, 1]);
    check(&packet, &expected);
}

#[test]
fn ex_fishing_hp_regen() {
    // EX_FISHING_HP_REGEN (0xFE, 0x28): objectId, time, fishHp, hpMode, goodUse, anim, penalty,
    // hpBarColor.
    let packet = ExFishingHpRegen {
        object_id: 0x6000_0001,
        time: 23,
        fish_hp: 88,
        hp_mode: 1,
        good_use: 2,
        anim: 1,
        penalty: 4,
        hp_bar_color: 1,
    };
    let mut expected = ex(0x28);
    for v in [0x6000_0001, 23, 88] {
        expected.extend(int(v));
    }
    expected.extend([1, 2, 1]);
    expected.extend(int(4));
    expected.push(1);
    check(&packet, &expected);
}

#[test]
fn ex_fishing_end() {
    // EX_FISHING_END (0xFE, 0x1F): objectId, win.
    for (win, byte) in [(false, 0), (true, 1)] {
        let packet = ExFishingEnd {
            object_id: 0x6000_0001,
            win,
        };
        let mut expected = ex(0x1F);
        expected.extend(int(0x6000_0001));
        expected.push(byte);
        check(&packet, &expected);
    }
}

#[test]
fn request_ex_fish_ranking() {
    // 0xD0 0x0018, no fields.
    check(&RequestExFishRanking, &[0xD0, 0x18, 0x00]);
}

#[test]
fn the_dispatcher_decodes_the_ranking_request_in_game_only() {
    let bytes = RequestExFishRanking.to_bytes();
    let decode = |state| decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..])).unwrap();
    assert_eq!(
        decode(ConnectionState::InGame),
        Some(ClientPacket::RequestExFishRanking(RequestExFishRanking))
    );
    assert_eq!(decode(ConnectionState::Connected), None);
}
