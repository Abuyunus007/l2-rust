//! Character screen packets against byte layouts worked out by hand from the Java `writeImpl`
//! code, and against recorded Java game server sessions when `L2_CAPTURE_DIR` is set.
//!
//! The expected bytes below are built with a tiny writer of their own (not the crate's
//! `PacketWriter`), listing the fields in the order of the Java source, so a mistake in the
//! crate's encoder cannot hide behind the same mistake in the test.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::*;
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

/// Independent little-endian builder.
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
    fn ints(mut self, vs: &[i32]) -> Self {
        for v in vs {
            self.0.extend(v.to_le_bytes());
        }
        self
    }
    fn long(mut self, v: i64) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn double(mut self, v: f64) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn zeros(mut self, n: usize) -> Self {
        self.0.extend(std::iter::repeat_n(0, n));
        self
    }
    /// Java `writeString`: UTF-16LE plus a 0x0000 terminator.
    fn text(mut self, s: &str) -> Self {
        for unit in s.encode_utf16() {
            self.0.extend(unit.to_le_bytes());
        }
        self.0.extend([0, 0]);
        self
    }
}

#[test]
fn char_selection_info_matches_the_java_layout() {
    // Item ids by slot; the Java wire order is
    // UNDER REAR LEAR NECK RFINGER LFINGER HEAD RHAND LHAND GLOVES CHEST LEGS FEET CLOAK RHAND
    // HAIR HAIR2 RBRACELET LBRACELET DECO1..DECO6 BELT.
    let mut items = [0; paperdoll::TOTAL_SLOTS];
    items[paperdoll::UNDER] = 1;
    items[paperdoll::REAR] = 2;
    items[paperdoll::LEAR] = 3;
    items[paperdoll::NECK] = 4;
    items[paperdoll::RFINGER] = 5;
    items[paperdoll::LFINGER] = 6;
    items[paperdoll::HEAD] = 7;
    items[paperdoll::RHAND] = 8;
    items[paperdoll::LHAND] = 9;
    items[paperdoll::GLOVES] = 10;
    items[paperdoll::CHEST] = 11;
    items[paperdoll::LEGS] = 12;
    items[paperdoll::FEET] = 13;
    items[paperdoll::CLOAK] = 14;
    items[paperdoll::HAIR] = 15;
    items[paperdoll::HAIR2] = 16;
    items[paperdoll::RBRACELET] = 17;
    items[paperdoll::LBRACELET] = 18;
    items[paperdoll::DECO1] = 19;
    items[paperdoll::DECO2] = 20;
    items[paperdoll::DECO3] = 21;
    items[paperdoll::DECO4] = 22;
    items[paperdoll::DECO5] = 23;
    items[paperdoll::DECO6] = 24;
    items[paperdoll::BELT] = 25;

    let packet = CharSelectionInfo {
        account_name: "acc".into(),
        session_id: 0x0A0B_0C0D,
        max_characters: 7,
        characters: vec![CharSelectEntry {
            name: "Ab".into(),
            object_id: 268_435_457,
            clan_id: 9,
            sex: 1,
            race: 3,
            base_class_id: 31,
            x: -1,
            y: 2,
            z: 3,
            current_hp: 1.5,
            current_mp: 2.5,
            sp: 4,
            exp: 5_000_000_000,
            exp_percent: 0.25,
            level: 40,
            karma: 6,
            pk_kills: 7,
            pvp_kills: 8,
            paperdoll: items,
            hair_style: 3,
            hair_color: 2,
            face: 1,
            max_hp: 3.5,
            max_mp: 4.5,
            delete_seconds: 86_400,
            class_id: 32,
            selected: true,
            enchant_effect: 16,
            augmentation_id: 12_345,
            vitality_points: 17_000,
        }],
    };

    let expected = Bytes::default()
        .byte(0x09) // CHARACTER_SELECTION_INFO
        .int(1) // character count
        .int(7) // CharMaxNumber
        .byte(0)
        // --- entry ---
        .text("Ab")
        .int(268_435_457)
        .text("acc")
        .int(0x0A0B_0C0D)
        .int(9) // clan id
        .int(0) // builder level
        .ints(&[1, 3, 31]) // sex, race, base class
        .int(1) // "GameServerName"
        .ints(&[-1, 2, 3]) // x y z
        .double(1.5)
        .double(2.5)
        .int(4) // sp
        .long(5_000_000_000)
        .double(0.25)
        .ints(&[40, 6, 7, 8]) // level, karma, pk, pvp
        .zeros(7 * 4)
        .ints(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]) // UNDER .. CLOAK
        .int(8) // RHAND again
        .ints(&[15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25]) // HAIR .. BELT
        .ints(&[3, 2, 1]) // hair style, colour, face
        .double(3.5)
        .double(4.5)
        .int(86_400)
        .int(32) // class id
        .int(1) // selected
        .byte(16) // enchant
        .int(12_345)
        .zeros(5 * 4) // transformation, pet ints
        .zeros(2 * 8) // pet hp, mp
        .int(17_000)
        .0;

    assert_eq!(packet.to_bytes(), expected);
    let decoded = CharSelectionInfo::decode(&mut PacketReader::new(&expected[1..])).unwrap();
    assert_eq!(decoded, packet);
}

