//! L-02 packets (macros, crystallize, the wear shop, item links, key mapping, inventory order,
//! rotating, a skill on the ground and the small requests) against byte layouts worked out by
//! hand from the Java `readImpl` / `writeImpl` code, plus round trips and the dispatcher.
//!
//! No recorded Java session holds any of these packets, so there is no fixture comparison: the
//! layouts below are the spec. The expected bytes are built with an independent little-endian
//! builder (not the crate's `PacketWriter`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::*;
use l2_protocol_game::server::{ExRpItemLink, ShopPreviewInfo};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

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
    /// Java `writeString`: UTF-16LE, zero terminated.
    fn string(mut self, text: &str) -> Self {
        for unit in text.encode_utf16() {
            self.0.extend(unit.to_le_bytes());
        }
        self.0.extend([0, 0]);
        self
    }
    fn raw(mut self, bytes: &[u8]) -> Self {
        self.0.extend(bytes);
        self
    }
    fn done(self) -> Vec<u8> {
        self.0
    }
}

/// Encodes, checks the bytes, decodes the bytes after `opcode_len` and expects the packet back,
/// then lets the dispatcher turn the bytes into `expected` in game.
fn check<P: Encode + Decode + PartialEq + std::fmt::Debug + Clone>(
    packet: &P,
    expected_bytes: &[u8],
    opcode_len: usize,
    expected: ClientPacket,
) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected_bytes, "encoded bytes");
    let decoded = P::decode(&mut PacketReader::new(&bytes[opcode_len..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
    let dispatched = decode_client(
        ConnectionState::InGame,
        bytes[0],
        &mut PacketReader::new(&bytes[1..]),
    )
    .unwrap();
    assert_eq!(dispatched, Some(expected), "dispatch");
    // Java checks the state before it reads a field: nothing in the character screen.
    assert_eq!(
        decode_client(
            ConnectionState::Connected,
            bytes[0],
            &mut PacketReader::new(&bytes[1..])
        )
        .unwrap(),
        None
    );
}

#[test]
fn request_make_macro() {
    let packet = RequestMakeMacro {
        id: 0,
        name: "Heal".into(),
        descr: "d".into(),
        acronym: "HL".into(),
        icon: 200,
        commands: vec![
            MacroCommand {
                entry: 1,
                kind: 1,
                d1: 1011,
                d2: 0,
                command: String::new(),
            },
            MacroCommand {
                entry: 2,
                kind: 3,
                d1: 0,
                d2: 255,
                command: "/sit".into(),
            },
        ],
    };
    let expected = Bytes::default()
        .byte(0xCD)
        .int(0)
        .string("Heal")
        .string("d")
        .string("HL")
        .byte(200)
        .byte(2)
        .byte(1)
        .byte(1)
        .int(1011)
        .byte(0)
        .string("")
        .byte(2)
        .byte(3)
        .int(0)
        .byte(255)
        .string("/sit")
        .done();
    check(
        &packet,
        &expected,
        1,
        ClientPacket::RequestMakeMacro(packet.clone()),
    );
}

/// Java keeps at most 12 commands (`MAX_MACRO_LENGTH`) and does not read the rest.
#[test]
fn request_make_macro_keeps_twelve_commands() {
    let mut w = Bytes::default()
        .int(5)
        .string("n")
        .string("")
        .string("")
        .byte(0)
        .byte(20);
    for i in 0..20 {
        w = w.byte(i).byte(1).int(i32::from(i)).byte(0).string("");
    }
    let bytes = w.done();
    let decoded = RequestMakeMacro::decode(&mut PacketReader::new(&bytes)).unwrap();
    assert_eq!(decoded.commands.len(), 12);
    assert_eq!(decoded.commands[11].d1, 11);
}

#[test]
fn request_delete_macro() {
    let packet = RequestDeleteMacro { id: 1001 };
    let expected = Bytes::default().byte(0xCE).int(1001).done();
    check(
        &packet,
        &expected,
        1,
        ClientPacket::RequestDeleteMacro(packet.clone()),
    );
}

#[test]
fn request_crystallize_item() {
    let packet = RequestCrystallizeItem {
        object_id: 268_500_001,
        count: 1,
    };
    let expected = Bytes::default().byte(0x2F).int(268_500_001).long(1).done();
    check(
        &packet,
        &expected,
        1,
        ClientPacket::RequestCrystallizeItem(packet.clone()),
    );
}

#[test]
fn request_preview_item() {
    let packet = RequestPreviewItem {
        unknown: 7,
        list_id: 300_011,
        items: Some(vec![2376, 2380]),
    };
    let expected = Bytes::default()
        .byte(0xC7)
        .int(7)
        .int(300_011)
        .int(2)
        .int(2376)
        .int(2380)
        .done();
    check(
        &packet,
        &expected,
        1,
        ClientPacket::RequestPreviewItem(packet.clone()),
    );
}

/// A count over 100 leaves Java's list null; a negative one is 0.
#[test]
fn request_preview_item_list_limits() {
    let long = Bytes::default().int(0).int(1).int(101).done();
    let decoded = RequestPreviewItem::decode(&mut PacketReader::new(&long)).unwrap();
    assert_eq!(decoded.items, None);
    let negative = Bytes::default().int(0).int(1).int(-4).done();
    let decoded = RequestPreviewItem::decode(&mut PacketReader::new(&negative)).unwrap();
    assert_eq!(decoded.items, Some(vec![]));
}

#[test]
fn request_ex_rq_item_link() {
    let packet = RequestExRqItemLink { object_id: 42 };
    let expected = Bytes::default().byte(0xD0).short(0x1E).int(42).done();
    check(
        &packet,
        &expected,
        3,
        ClientPacket::RequestExRqItemLink(packet.clone()),
    );
}

#[test]
fn request_save_key_mapping() {
    let packet = RequestSaveKeyMapping {
        ui_key_mapping: Some(vec![1, 0xFF, 3]),
    };
    let expected = Bytes::default()
        .byte(0xD0)
        .short(0x22)
        .int(3)
        .raw(&[1, 0xFF, 3])
        .done();
    check(
        &packet,
        &expected,
        3,
        ClientPacket::RequestSaveKeyMapping(packet.clone()),
    );
    // No bytes: Java keeps no array.
    let empty = RequestSaveKeyMapping { ui_key_mapping: None };
    let expected = Bytes::default().byte(0xD0).short(0x22).int(0).done();
    check(
        &empty,
        &expected,
        3,
        ClientPacket::RequestSaveKeyMapping(empty.clone()),
    );
    // A size past the packet is an error (`readBytes` throws).
    let short = Bytes::default().int(10).raw(&[1, 2]).done();
    assert!(RequestSaveKeyMapping::decode(&mut PacketReader::new(&short)).is_err());
}

#[test]
fn request_save_inventory_order() {
    let packet = RequestSaveInventoryOrder {
        order: vec![(100, 0), (101, 5)],
    };
    let expected = Bytes::default()
        .byte(0xD0)
        .short(0x24)
        .int(2)
        .int(100)
        .int(0)
        .int(101)
        .int(5)
        .done();
    check(
        &packet,
        &expected,
        3,
        ClientPacket::RequestSaveInventoryOrder(packet.clone()),
    );
}

/// Java limits the list to 125 entries (`LIMIT`) and throws for a negative size.
#[test]
fn request_save_inventory_order_limits() {
    let mut w = Bytes::default().int(200);
    for i in 0..200 {
        w = w.int(i).int(i);
    }
    let decoded = RequestSaveInventoryOrder::decode(&mut PacketReader::new(&w.done())).unwrap();
    assert_eq!(decoded.order.len(), 125);
    let negative = Bytes::default().int(-1).done();
    assert!(RequestSaveInventoryOrder::decode(&mut PacketReader::new(&negative)).is_err());
}

#[test]
fn request_ex_magic_skill_use_ground() {
    let packet = RequestExMagicSkillUseGround {
        x: -71_000,
        y: 258_000,
        z: -3_100,
        skill_id: 452,
        ctrl_pressed: true,
        shift_pressed: false,
    };
    let expected = Bytes::default()
        .byte(0xD0)
        .short(0x44)
        .int(-71_000)
        .int(258_000)
        .int(-3_100)
        .int(452)
        .int(1)
        .byte(0)
        .done();
    check(
        &packet,
        &expected,
        3,
        ClientPacket::RequestExMagicSkillUseGround(packet),
    );
}

#[test]
fn the_small_requests() {
    let p = RequestShowMiniMap;
    check(&p, &[0x6C], 1, ClientPacket::RequestShowMiniMap(p));
    let p = RequestRecordInfo;
    check(&p, &[0x6E], 1, ClientPacket::RequestRecordInfo(p));
    let p = RequestSkillList;
    check(&p, &[0x50], 1, ClientPacket::RequestSkillList(p));
    let p = MoveWithDelta { dx: 1, dy: -2, dz: 3 };
    let expected = Bytes::default().byte(0x52).int(1).int(-2).int(3).done();
    check(&p, &expected, 1, ClientPacket::MoveWithDelta(p.clone()));
    let p = StartRotating {
        degree: 16384,
        side: 1,
    };
    let expected = Bytes::default().byte(0x5B).int(16384).int(1).done();
    check(&p, &expected, 1, ClientPacket::StartRotating(p.clone()));
    let p = FinishRotating {
        degree: 32768,
        unknown: 0,
    };
    let expected = Bytes::default().byte(0x5C).int(32768).int(0).done();
    check(&p, &expected, 1, ClientPacket::FinishRotating(p.clone()));
}

#[test]
fn shop_preview_info() {
    // CHEST (6) 2376 and RHAND (5) 1: positions 10, 7 and 14 of the 19 Java writes.
    let packet = ShopPreviewInfo::from_slots(&[(6, 2376), (5, 1)]);
    let mut w = Bytes::default().byte(0xF6).int(25);
    for position in 0..19 {
        let id = match position {
            10 => 2376,
            7 | 14 => 1,
            _ => 0,
        };
        w = w.int(id);
    }
    assert_eq!(packet.to_bytes(), w.done());
}

#[test]
fn ex_rp_item_link() {
    let mut item = l2_protocol_game::server::items::ItemInfo::plain(268_500_001, 57, 5);
    item.location = 3;
    item.type2 = 4;
    item.enchant = 2;
    let packet = ExRpItemLink { item };
    let expected = Bytes::default()
        .byte(0xFE)
        .short(0x6C)
        .int(268_500_001) // object id
        .int(57) // display id
        .int(3) // location slot
        .long(5) // count
        .short(4) // type 2
        .short(0) // custom type 1
        .short(0) // equipped
        .int(0) // body part
        .short(2) // enchant
        .short(0) // custom type 2
        .int(0) // augmentation
        .int(-1) // mana
        .int(-9999) // time
        .short(-2) // attack element
        .short(0)
        .raw(&[0; 12]) // six defense elements
        .raw(&[0; 6]) // three enchant options
        .done();
    assert_eq!(packet.to_bytes(), expected);
}
