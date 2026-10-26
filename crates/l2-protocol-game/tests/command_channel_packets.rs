//! Command channel packets (M6-FU-13) against byte layouts worked out by hand from the Java
//! `writeImpl` / `readImpl` code, plus round trips and the dispatcher.
//!
//! The expected bytes are built with an independent little-endian builder (not the crate's
//! `PacketWriter`), so a mistake in the crate's encoder cannot hide behind the same mistake here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    RequestExAcceptJoinMpcc, RequestExAskJoinMpcc, RequestExMpccShowPartyMembersInfo, RequestExOustFromMpcc,
};
use l2_protocol_game::server::{
    ExAskJoinMpcc, ExCloseMpcc, ExMpccPartyInfoUpdate, ExMpccShowPartyMemberInfo,
    ExMultiPartyCommandChannelInfo, ExOpenMpcc, MpccMember, MpccPartyEntry,
};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

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

/// `0xFE` and the `u16` ex id, which `ServerPackets.writeId` writes for the ex packets.
fn ex_server(id: i16) -> Bytes {
    Bytes::default().byte(0xFE).short(id)
}

/// `0xD0` and the `u16` ex id of a client ex packet.
fn ex_client(id: i16) -> Bytes {
    Bytes::default().byte(0xD0).short(id)
}

fn check_round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P, expected: &[u8]) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected, "encoded bytes");
    // The opcode (0xFE) and the u16 ex id come first.
    let decoded = P::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
}

#[test]
fn window_packets_have_no_body() {
    // ServerPackets: EX_OPEN_MPCC (0xFE, 0x12), EX_CLOSE_MPCC (0xFE, 0x13).
    check_round_trip(&ExOpenMpcc, &ex_server(0x12).done());
    check_round_trip(&ExCloseMpcc, &ex_server(0x13).done());
}

#[test]
fn ask_join_mpcc_is_the_requestor_name() {
    // EX_ASK_JOIN_MPCC (0xFE, 0x1A): writeString(requestorName).
    check_round_trip(
        &ExAskJoinMpcc {
            requestor_name: "Zwerg".into(),
        },
        &ex_server(0x1A).string("Zwerg").done(),
    );
}

#[test]
fn party_info_update_is_name_leader_count_mode() {
    // EX_MPCC_PARTY_INFO_UPDATE (0xFE, 0x5B): name, leader oid, member count, mode.
    check_round_trip(
        &ExMpccPartyInfoUpdate {
            leader_name: "Anna".into(),
            leader_object_id: 0x1000_0001,
            member_count: 7,
            mode: 1,
        },
        &ex_server(0x5B)
            .string("Anna")
            .int(0x1000_0001)
            .int(7)
            .int(1)
            .done(),
    );
}

#[test]
fn show_party_member_info_lists_name_object_class() {
    // EX_MPCC_SHOW_PARTY_MEMBER_INFO (0xFE, 0x4B): count, then name, objectId, classId each.
    check_round_trip(
        &ExMpccShowPartyMemberInfo {
            members: vec![
                MpccMember {
                    name: "Anna".into(),
                    object_id: 1,
                    class_id: 88,
                },
                MpccMember {
                    name: "Bo".into(),
                    object_id: 2,
                    class_id: 10,
                },
            ],
        },
        &ex_server(0x4B)
            .int(2)
            .string("Anna")
            .int(1)
            .int(88)
            .string("Bo")
            .int(2)
            .int(10)
            .done(),
    );
    check_round_trip(
        &ExMpccShowPartyMemberInfo { members: vec![] },
        &ex_server(0x4B).int(0).done(),
    );
}

#[test]
fn multi_party_channel_info() {
    // EX_MULTI_PARTY_COMMAND_CHANNEL_INFO (0xFE, 0x31): leader name, 0 (loot), member count,
    // party count, then leader name, leader oid, member count of each party.
    check_round_trip(
        &ExMultiPartyCommandChannelInfo {
            leader_name: "Anna".into(),
            channel_loot: 0,
            member_count: 5,
            parties: vec![
                MpccPartyEntry {
                    leader_name: "Anna".into(),
                    leader_object_id: 1,
                    member_count: 3,
                },
                MpccPartyEntry {
                    leader_name: "Cy".into(),
                    leader_object_id: 3,
                    member_count: 2,
                },
            ],
        },
        &ex_server(0x31)
            .string("Anna")
            .int(0)
            .int(5)
            .int(2)
            .string("Anna")
            .int(1)
            .int(3)
            .string("Cy")
            .int(3)
            .int(2)
            .done(),
    );
}

#[test]
fn a_negative_count_is_an_error() {
    let bytes = Bytes::default().int(-1).done();
    assert!(ExMpccShowPartyMemberInfo::decode(&mut PacketReader::new(&bytes)).is_err());
    let bytes = Bytes::default().string("A").int(0).int(0).int(-4).done();
    assert!(ExMultiPartyCommandChannelInfo::decode(&mut PacketReader::new(&bytes)).is_err());
}

/// The client packets: `0xD0`, the ex id, then what `readImpl` reads.
#[test]
fn client_packets_round_trip_and_dispatch() {
    let cases: Vec<(Vec<u8>, ClientPacket)> = vec![
        (
            ex_client(0x06).string("Zwerg").done(),
            ClientPacket::RequestExAskJoinMpcc(RequestExAskJoinMpcc { name: "Zwerg".into() }),
        ),
        (
            ex_client(0x07).int(1).done(),
            ClientPacket::RequestExAcceptJoinMpcc(RequestExAcceptJoinMpcc { response: 1 }),
        ),
        (
            ex_client(0x08).string("Zwerg").done(),
            ClientPacket::RequestExOustFromMpcc(RequestExOustFromMpcc { name: "Zwerg".into() }),
        ),
        (
            ex_client(0x2D).int(0x1000_0001).done(),
            ClientPacket::RequestExMpccShowPartyMembersInfo(RequestExMpccShowPartyMembersInfo {
                party_leader_id: 0x1000_0001,
            }),
        ),
    ];
    for (bytes, expected) in cases {
        let decode = |state| decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decode(ConnectionState::InGame).as_ref(), Some(&expected));
        assert_eq!(decode(ConnectionState::Connected), None);
        // And the encoder writes the same bytes back.
        let again = match &expected {
            ClientPacket::RequestExAskJoinMpcc(p) => p.to_bytes(),
            ClientPacket::RequestExAcceptJoinMpcc(p) => p.to_bytes(),
            ClientPacket::RequestExOustFromMpcc(p) => p.to_bytes(),
            ClientPacket::RequestExMpccShowPartyMembersInfo(p) => p.to_bytes(),
            other => panic!("{other:?}"),
        };
        assert_eq!(again, bytes);
    }
}

#[test]
fn a_short_client_packet_is_truncated() {
    let bytes = ex_client(0x07).done();
    let result = decode_client(
        ConnectionState::InGame,
        bytes[0],
        &mut PacketReader::new(&bytes[1..]),
    );
    assert!(result.is_err());
}
