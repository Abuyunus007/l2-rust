//! M6 social packets (party, trade, private stores, clans and alliances, summons and pets)
//! against byte layouts worked out by hand from the Java `writeImpl` / `readImpl` code, plus
//! round trips.
//!
//! No recorded Java session holds any of these packets yet (the committed captures are a
//! character screen and a short solo session), so there is no fixture comparison: the layouts
//! below are the spec. The expected bytes are built with an independent little-endian builder
//! (not the crate's `PacketWriter`), so a mistake in the crate's encoder cannot hide behind the
//! same mistake here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};

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

// ---------------------------------------------------------------------------------------------
// Party
// ---------------------------------------------------------------------------------------------

mod party {
    use super::*;
    use l2_protocol_game::client::{
        RequestAnswerJoinParty, RequestChangePartyLeader, RequestJoinParty, RequestOustPartyMember,
        RequestWithDrawalParty,
    };
    use l2_protocol_game::server::party::{
        AskJoinParty, ExPartyPetWindowAdd, ExPartyPetWindowDelete, ExPartyPetWindowUpdate, JoinParty,
        PartyDistributionType, PartyMemberLocation, PartyMemberPosition, PartyMemberVitals, PartyPetWindow,
        PartySmallWindowAdd, PartySmallWindowAll, PartySmallWindowDelete, PartySmallWindowDeleteAll,
        PartySmallWindowUpdate, PartySummon, PartyWindowMember,
    };

    fn vitals(object_id: i32, name: &str) -> PartyMemberVitals {
        PartyMemberVitals {
            object_id,
            name: name.into(),
            cp: 10,
            max_cp: 20,
            hp: 30,
            max_hp: 40,
            mp: 50,
            max_mp: 60,
            level: 70,
            class_id: 88,
        }
    }

    /// The ten leading ints of a member, in Java order.
    fn vitals_bytes(b: Bytes, object_id: i32, name: &str) -> Bytes {
        b.int(object_id)
            .string(name)
            .int(10)
            .int(20)
            .int(30)
            .int(40)
            .int(50)
            .int(60)
            .int(70)
            .int(88)
    }

    fn summon() -> PartySummon {
        PartySummon {
            object_id: 0x6000_0001,
            display_id: 14_001,
            summon_type: 2,
            name: "Wolf".into(),
            hp: 5,
            max_hp: 6,
            mp: 7,
            max_mp: 8,
            level: 9,
        }
    }

    #[test]
    fn client_packets() {
        check_round_trip(
            &RequestJoinParty {
                name: "Ann".into(),
                party_distribution_type_id: 3,
            },
            &Bytes::default().byte(0x42).string("Ann").int(3).done(),
            1,
        );
        check_round_trip(
            &RequestAnswerJoinParty { response: -1 },
            &Bytes::default().byte(0x43).int(-1).done(),
            1,
        );
        check_round_trip(&RequestWithDrawalParty, &[0x44], 1);
        check_round_trip(
            &RequestOustPartyMember { name: "Bob".into() },
            &Bytes::default().byte(0x45).string("Bob").done(),
            1,
        );
        // Ex packet 0xD0 0x000C.
        check_round_trip(
            &RequestChangePartyLeader { name: "Bob".into() },
            &Bytes::default().byte(0xD0).short(0x0C).string("Bob").done(),
            3,
        );
    }

    #[test]
    fn distribution_types_match_java() {
        let all = [
            (PartyDistributionType::FindersKeepers, 0, 487),
            (PartyDistributionType::Random, 1, 488),
            (PartyDistributionType::RandomIncludingSpoil, 2, 798),
            (PartyDistributionType::ByTurn, 3, 799),
            (PartyDistributionType::ByTurnIncludingSpoil, 4, 800),
        ];
        for (kind, id, string_id) in all {
            assert_eq!(kind.id(), id);
            assert_eq!(kind.sys_string_id(), string_id);
            assert_eq!(PartyDistributionType::from_id(id), Some(kind));
        }
        assert_eq!(PartyDistributionType::from_id(5), None);
        assert_eq!(PartyDistributionType::from_id(-1), None);
    }

    #[test]
    fn invitation_packets() {
        check_round_trip(
            &AskJoinParty {
                requestor_name: "Ann".into(),
                distribution_type: PartyDistributionType::RandomIncludingSpoil,
            },
            &Bytes::default().byte(0x39).string("Ann").int(2).done(),
            1,
        );
        check_round_trip(
            &JoinParty { response: 1 },
            &Bytes::default().byte(0x3A).int(1).done(),
            1,
        );
        // An unknown loot rule is a bad packet when read back.
        let bad = Bytes::default().string("Ann").int(9).done();
        assert!(AskJoinParty::decode(&mut PacketReader::new(&bad)).is_err());
    }

    #[test]
    fn small_window_all() {
        let packet = PartySmallWindowAll {
            leader_object_id: 100,
            distribution_type: PartyDistributionType::ByTurn,
            members: vec![
                PartyWindowMember {
                    vitals: vitals(101, "Bob"),
                    race: 3,
                    summon: None,
                },
                PartyWindowMember {
                    vitals: vitals(102, "Cy"),
                    race: 1,
                    summon: Some(summon()),
                },
            ],
        };
        let mut expected = Bytes::default().byte(0x4E).int(100).int(3).int(2);
        // Member 1: vitals, 0, race, 0, 0, then a single 0 for "no summon".
        expected = vitals_bytes(expected, 101, "Bob")
            .int(0)
            .int(3)
            .int(0)
            .int(0)
            .int(0);
        // Member 2: the summon block starts with its object id; the template id carries +1000000.
        expected = vitals_bytes(expected, 102, "Cy")
            .int(0)
            .int(1)
            .int(0)
            .int(0)
            .int(0x6000_0001)
            .int(1_014_001)
            .int(2)
            .string("Wolf")
            .int(5)
            .int(6)
            .int(7)
            .int(8)
            .int(9);
        check_round_trip(&packet, &expected.done(), 1);
    }

    #[test]
    fn small_window_add_update_delete() {
        let add = PartySmallWindowAdd {
            leader_object_id: 100,
            distribution_type: PartyDistributionType::FindersKeepers,
            member: vitals(101, "Bob"),
        };
        let expected = vitals_bytes(Bytes::default().byte(0x4F).int(100).int(0), 101, "Bob")
            .int(0)
            .int(0)
            .done();
        check_round_trip(&add, &expected, 1);

        let update = PartySmallWindowUpdate {
            member: vitals(101, "Bob"),
        };
        let expected = vitals_bytes(Bytes::default().byte(0x52), 101, "Bob").done();
        check_round_trip(&update, &expected, 1);

        check_round_trip(
            &PartySmallWindowDelete {
                object_id: 101,
                name: "Bob".into(),
            },
            &Bytes::default().byte(0x51).int(101).string("Bob").done(),
            1,
        );
        check_round_trip(&PartySmallWindowDeleteAll, &[0x50], 1);
    }

    #[test]
    fn member_positions() {
        let packet = PartyMemberPosition {
            locations: vec![
                PartyMemberLocation {
                    object_id: 101,
                    x: -1,
                    y: 2,
                    z: -3,
                },
                PartyMemberLocation {
                    object_id: 102,
                    x: 4,
                    y: 5,
                    z: 6,
                },
            ],
        };
        let expected = Bytes::default()
            .byte(0xBA)
            .int(2)
            .int(101)
            .int(-1)
            .int(2)
            .int(-3)
            .int(102)
            .int(4)
            .int(5)
            .int(6)
            .done();
        check_round_trip(&packet, &expected, 1);
        // A negative count is rejected, not allocated.
        let bad = Bytes::default().int(-1).done();
        assert!(PartyMemberPosition::decode(&mut PacketReader::new(&bad)).is_err());
    }

    #[test]
    fn pet_window_packets() {
        let window = PartyPetWindow {
            summon: summon(),
            owner_object_id: 101,
        };
        let body = |b: Bytes| {
            b.int(0x6000_0001)
                .int(1_014_001)
                .int(2)
                .int(101)
                .string("Wolf")
                .int(5)
                .int(6)
                .int(7)
                .int(8)
                .int(9)
                .done()
        };
        // 0xFE + u16 ex id: ADD 0x18, UPDATE 0x19, DELETE 0x6A.
        check_round_trip(
            &ExPartyPetWindowAdd {
                window: window.clone(),
            },
            &body(Bytes::default().byte(0xFE).short(0x18)),
            3,
        );
        check_round_trip(
            &ExPartyPetWindowUpdate { window },
            &body(Bytes::default().byte(0xFE).short(0x19)),
            3,
        );
        check_round_trip(
            &ExPartyPetWindowDelete {
                summon_object_id: 0x6000_0001,
                owner_object_id: 101,
                summon_name: "Wolf".into(),
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x6A)
                .int(0x6000_0001)
                .int(101)
                .string("Wolf")
                .done(),
            3,
        );
    }
}

/// `ItemInfo::plain(object_id, item_id, count)` as `write_item` sends it: location 0, no type
/// info, mana -1, time -9999, attack element -2, no defence and no enchant options.
fn plain_item_bytes(b: Bytes, object_id: i32, item_id: i32, count: i64) -> Bytes {
    b.int(object_id)
        .int(item_id)
        .int(0) // location
        .long(count)
        .short(0) // type2
        .short(0) // custom type 1
        .short(0) // equipped
        .int(0) // body part
        .short(0) // enchant
        .short(0) // custom type 2
        .int(0) // augmentation
        .int(-1) // mana
        .int(-9999) // time
        .short(-2) // attack element type
        .short(0) // attack element power
        .raw(&[0; 12]) // six defence elements
        .raw(&[0; 6]) // three enchant options
}

