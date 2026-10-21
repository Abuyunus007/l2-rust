//! The multisell packets: `MultiSellChoose` (client) through the dispatcher, and the opcode of
//! `MultiSellList` (server). The layouts are Java's `readImpl` / `writeImpl`; the expected bytes
//! are built by hand, not with the crate's writer.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::MultiSellChoose;
use l2_protocol_game::server::{MultiSellEntry, MultiSellList};
use l2_protocol_game::{ClientOpcode, ClientPacket, ConnectionState, ServerOpcode, decode_client};

/// `readInt, readInt, readLong, readShort, readInt, readInt, 8 x readShort`.
fn java_bytes() -> Vec<u8> {
    let mut bytes = vec![0xB0];
    bytes.extend(1235i32.to_le_bytes());
    bytes.extend(300_001i32.to_le_bytes());
    bytes.extend(25i64.to_le_bytes());
    bytes.extend(7i16.to_le_bytes());
    bytes.extend(8i32.to_le_bytes());
    bytes.extend(9i32.to_le_bytes());
    for v in 1..=8i16 {
        bytes.extend((v * 10).to_le_bytes());
    }
    bytes
}

fn packet() -> MultiSellChoose {
    MultiSellChoose {
        list_id: 1235,
        entry_id: 300_001,
        amount: 25,
        unk1: 7,
        unk2: 8,
        unk3: 9,
        elemental: [10, 20, 30, 40, 50, 60, 70, 80],
    }
}

#[test]
fn multi_sell_choose_matches_javas_read_order() {
    assert_eq!(packet().to_bytes(), java_bytes());
    assert_eq!(MultiSellChoose::OPCODE, ClientOpcode::MultiSellChoose);
    assert_eq!(MultiSellChoose::OPCODE.id(), 0xB0);
}

#[test]
fn multi_sell_choose_round_trips() {
    let bytes = packet().to_bytes();
    let decoded = MultiSellChoose::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
    assert_eq!(decoded, packet());
    assert_eq!(MultiSellChoose::new(1, 2, 3).amount, 3);
}

#[test]
fn a_truncated_multi_sell_choose_is_an_error() {
    let bytes = java_bytes();
    for len in [1, 5, 12, 40] {
        assert!(MultiSellChoose::decode(&mut PacketReader::new(&bytes[1..len])).is_err());
    }
}

#[test]
fn the_dispatcher_decodes_it_in_game_only() {
    let bytes = java_bytes();
    let decoded = decode_client(
        ConnectionState::InGame,
        bytes[0],
        &mut PacketReader::new(&bytes[1..]),
    )
    .unwrap()
    .unwrap();
    assert_eq!(decoded, ClientPacket::MultiSellChoose(packet()));
    // Not before the character is in the world.
    for state in [
        ConnectionState::Connected,
        ConnectionState::Authenticated,
        ConnectionState::Entering,
    ] {
        assert!(
            decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..]))
                .unwrap()
                .is_none(),
            "{state:?}"
        );
    }
}

#[test]
fn multi_sell_list_has_the_java_opcode() {
    assert_eq!(MultiSellList::OPCODE, ServerOpcode::MultiSellList);
    let list = MultiSellList {
        list_id: 3,
        page: 1,
        finished: true,
        entries: Vec::<MultiSellEntry>::new(),
    };
    assert_eq!(list.to_bytes()[0], 0xD0);
}
