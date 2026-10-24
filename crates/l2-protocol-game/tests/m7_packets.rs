//! M7 residence packets (castle and clan hall sieges, fortresses, manor, Territory War and the
//! item auction) against byte layouts worked out by hand from the Java `writeImpl` / `readImpl`
//! code, plus round trips and the dispatcher.
//!
//! No recorded Java session holds any of these packets yet, so there is no fixture comparison:
//! the layouts below are the spec. The expected bytes are built with an independent little-endian
//! builder (not the crate's `PacketWriter`), so a mistake in the crate's encoder cannot hide
//! behind the same mistake here.
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
// Sieges
// ---------------------------------------------------------------------------------------------

mod siege {
    use super::*;
    use l2_protocol_game::client::{
        RequestConfirmSiegeWaitingList, RequestJoinSiege, RequestSiegeAttackerList, RequestSiegeDefenderList,
        RequestSiegeInfo,
    };
    use l2_protocol_game::server::siege::{
        CastleInfo, DefenderState, ExShowCastleInfo, SiegeAttackerList, SiegeClan, SiegeDefender,
        SiegeDefenderList, SiegeInfo, SiegeOwnerClan, SiegeTime,
    };

    fn clan(
        clan_id: i32,
        name: &str,
        leader: &str,
        crest_id: i32,
        ally_id: i32,
        ally: &str,
        ally_crest: i32,
    ) -> SiegeClan {
        SiegeClan {
            clan_id,
            name: name.into(),
            leader_name: leader.into(),
            crest_id,
            ally_id,
            ally_name: ally.into(),
            ally_crest_id: ally_crest,
        }
    }

    /// The clan record of the siege lists, with `state` only for the defender list.
    fn clan_bytes(b: Bytes, c: (i32, &str, &str, i32), state: Option<i32>, ally: (i32, &str, i32)) -> Bytes {
        let mut b = b.int(c.0).string(c.1).string(c.2).int(c.3).int(0);
        if let Some(state) = state {
            b = b.int(state);
        }
        b.int(ally.0).string(ally.1).string("").int(ally.2)
    }

    #[test]
    fn client_packets() {
        check_round_trip(&RequestSiegeInfo, &[0xAA], 1);
        check_round_trip(
            &RequestSiegeAttackerList { castle_id: 5 },
            &Bytes::default().byte(0xAB).int(5).done(),
            1,
        );
        check_round_trip(
            &RequestSiegeDefenderList { castle_id: 7 },
            &Bytes::default().byte(0xAC).int(7).done(),
            1,
        );
        check_round_trip(
            &RequestJoinSiege {
                castle_id: 1,
                is_attacker: 1,
                is_joining: 0,
            },
            &Bytes::default().byte(0xAD).int(1).int(1).int(0).done(),
            1,
        );
        check_round_trip(
            &RequestConfirmSiegeWaitingList {
                castle_id: 3,
                clan_id: 44,
                approved: 1,
            },
            // Java reads the castle id, then the clan id, then the answer.
            &Bytes::default().byte(0xAE).int(3).int(44).int(1).done(),
            1,
        );
    }

    #[test]
    fn attacker_list_matches_java_layout() {
        let packet = SiegeAttackerList {
            residence_id: 21,
            clans: vec![
                Some(clan(100, "Alpha", "Lead", 9, 55, "Ally", 8)),
                Some(clan(101, "Beta", "Bl", 3, 0, "", 0)),
            ],
        };
        let mut expected = Bytes::default()
            .byte(0xCA)
            .int(21)
            .int(0)
            .int(1)
            .int(0)
            .int(2)
            .int(2);
        expected = clan_bytes(expected, (100, "Alpha", "Lead", 9), None, (55, "Ally", 8));
        expected = clan_bytes(expected, (101, "Beta", "Bl", 3), None, (0, "", 0));
        check_round_trip(&packet, &expected.done(), 1);
    }

