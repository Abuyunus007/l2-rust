//! Small packets of the enter-world sequence and `SystemMessage`, checked against byte layouts
//! worked out by hand from the Java `writeImpl` code, and against recorded Java game server
//! sessions when `L2_CAPTURE_DIR` is set.
//!
//! The expected bytes are built with an independent little-endian builder (not the crate's
//! `PacketWriter`), so a mistake in the crate's encoder cannot hide behind the same mistake here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{EnterWorld, RequestKeyMapping};
use l2_protocol_game::server::enterworld::{
    EtcStatusUpdate, ExBasicActionList, ExGetBookMarkInfoPacket, ExShowScreenMessage, ExStorageMaxCount,
    ExUiSetting, ExVoteSystemInfo, FriendList, FriendListEntry, HennaInfo, OtherShortcut, QuestList,
    QuestListEntry, ShortcutEntry, ShortcutInit, ShortcutRegister, SkillCoolTime, SkillCoolTimeEntry,
    SkillList, SkillListEntry, TOP_CENTER, TeleportBookmark, ValidateLocation, remaining_seconds,
    reuse_seconds,
};
use l2_protocol_game::server::{ConfirmDlg, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ClientPacket, ConnectionState, NpcStringId, SystemMessageId, decode_client};

/// Independent little-endian builder.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn byte(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn short(mut self, v: u16) -> Self {
        self.0.extend(v.to_le_bytes());
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
    fn raw(mut self, bytes: &[u8]) -> Self {
        self.0.extend_from_slice(bytes);
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

#[test]
fn skill_list_layout() {
    let packet = SkillList {
        skills: vec![
            SkillListEntry {
                id: 1001,
                level: 3,
                passive: false,
                disabled: false,
                enchanted: true,
            },
            SkillListEntry {
                id: 2,
                level: 1,
                passive: true,
                disabled: true,
                enchanted: false,
            },
        ],
    };
    // Java: passive as int, level, id, disabled byte, enchanted byte.
    let expected = Bytes::default()
        .byte(0x5F)
        .int(2)
        .ints(&[0, 3, 1001])
        .byte(0)
        .byte(1)
        .ints(&[1, 1, 2])
        .byte(1)
        .byte(0)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn shortcut_init_layout() {
    let packet = ShortcutInit {
        shortcuts: vec![
            ShortcutEntry::Empty { slot: 0 },
            ShortcutEntry::Item {
                slot: 15,
                id: 1835,
                shared_reuse_group: 7,
            },
            ShortcutEntry::Skill {
                slot: 2,
                id: 1,
                level: 5,
            },
            ShortcutEntry::Other {
                kind: OtherShortcut::Bookmark,
                slot: 3,
                id: 9,
            },
        ],
    };
    let expected = Bytes::default()
        .byte(0x45)
        .int(4)
        // NONE: type and slot only.
        .ints(&[0, 0])
        // ITEM: id, 1, shared reuse group, 0, 0, two zero shorts.
        .ints(&[1, 15, 1835, 1, 7, 0, 0])
        .short(0)
        .short(0)
        // SKILL: id, level, zero byte, 1.
        .ints(&[2, 2, 1, 5])
        .byte(0)
        .int(1)
        // BOOKMARK: id, 1.
        .ints(&[6, 3, 9, 1])
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn shortcut_register_layout() {
    // Java: type SKILL (2), slot + page * 12, skill id, level, zero byte, 1.
    let packet = ShortcutRegister {
        slot: 14,
        skill_id: 1204,
        level: 7,
    };
    let expected = Bytes::default()
        .byte(0x44)
        .ints(&[2, 14, 1204, 7])
        .byte(0)
        .int(1)
        .done();
    check_round_trip(&packet, &expected, 1);
    // The ShortcutInit entry of the same skill has the same bytes after the opcode.
    let init = ShortcutInit {
        shortcuts: vec![ShortcutEntry::Skill {
            slot: 14,
            id: 1204,
            level: 7,
        }],
    };
    assert_eq!(init.to_bytes()[1 + 4..], packet.to_bytes()[1..]);
}

#[test]
fn quest_list_layout() {
    let packet = QuestList {
        quests: vec![QuestListEntry {
            quest_id: 101,
            status: 3,
        }],
    };
    // Opcode, u16 count, (id, status) pairs, then 128 zero bytes.
    let expected = Bytes::default()
        .byte(0x86)
        .short(1)
        .ints(&[101, 3])
        .zeros(128)
        .done();
    assert_eq!(expected.len(), 1 + 2 + 8 + 128);
    check_round_trip(&packet, &expected, 1);
    assert_eq!(QuestList::default().to_bytes().len(), 1 + 2 + 128);
}

#[test]
fn henna_info_layout() {
    let packet = HennaInfo {
        int: 1,
        str: 2,
        con: -1,
        men: 0,
        dex: 0,
        wit: 3,
        dye_ids: vec![1000],
    };
    // Six stat bytes (low byte of each value), slots 3, count, then dye id and constant 1.
    let expected = Bytes::default()
        .byte(0xE5)
        .raw(&[1, 2, 0xFF, 0, 0, 3])
        .int(3)
        .int(1)
        .ints(&[1000, 1])
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn friend_list_layout() {
    let packet = FriendList {
        friends: vec![
            FriendListEntry {
                object_id: 268_435_457,
                name: "Ab".into(),
                online: true,
            },
            FriendListEntry {
                object_id: 5,
                name: "C".into(),
                online: false,
            },
        ],
    };
    // Opcode 0x75 (L2_FRIEND_LIST), count, then per friend: id, name, online, online id or 0.
    let expected = Bytes::default()
        .byte(0x75)
        .int(2)
        .int(268_435_457)
        .text("Ab")
        .int(1)
        .int(268_435_457)
        .int(5)
        .text("C")
        .int(0)
        .int(0)
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn etc_status_update_layout() {
    let packet = EtcStatusUpdate {
        charges: 3,
        weight_penalty: 2,
        chat_blocked: true,
        danger_area: false,
        expertise_weapon_penalty: 1,
        expertise_armor_penalty: 4,
        charm_of_courage: true,
        death_penalty_level: 15,
        charged_souls: 6,
    };
    let expected = Bytes::default()
        .byte(0xF9)
        .ints(&[3, 2, 1, 0, 1, 4, 1, 15, 6])
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn ex_storage_max_count_layout() {
    let packet = ExStorageMaxCount {
        inventory: 80,
        warehouse: 120,
        clan: 30,
        private_sell: 5,
        private_buy: 5,
        dwarven_recipe: 500,
        common_recipe: 500,
        inventory_extra_slots: 0,
        inventory_quest_items: 100,
    };
    let expected = Bytes::default()
        .raw(&[0xFE, 0x2F, 0x00])
        .ints(&[80, 120, 30, 5, 5, 500, 500, 0, 100])
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn ex_basic_action_list_default_layout() {
    let packet = ExBasicActionList::default_list();
    // Java: 74 ids from 0, 99 from 1000, 16 from 5000.
    assert_eq!(packet.action_ids.len(), 74 + 99 + 16);
    assert_eq!(packet.action_ids[0], 0);
    assert_eq!(packet.action_ids[73], 73);
    assert_eq!(packet.action_ids[74], 1000);
    assert_eq!(packet.action_ids[172], 1098);
    assert_eq!(packet.action_ids[173], 5000);
    assert_eq!(packet.action_ids[188], 5015);
    let bytes = packet.to_bytes();
    // 3 bytes of ex id, count, 189 ints: 763 bytes, the same length the Java capture has.
    assert_eq!(bytes.len(), 3 + 4 + 189 * 4);
    assert_eq!(bytes[..3], [0xFE, 0x5F, 0x00]);
    check_round_trip(&packet, &bytes, 3);
}

#[test]
fn ex_vote_system_info_layout() {
    let packet = ExVoteSystemInfo {
        recom_left: 9,
        recom_have: 3,
        bonus_time: 3600,
        bonus_value: 100,
        bonus_type: 1,
    };
    let expected = Bytes::default()
        .raw(&[0xFE, 0xC9, 0x00])
        .ints(&[9, 3, 3600, 100, 1])
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn bookmark_info_layout() {
    let packet = ExGetBookMarkInfoPacket {
        slot: 3,
        bookmarks: vec![TeleportBookmark {
            id: 1,
            x: 2,
            y: 3,
            z: 4,
            name: "A".into(),
            icon: 5,
            tag: "T".into(),
        }],
    };
    // Dummy int 0, slot, count, then per bookmark: id, x, y, z, name, icon, tag.
    let expected = Bytes::default()
        .raw(&[0xFE, 0x84, 0x00])
        .ints(&[0, 3, 1, 1, 2, 3, 4])
        .text("A")
        .int(5)
        .text("T")
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn validate_location_layout() {
    let packet = ValidateLocation {
        object_id: 268_435_457,
        x: -71_301,
        y: 258_259,
        z: -3_132,
        heading: 32_768,
    };
    let expected = Bytes::default()
        .byte(0x79)
        .ints(&[268_435_457, -71_301, 258_259, -3_132, 32_768])
        .done();
    check_round_trip(&packet, &expected, 1);
}

#[test]
fn skill_cool_time_layout_and_helpers() {
    let packet = SkillCoolTime {
        entries: vec![SkillCoolTimeEntry {
            skill_id: 1,
            level: 2,
            reuse_seconds: 10,
            remaining_seconds: 1,
        }],
    };
    let expected = Bytes::default().byte(0xC7).ints(&[1, 1, 2, 10, 1]).done();
    check_round_trip(&packet, &expected, 1);

    // Java: the reuse delay when set, else the time left, then the time left at least 1 second.
    assert_eq!(reuse_seconds(12_500, 1_000), 12);
    assert_eq!(reuse_seconds(0, 1_500), 1);
    assert_eq!(reuse_seconds(0, 999), 0);
    assert_eq!(remaining_seconds(999), 1);
    assert_eq!(remaining_seconds(5_500), 5);
}

#[test]
fn ui_setting_layout() {
    let packet = ExUiSetting {
        ui_key_mapping: vec![1, 2, 3],
    };
    let expected = Bytes::default()
        .raw(&[0xFE, 0x70, 0x00])
        .int(3)
        .raw(&[1, 2, 3])
        .done();
    check_round_trip(&packet, &expected, 3);

    // Nothing stored: just the zero length.
    let empty = ExUiSetting::default();
    assert_eq!(empty.to_bytes(), [0xFE, 0x70, 0x00, 0, 0, 0, 0]);
    check_round_trip(&empty, &[0xFE, 0x70, 0x00, 0, 0, 0, 0], 3);
}

#[test]
fn screen_message_layouts() {
    // Plain text: type 2, system message -1, centre, effect off, time 3000, NPC string -1.
    let text = ExShowScreenMessage::text("Hi", 3000);
    assert_eq!(text.position, TOP_CENTER);
    let expected = Bytes::default()
        .raw(&[0xFE, 0x39, 0x00])
        .ints(&[2, -1, 2, 0, 0, 0, 0, 0, 3000, 0, -1])
        .text("Hi")
        .done();
    check_round_trip(&text, &expected, 3);

    // NPC string with one parameter: the string id, then one text per parameter.
    let npc_id = NpcStringId::from_id(1).expect("NpcStringId 1 exists");
    assert_eq!(npc_id.id(), 1);
    let npc = ExShowScreenMessage::npc_string(npc_id, 7, 6000, &["Bob"]);
    let expected = Bytes::default()
        .raw(&[0xFE, 0x39, 0x00])
        .ints(&[2, -1, 7, 0, 0, 0, 0, 0, 6000, 0, 1])
        .text("Bob")
        .done();
    check_round_trip(&npc, &expected, 3);

    // Styled text: type 1, size, fade and effect as given.
    let styled = ExShowScreenMessage::styled_text("S", 4, 500, 1, true, true);
    let expected = Bytes::default()
        .raw(&[0xFE, 0x39, 0x00])
        .ints(&[1, -1, 4, 0, 1, 0, 0, 1, 500, 1, -1])
        .text("S")
        .done();
    check_round_trip(&styled, &expected, 3);
}

#[test]
fn enter_world_layout() {
    let mut tracert = [[0; 4]; 5];
    tracert[0] = [10, 0, 0, 1];
    tracert[4] = [192, 168, 1, 255];
    let packet = EnterWorld {
        unknown1: [7; 32],
        unknown2: [1, 2, 3, 4],
        unknown3: [9; 32],
        unknown4: -5,
        tracert,
    };
    let bytes = packet.to_bytes();
    // Opcode and 104 body bytes: 32 + 16 + 32 + 4 + 20.
    assert_eq!(bytes.len(), 1 + EnterWorld::BODY_LEN);
    assert_eq!(bytes[0], 0x11);
    assert_eq!(bytes[1..33], [7; 32]);
    assert_eq!(bytes[33..37], 1_i32.to_le_bytes());
    assert_eq!(bytes[49..81], [9; 32]);
    assert_eq!(bytes[81..85], (-5_i32).to_le_bytes());
    assert_eq!(bytes[85..89], [10, 0, 0, 1]);
    assert_eq!(bytes[101..105], [192, 168, 1, 255]);
    check_round_trip(&packet, &bytes, 1);
    // Short packets are an error, as in Java (readBytes throws).
    assert!(EnterWorld::decode(&mut PacketReader::new(&bytes[1..104])).is_err());
}

#[test]
fn request_key_mapping_layout() {
    assert_eq!(RequestKeyMapping.to_bytes(), [0xD0, 0x21, 0x00]);
    check_round_trip(&RequestKeyMapping, &[0xD0, 0x21, 0x00], 3);
}

#[test]
fn system_message_layouts() {
    // "You use $s1." (id 46) with one item name parameter.
    let message = SystemMessage::new(SystemMessageId::YouUseS1).with(SystemMessageParam::ItemName(57));
    let expected = Bytes::default().byte(0x62).ints(&[46, 1, 3, 57]).done();
    check_round_trip(&message, &expected, 1);

    // Long numbers are an 8-byte value after their type int.
    let message =
        SystemMessage::new(SystemMessageId::YouUseS1).with(SystemMessageParam::LongNumber(5_000_000_000));
    assert_eq!(
        message.to_bytes(),
        Bytes::default()
            .byte(0x62)
            .ints(&[46, 1, 6])
            .long(5_000_000_000)
            .done()
    );

    // NPC names go on the wire as 1000000 + id.
    let message = SystemMessage::new(SystemMessageId::YouUseS1).with(SystemMessageParam::NpcName(18));
    assert_eq!(
        message.to_bytes(),
        Bytes::default().byte(0x62).ints(&[46, 1, 2, 1_000_018]).done()
    );

    // Plain text message: id S1_2, text parameter.
    let text = SystemMessage::text("hello");
    assert_eq!(text.id, SystemMessageId::S12);
    let expected = Bytes::default()
        .byte(0x62)
        .int(SystemMessageId::S12.id())
        .int(1)
        .int(0)
        .text("hello")
        .done();
    check_round_trip(&text, &expected, 1);
}

#[test]
fn confirm_dlg_layout() {
    // Java ConfirmDlg.writeImpl: the SystemMessage body (id, count, typed params), then the time
    // and the requester id. The resurrection question (id 1510) takes a player name and a string.
    let dlg = ConfirmDlg::new(
        SystemMessageId::C1IsMakingAnAttemptToResurrectYouIfYouChooseThisPathS2ExperiencePointsWillBeReturnedToYouDoYouWantToBeResurrected,
    )
    .with(SystemMessageParam::PlayerName("Cleric".into()))
    .with(SystemMessageParam::Text("1234".into()));
    let expected = Bytes::default()
        .byte(0xF3)
        .ints(&[1510, 2, 12])
        .text("Cleric")
        .int(0)
        .text("1234")
        .ints(&[0, 0])
        .done();
    check_round_trip(&dlg, &expected, 1);

    // The charm of courage question carries a 60 s limit and no parameters.
    let charm =
        ConfirmDlg::new(SystemMessageId::YourCharmOfCourageIsTryingToResurrectYouWouldYouLikeToResurrectNow)
            .with_time(60_000)
            .with_requester_id(5);
    let expected = Bytes::default().byte(0xF3).ints(&[2306, 0, 60_000, 5]).done();
    check_round_trip(&charm, &expected, 1);
}

#[test]
fn system_message_every_parameter_type_round_trips() {
    let params = [
        SystemMessageParam::Text("t".into()),
        SystemMessageParam::PlayerName("Ab".into()),
        SystemMessageParam::CastleName(9),
        SystemMessageParam::DoorName(23_150),
        SystemMessageParam::NpcName(25),
        SystemMessageParam::ItemName(57),
        SystemMessageParam::SkillName { id: 1, level: 2 },
        SystemMessageParam::ElementName(4),
        SystemMessageParam::SystemString(77),
        SystemMessageParam::InstanceName(3),
        SystemMessageParam::IntNumber(-8),
        SystemMessageParam::LongNumber(5_000_000_000),
        SystemMessageParam::ZoneName { x: 1, y: -2, z: 3 },
    ];
    for param in params {
        let message = SystemMessage::new(SystemMessageId::YouUseS1).with(param);
        let bytes = message.to_bytes();
        let decoded = SystemMessage::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, message);
    }
}

#[test]
fn system_message_written_count_follows_java() {
    // Java writes the declared parameter count even when the caller added none. Those slots
    // are never written, so the packet is short: this is what the client gets from Java too.
    let bare = SystemMessage::new(SystemMessageId::YouUseS1);
    assert_eq!(bare.to_bytes(), Bytes::default().byte(0x62).ints(&[46, 1]).done());
    // More parameters than declared: the count is the number of parameters.
    let many = SystemMessage::new(SystemMessageId::YouUseS1)
        .with(SystemMessageParam::IntNumber(1))
        .with(SystemMessageParam::IntNumber(2));
    assert_eq!(many.to_bytes()[5..9], 2_i32.to_le_bytes());
}

#[test]
fn system_message_rejects_bad_input() {
    // Unknown message id.
    let bytes = Bytes::default().ints(&[6508, 0]).done();
    assert!(SystemMessage::decode(&mut PacketReader::new(&bytes)).is_err());
    // Unknown parameter type.
    let bytes = Bytes::default().ints(&[46, 1, 42, 0]).done();
    assert!(SystemMessage::decode(&mut PacketReader::new(&bytes)).is_err());
    // Negative count.
    let bytes = Bytes::default().ints(&[46, -1]).done();
    assert!(SystemMessage::decode(&mut PacketReader::new(&bytes)).is_err());
}

#[test]
fn dispatcher_decodes_the_enter_world_packets() {
    let bytes = EnterWorld {
        unknown1: [0; 32],
        unknown2: [0; 4],
        unknown3: [0; 32],
        unknown4: 0,
        tracert: [[0; 4]; 5],
    }
    .to_bytes();
    let decoded = decode_client(
        ConnectionState::Entering,
        bytes[0],
        &mut PacketReader::new(&bytes[1..]),
    )
    .unwrap();
    assert!(matches!(decoded, Some(ClientPacket::EnterWorld(_))));
    // RequestKeyMapping is accepted while entering, through the ex dispatch.
    let mut reader = PacketReader::new(&[0x21, 0x00]);
    let decoded = decode_client(ConnectionState::Entering, 0xD0, &mut reader).unwrap();
    assert_eq!(decoded, Some(ClientPacket::RequestKeyMapping(RequestKeyMapping)));
}

// ---------------------------------------------------------------------------------------------
// Capture-driven check against the owner's Java sessions.
// ---------------------------------------------------------------------------------------------

/// Server packets (plain opcode) that must re-encode to exactly the recorded bytes.
fn check_server(opcode: u8, raw: &[u8]) -> Option<Result<bool, ()>> {
    let body = &mut PacketReader::new(&raw[1..]);
    let encoded = match opcode {
        0x62 => SystemMessage::decode(body).map(|p| p.to_bytes()),
        0x5F => SkillList::decode(body).map(|p| p.to_bytes()),
        0x45 => ShortcutInit::decode(body).map(|p| p.to_bytes()),
        0x86 => QuestList::decode(body).map(|p| p.to_bytes()),
        0xE5 => HennaInfo::decode(body).map(|p| p.to_bytes()),
        0x75 => FriendList::decode(body).map(|p| p.to_bytes()),
        0xF9 => EtcStatusUpdate::decode(body).map(|p| p.to_bytes()),
        0x79 => ValidateLocation::decode(body).map(|p| p.to_bytes()),
        0xC7 => SkillCoolTime::decode(body).map(|p| p.to_bytes()),
        _ => return None,
    };
    Some(encoded.map(|bytes| bytes == raw).map_err(|_| ()))
}

/// Ex server packets (`0xFE` plus the `u16` ex id) that must re-encode to the recorded bytes.
fn check_server_ex(ex: u16, raw: &[u8]) -> Option<Result<bool, ()>> {
    let body = &mut PacketReader::new(&raw[3..]);
    let encoded = match ex {
        0x2F => ExStorageMaxCount::decode(body).map(|p| p.to_bytes()),
        0x5F => ExBasicActionList::decode(body).map(|p| p.to_bytes()),
        0xC9 => ExVoteSystemInfo::decode(body).map(|p| p.to_bytes()),
        0x84 => ExGetBookMarkInfoPacket::decode(body).map(|p| p.to_bytes()),
        0x70 => ExUiSetting::decode(body).map(|p| p.to_bytes()),
        0x39 => ExShowScreenMessage::decode(body).map(|p| p.to_bytes()),
        _ => return None,
    };
    Some(encoded.map(|bytes| bytes == raw).map_err(|_| ()))
}

fn state_from_name(name: &str) -> Option<ConnectionState> {
    ConnectionState::ALL.iter().copied().find(|s| s.name() == name)
}

/// Client packets of this card (`EnterWorld`, `RequestKeyMapping`), decoded with the dispatcher.
/// The recorded state is only a guess, so every state is tried when it does not name one.
fn check_client(state: Option<&str>, raw: &[u8]) -> Option<Result<bool, ()>> {
    let states: Vec<ConnectionState> = match state.and_then(state_from_name) {
        Some(s) => vec![s],
        None => ConnectionState::ALL.to_vec(),
    };
    for state in states {
        let decoded = decode_client(state, raw[0], &mut PacketReader::new(&raw[1..]));
        let Ok(Some(packet)) = decoded else {
            continue;
        };
        let encoded = match packet {
            ClientPacket::EnterWorld(p) => p.to_bytes(),
            ClientPacket::RequestKeyMapping(p) => p.to_bytes(),
            _ => return None,
        };
        return Some(Ok(raw.starts_with(&encoded)));
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
            if raw.len() < 3 {
                continue;
            }
            let ex = record["ex_opcode"].as_u64();
            let result = match (record["dir"].as_str(), ex) {
                (Some("s2c"), None) if raw[0] == opcode as u8 => check_server(opcode as u8, &raw),
                (Some("s2c"), Some(ex)) if raw[0] == 0xFE => check_server_ex(ex as u16, &raw),
                (Some("c2s"), _) => check_client(record["state"].as_str(), &raw),
                _ => None,
            };
            match result {
                Some(Ok(true)) => compared += 1,
                Some(Ok(false)) | Some(Err(())) => mismatches.push(format!(
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
    assert!(compared > 0, "no matching packets found in {}", dir.display());
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
}
