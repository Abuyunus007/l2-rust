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

// Silence the unused-helper warnings of the builder methods the later groups use.
#[allow(dead_code)]
fn _builder_is_complete(b: Bytes) -> Vec<u8> {
    b.long(0).raw(&[]).done()
}