// ---------------------------------------------------------------------------------------------
// Trade
// ---------------------------------------------------------------------------------------------

mod trade {
    use super::*;
    use l2_protocol_game::client::{
        AddTradeItem, AnswerTradeRequest, TradeDone as ClientTradeDone, TradeRequest,
    };
    use l2_protocol_game::server::items::ItemInfo;
    use l2_protocol_game::server::trade::{
        SendTradeRequest, TradeDone, TradeItemInfo, TradeOtherAdd, TradeOtherDone, TradeOwnAdd, TradeStart,
        TradeUpdate,
    };

    fn item() -> TradeItemInfo {
        TradeItemInfo {
            type1: 1,
            object_id: 0x1000_0010,
            item_id: 57,
            count: 123_456_789_012,
            type2: 4,
            custom_type1: 5,
            body_part: 0x4000,
            enchant: 7,
            custom_type2: 8,
            attack_element_type: 2,
            attack_element_power: 30,
            element_defense: [1, 2, 3, 4, 5, 6],
            enchant_options: [11, 12, 13],
        }
    }

    /// The `writeTradeItem` body of [`item`].
    fn item_bytes(b: Bytes) -> Bytes {
        b.short(1) // type1
            .int(0x1000_0010)
            .int(57)
            .long(123_456_789_012)
            .short(4) // type2
            .short(5) // custom type 1
            .int(0x4000)
            .short(7) // enchant
            .short(0) // equipped, always 0
            .short(8) // custom type 2
            .short(2) // attack element type
            .short(30)
            .short(1)
            .short(2)
            .short(3)
            .short(4)
            .short(5)
            .short(6)
            .short(11)
            .short(12)
            .short(13)
    }

    #[test]
    fn client_packets() {
        check_round_trip(
            &TradeRequest {
                object_id: 0x1000_0001,
            },
            &Bytes::default().byte(0x1A).int(0x1000_0001).done(),
            1,
        );
        check_round_trip(
            &AnswerTradeRequest { response: 1 },
            &Bytes::default().byte(0x55).int(1).done(),
            1,
        );
        check_round_trip(
            &AddTradeItem {
                trade_id: 1,
                object_id: 0x1000_0010,
                count: 5_000_000_000,
            },
            &Bytes::default()
                .byte(0x1B)
                .int(1)
                .int(0x1000_0010)
                .long(5_000_000_000)
                .done(),
            1,
        );
        check_round_trip(
            &ClientTradeDone { response: 1 },
            &Bytes::default().byte(0x1C).int(1).done(),
            1,
        );
    }

    #[test]
    fn small_server_packets() {
        check_round_trip(
            &SendTradeRequest { sender_id: 77 },
            &Bytes::default().byte(0x70).int(77).done(),
            1,
        );
        check_round_trip(
            &TradeDone { num: 1 },
            &Bytes::default().byte(0x1C).int(1).done(),
            1,
        );
        check_round_trip(&TradeOtherDone, &[0x82], 1);
    }

    #[test]
    fn item_packets() {
        check_round_trip(
            &TradeOwnAdd { item: item() },
            &item_bytes(Bytes::default().byte(0x1A).short(1)).done(),
            1,
        );
        check_round_trip(
            &TradeOtherAdd { item: item() },
            &item_bytes(Bytes::default().byte(0x1B).short(1)).done(),
            1,
        );
        check_round_trip(
            &TradeUpdate {
                change: 3,
                item: item(),
            },
            &item_bytes(Bytes::default().byte(0x81).short(1).short(3)).done(),
            1,
        );
    }

    #[test]
    fn trade_update_change_follows_java() {
        // Java: `(newCount > 0) && stackable ? 3 : 2`.
        assert_eq!(TradeUpdate::change_for(5, true), 3);
        assert_eq!(TradeUpdate::change_for(0, true), 2);
        assert_eq!(TradeUpdate::change_for(5, false), 2);
        assert_eq!(TradeUpdate::change_for(-1, true), 2);
    }

    #[test]
    fn trade_start_lists_the_tradable_items() {
        let packet = TradeStart {
            partner_object_id: 0x1000_0002,
            items: vec![ItemInfo::plain(1, 57, 10), ItemInfo::plain(2, 1060, 3)],
        };
        let mut expected = Bytes::default().byte(0x14).int(0x1000_0002).short(2);
        expected = plain_item_bytes(expected, 1, 57, 10);
        expected = plain_item_bytes(expected, 2, 1060, 3);
        check_round_trip(&packet, &expected.done(), 1);
    }
}

// ---------------------------------------------------------------------------------------------
// Private stores
// ---------------------------------------------------------------------------------------------

mod store {
    use super::*;
    use l2_protocol_game::client::{
        BuyListEntry, RequestPrivateStoreBuy, RequestPrivateStoreManageBuy, RequestPrivateStoreManageSell,
        RequestPrivateStoreQuitBuy, RequestPrivateStoreQuitSell, RequestPrivateStoreSell, SellListEntry,
        SetPrivateStoreListBuy, SetPrivateStoreListSell, SetPrivateStoreMsgBuy, SetPrivateStoreMsgSell,
        SetPrivateStoreWholeMsg, StoreBuyRequestEntry, StoreSellRequestEntry,
    };
    use l2_protocol_game::server::items::ItemInfo;
    use l2_protocol_game::server::store::{
        BuyStoreEntry, ExPrivateStoreSetWholeMsg, PrivateStoreListBuy, PrivateStoreListSell,
        PrivateStoreManageListBuy, PrivateStoreManageListSell, PrivateStoreMsgBuy, PrivateStoreMsgSell,
        SellStoreEntry, StoreCandidate,
    };

    fn decode_body<P: Decode>(body: Vec<u8>) -> P {
        P::decode(&mut PacketReader::new(&body)).unwrap()
    }

    #[test]
    fn empty_and_message_client_packets() {
        check_round_trip(&RequestPrivateStoreManageSell, &[0x30], 1);
        check_round_trip(&RequestPrivateStoreManageBuy, &[0x99], 1);
        check_round_trip(&RequestPrivateStoreQuitSell, &[0x96], 1);
        check_round_trip(&RequestPrivateStoreQuitBuy, &[0x9C], 1);
        check_round_trip(
            &SetPrivateStoreMsgSell {
                store_msg: "cheap".into(),
            },
            &Bytes::default().byte(0x97).string("cheap").done(),
            1,
        );
        check_round_trip(
            &SetPrivateStoreMsgBuy {
                store_msg: "wanted".into(),
            },
            &Bytes::default().byte(0x9D).string("wanted").done(),
            1,
        );
        // Ex packet 0xD0 0x0049.
        check_round_trip(
            &SetPrivateStoreWholeMsg { msg: "pack".into() },
            &Bytes::default().byte(0xD0).short(0x49).string("pack").done(),
            3,
        );
    }

    #[test]
    fn set_list_sell() {
        let packet = SetPrivateStoreListSell {
            package_sale: true,
            items: Some(vec![
                SellListEntry {
                    item_id: 0x1000_0001,
                    count: 3,
                    price: 1_000,
                },
                SellListEntry {
                    item_id: 0x1000_0002,
                    count: 1,
                    price: 0,
                },
            ]),
        };
        let expected = Bytes::default()
            .byte(0x31)
            .int(1)
            .int(2)
            .int(0x1000_0001)
            .long(3)
            .long(1_000)
            .int(0x1000_0002)
            .long(1)
            .long(0)
            .done();
        check_round_trip(&packet, &expected, 1);
        // Java: `readInt() == 1` is the package flag; any other value is a normal sale.
        let body = Bytes::default().int(2).int(0).done();
        let decoded: SetPrivateStoreListSell = decode_body(body);
        assert!(!decoded.package_sale);
    }

    #[test]
    fn bad_lists_become_none() {
        let entry = |b: Bytes, id: i32, count: i64, price: i64| b.int(id).long(count).long(price);
        // Count does not match the bytes left: 2 announced, 1 present.
        let body = entry(Bytes::default().int(0).int(2), 1, 1, 1).done();
        assert_eq!(decode_body::<SetPrivateStoreListSell>(body).items, None);
        // Trailing bytes also break Java's `count * BATCH_LENGTH == remaining()` rule.
        let body = entry(Bytes::default().int(0).int(1), 1, 1, 1).byte(0).done();
        assert_eq!(decode_body::<SetPrivateStoreListSell>(body).items, None);
        // Count 0 and negative counts.
        assert_eq!(
            decode_body::<SetPrivateStoreListSell>(Bytes::default().int(0).int(0).done()).items,
            None
        );
        assert_eq!(
            decode_body::<SetPrivateStoreListSell>(Bytes::default().int(0).int(-1).done()).items,
            None
        );
        // One bad value drops the whole list: item id < 1, count < 1, price < 0.
        for (id, count, price) in [(0, 1, 1), (1, 0, 1), (1, 1, -1)] {
            let body = entry(Bytes::default().int(0).int(1), id, count, price).done();
            assert_eq!(decode_body::<SetPrivateStoreListSell>(body).items, None);
        }
        // Java multiplies in int, but it rejects counts above MAX_ITEM_IN_PACKET first, so a count
        // whose int product wraps to the 0 bytes left (2^30 * 20 == 5 * 2^32) is dropped, not read.
        let wrapped = Bytes::default().int(0).int(0x4000_0000).done();
        assert_eq!(decode_body::<SetPrivateStoreListSell>(wrapped).items, None);
        // A short body (no count) is a real decode error.
        assert!(SetPrivateStoreListSell::decode(&mut PacketReader::new(&[0, 0, 0, 0])).is_err());
    }

