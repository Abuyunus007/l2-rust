//! The duel packets (M6-DUEL-01) against byte layouts worked out by hand from the Java
//! `readImpl` / `writeImpl` code, plus round trips and the dispatcher.
//!
//! No recorded Java session holds a duel yet, so the layouts below are the spec. The expected
//! bytes are built with an independent little-endian builder (not the crate's `PacketWriter`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{RequestDuelAnswerStart, RequestDuelStart, RequestDuelSurrender};
use l2_protocol_game::server::{
    ExDuelAskStart, ExDuelEnd, ExDuelReady, ExDuelStart, ExDuelUpdateUserInfo, PlaySound,
};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

/// Java `writeInt`.
fn int(v: i32) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// Java `writeString`: UTF-16LE, zero terminated.
fn string(text: &str) -> Vec<u8> {
    let mut out: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    out.extend([0, 0]);
    out
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

/// The id byte and the u16 ex id of `EX_DUEL_*` (`ServerPackets`).
fn ex(id: u16) -> Vec<u8> {
    let mut out = vec![0xFE];
    out.extend(id.to_le_bytes());
    out
}

#[test]
fn request_duel_start() {
    // 0xD0 0x001B, readString, readInt.
    let packet = RequestDuelStart {
        player: "Bob".into(),
        party_duel: 1,
    };
    let mut expected = client_ex(0x1B);
    expected.extend(string("Bob"));
    expected.extend(int(1));
    check(&packet, &expected, 3);
}

#[test]
fn request_duel_answer_start() {
    // 0xD0 0x001C (REQUEST_DUAL_ANSWER_START), partyDuel, unk1, response.
    let packet = RequestDuelAnswerStart {
        party_duel: 1,
        unk1: 0,
        response: -1,
    };
    let mut expected = client_ex(0x1C);
    expected.extend(int(1));
    expected.extend(int(0));
    expected.extend(int(-1));
    check(&packet, &expected, 3);
}

#[test]
fn request_duel_surrender() {
    check(&RequestDuelSurrender, &client_ex(0x45), 3);
}

#[test]
fn ex_duel_ask_start() {
    let packet = ExDuelAskStart {
        requestor_name: "Alice".into(),
        party_duel: 0,
    };
    let mut expected = ex(0x4C);
    expected.extend(string("Alice"));
    expected.extend(int(0));
    check(&packet, &expected, 3);
}

#[test]
fn ex_duel_ready_start_end_write_the_party_flag_as_an_int() {
    for (party, flag) in [(false, 0), (true, 1)] {
        let mut expected = ex(0x4D);
        expected.extend(int(flag));
        check(&ExDuelReady { party_duel: party }, &expected, 3);

        let mut expected = ex(0x4E);
        expected.extend(int(flag));
        check(&ExDuelStart { party_duel: party }, &expected, 3);

        let mut expected = ex(0x4F);
        expected.extend(int(flag));
        check(&ExDuelEnd { party_duel: party }, &expected, 3);
    }
}

#[test]
fn ex_duel_update_user_info() {
    // name, objectId, classId, level, hp, maxHp, mp, maxMp, cp, maxCp.
    let packet = ExDuelUpdateUserInfo {
        name: "Bob".into(),
        object_id: 0x2000_0001,
        class_id: 88,
        level: 76,
        current_hp: 1000,
        max_hp: 2000,
        current_mp: 300,
        max_mp: 400,
        current_cp: 50,
        max_cp: 60,
    };
    let mut expected = ex(0x50);
    expected.extend(string("Bob"));
    for v in [0x2000_0001, 88, 76, 1000, 2000, 300, 400, 50, 60] {
        expected.extend(int(v));
    }
    check(&packet, &expected, 3);
}

#[test]
fn the_dispatcher_decodes_the_duel_packets_in_game_only() {
    let start = RequestDuelStart {
        player: "Bob".into(),
        party_duel: 0,
    };
    let answer = RequestDuelAnswerStart {
        party_duel: 0,
        unk1: 0,
        response: 1,
    };
    let cases = [
        (start.to_bytes(), ClientPacket::RequestDuelStart(start)),
        (answer.to_bytes(), ClientPacket::RequestDuelAnswerStart(answer)),
        (
            RequestDuelSurrender.to_bytes(),
            ClientPacket::RequestDuelSurrender(RequestDuelSurrender),
        ),
    ];
    for (bytes, expected) in cases {
        let decode = |state| decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decode(ConnectionState::InGame), Some(expected));
        assert_eq!(decode(ConnectionState::Connected), None);
    }
}

#[test]
fn play_sound_of_the_duel_start() {
    // 0x9E, unknown1 = 1, the file, then unknown3..unknown8 (all 0).
    let packet = PlaySound::with_unknown1(1, "B04_S01");
    let mut expected = vec![0x9E];
    expected.extend(int(1));
    expected.extend(string("B04_S01"));
    for _ in 0..6 {
        expected.extend(int(0));
    }
    check(&packet, &expected, 1);

    // `new PlaySound(soundFile)` has unknown1 = 0.
    assert_eq!(PlaySound::file("x").unknown1, 0);
}
