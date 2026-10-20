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

// Silence the unused-helper warnings of the builder methods the later groups use.
#[allow(dead_code)]
fn _builder_is_complete(b: Bytes) -> Vec<u8> {
    b.long(0).raw(&[]).done()
}
