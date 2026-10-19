//! Target, action and NPC dialog packets against byte layouts worked out by hand from the Java
//! `writeImpl` / `readImpl` code, round trips, the `setHtml` rules, and the recorded Java game
//! server sessions (`fixtures/captures`, or `L2_CAPTURE_DIR`).
//!
//! The expected bytes are built with an independent little-endian builder (not the crate's
//! `PacketWriter`), so a mistake in the crate's encoder cannot hide behind the same mistake here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    Action, BypassUserCmd, RequestBypassToServer, RequestLinkHtml, RequestTargetCanceld, SendBypassBuildCmd,
};
use l2_protocol_game::server::{
    ActionFailed, DoorInfo, DoorStatusUpdate, MyTargetSelected, NpcHtmlMessage, StaticObjectInfo,
    TargetSelected, TargetUnselected,
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
    fn short(mut self, v: i16) -> Self {
        self.0.extend(v.to_le_bytes());
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
// Server packets.
// ---------------------------------------------------------------------------------------------

#[test]
fn my_target_selected_layout() {
    // Java: objectId, colour as a short, constant int 0. Same bytes as a recorded packet
    // (`b9 c8a00010 0300 00000000`), colour 3 = level 40 minus level 37.
    let packet = MyTargetSelected::new(0x1000_A0C8, 40, 37, true);
    assert_eq!(packet.color, 3);
    let expected = Bytes::default()
        .byte(0xB9)
        .int(0x1000_A0C8)
        .short(3)
        .int(0)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn my_target_selected_colour_rules() {
    // Not auto-attackable: colour 0 whatever the levels are.
    assert_eq!(MyTargetSelected::new(1, 80, 10, false).color, 0);
    // A higher target gives a negative colour, written as the two's complement short.
    let packet = MyTargetSelected::new(1, 30, 33, true);
    assert_eq!(packet.color, -3);
    let expected = Bytes::default().byte(0xB9).int(1).short(-3).int(0).done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn target_selected_layout() {
    let packet = TargetSelected {
        object_id: 1,
        target_id: 2,
        x: -1,
        y: 20,
        z: 300,
    };
    // Java: object, target, x, y, z, constant 0.
    let expected = Bytes::default()
        .byte(0x23)
        .int(1)
        .int(2)
        .int(-1)
        .int(20)
        .int(300)
        .int(0)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn target_unselected_layout_matches_a_recorded_packet() {
    // Recorded (`242ec7001039b3000036a3000074f2ffff00000000`): target, x, y, z, constant 0.
    let raw = from_hex("242ec7001039b3000036a3000074f2ffff00000000");
    let packet = TargetUnselected {
        target_id: 0x1000_C72E,
        x: 0xB339,
        y: 0xA336,
        z: -3468,
    };
    let expected = Bytes::default()
        .byte(0x24)
        .int(0x1000_C72E)
        .int(0xB339)
        .int(0xA336)
        .int(-3468)
        .int(0)
        .done();
    assert_eq!(expected, raw);
    check_round_trip(&packet, &raw, 1);
}

#[test]
fn action_failed_is_the_opcode_alone() {
    // Java: ServerPackets.ACTION_FAIL.writeId only.
    check_round_trip(&ActionFailed, &[0x1F], 1);
}

#[test]
fn npc_html_message_wraps_a_fragment() {
    let packet = NpcHtmlMessage::new(0x1234, "<a>").unwrap();
    assert_eq!(packet.html, "<html><body><a></body></html>");
    assert_eq!(packet.item_id, 0);
    // Java: objectId, html string, item id.
    let expected = Bytes::default()
        .byte(0x19)
        .int(0x1234)
        .text("<html><body><a></body></html>")
        .int(0)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn npc_html_item_window_keeps_the_item_id() {
    let packet = NpcHtmlMessage::new_item(0, 7, "<html>x</html>").unwrap();
    // A full page is kept as it is.
    assert_eq!(packet.html, "<html>x</html>");
    let expected = Bytes::default()
        .byte(0x19)
        .int(0)
        .text("<html>x</html>")
        .int(7)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn npc_html_keeps_file_references_and_wraps_case_sensitively() {
    // Java: `..\L2` file references are passed on unchanged.
    assert_eq!(
        NpcHtmlMessage::new(1, "..\\L2Text\\x.htm").unwrap().html,
        "..\\L2Text\\x.htm"
    );
    // NOTE(java-quirk): Java's `contains("<html")` is case sensitive, so `<HTML>` gets wrapped.
    assert_eq!(
        NpcHtmlMessage::new(1, "<HTML>x</HTML>").unwrap().html,
        "<html><body><HTML>x</HTML></body></html>"
    );
}

#[test]
fn npc_html_too_long_is_cut_and_not_wrapped() {
    let long = "a".repeat(17_201);
    let packet = NpcHtmlMessage::new(1, &long).unwrap();
    assert_eq!(packet.html.encode_utf16().count(), 17_200);
    assert!(packet.html.chars().all(|c| c == 'a'));
}

#[test]
fn npc_html_limit_counts_utf16_units() {
    // 8600 emoji are 17200 UTF-16 units: not too long, so the fragment is wrapped as usual.
    let emoji = "\u{1F600}".repeat(8_600);
    let packet = NpcHtmlMessage::new(1, &emoji).unwrap();
    assert!(packet.html.starts_with("<html><body>"));
    assert!(packet.html.ends_with("</body></html>"));
    // One letter and 8600 emoji are 17201 units: cut to 17200 units, not wrapped. The cut lands
    // inside the last pair, which becomes U+FFFD (NOTE(java-quirk) on prepare_html).
    let too_long = format!("a{}", "\u{1F600}".repeat(8_600));
    let packet = NpcHtmlMessage::new(1, &too_long).unwrap();
    assert_eq!(packet.html.encode_utf16().count(), 17_200);
    assert!(packet.html.ends_with('\u{FFFD}'));
    assert!(!packet.html.starts_with("<html>"));
}

#[test]
fn npc_html_rejects_negative_ids_like_java_throws() {
    assert!(NpcHtmlMessage::new(-1, "x").is_none());
    assert!(NpcHtmlMessage::new_item(1, -1, "x").is_none());
}

#[test]
fn npc_html_decode_keeps_the_html_it_was_given() {
    // Decoding does not wrap: the bytes are the page as sent.
    let raw = Bytes::default().byte(0x19).int(5).text("<a>").int(0).done();
    let decoded = NpcHtmlMessage::decode(&mut PacketReader::new(&raw[1..])).unwrap();
    assert_eq!(decoded.html, "<a>");
    assert_eq!(decoded.npc_obj_id, 5);
}

#[test]
fn door_info_layout() {
    let packet = DoorInfo {
        object_id: 0x7000_0001,
        door_id: 24_190_001,
    };
    let expected = Bytes::default()
        .byte(0x4C)
        .int(0x7000_0001)
        .int(24_190_001)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn social_action_layout() {
    let packet = l2_protocol_game::server::SocialAction {
        object_id: 0x1000_0010,
        action_id: 5,
    };
    let expected = Bytes::default().byte(0x27).int(0x1000_0010).int(5).done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn show_town_map_layout() {
    let packet = l2_protocol_game::server::ShowTownMap {
        texture: "town_map.gludio".into(),
        x: 17_000,
        y: -9_000,
    };
    let expected = Bytes::default()
        .byte(0xEA)
        .text("town_map.gludio")
        .int(17_000)
        .int(-9_000)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn inventory_update_layout() {
    use l2_protocol_game::server::{InventoryUpdate, ItemInfo, ItemUpdate};
    let item = ItemInfo::plain(0x6000_0001, 57, 1234);
    let packet = InventoryUpdate {
        updates: vec![ItemUpdate {
            change: InventoryUpdate::MODIFIED,
            item,
        }],
    };
    let bytes = packet.to_bytes();
    // Opcode, entry count (short), change (short), then the item.
    assert_eq!(&bytes[..5], &[0x21, 1, 0, 2, 0]);
    let decoded = InventoryUpdate::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
    assert_eq!(decoded, packet);
}

#[test]
fn door_status_update_layout() {
    let packet = DoorStatusUpdate {
        object_id: 1,
        closed: true,
        damage: 0,
        is_enemy: false,
        door_id: 2,
        current_hp: 500,
        max_hp: 1000,
    };
    // Java: object, !isOpen (1 = closed), damage, isEnemy, id, current hp, max hp.
    let expected = Bytes::default()
        .byte(0x4D)
        .int(1)
        .int(1)
        .int(0)
        .int(0)
        .int(2)
        .int(500)
        .int(1000)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn static_object_info_for_a_static_object() {
    let packet = StaticObjectInfo::static_object(7, 0x1000_0010, 3);
    // Java: type 0, targetable, mesh, closed 0, enemy 0, hp 0, hp bar off, damage grade 0.
    let expected = Bytes::default()
        .byte(0x9F)
        .int(7)
        .int(0x1000_0010)
        .int(0)
        .int(1)
        .int(3)
        .int(0)
        .int(0)
        .int(0)
        .int(0)
        .int(0)
        .int(0)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn static_object_info_for_a_door_writes_hp_in_java_order() {
    let packet = StaticObjectInfo {
        static_object_id: 9,
        object_id: 0x7000_0002,
        kind: StaticObjectInfo::KIND_DOOR,
        targetable: false,
        mesh_index: 1,
        closed: true,
        is_enemy: true,
        current_hp: 40,
        max_hp: 100,
        show_hp: true,
        damage_grade: 2,
    };
    // Current hp is written before max hp, as in StaticObjectInfo.writeImpl.
    let expected = Bytes::default()
        .byte(0x9F)
        .int(9)
        .int(0x7000_0002)
        .int(1)
        .int(0)
        .int(1)
        .int(1)
        .int(1)
        .int(40)
        .int(100)
        .int(1)
        .int(2)
        .done();
    check_round_trip(&packet, &expected, 1);
}

// ---------------------------------------------------------------------------------------------
// Client packets.
// ---------------------------------------------------------------------------------------------

#[test]
fn action_layout_matches_a_recorded_click() {
    // Recorded (`1f33cb001092b2000021a200004cf2ffff00`): object, origin x, y, z, simple click.
    let raw = from_hex("1f33cb001092b2000021a200004cf2ffff00");
    let packet = Action {
        object_id: 0x1000_CB33,
        origin_x: 0xB292,
        origin_y: 0xA221,
        origin_z: -3508,
        action_id: Action::SIMPLE_CLICK,
    };
    let expected = Bytes::default()
        .byte(0x1F)
        .int(0x1000_CB33)
        .int(0xB292)
        .int(0xA221)
        .int(-3508)
        .byte(0)
        .done();
    assert_eq!(expected, raw);
    assert_eq!(packet.to_bytes(), raw);
    let decoded = Action::decode(&mut PacketReader::new(&raw[1..])).unwrap();
    assert_eq!(decoded, packet);
}

#[test]
fn action_shift_click_and_signed_action_byte() {
    let packet = Action {
        object_id: 5,
        origin_x: 0,
        origin_y: 0,
        origin_z: 0,
        action_id: Action::SHIFT_CLICK,
    };
    assert_eq!(packet.to_bytes().last(), Some(&1));
    // Java reads a signed byte: 0xFF is -1, which is no valid action.
    let raw = [0x1F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF];
    let decoded = Action::decode(&mut PacketReader::new(&raw[1..])).unwrap();
    assert_eq!(decoded.action_id, -1);
    check_round_trip(&decoded, &raw, 1);
}

#[test]
fn request_target_canceld_layout() {
    // Escape key: a zero short.
    let escape = RequestTargetCanceld { unselect: 0 };
    check_round_trip(&escape, &[0x48, 0, 0], 1);
    let clear = RequestTargetCanceld { unselect: 1 };
    check_round_trip(&clear, &[0x48, 1, 0], 1);
    // Signed short, as Java reads it.
    let negative = RequestTargetCanceld { unselect: -2 };
    check_round_trip(&negative, &[0x48, 0xFE, 0xFF], 1);
}

#[test]
fn request_bypass_keeps_the_raw_command() {
    let packet = RequestBypassToServer {
        command: "npc_12345_Chat 1".into(),
    };
    let expected = Bytes::default().byte(0x23).text("npc_12345_Chat 1").done();
    check_round_trip(&packet, &expected, 1);
    // Non-ASCII text survives too.
    let korean = RequestBypassToServer {
        command: "가_bypass?x=1".into(),
    };
    check_round_trip(
        &korean,
        &Bytes::default().byte(0x23).text("가_bypass?x=1").done(),
        1,
    );
}

#[test]
fn request_bypass_empty_or_missing_string_reads_as_empty() {
    // Java readString: an empty string is the terminator alone. Java also returns "" when the
    // data ends first (the exception is swallowed), so a short body is not an error.
    let decoded = RequestBypassToServer::decode(&mut PacketReader::new(&[0, 0])).unwrap();
    assert_eq!(decoded.command, "");
    let decoded = RequestBypassToServer::decode(&mut PacketReader::new(&[])).unwrap();
    assert_eq!(decoded.command, "");
}

#[test]
fn send_bypass_build_cmd_layout_and_trim() {
    // `//spawn 20101`: the client sends the text after the slashes.
    let packet = SendBypassBuildCmd {
        command: "spawn 20101".into(),
    };
    check_round_trip(
        &packet,
        &Bytes::default().byte(0x74).text("spawn 20101").done(),
        1,
    );
    // Java `readImpl` trims the text.
    let padded = Bytes::default().byte(0x74).text("  admin 	").done();
    let decoded = SendBypassBuildCmd::decode(&mut PacketReader::new(&padded[1..])).unwrap();
    assert_eq!(decoded.command, "admin");
    // Allowed in game only: the generated table decides.
    let body = Bytes::default().text("loc").done();
    let in_game = decode_client(ConnectionState::InGame, 0x74, &mut PacketReader::new(&body)).unwrap();
    assert_eq!(
        in_game,
        Some(ClientPacket::SendBypassBuildCmd(SendBypassBuildCmd {
            command: "loc".into()
        }))
    );
    let before = decode_client(
        ConnectionState::Authenticated,
        0x74,
        &mut PacketReader::new(&body),
    )
    .unwrap();
    assert_eq!(before, None);
}

#[test]
fn bypass_user_cmd_layout() {
    // `/loc` is user command 0.
    let packet = BypassUserCmd { command: 0 };
    check_round_trip(&packet, &Bytes::default().byte(0xB3).int(0).done(), 1);
    let other = BypassUserCmd { command: 52 };
    check_round_trip(&other, &Bytes::default().byte(0xB3).int(52).done(), 1);
    assert!(BypassUserCmd::decode(&mut PacketReader::new(&[1, 0])).is_err());
}

#[test]
fn request_link_html_layout() {
    let packet = RequestLinkHtml {
        link: "teleports/x.htm".into(),
    };
    check_round_trip(
        &packet,
        &Bytes::default().byte(0x22).text("teleports/x.htm").done(),
        1,
    );
}

#[test]
fn interaction_packets_are_accepted_in_game_only() {
    let mut body = Bytes::default().int(1).int(2).int(3).int(4).byte(0).done();
    body.extend([0; 4]);
    let action = decode_client(ConnectionState::InGame, 0x1F, &mut PacketReader::new(&body)).unwrap();
    assert!(matches!(action, Some(ClientPacket::Action(_))));
    let none = decode_client(
        ConnectionState::Authenticated,
        0x1F,
        &mut PacketReader::new(&body),
    )
    .unwrap();
    assert_eq!(none, None);

    let link = RequestLinkHtml { link: "a.htm".into() }.to_bytes();
    let decoded = decode_client(
        ConnectionState::InGame,
        link[0],
        &mut PacketReader::new(&link[1..]),
    )
    .unwrap();
    assert_eq!(
        decoded,
        Some(ClientPacket::RequestLinkHtml(RequestLinkHtml {
            link: "a.htm".into()
        }))
    );
}

// ---------------------------------------------------------------------------------------------
// Capture-driven check against the owner's Java sessions.
// ---------------------------------------------------------------------------------------------

fn from_hex(text: &str) -> Vec<u8> {
    hex::decode(text).unwrap()
}

/// Server packets of this file: the recorded bytes must re-encode exactly.
fn check_server(opcode: u8, raw: &[u8]) -> Option<bool> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0xB9 => MyTargetSelected::decode(body).ok()?.to_bytes(),
        0x23 => TargetSelected::decode(body).ok()?.to_bytes(),
        0x24 => TargetUnselected::decode(body).ok()?.to_bytes(),
        0x1F => ActionFailed::decode(body).ok()?.to_bytes(),
        0x19 => NpcHtmlMessage::decode(body).ok()?.to_bytes(),
        0x4C => DoorInfo::decode(body).ok()?.to_bytes(),
        0x4D => DoorStatusUpdate::decode(body).ok()?.to_bytes(),
        0x9F => StaticObjectInfo::decode(body).ok()?.to_bytes(),
        _ => return None,
    };
    Some(encoded == raw)
}

fn state_from_name(name: &str) -> Option<ConnectionState> {
    ConnectionState::ALL.iter().copied().find(|s| s.name() == name)
}

/// Client packets of this file. The recorded state is only a guess, so every state is tried.
fn check_client(state: Option<&str>, raw: &[u8]) -> Option<bool> {
    let states: Vec<ConnectionState> = match state.and_then(state_from_name) {
        Some(s) => vec![s],
        None => ConnectionState::ALL.to_vec(),
    };
    for state in states {
        let Ok(Some(packet)) = decode_client(state, raw[0], &mut PacketReader::new(&raw[1..])) else {
            continue;
        };
        let encoded = match packet {
            ClientPacket::Action(p) => p.to_bytes(),
            ClientPacket::RequestTargetCanceld(p) => p.to_bytes(),
            ClientPacket::RequestBypassToServer(p) => p.to_bytes(),
            ClientPacket::RequestLinkHtml(p) => p.to_bytes(),
            _ => return None,
        };
        // The client may pad a packet, so the recorded bytes may be longer than ours.
        return Some(raw.starts_with(&encoded));
    }
    None
}

/// Set `L2_CAPTURE_DIR` to a folder with `*.jsonl` captures (format: `l2-proxy` capture.rs).
/// Failure messages name the file, line and packet only, never the bytes: captures contain
/// account and character data.
#[test]
fn interaction_packets_match_recorded_java_sessions() {
    // The owner's own recordings (L2_CAPTURE_DIR), else the scrubbed fixtures in the repo.
    let from_env = std::env::var_os("L2_CAPTURE_DIR");
    let dir = from_env
        .clone()
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
        "interaction capture comparison: {compared} packets identical, {} different",
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
    // The committed fixtures hold recorded Action, ActionFailed and MyTargetSelected packets.
    if from_env.is_none() {
        assert!(compared > 0, "no interaction packets found in the fixtures");
    }
}
