//! Visibility and movement packets (`M3-GPROTO-05`): byte layouts worked out by hand from the
//! Java `writeImpl` / `readImpl` code, round trips, and a comparison against recorded Java
//! game server sessions (the scrubbed fixtures in the repo, or `L2_CAPTURE_DIR`).
//!
//! The expected bytes are built with a tiny writer of their own (not the crate's
//! `PacketWriter`), listing the fields in the order of the Java source, so a mistake in the
//! crate's encoder cannot hide behind the same mistake in the test.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{Appearing, CannotMoveAnymore, ValidatePosition};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::visibility::{compass_zone, relation, status};
use l2_protocol_game::server::{
    CharInfo, DeleteObject, ExSetCompassZoneCode, MoveToPawn, NpcInfo, RelationChanged, RelationEntry,
    StatusUpdate, StopMove, TeleportToLocation, ValidateLocation,
};
use l2_protocol_game::{ClientOpcode, ConnectionState, ServerOpcode, decode_client};
use l2_protocol_game::{ClientPacket, client, server};

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
    fn ints(mut self, vs: &[i32]) -> Self {
        for v in vs {
            self.0.extend(v.to_le_bytes());
        }
        self
    }
    fn double(mut self, v: f64) -> Self {
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
}

/// A value per slot: `base + slot`, so a swapped slot shows up.
fn per_slot(base: i32) -> [i32; paperdoll::TOTAL_SLOTS] {
    let mut out = [0; paperdoll::TOTAL_SLOTS];
    for (slot, value) in out.iter_mut().enumerate() {
        *value = base + slot as i32;
    }
    out
}

/// `CharInfo.PAPERDOLL_ORDER` as an independent list of slot numbers: UNDER, HEAD, RHAND,
/// LHAND, GLOVES, CHEST, LEGS, FEET, CLOAK, RHAND, HAIR, HAIR2, RBRACELET, LBRACELET,
/// DECO1..DECO6, BELT.
const CHAR_INFO_SLOTS: [usize; 21] = [
    0, 1, 5, 7, 10, 6, 11, 12, 23, 5, 2, 3, 16, 15, 17, 18, 19, 20, 21, 22, 24,
];

fn char_info_block(values: &[i32; paperdoll::TOTAL_SLOTS]) -> Vec<i32> {
    CHAR_INFO_SLOTS.iter().map(|&slot| values[slot]).collect()
}

fn sample_char_info() -> CharInfo {
    let mut item_ids = per_slot(2_000);
    // The packet writes slot 5 (RHAND) twice, so only a value that is the same for both copies
    // can round trip; it is, by construction (one array).
    item_ids[paperdoll::RHAND] = 2_005;
    CharInfo {
        x: -83_000,
        y: 243_000,
        z: -3_700,
        vehicle_id: 0,
        object_id: 268_480_001,
        name: "Ab".into(),
        race: 2,
        is_female: true,
        base_class: 31,
        paperdoll_item_ids: item_ids,
        paperdoll_augmentation_ids: per_slot(3_000),
        talisman_slots: 2,
        can_equip_cloak: true,
        pvp_flag: 1,
        karma: 9,
        m_atk_spd: 333,
        p_atk_spd: 330,
        run_speed: 126,
        walk_speed: 80,
        swim_run_speed: 50,
        swim_walk_speed: 40,
        fly_run_speed: 11,
        fly_walk_speed: 12,
        move_multiplier: 1.1,
        attack_speed_multiplier: 1.5,
        collision_radius: 8.5,
        collision_height: 23.25,
        hair_style: 3,
        hair_color: 2,
        face: 1,
        title: "Tt".into(),
        clan_id: 7,
        clan_crest_id: 8,
        ally_id: 9,
        ally_crest_id: 10,
        sitting: true,
        running: true,
        in_combat: false,
        alike_dead: true,
        invisible: false,
        mount_type: 2,
        private_store_type: 5,
        cubics: vec![1, 10],
        in_party_match_room: true,
        abnormal_visual_effects: 0x1_0000,
        water_or_flying: 2,
        recom_have: 33,
        mount_npc_id: 12_526,
        class_id: 97,
        enchant_effect: 12,
        team: 1,
        clan_crest_large_id: 77,
        noble: true,
        hero: false,
        fishing: true,
        fish_x: 100,
        fish_y: 200,
        fish_z: 300,
        name_color: 0x00FF_FFFF,
        heading: 32_768,
        pledge_class: 5,
        pledge_type: -1,
        title_color: 0x00EE_EEEE,
        cursed_weapon_level: 0,
        clan_reputation_score: 4_000,
        transformation_display_id: 6,
        agathion_id: 1_234,
        abnormal_visual_effect_special: 0x20,
    }
}

