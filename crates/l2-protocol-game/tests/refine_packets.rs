//! The augmentation packets (M6-AUGMENT-01) against byte layouts worked out by hand from the Java
//! `readImpl` / `writeImpl` code, plus round trips and the dispatcher.
//!
//! No recorded Java session holds an augmentation yet, so the layouts below are the spec. The
//! expected bytes are built with an independent little-endian builder (not the crate's
//! `PacketWriter`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    RequestConfirmCancelItem, RequestConfirmGemStone, RequestConfirmRefinerItem, RequestConfirmTargetItem,
    RequestRefine, RequestRefineCancel,
};
use l2_protocol_game::server::{
    ExPutCommissionResultForVariationMake, ExPutIntensiveResultForVariationMake,
    ExPutItemResultForVariationCancel, ExPutItemResultForVariationMake, ExShowVariationCancelWindow,
    ExShowVariationMakeWindow, ExVariationCancelResult, ExVariationResult,
};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

/// Java `writeInt`.
fn int(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// Java `writeLong`.
fn long(v: i64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// Encodes, compares with `expected`, decodes the bytes after `opcode_len` and expects the same
/// packet back.
fn check<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P, expected: &[u8], opcode_len: usize) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected, "encoded bytes");
    let decoded = P::decode(&mut PacketReader::new(&bytes[opcode_len..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
}

/// The id byte and the u16 ex id of a client packet (`ExClientPackets`): `0xD0`, then the id.
fn client_ex(id: u16) -> Vec<u8> {
    let mut out = vec![0xD0];
    out.extend(id.to_le_bytes());
    out
}

/// The id byte and the u16 ex id of a server ex packet (`ServerPackets`).
fn ex(id: u16) -> Vec<u8> {
    let mut out = vec![0xFE];
    out.extend(id.to_le_bytes());
    out
}

fn build(prefix: Vec<u8>, parts: &[Vec<u8>]) -> Vec<u8> {
    let mut out = prefix;
    for part in parts {
        out.extend(part);
    }
    out
}

#[test]
fn request_confirm_target_item() {
    // 0xD0 0x0026, readInt.
    check(
        &RequestConfirmTargetItem {
            object_id: 0x1000_0042,
        },
        &build(client_ex(0x26), &[int(0x1000_0042)]),
        3,
    );
}

#[test]
fn request_confirm_refiner_item() {
    // 0xD0 0x0027, readInt, readInt.
    check(
        &RequestConfirmRefinerItem {
            target_item_object_id: 11,
            refiner_item_object_id: 12,
        },
        &build(client_ex(0x27), &[int(11), int(12)]),
        3,
    );
}

#[test]
fn request_confirm_gem_stone() {
    // 0xD0 0x0028, readInt x3, readLong.
    check(
        &RequestConfirmGemStone {
            target_item_object_id: 11,
            refiner_item_object_id: 12,
            gemstone_item_object_id: 13,
            gemstone_count: 25,
        },
        &build(client_ex(0x28), &[int(11), int(12), int(13), long(25)]),
        3,
    );
}

#[test]
fn request_refine() {
    // 0xD0 0x0041, readInt x3, readLong.
    check(
        &RequestRefine {
            target_item_object_id: 21,
            refiner_item_object_id: 22,
            gemstone_item_object_id: 23,
            gemstone_count: 360,
        },
        &build(client_ex(0x41), &[int(21), int(22), int(23), long(360)]),
        3,
    );
}

#[test]
fn request_confirm_cancel_item_and_refine_cancel() {
    // 0xD0 0x0042 and 0xD0 0x0043, readInt each.
    check(
        &RequestConfirmCancelItem { object_id: 31 },
        &build(client_ex(0x42), &[int(31)]),
        3,
    );
    check(
        &RequestRefineCancel {
            target_item_object_id: 32,
        },
        &build(client_ex(0x43), &[int(32)]),
        3,
    );
}

#[test]
fn a_short_packet_is_an_error_and_padding_is_ignored() {
    assert!(RequestRefine::decode(&mut PacketReader::new(&int(1))).is_err());
    let mut body = int(7);
    body.extend(int(99));
    let decoded = RequestConfirmTargetItem::decode(&mut PacketReader::new(&body)).unwrap();
    assert_eq!(decoded.object_id, 7);
}

#[test]
fn the_variation_windows_are_just_the_ids() {
    check(&ExShowVariationMakeWindow, &ex(0x51), 3);
    check(&ExShowVariationCancelWindow, &ex(0x52), 3);
}

#[test]
fn ex_put_item_result_for_variation_make() {
    // 0xFE 0x0053: objectId, itemId, 1.
    check(
        &ExPutItemResultForVariationMake {
            item_object_id: 100,
            item_id: 2500,
        },
        &build(ex(0x53), &[int(100), int(2500), int(1)]),
        3,
    );
}

#[test]
fn ex_put_intensive_result_for_variation_make() {
    // 0xFE 0x0054: refiner objectId, life stone id, gemstone id, gemstone count (writeLong), 1.
    check(
        &ExPutIntensiveResultForVariationMake {
            refiner_item_object_id: 101,
            life_stone_id: 8733,
            gemstone_item_id: 2130,
            gemstone_count: 20,
        },
        &build(ex(0x54), &[int(101), int(8733), int(2130), long(20), int(1)]),
        3,
    );
}

#[test]
fn ex_put_commission_result_for_variation_make() {
    // 0xFE 0x0055: gemstone objectId, item id, count (writeLong), 0, 0, 1.
    check(
        &ExPutCommissionResultForVariationMake {
            gemstone_object_id: 102,
            item_id: 2130,
            gemstone_count: 20,
        },
        &build(ex(0x55), &[int(102), int(2130), long(20), int(0), int(0), int(1)]),
        3,
    );
}

#[test]
fn ex_variation_result() {
    // 0xFE 0x0056: stat12, stat34, result.
    check(
        &ExVariationResult {
            stat12: 1234,
            stat34: 16345,
            result: 1,
        },
        &build(ex(0x56), &[int(1234), int(16345), int(1)]),
        3,
    );
    // A failure is all zeros.
    check(
        &ExVariationResult {
            stat12: 0,
            stat34: 0,
            result: 0,
        },
        &build(ex(0x56), &[int(0), int(0), int(0)]),
        3,
    );
}

#[test]
fn ex_put_item_result_for_variation_cancel() {
    // 0xFE 0x0057: objectId, displayId, aug1, aug2, price (writeLong), 1.
    check(
        &ExPutItemResultForVariationCancel {
            item_object_id: 103,
            item_id: 2500,
            aug1: 1234,
            aug2: 16345,
            price: 480_000,
        },
        &build(
            ex(0x57),
            &[int(103), int(2500), int(1234), int(16345), long(480_000), int(1)],
        ),
        3,
    );
}

#[test]
fn ex_variation_cancel_result() {
    // 0xFE 0x0058: result.
    check(
        &ExVariationCancelResult { result: 1 },
        &build(ex(0x58), &[int(1)]),
        3,
    );
}

#[test]
fn the_dispatcher_decodes_the_augmentation_packets_in_game_only() {
    let target = RequestConfirmTargetItem { object_id: 3 };
    let refiner = RequestConfirmRefinerItem {
        target_item_object_id: 4,
        refiner_item_object_id: 5,
    };
    let gem = RequestConfirmGemStone {
        target_item_object_id: 4,
        refiner_item_object_id: 5,
        gemstone_item_object_id: 6,
        gemstone_count: 20,
    };
    let refine = RequestRefine {
        target_item_object_id: 4,
        refiner_item_object_id: 5,
        gemstone_item_object_id: 6,
        gemstone_count: 20,
    };
    let confirm_cancel = RequestConfirmCancelItem { object_id: 7 };
    let cancel = RequestRefineCancel {
        target_item_object_id: 8,
    };
    let cases = [
        (target.to_bytes(), ClientPacket::RequestConfirmTargetItem(target)),
        (
            refiner.to_bytes(),
            ClientPacket::RequestConfirmRefinerItem(refiner),
        ),
        (gem.to_bytes(), ClientPacket::RequestConfirmGemStone(gem)),
        (refine.to_bytes(), ClientPacket::RequestRefine(refine)),
        (
            confirm_cancel.to_bytes(),
            ClientPacket::RequestConfirmCancelItem(confirm_cancel),
        ),
        (cancel.to_bytes(), ClientPacket::RequestRefineCancel(cancel)),
    ];
    for (bytes, expected) in cases {
        let decode = |state| decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decode(ConnectionState::InGame), Some(expected));
        assert_eq!(decode(ConnectionState::Connected), None);
    }
}
