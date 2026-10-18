//! Decoded fields of game packets, for `Record::fields` in the capture.
//!
//! The game relay decodes a copy of every packet with the packet structs of `l2-protocol-game`
//! and turns the result into a JSON object, like the login side does for login packets. Decoding
//! never affects forwarding: a body that does not decode gets a `decode_error` note instead.
//!
//! Redaction: the game `AuthLogin` (account name and the two key pairs) only ever becomes
//! `{"credentials": "<redacted>"}`, with no account name, even when the login side's
//! `log_accounts` is on. The character list (`CharSelectionInfo`) and the character selection
//! (`CharSelected`) echo the account name and the play session id, so those two are redacted
//! as well.
//!
//! `null` means "not decoded": packets without a struct yet, and `CreatureSay` (the server's
//! chat packet has an encoder only). `{}` means decoded, with no fields.

use l2_net::{Decode, PacketError, PacketReader};
use l2_protocol_game::server::charscreen::{
    CharCreateFail, CharCreateOk, CharDeleteFail, CharDeleteSuccess, CharSelected, CharSelectionInfo,
    NewCharacterSuccess, ServerClose,
};
use l2_protocol_game::server::chat::{LeaveWorld, NetPing, RestartResponse};
use l2_protocol_game::server::enterworld::{
    EtcStatusUpdate, ExBasicActionList, ExGetBookMarkInfoPacket, ExShowScreenMessage, ExStorageMaxCount,
    ExUiSetting, ExVoteSystemInfo, FriendList, HennaInfo, QuestList, ShortcutEntry, ShortcutInit,
    SkillCoolTime, SkillList, ValidateLocation,
};
use l2_protocol_game::server::handshake::KeyPacket;
use l2_protocol_game::server::items::{InventoryBlock, ItemInfo, ItemList};
use l2_protocol_game::server::system_message::{SystemMessage, SystemMessageParam};
use l2_protocol_game::server::userinfo::UserInfo;
use l2_protocol_game::{ClientOpcode, ClientPacket, ConnectionState, ServerOpcode, decode_client};
use serde_json::{Value, json};

use crate::capture::{Dir, REDACTED};

/// Builds a JSON object from struct fields that have the same name as their JSON keys.
macro_rules! object {
    ($src:expr => $($field:ident),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut map = serde_json::Map::new();
        $( map.insert(stringify!($field).to_owned(), json!($src.$field)); )*
        Value::Object(map)
    }};
}

/// The `fields` of one game packet, or `None` when it is not decoded. `raw` is the plaintext
/// payload with the opcode first. `ex_opcode` is the `u16` after `0xFE` (server) or `0xD0`
/// (client; the client decoder reads it from `raw` itself). `redacted` is set by the relay for
/// `AuthLogin`, whose `raw` is cut down to the opcode byte.
pub(crate) fn describe(
    dir: Dir,
    state: ConnectionState,
    ex_opcode: Option<u16>,
    raw: &[u8],
    redacted: bool,
) -> Option<Value> {
    let &opcode = raw.first()?;
    // Checked here too, not only through `redacted`, so no caller can leak the credentials.
    if redacted || (dir == Dir::C2s && opcode == ClientOpcode::AuthLogin.id()) {
        return Some(json!({ "credentials": REDACTED }));
    }
    match dir {
        Dir::C2s => describe_client(state, raw),
        Dir::S2c => describe_server(ex_opcode, raw),
    }
}

fn decode_error(error: &PacketError) -> Value {
    json!({ "decode_error": error.to_string() })
}

/// Decodes one packet body with its struct and maps it to fields.
fn decoded<P: Decode>(r: &mut PacketReader<'_>, fields: fn(&P) -> Value) -> Value {
    match P::decode(r) {
        Ok(packet) => fields(&packet),
        Err(error) => decode_error(&error),
    }
}

fn no_fields<P>(_: &P) -> Value {
    json!({})
}

// ---------------------------------------------------------------------------------------------
// Client packets
// ---------------------------------------------------------------------------------------------

fn describe_client(state: ConnectionState, raw: &[u8]) -> Option<Value> {
    let (&opcode, body) = raw.split_first()?;
    // The dispatcher checks the state first, like Java. The state here is only a guess, and the
    // client's packets are not ordered against the server's answers, so the guess can lag behind.
    // The field layout does not depend on the state: try the guessed state, then the others.
    for candidate in std::iter::once(state).chain(ConnectionState::ALL.iter().copied()) {
        match decode_client(candidate, opcode, &mut PacketReader::new(body)) {
            Ok(Some(packet)) => return client_fields(&packet),
            Ok(None) => {}
            Err(error) => return Some(decode_error(&error)),
        }
    }
    None
}