    #[test]
    fn set_list_buy() {
        let packet = SetPrivateStoreListBuy {
            items: Some(vec![BuyListEntry {
                item_id: 57,
                unknown1: 9,
                count: 10,
                price: 20,
                unknown2: [1, 2, 3, 4],
            }]),
        };
        let expected = Bytes::default()
            .byte(0x9A)
            .int(1)
            .int(57)
            .int(9)
            .long(10)
            .long(20)
            .int(1)
            .int(2)
            .int(3)
            .int(4)
            .done();
        check_round_trip(&packet, &expected, 1);
        // 40 bytes per entry: a 20 byte entry does not match.
        let body = Bytes::default().int(1).int(57).int(0).long(1).long(1).done();
        assert_eq!(decode_body::<SetPrivateStoreListBuy>(body).items, None);
    }

    #[test]
    fn customer_requests() {
        let buy = RequestPrivateStoreBuy {
            store_player_id: 0x1000_0003,
            items: Some(vec![
                StoreBuyRequestEntry {
                    object_id: 11,
                    count: 2,
                    price: 300,
                },
                // NOTE(java-quirk): the same object id twice stays twice (inverted `equals`).
                StoreBuyRequestEntry {
                    object_id: 11,
                    count: 1,
                    price: 300,
                },
            ]),
        };
        let expected = Bytes::default()
            .byte(0x83)
            .int(0x1000_0003)
            .int(2)
            .int(11)
            .long(2)
            .long(300)
            .int(11)
            .long(1)
            .long(300)
            .done();
        check_round_trip(&buy, &expected, 1);

        let sell = RequestPrivateStoreSell {
            store_player_id: 0x1000_0003,
            items: Some(vec![StoreSellRequestEntry {
                object_id: 12,
                item_id: 57,
                unknown: [3, 4],
                count: 5,
                price: 6,
            }]),
        };
        let expected = Bytes::default()
            .byte(0x9F)
            .int(0x1000_0003)
            .int(1)
            .int(12)
            .int(57)
            .short(3)
            .short(4)
            .long(5)
            .long(6)
            .done();
        check_round_trip(&sell, &expected, 1);
        // The sell request needs an item id as well.
        let body = Bytes::default()
            .int(1)
            .int(1)
            .int(12)
            .int(0)
            .short(0)
            .short(0)
            .long(5)
            .long(6)
            .done();
        assert_eq!(decode_body::<RequestPrivateStoreSell>(body).items, None);
    }

    #[test]
    fn manage_lists() {
        let sell = PrivateStoreManageListSell {
            object_id: 0x1000_0001,
            package_sale: true,
            player_adena: 5_000,
            item_list: vec![StoreCandidate {
                item: ItemInfo::plain(1, 57, 10),
                reference_price: 200,
            }],
            sell_list: vec![SellStoreEntry {
                item: ItemInfo::plain(2, 1060, 3),
                price: 15,
                reference_price: 40,
            }],
        };
        let mut expected = Bytes::default()
            .byte(0xA0)
            .int(0x1000_0001)
            .int(1)
            .long(5_000)
            .int(1);
        expected = plain_item_bytes(expected, 1, 57, 10).long(200).int(1);
        expected = plain_item_bytes(expected, 2, 1060, 3).long(15).long(40);
        check_round_trip(&sell, &expected.done(), 1);

        let buy = PrivateStoreManageListBuy {
            object_id: 0x1000_0001,
            player_adena: 5_000,
            item_list: vec![StoreCandidate {
                item: ItemInfo::plain(1, 57, 10),
                reference_price: 200,
            }],
            buy_list: vec![BuyStoreEntry {
                item: ItemInfo::plain(2, 1060, 3),
                price: 15,
                reference_price: 40,
                count: 8,
            }],
        };
        let mut expected = Bytes::default().byte(0xBD).int(0x1000_0001).long(5_000).int(1);
        expected = plain_item_bytes(expected, 1, 57, 10).long(200).int(1);
        expected = plain_item_bytes(expected, 2, 1060, 3).long(15).long(40).long(8);
        check_round_trip(&buy, &expected.done(), 1);
    }

    #[test]
    fn customer_lists() {
        let sell = PrivateStoreListSell {
            seller_object_id: 0x1000_0001,
            package_sale: false,
            player_adena: 77,
            items: vec![SellStoreEntry {
                item: ItemInfo::plain(2, 1060, 3),
                price: 15,
                reference_price: 40,
            }],
        };
        let expected = plain_item_bytes(
            Bytes::default()
                .byte(0xA1)
                .int(0x1000_0001)
                .int(0)
                .long(77)
                .int(1),
            2,
            1060,
            3,
        )
        .long(15)
        .long(40)
        .done();
        check_round_trip(&sell, &expected, 1);

        let buy = PrivateStoreListBuy {
            store_object_id: 0x1000_0001,
            player_adena: 77,
            items: vec![BuyStoreEntry {
                item: ItemInfo::plain(2, 1060, 3),
                price: 15,
                reference_price: 40,
                count: 9,
            }],
        };
        // Java repeats the object id after the item block.
        let expected = plain_item_bytes(
            Bytes::default().byte(0xBE).int(0x1000_0001).long(77).int(1),
            2,
            1060,
            3,
        )
        .int(2)
        .long(15)
        .long(40)
        .long(9)
        .done();
        check_round_trip(&buy, &expected, 1);
    }

