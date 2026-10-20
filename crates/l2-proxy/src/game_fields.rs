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
    NewCharacterSuccess, ServerClose, SsqInfo,
};
use l2_protocol_game::server::chat::{LeaveWorld, NetPing, RestartResponse};
use l2_protocol_game::server::combat::{
    Attack, AttackHit, AutoAttackStart, AutoAttackStop, ChangeMoveType, ChangeWaitType, Die,
    ExUseSharedGroupItem, Revive,
};
use l2_protocol_game::server::effect_packets::FlyToLocation;
use l2_protocol_game::server::enterworld::{
    EtcStatusUpdate, ExBasicActionList, ExGetBookMarkInfoPacket, ExShowScreenMessage, ExStorageMaxCount,
    ExUiSetting, ExVoteSystemInfo, FriendList, HennaInfo, QuestList, ShortcutEntry, ShortcutInit,
    ShortcutRegister, SkillCoolTime, SkillList, ValidateLocation,
};
use l2_protocol_game::server::entry::{
    ExBrExtraUserInfo, ExNevitAdventPointInfo, ExNevitAdventTimeChange, ExQuestItemList, ExRotation,
    ExShowContactList, MacroCommandEntry, MacroEntry, MacroList,
};
use l2_protocol_game::server::ground_item::{DropItem, GetItem, SpawnItem};
use l2_protocol_game::server::handshake::{KeyPacket, LoginFail};
use l2_protocol_game::server::interaction::{
    ActionFailed, DoorInfo, DoorStatusUpdate, MyTargetSelected, NpcHtmlMessage, StaticObjectInfo,
    TargetSelected, TargetUnselected,
};
use l2_protocol_game::server::items::{InventoryBlock, ItemInfo, ItemList};
use l2_protocol_game::server::movement::{MoveToLocation, MoveToPawn, StopMove, TeleportToLocation};
use l2_protocol_game::server::skills::{
    AbnormalStatusUpdate, AcquireSkillDone, AcquireSkillInfo, AcquireSkillList, EffectIcon, ExAutoSoulShot,
    MagicSkillCanceled, MagicSkillLaunched, MagicSkillUse, PartySpelled, ShortBuffStatusUpdate,
    SkillPosition,
};
use l2_protocol_game::server::system_message::{ConfirmDlg, SystemMessage, SystemMessageParam};
use l2_protocol_game::server::userinfo::UserInfo;
use l2_protocol_game::server::visibility::{
    CharInfo, DeleteObject, ExSetCompassZoneCode, NpcInfo, RelationChanged, StatusUpdate,
};
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
        | ClientPacket::RequestNetPing(_)
        | ClientPacket::RequestSkillCoolTime(_) => json!({}),
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
        ClientPacket::Action(p) => object!(p => object_id, origin_x, origin_y, origin_z, action_id),
        ClientPacket::RequestTargetCanceld(p) => object!(p => unselect),
        ClientPacket::RequestBypassToServer(p) => object!(p => command),
        ClientPacket::RequestLinkHtml(p) => object!(p => link),
        ClientPacket::SendBypassBuildCmd(p) => object!(p => command),
        ClientPacket::BypassUserCmd(p) => object!(p => command),
        ClientPacket::MoveToLocation(p) => {
            object!(p => target_x, target_y, target_z, origin_x, origin_y, origin_z, movement_mode)
        }
        ClientPacket::ValidatePosition(p) => object!(p => x, y, z, heading, vehicle_id),
        ClientPacket::CannotMoveAnymore(p) => object!(p => x, y, z, heading),
        ClientPacket::Appearing(_) => json!({}),
        ClientPacket::AttackRequest(p) => object!(p => object_id, origin_x, origin_y, origin_z, attack_id),
        ClientPacket::RequestActionUse(p) => object!(p => action_id, ctrl_pressed, shift_pressed),
        ClientPacket::RequestRestartPoint(p) => object!(p => requested_point_type),
        ClientPacket::UseItem(p) => object!(p => object_id, ctrl_pressed),
        ClientPacket::RequestUnEquipItem(p) => object!(p => paperdoll_slot),
        ClientPacket::RequestDropItem(p) => object!(p => object_id, count, x, y, z),
        ClientPacket::RequestDestroyItem(p) => object!(p => object_id, count),
        ClientPacket::RequestItemList(_) => json!({}),
        ClientPacket::RequestMagicSkillUse(p) => object!(p => magic_id, ctrl_pressed, shift_pressed),
        ClientPacket::RequestAutoSoulShot(p) => object!(p => item_id, shot_type),
        ClientPacket::RequestShortCutReg(p) => object!(p => type_id, slot, id, level, character_type),
        ClientPacket::RequestShortCutDel(p) => object!(p => slot),
        ClientPacket::RequestAcquireSkill(p) => {
            let mut fields = object!(p => id, level, sub_type);
            fields["skill_type"] = json!(p.skill_type.id());
            fields
        }
        ClientPacket::RequestAcquireSkillInfo(p) => {
            let mut fields = object!(p => id, level);
            fields["skill_type"] = json!(p.skill_type.id());
            fields
        }
        ClientPacket::RequestDispel(p) => object!(p => object_id, skill_id, skill_level),
        ClientPacket::RequestVoteNew(p) => object!(p => target_id),
        ClientPacket::DlgAnswer(p) => object!(p => message_id, answer, requester_id),
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
        ServerOpcode::ShortCutRegister => decoded::<ShortcutRegister>(&mut r, shortcut_register_fields),
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
        ServerOpcode::LoginFail => decoded::<LoginFail>(&mut r, login_fail_fields),
        ServerOpcode::SsqInfo => decoded::<SsqInfo>(&mut r, ssq_info_fields),
        ServerOpcode::ActionFail => decoded::<ActionFailed>(&mut r, no_fields),
        ServerOpcode::MyTargetSelected => decoded::<MyTargetSelected>(&mut r, my_target_selected_fields),
        ServerOpcode::TargetSelected => decoded::<TargetSelected>(&mut r, target_selected_fields),
        ServerOpcode::TargetUnselected => decoded::<TargetUnselected>(&mut r, target_unselected_fields),
        ServerOpcode::NpcHtmlMessage => decoded::<NpcHtmlMessage>(&mut r, npc_html_message_fields),
        ServerOpcode::DoorInfo => decoded::<DoorInfo>(&mut r, door_info_fields),
        ServerOpcode::DoorStatusUpdate => decoded::<DoorStatusUpdate>(&mut r, door_status_update_fields),
        ServerOpcode::StaticObject => decoded::<StaticObjectInfo>(&mut r, static_object_info_fields),
        ServerOpcode::CharInfo => decoded::<CharInfo>(&mut r, char_info_fields),
        ServerOpcode::NpcInfo => decoded::<NpcInfo>(&mut r, npc_info_fields),
        ServerOpcode::DeleteObject => decoded::<DeleteObject>(&mut r, delete_object_fields),
        ServerOpcode::RelationChanged => decoded::<RelationChanged>(&mut r, relation_changed_fields),
        ServerOpcode::StatusUpdate => decoded::<StatusUpdate>(&mut r, status_update_fields),
        ServerOpcode::ExSetCompassZoneCode => decoded::<ExSetCompassZoneCode>(&mut r, compass_zone_fields),
        ServerOpcode::MoveToLocation => decoded::<MoveToLocation>(&mut r, move_to_location_fields),
        ServerOpcode::StopMove => decoded::<StopMove>(&mut r, stop_move_fields),
        ServerOpcode::MoveToPawn => decoded::<MoveToPawn>(&mut r, move_to_pawn_fields),
        ServerOpcode::TeleportToLocation => {
            decoded::<TeleportToLocation>(&mut r, teleport_to_location_fields)
        }
        ServerOpcode::MacroList => decoded::<MacroList>(&mut r, macro_list_fields),
        ServerOpcode::ExQuestItemList => decoded::<ExQuestItemList>(&mut r, quest_item_list_fields),
        ServerOpcode::ExRotation => decoded::<ExRotation>(&mut r, rotation_fields),
        ServerOpcode::ExReceiveShowPostFriend => decoded::<ExShowContactList>(&mut r, contact_list_fields),
        ServerOpcode::ExBrExtraUserInfo => decoded::<ExBrExtraUserInfo>(&mut r, br_extra_user_info_fields),
        ServerOpcode::ExNavitAdventPointInfo => decoded::<ExNevitAdventPointInfo>(&mut r, nevit_point_fields),
        ServerOpcode::ExNavitAdventTimeChange => {
            decoded::<ExNevitAdventTimeChange>(&mut r, nevit_time_fields)
        }
        ServerOpcode::Attack => decoded::<Attack>(&mut r, attack_fields),
        ServerOpcode::AutoAttackStart => decoded::<AutoAttackStart>(&mut r, auto_attack_start_fields),
        ServerOpcode::AutoAttackStop => decoded::<AutoAttackStop>(&mut r, auto_attack_stop_fields),
        ServerOpcode::Die => decoded::<Die>(&mut r, die_fields),
        ServerOpcode::Revive => decoded::<Revive>(&mut r, revive_fields),
        ServerOpcode::ChangeMoveType => decoded::<ChangeMoveType>(&mut r, change_move_type_fields),
        ServerOpcode::ChangeWaitType => decoded::<ChangeWaitType>(&mut r, change_wait_type_fields),
        ServerOpcode::ExUseSharedGroupItem => {
            decoded::<ExUseSharedGroupItem>(&mut r, shared_group_item_fields)
        }
        ServerOpcode::SpawnItem => decoded::<SpawnItem>(&mut r, spawn_item_fields),
        ServerOpcode::DropItem => decoded::<DropItem>(&mut r, drop_item_fields),
        ServerOpcode::GetItem => decoded::<GetItem>(&mut r, get_item_fields),
        ServerOpcode::MagicSkillUse => decoded::<MagicSkillUse>(&mut r, magic_skill_use_fields),
        ServerOpcode::MagicSkillLaunched => {
            decoded::<MagicSkillLaunched>(&mut r, magic_skill_launched_fields)
        }
        ServerOpcode::MagicSkillCanceled => {
            decoded::<MagicSkillCanceled>(&mut r, magic_skill_canceled_fields)
        }
        ServerOpcode::AbnormalStatusUpdate => decoded::<AbnormalStatusUpdate>(&mut r, abnormal_status_fields),
        ServerOpcode::ShortBuffStatusUpdate => decoded::<ShortBuffStatusUpdate>(&mut r, short_buff_fields),
        ServerOpcode::ExAutoSoulshot => decoded::<ExAutoSoulShot>(&mut r, auto_soulshot_fields),
        ServerOpcode::AcquireSkillList => decoded::<AcquireSkillList>(&mut r, acquire_skill_list_fields),
        ServerOpcode::AcquireSkillInfo => decoded::<AcquireSkillInfo>(&mut r, acquire_skill_info_fields),
        ServerOpcode::AcquireSkillDone => decoded::<AcquireSkillDone>(&mut r, no_fields),
        ServerOpcode::PartySpelled => decoded::<PartySpelled>(&mut r, party_spelled_fields),
        ServerOpcode::ConfirmDlg => decoded::<ConfirmDlg>(&mut r, confirm_dlg_fields),
        ServerOpcode::FlyToLocation => decoded::<FlyToLocation>(&mut r, fly_to_location_fields),
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