fn client_fields(packet: &ClientPacket) -> Option<Value> {
    Some(match packet {
        ClientPacket::ProtocolVersion(p) => object!(p => version),
        ClientPacket::AuthLogin(_) => json!({ "credentials": REDACTED }),
        ClientPacket::NewCharacter(_)
        | ClientPacket::RequestGotoLobby(_)
        | ClientPacket::RequestKeyMapping(_)
        | ClientPacket::Logout(_)
        | ClientPacket::RequestRestart(_)
        | ClientPacket::RequestNetPing(_) => json!({}),
        ClientPacket::CharacterCreate(p) => object!(
            p => name, race, is_female, class_id, int, str, con, men, dex, wit, hair_style, hair_color, face
        ),
        ClientPacket::CharacterDelete(p) => object!(p => char_slot),
        ClientPacket::CharacterRestore(p) => object!(p => char_slot),
        ClientPacket::CharacterSelect(p) => object!(p => char_slot, unk1, unk2, unk3, unk4),
        ClientPacket::EnterWorld(p) => {
            let mut fields = object!(p => unknown2, unknown4, tracert);
            fields["unknown1"] = json!(hex::encode(p.unknown1));
            fields["unknown3"] = json!(hex::encode(p.unknown3));
            fields
        }
        ClientPacket::Say2(p) => object!(p => text, chat_type, target),
        ClientPacket::Unported(_) | ClientPacket::UnportedEx(_) => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// Server packets
// ---------------------------------------------------------------------------------------------

fn describe_server(ex_opcode: Option<u16>, raw: &[u8]) -> Option<Value> {
    let &opcode = raw.first()?;
    // `None` for the single byte EX_DUMMY and for ids without a table entry.
    let packet = ServerOpcode::from_id(opcode, ex_opcode)?;
    let start = if ex_opcode.is_some() { 3 } else { 1 };
    let mut r = PacketReader::new(raw.get(start..).unwrap_or_default());
    Some(match packet {
        ServerOpcode::VersionCheck => decoded::<KeyPacket>(&mut r, key_packet_fields),
        ServerOpcode::CharacterSelectionInfo => decoded::<CharSelectionInfo>(&mut r, char_selection_fields),
        ServerOpcode::NewCharacterSuccess => decoded::<NewCharacterSuccess>(&mut r, new_character_fields),
        ServerOpcode::CharacterCreateSuccess => decoded::<CharCreateOk>(&mut r, no_fields),
        ServerOpcode::CharacterCreateFail => decoded::<CharCreateFail>(&mut r, reason_fields_create),
        ServerOpcode::CharacterDeleteSuccess => decoded::<CharDeleteSuccess>(&mut r, no_fields),
        ServerOpcode::CharacterDeleteFail => decoded::<CharDeleteFail>(&mut r, reason_fields_delete),
        ServerOpcode::CharacterSelected => decoded::<CharSelected>(&mut r, char_selected_fields),
        ServerOpcode::ServerClose => decoded::<ServerClose>(&mut r, no_fields),
        ServerOpcode::ItemList => decoded::<ItemList>(&mut r, item_list_fields),
        ServerOpcode::UserInfo => decoded::<UserInfo>(&mut r, user_info_fields),
        ServerOpcode::ShortCutInit => decoded::<ShortcutInit>(&mut r, shortcut_init_fields),
        ServerOpcode::SkillList => decoded::<SkillList>(&mut r, skill_list_fields),
        ServerOpcode::SystemMessage => decoded::<SystemMessage>(&mut r, system_message_fields),
        ServerOpcode::RestartResponse => decoded::<RestartResponse>(&mut r, restart_response_fields),
        ServerOpcode::NetPing => decoded::<NetPing>(&mut r, net_ping_fields),
        ServerOpcode::LogOutOk => decoded::<LeaveWorld>(&mut r, no_fields),
        ServerOpcode::L2FriendList => decoded::<FriendList>(&mut r, friend_list_fields),
        ServerOpcode::ValidateLocation => decoded::<ValidateLocation>(&mut r, validate_location_fields),
        ServerOpcode::QuestList => decoded::<QuestList>(&mut r, quest_list_fields),
        ServerOpcode::SkillCoolTime => decoded::<SkillCoolTime>(&mut r, skill_cool_time_fields),
        ServerOpcode::HennaInfo => decoded::<HennaInfo>(&mut r, henna_info_fields),
        ServerOpcode::EtcStatusUpdate => decoded::<EtcStatusUpdate>(&mut r, etc_status_fields),
        ServerOpcode::ExStorageMaxCount => decoded::<ExStorageMaxCount>(&mut r, storage_max_count_fields),
        ServerOpcode::ExBasicActionList => decoded::<ExBasicActionList>(&mut r, basic_action_list_fields),
        ServerOpcode::ExVoteSystemInfo => decoded::<ExVoteSystemInfo>(&mut r, vote_system_info_fields),
        ServerOpcode::ExGetBookmarkInfo => decoded::<ExGetBookMarkInfoPacket>(&mut r, bookmark_info_fields),
        ServerOpcode::ExUiSetting => decoded::<ExUiSetting>(&mut r, ui_setting_fields),
        ServerOpcode::ExShowScreenMessage => decoded::<ExShowScreenMessage>(&mut r, screen_message_fields),
        // SAY2 (`CreatureSay`) has an encoder only, and the rest have no struct yet.
        _ => return None,
    })
}

fn key_packet_fields(p: &KeyPacket) -> Value {
    json!({
        "result": p.result,
        "key": hex::encode(p.key),
        "use_encryption": p.use_encryption,
        "server_id": p.server_id,
    })
}

fn char_selection_fields(p: &CharSelectionInfo) -> Value {
    // The account name and the session id are written inside every entry; they are never shown.
    let characters: Vec<Value> = p
        .characters
        .iter()
        .map(|c| {
            object!(
                c => name, object_id, clan_id, sex, race, base_class_id, x, y, z, current_hp, current_mp, sp, exp,
                exp_percent, level, karma, pk_kills, pvp_kills, paperdoll, hair_style, hair_color, face, max_hp,
                max_mp, delete_seconds, class_id, selected, enchant_effect, augmentation_id, vitality_points
            )
        })
        .collect();
    json!({
        "account_name": REDACTED,
        "session_id": REDACTED,
        "max_characters": p.max_characters,
        "characters": characters,
    })
}

fn new_character_fields(p: &NewCharacterSuccess) -> Value {
    let templates: Vec<Value> = p
        .templates
        .iter()
        .map(|t| object!(t => race, class_id, base_str, base_dex, base_con, base_int, base_wit, base_men))
        .collect();
    json!({ "templates": templates })
}

fn reason_fields_create(p: &CharCreateFail) -> Value {
    object!(p => reason)
}

fn reason_fields_delete(p: &CharDeleteFail) -> Value {
    object!(p => reason)
}

fn char_selected_fields(p: &CharSelected) -> Value {
    // The play session id is redacted like the one in the character list.
    let mut fields = object!(
        p => name, object_id, title, clan_id, is_female, race, class_id, x, y, z, current_hp, current_mp, sp, exp,
        level, karma, pk_kills, int, str, con, men, dex, wit, game_time
    );
    fields["session_id"] = json!(REDACTED);
    fields
}

fn item_fields(item: &ItemInfo) -> Value {
    object!(
        item => object_id, item_id, location, count, type2, custom_type1, equipped, body_part, enchant,
        custom_type2, augmentation, mana, time, attack_element_type, attack_element_power, element_defense,
        enchant_options
    )
}

fn block_fields(block: &InventoryBlock) -> Value {
    object!(block => mode, items)
}

fn item_list_fields(p: &ItemList) -> Value {
    let items: Vec<Value> = p.items.iter().map(item_fields).collect();
    let block = p.block.as_ref().map_or(Value::Null, block_fields);
    json!({ "show_window": p.show_window, "items": items, "block": block })
}

fn user_info_fields(p: &UserInfo) -> Value {
    object!(
        p => x, y, z, vehicle_id, object_id, name, race, is_female, base_class, level, exp, exp_percent, str, dex,
        con, int, wit, men, max_hp, current_hp, max_mp, current_mp, sp, current_load, max_load, weapon_equipped,
        paperdoll_object_ids, paperdoll_item_ids, paperdoll_augmentation_ids, talisman_slots, can_equip_cloak,
        p_atk, p_atk_spd, p_def, evasion, accuracy, critical, m_atk, m_atk_spd, m_def, pvp_flag, karma, run_speed,
        walk_speed, swim_run_speed, swim_walk_speed, fly_run_speed, fly_walk_speed, move_multiplier,
        attack_speed_multiplier, collision_radius, collision_height, hair_style, hair_color, face, is_gm, title,
        clan_id, clan_crest_id, ally_id, ally_crest_id, relation, mount_type, private_store_type,
        has_dwarven_craft, pk_kills, pvp_kills, cubics, in_party_match_room, abnormal_visual_effects,
        water_or_flying, clan_privileges, recom_left, recom_have, mount_npc_id, inventory_limit, class_id, max_cp,
        current_cp, enchant_effect, team, clan_crest_large_id, noble, hero, fishing, fish_x, fish_y, fish_z,
        name_color, running, pledge_class, pledge_type, title_color, cursed_weapon_level,
        transformation_display_id, attack_element, attack_element_value, defense_fire, defense_water, defense_wind,
        defense_earth, defense_holy, defense_dark, agathion_id, fame, minimap_allowed, vitality_points,
        abnormal_visual_effect_special
    )
}

fn shortcut_fields(shortcut: &ShortcutEntry) -> Value {
    match *shortcut {
        ShortcutEntry::Empty { slot } => json!({ "kind": "empty", "slot": slot }),
        ShortcutEntry::Item {
            slot,
            id,
            shared_reuse_group,
        } => json!({ "kind": "item", "slot": slot, "id": id, "shared_reuse_group": shared_reuse_group }),
        ShortcutEntry::Skill { slot, id, level } => {
            json!({ "kind": "skill", "slot": slot, "id": id, "level": level })
        }
        ShortcutEntry::Other { kind, slot, id } => {
            json!({ "kind": "other", "type": kind as i32, "slot": slot, "id": id })
        }
    }
}

fn shortcut_init_fields(p: &ShortcutInit) -> Value {
    let shortcuts: Vec<Value> = p.shortcuts.iter().map(shortcut_fields).collect();
    json!({ "shortcuts": shortcuts })
}

fn skill_list_fields(p: &SkillList) -> Value {
    let skills: Vec<Value> = p
        .skills
        .iter()
        .map(|s| object!(s => id, level, passive, disabled, enchanted))
        .collect();
    json!({ "skills": skills })
}

fn system_message_fields(p: &SystemMessage) -> Value {
    let params: Vec<Value> = p.params.iter().map(param_fields).collect();
    json!({ "id": p.id.id(), "name": p.id.name(), "params": params })
}

/// Named after the Java `addXxx` method that wrote the parameter.
fn param_fields(param: &SystemMessageParam) -> Value {
    match param {
        SystemMessageParam::Text(v) => json!({ "type": "text", "value": v }),
        SystemMessageParam::PlayerName(v) => json!({ "type": "player_name", "value": v }),
        SystemMessageParam::CastleName(v) => json!({ "type": "castle_name", "value": v }),
        SystemMessageParam::DoorName(v) => json!({ "type": "door_name", "value": v }),
        SystemMessageParam::NpcName(v) => json!({ "type": "npc_name", "value": v }),
        SystemMessageParam::ItemName(v) => json!({ "type": "item_name", "value": v }),
        SystemMessageParam::SkillName { id, level } => {
            json!({ "type": "skill_name", "id": id, "level": level })
        }
        SystemMessageParam::ElementName(v) => json!({ "type": "element_name", "value": v }),
        SystemMessageParam::SystemString(v) => json!({ "type": "system_string", "value": v }),
        SystemMessageParam::InstanceName(v) => json!({ "type": "instance_name", "value": v }),
        SystemMessageParam::IntNumber(v) => json!({ "type": "int", "value": v }),
        SystemMessageParam::LongNumber(v) => json!({ "type": "long", "value": v }),
        SystemMessageParam::ZoneName { x, y, z } => json!({ "type": "zone_name", "x": x, "y": y, "z": z }),
    }
}

fn restart_response_fields(p: &RestartResponse) -> Value {
    object!(p => result)
}

fn net_ping_fields(p: &NetPing) -> Value {
    object!(p => game_time)
}

fn friend_list_fields(p: &FriendList) -> Value {
    let friends: Vec<Value> = p
        .friends
        .iter()
        .map(|f| object!(f => object_id, name, online))
        .collect();
    json!({ "friends": friends })
}

fn validate_location_fields(p: &ValidateLocation) -> Value {
    object!(p => object_id, x, y, z, heading)
}

fn quest_list_fields(p: &QuestList) -> Value {
    let quests: Vec<Value> = p.quests.iter().map(|q| object!(q => quest_id, status)).collect();
    json!({ "quests": quests })
}

fn skill_cool_time_fields(p: &SkillCoolTime) -> Value {
    let entries: Vec<Value> = p
        .entries
        .iter()
        .map(|e| object!(e => skill_id, level, reuse_seconds, remaining_seconds))
        .collect();
    json!({ "entries": entries })
}

fn henna_info_fields(p: &HennaInfo) -> Value {
    object!(p => int, str, con, men, dex, wit, dye_ids)
}

fn etc_status_fields(p: &EtcStatusUpdate) -> Value {
    object!(
        p => charges, weight_penalty, chat_blocked, danger_area, expertise_weapon_penalty,
        expertise_armor_penalty, charm_of_courage, death_penalty_level, charged_souls
    )
}

fn storage_max_count_fields(p: &ExStorageMaxCount) -> Value {
    object!(
        p => inventory, warehouse, clan, private_sell, private_buy, dwarven_recipe, common_recipe,
        inventory_extra_slots, inventory_quest_items
    )
}

fn basic_action_list_fields(p: &ExBasicActionList) -> Value {
    object!(p => action_ids)
}

fn vote_system_info_fields(p: &ExVoteSystemInfo) -> Value {
    object!(p => recom_left, recom_have, bonus_time, bonus_value, bonus_type)
}

fn bookmark_info_fields(p: &ExGetBookMarkInfoPacket) -> Value {
    let bookmarks: Vec<Value> = p
        .bookmarks
        .iter()
        .map(|b| object!(b => id, x, y, z, name, icon, tag))
        .collect();
    json!({ "slot": p.slot, "bookmarks": bookmarks })
}

fn ui_setting_fields(p: &ExUiSetting) -> Value {
    json!({ "ui_key_mapping": hex::encode(&p.ui_key_mapping) })
}

fn screen_message_fields(p: &ExShowScreenMessage) -> Value {
    object!(
        p => kind, sys_message_id, position, unknown1, size, unknown2, unknown3, show_effect, time, fade,
        npc_string_id, text, params
    )
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use l2_net::Encode;
    use l2_protocol_game::SystemMessageId;
    use l2_protocol_game::client::{
        AuthLogin, CharacterCreate, CharacterDelete, CharacterRestore, CharacterSelect, EnterWorld, Logout,
        NewCharacter, ProtocolVersion, RequestGotoLobby, RequestKeyMapping, RequestNetPing, RequestRestart,
        Say2,
    };
    use l2_protocol_game::server::chat::{ChatType, CreatureSay, SayBody, SenderField};
    use l2_protocol_game::server::enterworld::{
        FriendListEntry, OtherShortcut, QuestListEntry, SkillCoolTimeEntry, SkillListEntry, TeleportBookmark,
    };
    use l2_protocol_game::server::{CharSelectEntry, NewCharTemplate};

    /// The ex id of a server packet the way the relay reads it.
    fn ex_of(raw: &[u8]) -> Option<u16> {
        (raw.first() == Some(&0xFE) && raw.len() >= 3).then(|| u16::from_le_bytes([raw[1], raw[2]]))
    }

    fn s2c(bytes: &[u8]) -> Option<Value> {
        describe(Dir::S2c, ConnectionState::InGame, ex_of(bytes), bytes, false)
    }

    fn s2c_fields(bytes: &[u8]) -> Value {
        s2c(bytes).expect("the packet has fields")
    }

    fn c2s(state: ConnectionState, bytes: &[u8]) -> Option<Value> {
        describe(Dir::C2s, state, None, bytes, false)
    }

    fn c2s_fields(state: ConnectionState, bytes: &[u8]) -> Value {
        c2s(state, bytes).expect("the packet has fields")
    }

    #[test]
    fn client_packets_show_their_fields() {
        use ConnectionState::{Authenticated, Connected, Entering, InGame};
        assert_eq!(
            c2s_fields(Connected, &ProtocolVersion { version: 273 }.to_bytes()),
            json!({ "version": 273 })
        );
        let create = CharacterCreate {
            name: "Neo".into(),
            race: 1,
            is_female: true,
            class_id: 2,
            int: 3,
            str: 4,
            con: 5,
            men: 6,
            dex: 7,
            wit: 8,
            hair_style: 1,
            hair_color: 2,
            face: 0,
        };
        let fields = c2s_fields(Authenticated, &create.to_bytes());
        assert_eq!(fields["name"], "Neo");
        assert_eq!(fields["is_female"], true);
        assert_eq!(fields["wit"], 8);
        assert_eq!(
            c2s_fields(Authenticated, &CharacterDelete { char_slot: 2 }.to_bytes()),
            json!({ "char_slot": 2 })
        );
        assert_eq!(
            c2s_fields(Authenticated, &CharacterRestore { char_slot: 1 }.to_bytes()),
            json!({ "char_slot": 1 })
        );
        let select = CharacterSelect {
            char_slot: 3,
            unk1: 1,
            unk2: 2,
            unk3: 3,
            unk4: 4,
        };
        assert_eq!(
            c2s_fields(Authenticated, &select.to_bytes()),
            json!({ "char_slot": 3, "unk1": 1, "unk2": 2, "unk3": 3, "unk4": 4 })
        );
        let enter = EnterWorld {
            unknown1: [0; 32],
            unknown2: [1, 2, 3, 4],
            unknown3: [0xAB; 32],
            unknown4: 9,
            tracert: [[10, 0, 0, 1]; 5],
        };
        let fields = c2s_fields(Entering, &enter.to_bytes());
        assert_eq!(fields["unknown2"], json!([1, 2, 3, 4]));
        assert_eq!(fields["unknown3"], hex::encode([0xAB; 32]));
        assert_eq!(fields["tracert"][0], json!([10, 0, 0, 1]));
        let say = Say2 {
            text: "hi".into(),
            chat_type: 2,
            target: Some("B".into()),
        };
        assert_eq!(
            c2s_fields(InGame, &say.to_bytes()),
            json!({ "text": "hi", "chat_type": 2, "target": "B" })
        );
        // Packets without fields are decoded, with an empty object.
        assert_eq!(c2s_fields(Authenticated, &NewCharacter.to_bytes()), json!({}));
        assert_eq!(c2s_fields(Authenticated, &RequestGotoLobby.to_bytes()), json!({}));
        assert_eq!(c2s_fields(Entering, &RequestKeyMapping.to_bytes()), json!({}));
        assert_eq!(c2s_fields(InGame, &Logout.to_bytes()), json!({}));
        assert_eq!(c2s_fields(InGame, &RequestRestart.to_bytes()), json!({}));
        assert_eq!(c2s_fields(InGame, &RequestNetPing.to_bytes()), json!({}));
    }

    #[test]
    fn a_client_packet_is_decoded_even_when_the_state_guess_lags() {
        // The dispatcher accepts ProtocolVersion only in CONNECTED, but the guess may say InGame
        // when the packet arrives, so the fields are still shown.
        assert_eq!(
            c2s_fields(
                ConnectionState::InGame,
                &ProtocolVersion { version: 1 }.to_bytes()
            ),
            json!({ "version": 1 })
        );
        // An ex id that no table entry has: no fields.
        assert_eq!(c2s(ConnectionState::InGame, &[0xD0, 0xFF, 0xFF]), None);
    }

    #[test]
    fn auth_login_is_redacted_whatever_the_raw_bytes_are() {
        let login = AuthLogin {
            login_name: "secretaccount".into(),
            play_key2: 0x1111,
            play_key1: 0x2222,
            login_key1: 0x3333,
            login_key2: 0x4444,
        };
        let full = login.to_bytes();
        let marker = json!({ "credentials": REDACTED });
        // The full body (no redaction flag): the opcode check alone must hide it.
        let fields = c2s(ConnectionState::Connected, &full).expect("redacted");
        assert_eq!(fields, marker);
        assert!(!fields.to_string().contains("secretaccount"));
        // The cut-down body the relay records, with the flag set.
        let cut = [ClientOpcode::AuthLogin.id()];
        assert_eq!(
            describe(Dir::C2s, ConnectionState::Connected, None, &cut, true),
            Some(marker)
        );
    }

    #[test]
    fn server_packets_show_their_fields() {
        let key = KeyPacket {
            result: KeyPacket::RESULT_OK,
            key: [1, 2, 3, 4, 5, 6, 7, 8],
            use_encryption: true,
            server_id: 2,
        };
        let fields = s2c_fields(&key.to_bytes());
        assert_eq!(fields["key"], "0102030405060708");
        assert_eq!(fields["use_encryption"], true);
        assert_eq!(fields["server_id"], 2);

        let templates = NewCharacterSuccess {
            templates: vec![NewCharTemplate {
                race: 0,
                class_id: 88,
                base_str: 40,
                base_dex: 30,
                base_con: 43,
                base_int: 21,
                base_wit: 11,
                base_men: 25,
            }],
        };
        assert_eq!(s2c_fields(&templates.to_bytes())["templates"][0]["base_str"], 40);

        assert_eq!(
            s2c_fields(&CharCreateFail { reason: 2 }.to_bytes()),
            json!({ "reason": 2 })
        );
        assert_eq!(
            s2c_fields(&CharDeleteFail { reason: 1 }.to_bytes()),
            json!({ "reason": 1 })
        );
        assert_eq!(s2c_fields(&CharCreateOk.to_bytes()), json!({}));
        assert_eq!(s2c_fields(&CharDeleteSuccess.to_bytes()), json!({}));
        assert_eq!(s2c_fields(&ServerClose.to_bytes()), json!({}));
        assert_eq!(s2c_fields(&LeaveWorld.to_bytes()), json!({}));
        assert_eq!(
            s2c_fields(&RestartResponse { result: true }.to_bytes()),
            json!({ "result": true })
        );
        assert_eq!(
            s2c_fields(&NetPing { game_time: 99 }.to_bytes()),
            json!({ "game_time": 99 })
        );
        assert_eq!(
            s2c_fields(
                &ValidateLocation {
                    object_id: 7,
                    x: 1,
                    y: 2,
                    z: 3,
                    heading: 4,
                }
                .to_bytes()
            ),
            json!({ "object_id": 7, "x": 1, "y": 2, "z": 3, "heading": 4 })
        );
    }

    #[test]
    fn character_list_hides_the_account_name_and_session_id() {
        let list = CharSelectionInfo {
            account_name: "secretaccount".into(),
            session_id: 0x5EED,
            max_characters: 7,
            characters: vec![CharSelectEntry {
                name: "Hero".into(),
                object_id: 268_435_456,
                level: 20,
                ..CharSelectEntry::default()
            }],
        };
        let fields = s2c_fields(&list.to_bytes());
        assert_eq!(fields["account_name"], REDACTED);
        assert_eq!(fields["session_id"], REDACTED);
        assert_eq!(fields["max_characters"], 7);
        assert_eq!(fields["characters"][0]["name"], "Hero");
        assert_eq!(fields["characters"][0]["level"], 20);
        assert_eq!(
            fields["characters"][0]["paperdoll"].as_array().map(Vec::len),
            Some(25)
        );
        let text = fields.to_string();
        assert!(!text.contains("secretaccount"), "account name leaked");
        assert!(!text.contains("24301"), "session id leaked");
    }

    #[test]
    fn character_selection_hides_the_session_id() {
        let selected = CharSelected {
            name: "Hero".into(),
            object_id: 1,
            title: "t".into(),
            session_id: 0x5EED,
            clan_id: 0,
            is_female: false,
            race: 2,
            class_id: 3,
            x: 4,
            y: 5,
            z: 6,
            current_hp: 7.0,
            current_mp: 8.0,
            sp: 9,
            exp: 10,
            level: 11,
            karma: 12,
            pk_kills: 13,
            int: 14,
            str: 15,
            con: 16,
            men: 17,
            dex: 18,
            wit: 19,
            game_time: 20,
        };
        let fields = s2c_fields(&selected.to_bytes());
        assert_eq!(fields["session_id"], REDACTED);
        assert_eq!(fields["name"], "Hero");
        assert_eq!(fields["wit"], 19);
        assert!(!fields.to_string().contains("24301"));
    }

    #[test]
    fn item_list_and_user_info_show_their_fields() {
        let items = ItemList {
            show_window: true,
            items: vec![ItemInfo::plain(0x1000, 57, 100)],
            block: None,
        };
        let fields = s2c_fields(&items.to_bytes());
        assert_eq!(fields["show_window"], true);
        assert_eq!(fields["items"][0]["item_id"], 57);
        assert_eq!(fields["items"][0]["count"], 100);
        assert_eq!(fields["block"], Value::Null);

        let blocked = ItemList {
            show_window: false,
            items: Vec::new(),
            block: Some(InventoryBlock {
                mode: 1,
                items: vec![5],
            }),
        };
        assert_eq!(
            s2c_fields(&blocked.to_bytes())["block"],
            json!({ "mode": 1, "items": [5] })
        );

        let info = UserInfo {
            name: "Hero".into(),
            level: 20,
            x: -1,
            ..UserInfo::default()
        };
        let fields = s2c_fields(&info.to_bytes());
        assert_eq!(fields["name"], "Hero");
        assert_eq!(fields["level"], 20);
        assert_eq!(fields["x"], -1);
        assert_eq!(fields["paperdoll_item_ids"].as_array().map(Vec::len), Some(25));
    }

    #[test]
    fn lists_and_small_server_packets_show_their_fields() {
        let shortcuts = ShortcutInit {
            shortcuts: vec![
                ShortcutEntry::Empty { slot: 0 },
                ShortcutEntry::Skill {
                    slot: 1,
                    id: 3,
                    level: 2,
                },
                ShortcutEntry::Item {
                    slot: 2,
                    id: 5,
                    shared_reuse_group: 9,
                },
                ShortcutEntry::Other {
                    kind: OtherShortcut::Macro,
                    slot: 3,
                    id: 4,
                },
            ],
        };
        let fields = s2c_fields(&shortcuts.to_bytes());
        assert_eq!(fields["shortcuts"][0]["kind"], "empty");
        assert_eq!(
            fields["shortcuts"][1],
            json!({ "kind": "skill", "slot": 1, "id": 3, "level": 2 })
        );
        assert_eq!(fields["shortcuts"][2]["shared_reuse_group"], 9);
        assert_eq!(
            fields["shortcuts"][3],
            json!({ "kind": "other", "type": 4, "slot": 3, "id": 4 })
        );

        let skills = SkillList {
            skills: vec![SkillListEntry {
                id: 3,
                level: 2,
                passive: true,
                disabled: false,
                enchanted: true,
            }],
        };
        assert_eq!(
            s2c_fields(&skills.to_bytes())["skills"][0],
            json!({ "id": 3, "level": 2, "passive": true, "disabled": false, "enchanted": true })
        );

        let friends = FriendList {
            friends: vec![FriendListEntry {
                object_id: 8,
                name: "Pal".into(),
                online: true,
            }],
        };
        assert_eq!(
            s2c_fields(&friends.to_bytes())["friends"][0],
            json!({ "object_id": 8, "name": "Pal", "online": true })
        );

        let quests = QuestList {
            quests: vec![QuestListEntry {
                quest_id: 1,
                status: 2,
            }],
        };
        assert_eq!(
            s2c_fields(&quests.to_bytes())["quests"][0],
            json!({ "quest_id": 1, "status": 2 })
        );

        let cool = SkillCoolTime {
            entries: vec![SkillCoolTimeEntry {
                skill_id: 4,
                level: 1,
                reuse_seconds: 10,
                remaining_seconds: 5,
            }],
        };
        assert_eq!(
            s2c_fields(&cool.to_bytes())["entries"][0],
            json!({ "skill_id": 4, "level": 1, "reuse_seconds": 10, "remaining_seconds": 5 })
        );

        let henna = HennaInfo {
            int: 1,
            str: -2,
            dye_ids: vec![7],
            ..HennaInfo::default()
        };
        let fields = s2c_fields(&henna.to_bytes());
        assert_eq!(fields["str"], -2);
        assert_eq!(fields["dye_ids"], json!([7]));

        let etc = EtcStatusUpdate {
            charges: 3,
            danger_area: true,
            ..EtcStatusUpdate::default()
        };
        let fields = s2c_fields(&etc.to_bytes());
        assert_eq!(fields["charges"], 3);
        assert_eq!(fields["danger_area"], true);
    }

    #[test]
    fn ex_packets_show_their_fields() {
        let storage = ExStorageMaxCount {
            inventory: 80,
            ..ExStorageMaxCount::default()
        };
        assert_eq!(s2c_fields(&storage.to_bytes())["inventory"], 80);

        let actions = ExBasicActionList::default_list();
        assert_eq!(
            s2c_fields(&actions.to_bytes())["action_ids"]
                .as_array()
                .map(Vec::len),
            Some(74 + 99 + 16)
        );

        let vote = ExVoteSystemInfo {
            recom_left: 9,
            ..ExVoteSystemInfo::default()
        };
        assert_eq!(s2c_fields(&vote.to_bytes())["recom_left"], 9);

        let bookmarks = ExGetBookMarkInfoPacket {
            slot: 2,
            bookmarks: vec![TeleportBookmark {
                id: 1,
                x: 2,
                y: 3,
                z: 4,
                name: "home".into(),
                icon: 5,
                tag: "t".into(),
            }],
        };
        assert_eq!(
            s2c_fields(&bookmarks.to_bytes())["bookmarks"][0],
            json!({ "id": 1, "x": 2, "y": 3, "z": 4, "name": "home", "icon": 5, "tag": "t" })
        );

        let ui = ExUiSetting {
            ui_key_mapping: vec![1, 2, 3],
        };
        assert_eq!(s2c_fields(&ui.to_bytes()), json!({ "ui_key_mapping": "010203" }));

        let message = ExShowScreenMessage::text("hi", 5);
        let fields = s2c_fields(&message.to_bytes());
        assert_eq!(fields["kind"], 2);
        assert_eq!(fields["text"], "hi");
        assert_eq!(fields["time"], 5);
        assert_eq!(fields["npc_string_id"], -1);
    }

    #[test]
    fn system_message_shows_its_id_and_typed_params() {
        let message = SystemMessage::text("hello").with(SystemMessageParam::NpcName(5));
        let fields = s2c_fields(&message.to_bytes());
        assert_eq!(fields["id"], SystemMessageId::S12.id());
        assert_eq!(fields["name"], SystemMessageId::S12.name());
        assert_eq!(fields["params"][0], json!({ "type": "text", "value": "hello" }));
        // The wire value is 1000000 + id; the decoder gives back the plain id.
        assert_eq!(fields["params"][1], json!({ "type": "npc_name", "value": 5 }));
    }

    #[test]
    fn creature_say_has_no_decoder_so_no_fields() {
        let say = CreatureSay {
            sender_object_id: 1,
            chat_type: ChatType::General,
            sender: SenderField::CharId(2),
            message_id: -1,
            body: SayBody::Text("hi".into()),
        };
        assert_eq!(s2c(&say.to_bytes()), None);
    }

    #[test]
    fn packets_without_a_struct_or_with_a_bad_body() {
        // EX_DUMMY is a single 0xFE byte and has no struct.
        assert_eq!(s2c(&[0xFE]), None);
        // An opcode that is not in the table.
        assert_eq!(s2c(&[0x07, 1, 2]), None);
        // An empty payload.
        assert_eq!(s2c(&[]), None);
        assert_eq!(
            describe(Dir::C2s, ConnectionState::Connected, None, &[], false),
            None
        );
        // A known packet with a body that is too short: a decode error note, never a panic.
        let truncated = [ServerOpcode::CharacterCreateFail.id(), 1];
        let fields = s2c_fields(&truncated);
        assert!(fields["decode_error"].is_string(), "{fields}");
    }

    #[test]
    fn every_table_entry_survives_any_body() {
        // Forwarding must never depend on decoding: no opcode or body length may panic.
        for &state in ConnectionState::ALL {
            for opcode in 0..=u8::MAX {
                for len in 0..6 {
                    let raw: Vec<u8> = std::iter::once(opcode)
                        .chain(std::iter::repeat_n(0xFF, len))
                        .collect();
                    let _ = describe(Dir::C2s, state, None, &raw, false);
                    let _ = describe(Dir::S2c, state, ex_of(&raw), &raw, false);
                }
            }
        }
        for op in ServerOpcode::ALL {
            let mut raw = vec![op.id()];
            if let Some(ex) = op.ex_id() {
                raw.extend(ex.to_le_bytes());
            }
            let _ = s2c(&raw);
        }
    }
}