#[test]
fn char_selected_matches_the_java_layout() {
    let packet = CharSelected {
        name: "Ab".into(),
        object_id: 268_435_457,
        title: "T".into(),
        session_id: 0x0A0B_0C0D,
        clan_id: 9,
        is_female: true,
        race: 3,
        class_id: 31,
        x: -1,
        y: 2,
        z: 3,
        current_hp: 1.5,
        current_mp: 2.5,
        sp: 4,
        exp: 5_000_000_000,
        level: 40,
        karma: 6,
        pk_kills: 7,
        int: 21,
        str: 41,
        con: 43,
        men: 25,
        dex: 30,
        wit: 11,
        game_time: 1_000,
    };
    let expected = Bytes::default()
        .byte(0x0B) // CHARACTER_SELECTED
        .text("Ab")
        .int(268_435_457)
        .text("T")
        .int(0x0A0B_0C0D)
        .int(9)
        .int(0)
        .ints(&[1, 3, 31]) // female, race, class
        .int(1)
        .ints(&[-1, 2, 3])
        .double(1.5)
        .double(2.5)
        .int(4)
        .long(5_000_000_000)
        .ints(&[40, 6, 7]) // level, karma, pk kills
        .ints(&[21, 41, 43, 25, 30, 11]) // INT STR CON MEN DEX WIT
        .int(1_000) // game time
        .int(0)
        .int(31) // class id again
        .zeros(4 * 4)
        .zeros(64)
        .int(0)
        .0;
    assert_eq!(packet.to_bytes(), expected);
    assert_eq!(
        CharSelected::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        packet
    );
}

#[test]
fn new_character_success_for_the_eleven_java_templates() {
    // Java sends 11 templates; the layout is 20 ints each.
    let templates: Vec<_> = (0..11)
        .map(|i| NewCharTemplate {
            race: i / 2,
            class_id: i,
            base_str: 40,
            base_dex: 30,
            base_con: 43,
            base_int: 21,
            base_wit: 11,
            base_men: 25,
        })
        .collect();
    let bytes = NewCharacterSuccess { templates }.to_bytes();
    assert_eq!(bytes.len(), 1 + 4 + 11 * 20 * 4);
    // Template 3: race 1, class 3, then STR block: 0x46, 40, 0x0A.
    let at = 1 + 4 + 3 * 80;
    let expected = Bytes::default().ints(&[1, 3, 0x46, 40, 0x0A, 0x46, 30, 0x0A]).0;
    assert_eq!(bytes[at..at + 32], expected[..]);
}

#[test]
fn key_packet_for_a_rejected_protocol() {
    // Java: `new KeyPacket(key, 0)` when the protocol version is not in the list.
    let packet = KeyPacket {
        result: 0,
        key: [0xAA; 8],
        use_encryption: true,
        server_id: 2,
    };
    let expected = Bytes::default()
        .byte(0x2E)
        .byte(0)
        .0
        .into_iter()
        .chain([0xAA; 8])
        .chain(Bytes::default().int(1).int(2).byte(1).int(0).0)
        .collect::<Vec<u8>>();
    assert_eq!(packet.to_bytes(), expected);
}

// ---------------------------------------------------------------------------------------------
// Capture-driven check against the owner's Java sessions.
// ---------------------------------------------------------------------------------------------

