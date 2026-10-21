#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `AbstractNpcInfo.SummonInfo` and `NpcSay` (M6-SUMMON-01): round trips and the byte layout of
//! Java's `writeImpl`.

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::{NpcInfo, NpcSay, SummonInfo};

fn sample() -> SummonInfo {
    SummonInfo {
        object_id: 0x6000_0001,
        display_id: 14111,
        attackable: true,
        x: -71_338,
        y: 258_271,
        z: -3_104,
        heading: 1234,
        m_atk_spd: 333,
        p_atk_spd: 253,
        run_speed: 120,
        walk_speed: 80,
        swim_run_speed: 50,
        swim_walk_speed: 50,
        fly_run_speed: 0,
        fly_walk_speed: 0,
        move_multiplier: 1.0,
        attack_speed_multiplier: 0.843_333,
        collision_radius: 9.0,
        collision_height: 16.0,
        weapon: 0,
        armor: 0,
        in_combat: true,
        alike_dead: false,
        spawn_state: 2,
        name: "Kat the Cat".into(),
        title: "Owner".into(),
        pvp_flag: 1,
        karma: 0,
        abnormal_visual_effects: 4,
        water_or_flying: 0,
        team: 0,
        enchant_effect: 0,
        form_id: 3,
        abnormal_visual_effect_special: 0,
    }
}

#[test]
fn summon_info_round_trips() {
    let packet = sample();
    let bytes = packet.to_bytes();
    let decoded = SummonInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
    assert_eq!(decoded, packet);
}

/// The fixed offsets of `SummonInfo.writeImpl`: the `NPC_INFO` opcode, the display id plus
/// 1000000, the running byte always 1, title color 1, the owner's flag and karma, the form, and
/// one int less than `NpcInfo` at the end (no display effect).
#[test]
fn summon_info_layout_follows_java() {
    let packet = sample();
    let bytes = packet.to_bytes();
    assert_eq!(bytes[0], 0x0C, "NPC_INFO");
    let int_at = |at: usize| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!(int_at(1), 0x6000_0001);
    assert_eq!(int_at(5), 14111 + 1_000_000);
    assert_eq!(int_at(9), 1, "attackable");
    // 18 ints from the object id to the fly speeds, then four doubles, then three ints.
    let after_doubles = 1 + 18 * 4 + 4 * 8;
    assert_eq!(int_at(after_doubles), 0, "weapon");
    let flags = after_doubles + 12;
    assert_eq!(
        &bytes[flags..flags + 5],
        &[1, 1, 1, 0, 2],
        "name above, running, combat, dead, summoned"
    );
    // `writeInt(-1)` before the name.
    assert_eq!(int_at(flags + 5), -1);
    // A NpcInfo with the same values is one int longer.
    let npc = NpcInfo {
        object_id: packet.object_id,
        display_id: packet.display_id,
        name: packet.name.clone(),
        title: packet.title.clone(),
        ..NpcInfo::default()
    };
    assert_eq!(npc.to_bytes().len(), bytes.len() + 4);
    // The tail: form id, two 1 bytes, the special visual effects.
    let end = bytes.len();
    assert_eq!(int_at(end - 10), 3, "form id");
    assert_eq!(&bytes[end - 6..end - 4], &[1, 1]);
    assert_eq!(int_at(end - 4), 0);
}

#[test]
fn npc_say_round_trips_with_a_string_id_and_with_text() {
    let with_id = NpcSay {
        object_id: 7,
        text_type: 22,
        npc_id: 12564,
        npc_string: 42239,
        text: String::new(),
        parameters: vec!["a".into(), "b".into()],
    };
    let bytes = with_id.to_bytes();
    assert_eq!(bytes[0], 0x30);
    assert_eq!(
        i32::from_le_bytes(bytes[9..13].try_into().unwrap()),
        12564 + 1_000_000
    );
    assert_eq!(
        NpcSay::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
        with_id
    );
    let with_text = NpcSay {
        npc_string: -1,
        text: "Hello".into(),
        parameters: vec![],
        ..with_id
    };
    let bytes = with_text.to_bytes();
    assert_eq!(
        NpcSay::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
        with_text
    );
}