#[test]
fn char_info_layout_follows_the_java_write_impl() {
    let p = sample_char_info();
    let expected = Bytes::default()
        .byte(0x31)
        .ints(&[-83_000, 243_000, -3_700, 0, 268_480_001])
        .text("Ab")
        .ints(&[2, 1, 31])
        .ints(&char_info_block(&p.paperdoll_item_ids))
        .ints(&char_info_block(&p.paperdoll_augmentation_ids))
        .ints(&[2, 1]) // talisman slots, can equip cloak
        .ints(&[1, 9]) // pvp flag, karma
        .ints(&[333, 330, 0]) // m atk spd, p atk spd, unknown
        .ints(&[126, 80, 50, 40, 11, 12, 11, 12])
        .double(1.1)
        .double(1.5)
        .double(8.5)
        .double(23.25)
        .ints(&[3, 2, 1])
        .text("Tt")
        .ints(&[7, 8, 9, 10])
        .byte(0) // sitting: Java writes !sitting
        .byte(1) // running
        .byte(0) // in combat
        .byte(1) // alike dead
        .byte(0) // invisible
        .byte(2) // mount type
        .byte(5) // private store type
        .short(2)
        .short(1)
        .short(10)
        .byte(1) // party match room
        .int(0x1_0000)
        .byte(2) // water / flying
        .short(33) // recom have
        .int(12_526 + 1_000_000) // mount npc id, +1000000 always
        .int(97) // class id
        .int(0)
        .byte(12) // enchant
        .byte(1) // team
        .int(77) // large crest
        .byte(1) // noble
        .byte(0) // hero
        .byte(1) // fishing
        .ints(&[100, 200, 300])
        .int(0x00FF_FFFF)
        .int(32_768) // heading
        .ints(&[5, -1, 0x00EE_EEEE, 0])
        .int(4_000) // clan reputation
        .ints(&[6, 1_234])
        .int(1)
        .int(0x20)
        .0;
    assert_eq!(p.to_bytes(), expected);
    assert_eq!(
        CharInfo::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        {
            // Slots that are not on the wire do not survive the trip.
            let mut q = p.clone();
            for slot in 0..paperdoll::TOTAL_SLOTS {
                if !CHAR_INFO_SLOTS.contains(&slot) {
                    q.paperdoll_item_ids[slot] = 0;
                    q.paperdoll_augmentation_ids[slot] = 0;
                }
            }
            q
        }
    );
}

#[test]
fn char_info_without_a_mount_sends_1000000() {
    let bytes = CharInfo::default().to_bytes();
    // The mount npc id sits at: opcode(1) + 5 ints + name(2) + 3 ints + 2 * 21 ints + 7 ints +
    // 8 speeds + 4 doubles + 3 ints + title(2) + 4 ints + 7 bytes + cubic count(2) +
    // party match(1) + effects(4) + water(1) + recom(2).
    let at = 1 + 20 + 2 + 12 + 168 + 28 + 32 + 32 + 12 + 2 + 16 + 7 + 2 + 1 + 4 + 1 + 2;
    assert_eq!(bytes[at..at + 4], 1_000_000_i32.to_le_bytes());
    let decoded = CharInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
    assert_eq!(decoded.mount_npc_id, 0);
}

#[test]
fn char_info_round_trip_and_truncation() {
    // Slots that are not on the wire are zero in the sample's decoded form, so start from a
    // packet that only uses wire slots.
    let mut p = sample_char_info();
    for slot in 0..paperdoll::TOTAL_SLOTS {
        if !CHAR_INFO_SLOTS.contains(&slot) {
            p.paperdoll_item_ids[slot] = 0;
            p.paperdoll_augmentation_ids[slot] = 0;
        }
    }
    let bytes = p.to_bytes();
    assert_eq!(CharInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap(), p);
    for cut in [1, 10, 40, bytes.len() - 1] {
        assert!(
            CharInfo::decode(&mut PacketReader::new(&bytes[1..cut])).is_err(),
            "cut at {cut}"
        );
    }
    assert_eq!(bytes[0], 0x31);
}