    #[test]
    fn store_titles() {
        check_round_trip(
            &PrivateStoreMsgSell {
                object_id: 5,
                message: "cheap".into(),
            },
            &Bytes::default().byte(0xA2).int(5).string("cheap").done(),
            1,
        );
        check_round_trip(
            &PrivateStoreMsgBuy {
                object_id: 5,
                message: "wanted".into(),
            },
            &Bytes::default().byte(0xBF).int(5).string("wanted").done(),
            1,
        );
        check_round_trip(
            &ExPrivateStoreSetWholeMsg {
                object_id: 5,
                message: "pack".into(),
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x80)
                .int(5)
                .string("pack")
                .done(),
            3,
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Clans, wars and alliances
// ---------------------------------------------------------------------------------------------

mod pledge_client {
    use super::*;
    use l2_protocol_game::client::*;

    /// A plain packet: id byte, then the fields.
    fn plain(id: u8) -> Bytes {
        Bytes::default().byte(id)
    }

    /// An ex packet: 0xD0, then the u16 ex id.
    fn ex(id: i16) -> Bytes {
        Bytes::default().byte(0xD0).short(id)
    }

    #[test]
    fn join_and_leave() {
        check_round_trip(
            &RequestJoinPledge {
                target: 0x1000_0005,
                pledge_type: -1,
            },
            &plain(0x26).int(0x1000_0005).int(-1).done(),
            1,
        );
        check_round_trip(
            &RequestAnswerJoinPledge { answer: 1 },
            &plain(0x27).int(1).done(),
            1,
        );
        check_round_trip(&RequestWithdrawalPledge, &[0x28], 1);
        check_round_trip(
            &RequestOustPledgeMember { target: "Bob".into() },
            &plain(0x29).string("Bob").done(),
            1,
        );
    }

    #[test]
    fn information_requests() {
        check_round_trip(&RequestPledgeMemberList, &[0x4D], 1);
        check_round_trip(&RequestPledgeInfo { clan_id: 77 }, &plain(0x65).int(77).done(), 1);
        check_round_trip(
            &RequestPledgeExtendedInfo { name: "Clan".into() },
            &plain(0x66).string("Clan").done(),
            1,
        );
        check_round_trip(&RequestPledgeCrest { crest_id: 5 }, &plain(0x67).int(5).done(), 1);
        check_round_trip(
            &RequestExPledgeCrestLarge { crest_id: 5 },
            &ex(0x10).int(5).done(),
            3,
        );
        check_round_trip(&RequestPledgePowerGradeList, &ex(0x13).done(), 3);
        check_round_trip(
            &RequestPledgeMemberPowerInfo {
                unk1: 1,
                player: "Bob".into(),
            },
            &ex(0x14).int(1).string("Bob").done(),
            3,
        );
        check_round_trip(
            &RequestPledgeMemberInfo {
                unk1: 2,
                player: "Bob".into(),
            },
            &ex(0x16).int(2).string("Bob").done(),
            3,
        );
        check_round_trip(
            &RequestPledgeWarList { unk1: 0, tab: 1 },
            &ex(0x17).int(0).int(1).done(),
            3,
        );
    }

    #[test]
    fn management_requests() {
        check_round_trip(
            &RequestPledgeSetAcademyMaster {
                set: 1,
                curr_player_name: "Sponsor".into(),
                target_player_name: "Pupil".into(),
            },
            &ex(0x12).int(1).string("Sponsor").string("Pupil").done(),
            3,
        );
        check_round_trip(
            &RequestPledgeSetMemberPowerGrade {
                member: "Bob".into(),
                power_grade: 3,
            },
            &ex(0x15).string("Bob").int(3).done(),
            3,
        );
        check_round_trip(
            &RequestPledgeReorganizeMember {
                is_member_selected: 1,
                member_name: "Bob".into(),
                new_pledge_type: 100,
                selected_member: "Eve".into(),
            },
            &ex(0x2C).int(1).string("Bob").int(100).string("Eve").done(),
            3,
        );
    }

    #[test]
    fn pledge_power_reads_privileges_only_for_action_two() {
        check_round_trip(
            &RequestPledgePower {
                rank: 3,
                action: 2,
                privs: 0x1234,
            },
            &plain(0xCC).int(3).int(2).int(0x1234).done(),
            1,
        );
        check_round_trip(
            &RequestPledgePower {
                rank: 3,
                action: 1,
                privs: 0,
            },
            &plain(0xCC).int(3).int(1).done(),
            1,
        );
    }

    #[test]
    fn crest_uploads() {
        check_round_trip(
            &RequestSetPledgeCrest {
                length: 3,
                data: Some(vec![1, 2, 3]),
            },
            &plain(0x09).int(3).raw(&[1, 2, 3]).done(),
            1,
        );
        check_round_trip(
            &RequestExSetPledgeCrestLarge {
                length: 2,
                data: Some(vec![9, 8]),
            },
            &ex(0x11).int(2).raw(&[9, 8]).done(),
            3,
        );
        check_round_trip(
            &RequestSetAllyCrest {
                length: 1,
                data: Some(vec![7]),
            },
            &plain(0x91).int(1).raw(&[7]).done(),
            1,
        );
        // Java stops reading above the limit (256, 2176, 192): no data, and bytes may follow.
        for (limit, over) in [(256, 257), (192, 193)] {
            let body = Bytes::default().int(over).raw(&[0; 4]).done();
            let (length, data) = if limit == 256 {
                let p = RequestSetPledgeCrest::decode(&mut PacketReader::new(&body)).unwrap();
                (p.length, p.data)
            } else {
                let p = RequestSetAllyCrest::decode(&mut PacketReader::new(&body)).unwrap();
                (p.length, p.data)
            };
            assert_eq!((length, data), (over, None));
        }
        let body = Bytes::default().int(2177).done();
        let p = RequestExSetPledgeCrestLarge::decode(&mut PacketReader::new(&body)).unwrap();
        assert_eq!(p.data, None);
        // A length of exactly the limit is read in full, and a negative length or one beyond the
        // packet is Java's BufferUnderflowException: a decode error.
        let at_limit = Bytes::default().int(256).raw(&[5; 256]).done();
        let p = RequestSetPledgeCrest::decode(&mut PacketReader::new(&at_limit)).unwrap();
        assert_eq!(p.data.map(|d| d.len()), Some(256));
        let negative = Bytes::default().int(-1).done();
        assert!(RequestSetPledgeCrest::decode(&mut PacketReader::new(&negative)).is_err());
        let short = Bytes::default().int(10).raw(&[1, 2]).done();
        assert!(RequestSetPledgeCrest::decode(&mut PacketReader::new(&short)).is_err());
    }

    #[test]
    fn war_requests() {
        check_round_trip(
            &RequestStartPledgeWar {
                pledge_name: "Foes".into(),
            },
            &plain(0x03).string("Foes").done(),
            1,
        );
        check_round_trip(
            &RequestReplyStartPledgeWar {
                req_name: "Foes".into(),
                answer: 1,
            },
            &plain(0x04).string("Foes").int(1).done(),
            1,
        );
        check_round_trip(
            &RequestStopPledgeWar {
                pledge_name: "Foes".into(),
            },
            &plain(0x05).string("Foes").done(),
            1,
        );
        check_round_trip(
            &RequestReplyStopPledgeWar {
                req_name: "Foes".into(),
                answer: 0,
            },
            &plain(0x06).string("Foes").int(0).done(),
            1,
        );
        check_round_trip(
            &RequestSurrenderPledgeWar {
                pledge_name: "Foes".into(),
            },
            &plain(0x07).string("Foes").done(),
            1,
        );
        check_round_trip(
            &RequestReplySurrenderPledgeWar {
                req_name: "Foes".into(),
                answer: 1,
            },
            &plain(0x08).string("Foes").int(1).done(),
            1,
        );
    }

    #[test]
    fn alliance_requests() {
        check_round_trip(
            &RequestJoinAlly { id: 0x1000_0009 },
            &plain(0x8C).int(0x1000_0009).done(),
            1,
        );
        check_round_trip(
            &RequestAnswerJoinAlly { response: 1 },
            &plain(0x8D).int(1).done(),
            1,
        );
        check_round_trip(&AllyLeave, &[0x8E], 1);
        check_round_trip(
            &AllyDismiss {
                clan_name: "Clan".into(),
            },
            &plain(0x8F).string("Clan").done(),
            1,
        );
        check_round_trip(&RequestDismissAlly, &[0x90], 1);
        check_round_trip(&RequestAllyCrest { crest_id: 4 }, &plain(0x92).int(4).done(), 1);
        check_round_trip(&RequestAllyInfo, &[0x2E], 1);
    }
}

mod pledge_server {
    use super::*;
    use l2_protocol_game::server::pledge::*;

    fn member() -> PledgeMember {
        PledgeMember {
            name: "Bob".into(),
            level: 40,
            class_id: 18,
            sex: 1,
            race: 2,
            online_object_id: 0x1000_0001,
            has_sponsor: true,
        }
    }

    fn member_bytes(b: Bytes) -> Bytes {
        b.string("Bob")
            .int(40)
            .int(18)
            .int(1)
            .int(2)
            .int(0x1000_0001)
            .int(1)
    }

    #[test]
    fn ask_join_pledge_follows_the_value_dependent_layout() {
        // Main clan, no sub pledge: requestor, clan name.
        check_round_trip(
            &AskJoinPledge {
                requestor_object_id: 7,
                sub_pledge_name: None,
                pledge_type: 0,
                pledge_name: "Clan".into(),
            },
            &Bytes::default().byte(0x2C).int(7).string("Clan").done(),
            1,
        );
        // A type but no sub pledge name: requestor, type, clan name.
        check_round_trip(
            &AskJoinPledge {
                requestor_object_id: 7,
                sub_pledge_name: None,
                pledge_type: 100,
                pledge_name: "Clan".into(),
            },
            &Bytes::default().byte(0x2C).int(7).int(100).string("Clan").done(),
            1,
        );
        // Knight unit: the sub pledge name comes first because the type is above 0.
        check_round_trip(
            &AskJoinPledge {
                requestor_object_id: 7,
                sub_pledge_name: Some("Knights".into()),
                pledge_type: 100,
                pledge_name: "Clan".into(),
            },
            &Bytes::default()
                .byte(0x2C)
                .int(7)
                .string("Knights")
                .int(100)
                .string("Clan")
                .done(),
            1,
        );
        // Academy (type -1): Java writes the clan name where the sub pledge name would go.
        check_round_trip(
            &AskJoinPledge {
                requestor_object_id: 7,
                sub_pledge_name: Some("Clan".into()),
                pledge_type: -1,
                pledge_name: "Clan".into(),
            },
            &Bytes::default()
                .byte(0x2C)
                .int(7)
                .string("Clan")
                .int(-1)
                .string("Clan")
                .done(),
            1,
        );
        // The encoder follows Java even when the sub pledge name would be lost.
        let lossy = AskJoinPledge {
            requestor_object_id: 7,
            sub_pledge_name: Some("Academy".into()),
            pledge_type: -1,
            pledge_name: "Clan".into(),
        };
        assert_eq!(
            lossy.to_bytes(),
            Bytes::default()
                .byte(0x2C)
                .int(7)
                .string("Clan")
                .int(-1)
                .string("Clan")
                .done()
        );
        // Bytes that fit no layout are an error, not a guess.
        assert!(AskJoinPledge::decode(&mut PacketReader::new(&[1, 0, 0, 0, 0xFF])).is_err());
    }

    #[test]
    fn simple_packets() {
        check_round_trip(
            &JoinPledge { pledge_id: 9 },
            &Bytes::default().byte(0x2D).int(9).done(),
            1,
        );
        check_round_trip(
            &PledgeInfo {
                clan_id: 9,
                clan_name: "Clan".into(),
                ally_name: "Ally".into(),
            },
            &Bytes::default()
                .byte(0x89)
                .int(9)
                .string("Clan")
                .string("Ally")
                .done(),
            1,
        );
        check_round_trip(
            &PledgeShowMemberListDelete {
                player_name: "Bob".into(),
            },
            &Bytes::default().byte(0x5D).string("Bob").done(),
            1,
        );
        check_round_trip(&PledgeShowMemberListDeleteAll, &[0x88], 1);
        check_round_trip(
            &PledgeStatusChanged {
                leader_id: 1,
                clan_id: 2,
                crest_id: 3,
                ally_id: 4,
                ally_crest_id: 5,
            },
            &Bytes::default()
                .byte(0xCD)
                .int(1)
                .int(2)
                .int(3)
                .int(4)
                .int(5)
                .int(0)
                .int(0)
                .done(),
            1,
        );
        check_round_trip(
            &AskJoinAlly {
                requestor_object_id: 3,
                requestor_name: "Boss".into(),
            },
            &Bytes::default().byte(0xBB).int(3).string("Boss").done(),
            1,
        );
    }

    #[test]
    fn crest_packets() {
        check_round_trip(
            &PledgeCrest {
                crest_id: 5,
                data: vec![1, 2, 3],
            },
            &Bytes::default().byte(0x6A).int(5).int(3).raw(&[1, 2, 3]).done(),
            1,
        );
        // A crest that does not exist is a length of 0.
        check_round_trip(
            &PledgeCrest {
                crest_id: 5,
                data: vec![],
            },
            &Bytes::default().byte(0x6A).int(5).int(0).done(),
            1,
        );
        check_round_trip(
            &AllyCrest {
                crest_id: 6,
                data: vec![4],
            },
            &Bytes::default().byte(0xAF).int(6).int(1).raw(&[4]).done(),
            1,
        );
        check_round_trip(
            &ExPledgeEmblem {
                crest_id: 7,
                data: vec![9, 9],
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x1B)
                .int(0)
                .int(7)
                .int(2)
                .raw(&[9, 9])
                .done(),
            3,
        );
    }

    #[test]
    fn member_list_all() {
        let packet = PledgeShowMemberListAll {
            main_or_subpledge: 0,
            clan_id: 0x1000_0050,
            pledge_type: 0,
            clan_name: "Clan".into(),
            leader_name: "Boss".into(),
            crest_id: 11,
            level: 5,
            castle_id: 1,
            hideout_id: 2,
            fort_id: 3,
            rank: 4,
            reputation_score: 5_000,
            ally_id: 6,
            ally_name: "Ally".into(),
            ally_crest_id: 12,
            at_war: true,
            members: vec![member()],
        };
        let expected = member_bytes(
            Bytes::default()
                .byte(0x5A)
                .int(0)
                .int(0x1000_0050)
                .int(0)
                .string("Clan")
                .string("Boss")
                .int(11)
                .int(5)
                .int(1)
                .int(2)
                .int(3)
                .int(4)
                .int(5_000)
                .int(0)
                .int(0)
                .int(6)
                .string("Ally")
                .int(12)
                .int(1)
                .int(0) // territory castle id
                .int(1), // member count
        )
        .done();
        check_round_trip(&packet, &expected, 1);
    }

    #[test]
    fn member_list_update_add() {
        let online = PledgeShowMemberListUpdate {
            name: "Bob".into(),
            level: 40,
            class_id: 18,
            female: true,
            race: 2,
            online: Some(PledgeMemberOnline {
                object_id: 0x1000_0001,
                pledge_type: 100,
            }),
            has_sponsor: false,
        };
        check_round_trip(
            &online,
            &Bytes::default()
                .byte(0x5B)
                .string("Bob")
                .int(40)
                .int(18)
                .int(1)
                .int(2)
                .int(0x1000_0001)
                .int(100)
                .int(0)
                .done(),
            1,
        );
        // Going offline sends 0 for the object id and the pledge type.
        let offline = PledgeShowMemberListUpdate {
            online: None,
            ..online
        };
        check_round_trip(
            &offline,
            &Bytes::default()
                .byte(0x5B)
                .string("Bob")
                .int(40)
                .int(18)
                .int(1)
                .int(2)
                .int(0)
                .int(0)
                .int(0)
                .done(),
            1,
        );
        check_round_trip(
            &PledgeShowMemberListAdd {
                name: "Bob".into(),
                level: 40,
                class_id: 18,
                online_object_id: 0x1000_0001,
                pledge_type: 100,
            },
            &Bytes::default()
                .byte(0x5C)
                .string("Bob")
                .int(40)
                .int(18)
                .int(0)
                .int(1)
                .int(0x1000_0001)
                .int(100)
                .done(),
            1,
        );
    }

    #[test]
    fn member_details_and_sub_pledges() {
        check_round_trip(
            &PledgeReceiveMemberInfo {
                pledge_type: 100,
                name: "Bob".into(),
                title: "Sir".into(),
                power_grade: 3,
                pledge_name: "Knights".into(),
                apprentice_or_sponsor_name: "Eve".into(),
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x3E)
                .int(100)
                .string("Bob")
                .string("Sir")
                .int(3)
                .string("Knights")
                .string("Eve")
                .done(),
            3,
        );
        check_round_trip(
            &PledgeReceivePowerInfo {
                power_grade: 3,
                name: "Bob".into(),
                privileges: 0x7FFE,
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x3D)
                .int(3)
                .string("Bob")
                .int(0x7FFE)
                .done(),
            3,
        );
        check_round_trip(
            &PledgeReceiveSubPledgeCreated {
                id: 100,
                name: "Knights".into(),
                leader_name: String::new(),
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x40)
                .int(1)
                .int(100)
                .string("Knights")
                .string("")
                .done(),
            3,
        );
    }

    #[test]
    fn info_update() {
        check_round_trip(
            &PledgeShowInfoUpdate {
                clan_id: 1,
                crest_id: 2,
                level: 3,
                castle_id: 4,
                hideout_id: 5,
                fort_id: 6,
                rank: 7,
                reputation_score: 8,
                ally_id: 9,
                ally_name: "Ally".into(),
                ally_crest_id: 10,
                at_war: true,
            },
            &Bytes::default()
                .byte(0x8E)
                .int(1)
                .int(2)
                .int(3)
                .int(4)
                .int(5)
                .int(6)
                .int(7)
                .int(8)
                .int(0)
                .int(0)
                .int(9)
                .string("Ally")
                .int(10)
                .int(1)
                .done(),
            1,
        );
    }

    #[test]
    fn gm_view() {
        let packet = GmViewPledgeInfo {
            player_name: "Boss".into(),
            clan_id: 1,
            clan_name: "Clan".into(),
            leader_name: "Boss".into(),
            crest_id: 2,
            level: 3,
            castle_id: 4,
            hideout_id: 5,
            fort_id: 6,
            rank: 7,
            reputation_score: 8,
            ally_id: 9,
            ally_name: "Ally".into(),
            ally_crest_id: 10,
            at_war: false,
            members: vec![member()],
        };
        let expected = member_bytes(
            Bytes::default()
                .byte(0x96)
                .string("Boss")
                .int(1)
                .int(0)
                .string("Clan")
                .string("Boss")
                .int(2)
                .int(3)
                .int(4)
                .int(5)
                .int(6)
                .int(7)
                .int(8)
                .int(0)
                .int(0)
                .int(9)
                .string("Ally")
                .int(10)
                .int(0)
                .int(0) // T3 unknown
                .int(1),
        )
        .done();
        check_round_trip(&packet, &expected, 1);
    }

    #[test]
    fn privileges_and_skills() {
        check_round_trip(
            &PledgePowerGradeList {
                privs: vec![
                    RankPrivileges {
                        rank: 1,
                        privileges: 0x7FFE,
                    },
                    RankPrivileges {
                        rank: 2,
                        privileges: 0x10,
                    },
                ],
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x3C)
                .int(2)
                .int(1)
                .int(0x7FFE)
                .int(2)
                .int(0x10)
                .done(),
            3,
        );
        check_round_trip(
            &ManagePledgePower { privileges: 0x30 },
            &Bytes::default().byte(0x2A).int(0).int(0).int(0x30).done(),
            1,
        );
        check_round_trip(
            &PledgeSkillList {
                skills: vec![PledgeSkillEntry {
                    display_id: 370,
                    display_level: 1,
                }],
                sub_skills: vec![SubPledgeSkill {
                    sub_type: 100,
                    skill_id: 611,
                    skill_level: 2,
                }],
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x3A)
                .int(1)
                .int(1)
                .int(370)
                .int(1)
                .int(100)
                .int(611)
                .int(2)
                .done(),
            3,
        );
        check_round_trip(
            &PledgeSkillListAdd { id: 370, level: 3 },
            &Bytes::default().byte(0xFE).short(0x3B).int(370).int(3).done(),
            3,
        );
        check_round_trip(
            &ExSubPledgeSkillAdd {
                sub_type: 100,
                skill_id: 611,
                skill_level: 2,
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x76)
                .int(100)
                .int(611)
                .int(2)
                .done(),
            3,
        );
    }

    #[test]
    fn wars() {
        // Java skips clans it cannot find but still writes the full size as the count.
        check_round_trip(
            &PledgeReceiveWarList {
                tab: 1,
                list_size: 3,
                clan_names: vec!["Foes".into(), "Rivals".into()],
            },
            &Bytes::default()
                .byte(0xFE)
                .short(0x3F)
                .int(1)
                .int(0)
                .int(3)
                .string("Foes")
                .int(1)
                .int(1)
                .string("Rivals")
                .int(1)
                .int(1)
                .done(),
            3,
        );
        // The three war notices differ in the order of the two names.
        check_round_trip(
            &StartPledgeWar {
                player_name: "Boss".into(),
                pledge_name: "Clan".into(),
            },
            &Bytes::default().byte(0x63).string("Boss").string("Clan").done(),
            1,
        );
        check_round_trip(
            &StopPledgeWar {
                pledge_name: "Clan".into(),
                player_name: "Boss".into(),
            },
            &Bytes::default().byte(0x65).string("Clan").string("Boss").done(),
            1,
        );
        check_round_trip(
            &SurrenderPledgeWar {
                pledge_name: "Clan".into(),
                player_name: "Boss".into(),
            },
            &Bytes::default().byte(0x67).string("Clan").string("Boss").done(),
            1,
        );
    }

    #[test]
    fn alliance_info() {
        check_round_trip(
            &AllianceInfo {
                name: "Ally".into(),
                total: 30,
                online: 12,
                leader_clan: "Clan".into(),
                leader_player: "Boss".into(),
                allies: vec![
                    AllianceClanInfo {
                        clan_name: "Clan".into(),
                        level: 5,
                        leader_name: "Boss".into(),
                        total: 20,
                        online: 8,
                    },
                    AllianceClanInfo {
                        clan_name: "Other".into(),
                        level: 3,
                        leader_name: "Duke".into(),
                        total: 10,
                        online: 4,
                    },
                ],
            },
            &Bytes::default()
                .byte(0xB5)
                .string("Ally")
                .int(30)
                .int(12)
                .string("Clan")
                .string("Boss")
                .int(2)
                .string("Clan")
                .int(0)
                .int(5)
                .string("Boss")
                .int(20)
                .int(8)
                .string("Other")
                .int(0)
                .int(3)
                .string("Duke")
                .int(10)
                .int(4)
                .done(),
            1,
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Summons and pets
// ---------------------------------------------------------------------------------------------

mod summon {
    use super::*;
    use l2_protocol_game::client::{
        RequestChangePetName, RequestGetItemFromPet, RequestGiveItemToPet, RequestPetGetItem,
        RequestPetUseItem,
    };
    use l2_protocol_game::server::items::{ItemInfo, ItemUpdate};
    use l2_protocol_game::server::summon::{
        ExChangeNpcState, PetDelete, PetInfo, PetInventoryUpdate, PetItemList, PetStatusShow,
        PetStatusUpdate, Ride, SetSummonRemainTime,
    };

    trait DoubleBytes {
        fn double(self, v: f64) -> Self;
    }

    impl DoubleBytes for Bytes {
        fn double(mut self, v: f64) -> Self {
            self.0.extend(v.to_le_bytes());
            self
        }
    }

    #[test]
    fn client_packets() {
        check_round_trip(
            &RequestPetGetItem {
                object_id: 0x1000_0020,
            },
            &Bytes::default().byte(0x98).int(0x1000_0020).done(),
            1,
        );
        check_round_trip(
            &RequestGetItemFromPet {
                object_id: 0x1000_0020,
                amount: 5_000_000_000,
                unknown: 0,
            },
            &Bytes::default()
                .byte(0x2C)
                .int(0x1000_0020)
                .long(5_000_000_000)
                .int(0)
                .done(),
            1,
        );
        check_round_trip(
            &RequestGiveItemToPet {
                object_id: 0x1000_0020,
                amount: 3,
            },
            &Bytes::default().byte(0x95).int(0x1000_0020).long(3).done(),
            1,
        );
        check_round_trip(
            &RequestPetUseItem {
                object_id: 0x1000_0020,
            },
            &Bytes::default().byte(0x94).int(0x1000_0020).done(),
            1,
        );
        check_round_trip(
            &RequestChangePetName { name: "Rex".into() },
            &Bytes::default().byte(0x93).string("Rex").done(),
            1,
        );
    }

    #[test]
    fn small_server_packets() {
        check_round_trip(
            &PetStatusShow { summon_type: 2 },
            &Bytes::default().byte(0xB1).int(2).done(),
            1,
        );
        check_round_trip(
            &PetDelete {
                pet_type: 1,
                pet_object_id: 0x6000_0001,
            },
            &Bytes::default().byte(0xB7).int(1).int(0x6000_0001).done(),
            1,
        );
        check_round_trip(
            &SetSummonRemainTime {
                max_time: 1_200_000,
                remaining_time: 600_000,
            },
            &Bytes::default().byte(0xD1).int(1_200_000).int(600_000).done(),
            1,
        );
    }

    /// `Ride` (0x8C): seven ints; the npc id carries the offset of 1000000 even when nothing is
    /// ridden (a dismount sends 1000000).
    #[test]
    fn ride() {
        check_round_trip(
            &Ride {
                object_id: 0x6000_0001,
                mounted: true,
                ride_type: 2,
                ride_npc_id: 12_621,
                x: 82_698,
                y: 148_638,
                z: -3_473,
            },
            &Bytes::default()
                .byte(0x8C)
                .int(0x6000_0001)
                .int(1)
                .int(2)
                .int(1_012_621)
                .int(82_698)
                .int(148_638)
                .int(-3_473)
                .done(),
            1,
        );
        check_round_trip(
            &Ride {
                object_id: 7,
                mounted: false,
                ride_type: 0,
                ride_npc_id: 0,
                x: 1,
                y: 2,
                z: 3,
            },
            &Bytes::default()
                .byte(0x8C)
                .int(7)
                .int(0)
                .int(0)
                .int(1_000_000)
                .int(1)
                .int(2)
                .int(3)
                .done(),
            1,
        );
    }

    /// `ExChangeNpcState`: `0xFE`, the ex id `0xBE` and two ints; `Pet.setCurrentFed` sends `0x64`
    /// when the food runs out and `0x65` when it comes back.
    #[test]
    fn change_npc_state() {
        for state in [0x64, 0x65] {
            check_round_trip(
                &ExChangeNpcState {
                    object_id: 0x6000_0001,
                    state,
                },
                &Bytes::default()
                    .byte(0xFE)
                    .short(0xBE)
                    .int(0x6000_0001)
                    .int(state)
                    .done(),
                3,
            );
        }
    }

    #[test]
    fn status_update() {
        let packet = PetStatusUpdate {
            summon_type: 2,
            object_id: 0x6000_0001,
            x: -1,
            y: 2,
            z: -3,
            title: "Owner".into(),
            cur_fed: 100,
            max_fed: 200,
            cur_hp: 10,
            max_hp: 20,
            cur_mp: 30,
            max_mp: 40,
            level: 15,
            exp: 5_000_000_000,
            exp_for_this_level: 4_000_000_000,
            exp_for_next_level: 6_000_000_000,
        };
        let expected = Bytes::default()
            .byte(0xB6)
            .int(2)
            .int(0x6000_0001)
            .int(-1)
            .int(2)
            .int(-3)
            .string("Owner")
            .int(100)
            .int(200)
            .int(10)
            .int(20)
            .int(30)
            .int(40)
            .int(15)
            .long(5_000_000_000)
            .long(4_000_000_000)
            .long(6_000_000_000)
            .done();
        check_round_trip(&packet, &expected, 1);
    }

    #[test]
    fn pet_inventory() {
        let items = vec![ItemInfo::plain(1, 57, 10), ItemInfo::plain(2, 1060, 3)];
        let mut expected = Bytes::default().byte(0xB3).short(2);
        expected = plain_item_bytes(expected, 1, 57, 10);
        expected = plain_item_bytes(expected, 2, 1060, 3);
        check_round_trip(&PetItemList { items }, &expected.done(), 1);

        let update = PetInventoryUpdate {
            updates: vec![ItemUpdate {
                change: 1,
                item: ItemInfo::plain(1, 57, 10),
            }],
        };
        let expected = plain_item_bytes(Bytes::default().byte(0xB4).short(1).short(1), 1, 57, 10).done();
        check_round_trip(&update, &expected, 1);
    }

    fn pet_info() -> PetInfo {
        PetInfo {
            summon_type: 2,
            object_id: 0x6000_0001,
            display_id: 12_077,
            x: 1,
            y: 2,
            z: 3,
            heading: 4,
            m_atk_spd: 5,
            p_atk_spd: 6,
            run_speed: 7,
            walk_speed: 8,
            swim_run_speed: 9,
            swim_walk_speed: 10,
            fly_run_speed: 11,
            fly_walk_speed: 12,
            move_multiplier: 1.5,
            attack_speed_multiplier: 1.25,
            collision_radius: 8.0,
            collision_height: 25.5,
            weapon: 13,
            armor: 14,
            has_owner: true,
            running: true,
            in_combat: false,
            alike_dead: true,
            spawn_state: 2,
            name: "Rex".into(),
            title: "Owner".into(),
            pvp_flag: 0,
            karma: 15,
            cur_fed: 16,
            max_fed: 17,
            cur_hp: 18,
            max_hp: 19,
            cur_mp: 20,
            max_mp: 21,
            sp: 22,
            level: 23,
            exp: 1_000,
            exp_for_this_level: 500,
            exp_for_next_level: 2_000,
            weight: 24,
            max_load: 25,
            p_atk: 26,
            p_def: 27,
            m_atk: 28,
            m_def: 29,
            accuracy: 30,
            evasion: 31,
            critical: 32,
            move_speed: 33,
            atk_speed: 34,
            casting_speed: 35,
            abnormal_visual_effects: 36,
            mountable: true,
            water_or_flying: 1,
            team_id: 2,
            soul_shots_per_hit: 37,
            spirit_shots_per_hit: 38,
            form_id: 39,
            abnormal_visual_effect_special: 40,
        }
    }

    fn pet_info_bytes(exp_this_level_written: i64) -> Vec<u8> {
        Bytes::default()
            .byte(0xB2)
            .int(2)
            .int(0x6000_0001)
            .int(1_012_077) // display id + 1000000
            .int(0) // attackable
            .int(1)
            .int(2)
            .int(3)
            .int(4)
            .int(0)
            .int(5)
            .int(6)
            .int(7)
            .int(8)
            .int(9)
            .int(10)
            .int(11)
            .int(12)
            .int(11)
            .int(12)
            .double(1.5)
            .double(1.25)
            .double(8.0)
            .double(25.5)
            .int(13)
            .int(14)
            .int(0) // left hand
            .byte(1)
            .byte(1)
            .byte(0)
            .byte(1)
            .byte(2)
            .int(-1)
            .string("Rex")
            .int(-1)
            .string("Owner")
            .int(1)
            .int(0)
            .int(15)
            .int(16)
            .int(17)
            .int(18)
            .int(19)
            .int(20)
            .int(21)
            .int(22)
            .int(23)
            .long(1_000)
            .long(exp_this_level_written)
            .long(2_000)
            .int(24)
            .int(25)
            .int(26)
            .int(27)
            .int(28)
            .int(29)
            .int(30)
            .int(31)
            .int(32)
            .int(33)
            .int(34)
            .int(35)
            .int(36)
            .short(1) // mountable
            .byte(1) // water
            .short(0)
            .byte(2) // team
            .int(37)
            .int(38)
            .int(39)
            .int(40)
            .done()
    }

    #[test]
    fn pet_info_layout() {
        check_round_trip(&pet_info(), &pet_info_bytes(500), 1);
    }

    #[test]
    fn pet_info_never_writes_a_level_start_above_the_experience() {
        // NOTE(java-quirk): Java writes `exp` instead when `expForThisLevel > exp`.
        let packet = PetInfo {
            exp: 1_000,
            exp_for_this_level: 1_500,
            ..pet_info()
        };
        assert_eq!(packet.to_bytes(), pet_info_bytes(1_000));
    }
}

// ---------------------------------------------------------------------------------------------
// Warehouses and freight (M6-WH-01)
// ---------------------------------------------------------------------------------------------

mod warehouse {
    use super::*;
    use l2_protocol_game::client::{
        RequestPackageSend, RequestPackageSendableItemList, SendWareHouseDepositList,
        SendWareHouseWithDrawList, WarehouseListEntry,
    };
    use l2_protocol_game::server::items::ItemInfo;
    use l2_protocol_game::server::{
        PackageSendableList, PackageToList, SortedWareHouseWithdrawalList, SortedWarehouseItem,
        WareHouseDepositList, WareHouseWithdrawalList,
    };

    fn entry(object_id: i32, count: i64) -> WarehouseListEntry {
        WarehouseListEntry { object_id, count }
    }

    /// The list a client sends: count, then `objectId` and `count` per entry (12 bytes each).
    fn list_bytes(b: Bytes, entries: &[(i32, i64)]) -> Bytes {
        let mut b = b.int(entries.len() as i32);
        for (object_id, count) in entries {
            b = b.int(*object_id).long(*count);
        }
        b
    }

    fn decode_client_body<P: Decode>(bytes: &[u8]) -> P {
        P::decode(&mut PacketReader::new(bytes)).unwrap()
    }

    #[test]
    fn the_deposit_list_round_trips_and_has_java_s_opcode() {
        let packet = SendWareHouseDepositList {
            items: Some(vec![entry(0x1000_0010, 5), entry(0x1000_0011, 1)]),
        };
        check_round_trip(
            &packet,
            &list_bytes(Bytes::default().byte(0x3B), &[(0x1000_0010, 5), (0x1000_0011, 1)]).done(),
            1,
        );
    }

    #[test]
    fn the_withdraw_list_round_trips_and_has_java_s_opcode() {
        let packet = SendWareHouseWithDrawList {
            items: Some(vec![entry(7, 3_000_000_000)]),
        };
        check_round_trip(
            &packet,
            &list_bytes(Bytes::default().byte(0x3C), &[(7, 3_000_000_000)]).done(),
            1,
        );
    }

    #[test]
    fn the_package_packets_round_trip() {
        let send = RequestPackageSend {
            object_id: 0x1000_0099,
            items: Some(vec![entry(8, 0), entry(9, 2)]),
        };
        check_round_trip(
            &send,
            &list_bytes(Bytes::default().byte(0xA8).int(0x1000_0099), &[(8, 0), (9, 2)]).done(),
            1,
        );
        check_round_trip(
            &RequestPackageSendableItemList {
                object_id: 0x1000_0099,
            },
            &Bytes::default().byte(0xA7).int(0x1000_0099).done(),
            1,
        );
    }

    /// Java `readImpl`: a count that does not match the bytes left, a count below 1, or an entry
    /// with a nonsense value leaves `_items` null, and the handler returns without a word.
    #[test]
    fn bad_lists_are_dropped_like_java() {
        // The count says 2, the bytes hold 1.
        let short = Bytes::default().int(2).int(5).long(1).done();
        assert_eq!(decode_client_body::<SendWareHouseDepositList>(&short).items, None);
        // The count says 1, the bytes hold 2.
        let long = list_bytes(Bytes::default(), &[(5, 1), (6, 1)]).done();
        let long = [&1_i32.to_le_bytes()[..], &long[4..]].concat();
        assert_eq!(decode_client_body::<SendWareHouseWithDrawList>(&long).items, None);
        // Count 0 and a negative count.
        assert_eq!(
            decode_client_body::<SendWareHouseDepositList>(&Bytes::default().int(0).done()).items,
            None
        );
        assert_eq!(
            decode_client_body::<SendWareHouseDepositList>(&Bytes::default().int(-1).done()).items,
            None
        );
        // Deposit: an object id below 1 or a count below 1.
        for bad in [(0, 1), (-4, 1), (5, 0), (5, -1)] {
            let bytes = list_bytes(Bytes::default(), &[bad]).done();
            assert_eq!(
                decode_client_body::<SendWareHouseDepositList>(&bytes).items,
                None,
                "{bad:?}"
            );
        }
        // Withdraw and package: a count of 0 is fine, below 0 is not; the object id still needs 1.
        let zero = list_bytes(Bytes::default(), &[(5, 0)]).done();
        assert_eq!(
            decode_client_body::<SendWareHouseWithDrawList>(&zero).items,
            Some(vec![entry(5, 0)])
        );
        for bad in [(0, 0), (5, -1)] {
            let bytes = list_bytes(Bytes::default(), &[bad]).done();
            assert_eq!(
                decode_client_body::<SendWareHouseWithDrawList>(&bytes).items,
                None,
                "{bad:?}"
            );
        }
        // `RequestPackageSend` reads the receiver first, then the list under the same rules.
        let bytes = list_bytes(Bytes::default().int(77), &[(5, 0), (6, -2)]).done();
        let decoded = decode_client_body::<RequestPackageSend>(&bytes);
        assert_eq!((decoded.object_id, decoded.items), (77, None));
    }

    fn info() -> ItemInfo {
        ItemInfo {
            object_id: 0x1000_0010,
            item_id: 57,
            location: 3,
            count: 123_456_789_012,
            type2: 4,
            custom_type1: 5,
            equipped: 0,
            body_part: 0x4000,
            enchant: 7,
            custom_type2: 8,
            augmentation: 0x0003_0002,
            mana: -1,
            time: -9999,
            attack_element_type: 2,
            attack_element_power: 30,
            element_defense: [1, 2, 3, 4, 5, 6],
            enchant_options: [11, 12, 13],
        }
    }

    /// `AbstractItemPacket.writeItem(ItemInfo)` of [`info`], then the repeated object id.
    fn info_bytes(b: Bytes) -> Bytes {
        b.int(0x1000_0010)
            .int(57)
            .int(3) // location
            .long(123_456_789_012)
            .short(4) // type2
            .short(5) // custom type 1
            .short(0) // equipped
            .int(0x4000)
            .short(7) // enchant
            .short(8) // custom type 2
            .int(0x0003_0002) // augmentation
            .int(-1) // mana
            .int(-9999) // time
            .short(2) // attack element
            .short(30)
            .short(1)
            .short(2)
            .short(3)
            .short(4)
            .short(5)
            .short(6)
            .short(11)
            .short(12)
            .short(13)
            .int(0x1000_0010) // the object id again
    }

    #[test]
    fn the_deposit_window_matches_java_s_write_impl() {
        // `WareHouseDepositList`: type (short), adena (long), count (short), items.
        let packet = WareHouseDepositList {
            wh_type: 4,
            player_adena: 9_000_000_000,
            items: vec![info()],
        };
        check_round_trip(
            &packet,
            &info_bytes(Bytes::default().byte(0x41).short(4).long(9_000_000_000).short(1)).done(),
            1,
        );
    }

    #[test]
    fn the_withdraw_window_matches_java_s_write_impl() {
        let packet = WareHouseWithdrawalList {
            wh_type: 1,
            player_adena: 5,
            items: vec![info(), ItemInfo::plain(2, 3, 4)],
        };
        let mut expected = info_bytes(Bytes::default().byte(0x42).short(1).long(5).short(2));
        expected = expected
            .int(2)
            .int(3)
            .int(0)
            .long(4)
            .short(0)
            .short(0)
            .short(0)
            .int(0)
            .short(0)
            .short(0)
            .int(0)
            .int(-1)
            .int(-9999)
            .short(-2)
            .short(0)
            .raw(&[0; 18])
            .int(2);
        check_round_trip(&packet, &expected.done(), 1);
    }

    #[test]
    fn the_sorted_withdraw_window_matches_java_s_write_impl() {
        let packet = SortedWareHouseWithdrawalList {
            wh_type: 4,
            player_adena: 70,
            items: vec![SortedWarehouseItem {
                object_id: 0x1000_0010,
                display_id: 57,
                location_slot: 3,
                count: 123_456_789_012,
                type2: 4,
                custom_type1: 5,
                body_part: 0x4000,
                enchant: 7,
                custom_type2: 8,
                augmentation: 0x0003_0002,
                mana: -1,
                time: -1,
                attack_element_type: 2,
                attack_element_power: 30,
                element_defense: [1, 2, 3, 4, 5, 6],
                enchant_options: [11, 12, 13],
            }],
        };
        // `SortedWareHouseWithdrawalList.writeImpl`: a constant 0 short where `ItemInfo` has the
        // equipped flag, the time -1 (`WarehouseItem`), the object id again at the end.
        let expected = Bytes::default()
            .byte(0x42)
            .short(4)
            .long(70)
            .short(1)
            .int(0x1000_0010)
            .int(57)
            .int(3)
            .long(123_456_789_012)
            .short(4)
            .short(5)
            .short(0) // "Can't be equipped in WH"
            .int(0x4000)
            .short(7)
            .short(8)
            .int(0x0003_0002)
            .int(-1)
            .int(-1)
            .short(2)
            .short(30)
            .short(1)
            .short(2)
            .short(3)
            .short(4)
            .short(5)
            .short(6)
            .short(11)
            .short(12)
            .short(13)
            .int(0x1000_0010)
            .done();
        check_round_trip(&packet, &expected, 1);
    }

    #[test]
    fn the_package_lists_match_java_s_write_impl() {
        let to_list = PackageToList {
            players: vec![(0x1000_0001, "Neo".into()), (0x1000_0002, "Trinity".into())],
        };
        check_round_trip(
            &to_list,
            &Bytes::default()
                .byte(0xC8)
                .int(2)
                .int(0x1000_0001)
                .string("Neo")
                .int(0x1000_0002)
                .string("Trinity")
                .done(),
            1,
        );
        check_round_trip(
            &PackageToList::default(),
            &Bytes::default().byte(0xC8).int(0).done(),
            1,
        );
        // `PackageSendableList`: the receiver (int), adena (long), count (int), items.
        let sendable = PackageSendableList {
            player_object_id: 0x1000_0001,
            adena: 12,
            items: vec![info()],
        };
        check_round_trip(
            &sendable,
            &info_bytes(Bytes::default().byte(0xD2).int(0x1000_0001).long(12).int(1)).done(),
            1,
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Friends and block list (M6-FRIEND-01)
// ---------------------------------------------------------------------------------------------

mod friend {
    use super::*;
    use l2_protocol_game::client::{
        RequestAnswerFriendInvite, RequestBlock, RequestFriendDel, RequestFriendInvite, RequestFriendList,
        RequestSendFriendMsg,
    };
    use l2_protocol_game::server::{FriendAddRequest, FriendPacket, FriendStatusPacket, L2FriendSay};

    #[test]
    fn client_friend_packets() {
        let p = RequestFriendInvite { name: "Ann".into() };
        check_round_trip(&p, &Bytes::default().byte(0x77).string("Ann").done(), 1);
        let p = RequestAnswerFriendInvite { response: 1 };
        check_round_trip(&p, &Bytes::default().byte(0x78).int(1).done(), 1);
        let p = RequestFriendList;
        check_round_trip(&p, &Bytes::default().byte(0x79).done(), 1);
        let p = RequestFriendDel { name: "Bob".into() };
        check_round_trip(&p, &Bytes::default().byte(0x7A).string("Bob").done(), 1);
        // Java readImpl: the text, then the receiver.
        let p = RequestSendFriendMsg {
            message: "hi".into(),
            receiver: "Bob".into(),
        };
        check_round_trip(
            &p,
            &Bytes::default().byte(0x6B).string("hi").string("Bob").done(),
            1,
        );
        // Block with a name, then the list request without one.
        let p = RequestBlock {
            kind: 0,
            name: Some("Eve".into()),
        };
        check_round_trip(&p, &Bytes::default().byte(0xA9).int(0).string("Eve").done(), 1);
        let p = RequestBlock { kind: 2, name: None };
        check_round_trip(&p, &Bytes::default().byte(0xA9).int(2).done(), 1);
    }

    #[test]
    fn server_friend_packets() {
        // Java FriendAddRequest.writeImpl: the name, then int 0.
        let p = FriendAddRequest {
            requestor_name: "Ann".into(),
        };
        check_round_trip(&p, &Bytes::default().byte(0x83).string("Ann").int(0).done(), 1);
        // Java FriendPacket: 1 for an added friend, 3 for a removed one, then the id, the name,
        // the online flag and the id again when online (else 0).
        let p = FriendPacket {
            added: true,
            object_id: 0x1000_0002,
            name: "Bob".into(),
            online: true,
        };
        check_round_trip(
            &p,
            &Bytes::default()
                .byte(0x76)
                .int(1)
                .int(0x1000_0002)
                .string("Bob")
                .int(1)
                .int(0x1000_0002)
                .done(),
            1,
        );
        let p = FriendPacket {
            added: false,
            object_id: 0x1000_0002,
            name: "Bob".into(),
            online: false,
        };
        check_round_trip(
            &p,
            &Bytes::default()
                .byte(0x76)
                .int(3)
                .int(0x1000_0002)
                .string("Bob")
                .int(0)
                .int(0)
                .done(),
            1,
        );
        // Java FriendStatusPacket: online, name, id.
        let p = FriendStatusPacket {
            online: false,
            name: "Bob".into(),
            object_id: 0x1000_0002,
        };
        check_round_trip(
            &p,
            &Bytes::default()
                .byte(0x77)
                .int(0)
                .string("Bob")
                .int(0x1000_0002)
                .done(),
            1,
        );
        // Java L2FriendSay: int 0, receiver, sender, message.
        let p = L2FriendSay {
            sender: "Ann".into(),
            receiver: "Bob".into(),
            message: "hi".into(),
        };
        check_round_trip(
            &p,
            &Bytes::default()
                .byte(0x78)
                .int(0)
                .string("Bob")
                .string("Ann")
                .string("hi")
                .done(),
            1,
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------------------------

mod dispatch {
    use super::*;
    use l2_protocol_game::client::*;
    use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

    /// What the dispatcher makes of the bytes an encoder wrote: the right variant in game, and
    /// nothing in the character screen states (Java checks the state before it reads a field).
    fn dispatches<P: Encode>(packet: &P, expected: ClientPacket) {
        let bytes = packet.to_bytes();
        let decode = |state| decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decode(ConnectionState::InGame), Some(expected));
        assert_eq!(decode(ConnectionState::Authenticated), None);
        assert_eq!(decode(ConnectionState::Connected), None);
    }

    #[test]
    fn warehouse_packets() {
        let p = SendWareHouseDepositList {
            items: Some(vec![WarehouseListEntry {
                object_id: 5,
                count: 1,
            }]),
        };
        dispatches(&p, ClientPacket::SendWareHouseDepositList(p.clone()));
        let p = SendWareHouseWithDrawList { items: None };
        dispatches(&p, ClientPacket::SendWareHouseWithDrawList(p.clone()));
        let p = RequestPackageSend {
            object_id: 9,
            items: Some(vec![WarehouseListEntry {
                object_id: 5,
                count: 0,
            }]),
        };
        dispatches(&p, ClientPacket::RequestPackageSend(p.clone()));
        let p = RequestPackageSendableItemList { object_id: 9 };
        dispatches(&p, ClientPacket::RequestPackageSendableItemList(p));
    }

    #[test]
    fn party_and_trade_packets() {
        let p = RequestJoinParty {
            name: "Ann".into(),
            party_distribution_type_id: 2,
        };
        dispatches(&p, ClientPacket::RequestJoinParty(p.clone()));
        let p = RequestAnswerJoinParty { response: 1 };
        dispatches(&p, ClientPacket::RequestAnswerJoinParty(p));
        let p = RequestOustPartyMember { name: "Bob".into() };
        dispatches(&p, ClientPacket::RequestOustPartyMember(p.clone()));
        let p = TradeRequest { object_id: 5 };
        dispatches(&p, ClientPacket::TradeRequest(p));
        let p = AnswerTradeRequest { response: 1 };
        dispatches(&p, ClientPacket::AnswerTradeRequest(p));
        let p = AddTradeItem {
            trade_id: 1,
            object_id: 2,
            count: 3,
        };
        dispatches(&p, ClientPacket::AddTradeItem(p));
        let p = TradeDone { response: 1 };
        dispatches(&p, ClientPacket::TradeDone(p));
    }

    #[test]
    fn store_packets() {
        let p = SetPrivateStoreMsgSell {
            store_msg: "x".into(),
        };
        dispatches(&p, ClientPacket::SetPrivateStoreMsgSell(p.clone()));
        let p = RequestPrivateStoreBuy {
            store_player_id: 1,
            items: None,
        };
        dispatches(&p, ClientPacket::RequestPrivateStoreBuy(p.clone()));
    }

    #[test]
    fn pledge_and_pet_packets() {
        let p = RequestJoinPledge {
            target: 1,
            pledge_type: 0,
        };
        dispatches(&p, ClientPacket::RequestJoinPledge(p.clone()));
        let p = RequestGetItemFromPet {
            object_id: 1,
            amount: 2,
            unknown: 0,
        };
        dispatches(&p, ClientPacket::RequestGetItemFromPet(p.clone()));
    }

    #[test]
    fn ex_social_packets() {
        // Ex packets: the dispatcher consumes the u16 ex id, so the whole 0xD0 packet goes in.
        for (packet, expected) in [
            (
                RequestChangePartyLeader { name: "Bob".into() }.to_bytes(),
                ClientPacket::RequestChangePartyLeader(RequestChangePartyLeader { name: "Bob".into() }),
            ),
            (
                SetPrivateStoreWholeMsg { msg: "pack".into() }.to_bytes(),
                ClientPacket::SetPrivateStoreWholeMsg(SetPrivateStoreWholeMsg { msg: "pack".into() }),
            ),
            (
                RequestPledgePowerGradeList.to_bytes(),
                ClientPacket::RequestPledgePowerGradeList(RequestPledgePowerGradeList),
            ),
            (
                RequestExSetPledgeCrestLarge {
                    length: 1,
                    data: Some(vec![3]),
                }
                .to_bytes(),
                ClientPacket::RequestExSetPledgeCrestLarge(RequestExSetPledgeCrestLarge {
                    length: 1,
                    data: Some(vec![3]),
                }),
            ),
        ] {
            let decode =
                |state| decode_client(state, packet[0], &mut PacketReader::new(&packet[1..])).unwrap();
            assert_eq!(decode(ConnectionState::InGame), Some(expected));
            assert_eq!(decode(ConnectionState::Connected), None);
        }
    }
}