    #[test]
    fn attacker_list_counts_a_clan_the_table_does_not_know() {
        // Java skips the record of a clan that `ClanTable` does not know, but the count still
        // includes it (NOTE(java-quirk)): two entries, one record.
        let packet = SiegeAttackerList {
            residence_id: 21,
            clans: vec![None, Some(clan(100, "Alpha", "Lead", 9, 55, "Ally", 8))],
        };
        let mut expected = Bytes::default()
            .byte(0xCA)
            .int(21)
            .int(0)
            .int(1)
            .int(0)
            .int(2)
            .int(2);
        expected = clan_bytes(expected, (100, "Alpha", "Lead", 9), None, (55, "Ally", 8));
        assert_eq!(packet.to_bytes(), expected.done());
        // Nobody known: 0 and 0 in the count, as Java writes an empty list.
        let empty = SiegeAttackerList {
            residence_id: 21,
            clans: Vec::new(),
        };
        check_round_trip(
            &empty,
            &Bytes::default()
                .byte(0xCA)
                .int(21)
                .int(0)
                .int(1)
                .int(0)
                .int(0)
                .int(0)
                .done(),
            1,
        );
    }

    #[test]
    fn defender_list_matches_java_layout() {
        let packet = SiegeDefenderList {
            castle_id: 3,
            clans: vec![
                Some(SiegeDefender {
                    clan: clan(100, "Alpha", "Lead", 9, 55, "Ally", 8),
                    state: DefenderState::Owner,
                }),
                Some(SiegeDefender {
                    clan: clan(101, "Beta", "Bl", 3, 0, "", 0),
                    state: DefenderState::DefenderPending,
                }),
                Some(SiegeDefender {
                    clan: clan(102, "Gamma", "Gl", 4, 0, "", 0),
                    state: DefenderState::Defender,
                }),
            ],
        };
        let mut expected = Bytes::default()
            .byte(0xCB)
            .int(3)
            .int(0)
            .int(1)
            .int(0)
            .int(3)
            .int(3);
        expected = clan_bytes(expected, (100, "Alpha", "Lead", 9), Some(1), (55, "Ally", 8));
        expected = clan_bytes(expected, (101, "Beta", "Bl", 3), Some(2), (0, "", 0));
        expected = clan_bytes(expected, (102, "Gamma", "Gl", 4), Some(3), (0, "", 0));
        check_round_trip(&packet, &expected.done(), 1);
    }

    #[test]
    fn siege_info_with_hours_and_an_owner() {
        let packet = SiegeInfo {
            residence_id: 21,
            viewer_is_owner_leader: true,
            owner_id: 9,
            owner: Some(SiegeOwnerClan {
                name: "Lord".into(),
                leader_name: "Boss".into(),
                ally_id: 4,
                ally_name: "Ally".into(),
            }),
            now_secs: 1000,
            time: SiegeTime::Hours(vec![11, 12]),
        };
        let expected = Bytes::default()
            .byte(0xC9)
            .int(21)
            .int(1)
            .int(9)
            .string("Lord")
            .string("Boss")
            .int(4)
            .string("Ally")
            .int(1000)
            // The hour list: 0, the count, then one time per hour.
            .int(0)
            .int(2)
            .int(11)
            .int(12)
            .done();
        check_round_trip(&packet, &expected, 1);
    }

    #[test]
    fn siege_info_without_an_owner_writes_empty_names() {
        let packet = SiegeInfo {
            residence_id: 21,
            viewer_is_owner_leader: false,
            owner_id: 0,
            owner: None,
            now_secs: 1000,
            time: SiegeTime::Date(777),
        };
        let expected = Bytes::default()
            .byte(0xC9)
            .int(21)
            .int(0)
            .int(0)
            .string("")
            .string("")
            .int(0)
            .string("")
            .int(1000)
            // The date, then 0.
            .int(777)
            .int(0)
            .done();
        check_round_trip(&packet, &expected, 1);
    }