fn sample_npc_info() -> NpcInfo {
    NpcInfo {
        object_id: 268_500_000,
        display_id: 18_342,
        attackable: true,
        x: 82_000,
        y: 148_000,
        z: -3_470,
        heading: 16_384,
        m_atk_spd: 333,
        p_atk_spd: 253,
        run_speed: 120,
        walk_speed: 50,
        swim_run_speed: 20,
        swim_walk_speed: 10,
        fly_run_speed: 0,
        fly_walk_speed: 0,
        move_multiplier: 1.0,
        attack_speed_multiplier: 1.25,
        collision_radius: 9.5,
        collision_height: 22.0,
        right_hand: 15,
        left_hand: 16,
        running: true,
        in_combat: true,
        alike_dead: false,
        show_summon_animation: true,
        name: "Nm".into(),
        title: "Ti".into(),
        abnormal_visual_effects: 0x40,
        clan_id: 5,
        clan_crest_id: 6,
        ally_id: 7,
        ally_crest_id: 8,
        water_or_flying: 1,
        team: 2,
        enchant_effect: 3,
        flying: true,
        color_effect: 4,
        targetable: true,
        show_name: false,
        abnormal_visual_effect_special: 9,
        display_effect: 1,
    }
}

#[test]
fn npc_info_layout_follows_the_java_write_impl() {
    let p = sample_npc_info();
    let expected = Bytes::default()
        .byte(0x0C)
        .int(268_500_000)
        .int(18_342 + 1_000_000) // npc type id
        .int(1) // attackable
        .ints(&[82_000, 148_000, -3_470, 16_384])
        .int(0)
        .ints(&[333, 253]) // m atk spd, p atk spd
        .ints(&[120, 50, 20, 10, 0, 0, 0, 0])
        .double(1.0)
        .double(1.25)
        .double(9.5)
        .double(22.0)
        .ints(&[15, 0, 16]) // right hand, chest (never set), left hand
        .byte(1) // name above char
        .byte(1) // running
        .byte(1) // in combat
        .byte(0) // alike dead
        .byte(2) // summoned
        .int(-1)
        .text("Nm")
        .int(-1)
        .text("Ti")
        .ints(&[0, 0, 0]) // title color, pvp flag, karma
        .int(0x40)
        .ints(&[5, 6, 7, 8])
        .byte(1) // water
        .byte(2) // team
        .double(9.5)
        .double(22.0)
        .int(3) // enchant
        .int(1) // flying
        .int(0)
        .int(4) // color effect
        .byte(1) // targetable
        .byte(0) // show name
        .int(9)
        .int(1) // display effect
        .0;
    assert_eq!(p.to_bytes(), expected);
    assert_eq!(
        NpcInfo::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        p
    );
}

#[test]
fn npc_info_plain_values() {
    // Not summoned (0 on the wire), empty name and title (the client uses its own table).
    let p = NpcInfo {
        show_summon_animation: false,
        ..NpcInfo::default()
    };
    let bytes = p.to_bytes();
    assert_eq!(bytes[0], 0x0C);
    assert_eq!(NpcInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap(), p);
    // The summoned byte: opcode + 8 ints + 2 ints + 8 speeds + 4 doubles + 3 ints + 3 bytes.
    let at = 1 + 32 + 8 + 32 + 32 + 12 + 3;
    assert_eq!(bytes[at], 0);
    assert!(NpcInfo::decode(&mut PacketReader::new(&bytes[1..bytes.len() - 1])).is_err());
}

#[test]
fn delete_object_writes_a_trailing_one() {
    let p = DeleteObject { object_id: 77 };
    let expected = Bytes::default().byte(0x08).ints(&[77, 1]).0;
    assert_eq!(p.to_bytes(), expected);
    assert_eq!(
        DeleteObject::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        p
    );
}