/// Server packets that must re-encode to exactly the recorded bytes.
fn check_server(opcode: u8, raw: &[u8]) -> Option<bool> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0x2E => KeyPacket::decode(body).ok()?.to_bytes(),
        0x09 => CharSelectionInfo::decode(body).ok()?.to_bytes(),
        0x0B => CharSelected::decode(body).ok()?.to_bytes(),
        0x0D => NewCharacterSuccess::decode(body).ok()?.to_bytes(),
        0x0F => CharCreateOk::decode(body).ok()?.to_bytes(),
        0x10 => CharCreateFail::decode(body).ok()?.to_bytes(),
        0x1D => CharDeleteSuccess::decode(body).ok()?.to_bytes(),
        0x1E => CharDeleteFail::decode(body).ok()?.to_bytes(),
        0x20 => ServerClose::decode(body).ok()?.to_bytes(),
        _ => return None,
    };
    Some(encoded == raw)
}

fn state_from_name(name: &str) -> Option<ConnectionState> {
    ConnectionState::ALL.iter().copied().find(|s| s.name() == name)
}

/// Client packets: decode with the dispatcher, encode again, expect the recorded bytes (the
/// client may pad, so the recorded packet may be longer).
fn check_client(state: Option<&str>, raw: &[u8]) -> Option<bool> {
    // The recorded state is only a guess, so fall back to trying all of them.
    let states: Vec<ConnectionState> = match state.and_then(state_from_name) {
        Some(s) => vec![s],
        None => ConnectionState::ALL.to_vec(),
    };
    for state in states {
        let Ok(Some(packet)) = decode_client(state, raw[0], &mut PacketReader::new(&raw[1..])) else {
            continue;
        };
        let encoded = match packet {
            ClientPacket::ProtocolVersion(p) => p.to_bytes(),
            ClientPacket::AuthLogin(p) => p.to_bytes(),
            ClientPacket::NewCharacter(p) => p.to_bytes(),
            ClientPacket::CharacterCreate(p) => p.to_bytes(),
            ClientPacket::CharacterDelete(p) => p.to_bytes(),
            ClientPacket::CharacterRestore(p) => p.to_bytes(),
            ClientPacket::CharacterSelect(p) => p.to_bytes(),
            ClientPacket::RequestGotoLobby(p) => p.to_bytes(),
            ClientPacket::EnterWorld(p) => p.to_bytes(),
            ClientPacket::RequestKeyMapping(p) => p.to_bytes(),
            // Chat packets are checked in tests/chat.rs.
            // Chat packets are checked in tests/chat.rs, interaction packets in tests/interaction.rs.
            ClientPacket::Say2(_)
            | ClientPacket::Logout(_)
            | ClientPacket::RequestRestart(_)
            | ClientPacket::RequestNetPing(_)
            | ClientPacket::Action(_)
            | ClientPacket::RequestTargetCanceld(_)
            | ClientPacket::RequestBypassToServer(_)
            | ClientPacket::RequestLinkHtml(_)
            | ClientPacket::Unported(_)
            | ClientPacket::UnportedEx(_) => return None,
        };
        return Some(raw.starts_with(&encoded));
    }
    None
}

/// Set `L2_CAPTURE_DIR` to a folder with `*.jsonl` captures (format: `l2-proxy` capture.rs).
/// Failure messages name the file, line and packet only, never the bytes: captures contain
/// account and character data.
#[test]
fn packets_match_recorded_java_sessions() {
    // The owner's own recordings (L2_CAPTURE_DIR), else the scrubbed fixtures in the repo
    // (M2-CAP-01), so CI compares against real Java packets too.
    let dir = std::env::var_os("L2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"));
    let mut compared = 0;
    let mut mismatches = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        for (i, line) in text.lines().enumerate().skip(1) {
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if record["channel"] != "game" {
                continue;
            }
            let (Some(opcode), Some(raw)) = (record["opcode"].as_u64(), record["raw"].as_str()) else {
                continue;
            };
            let Ok(raw) = hex::decode(raw) else { continue };
            if raw.is_empty() {
                continue;
            }
            let result = match record["dir"].as_str() {
                Some("s2c") if record.get("ex_opcode").is_none_or(|v| v.is_null()) => {
                    check_server(opcode as u8, &raw)
                }
                Some("c2s") => check_client(record["state"].as_str(), &raw),
                _ => None,
            };
            match result {
                Some(true) => compared += 1,
                Some(false) => mismatches.push(format!(
                    "{} line {}: {} ({} bytes)",
                    path.file_name().unwrap().to_string_lossy(),
                    i + 1,
                    record["name"].as_str().unwrap_or("?"),
                    raw.len()
                )),
                None => {}
            }
        }
    }
    eprintln!(
        "capture comparison: {compared} packets identical, {} different",
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
}
