//! Crafting packets (M6-CRAFT-01) against byte layouts worked out by hand from the Java
//! `writeImpl` / `readImpl` code, plus round trips and the dispatcher. Same approach as
//! `m6_packets.rs`: no recorded session holds these packets, so the layouts are the spec.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    RecipeShopListEntry, RequestRecipeBookDestroy, RequestRecipeBookOpen, RequestRecipeItemMakeInfo,
    RequestRecipeItemMakeSelf, RequestRecipeShopListSet, RequestRecipeShopMakeInfo,
    RequestRecipeShopMakeItem, RequestRecipeShopManagePrev, RequestRecipeShopManageQuit,
    RequestRecipeShopMessageSet,
};
use l2_protocol_game::generated::opcodes::ClientOpcode;
use l2_protocol_game::server::{
    ManufactureEntry, RecipeBookItemList, RecipeItemMakeInfo, RecipeShopItemInfo, RecipeShopManageList,
    RecipeShopMsg, RecipeShopSellList,
};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

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
    fn string(mut self, text: &str) -> Self {
        for unit in text.encode_utf16() {
            self.0.extend(unit.to_le_bytes());
        }
        self.0.extend([0, 0]);
        self
    }
    fn done(self) -> Vec<u8> {
        self.0
    }
}

fn check<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P, expected: &[u8]) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected, "encoded bytes");
    let decoded = P::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
}

#[test]
fn recipe_book_item_list_layout() {
    // 0xDC, !isDwarvenCraft, maxMp, size, then (id, 1-based position).
    let packet = RecipeBookItemList {
        is_common: false,
        max_mp: 900,
        recipes: vec![7, 3],
    };
    check(
        &packet,
        &Bytes::default()
            .byte(0xDC)
            .int(0)
            .int(900)
            .int(2)
            .int(7)
            .int(1)
            .int(3)
            .int(2)
            .done(),
    );
    // A null recipe collection writes a 0 count, the same bytes as an empty book.
    let empty = RecipeBookItemList {
        is_common: true,
        max_mp: 5,
        recipes: vec![],
    };
    check(&empty, &Bytes::default().byte(0xDC).int(1).int(5).int(0).done());
}

#[test]
fn recipe_item_make_info_layout() {
    let packet = RecipeItemMakeInfo {
        id: 12,
        is_common: true,
        cur_mp: 80,
        max_mp: 100,
        success: false,
    };
    check(
        &packet,
        &Bytes::default()
            .byte(0xDD)
            .int(12)
            .int(1)
            .int(80)
            .int(100)
            .int(0)
            .done(),
    );
}

#[test]
fn recipe_shop_item_info_ends_with_minus_one() {
    let packet = RecipeShopItemInfo {
        object_id: 0x1000_0001,
        recipe_id: 4,
        cur_mp: 30,
        max_mp: 60,
    };
    check(
        &packet,
        &Bytes::default()
            .byte(0xE0)
            .int(0x1000_0001)
            .int(4)
            .int(30)
            .int(60)
            .int(-1)
            .done(),
    );
}

#[test]
fn recipe_shop_manage_list_layout() {
    let packet = RecipeShopManageList {
        seller_id: 99,
        adena: -5,
        is_common: false,
        recipes: vec![1, 2],
        items: vec![ManufactureEntry {
            recipe_id: 2,
            cost: 1_000_000_000_000,
        }],
    };
    check(
        &packet,
        &Bytes::default()
            .byte(0xDE)
            .int(99)
            .int(-5)
            .int(0)
            .int(2)
            .int(1)
            .int(1)
            .int(2)
            .int(2)
            .int(1)
            .int(2)
            .int(0)
            .long(1_000_000_000_000)
            .done(),
    );
}

#[test]
fn recipe_shop_msg_and_sell_list_layout() {
    check(
        &RecipeShopMsg {
            object_id: 5,
            message: "Arrows".into(),
        },
        &Bytes::default().byte(0xE1).int(5).string("Arrows").done(),
    );
    let sell = RecipeShopSellList {
        manufacturer_id: 5,
        cur_mp: 10,
        max_mp: 20,
        buyer_adena: 77,
        items: vec![
            ManufactureEntry {
                recipe_id: 1,
                cost: 10,
            },
            ManufactureEntry {
                recipe_id: 9,
                cost: 0,
            },
        ],
    };
    check(
        &sell,
        &Bytes::default()
            .byte(0xDF)
            .int(5)
            .int(10)
            .int(20)
            .long(77)
            .int(2)
            .int(1)
            .int(0)
            .long(10)
            .int(9)
            .int(0)
            .long(0)
            .done(),
    );
}