    #[test]
    fn siege_info_with_an_owner_id_and_no_clan_writes_no_owner_block() {
        // NOTE(java-quirk): Java writes no owner block when the owner id names no clan, so the
        // packet is shorter. Only the encoder is checked: the decoder reads a block there.
        let packet = SiegeInfo {
            residence_id: 21,
            viewer_is_owner_leader: false,
            owner_id: 9,
            owner: None,
            now_secs: 1000,
            time: SiegeTime::Date(777),
        };
        let expected = Bytes::default()
            .byte(0xC9)
            .int(21)
            .int(0)
            .int(9)
            .int(1000)
            .int(777)
            .int(0)
            .done();
        assert_eq!(packet.to_bytes(), expected);
    }

    #[test]
    fn castle_list_matches_java_layout() {
        let packet = ExShowCastleInfo {
            castles: vec![
                CastleInfo {
                    residence_id: 1,
                    owner_name: "Clan".into(),
                    tax_percent: 10,
                    siege_date_secs: 99,
                },
                CastleInfo {
                    residence_id: 2,
                    owner_name: String::new(),
                    tax_percent: 0,
                    siege_date_secs: 0,
                },
            ],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x14)
            .int(2)
            .int(1)
            .string("Clan")
            .int(10)
            .int(99)
            .int(2)
            .string("")
            .int(0)
            .int(0)
            .done();
        check_round_trip(&packet, &expected, 3);
    }
}

// ---------------------------------------------------------------------------------------------
// Fortresses
// ---------------------------------------------------------------------------------------------

mod fort {
    use super::*;
    use l2_protocol_game::client::{
        RequestAllFortressInfo, RequestFortressMapInfo, RequestFortressSiegeInfo,
    };
    use l2_protocol_game::server::fort::{
        ExShowFortressInfo, ExShowFortressMapInfo, ExShowFortressSiegeInfo, FortressInfo,
    };

    #[test]
    fn client_packets() {
        check_round_trip(
            &RequestAllFortressInfo,
            &Bytes::default().byte(0xD0).short(0x3D).done(),
            3,
        );
        check_round_trip(
            &RequestFortressSiegeInfo,
            &Bytes::default().byte(0xD0).short(0x3F).done(),
            3,
        );
        check_round_trip(
            &RequestFortressMapInfo { fortress_id: 3 },
            &Bytes::default().byte(0xD0).short(0x48).int(3).done(),
            3,
        );
    }

