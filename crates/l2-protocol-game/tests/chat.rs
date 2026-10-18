//! Chat, logout, restart and net ping packets against byte layouts worked out by hand from the
//! Java `writeImpl`/`readImpl` code, and against recorded Java game server sessions when
//! `L2_CAPTURE_DIR` is set.
//!
//! The expected bytes are built with a tiny writer of their own (not the crate's `PacketWriter`),
//! listing the fields in the order of the Java source, so a mistake in the crate's encoder cannot
//! hide behind the same mistake in the test.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{Logout, RequestNetPing, RequestRestart, Say2};
use l2_protocol_game::server::{
    ChatType, CreatureSay, LeaveWorld, NetPing, RestartResponse, SayBody, SenderField,
};
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
    /// Java `writeString`: UTF-16LE plus a 0x0000 terminator.
    fn text(mut self, s: &str) -> Self {
        for unit in s.encode_utf16() {
            self.0.extend(unit.to_le_bytes());
        }
        self.0.extend([0, 0]);
        self
    }
    fn done(self) -> Vec<u8> {
        self.0
    }
}

#[test]
fn chat_type_ids_match_java() {
    assert_eq!(ChatType::ALL.len(), 24);
    for (index, chat_type) in ChatType::ALL.iter().enumerate() {
        assert_eq!(chat_type.client_id(), index as i32, "{chat_type:?}");
        assert_eq!(ChatType::from_client_id(index as i32), Some(*chat_type));
    }
    // Spot checks against ChatType.java.
    assert_eq!(ChatType::Whisper.client_id(), 2);
    assert_eq!(ChatType::PetitionGm.client_id(), 7);
    assert_eq!(ChatType::NpcGeneral.client_id(), 22);
    assert_eq!(ChatType::from_client_id(24), None);
    assert_eq!(ChatType::from_client_id(-1), None);
}

#[test]
fn say2_layouts() {
    // Plain channel: text, then the channel id, no target.
    let general = Say2 {
        text: "hi".into(),
        chat_type: ChatType::General.client_id(),
        target: None,
    };
    assert_eq!(
        general.to_bytes(),
        Bytes::default().byte(0x49).text("hi").int(0).done()
    );
    // Whisper: the target name follows the channel id.
    let whisper = Say2 {
        text: "yo".into(),
        chat_type: ChatType::Whisper.client_id(),
        target: Some("Bob".into()),
    };
    assert_eq!(
        whisper.to_bytes(),
        Bytes::default().byte(0x49).text("yo").int(2).text("Bob").done()
    );
    assert_eq!(whisper.chat_type(), Some(ChatType::Whisper));
    assert_eq!(
        Say2 {
            text: String::new(),
            chat_type: 99,
            target: None
        }
        .chat_type(),
        None
    );
}

#[test]
fn say2_round_trips() {
    for packet in [
        Say2 {
            text: "hello world".into(),
            chat_type: ChatType::Shout.client_id(),
            target: None,
        },
        Say2 {
            text: "psst".into(),
            chat_type: ChatType::Whisper.client_id(),
            target: Some("Friend".into()),
        },
    ] {
        let bytes = packet.to_bytes();
        assert_eq!(Say2::decode(&mut PacketReader::new(&bytes[1..])).unwrap(), packet);
    }
}

#[test]
fn say2_ignores_trailing_padding() {
    // Java reads text, type and (for whispers) the target; the rest of the packet is ignored.
    let bytes = Bytes::default().text("a").int(1).byte(0xEE).byte(0xEE).done();
    let mut reader = PacketReader::new(&bytes);
    let packet = Say2::decode(&mut reader).unwrap();
    assert_eq!(packet.text, "a");
    assert_eq!(packet.chat_type, 1);
    assert_eq!(packet.target, None);
}

#[test]
fn body_less_client_packets() {
    assert_eq!(Logout.to_bytes(), [0x00]);
    assert_eq!(RequestRestart.to_bytes(), [0x57]);
    assert_eq!(RequestNetPing.to_bytes(), [0xB1]);
    assert_eq!(Logout::decode(&mut PacketReader::new(&[])).unwrap(), Logout);
    assert_eq!(
        RequestRestart::decode(&mut PacketReader::new(&[])).unwrap(),
        RequestRestart
    );
    assert_eq!(
        RequestNetPing::decode(&mut PacketReader::new(&[])).unwrap(),
        RequestNetPing
    );
}

