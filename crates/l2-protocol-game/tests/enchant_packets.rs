//! The enchant and element attribute packets (M6-ENCHANT-01) against byte layouts worked out by
//! hand from the Java `readImpl` / `writeImpl` code, plus round trips and the dispatcher.
//!
//! No recorded Java session holds an enchant yet, so the layouts below are the spec. The expected
//! bytes are built with an independent little-endian builder (not the crate's `PacketWriter`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    RequestEnchantItem, RequestExCancelEnchantItem, RequestExEnchantItemAttribute,
    RequestExRemoveItemAttribute, RequestExTryToPutEnchantSupportItem, RequestExTryToPutEnchantTargetItem,
};
use l2_protocol_game::server::{
    ChooseInventoryItem, EnchantResult, ExAttributeEnchantResult, ExBaseAttributeCancelResult,
    ExChooseInventoryAttributeItem, ExPutEnchantSupportItemResult, ExPutEnchantTargetItemResult,
    ExShowBaseAttributeCancelWindow,
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

#[test]
fn request_enchant_item() {
    // 0x5F, readInt (the support id is commented out in Java).
    let mut expected = vec![0x5F];
    expected.extend(int(0x1000_0042));
    check(
        &RequestEnchantItem {
            object_id: 0x1000_0042,
        },
        &expected,
        1,
    );
}

#[test]
fn request_enchant_item_ignores_the_padding_of_the_client() {
    // Java reads one int; the client may send the support id too.
    let mut body = int(7);
    body.extend(int(99));
    let decoded = RequestEnchantItem::decode(&mut PacketReader::new(&body)).unwrap();
    assert_eq!(decoded.object_id, 7);
}

#[test]
fn request_ex_try_to_put_enchant_target_item() {
    // 0xD0 0x004C, readInt.
    let mut expected = client_ex(0x4C);
    expected.extend(int(5));
    check(&RequestExTryToPutEnchantTargetItem { object_id: 5 }, &expected, 3);
}

#[test]
fn request_ex_try_to_put_enchant_support_item() {
    // 0xD0 0x004D, supportObjectId, enchantObjectId.
    let packet = RequestExTryToPutEnchantSupportItem {
        support_object_id: 11,
        enchant_object_id: 22,
    };
    let mut expected = client_ex(0x4D);
    expected.extend(int(11));
    expected.extend(int(22));
    check(&packet, &expected, 3);
}

#[test]
fn request_ex_cancel_enchant_item() {
    check(&RequestExCancelEnchantItem, &client_ex(0x4E), 3);
}

#[test]
fn request_ex_enchant_item_attribute() {
    // 0xD0 0x0035, readInt; Java compares it with 0xFFFFFFFF (-1 as an int) for "cancelled".
    let mut expected = client_ex(0x35);
    expected.extend(int(-1));
    check(&RequestExEnchantItemAttribute { object_id: -1 }, &expected, 3);
}

#[test]
fn request_ex_remove_item_attribute() {
    // 0xD0 0x0023, objectId, element (an int that Java casts to a byte).
    let packet = RequestExRemoveItemAttribute {
        object_id: 9,
        element: 3,
    };
    let mut expected = client_ex(0x23);
    expected.extend(int(9));
    expected.extend(int(3));
    check(&packet, &expected, 3);
}

#[test]
fn choose_inventory_item() {
    // 0x7C, itemId.
    let mut expected = vec![0x7C];
    expected.extend(int(729));
    check(&ChooseInventoryItem { item_id: 729 }, &expected, 1);
}

#[test]
fn enchant_result_writes_the_count_as_a_long() {
    // 0x87, result, crystal, count (an int field written with writeLong).
    let packet = EnchantResult {
        result: 1,
        crystal: 1460,
        count: 7,
    };
    let mut expected = vec![0x87];
    expected.extend(int(1));
    expected.extend(int(1460));
    expected.extend(long(7));
    check(&packet, &expected, 1);

    // The "error" answer of every refusal.
    let mut expected = vec![0x87];
    expected.extend(int(2));
    expected.extend(int(0));
    expected.extend(long(0));
    check(
        &EnchantResult {
            result: 2,
            crystal: 0,
            count: 0,
        },
        &expected,
        1,
    );
}

#[test]
fn put_enchant_item_results() {
    let mut expected = ex(0x81);
    expected.extend(int(0x1000_0042));
    check(
        &ExPutEnchantTargetItemResult { result: 0x1000_0042 },
        &expected,
        3,
    );

    let mut expected = ex(0x82);
    expected.extend(int(0));
    check(&ExPutEnchantSupportItemResult { result: 0 }, &expected, 3);
}

#[test]
fn ex_attribute_enchant_result() {
    let mut expected = ex(0x61);
    expected.extend(int(20));
    check(&ExAttributeEnchantResult { result: 20 }, &expected, 3);
}

#[test]
fn ex_base_attribute_cancel_result_starts_with_a_one() {
    // 0xFE 0x0075: the constant 1, objId, attribute.
    let packet = ExBaseAttributeCancelResult {
        object_id: 77,
        attribute: 4,
    };
    let mut expected = ex(0x75);
    expected.extend(int(1));
    expected.extend(int(77));
    expected.extend(int(4));
    check(&packet, &expected, 3);
}

#[test]
fn ex_choose_inventory_attribute_item_flags_the_stones_element() {
    // 0xFE 0x0062: itemId, fire, water, wind, earth, holy, dark (1 for the stone's element), the
    // stone's highest level.
    let packet = ExChooseInventoryAttributeItem::new(9549, 2, 3);
    assert_eq!(packet.elements, [false, false, true, false, false, false]);
    let mut expected = ex(0x62);
    expected.extend(int(9549));
    for flag in [0, 0, 1, 0, 0, 0] {
        expected.extend(int(flag));
    }
    expected.extend(int(3));
    check(&packet, &expected, 3);

    // Dark is the last flag.
    let dark = ExChooseInventoryAttributeItem::new(9550, 5, 3);
    assert_eq!(dark.elements, [false, false, false, false, false, true]);
}

#[test]
fn ex_show_base_attribute_cancel_window_lists_object_ids_and_prices() {
    // 0xFE 0x0074: the count, then objectId (int) and price (long) each.
    let packet = ExShowBaseAttributeCancelWindow {
        items: vec![(100, 50_000), (101, 160_000)],
    };
    let mut expected = ex(0x74);
    expected.extend(int(2));
    expected.extend(int(100));
    expected.extend(long(50_000));
    expected.extend(int(101));
    expected.extend(long(160_000));
    check(&packet, &expected, 3);

    // An empty window is just the count.
    let mut expected = ex(0x74);
    expected.extend(int(0));
    check(&ExShowBaseAttributeCancelWindow::default(), &expected, 3);
}

#[test]
fn the_dispatcher_decodes_the_enchant_packets_in_game_only() {
    let enchant = RequestEnchantItem { object_id: 3 };
    let target = RequestExTryToPutEnchantTargetItem { object_id: 4 };
    let support = RequestExTryToPutEnchantSupportItem {
        support_object_id: 5,
        enchant_object_id: 6,
    };
    let attribute = RequestExEnchantItemAttribute { object_id: 7 };
    let remove = RequestExRemoveItemAttribute {
        object_id: 8,
        element: 1,
    };
    let cases = [
        (enchant.to_bytes(), ClientPacket::RequestEnchantItem(enchant)),
        (
            target.to_bytes(),
            ClientPacket::RequestExTryToPutEnchantTargetItem(target),
        ),
        (
            support.to_bytes(),
            ClientPacket::RequestExTryToPutEnchantSupportItem(support),
        ),
        (
            RequestExCancelEnchantItem.to_bytes(),
            ClientPacket::RequestExCancelEnchantItem(RequestExCancelEnchantItem),
        ),
        (
            attribute.to_bytes(),
            ClientPacket::RequestExEnchantItemAttribute(attribute),
        ),
        (
            remove.to_bytes(),
            ClientPacket::RequestExRemoveItemAttribute(remove),
        ),
    ];
    for (bytes, expected) in cases {
        let decode = |state| decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decode(ConnectionState::InGame), Some(expected));
        assert_eq!(decode(ConnectionState::Connected), None);
    }
}