#[test]
fn simple_client_packets_round_trip() {
    check(
        &RequestRecipeBookOpen { recipe_type: 1 },
        &Bytes::default().byte(0xB5).int(1).done(),
    );
    check(
        &RequestRecipeBookDestroy { recipe_id: 40 },
        &Bytes::default().byte(0xB6).int(40).done(),
    );
    check(
        &RequestRecipeItemMakeInfo { id: 3 },
        &Bytes::default().byte(0xB7).int(3).done(),
    );
    check(
        &RequestRecipeItemMakeSelf { id: 3 },
        &Bytes::default().byte(0xB8).int(3).done(),
    );
    check(
        &RequestRecipeShopMessageSet { name: "Shop".into() },
        &Bytes::default().byte(0xBA).string("Shop").done(),
    );
    check(&RequestRecipeShopManageQuit, &[0xBC]);
    check(
        &RequestRecipeShopMakeInfo {
            player_object_id: 8,
            recipe_id: 2,
        },
        &Bytes::default().byte(0xBE).int(8).int(2).done(),
    );
    check(
        &RequestRecipeShopMakeItem {
            id: 8,
            recipe_id: 2,
            unknown: 0,
        },
        &Bytes::default().byte(0xBF).int(8).int(2).long(0).done(),
    );
    check(&RequestRecipeShopManagePrev, &[0xC0]);
}

#[test]
fn list_set_reads_java_batches() {
    let packet = RequestRecipeShopListSet {
        items: Some(vec![
            RecipeShopListEntry {
                recipe_id: 1,
                cost: 5,
            },
            RecipeShopListEntry {
                recipe_id: 2,
                cost: 0,
            },
        ]),
    };
    check(
        &packet,
        &Bytes::default()
            .byte(0xBB)
            .int(2)
            .int(1)
            .long(5)
            .int(2)
            .long(0)
            .done(),
    );
}

#[test]
fn list_set_drops_the_bad_lists_java_drops() {
    let decode = |body: Vec<u8>| RequestRecipeShopListSet::decode(&mut PacketReader::new(&body)).unwrap();
    // count 0 and a negative count.
    assert_eq!(decode(Bytes::default().int(0).done()).items, None);
    assert_eq!(decode(Bytes::default().int(-1).done()).items, None);
    // `count * 12 != remaining()`: one entry announced, two present.
    let long = Bytes::default().int(1).int(1).long(1).int(2).long(2).done();
    assert_eq!(decode(long).items, None);
    // A negative price.
    assert_eq!(decode(Bytes::default().int(1).int(1).long(-1).done()).items, None);
}

#[test]
fn dispatcher_routes_the_recipe_opcodes_in_game_only() {
    let body = Bytes::default().int(1).done();
    let packet = decode_client(ConnectionState::InGame, 0xB5, &mut PacketReader::new(&body)).unwrap();
    assert_eq!(
        packet,
        Some(ClientPacket::RequestRecipeBookOpen(RequestRecipeBookOpen {
            recipe_type: 1
        }))
    );
    assert_eq!(
        decode_client(
            ConnectionState::Authenticated,
            0xB5,
            &mut PacketReader::new(&body)
        )
        .unwrap(),
        None
    );
    // 0xB9 and 0xBD map to `null` in Java's ClientPackets: no packet is made, the world only
    // hears "not ported" (and does nothing), like Java's dropped packet.
    for (opcode, name) in [
        (0xB9, ClientOpcode::RequestRecipeShopManageList),
        (0xBD, ClientOpcode::RequestRecipeShopManageCancel),
    ] {
        assert_eq!(
            decode_client(ConnectionState::InGame, opcode, &mut PacketReader::new(&[])).unwrap(),
            Some(ClientPacket::Unported(name))
        );
    }
    let manage_quit = decode_client(ConnectionState::InGame, 0xBC, &mut PacketReader::new(&[])).unwrap();
    assert_eq!(
        manage_quit,
        Some(ClientPacket::RequestRecipeShopManageQuit(
            RequestRecipeShopManageQuit
        ))
    );
}
