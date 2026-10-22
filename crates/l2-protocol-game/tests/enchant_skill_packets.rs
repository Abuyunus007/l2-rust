//! The skill enchant packets (M6-ENCHANT-02) against byte layouts worked out by hand from the Java
//! `readImpl` / `writeImpl` code, plus round trips and the dispatcher.
//!
//! No recorded Java session holds a skill enchant yet, so the layouts below are the spec. The ex
//! ids are the ones of the generated `ExClientPackets` / `ServerPackets` tables. The expected bytes
//! are built with an independent little-endian builder (not the crate's `PacketWriter`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    RequestExEnchantSkill, RequestExEnchantSkillInfo, RequestExEnchantSkillInfoDetail,
    RequestExEnchantSkillRouteChange, RequestExEnchantSkillSafe, RequestExEnchantSkillUntrain,
};
use l2_protocol_game::server::{
    ExEnchantSkillInfo, ExEnchantSkillInfoDetail, ExEnchantSkillList, ExEnchantSkillResult,
};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

/// Java `writeInt`.
fn int(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn check<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P, expected: &[u8], opcode_len: usize) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected, "encoded bytes");
    let decoded = P::decode(&mut PacketReader::new(&bytes[opcode_len..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
}

/// `0xD0` and the u16 ex id of a client ex packet.
fn client_ex(id: u16) -> Vec<u8> {
    let mut out = vec![0xD0];
    out.extend(id.to_le_bytes());
    out
}

/// `0xFE` and the u16 ex id of a server ex packet.
fn ex(id: u16) -> Vec<u8> {
    let mut out = vec![0xFE];
    out.extend(id.to_le_bytes());
    out
}

#[test]
fn the_requests_read_their_ints() {
    // readInt skill id, readInt skill level.
    for (id, packet) in [
        (
            0x0E,
            RequestExEnchantSkillInfo {
                skill_id: 1,
                skill_level: 101,
            }
            .to_bytes(),
        ),
        (
            0x0F,
            RequestExEnchantSkill {
                skill_id: 1,
                skill_level: 101,
            }
            .to_bytes(),
        ),
        (
            0x32,
            RequestExEnchantSkillSafe {
                skill_id: 1,
                skill_level: 101,
            }
            .to_bytes(),
        ),
        (
            0x33,
            RequestExEnchantSkillUntrain {
                skill_id: 1,
                skill_level: 101,
            }
            .to_bytes(),
        ),
        (
            0x34,
            RequestExEnchantSkillRouteChange {
                skill_id: 1,
                skill_level: 101,
            }
            .to_bytes(),
        ),
    ] {
        let mut expected = client_ex(id);
        expected.extend(int(1));
        expected.extend(int(101));
        assert_eq!(packet, expected, "ex id {id:#x}");
    }
}

#[test]
fn request_info_detail_has_the_type_first() {
    // readInt type, readInt skill id, readInt skill level.
    let mut expected = client_ex(0x46);
    expected.extend(int(3));
    expected.extend(int(1331));
    expected.extend(int(207));
    check(
        &RequestExEnchantSkillInfoDetail {
            kind: 3,
            skill_id: 1331,
            skill_level: 207,
        },
        &expected,
        3,
    );
}

#[test]
fn the_requests_ignore_the_padding_of_the_client() {
    let mut body = int(5);
    body.extend(int(6));
    body.extend(int(99));
    let decoded = RequestExEnchantSkill::decode(&mut PacketReader::new(&body)).unwrap();
    assert_eq!((decoded.skill_id, decoded.skill_level), (5, 6));
    assert!(RequestExEnchantSkill::decode(&mut PacketReader::new(&int(5))).is_err());
}

#[test]
fn ex_enchant_skill_info() {
    // writeInt id, level, !maxEnchanted, level > 100, routes.size(), each route.
    let mut expected = ex(0x2A);
    for v in [1, 101, 1, 1, 3, 101, 201, 301] {
        expected.extend(int(v));
    }
    check(
        &ExEnchantSkillInfo {
            skill_id: 1,
            level: 101,
            max_enchanted: false,
            routes: vec![101, 201, 301],
        },
        &expected,
        3,
    );

    // A skill at its last step is "max enchanted": 0 written. A plain skill is not "enchanted".
    let mut expected = ex(0x2A);
    for v in [7, 40, 0, 0, 1, 101] {
        expected.extend(int(v));
    }
    check(
        &ExEnchantSkillInfo {
            skill_id: 7,
            level: 40,
            max_enchanted: true,
            routes: vec![101],
        },
        &expected,
        3,
    );
}

#[test]
fn ex_enchant_skill_info_detail() {
    // writeInt type, id, level, sp * multi, chance, 2, ADENA_ID, adena, bookId, reqCount.
    let mut expected = ex(0x5E);
    for v in [1, 1, 102, 5 * 575_980, 80, 2, 57, 5 * 74_250, 9627, 1] {
        expected.extend(int(v));
    }
    check(
        &ExEnchantSkillInfoDetail {
            kind: 1,
            skill_id: 1,
            skill_level: 102,
            sp: 5 * 575_980,
            chance: 80,
            adena: 5 * 74_250,
            book_id: 9627,
            book_required: true,
        },
        &expected,
        3,
    );
}

#[test]
fn ex_enchant_skill_list_and_result() {
    // writeInt type.ordinal(), size, then id and level of each.
    let mut expected = ex(0x29);
    for v in [3, 2, 1, 101, 5, 203] {
        expected.extend(int(v));
    }
    check(
        &ExEnchantSkillList {
            kind: 3,
            skills: vec![(1, 101), (5, 203)],
        },
        &expected,
        3,
    );

    // writeInt(boolean).
    let mut expected = ex(0xA7);
    expected.extend(int(1));
    check(&ExEnchantSkillResult { enchanted: true }, &expected, 3);
    let mut expected = ex(0xA7);
    expected.extend(int(0));
    check(&ExEnchantSkillResult { enchanted: false }, &expected, 3);
}

#[test]
fn the_dispatcher_decodes_the_requests_in_game_only() {
    let detail = RequestExEnchantSkillInfoDetail {
        kind: 2,
        skill_id: 9,
        skill_level: 100,
    };
    let info = RequestExEnchantSkillInfo {
        skill_id: 9,
        skill_level: 101,
    };
    let enchant = RequestExEnchantSkill {
        skill_id: 9,
        skill_level: 101,
    };
    let safe = RequestExEnchantSkillSafe {
        skill_id: 9,
        skill_level: 102,
    };
    let untrain = RequestExEnchantSkillUntrain {
        skill_id: 9,
        skill_level: 101,
    };
    let route = RequestExEnchantSkillRouteChange {
        skill_id: 9,
        skill_level: 203,
    };
    let cases = [
        (info.to_bytes(), ClientPacket::RequestExEnchantSkillInfo(info)),
        (
            detail.to_bytes(),
            ClientPacket::RequestExEnchantSkillInfoDetail(detail),
        ),
        (enchant.to_bytes(), ClientPacket::RequestExEnchantSkill(enchant)),
        (safe.to_bytes(), ClientPacket::RequestExEnchantSkillSafe(safe)),
        (
            untrain.to_bytes(),
            ClientPacket::RequestExEnchantSkillUntrain(untrain),
        ),
        (
            route.to_bytes(),
            ClientPacket::RequestExEnchantSkillRouteChange(route),
        ),
    ];
    for (bytes, expected) in cases {
        let decode = |state| decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decode(ConnectionState::InGame), Some(expected));
        assert_eq!(decode(ConnectionState::Connected), None);
    }
}