fn shortcut_register_fields(p: &ShortcutRegister) -> Value {
    match *p {
        ShortcutRegister::Empty { slot } => json!({ "kind": "empty", "slot": slot }),
        ShortcutRegister::Item {
            slot,
            id,
            character_type,
            shared_reuse_group,
        } => json!({
            "kind": "item", "slot": slot, "id": id,
            "character_type": character_type, "shared_reuse_group": shared_reuse_group,
        }),
        ShortcutRegister::Skill {
            slot,
            id,
            level,
            character_type,
        } => json!({
            "kind": "skill", "slot": slot, "id": id, "level": level, "character_type": character_type,
        }),
        ShortcutRegister::Other {
            kind,
            slot,
            id,
            character_type,
        } => json!({
            "kind": "other", "type": kind as i32, "slot": slot, "id": id, "character_type": character_type,
        }),
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

fn confirm_dlg_fields(p: &ConfirmDlg) -> Value {
    let mut fields = system_message_fields(&p.message);
    fields["time"] = json!(p.time);
    fields["requester_id"] = json!(p.requester_id);
    fields
}

fn fly_to_location_fields(p: &FlyToLocation) -> Value {
    object!(p => object_id, dest_x, dest_y, dest_z, org_x, org_y, org_z, fly_type)
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

fn login_fail_fields(p: &LoginFail) -> Value {
    object!(p => reason)
}

fn ssq_info_fields(p: &SsqInfo) -> Value {
    object!(p => state)
}

// Visibility: the packets that put other objects (players, NPCs, doors) on the client's screen.

fn char_info_fields(p: &CharInfo) -> Value {
    object!(
        p => x, y, z, vehicle_id, object_id, name, race, is_female, base_class, paperdoll_item_ids,
        paperdoll_augmentation_ids, talisman_slots, can_equip_cloak, pvp_flag, karma, m_atk_spd, p_atk_spd,
        run_speed, walk_speed, swim_run_speed, swim_walk_speed, fly_run_speed, fly_walk_speed, move_multiplier,
        attack_speed_multiplier, collision_radius, collision_height, hair_style, hair_color, face, title, clan_id,
        clan_crest_id, ally_id, ally_crest_id, sitting, running, in_combat, alike_dead, invisible, mount_type,
        private_store_type, cubics, in_party_match_room, abnormal_visual_effects, water_or_flying, recom_have,
        mount_npc_id, class_id, enchant_effect, team, clan_crest_large_id, noble, hero, fishing, fish_x, fish_y,
        fish_z, name_color, heading, pledge_class, pledge_type, title_color, cursed_weapon_level,
        clan_reputation_score, transformation_display_id, agathion_id, abnormal_visual_effect_special
    )
}

fn npc_info_fields(p: &NpcInfo) -> Value {
    object!(
        p => object_id, display_id, attackable, x, y, z, heading, m_atk_spd, p_atk_spd, run_speed, walk_speed,
        swim_run_speed, swim_walk_speed, fly_run_speed, fly_walk_speed, move_multiplier, attack_speed_multiplier,
        collision_radius, collision_height, right_hand, left_hand, running, in_combat, alike_dead,
        show_summon_animation, name, title, abnormal_visual_effects, clan_id, clan_crest_id, ally_id,
        ally_crest_id, water_or_flying, team, enchant_effect, flying, color_effect, targetable, show_name,
        abnormal_visual_effect_special, display_effect
    )
}

fn delete_object_fields(p: &DeleteObject) -> Value {
    object!(p => object_id)
}

fn relation_changed_fields(p: &RelationChanged) -> Value {
    let relations: Vec<Value> = p
        .relations
        .iter()
        .map(|r| object!(r => object_id, relation, auto_attackable, karma, pvp_flag))
        .collect();
    json!({ "relations": relations })
}

fn status_update_fields(p: &StatusUpdate) -> Value {
    let attributes: Vec<Value> = p
        .attributes
        .iter()
        .map(|&(id, value)| json!({ "id": id, "value": value }))
        .collect();
    json!({ "object_id": p.object_id, "attributes": attributes })
}

fn compass_zone_fields(p: &ExSetCompassZoneCode) -> Value {
    object!(p => zone_type)
}

// Movement

fn move_to_location_fields(p: &MoveToLocation) -> Value {
    object!(p => object_id, dest_x, dest_y, dest_z, x, y, z)
}

fn stop_move_fields(p: &StopMove) -> Value {
    object!(p => object_id, x, y, z, heading)
}

fn move_to_pawn_fields(p: &MoveToPawn) -> Value {
    object!(p => object_id, target_id, distance, x, y, z, target_x, target_y, target_z)
}

fn teleport_to_location_fields(p: &TeleportToLocation) -> Value {
    object!(p => object_id, x, y, z, heading)
}

// Interaction: targets, dialogs, doors and other static objects.

fn my_target_selected_fields(p: &MyTargetSelected) -> Value {
    object!(p => object_id, color)
}

fn target_selected_fields(p: &TargetSelected) -> Value {
    object!(p => object_id, target_id, x, y, z)
}

fn target_unselected_fields(p: &TargetUnselected) -> Value {
    object!(p => target_id, x, y, z)
}

fn npc_html_message_fields(p: &NpcHtmlMessage) -> Value {
    object!(p => npc_obj_id, html, item_id)
}

fn door_info_fields(p: &DoorInfo) -> Value {
    object!(p => object_id, door_id)
}

fn door_status_update_fields(p: &DoorStatusUpdate) -> Value {
    object!(p => object_id, closed, damage, is_enemy, door_id, current_hp, max_hp)
}

fn static_object_info_fields(p: &StaticObjectInfo) -> Value {
    object!(
        p => static_object_id, object_id, kind, targetable, mesh_index, closed, is_enemy, current_hp, max_hp,
        show_hp, damage_grade
    )
}

// Enter-world sequence: macros, quest items, rotation, contacts and the Nevit packets.

fn macro_command_fields(c: &MacroCommandEntry) -> Value {
    object!(c => kind, d1, d2, cmd)
}

fn macro_entry_fields(m: &MacroEntry) -> Value {
    let commands: Vec<Value> = m.commands.iter().map(macro_command_fields).collect();
    let mut fields = object!(m => id, name, descr, acronym, icon);
    fields["commands"] = json!(commands);
    fields
}

fn macro_list_fields(p: &MacroList) -> Value {
    let entry = p.macro_entry.as_ref().map_or(Value::Null, macro_entry_fields);
    json!({ "revision": p.revision, "count": p.count, "macro": entry })
}

fn quest_item_list_fields(p: &ExQuestItemList) -> Value {
    let items: Vec<Value> = p.items.iter().map(item_fields).collect();
    let block = p.block.as_ref().map_or(Value::Null, block_fields);
    json!({ "items": items, "block": block })
}

fn rotation_fields(p: &ExRotation) -> Value {
    object!(p => char_id, heading)
}

fn contact_list_fields(p: &ExShowContactList) -> Value {
    object!(p => contacts)
}

fn br_extra_user_info_fields(p: &ExBrExtraUserInfo) -> Value {
    object!(p => object_id, abnormal_visual_effects_event, lecture_mark)
}

fn nevit_point_fields(p: &ExNevitAdventPointInfo) -> Value {
    object!(p => points)
}

fn nevit_time_fields(p: &ExNevitAdventTimeChange) -> Value {
    object!(p => started, time)
}

// Combat and items: attacks, death and revive, move and wait types, the ground items.

fn attack_hit_fields(hit: &AttackHit) -> Value {
    object!(hit => target_id, damage, flags)
}

fn attack_fields(p: &Attack) -> Value {
    let other_hits: Vec<Value> = p.other_hits.iter().map(attack_hit_fields).collect();
    let mut fields = object!(
        p => attacker_id, attacker_x, attacker_y, attacker_z, target_x, target_y, target_z
    );
    fields["first_hit"] = attack_hit_fields(&p.first_hit);
    fields["other_hits"] = json!(other_hits);
    fields
}

fn auto_attack_start_fields(p: &AutoAttackStart) -> Value {
    object!(p => target_id)
}

fn auto_attack_stop_fields(p: &AutoAttackStop) -> Value {
    object!(p => target_id)
}

fn die_fields(p: &Die) -> Value {
    object!(
        p => object_id, can_teleport, to_clan_hall, to_castle, to_siege_hq, sweepable, fixed_res, to_fortress
    )
}

fn revive_fields(p: &Revive) -> Value {
    object!(p => object_id)
}

fn change_move_type_fields(p: &ChangeMoveType) -> Value {
    object!(p => object_id, running)
}

fn change_wait_type_fields(p: &ChangeWaitType) -> Value {
    object!(p => object_id, move_type, x, y, z)
}

fn shared_group_item_fields(p: &ExUseSharedGroupItem) -> Value {
    object!(p => item_id, group_id, remaining_secs, total_secs)
}

fn spawn_item_fields(p: &SpawnItem) -> Value {
    object!(p => object_id, display_id, x, y, z, stackable, count)
}

fn drop_item_fields(p: &DropItem) -> Value {
    object!(p => player_id, object_id, display_id, x, y, z, stackable, count)
}

fn get_item_fields(p: &GetItem) -> Value {
    object!(p => player_id, object_id, x, y, z)
}

// Skills and buffs: casting, buff icons, the learn lists and the soulshot switch.

fn skill_position_fields(p: &SkillPosition) -> Value {
    object!(p => x, y, z)
}

fn magic_skill_use_fields(p: &MagicSkillUse) -> Value {
    let mut fields = object!(p => creature_id, target_id, skill_id, skill_level, hit_time, reuse_delay);
    fields["creature"] = skill_position_fields(&p.creature);
    fields["ground_location"] = p
        .ground_location
        .as_ref()
        .map_or(Value::Null, skill_position_fields);
    fields["target"] = skill_position_fields(&p.target);
    fields
}

fn magic_skill_launched_fields(p: &MagicSkillLaunched) -> Value {
    object!(p => creature_id, skill_id, skill_level, target_ids)
}

fn magic_skill_canceled_fields(p: &MagicSkillCanceled) -> Value {
    object!(p => object_id)
}

fn effect_icon_fields(icon: &EffectIcon) -> Value {
    object!(icon => display_id, display_level, time, in_use)
}

fn abnormal_status_fields(p: &AbnormalStatusUpdate) -> Value {
    let effects: Vec<Value> = p.effects.iter().map(effect_icon_fields).collect();
    json!({ "effects": effects })
}

fn short_buff_fields(p: &ShortBuffStatusUpdate) -> Value {
    object!(p => skill_id, skill_level, duration)
}

fn auto_soulshot_fields(p: &ExAutoSoulShot) -> Value {
    object!(p => item_id, shot_type)
}

fn acquire_skill_list_fields(p: &AcquireSkillList) -> Value {
    let skills: Vec<Value> = p
        .skills
        .iter()
        .map(|s| object!(s => id, next_level, max_level, sp_cost, requirements))
        .collect();
    json!({ "skill_type": p.skill_type.id(), "skills": skills })
}

fn acquire_skill_info_fields(p: &AcquireSkillInfo) -> Value {
    let requirements: Vec<Value> = p
        .requirements
        .iter()
        .map(|r| object!(r => req_type, item_id, count, unk))
        .collect();
    let mut fields = object!(p => skill_id, level, sp_cost);
    fields["skill_type"] = json!(p.skill_type.id());
    fields["requirements"] = json!(requirements);
    fields
}

fn party_spelled_fields(p: &PartySpelled) -> Value {
    let effects: Vec<Value> = p.effects.iter().map(effect_icon_fields).collect();
    let mut fields = object!(p => creature_type, creature_id);
    fields["effects"] = json!(effects);
    fields
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
        AttackRequest, AuthLogin, CharacterCreate, CharacterDelete, CharacterRestore, CharacterSelect,
        EnterWorld, Logout, NewCharacter, ProtocolVersion, RequestAcquireSkill, RequestAcquireSkillInfo,
        RequestActionUse, RequestAutoSoulShot, RequestDestroyItem, RequestDispel, RequestDropItem,
        RequestGotoLobby, RequestItemList, RequestKeyMapping, RequestMagicSkillUse, RequestNetPing,
        RequestRestart, RequestRestartPoint, RequestShortCutDel, RequestShortCutReg, RequestUnEquipItem,
        Say2, UseItem,
    };
    use l2_protocol_game::server::chat::{ChatType, CreatureSay, SayBody, SenderField};
    use l2_protocol_game::server::combat::{HITFLAG_CRIT, HITFLAG_MISS};
    use l2_protocol_game::server::enterworld::{
        FriendListEntry, OtherShortcut, QuestListEntry, SkillCoolTimeEntry, SkillListEntry, TeleportBookmark,
    };
    use l2_protocol_game::server::skills::{AcquireSkillRequirement, AcquireSkillType};
    use l2_protocol_game::server::visibility::{RelationEntry, compass_zone, relation};
    use l2_protocol_game::server::{CharSelectEntry, NewCharTemplate};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use crate::capture::CaptureReader;

    /// The server packets that have a decoder in `describe_server`. A test checks that the
    /// match agrees with this list, so a new decoder has to be added to both.
    const DECODED_SERVER_PACKETS: &[ServerOpcode] = &[
        ServerOpcode::VersionCheck,
        ServerOpcode::CharacterSelectionInfo,
        ServerOpcode::NewCharacterSuccess,
        ServerOpcode::CharacterCreateSuccess,
        ServerOpcode::CharacterCreateFail,
        ServerOpcode::CharacterDeleteSuccess,
        ServerOpcode::CharacterDeleteFail,
        ServerOpcode::CharacterSelected,
        ServerOpcode::ServerClose,
        ServerOpcode::ItemList,
        ServerOpcode::UserInfo,
        ServerOpcode::ShortCutInit,
        ServerOpcode::ShortCutRegister,
        ServerOpcode::SkillList,
        ServerOpcode::SystemMessage,
        ServerOpcode::RestartResponse,
        ServerOpcode::NetPing,
        ServerOpcode::LogOutOk,
        ServerOpcode::L2FriendList,
        ServerOpcode::ValidateLocation,
        ServerOpcode::QuestList,
        ServerOpcode::SkillCoolTime,
        ServerOpcode::HennaInfo,
        ServerOpcode::EtcStatusUpdate,
        ServerOpcode::ExStorageMaxCount,
        ServerOpcode::ExBasicActionList,
        ServerOpcode::ExVoteSystemInfo,
        ServerOpcode::ExGetBookmarkInfo,
        ServerOpcode::ExUiSetting,
        ServerOpcode::ExShowScreenMessage,
        ServerOpcode::LoginFail,
        ServerOpcode::SsqInfo,
        ServerOpcode::ActionFail,
        ServerOpcode::MyTargetSelected,
        ServerOpcode::TargetSelected,
        ServerOpcode::TargetUnselected,
        ServerOpcode::NpcHtmlMessage,
        ServerOpcode::DoorInfo,
        ServerOpcode::DoorStatusUpdate,
        ServerOpcode::StaticObject,
        ServerOpcode::CharInfo,
        ServerOpcode::NpcInfo,
        ServerOpcode::DeleteObject,
        ServerOpcode::RelationChanged,
        ServerOpcode::StatusUpdate,
        ServerOpcode::ExSetCompassZoneCode,
        ServerOpcode::MoveToLocation,
        ServerOpcode::StopMove,
        ServerOpcode::MoveToPawn,
        ServerOpcode::TeleportToLocation,
        ServerOpcode::MacroList,
        ServerOpcode::ExQuestItemList,
        ServerOpcode::ExRotation,
        ServerOpcode::ExReceiveShowPostFriend,
        ServerOpcode::ExBrExtraUserInfo,
        ServerOpcode::ExNavitAdventPointInfo,
        ServerOpcode::ExNavitAdventTimeChange,
        ServerOpcode::Attack,
        ServerOpcode::AutoAttackStart,
        ServerOpcode::AutoAttackStop,
        ServerOpcode::Die,
        ServerOpcode::Revive,
        ServerOpcode::ChangeMoveType,
        ServerOpcode::ChangeWaitType,
        ServerOpcode::ExUseSharedGroupItem,
        ServerOpcode::SpawnItem,
        ServerOpcode::DropItem,
        ServerOpcode::GetItem,
        ServerOpcode::MagicSkillUse,
        ServerOpcode::MagicSkillLaunched,
        ServerOpcode::MagicSkillCanceled,
        ServerOpcode::AbnormalStatusUpdate,
        ServerOpcode::ShortBuffStatusUpdate,
        ServerOpcode::ExAutoSoulshot,
        ServerOpcode::AcquireSkillList,
        ServerOpcode::AcquireSkillInfo,
        ServerOpcode::AcquireSkillDone,
        ServerOpcode::PartySpelled,
        ServerOpcode::ConfirmDlg,
        ServerOpcode::FlyToLocation,
    ];

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

        let register = ShortcutRegister::Skill {
            slot: 14,
            id: 1204,
            level: 7,
            character_type: 1,
        };
        assert_eq!(
            s2c_fields(&register.to_bytes()),
            json!({ "kind": "skill", "slot": 14, "id": 1204, "level": 7, "character_type": 1 })
        );
        let register = ShortcutRegister::Item {
            slot: 2,
            id: 77,
            character_type: 1,
            shared_reuse_group: -1,
        };
        assert_eq!(
            s2c_fields(&register.to_bytes()),
            json!({ "kind": "item", "slot": 2, "id": 77, "character_type": 1, "shared_reuse_group": -1 })
        );
        let register = ShortcutRegister::Other {
            kind: OtherShortcut::Recipe,
            slot: 3,
            id: 4,
            character_type: 2,
        };
        assert_eq!(
            s2c_fields(&register.to_bytes()),
            json!({ "kind": "other", "type": 5, "slot": 3, "id": 4, "character_type": 2 })
        );
        assert_eq!(
            s2c_fields(&ShortcutRegister::Empty { slot: 9 }.to_bytes()),
            json!({ "kind": "empty", "slot": 9 })
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

    #[test]
    fn the_decoder_match_agrees_with_the_list_of_decoded_packets() {
        for op in ServerOpcode::ALL {
            let mut raw = vec![op.id()];
            if let Some(ex) = op.ex_id() {
                raw.extend(ex.to_le_bytes());
            }
            // An empty body: a decoder still answers, with a decode_error, so `None` means
            // "no decoder".
            assert_eq!(
                s2c(&raw).is_some(),
                DECODED_SERVER_PACKETS.contains(&op),
                "{}",
                op.name()
            );
        }
    }

    #[test]
    fn visibility_packets_show_their_fields() {
        let char_info = CharInfo {
            name: "Hero".into(),
            object_id: 268_435_457,
            x: 1,
            y: -2,
            z: 3,
            mount_npc_id: 12_345,
            running: true,
            ..CharInfo::default()
        };
        let fields = s2c_fields(&char_info.to_bytes());
        assert_eq!(fields["name"], "Hero");
        assert_eq!(fields["object_id"], 268_435_457);
        assert_eq!(fields["y"], -2);
        assert_eq!(fields["mount_npc_id"], 12_345);
        assert_eq!(fields["running"], true);
        assert_eq!(fields["paperdoll_item_ids"].as_array().map(Vec::len), Some(25));

        let npc = NpcInfo {
            object_id: 5,
            display_id: 20_101,
            attackable: true,
            name: "Guard".into(),
            x: 7,
            ..NpcInfo::default()
        };
        let fields = s2c_fields(&npc.to_bytes());
        assert_eq!(fields["object_id"], 5);
        assert_eq!(fields["display_id"], 20_101);
        assert_eq!(fields["attackable"], true);
        assert_eq!(fields["name"], "Guard");
        assert_eq!(fields["x"], 7);

        assert_eq!(
            s2c_fields(&DeleteObject { object_id: 9 }.to_bytes()),
            json!({ "object_id": 9 })
        );

        let entry = RelationEntry {
            object_id: 4,
            relation: relation::PARTY1,
            auto_attackable: true,
            karma: 0,
            pvp_flag: 1,
        };
        assert_eq!(
            s2c_fields(&RelationChanged::single(entry).to_bytes()),
            json!({
                "relations": [
                    { "object_id": 4, "relation": 1, "auto_attackable": true, "karma": 0, "pvp_flag": 1 }
                ]
            })
        );

        let mut status = StatusUpdate::new(10);
        status.add_attribute(9, 100);
        status.add_attribute(10, 50);
        assert_eq!(
            s2c_fields(&status.to_bytes()),
            json!({
                "object_id": 10,
                "attributes": [{ "id": 9, "value": 100 }, { "id": 10, "value": 50 }]
            })
        );

        let zone = ExSetCompassZoneCode {
            zone_type: compass_zone::PEACEZONE,
        };
        assert_eq!(s2c_fields(&zone.to_bytes()), json!({ "zone_type": 0x0C }));
    }

    #[test]
    fn movement_packets_show_their_fields() {
        let to = MoveToLocation {
            object_id: 1,
            dest_x: 10,
            dest_y: 20,
            dest_z: 30,
            x: 1,
            y: 2,
            z: 3,
        };
        assert_eq!(
            s2c_fields(&to.to_bytes()),
            json!({ "object_id": 1, "dest_x": 10, "dest_y": 20, "dest_z": 30, "x": 1, "y": 2, "z": 3 })
        );

        let stop = StopMove {
            object_id: 2,
            x: 4,
            y: 5,
            z: 6,
            heading: 7,
        };
        assert_eq!(
            s2c_fields(&stop.to_bytes()),
            json!({ "object_id": 2, "x": 4, "y": 5, "z": 6, "heading": 7 })
        );

        let pawn = MoveToPawn {
            object_id: 3,
            target_id: 4,
            distance: 25,
            x: 1,
            y: 2,
            z: 3,
            target_x: 4,
            target_y: 5,
            target_z: 6,
        };
        assert_eq!(
            s2c_fields(&pawn.to_bytes()),
            json!({
                "object_id": 3, "target_id": 4, "distance": 25, "x": 1, "y": 2, "z": 3,
                "target_x": 4, "target_y": 5, "target_z": 6
            })
        );

        let teleport = TeleportToLocation {
            object_id: 8,
            x: 1,
            y: 2,
            z: 3,
            heading: 4,
        };
        assert_eq!(
            s2c_fields(&teleport.to_bytes()),
            json!({ "object_id": 8, "x": 1, "y": 2, "z": 3, "heading": 4 })
        );
    }

    #[test]
    fn interaction_packets_show_their_fields() {
        let mine = MyTargetSelected {
            object_id: 3,
            color: 2,
        };
        assert_eq!(
            s2c_fields(&mine.to_bytes()),
            json!({ "object_id": 3, "color": 2 })
        );

        let selected = TargetSelected {
            object_id: 3,
            target_id: 4,
            x: 1,
            y: 2,
            z: 3,
        };
        assert_eq!(
            s2c_fields(&selected.to_bytes()),
            json!({ "object_id": 3, "target_id": 4, "x": 1, "y": 2, "z": 3 })
        );

        let unselected = TargetUnselected {
            target_id: 4,
            x: 1,
            y: 2,
            z: 3,
        };
        assert_eq!(
            s2c_fields(&unselected.to_bytes()),
            json!({ "target_id": 4, "x": 1, "y": 2, "z": 3 })
        );

        assert_eq!(s2c_fields(&ActionFailed.to_bytes()), json!({}));

        let html = NpcHtmlMessage {
            npc_obj_id: 5,
            html: "<html>hi</html>".into(),
            item_id: 0,
        };
        assert_eq!(
            s2c_fields(&html.to_bytes()),
            json!({ "npc_obj_id": 5, "html": "<html>hi</html>", "item_id": 0 })
        );

        let door = DoorInfo {
            object_id: 6,
            door_id: 7,
        };
        assert_eq!(
            s2c_fields(&door.to_bytes()),
            json!({ "object_id": 6, "door_id": 7 })
        );

        let door_status = DoorStatusUpdate {
            object_id: 6,
            closed: true,
            damage: 0,
            is_enemy: false,
            door_id: 7,
            current_hp: 100,
            max_hp: 200,
        };
        assert_eq!(
            s2c_fields(&door_status.to_bytes()),
            json!({
                "object_id": 6, "closed": true, "damage": 0, "is_enemy": false, "door_id": 7,
                "current_hp": 100, "max_hp": 200
            })
        );

        let object = StaticObjectInfo {
            static_object_id: 1,
            object_id: 2,
            kind: 0,
            targetable: true,
            mesh_index: 3,
            closed: false,
            is_enemy: false,
            current_hp: 0,
            max_hp: 0,
            show_hp: false,
            damage_grade: 0,
        };
        let fields = s2c_fields(&object.to_bytes());
        assert_eq!(fields["static_object_id"], 1);
        assert_eq!(fields["mesh_index"], 3);
        assert_eq!(fields["targetable"], true);
    }

    #[test]
    fn enter_world_packets_show_their_fields() {
        let macros = MacroList {
            revision: 1,
            count: 1,
            macro_entry: Some(MacroEntry {
                id: 3,
                name: "heal".into(),
                descr: "d".into(),
                acronym: "h".into(),
                icon: 2,
                commands: vec![MacroCommandEntry {
                    kind: 3,
                    d1: 0,
                    d2: 0,
                    cmd: "/heal".into(),
                }],
            }),
        };
        let fields = s2c_fields(&macros.to_bytes());
        assert_eq!(fields["revision"], 1);
        assert_eq!(fields["macro"]["name"], "heal");
        assert_eq!(fields["macro"]["commands"][0]["cmd"], "/heal");
        let empty = MacroList {
            revision: 2,
            count: 0,
            macro_entry: None,
        };
        assert_eq!(s2c_fields(&empty.to_bytes())["macro"], Value::Null);

        let quest_items = ExQuestItemList {
            items: vec![ItemInfo::plain(0x1000, 57, 100)],
            block: None,
        };
        let fields = s2c_fields(&quest_items.to_bytes());
        assert_eq!(fields["items"][0]["item_id"], 57);
        assert_eq!(fields["block"], Value::Null);

        let rotation = ExRotation {
            char_id: 1,
            heading: 2,
        };
        assert_eq!(
            s2c_fields(&rotation.to_bytes()),
            json!({ "char_id": 1, "heading": 2 })
        );

        let contacts = ExShowContactList {
            contacts: vec!["Pal".into()],
        };
        assert_eq!(s2c_fields(&contacts.to_bytes()), json!({ "contacts": ["Pal"] }));

        let extra = ExBrExtraUserInfo {
            object_id: 3,
            abnormal_visual_effects_event: 4,
            lecture_mark: 1,
        };
        assert_eq!(
            s2c_fields(&extra.to_bytes()),
            json!({ "object_id": 3, "abnormal_visual_effects_event": 4, "lecture_mark": 1 })
        );

        assert_eq!(
            s2c_fields(&ExNevitAdventPointInfo { points: 7 }.to_bytes()),
            json!({ "points": 7 })
        );
        assert_eq!(
            s2c_fields(
                &ExNevitAdventTimeChange {
                    started: true,
                    time: 60,
                }
                .to_bytes()
            ),
            json!({ "started": true, "time": 60 })
        );
    }

    #[test]
    fn combat_and_item_client_packets_show_their_fields() {
        use ConnectionState::InGame;
        let attack = AttackRequest {
            object_id: 268_435_460,
            origin_x: 1,
            origin_y: 2,
            origin_z: 3,
            attack_id: AttackRequest::SHIFT_CLICK,
        };
        let expected = json!({
            "object_id": 268_435_460, "origin_x": 1, "origin_y": 2, "origin_z": 3, "attack_id": 1
        });
        assert_eq!(c2s_fields(InGame, &attack.to_bytes()), expected);
        // ATTACK_REQUEST (0x32) is the same struct with the other id.
        let mut second_id = attack.to_bytes();
        second_id[0] = ClientOpcode::AttackRequest.id();
        assert_eq!(c2s_fields(InGame, &second_id), expected);

        assert_eq!(
            c2s_fields(
                InGame,
                &RequestActionUse {
                    action_id: 15,
                    ctrl_pressed: true,
                    shift_pressed: false,
                }
                .to_bytes()
            ),
            json!({ "action_id": 15, "ctrl_pressed": true, "shift_pressed": false })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestRestartPoint {
                    requested_point_type: 5,
                }
                .to_bytes()
            ),
            json!({ "requested_point_type": 5 })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &UseItem {
                    object_id: 7,
                    ctrl_pressed: false,
                }
                .to_bytes()
            ),
            json!({ "object_id": 7, "ctrl_pressed": false })
        );
        assert_eq!(
            c2s_fields(InGame, &RequestUnEquipItem { paperdoll_slot: 7 }.to_bytes()),
            json!({ "paperdoll_slot": 7 })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestDropItem {
                    object_id: 9,
                    count: 3,
                    x: 1,
                    y: 2,
                    z: 3,
                }
                .to_bytes()
            ),
            json!({ "object_id": 9, "count": 3, "x": 1, "y": 2, "z": 3 })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestDestroyItem {
                    object_id: 9,
                    count: 3,
                }
                .to_bytes()
            ),
            json!({ "object_id": 9, "count": 3 })
        );
        assert_eq!(c2s_fields(InGame, &RequestItemList.to_bytes()), json!({}));
    }

    #[test]
    fn combat_and_item_server_packets_show_their_fields() {
        let attack = Attack {
            attacker_id: 1,
            attacker_x: 2,
            attacker_y: 3,
            attacker_z: 4,
            first_hit: AttackHit {
                target_id: 5,
                damage: 6,
                flags: HITFLAG_CRIT,
            },
            other_hits: vec![AttackHit {
                target_id: 5,
                damage: 0,
                flags: HITFLAG_MISS,
            }],
            target_x: 7,
            target_y: 8,
            target_z: 9,
        };
        let fields = s2c_fields(&attack.to_bytes());
        assert_eq!(fields["attacker_id"], 1);
        assert_eq!(fields["target_z"], 9);
        assert_eq!(
            fields["first_hit"],
            json!({ "target_id": 5, "damage": 6, "flags": 0x20 })
        );
        assert_eq!(
            fields["other_hits"],
            json!([{ "target_id": 5, "damage": 0, "flags": 0x80 }])
        );

        assert_eq!(
            s2c_fields(&AutoAttackStart { target_id: 8 }.to_bytes()),
            json!({ "target_id": 8 })
        );
        assert_eq!(
            s2c_fields(&AutoAttackStop { target_id: 8 }.to_bytes()),
            json!({ "target_id": 8 })
        );
        let die = Die {
            object_id: 3,
            can_teleport: true,
            to_clan_hall: false,
            to_castle: true,
            to_siege_hq: false,
            sweepable: false,
            fixed_res: true,
            to_fortress: false,
        };
        let fields = s2c_fields(&die.to_bytes());
        assert_eq!(fields["can_teleport"], true);
        assert_eq!(fields["to_castle"], true);
        assert_eq!(fields["fixed_res"], true);
        assert_eq!(fields["to_fortress"], false);
        assert_eq!(
            s2c_fields(&Revive { object_id: 4 }.to_bytes()),
            json!({ "object_id": 4 })
        );
        assert_eq!(
            s2c_fields(
                &ChangeMoveType {
                    object_id: 5,
                    running: true,
                }
                .to_bytes()
            ),
            json!({ "object_id": 5, "running": true })
        );
        assert_eq!(
            s2c_fields(
                &ChangeWaitType {
                    object_id: 6,
                    move_type: ChangeWaitType::WT_STANDING,
                    x: 1,
                    y: 2,
                    z: 3,
                }
                .to_bytes()
            ),
            json!({ "object_id": 6, "move_type": 1, "x": 1, "y": 2, "z": 3 })
        );
        assert_eq!(
            s2c_fields(&ExUseSharedGroupItem::new(1061, 32, 12_000, 60_000).to_bytes()),
            json!({ "item_id": 1061, "group_id": 32, "remaining_secs": 12, "total_secs": 60 })
        );

        let spawn = SpawnItem {
            object_id: 9,
            display_id: 57,
            x: 1,
            y: 2,
            z: 3,
            stackable: true,
            count: 5,
        };
        let fields = s2c_fields(&spawn.to_bytes());
        assert_eq!(fields["display_id"], 57);
        assert_eq!(fields["stackable"], true);
        assert_eq!(fields["count"], 5);
        let drop = DropItem {
            player_id: 10,
            object_id: 9,
            display_id: 57,
            x: 1,
            y: 2,
            z: 3,
            stackable: false,
            count: 1,
        };
        let fields = s2c_fields(&drop.to_bytes());
        assert_eq!(fields["player_id"], 10);
        assert_eq!(fields["stackable"], false);
        assert_eq!(
            s2c_fields(
                &GetItem {
                    player_id: 10,
                    object_id: 9,
                    x: 1,
                    y: 2,
                    z: 3,
                }
                .to_bytes()
            ),
            json!({ "player_id": 10, "object_id": 9, "x": 1, "y": 2, "z": 3 })
        );
    }

    #[test]
    fn login_fail_and_seven_signs_show_their_fields() {
        assert_eq!(
            s2c_fields(&LoginFail { reason: 2 }.to_bytes()),
            json!({ "reason": 2 })
        );
        assert_eq!(
            s2c_fields(&SsqInfo { state: 1 }.to_bytes()),
            json!({ "state": 1 })
        );
    }

    #[test]
    fn skill_packets_show_their_fields() {
        use ConnectionState::InGame;
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestMagicSkillUse {
                    magic_id: 1177,
                    ctrl_pressed: true,
                    shift_pressed: false,
                }
                .to_bytes()
            ),
            json!({ "magic_id": 1177, "ctrl_pressed": true, "shift_pressed": false })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestAutoSoulShot {
                    item_id: 1463,
                    shot_type: RequestAutoSoulShot::ON,
                }
                .to_bytes()
            ),
            json!({ "item_id": 1463, "shot_type": 1 })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestShortCutReg {
                    type_id: 2,
                    slot: 14,
                    id: 1204,
                    level: 7,
                    character_type: 1,
                }
                .to_bytes()
            ),
            json!({ "type_id": 2, "slot": 14, "id": 1204, "level": 7, "character_type": 1 })
        );
        assert_eq!(
            c2s_fields(InGame, &RequestShortCutDel { slot: 14 }.to_bytes()),
            json!({ "slot": 14 })
        );
        let learn = RequestAcquireSkill {
            id: 3,
            level: 2,
            skill_type: AcquireSkillType::Subpledge,
            sub_type: 1001,
        };
        assert_eq!(
            c2s_fields(InGame, &learn.to_bytes()),
            json!({ "id": 3, "level": 2, "sub_type": 1001, "skill_type": 3 })
        );
        let info = RequestAcquireSkillInfo {
            id: 4,
            level: 1,
            skill_type: AcquireSkillType::Class,
        };
        assert_eq!(
            c2s_fields(InGame, &info.to_bytes()),
            json!({ "id": 4, "level": 1, "skill_type": 0 })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestDispel {
                    object_id: 9,
                    skill_id: 1204,
                    skill_level: 2,
                }
                .to_bytes()
            ),
            json!({ "object_id": 9, "skill_id": 1204, "skill_level": 2 })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &l2_protocol_game::client::RequestVoteNew {
                    target_id: 268_435_456
                }
                .to_bytes()
            ),
            json!({ "target_id": 268_435_456 })
        );
        assert_eq!(
            c2s_fields(
                InGame,
                &l2_protocol_game::client::DlgAnswer {
                    message_id: 1510,
                    answer: 1,
                    requester_id: 0,
                }
                .to_bytes()
            ),
            json!({ "message_id": 1510, "answer": 1, "requester_id": 0 })
        );
    }

    #[test]
    fn confirm_dlg_and_fly_to_location_show_their_fields() {
        let dlg = ConfirmDlg::new(
            SystemMessageId::YourCharmOfCourageIsTryingToResurrectYouWouldYouLikeToResurrectNow,
        )
        .with_time(60_000);
        let fields = s2c_fields(&dlg.to_bytes());
        assert_eq!(fields["id"], 2306);
        assert_eq!(fields["time"], 60_000);
        assert_eq!(fields["requester_id"], 0);
        let fly = FlyToLocation {
            object_id: 1,
            dest_x: 2,
            dest_y: 3,
            dest_z: 4,
            org_x: 5,
            org_y: 6,
            org_z: 7,
            fly_type: 2,
        };
        assert_eq!(
            s2c_fields(&fly.to_bytes()),
            json!({ "object_id": 1, "dest_x": 2, "dest_y": 3, "dest_z": 4, "org_x": 5, "org_y": 6, "org_z": 7, "fly_type": 2 })
        );
    }

    #[test]
    fn skill_server_packets_show_their_fields() {
        let use_skill = MagicSkillUse {
            creature_id: 1,
            target_id: 2,
            skill_id: 1177,
            skill_level: 1,
            hit_time: 1000,
            reuse_delay: 500,
            creature: SkillPosition { x: 10, y: 20, z: 30 },
            ground_location: None,
            target: SkillPosition { x: 40, y: 50, z: 60 },
        };
        let fields = s2c_fields(&use_skill.to_bytes());
        assert_eq!(fields["skill_id"], 1177);
        assert_eq!(fields["ground_location"], Value::Null);
        assert_eq!(fields["creature"], json!({ "x": 10, "y": 20, "z": 30 }));
        assert_eq!(fields["target"]["z"], 60);

        let ground = MagicSkillUse {
            ground_location: Some(SkillPosition { x: 7, y: 8, z: 9 }),
            ..use_skill
        };
        assert_eq!(
            s2c_fields(&ground.to_bytes())["ground_location"],
            json!({ "x": 7, "y": 8, "z": 9 })
        );
        assert_eq!(
            s2c_fields(
                &MagicSkillLaunched {
                    creature_id: 1,
                    skill_id: 1177,
                    skill_level: 1,
                    target_ids: vec![2, 3],
                }
                .to_bytes()
            ),
            json!({ "creature_id": 1, "skill_id": 1177, "skill_level": 1, "target_ids": [2, 3] })
        );
        assert_eq!(
            s2c_fields(&MagicSkillCanceled { object_id: 5 }.to_bytes()),
            json!({ "object_id": 5 })
        );

        let mut buffs = AbnormalStatusUpdate::default();
        buffs.add_skill(
            EffectIcon {
                display_id: 1204,
                display_level: 2,
                time: 90,
                in_use: true,
            },
            false,
        );
        assert_eq!(
            s2c_fields(&buffs.to_bytes()),
            json!({ "effects": [{ "display_id": 1204, "display_level": 2, "time": 90, "in_use": true }] })
        );
        assert_eq!(
            s2c_fields(
                &ShortBuffStatusUpdate {
                    skill_id: 1331,
                    skill_level: 1,
                    duration: 12,
                }
                .to_bytes()
            ),
            json!({ "skill_id": 1331, "skill_level": 1, "duration": 12 })
        );
        assert_eq!(
            s2c_fields(
                &ExAutoSoulShot {
                    item_id: 6,
                    shot_type: 0
                }
                .to_bytes()
            ),
            json!({ "item_id": 6, "shot_type": 0 })
        );

        let mut list = AcquireSkillList::new(AcquireSkillType::Pledge);
        list.add_skill(370, 1, 3, 100, 0);
        assert_eq!(
            s2c_fields(&list.to_bytes()),
            json!({
                "skill_type": 2,
                "skills": [{ "id": 370, "next_level": 1, "max_level": 3, "sp_cost": 100, "requirements": 0 }]
            })
        );
        let info = AcquireSkillInfo {
            skill_type: AcquireSkillType::Class,
            skill_id: 1,
            level: 1,
            sp_cost: 0,
            requirements: vec![AcquireSkillRequirement::item(57, 2)],
        };
        let fields = s2c_fields(&info.to_bytes());
        assert_eq!(fields["skill_type"], 0);
        assert_eq!(fields["requirements"][0]["item_id"], 57);
        assert_eq!(fields["requirements"][0]["count"], 2);
        assert_eq!(fields["requirements"][0]["req_type"], 99);
        assert_eq!(s2c_fields(&AcquireSkillDone.to_bytes()), json!({}));
        let party = PartySpelled {
            creature_type: PartySpelled::PET,
            creature_id: 8,
            effects: Vec::new(),
        };
        assert_eq!(
            s2c_fields(&party.to_bytes()),
            json!({ "creature_type": 1, "creature_id": 8, "effects": [] })
        );
    }

    /// Every server packet of the committed Java captures with a decoder must have fields, and
    /// no other server packet may have any. A decode error here means a `Decode` impl does not
    /// match what Java sends.
    #[test]
    fn every_decoded_server_packet_in_the_committed_captures_has_fields() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures");
        let mut seen = BTreeSet::new();
        let mut problems = Vec::new();
        for entry in fs::read_dir(&dir).expect("the committed captures folder") {
            let path = entry.expect("a directory entry").path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let file = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let records = CaptureReader::open(&path)
                .expect("a readable capture")
                .read_all()
                .expect("well-formed records");
            for (index, record) in records.iter().enumerate() {
                if record.dir != Dir::S2c {
                    continue;
                }
                let raw = record.raw_bytes().expect("a hex payload");
                let fields = describe(Dir::S2c, ConnectionState::InGame, record.ex_opcode, &raw, false);
                let decoded = ServerOpcode::from_id(record.opcode, record.ex_opcode)
                    .filter(|op| DECODED_SERVER_PACKETS.contains(op));
                // The header is line 1, so a record's line is its index plus two.
                let place = format!("{file} line {}", index + 2);
                match &fields {
                    None if decoded.is_some() => {
                        problems.push(format!("{place}: {} has no fields", record.name));
                    }
                    Some(_) if decoded.is_none() => {
                        problems.push(format!("{place}: {} has fields without a decoder", record.name));
                    }
                    Some(v) if v.get("decode_error").is_some() => {
                        problems.push(format!("{place}: {} {}", record.name, v["decode_error"]));
                    }
                    _ => {}
                }
                if let Some(op) = decoded {
                    seen.insert(op);
                }
            }
        }
        assert!(
            problems.is_empty(),
            "{} problems:\n{}",
            problems.len(),
            problems.join("\n")
        );
        // The sweep must actually reach the packet families it is meant to cover.
        for op in [
            ServerOpcode::NpcInfo,
            ServerOpcode::MoveToLocation,
            ServerOpcode::StatusUpdate,
            ServerOpcode::StopMove,
            ServerOpcode::MoveToPawn,
            ServerOpcode::DeleteObject,
            ServerOpcode::MyTargetSelected,
            ServerOpcode::UserInfo,
            ServerOpcode::ActionFail,
            ServerOpcode::ExRotation,
            ServerOpcode::ExBrExtraUserInfo,
            ServerOpcode::ExNavitAdventTimeChange,
            ServerOpcode::Attack,
            ServerOpcode::Die,
            ServerOpcode::DropItem,
            ServerOpcode::GetItem,
        ] {
            assert!(seen.contains(&op), "the captures hold no {}", op.name());
        }
    }
}