    #[test]
    fn fortress_list_matches_java_layout() {
        let packet = ExShowFortressInfo {
            forts: vec![FortressInfo {
                residence_id: 101,
                owner_clan_name: "Fo".into(),
                siege_in_progress: true,
                owned_time: 5,
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x15)
            .int(1)
            .int(101)
            .string("Fo")
            .int(1)
            .int(5)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn siege_window_barracks_follow_the_commander_count() {
        // 3 commander spawns, 1 commander alive: value 2 (Java's switch: 3 - 1).
        let packet = ExShowFortressSiegeInfo::new(2, 4, 3, 1);
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x17)
            .int(2)
            .int(4)
            .int(2)
            .done();
        check_round_trip(&packet, &expected, 3);
        // 4 commander spawns, 4 alive: value 1 (Java's switch: 5 - 4).
        assert_eq!(ExShowFortressSiegeInfo::new(2, 4, 4, 4).barracks, vec![1]);
        // No commander spawns: one zero per barracks.
        let packet = ExShowFortressSiegeInfo::new(2, 3, 0, 0);
        assert_eq!(packet.barracks, vec![0, 0, 0]);
        // A combination Java has no case for writes nothing after the sizes.
        assert!(ExShowFortressSiegeInfo::new(2, 4, 3, 4).barracks.is_empty());
    }

    #[test]
    fn fortress_map_barracks_follow_the_commander_status() {
        // Spawned commander is 0, a free barracks 1.
        let packet = ExShowFortressMapInfo::new(2, true, 4, &[true, false, true]);
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x7D)
            .int(2)
            .int(1)
            .int(4)
            .int(0)
            .int(1)
            .int(0)
            .done();
        check_round_trip(&packet, &expected, 3);
        // Four commanders: Java writes a 1 for the control room before the fourth status.
        let packet = ExShowFortressMapInfo::new(2, true, 4, &[true, true, false, true]);
        assert_eq!(packet.barracks, vec![0, 0, 1, 1, 0]);
        // Not in progress: zeros for every barracks.
        let packet = ExShowFortressMapInfo::new(2, false, 3, &[true, true, true]);
        assert_eq!(packet.barracks, vec![0, 0, 0]);
        check_round_trip(
            &packet,
            &Bytes::default()
                .byte(0xFE)
                .short(0x7D)
                .int(2)
                .int(0)
                .int(3)
                .int(0)
                .int(0)
                .int(0)
                .done(),
            3,
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Manor
// ---------------------------------------------------------------------------------------------

mod manor_client {
    use super::*;
    use l2_protocol_game::client::{
        CropSale, CropSaleItem, RequestBuySeed, RequestManorList, RequestProcureCropList, RequestSetCrop,
        RequestSetSeed, SeedPurchase, SeedSale,
    };

    #[test]
    fn manor_list_is_a_bare_ex_packet() {
        check_round_trip(
            &RequestManorList,
            &Bytes::default().byte(0xD0).short(0x01).done(),
            3,
        );
    }

    #[test]
    fn procure_crop_list_layout_and_bad_lists() {
        let packet = RequestProcureCropList {
            items: Some(vec![CropSaleItem {
                object_id: 5,
                item_id: 6,
                manor_id: 1,
                count: 7,
            }]),
        };
        let expected = Bytes::default()
            .byte(0xD0)
            .short(0x02)
            .int(1)
            .int(5)
            .int(6)
            .int(1)
            .long(7)
            .done();
        check_round_trip(&packet, &expected, 3);
        // An object id below 1 makes Java drop the whole list.
        let bad = Bytes::default()
            .byte(0xFE)
            .short(0x02)
            .int(1)
            .int(0)
            .int(6)
            .int(1)
            .long(7)
            .done();
        assert_eq!(
            RequestProcureCropList::decode(&mut PacketReader::new(&bad[3..])).unwrap(),
            RequestProcureCropList { items: None }
        );
        // A count of 2 for one entry's bytes: the length does not match, the list is dropped.
        let short = Bytes::default()
            .byte(0xFE)
            .short(0x02)
            .int(2)
            .int(5)
            .int(6)
            .int(1)
            .long(7)
            .done();
        assert_eq!(
            RequestProcureCropList::decode(&mut PacketReader::new(&short[3..])).unwrap(),
            RequestProcureCropList { items: None }
        );
    }

    #[test]
    fn set_seed_keeps_only_rows_with_sales() {
        let packet = RequestSetSeed {
            manor_id: 9,
            items: vec![SeedSale {
                item_id: 4,
                sales: 100,
                price: 200,
            }],
        };
        let expected = Bytes::default()
            .byte(0xD0)
            .short(0x03)
            .int(9)
            .int(1)
            .int(4)
            .long(100)
            .long(200)
            .done();
        check_round_trip(&packet, &expected, 3);
        // Two rows, the second with 0 sales: Java drops it.
        let body = Bytes::default()
            .byte(0xD0)
            .short(0x03)
            .int(9)
            .int(2)
            .int(4)
            .long(100)
            .long(200)
            .int(5)
            .long(0)
            .long(300)
            .done();
        assert_eq!(
            RequestSetSeed::decode(&mut PacketReader::new(&body[3..])).unwrap(),
            packet
        );
    }

    #[test]
    fn set_crop_layout_has_a_reward_byte() {
        let packet = RequestSetCrop {
            manor_id: 9,
            items: vec![CropSale {
                item_id: 4,
                sales: 100,
                price: 200,
                reward_type: 2,
            }],
        };
        let expected = Bytes::default()
            .byte(0xD0)
            .short(0x04)
            .int(9)
            .int(1)
            .int(4)
            .long(100)
            .long(200)
            .byte(2)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn buy_seed_layout_and_bad_list() {
        let packet = RequestBuySeed {
            manor_id: 2,
            items: Some(vec![SeedPurchase {
                item_id: 10,
                count: 3,
            }]),
        };
        let expected = Bytes::default().byte(0xC5).int(2).int(1).int(10).long(3).done();
        check_round_trip(&packet, &expected, 1);
        // A count of 0 for an entry: Java's null list, which the world answers with ActionFailed.
        let bad = Bytes::default().byte(0xC5).int(2).int(1).int(10).long(0).done();
        assert_eq!(
            RequestBuySeed::decode(&mut PacketReader::new(&bad[1..])).unwrap(),
            RequestBuySeed {
                manor_id: 2,
                items: None
            }
        );
    }
}

mod manor_server {
    use super::*;
    use l2_protocol_game::server::manor::{
        CropOffer, CropProcureInfo, CropSettingInfo, ExShowCropInfo, ExShowCropSetting,
        ExShowManorDefaultInfo, ExShowProcureCropDetail, ExShowSeedInfo, ExShowSeedSetting,
        ExShowSellCropList, ManorDefaultCrop, PeriodCropSales, PeriodSales, ProcureCastle,
        SeedProductionInfo, SeedSettingInfo, SellCropItem,
    };

    #[test]
    fn manor_list_layout() {
        use l2_protocol_game::server::manor::ExSendManorList;
        // Java: ExSendManorList.writeImpl: the count, then the id and the lower case name.
        let packet = ExSendManorList {
            castles: vec![(1, "gludio".to_owned()), (2, "dion".to_owned())],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x22)
            .int(2)
            .int(1)
            .string("gludio")
            .int(2)
            .string("dion")
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn buy_list_seed_layout() {
        use l2_protocol_game::server::manor::{BuyListSeed, SeedForSale};
        // Java: BuyListSeed.writeImpl: money, manor id, a short count, then one item row per seed.
        let packet = BuyListSeed {
            money: 5000,
            manor_id: 3,
            seeds: vec![SeedForSale {
                seed_id: 5016,
                amount: 40,
                price: 12,
            }],
        };
        let expected = Bytes::default()
            .byte(0xE9)
            .long(5000)
            .int(3)
            .short(1)
            .int(5016)
            .int(5016)
            .int(0)
            .long(40)
            .short(5)
            .short(0)
            .short(0)
            .int(0)
            .short(0)
            .short(0)
            .int(0)
            .int(-1)
            .int(-9999)
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
            .short(0)
            .long(12)
            .done();
        check_round_trip(&packet, &expected, 1);
        // An empty window is the money, the manor and a short 0.
        let empty = BuyListSeed {
            money: 1,
            manor_id: 2,
            seeds: Vec::new(),
        };
        let expected = Bytes::default().byte(0xE9).long(1).int(2).short(0).done();
        check_round_trip(&empty, &expected, 1);
    }

    #[test]
    fn default_crop_table_layout() {
        let packet = ExShowManorDefaultInfo {
            hide_buttons: true,
            crops: vec![ManorDefaultCrop {
                crop_id: 6,
                level: 1,
                seed_price: 20,
                crop_price: 30,
                reward_1: 50,
                reward_2: 60,
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x25)
            .byte(1)
            .int(1)
            .int(6)
            .int(1)
            .int(20)
            .int(30)
            .byte(1)
            .int(50)
            .byte(1)
            .int(60)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn seed_production_layout() {
        let packet = ExShowSeedInfo {
            hide_buttons: true,
            manor_id: 1,
            seeds: vec![SeedProductionInfo {
                seed_id: 5,
                left: 10,
                started: 20,
                price: 30,
                level: 40,
                reward_1: 50,
                reward_2: 60,
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x23)
            .byte(1)
            .int(1)
            .int(0)
            .int(1)
            .int(5)
            .long(10)
            .long(20)
            .long(30)
            .int(40)
            .byte(1)
            .int(50)
            .byte(1)
            .int(60)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn crop_production_layout_has_the_reward_byte_after_the_price() {
        let packet = ExShowCropInfo {
            hide_buttons: false,
            manor_id: 1,
            crops: vec![CropProcureInfo {
                crop_id: 6,
                left: 1,
                started: 2,
                price: 3,
                reward: 2,
                level: 4,
                reward_1: 5,
                reward_2: 6,
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x24)
            .byte(0)
            .int(1)
            .int(0)
            .int(1)
            .int(6)
            .long(1)
            .long(2)
            .long(3)
            .byte(2)
            .int(4)
            .byte(1)
            .int(5)
            .byte(1)
            .int(6)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn seed_setting_layout() {
        let packet = ExShowSeedSetting {
            manor_id: 1,
            seeds: vec![SeedSettingInfo {
                seed_id: 5,
                level: 40,
                reward_1: 50,
                reward_2: 60,
                sale_limit: 70,
                reference_price: 80,
                min_price: 90,
                max_price: 100,
                current: PeriodSales { amount: 1, price: 2 },
                next: PeriodSales { amount: 3, price: 4 },
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x26)
            .int(1)
            .int(1)
            .int(5)
            .int(40)
            .byte(1)
            .int(50)
            .byte(1)
            .int(60)
            .int(70)
            .int(80)
            .int(90)
            .int(100)
            .long(1)
            .long(2)
            .long(3)
            .long(4)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn crop_setting_layout_has_java_constant_zero() {
        let packet = ExShowCropSetting {
            manor_id: 1,
            crops: vec![CropSettingInfo {
                crop_id: 6,
                level: 4,
                reward_1: 5,
                reward_2: 6,
                sale_limit: 7,
                min_price: 8,
                max_price: 9,
                current: PeriodCropSales {
                    amount: 1,
                    price: 2,
                    reward: 3,
                },
                next: PeriodCropSales {
                    amount: 4,
                    price: 5,
                    reward: 6,
                },
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x2B)
            .int(1)
            .int(1)
            .int(6)
            .int(4)
            .byte(1)
            .int(5)
            .byte(1)
            .int(6)
            .int(7)
            // Java's `// ???` int, always 0.
            .int(0)
            .int(8)
            .int(9)
            .long(1)
            .long(2)
            .byte(3)
            .long(4)
            .long(5)
            .byte(6)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn procure_detail_layout() {
        let packet = ExShowProcureCropDetail {
            crop_id: 6,
            castles: vec![ProcureCastle {
                castle_id: 1,
                amount: 100,
                price: 200,
                reward: 2,
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x78)
            .int(6)
            .int(1)
            .int(1)
            .long(100)
            .long(200)
            .byte(2)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn sell_crop_list_uses_0xffffffff_for_no_procurement() {
        let packet = ExShowSellCropList {
            manor_id: 1,
            items: vec![
                SellCropItem {
                    object_id: 500,
                    item_id: 6,
                    seed_level: 4,
                    reward_1: 5,
                    reward_2: 6,
                    offer: Some(CropOffer {
                        amount: 100,
                        price: 200,
                        reward: 2,
                    }),
                    my_crops: 9,
                },
                SellCropItem {
                    object_id: 501,
                    item_id: 7,
                    seed_level: 1,
                    reward_1: 2,
                    reward_2: 3,
                    offer: None,
                    my_crops: 4,
                },
            ],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x2C)
            .int(1)
            .int(2)
            .int(500)
            .int(6)
            .int(4)
            .byte(1)
            .int(5)
            .byte(1)
            .int(6)
            .int(1)
            .long(100)
            .long(200)
            .byte(2)
            .long(9)
            .int(501)
            .int(7)
            .int(1)
            .byte(1)
            .int(2)
            .byte(1)
            .int(3)
            // No procurement: manor -1 (Java 0xFFFFFFFF), zeros, reward 0.
            .int(-1)
            .long(0)
            .long(0)
            .byte(0)
            .long(4)
            .done();
        check_round_trip(&packet, &expected, 3);
    }
}

// ---------------------------------------------------------------------------------------------
// Territory War
// ---------------------------------------------------------------------------------------------

mod territory {
    use super::*;
    use l2_protocol_game::client::{RequestDominionInfo, RequestJoinDominionWar};
    use l2_protocol_game::server::territory::{
        DominionTerritory, ExDominionWarEnd, ExDominionWarStart, ExReplyDominionInfo, ExShowDominionRegistry,
        TerritoryWards,
    };

    #[test]
    fn client_packets() {
        check_round_trip(
            &RequestJoinDominionWar {
                territory_id: 85,
                is_clan: 1,
                is_joining: 1,
            },
            &Bytes::default()
                .byte(0xD0)
                .short(0x57)
                .int(85)
                .int(1)
                .int(1)
                .done(),
            3,
        );
        check_round_trip(
            &RequestDominionInfo,
            &Bytes::default().byte(0xD0).short(0x58).done(),
            3,
        );
    }

    #[test]
    fn war_start_repeats_the_territory_only_when_disguised() {
        let disguised = ExDominionWarStart {
            object_id: 1000,
            territory_id: 85,
            is_disguised: true,
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0xA3)
            .int(1000)
            .int(1)
            .int(85)
            .int(1)
            .int(85)
            .done();
        check_round_trip(&disguised, &expected, 3);
        let plain = ExDominionWarStart {
            object_id: 1000,
            territory_id: 85,
            is_disguised: false,
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0xA3)
            .int(1000)
            .int(1)
            .int(85)
            .int(0)
            .int(0)
            .done();
        check_round_trip(&plain, &expected, 3);
    }

    #[test]
    fn war_end_is_a_bare_ex_packet() {
        check_round_trip(
            &ExDominionWarEnd,
            &Bytes::default().byte(0xFE).short(0xA4).done(),
            3,
        );
    }

    #[test]
    fn registration_window_layout() {
        let packet = ExShowDominionRegistry {
            castle_id: 5,
            owner_clan_name: "Own".into(),
            owner_leader_name: "Lead".into(),
            owner_ally_name: "Ally".into(),
            clan_requests: 3,
            merc_requests: 4,
            war_time: 100,
            current_time: 90,
            clan_registered: true,
            merc_registered: false,
            territories: vec![TerritoryWards {
                territory_id: 85,
                ward_ids: vec![1, 2],
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x90)
            // The territory id, 80 + castle id.
            .int(85)
            .string("Own")
            .string("Lead")
            .string("Ally")
            .int(3)
            .int(4)
            .int(100)
            .int(90)
            .int(1)
            .int(0)
            // Java's constant 1.
            .int(1)
            .int(1)
            .int(85)
            .int(2)
            .int(1)
            .int(2)
            .done();
        check_round_trip(&packet, &expected, 3);
    }

    #[test]
    fn territory_list_layout() {
        let packet = ExReplyDominionInfo {
            territories: vec![DominionTerritory {
                territory_id: 85,
                name: "castle_dominion".into(),
                owner_clan_name: "Own".into(),
                ward_ids: vec![1],
                war_start_secs: 100,
            }],
        };
        let expected = Bytes::default()
            .byte(0xFE)
            .short(0x92)
            .int(1)
            .int(85)
            .string("castle_dominion")
            .string("Own")
            .int(1)
            .int(1)
            .int(100)
            .done();
        check_round_trip(&packet, &expected, 3);
    }
}

// ---------------------------------------------------------------------------------------------
// Item auction
// ---------------------------------------------------------------------------------------------

mod auction {
    use super::*;
    use l2_protocol_game::client::{RequestBidItemAuction, RequestInfoItemAuction};
    use l2_protocol_game::server::auction::{ExItemAuctionInfo, ItemAuctionNext};
    use l2_protocol_game::server::items::ItemInfo;

    #[test]
    fn client_packets() {
        check_round_trip(
            &RequestBidItemAuction {
                instance_id: 3,
                bid: 1000,
            },
            &Bytes::default().byte(0xD0).short(0x39).int(3).long(1000).done(),
            3,
        );
        check_round_trip(
            &RequestInfoItemAuction { instance_id: 3 },
            &Bytes::default().byte(0xD0).short(0x3A).int(3).done(),
            3,
        );
    }

    #[test]
    fn item_auction_starts_with_the_inverse_refresh_byte() {
        let item = ItemInfo::plain(268_435_457, 57, 5);
        let packet = ExItemAuctionInfo {
            refresh: false,
            instance_id: 2,
            bid: 500,
            time_remaining: 60,
            item: item.clone(),
            next: None,
        };
        let bytes = packet.to_bytes();
        // EX_ITEM_AUCTION_INFO, then `!refresh` as one byte, then the instance id.
        assert_eq!(&bytes[..3], &[0xFE, 0x68, 0x00]);
        assert_eq!(bytes[3], 1, "refresh is false, so Java writes 1");
        assert_eq!(&bytes[4..8], &2_i32.to_le_bytes());
        check_round_trip(&packet, &bytes, 3);
    }

    #[test]
    fn item_auction_with_the_next_auction() {
        let packet = ExItemAuctionInfo {
            refresh: true,
            instance_id: 2,
            bid: 500,
            time_remaining: 60,
            item: ItemInfo::plain(268_435_457, 57, 5),
            next: Some(ItemAuctionNext {
                init_bid: 300,
                start_secs: 1234,
                item: ItemInfo::plain(268_435_458, 58, 1),
            }),
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[3], 0, "refresh is true, so Java writes 0");
        check_round_trip(&packet, &bytes, 3);
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
    fn residence_packets_reach_their_variants() {
        let p = RequestSiegeInfo;
        dispatches(&p, ClientPacket::RequestSiegeInfo(p));
        let p = RequestSiegeAttackerList { castle_id: 5 };
        dispatches(&p, ClientPacket::RequestSiegeAttackerList(p.clone()));
        let p = RequestSiegeDefenderList { castle_id: 5 };
        dispatches(&p, ClientPacket::RequestSiegeDefenderList(p.clone()));
        let p = RequestJoinSiege {
            castle_id: 5,
            is_attacker: 1,
            is_joining: 1,
        };
        dispatches(&p, ClientPacket::RequestJoinSiege(p.clone()));
        let p = RequestConfirmSiegeWaitingList {
            castle_id: 5,
            clan_id: 6,
            approved: 1,
        };
        dispatches(&p, ClientPacket::RequestConfirmSiegeWaitingList(p.clone()));
        let p = RequestAllFortressInfo;
        dispatches(&p, ClientPacket::RequestAllFortressInfo(p));
        let p = RequestFortressSiegeInfo;
        dispatches(&p, ClientPacket::RequestFortressSiegeInfo(p));
        let p = RequestFortressMapInfo { fortress_id: 3 };
        dispatches(&p, ClientPacket::RequestFortressMapInfo(p.clone()));
        let p = RequestManorList;
        dispatches(&p, ClientPacket::RequestManorList(p));
        let p = RequestProcureCropList { items: None };
        dispatches(&p, ClientPacket::RequestProcureCropList(p.clone()));
        let p = RequestSetSeed {
            manor_id: 5,
            items: Vec::new(),
        };
        dispatches(&p, ClientPacket::RequestSetSeed(p.clone()));
        let p = RequestSetCrop {
            manor_id: 5,
            items: Vec::new(),
        };
        dispatches(&p, ClientPacket::RequestSetCrop(p.clone()));
        let p = RequestBuySeed {
            manor_id: 5,
            items: None,
        };
        dispatches(&p, ClientPacket::RequestBuySeed(p.clone()));
        let p = RequestJoinDominionWar {
            territory_id: 85,
            is_clan: 0,
            is_joining: 0,
        };
        dispatches(&p, ClientPacket::RequestJoinDominionWar(p.clone()));
        let p = RequestDominionInfo;
        dispatches(&p, ClientPacket::RequestDominionInfo(p));
        let p = RequestBidItemAuction {
            instance_id: 1,
            bid: 2,
        };
        dispatches(&p, ClientPacket::RequestBidItemAuction(p.clone()));
        let p = RequestInfoItemAuction { instance_id: 1 };
        dispatches(&p, ClientPacket::RequestInfoItemAuction(p.clone()));
    }
}