#[test]
fn relation_changed_single_and_list_share_a_layout() {
    let a = RelationEntry {
        object_id: 5,
        relation: relation::CLAN_MEMBER | relation::LEADER,
        auto_attackable: true,
        karma: 3,
        pvp_flag: 1,
    };
    let b = RelationEntry {
        object_id: 6,
        relation: 0,
        auto_attackable: false,
        karma: 0,
        pvp_flag: 0,
    };
    let single = RelationChanged::single(a);
    let expected = Bytes::default().byte(0xCE).ints(&[1, 5, 0xC0, 1, 3, 1]).0;
    assert_eq!(single.to_bytes(), expected);

    let list = RelationChanged {
        relations: vec![a, b],
    };
    let expected = Bytes::default()
        .byte(0xCE)
        .ints(&[2, 5, 0xC0, 1, 3, 1, 6, 0, 0, 0, 0])
        .0;
    assert_eq!(list.to_bytes(), expected);
    assert_eq!(
        RelationChanged::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        list
    );
    // A negative count is invalid, a count larger than the data is truncated.
    assert!(RelationChanged::decode(&mut PacketReader::new(&(-1_i32).to_le_bytes())).is_err());
    assert!(RelationChanged::decode(&mut PacketReader::new(&2_i32.to_le_bytes())).is_err());
}

#[test]
fn status_update_writes_pairs_in_call_order() {
    let mut p = StatusUpdate::new(10);
    assert!(!p.has_attributes());
    p.add_attribute(status::CUR_HP, 900);
    p.add_attribute(status::MAX_HP, 1_000);
    p.add_attribute(status::CUR_CP, 500);
    assert!(p.has_attributes());
    let expected = Bytes::default()
        .byte(0x18)
        .ints(&[10, 3, 0x09, 900, 0x0a, 1_000, 0x21, 500])
        .0;
    assert_eq!(p.to_bytes(), expected);
    assert_eq!(
        StatusUpdate::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        p
    );
    // An empty list is a valid packet.
    let empty = StatusUpdate::new(1);
    assert_eq!(empty.to_bytes(), Bytes::default().byte(0x18).ints(&[1, 0]).0);
    assert!(StatusUpdate::decode(&mut PacketReader::new(&[1, 0, 0, 0, 1, 0, 0, 0])).is_err());
    assert_eq!(status::MAX_CP, 0x22);
}

#[test]
fn compass_zone_code_is_an_ex_packet() {
    let p = ExSetCompassZoneCode {
        zone_type: compass_zone::PEACEZONE,
    };
    // 0xFE, ex id 0x0033 (u16), zone type.
    let expected = Bytes::default().byte(0xFE).short(0x33).int(0x0C).0;
    assert_eq!(p.to_bytes(), expected);
    assert_eq!(
        ExSetCompassZoneCode::decode(&mut PacketReader::new(&expected[3..])).unwrap(),
        p
    );
}

#[test]
fn move_to_location_writes_destination_before_position() {
    let p = server::MoveToLocation {
        object_id: 1,
        dest_x: 2,
        dest_y: 3,
        dest_z: 4,
        x: 5,
        y: 6,
        z: 7,
    };
    let expected = Bytes::default().byte(0x2F).ints(&[1, 2, 3, 4, 5, 6, 7]).0;
    assert_eq!(p.to_bytes(), expected);
    assert_eq!(
        server::MoveToLocation::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        p
    );
}

#[test]
fn stop_move_pawn_and_teleport_layouts() {
    let stop = StopMove {
        object_id: 1,
        x: 2,
        y: 3,
        z: 4,
        heading: 5,
    };
    let expected = Bytes::default().byte(0x47).ints(&[1, 2, 3, 4, 5]).0;
    assert_eq!(stop.to_bytes(), expected);
    assert_eq!(
        StopMove::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        stop
    );

    let pawn = MoveToPawn {
        object_id: 1,
        target_id: 2,
        distance: 3,
        x: 4,
        y: 5,
        z: 6,
        target_x: 7,
        target_y: 8,
        target_z: 9,
    };
    let expected = Bytes::default().byte(0x72).ints(&[1, 2, 3, 4, 5, 6, 7, 8, 9]).0;
    assert_eq!(pawn.to_bytes(), expected);
    assert_eq!(
        MoveToPawn::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        pawn
    );

    let tele = TeleportToLocation {
        object_id: 1,
        x: 2,
        y: 3,
        z: 4,
        heading: 5,
    };
    // Object id, x, y, z, fade flag (always 0), heading.
    let expected = Bytes::default().byte(0x22).ints(&[1, 2, 3, 4, 0, 5]).0;
    assert_eq!(tele.to_bytes(), expected);
    assert_eq!(
        TeleportToLocation::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        tele
    );
    assert!(TeleportToLocation::decode(&mut PacketReader::new(&expected[1..20])).is_err());
}