#[test]
fn leave_world_layout() {
    assert_eq!(LeaveWorld.to_bytes(), [0x84]);
    assert_eq!(
        LeaveWorld::decode(&mut PacketReader::new(&[])).unwrap(),
        LeaveWorld
    );
}

#[test]
fn restart_response_layout_and_round_trip() {
    assert_eq!(RestartResponse { result: true }.to_bytes(), [0x71, 1, 0, 0, 0]);
    assert_eq!(RestartResponse { result: false }.to_bytes(), [0x71, 0, 0, 0, 0]);
    for result in [true, false] {
        let packet = RestartResponse { result };
        let bytes = packet.to_bytes();
        assert_eq!(
            RestartResponse::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }
}

#[test]
fn net_ping_layout_and_round_trip() {
    let packet = NetPing {
        game_time: 0x0102_0304,
    };
    assert_eq!(packet.to_bytes(), [0xD9, 0x04, 0x03, 0x02, 0x01]);
    let bytes = packet.to_bytes();
    assert_eq!(
        NetPing::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
        packet
    );
}

#[test]
fn creature_say_layouts() {
    // Player chat: the sender's object id, channel, sender name, message id -1, then the text.
    let shout = CreatureSay {
        sender_object_id: 268_435_457,
        chat_type: ChatType::Shout,
        sender: SenderField::Name("Ab".into()),
        message_id: -1,
        body: SayBody::Text("hello".into()),
    };
    assert_eq!(
        shout.to_bytes(),
        Bytes::default()
            .byte(0x4A)
            .int(268_435_457)
            .int(1)
            .text("Ab")
            .int(-1)
            .text("hello")
            .done()
    );
    // NPC string with two parameters: no text, the parameters follow the message id.
    let npc = CreatureSay {
        sender_object_id: 5,
        chat_type: ChatType::NpcGeneral,
        sender: SenderField::Name("Guard".into()),
        message_id: 1000,
        body: SayBody::Parameters(vec!["x".into(), "y".into()]),
    };
    assert_eq!(
        npc.to_bytes(),
        Bytes::default()
            .byte(0x4A)
            .int(5)
            .int(22)
            .text("Guard")
            .int(1000)
            .text("x")
            .text("y")
            .done()
    );
    // System message: no sender object and no name, so Java writes the char id instead, and
    // there are no parameters.
    let system = CreatureSay {
        sender_object_id: 0,
        chat_type: ChatType::Party,
        sender: SenderField::CharId(77),
        message_id: 1234,
        body: SayBody::Parameters(vec![]),
    };
    assert_eq!(
        system.to_bytes(),
        Bytes::default().byte(0x4A).int(0).int(3).int(77).int(1234).done()
    );
}

// --- capture comparison -----------------------------------------------------------------------

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
            ClientPacket::Say2(p) => p.to_bytes(),
            ClientPacket::Logout(p) => p.to_bytes(),
            ClientPacket::RequestRestart(p) => p.to_bytes(),
            ClientPacket::RequestNetPing(p) => p.to_bytes(),
            _ => return None,
        };
        return Some(raw.starts_with(&encoded));
    }
    None
}

/// Server packets of this file that have a decoder. `CreatureSay` has none (see `server::chat`).
fn check_server(opcode: u8, raw: &[u8]) -> Option<bool> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0x84 => LeaveWorld::decode(body).ok()?.to_bytes(),
        0x71 => RestartResponse::decode(body).ok()?.to_bytes(),
        0xD9 => NetPing::decode(body).ok()?.to_bytes(),
        _ => return None,
    };
    Some(encoded == raw)
}

/// Set `L2_CAPTURE_DIR` to a folder with `*.jsonl` captures (format: `l2-proxy` capture.rs).
/// Failure messages name the file, line and packet only, never the bytes: captures contain
/// account and character data.
#[test]
fn chat_packets_match_recorded_java_sessions() {
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
        "chat capture comparison: {compared} packets identical, {} different",
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
}