#[test]
fn client_packets_layouts_and_dispatch() {
    let mv = client::MoveToLocation {
        target_x: 1,
        target_y: 2,
        target_z: 3,
        origin_x: 4,
        origin_y: 5,
        origin_z: 6,
        movement_mode: 1,
    };
    let expected = Bytes::default().byte(0x0F).ints(&[1, 2, 3, 4, 5, 6, 1]).0;
    assert_eq!(mv.to_bytes(), expected);
    assert_eq!(
        client::MoveToLocation::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        mv
    );

    let validate = ValidatePosition {
        x: 1,
        y: 2,
        z: 3,
        heading: 4,
        vehicle_id: 0,
    };
    let expected = Bytes::default().byte(0x59).ints(&[1, 2, 3, 4, 0]).0;
    assert_eq!(validate.to_bytes(), expected);
    assert_eq!(
        ValidatePosition::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        validate
    );

    let stuck = CannotMoveAnymore {
        x: 1,
        y: 2,
        z: 3,
        heading: 4,
    };
    let expected = Bytes::default().byte(0x47).ints(&[1, 2, 3, 4]).0;
    assert_eq!(stuck.to_bytes(), expected);
    assert_eq!(
        CannotMoveAnymore::decode(&mut PacketReader::new(&expected[1..])).unwrap(),
        stuck
    );

    assert_eq!(Appearing.to_bytes(), [0x3A]);
    assert_eq!(Appearing::decode(&mut PacketReader::new(&[])).unwrap(), Appearing);

    // Short bodies are errors.
    assert!(client::MoveToLocation::decode(&mut PacketReader::new(&[0; 27])).is_err());
    assert!(ValidatePosition::decode(&mut PacketReader::new(&[0; 19])).is_err());
    assert!(CannotMoveAnymore::decode(&mut PacketReader::new(&[0; 15])).is_err());

    // The dispatcher maps each opcode in IN_GAME to its struct.
    let dispatch = |opcode: ClientOpcode, bytes: &[u8]| {
        decode_client(
            ConnectionState::InGame,
            opcode.id(),
            &mut PacketReader::new(&bytes[1..]),
        )
        .unwrap()
        .unwrap()
    };
    assert_eq!(
        dispatch(ClientOpcode::MoveToLocation, &mv.to_bytes()),
        ClientPacket::MoveToLocation(mv)
    );
    assert_eq!(
        dispatch(ClientOpcode::ValidatePosition, &validate.to_bytes()),
        ClientPacket::ValidatePosition(validate)
    );
    assert_eq!(
        dispatch(ClientOpcode::CannotMoveAnymore, &stuck.to_bytes()),
        ClientPacket::CannotMoveAnymore(stuck)
    );
    assert_eq!(
        dispatch(ClientOpcode::Appearing, &Appearing.to_bytes()),
        ClientPacket::Appearing(Appearing)
    );
    // Not in the character screen.
    assert_eq!(
        decode_client(
            ConnectionState::Authenticated,
            0x0F,
            &mut PacketReader::new(&[0; 28])
        )
        .unwrap(),
        None
    );
}

#[test]
fn opcodes_match_the_java_tables() {
    for (opcode, id) in [
        (CharInfo::OPCODE, 0x31),
        (NpcInfo::OPCODE, 0x0C),
        (DeleteObject::OPCODE, 0x08),
        (RelationChanged::OPCODE, 0xCE),
        (StatusUpdate::OPCODE, 0x18),
        (server::MoveToLocation::OPCODE, 0x2F),
        (StopMove::OPCODE, 0x47),
        (MoveToPawn::OPCODE, 0x72),
        (TeleportToLocation::OPCODE, 0x22),
        (ValidateLocation::OPCODE, 0x79),
    ] {
        assert_eq!(opcode.id(), id, "{}", opcode.name());
    }
    assert_eq!(ExSetCompassZoneCode::OPCODE, ServerOpcode::ExSetCompassZoneCode);
}

// ---------------------------------------------------------------------------------------------
// Capture-driven check against the owner's Java sessions.
// ---------------------------------------------------------------------------------------------

/// Re-encodes a recorded server packet of a ported kind, given the bytes after the opcode(s).
/// `None` when this module does not cover the packet; `Some(false)` when decoding fails or
/// the bytes change.
fn check_server(name: &str, raw: &[u8]) -> Option<bool> {
    fn same<T: Decode + Encode>(raw: &[u8], skip: usize) -> bool {
        T::decode(&mut PacketReader::new(&raw[skip..])).is_ok_and(|p| p.to_bytes() == raw)
    }
    Some(match name {
        "NPC_INFO" => same::<NpcInfo>(raw, 1),
        "MOVE_TO_LOCATION" => same::<server::MoveToLocation>(raw, 1),
        "MOVE_TO_PAWN" => same::<MoveToPawn>(raw, 1),
        "STOP_MOVE" => same::<StopMove>(raw, 1),
        "DELETE_OBJECT" => same::<DeleteObject>(raw, 1),
        "STATUS_UPDATE" => same::<StatusUpdate>(raw, 1),
        "VALIDATE_LOCATION" => same::<ValidateLocation>(raw, 1),
        "CHAR_INFO" => same::<CharInfo>(raw, 1),
        "TELEPORT_TO_LOCATION" => same::<TeleportToLocation>(raw, 1),
        "RELATION_CHANGED" => same::<RelationChanged>(raw, 1),
        "EX_SET_COMPASS_ZONE_CODE" => same::<ExSetCompassZoneCode>(raw, 3),
        _ => return None,
    })
}

/// The same for packets the client sent (`c2s`).
fn check_client(name: &str, raw: &[u8]) -> Option<bool> {
    fn same<T: Decode + Encode>(raw: &[u8]) -> bool {
        T::decode(&mut PacketReader::new(&raw[1..])).is_ok_and(|p| p.to_bytes() == raw)
    }
    Some(match name {
        "MOVE_TO_LOCATION" => same::<client::MoveToLocation>(raw),
        "VALIDATE_POSITION" => same::<ValidatePosition>(raw),
        "CANNOT_MOVE_ANYMORE" => same::<CannotMoveAnymore>(raw),
        "APPEARING" => same::<Appearing>(raw),
        _ => return None,
    })
}

/// Set `L2_CAPTURE_DIR` to a folder with `*.jsonl` captures (format: `l2-proxy` capture.rs).
/// Failure messages name the file, line and packet only, never the bytes: captures contain
/// account and character data.
#[test]
fn visibility_and_movement_packets_match_recorded_java_sessions() {
    // The owner's own recordings (L2_CAPTURE_DIR), else the scrubbed fixtures in the repo
    // (M2-CAP-01), so CI compares against real Java packets too.
    let dir = std::env::var_os("L2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"));
    let mut compared = std::collections::BTreeMap::<String, usize>::new();
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
            let (Some(name), Some(raw)) = (record["name"].as_str(), record["raw"].as_str()) else {
                continue;
            };
            let Ok(raw) = hex::decode(raw) else { continue };
            if raw.is_empty() {
                continue;
            }
            let result = match record["dir"].as_str() {
                Some("s2c") => check_server(name, &raw),
                Some("c2s") => check_client(name, &raw),
                _ => None,
            };
            let Some(identical) = result else { continue };
            let key = format!("{} {name}", record["dir"].as_str().unwrap_or("?"));
            if identical {
                *compared.entry(key).or_default() += 1;
            } else {
                mismatches.push(format!(
                    "{} line {}: {key} ({} bytes)",
                    path.file_name().unwrap().to_string_lossy(),
                    i + 1,
                    raw.len()
                ));
            }
        }
    }
    let total: usize = compared.values().sum();
    eprintln!(
        "capture comparison: {total} packets identical, {} different; per packet: {compared:?}",
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "packets differ from the capture:\n{}",
        mismatches.join("\n")
    );
}
