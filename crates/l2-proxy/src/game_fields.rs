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
use l2_protocol_game::server::duel::{
    ExDuelAskStart, ExDuelEnd, ExDuelReady, ExDuelStart, ExDuelUpdateUserInfo,
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
use l2_protocol_game::server::gm_view::GmViewItemList;
use l2_protocol_game::server::ground_item::{DropItem, GetItem, SpawnItem};
use l2_protocol_game::server::handshake::{KeyPacket, LoginFail};
use l2_protocol_game::server::interaction::{
    ActionFailed, DoorInfo, DoorStatusUpdate, MyTargetSelected, NpcHtmlMessage, StaticObjectInfo,
    TargetSelected, TargetUnselected,
};
use l2_protocol_game::server::items::{InventoryBlock, ItemInfo, ItemList};
use l2_protocol_game::server::movement::{MoveToLocation, MoveToPawn, StopMove, TeleportToLocation};
use l2_protocol_game::server::party::{
    AskJoinParty, ExPartyPetWindowAdd, ExPartyPetWindowDelete, ExPartyPetWindowUpdate, JoinParty,
    PartyMemberPosition, PartyMemberVitals, PartyPetWindow, PartySmallWindowAdd, PartySmallWindowAll,
    PartySmallWindowDelete, PartySmallWindowDeleteAll, PartySmallWindowUpdate, PartySummon,
};
use l2_protocol_game::server::pledge::{
    AllianceInfo, AllyCrest, AskJoinAlly, AskJoinPledge, ExPledgeEmblem, ExSubPledgeSkillAdd,
    GmViewPledgeInfo, JoinPledge, ManagePledgePower, PledgeCrest, PledgeInfo, PledgeMember,
    PledgePowerGradeList, PledgeReceiveMemberInfo, PledgeReceivePowerInfo, PledgeReceiveSubPledgeCreated,
    PledgeReceiveWarList, PledgeShowInfoUpdate, PledgeShowMemberListAdd, PledgeShowMemberListAll,
    PledgeShowMemberListDelete, PledgeShowMemberListDeleteAll, PledgeShowMemberListUpdate, PledgeSkillList,
    PledgeSkillListAdd, PledgeStatusChanged, StartPledgeWar, StopPledgeWar, SurrenderPledgeWar,
};
use l2_protocol_game::server::shop::{BuyList, ExBuySellList};
use l2_protocol_game::server::skills::{
    AbnormalStatusUpdate, AcquireSkillDone, AcquireSkillInfo, AcquireSkillList, EffectIcon, ExAutoSoulShot,
    MagicSkillCanceled, MagicSkillLaunched, MagicSkillUse, PartySpelled, ShortBuffStatusUpdate,
    SkillPosition,
};
use l2_protocol_game::server::sound::PlaySound;
use l2_protocol_game::server::store::{
    BuyStoreEntry, ExPrivateStoreSetWholeMsg, PrivateStoreListBuy, PrivateStoreListSell,
    PrivateStoreManageListBuy, PrivateStoreManageListSell, PrivateStoreMsgBuy, PrivateStoreMsgSell,
    SellStoreEntry, StoreCandidate,
};
use l2_protocol_game::server::summon::{
    PetDelete, PetInfo, PetInventoryUpdate, PetItemList, PetStatusShow, PetStatusUpdate, SetSummonRemainTime,
};
use l2_protocol_game::server::system_message::{ConfirmDlg, SystemMessage, SystemMessageParam};
use l2_protocol_game::server::trade::{
    SendTradeRequest, TradeDone as ServerTradeDone, TradeItemInfo, TradeOtherAdd, TradeOtherDone,
    TradeOwnAdd, TradeStart, TradeUpdate,
};
use l2_protocol_game::server::userinfo::UserInfo;
use l2_protocol_game::server::visibility::{
    CharInfo, DeleteObject, ExSetCompassZoneCode, NpcInfo, RelationChanged, StatusUpdate,
};
use l2_protocol_game::server::warehouse::{
    PackageSendableList, PackageToList, WareHouseDepositList, WareHouseWithdrawalList,
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
        ClientPacket::AnswerCoupleAction(p) => object!(p => action_id, answer, object_id),
        ClientPacket::DlgAnswer(p) => object!(p => message_id, answer, requester_id),
        ClientPacket::RequestJoinParty(p) => object!(p => name, party_distribution_type_id),
        ClientPacket::RequestAnswerJoinParty(p) => object!(p => response),
        ClientPacket::RequestWithDrawalParty(_) => json!({}),
        ClientPacket::RequestOustPartyMember(p) => object!(p => name),
        ClientPacket::RequestChangePartyLeader(p) => object!(p => name),
        ClientPacket::RequestBuyItem(p) => {
            let items = p
                .items
                .as_ref()
                .map(|v| v.iter().map(|i| object!(i => item_id, count)).collect::<Vec<_>>());
            json!({ "list_id": p.list_id, "items": items })
        }
        ClientPacket::RequestSellItem(p) => {
            let items = p.items.as_ref().map(|v| {
                v.iter()
                    .map(|i| object!(i => object_id, item_id, count))
                    .collect::<Vec<_>>()
            });
            json!({ "list_id": p.list_id, "items": items })
        }
        ClientPacket::RequestRefundItem(p) => object!(p => list_id, items),
        ClientPacket::RequestBuySellUIClose(_) => json!({}),
        ClientPacket::TradeRequest(p) => object!(p => object_id),
        ClientPacket::AnswerTradeRequest(p) => object!(p => response),
        ClientPacket::AddTradeItem(p) => object!(p => trade_id, object_id, count),
        ClientPacket::TradeDone(p) => object!(p => response),
        ClientPacket::SendWareHouseDepositList(p) => json!({ "items": warehouse_list_fields(&p.items) }),
        ClientPacket::SendWareHouseWithDrawList(p) => json!({ "items": warehouse_list_fields(&p.items) }),
        ClientPacket::RequestPackageSend(p) => {
            json!({ "object_id": p.object_id, "items": warehouse_list_fields(&p.items) })
        }
        ClientPacket::RequestPackageSendableItemList(p) => object!(p => object_id),
        ClientPacket::RequestPrivateStoreManageSell(_)
        | ClientPacket::RequestPrivateStoreManageBuy(_)
        | ClientPacket::RequestPrivateStoreQuitSell(_)
        | ClientPacket::RequestPrivateStoreQuitBuy(_) => json!({}),
        ClientPacket::SetPrivateStoreMsgSell(p) => object!(p => store_msg),
        ClientPacket::SetPrivateStoreMsgBuy(p) => object!(p => store_msg),
        ClientPacket::SetPrivateStoreWholeMsg(p) => object!(p => msg),
        ClientPacket::SetPrivateStoreListSell(p) => {
            let items = p.items.as_ref().map(|v| {
                v.iter()
                    .map(|i| object!(i => item_id, count, price))
                    .collect::<Vec<_>>()
            });
            json!({ "package_sale": p.package_sale, "items": items })
        }
        ClientPacket::SetPrivateStoreListBuy(p) => {
            let items = p.items.as_ref().map(|v| {
                v.iter()
                    .map(|i| object!(i => item_id, count, price))
                    .collect::<Vec<_>>()
            });
            json!({ "items": items })
        }
        ClientPacket::RequestPrivateStoreBuy(p) => {
            let items = p.items.as_ref().map(|v| {
                v.iter()
                    .map(|i| object!(i => object_id, count, price))
                    .collect::<Vec<_>>()
            });
            json!({ "store_player_id": p.store_player_id, "items": items })
        }
        ClientPacket::RequestPrivateStoreSell(p) => {
            let items = p.items.as_ref().map(|v| {
                v.iter()
                    .map(|i| object!(i => object_id, item_id, count, price))
                    .collect::<Vec<_>>()
            });
            json!({ "store_player_id": p.store_player_id, "items": items })
        }
        ClientPacket::RequestJoinPledge(p) => object!(p => target, pledge_type),
        ClientPacket::RequestAnswerJoinPledge(p) => object!(p => answer),
        ClientPacket::RequestOustPledgeMember(p) => object!(p => target),
        ClientPacket::RequestPledgeInfo(p) => object!(p => clan_id),
        ClientPacket::RequestPledgeExtendedInfo(p) => object!(p => name),
        ClientPacket::RequestPledgeCrest(p) => object!(p => crest_id),
        ClientPacket::RequestExPledgeCrestLarge(p) => object!(p => crest_id),
        ClientPacket::RequestPledgeMemberPowerInfo(p) => object!(p => unk1, player),
        ClientPacket::RequestPledgeMemberInfo(p) => object!(p => unk1, player),
        ClientPacket::RequestPledgeWarList(p) => object!(p => unk1, tab),
        ClientPacket::RequestPledgeSetAcademyMaster(p) => {
            object!(p => set, curr_player_name, target_player_name)
        }
        ClientPacket::RequestPledgeSetMemberPowerGrade(p) => object!(p => member, power_grade),
        ClientPacket::RequestPledgeReorganizeMember(p) => {
            object!(p => is_member_selected, member_name, new_pledge_type, selected_member)
        }
        ClientPacket::RequestPledgePower(p) => object!(p => rank, action, privs),
        ClientPacket::RequestSetPledgeCrest(p) => {
            json!({ "length": p.length, "data": p.data.as_ref().map(hex::encode) })
        }
        ClientPacket::RequestExSetPledgeCrestLarge(p) => {
            json!({ "length": p.length, "data": p.data.as_ref().map(hex::encode) })
        }
        ClientPacket::RequestSetAllyCrest(p) => {
            json!({ "length": p.length, "data": p.data.as_ref().map(hex::encode) })
        }
        ClientPacket::RequestStartPledgeWar(p) => object!(p => pledge_name),
        ClientPacket::RequestReplyStartPledgeWar(p) => object!(p => req_name, answer),
        ClientPacket::RequestStopPledgeWar(p) => object!(p => pledge_name),
        ClientPacket::RequestReplyStopPledgeWar(p) => object!(p => req_name, answer),
        ClientPacket::RequestSurrenderPledgeWar(p) => object!(p => pledge_name),
        ClientPacket::RequestReplySurrenderPledgeWar(p) => object!(p => req_name, answer),
        ClientPacket::RequestJoinAlly(p) => object!(p => id),
        ClientPacket::RequestAnswerJoinAlly(p) => object!(p => response),
        ClientPacket::AllyDismiss(p) => object!(p => clan_name),
        ClientPacket::RequestAllyCrest(p) => object!(p => crest_id),
        ClientPacket::RequestWithdrawalPledge(_)
        | ClientPacket::RequestPledgeMemberList(_)
        | ClientPacket::RequestPledgePowerGradeList(_)
        | ClientPacket::AllyLeave(_)
        | ClientPacket::RequestDismissAlly(_)
        | ClientPacket::RequestAllyInfo(_) => json!({}),
        ClientPacket::RequestPetGetItem(p) => object!(p => object_id),
        ClientPacket::RequestGetItemFromPet(p) => object!(p => object_id, amount, unknown),
        ClientPacket::RequestGiveItemToPet(p) => object!(p => object_id, amount),
        ClientPacket::RequestPetUseItem(p) => object!(p => object_id),
        ClientPacket::RequestChangePetName(p) => object!(p => name),
        ClientPacket::RequestDuelStart(p) => object!(p => player, party_duel),
        ClientPacket::RequestDuelAnswerStart(p) => object!(p => party_duel, unk1, response),
        ClientPacket::RequestDuelSurrender(_) => json!({}),
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
        ServerOpcode::GmViewItemList => decoded::<GmViewItemList>(&mut r, gm_view_item_list_fields),
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
        ServerOpcode::AskJoinParty => decoded::<AskJoinParty>(&mut r, ask_join_party_fields),
        ServerOpcode::JoinParty => decoded::<JoinParty>(&mut r, join_party_fields),
        ServerOpcode::PartySmallWindowAll => {
            decoded::<PartySmallWindowAll>(&mut r, party_small_window_all_fields)
        }
        ServerOpcode::PartySmallWindowAdd => {
            decoded::<PartySmallWindowAdd>(&mut r, party_small_window_add_fields)
        }
        ServerOpcode::PartySmallWindowDeleteAll => decoded::<PartySmallWindowDeleteAll>(&mut r, no_fields),
        ServerOpcode::PartySmallWindowDelete => {
            decoded::<PartySmallWindowDelete>(&mut r, party_small_window_delete_fields)
        }
        ServerOpcode::PartySmallWindowUpdate => {
            decoded::<PartySmallWindowUpdate>(&mut r, party_small_window_update_fields)
        }
        ServerOpcode::PartyMemberPosition => {
            decoded::<PartyMemberPosition>(&mut r, party_member_position_fields)
        }
        ServerOpcode::ExPartyPetWindowAdd => decoded::<ExPartyPetWindowAdd>(&mut r, pet_window_add_fields),
        ServerOpcode::ExPartyPetWindowUpdate => {
            decoded::<ExPartyPetWindowUpdate>(&mut r, pet_window_update_fields)
        }
        ServerOpcode::ExPartyPetWindowDelete => {
            decoded::<ExPartyPetWindowDelete>(&mut r, pet_window_delete_fields)
        }
        // One id for two packets: the first int is 0 for the merchant's list, 1 for the sell side.
        ServerOpcode::ExBuySellList => {
            if raw.get(start..start + 4) == Some(&[0, 0, 0, 0]) {
                decoded::<BuyList>(&mut r, buy_list_fields)
            } else {
                decoded::<ExBuySellList>(&mut r, ex_buy_sell_list_fields)
            }
        }
        ServerOpcode::TradeRequest => decoded::<SendTradeRequest>(&mut r, send_trade_request_fields),
        ServerOpcode::TradeStart => decoded::<TradeStart>(&mut r, trade_start_fields),
        ServerOpcode::TradeOwnAdd => decoded::<TradeOwnAdd>(&mut r, trade_own_add_fields),
        ServerOpcode::TradeOtherAdd => decoded::<TradeOtherAdd>(&mut r, trade_other_add_fields),
        ServerOpcode::TradeUpdate => decoded::<TradeUpdate>(&mut r, trade_update_fields),
        ServerOpcode::TradePressOtherOk => decoded::<TradeOtherDone>(&mut r, no_fields),
        ServerOpcode::TradeDone => decoded::<ServerTradeDone>(&mut r, server_trade_done_fields),
        ServerOpcode::PrivateStoreSellManageList => {
            decoded::<PrivateStoreManageListSell>(&mut r, store_manage_sell_fields)
        }
        ServerOpcode::PrivateStoreBuyManageList => {
            decoded::<PrivateStoreManageListBuy>(&mut r, store_manage_buy_fields)
        }
        ServerOpcode::PrivateStoreSellList => decoded::<PrivateStoreListSell>(&mut r, store_list_sell_fields),
        ServerOpcode::PrivateStoreBuyList => decoded::<PrivateStoreListBuy>(&mut r, store_list_buy_fields),
        ServerOpcode::PrivateStoreSellMsg => decoded::<PrivateStoreMsgSell>(&mut r, store_msg_sell_fields),
        ServerOpcode::PrivateStoreBuyMsg => decoded::<PrivateStoreMsgBuy>(&mut r, store_msg_buy_fields),
        ServerOpcode::ExPrivateStorePackageMsg => {
            decoded::<ExPrivateStoreSetWholeMsg>(&mut r, store_whole_msg_fields)
        }
        ServerOpcode::AskJoinPledge => decoded::<AskJoinPledge>(&mut r, ask_join_pledge_fields),
        ServerOpcode::JoinPledge => decoded::<JoinPledge>(&mut r, join_pledge_fields),
        ServerOpcode::PledgeInfo => decoded::<PledgeInfo>(&mut r, pledge_info_fields),
        ServerOpcode::PledgeCrest => decoded::<PledgeCrest>(&mut r, pledge_crest_fields),
        ServerOpcode::ExPledgeEmblem => decoded::<ExPledgeEmblem>(&mut r, ex_pledge_emblem_fields),
        ServerOpcode::AllianceCrest => decoded::<AllyCrest>(&mut r, ally_crest_fields),
        ServerOpcode::PledgeShowMemberListAll => {
            decoded::<PledgeShowMemberListAll>(&mut r, pledge_show_member_list_all_fields)
        }
        ServerOpcode::PledgeShowMemberListUpdate => {
            decoded::<PledgeShowMemberListUpdate>(&mut r, pledge_show_member_list_update_fields)
        }
        ServerOpcode::PledgeShowMemberListAdd => {
            decoded::<PledgeShowMemberListAdd>(&mut r, pledge_show_member_list_add_fields)
        }
        ServerOpcode::PledgeShowMemberListDelete => {
            decoded::<PledgeShowMemberListDelete>(&mut r, pledge_show_member_list_delete_fields)
        }
        ServerOpcode::PledgeShowMemberListDeleteAll => {
            decoded::<PledgeShowMemberListDeleteAll>(&mut r, no_fields)
        }
        ServerOpcode::PledgeReceiveMemberInfo => {
            decoded::<PledgeReceiveMemberInfo>(&mut r, pledge_receive_member_info_fields)
        }
        ServerOpcode::PledgeReceivePowerInfo => {
            decoded::<PledgeReceivePowerInfo>(&mut r, pledge_receive_power_info_fields)
        }
        ServerOpcode::PledgeReceiveSubPledgeCreated => {
            decoded::<PledgeReceiveSubPledgeCreated>(&mut r, pledge_receive_sub_pledge_created_fields)
        }
        ServerOpcode::PledgeShowInfoUpdate => {
            decoded::<PledgeShowInfoUpdate>(&mut r, pledge_show_info_update_fields)
        }
        ServerOpcode::PledgeStatusChanged => {
            decoded::<PledgeStatusChanged>(&mut r, pledge_status_changed_fields)
        }
        ServerOpcode::GmViewPledgeInfo => decoded::<GmViewPledgeInfo>(&mut r, gm_view_pledge_info_fields),
        ServerOpcode::PledgePowerGradeList => {
            decoded::<PledgePowerGradeList>(&mut r, pledge_power_grade_list_fields)
        }
        ServerOpcode::ManagePledgePower => decoded::<ManagePledgePower>(&mut r, manage_pledge_power_fields),
        ServerOpcode::PledgeSkillList => decoded::<PledgeSkillList>(&mut r, pledge_skill_list_fields),
        ServerOpcode::PledgeSkillListAdd => {
            decoded::<PledgeSkillListAdd>(&mut r, pledge_skill_list_add_fields)
        }
        ServerOpcode::ExSubPledgeSkillAdd => {
            decoded::<ExSubPledgeSkillAdd>(&mut r, ex_sub_pledge_skill_add_fields)
        }
        ServerOpcode::PledgeReceiveWarList => {
            decoded::<PledgeReceiveWarList>(&mut r, pledge_receive_war_list_fields)
        }
        ServerOpcode::StartPledgeWar => decoded::<StartPledgeWar>(&mut r, start_pledge_war_fields),
        ServerOpcode::StopPledgeWar => decoded::<StopPledgeWar>(&mut r, stop_pledge_war_fields),
        ServerOpcode::SurrenderPledgeWar => {
            decoded::<SurrenderPledgeWar>(&mut r, surrender_pledge_war_fields)
        }
        ServerOpcode::AskJoinAlliance => decoded::<AskJoinAlly>(&mut r, ask_join_ally_fields),
        ServerOpcode::AllianceInfo => decoded::<AllianceInfo>(&mut r, alliance_info_fields),
        ServerOpcode::PetInfo => decoded::<PetInfo>(&mut r, pet_info_fields),
        ServerOpcode::PetStatusShow => decoded::<PetStatusShow>(&mut r, pet_status_show_fields),
        ServerOpcode::PetStatusUpdate => decoded::<PetStatusUpdate>(&mut r, pet_status_update_fields),
        ServerOpcode::PlaySound => decoded::<PlaySound>(&mut r, play_sound_fields),
        ServerOpcode::ExDuelAskStart => decoded::<ExDuelAskStart>(&mut r, duel_ask_start_fields),
        ServerOpcode::ExDuelReady => decoded::<ExDuelReady>(&mut r, duel_ready_fields),
        ServerOpcode::ExDuelStart => decoded::<ExDuelStart>(&mut r, duel_start_fields),
        ServerOpcode::ExDuelEnd => decoded::<ExDuelEnd>(&mut r, duel_end_fields),
        ServerOpcode::ExDuelUpdateUserInfo => {
            decoded::<ExDuelUpdateUserInfo>(&mut r, duel_update_user_info_fields)
        }
        ServerOpcode::PetDelete => decoded::<PetDelete>(&mut r, pet_delete_fields),
        ServerOpcode::SetSummonRemainTime => {
            decoded::<SetSummonRemainTime>(&mut r, set_summon_remain_time_fields)
        }
        ServerOpcode::PetItemList => decoded::<PetItemList>(&mut r, pet_item_list_fields),
        ServerOpcode::PetInventoryUpdate => {
            decoded::<PetInventoryUpdate>(&mut r, pet_inventory_update_fields)
        }
        ServerOpcode::WarehouseDepositList => {
            decoded::<WareHouseDepositList>(&mut r, warehouse_deposit_list_fields)
        }
        // The sorted list (`SortedWareHouseWithdrawalList`) has the same opcode and the same item
        // layout (its "equipped" short is a constant 0): one decoder reads both.
        ServerOpcode::WarehouseWithdrawList => {
            decoded::<WareHouseWithdrawalList>(&mut r, warehouse_withdrawal_list_fields)
        }
        ServerOpcode::PackageToList => decoded::<PackageToList>(&mut r, package_to_list_fields),
        ServerOpcode::PackageSendableList => {
            decoded::<PackageSendableList>(&mut r, package_sendable_list_fields)
        }
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

fn gm_view_item_list_fields(p: &GmViewItemList) -> Value {
    let items: Vec<Value> = p.items.iter().map(item_fields).collect();
    json!({ "player_name": p.player_name, "limit": p.limit, "items": items })
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

fn ask_join_party_fields(p: &AskJoinParty) -> Value {
    let mut fields = object!(p => requestor_name);
    fields["distribution_type"] = json!(p.distribution_type.id());
    fields
}

fn join_party_fields(p: &JoinParty) -> Value {
    object!(p => response)
}

fn party_vitals_fields(v: &PartyMemberVitals) -> Value {
    object!(v => object_id, name, cp, max_cp, hp, max_hp, mp, max_mp, level, class_id)
}

fn party_summon_fields(s: &PartySummon) -> Value {
    object!(s => object_id, display_id, summon_type, name, hp, max_hp, mp, max_mp, level)
}

fn party_small_window_all_fields(p: &PartySmallWindowAll) -> Value {
    let members: Vec<Value> = p
        .members
        .iter()
        .map(|m| {
            let mut fields = party_vitals_fields(&m.vitals);
            fields["race"] = json!(m.race);
            fields["summon"] = m.summon.as_ref().map_or(Value::Null, party_summon_fields);
            fields
        })
        .collect();
    let mut fields = object!(p => leader_object_id);
    fields["distribution_type"] = json!(p.distribution_type.id());
    fields["members"] = json!(members);
    fields
}

fn party_small_window_add_fields(p: &PartySmallWindowAdd) -> Value {
    let mut fields = object!(p => leader_object_id);
    fields["distribution_type"] = json!(p.distribution_type.id());
    fields["member"] = party_vitals_fields(&p.member);
    fields
}

fn party_small_window_delete_fields(p: &PartySmallWindowDelete) -> Value {
    object!(p => object_id, name)
}

fn party_small_window_update_fields(p: &PartySmallWindowUpdate) -> Value {
    party_vitals_fields(&p.member)
}

fn party_member_position_fields(p: &PartyMemberPosition) -> Value {
    let locations: Vec<Value> = p
        .locations
        .iter()
        .map(|l| object!(l => object_id, x, y, z))
        .collect();
    json!({ "locations": locations })
}

fn pet_window_fields(w: &PartyPetWindow) -> Value {
    let mut fields = party_summon_fields(&w.summon);
    fields["owner_object_id"] = json!(w.owner_object_id);
    fields
}

fn pet_window_add_fields(p: &ExPartyPetWindowAdd) -> Value {
    pet_window_fields(&p.window)
}

fn pet_window_update_fields(p: &ExPartyPetWindowUpdate) -> Value {
    pet_window_fields(&p.window)
}

fn pet_window_delete_fields(p: &ExPartyPetWindowDelete) -> Value {
    object!(p => summon_object_id, owner_object_id, summon_name)
}

fn send_trade_request_fields(p: &SendTradeRequest) -> Value {
    object!(p => sender_id)
}

fn trade_item_fields(item: &TradeItemInfo) -> Value {
    object!(
        item => type1, object_id, item_id, count, type2, custom_type1, body_part, enchant, custom_type2,
        attack_element_type, attack_element_power, element_defense, enchant_options
    )
}

fn buy_list_fields(p: &BuyList) -> Value {
    let products: Vec<Value> = p
        .products
        .iter()
        .map(|i| object!(i => item_id, count, type2, type1, body_part, enchant, price))
        .collect();
    json!({ "money": p.money, "list_id": p.list_id, "size": p.size, "products": products })
}

fn ex_buy_sell_list_fields(p: &ExBuySellList) -> Value {
    let sell: Vec<Value> = p
        .sell_list
        .iter()
        .map(|e| json!({ "item": item_fields(&e.item), "price": e.price }))
        .collect();
    let refund: Vec<Value> = p
        .refund_list
        .iter()
        .map(|e| json!({ "item": item_fields(&e.item), "index": e.index, "price": e.price }))
        .collect();
    json!({ "sell_list": sell, "refund_list": refund, "done": p.done })
}

fn trade_start_fields(p: &TradeStart) -> Value {
    let items: Vec<Value> = p.items.iter().map(item_fields).collect();
    json!({ "partner_object_id": p.partner_object_id, "items": items })
}

fn trade_own_add_fields(p: &TradeOwnAdd) -> Value {
    json!({ "item": trade_item_fields(&p.item) })
}

fn trade_other_add_fields(p: &TradeOtherAdd) -> Value {
    json!({ "item": trade_item_fields(&p.item) })
}

fn trade_update_fields(p: &TradeUpdate) -> Value {
    json!({ "change": p.change, "item": trade_item_fields(&p.item) })
}

fn server_trade_done_fields(p: &ServerTradeDone) -> Value {
    object!(p => num)
}

fn store_candidate_fields(c: &StoreCandidate) -> Value {
    json!({ "item": item_fields(&c.item), "reference_price": c.reference_price })
}

fn sell_store_entry_fields(e: &SellStoreEntry) -> Value {
    json!({ "item": item_fields(&e.item), "price": e.price, "reference_price": e.reference_price })
}

fn buy_store_entry_fields(e: &BuyStoreEntry) -> Value {
    json!({
        "item": item_fields(&e.item),
        "price": e.price,
        "reference_price": e.reference_price,
        "count": e.count,
    })
}

fn store_manage_sell_fields(p: &PrivateStoreManageListSell) -> Value {
    json!({
        "object_id": p.object_id,
        "package_sale": p.package_sale,
        "player_adena": p.player_adena,
        "item_list": p.item_list.iter().map(store_candidate_fields).collect::<Vec<_>>(),
        "sell_list": p.sell_list.iter().map(sell_store_entry_fields).collect::<Vec<_>>(),
    })
}

fn store_manage_buy_fields(p: &PrivateStoreManageListBuy) -> Value {
    json!({
        "object_id": p.object_id,
        "player_adena": p.player_adena,
        "item_list": p.item_list.iter().map(store_candidate_fields).collect::<Vec<_>>(),
        "buy_list": p.buy_list.iter().map(buy_store_entry_fields).collect::<Vec<_>>(),
    })
}

fn store_list_sell_fields(p: &PrivateStoreListSell) -> Value {
    json!({
        "seller_object_id": p.seller_object_id,
        "package_sale": p.package_sale,
        "player_adena": p.player_adena,
        "items": p.items.iter().map(sell_store_entry_fields).collect::<Vec<_>>(),
    })
}

fn store_list_buy_fields(p: &PrivateStoreListBuy) -> Value {
    json!({
        "store_object_id": p.store_object_id,
        "player_adena": p.player_adena,
        "items": p.items.iter().map(buy_store_entry_fields).collect::<Vec<_>>(),
    })
}

fn store_msg_sell_fields(p: &PrivateStoreMsgSell) -> Value {
    object!(p => object_id, message)
}

fn store_msg_buy_fields(p: &PrivateStoreMsgBuy) -> Value {
    object!(p => object_id, message)
}

fn store_whole_msg_fields(p: &ExPrivateStoreSetWholeMsg) -> Value {
    object!(p => object_id, message)
}

fn join_pledge_fields(p: &JoinPledge) -> Value {
    object!(p => pledge_id)
}

fn pledge_info_fields(p: &PledgeInfo) -> Value {
    object!(p => clan_id, clan_name, ally_name)
}

fn pledge_crest_fields(p: &PledgeCrest) -> Value {
    json!({ "crest_id": p.crest_id, "data": hex::encode(&p.data) })
}

fn ex_pledge_emblem_fields(p: &ExPledgeEmblem) -> Value {
    json!({ "crest_id": p.crest_id, "data": hex::encode(&p.data) })
}

fn ally_crest_fields(p: &AllyCrest) -> Value {
    json!({ "crest_id": p.crest_id, "data": hex::encode(&p.data) })
}

fn pledge_show_member_list_add_fields(p: &PledgeShowMemberListAdd) -> Value {
    object!(p => name, level, class_id, online_object_id, pledge_type)
}

fn pledge_show_member_list_delete_fields(p: &PledgeShowMemberListDelete) -> Value {
    object!(p => player_name)
}

fn pledge_receive_member_info_fields(p: &PledgeReceiveMemberInfo) -> Value {
    object!(p => pledge_type, name, title, power_grade, pledge_name, apprentice_or_sponsor_name)
}

fn pledge_receive_power_info_fields(p: &PledgeReceivePowerInfo) -> Value {
    object!(p => power_grade, name, privileges)
}

fn pledge_receive_sub_pledge_created_fields(p: &PledgeReceiveSubPledgeCreated) -> Value {
    object!(p => id, name, leader_name)
}

fn pledge_show_info_update_fields(p: &PledgeShowInfoUpdate) -> Value {
    object!(p => clan_id, crest_id, level, castle_id, hideout_id, fort_id, rank, reputation_score, ally_id, ally_name, ally_crest_id, at_war)
}

fn pledge_status_changed_fields(p: &PledgeStatusChanged) -> Value {
    object!(p => leader_id, clan_id, crest_id, ally_id, ally_crest_id)
}

fn manage_pledge_power_fields(p: &ManagePledgePower) -> Value {
    object!(p => privileges)
}

fn pledge_skill_list_add_fields(p: &PledgeSkillListAdd) -> Value {
    object!(p => id, level)
}

fn ex_sub_pledge_skill_add_fields(p: &ExSubPledgeSkillAdd) -> Value {
    object!(p => sub_type, skill_id, skill_level)
}

fn pledge_receive_war_list_fields(p: &PledgeReceiveWarList) -> Value {
    object!(p => tab, list_size, clan_names)
}

fn start_pledge_war_fields(p: &StartPledgeWar) -> Value {
    object!(p => player_name, pledge_name)
}

fn stop_pledge_war_fields(p: &StopPledgeWar) -> Value {
    object!(p => pledge_name, player_name)
}

fn surrender_pledge_war_fields(p: &SurrenderPledgeWar) -> Value {
    object!(p => pledge_name, player_name)
}

fn ask_join_ally_fields(p: &AskJoinAlly) -> Value {
    object!(p => requestor_object_id, requestor_name)
}

fn ask_join_pledge_fields(p: &AskJoinPledge) -> Value {
    object!(p => requestor_object_id, sub_pledge_name, pledge_type, pledge_name)
}

fn pledge_member_fields(m: &PledgeMember) -> Value {
    object!(m => name, level, class_id, sex, race, online_object_id, has_sponsor)
}

fn pledge_show_member_list_all_fields(p: &PledgeShowMemberListAll) -> Value {
    let members: Vec<Value> = p.members.iter().map(pledge_member_fields).collect();
    let mut fields = object!(
        p => main_or_subpledge, clan_id, pledge_type, clan_name, leader_name, crest_id, level, castle_id,
        hideout_id, fort_id, rank, reputation_score, ally_id, ally_name, ally_crest_id, at_war
    );
    fields["members"] = json!(members);
    fields
}

fn pledge_show_member_list_update_fields(p: &PledgeShowMemberListUpdate) -> Value {
    let online = p
        .online
        .map(|o| json!({ "object_id": o.object_id, "pledge_type": o.pledge_type }));
    let mut fields = object!(p => name, level, class_id, female, race, has_sponsor);
    fields["online"] = json!(online);
    fields
}

fn gm_view_pledge_info_fields(p: &GmViewPledgeInfo) -> Value {
    let members: Vec<Value> = p.members.iter().map(pledge_member_fields).collect();
    let mut fields = object!(
        p => player_name, clan_id, clan_name, leader_name, crest_id, level, castle_id, hideout_id, fort_id,
        rank, reputation_score, ally_id, ally_name, ally_crest_id, at_war
    );
    fields["members"] = json!(members);
    fields
}

fn pledge_power_grade_list_fields(p: &PledgePowerGradeList) -> Value {
    let privs: Vec<Value> = p.privs.iter().map(|g| object!(g => rank, privileges)).collect();
    json!({ "privs": privs })
}

fn pledge_skill_list_fields(p: &PledgeSkillList) -> Value {
    let skills: Vec<Value> = p
        .skills
        .iter()
        .map(|s| object!(s => display_id, display_level))
        .collect();
    let sub_skills: Vec<Value> = p
        .sub_skills
        .iter()
        .map(|s| object!(s => sub_type, skill_id, skill_level))
        .collect();
    json!({ "skills": skills, "sub_skills": sub_skills })
}

fn alliance_info_fields(p: &AllianceInfo) -> Value {
    let allies: Vec<Value> = p
        .allies
        .iter()
        .map(|a| object!(a => clan_name, level, leader_name, total, online))
        .collect();
    let mut fields = object!(p => name, total, online, leader_clan, leader_player);
    fields["allies"] = json!(allies);
    fields
}

fn pet_info_fields(p: &PetInfo) -> Value {
    object!(
        p => summon_type, object_id, display_id, x, y, z, heading, m_atk_spd, p_atk_spd, run_speed, walk_speed,
        swim_run_speed, swim_walk_speed, fly_run_speed, fly_walk_speed, move_multiplier,
        attack_speed_multiplier, collision_radius, collision_height, weapon, armor, has_owner, running,
        in_combat, alike_dead, spawn_state, name, title, pvp_flag, karma, cur_fed, max_fed, cur_hp, max_hp,
        cur_mp, max_mp, sp, level, exp, exp_for_this_level, exp_for_next_level, weight, max_load, p_atk,
        p_def, m_atk, m_def, accuracy, evasion, critical, move_speed, atk_speed, casting_speed,
        abnormal_visual_effects, mountable, water_or_flying, team_id, soul_shots_per_hit,
        spirit_shots_per_hit, form_id, abnormal_visual_effect_special
    )
}

fn pet_status_show_fields(p: &PetStatusShow) -> Value {
    object!(p => summon_type)
}

fn play_sound_fields(p: &PlaySound) -> Value {
    object!(p => unknown1, sound_file, unknown3, unknown4, unknown5, unknown6, unknown7, unknown8)
}

fn duel_ask_start_fields(p: &ExDuelAskStart) -> Value {
    object!(p => requestor_name, party_duel)
}

fn duel_ready_fields(p: &ExDuelReady) -> Value {
    object!(p => party_duel)
}

fn duel_start_fields(p: &ExDuelStart) -> Value {
    object!(p => party_duel)
}

fn duel_end_fields(p: &ExDuelEnd) -> Value {
    object!(p => party_duel)
}

fn duel_update_user_info_fields(p: &ExDuelUpdateUserInfo) -> Value {
    object!(p => name, object_id, class_id, level, current_hp, max_hp, current_mp, max_mp, current_cp, max_cp)
}

fn pet_status_update_fields(p: &PetStatusUpdate) -> Value {
    object!(
        p => summon_type, object_id, x, y, z, title, cur_fed, max_fed, cur_hp, max_hp, cur_mp, max_mp, level,
        exp, exp_for_this_level, exp_for_next_level
    )
}

fn pet_delete_fields(p: &PetDelete) -> Value {
    object!(p => pet_type, pet_object_id)
}

fn set_summon_remain_time_fields(p: &SetSummonRemainTime) -> Value {
    object!(p => max_time, remaining_time)
}

fn warehouse_list_fields(items: &Option<Vec<l2_protocol_game::client::WarehouseListEntry>>) -> Value {
    // `None` is a list Java drops.
    items.as_ref().map_or(Value::Null, |v| {
        Value::Array(v.iter().map(|i| object!(i => object_id, count)).collect())
    })
}

fn warehouse_deposit_list_fields(p: &WareHouseDepositList) -> Value {
    json!({
        "wh_type": p.wh_type,
        "player_adena": p.player_adena,
        "items": p.items.iter().map(item_fields).collect::<Vec<_>>(),
    })
}

fn warehouse_withdrawal_list_fields(p: &WareHouseWithdrawalList) -> Value {
    json!({
        "wh_type": p.wh_type,
        "player_adena": p.player_adena,
        "items": p.items.iter().map(item_fields).collect::<Vec<_>>(),
    })
}

fn package_to_list_fields(p: &PackageToList) -> Value {
    json!({
        "players": p
            .players
            .iter()
            .map(|(id, name)| json!({ "object_id": id, "name": name }))
            .collect::<Vec<_>>(),
    })
}

fn package_sendable_list_fields(p: &PackageSendableList) -> Value {
    json!({
        "player_object_id": p.player_object_id,
        "adena": p.adena,
        "items": p.items.iter().map(item_fields).collect::<Vec<_>>(),
    })
}

fn pet_item_list_fields(p: &PetItemList) -> Value {
    json!({ "items": p.items.iter().map(item_fields).collect::<Vec<_>>() })
}

fn pet_inventory_update_fields(p: &PetInventoryUpdate) -> Value {
    let updates: Vec<Value> = p
        .updates
        .iter()
        .map(|u| json!({ "change": u.change, "item": item_fields(&u.item) }))
        .collect();
    json!({ "updates": updates })
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
        ServerOpcode::GmViewItemList,
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
        ServerOpcode::AskJoinParty,
        ServerOpcode::JoinParty,
        ServerOpcode::PartySmallWindowAll,
        ServerOpcode::PartySmallWindowAdd,
        ServerOpcode::PartySmallWindowDeleteAll,
        ServerOpcode::PartySmallWindowDelete,
        ServerOpcode::PartySmallWindowUpdate,
        ServerOpcode::PartyMemberPosition,
        ServerOpcode::ExPartyPetWindowAdd,
        ServerOpcode::ExPartyPetWindowUpdate,
        ServerOpcode::ExPartyPetWindowDelete,
        ServerOpcode::ExBuySellList,
        ServerOpcode::TradeRequest,
        ServerOpcode::TradeStart,
        ServerOpcode::TradeOwnAdd,
        ServerOpcode::TradeOtherAdd,
        ServerOpcode::TradeUpdate,
        ServerOpcode::TradePressOtherOk,
        ServerOpcode::TradeDone,
        ServerOpcode::PrivateStoreSellManageList,
        ServerOpcode::PrivateStoreBuyManageList,
        ServerOpcode::PrivateStoreSellList,
        ServerOpcode::PrivateStoreBuyList,
        ServerOpcode::PrivateStoreSellMsg,
        ServerOpcode::PrivateStoreBuyMsg,
        ServerOpcode::ExPrivateStorePackageMsg,
        ServerOpcode::AskJoinPledge,
        ServerOpcode::JoinPledge,
        ServerOpcode::PledgeInfo,
        ServerOpcode::PledgeCrest,
        ServerOpcode::ExPledgeEmblem,
        ServerOpcode::AllianceCrest,
        ServerOpcode::PledgeShowMemberListAll,
        ServerOpcode::PledgeShowMemberListUpdate,
        ServerOpcode::PledgeShowMemberListAdd,
        ServerOpcode::PledgeShowMemberListDelete,
        ServerOpcode::PledgeShowMemberListDeleteAll,
        ServerOpcode::PledgeReceiveMemberInfo,
        ServerOpcode::PledgeReceivePowerInfo,
        ServerOpcode::PledgeReceiveSubPledgeCreated,
        ServerOpcode::PledgeShowInfoUpdate,
        ServerOpcode::PledgeStatusChanged,
        ServerOpcode::GmViewPledgeInfo,
        ServerOpcode::PledgePowerGradeList,
        ServerOpcode::ManagePledgePower,
        ServerOpcode::PledgeSkillList,
        ServerOpcode::PledgeSkillListAdd,
        ServerOpcode::ExSubPledgeSkillAdd,
        ServerOpcode::PledgeReceiveWarList,
        ServerOpcode::StartPledgeWar,
        ServerOpcode::StopPledgeWar,
        ServerOpcode::SurrenderPledgeWar,
        ServerOpcode::AskJoinAlliance,
        ServerOpcode::AllianceInfo,
        ServerOpcode::PetInfo,
        ServerOpcode::PetStatusShow,
        ServerOpcode::PetStatusUpdate,
        ServerOpcode::PlaySound,
        ServerOpcode::ExDuelAskStart,
        ServerOpcode::ExDuelReady,
        ServerOpcode::ExDuelStart,
        ServerOpcode::ExDuelEnd,
        ServerOpcode::ExDuelUpdateUserInfo,
        ServerOpcode::PetDelete,
        ServerOpcode::SetSummonRemainTime,
        ServerOpcode::PetItemList,
        ServerOpcode::PetInventoryUpdate,
        ServerOpcode::WarehouseDepositList,
        ServerOpcode::WarehouseWithdrawList,
        ServerOpcode::PackageToList,
        ServerOpcode::PackageSendableList,
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
    fn gm_view_item_list_shows_its_fields() {
        let list = GmViewItemList {
            player_name: "Target".into(),
            limit: 80,
            items: vec![ItemInfo::plain(0x1000, 57, 100)],
        };
        let fields = s2c_fields(&list.to_bytes());
        assert_eq!(fields["player_name"], "Target");
        assert_eq!(fields["limit"], 80);
        assert_eq!(fields["items"][0]["item_id"], 57);
        assert_eq!(fields["items"][0]["count"], 100);
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

    #[test]
    fn party_packets_show_their_fields() {
        use l2_protocol_game::client::{
            RequestAnswerJoinParty, RequestChangePartyLeader, RequestJoinParty, RequestOustPartyMember,
            RequestWithDrawalParty,
        };
        use l2_protocol_game::server::party::{PartyDistributionType, PartyWindowMember};
        let in_game = ConnectionState::InGame;
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestJoinParty {
                    name: "Bob".into(),
                    party_distribution_type_id: 1
                }
                .to_bytes()
            ),
            json!({ "name": "Bob", "party_distribution_type_id": 1 })
        );
        assert_eq!(
            c2s_fields(in_game, &RequestAnswerJoinParty { response: -1 }.to_bytes()),
            json!({ "response": -1 })
        );
        assert_eq!(c2s_fields(in_game, &RequestWithDrawalParty.to_bytes()), json!({}));
        assert_eq!(
            c2s_fields(in_game, &RequestOustPartyMember { name: "Eve".into() }.to_bytes()),
            json!({ "name": "Eve" })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestChangePartyLeader { name: "Eve".into() }.to_bytes()
            ),
            json!({ "name": "Eve" })
        );

        assert_eq!(
            s2c_fields(
                &AskJoinParty {
                    requestor_name: "Ann".into(),
                    distribution_type: PartyDistributionType::ByTurn
                }
                .to_bytes()
            ),
            json!({ "requestor_name": "Ann", "distribution_type": 3 })
        );
        assert_eq!(
            s2c_fields(&JoinParty { response: 1 }.to_bytes()),
            json!({ "response": 1 })
        );
        let vitals = PartyMemberVitals {
            object_id: 7,
            name: "Cy".into(),
            cp: 1,
            max_cp: 2,
            hp: 3,
            max_hp: 4,
            mp: 5,
            max_mp: 6,
            level: 40,
            class_id: 18,
        };
        let all = PartySmallWindowAll {
            leader_object_id: 7,
            distribution_type: PartyDistributionType::Random,
            members: vec![PartyWindowMember {
                vitals: vitals.clone(),
                race: 2,
                summon: Some(PartySummon {
                    object_id: 9,
                    display_id: 14_000,
                    summon_type: 1,
                    name: "Pet".into(),
                    hp: 1,
                    max_hp: 2,
                    mp: 3,
                    max_mp: 4,
                    level: 5,
                }),
            }],
        };
        let fields = s2c_fields(&all.to_bytes());
        assert_eq!(fields["members"][0]["name"], "Cy");
        assert_eq!(fields["members"][0]["race"], 2);
        assert_eq!(fields["members"][0]["summon"]["display_id"], 14_000);
        assert_eq!(
            s2c_fields(&PartySmallWindowUpdate { member: vitals }.to_bytes())["max_hp"],
            4
        );
        assert_eq!(s2c_fields(&PartySmallWindowDeleteAll.to_bytes()), json!({}));
        assert_eq!(
            s2c_fields(
                &PartySmallWindowDelete {
                    object_id: 7,
                    name: "Cy".into()
                }
                .to_bytes()
            ),
            json!({ "object_id": 7, "name": "Cy" })
        );
        assert_eq!(
            s2c_fields(
                &ExPartyPetWindowDelete {
                    summon_object_id: 9,
                    owner_object_id: 7,
                    summon_name: "Pet".into()
                }
                .to_bytes()
            ),
            json!({ "summon_object_id": 9, "owner_object_id": 7, "summon_name": "Pet" })
        );
    }
    #[test]
    fn shop_packets_show_their_fields() {
        use l2_protocol_game::client::{
            BuyItemEntry, RequestBuyItem, RequestBuySellUIClose, RequestRefundItem, RequestSellItem,
            SellItemEntry,
        };
        use l2_protocol_game::server::shop::{BuyListProduct, RefundEntry, SellEntry};
        let in_game = ConnectionState::InGame;
        let buy = RequestBuyItem {
            list_id: 382,
            items: Some(vec![BuyItemEntry {
                item_id: 1835,
                count: 5,
            }]),
        };
        assert_eq!(
            c2s_fields(in_game, &buy.to_bytes()),
            json!({ "list_id": 382, "items": [{ "item_id": 1835, "count": 5 }] })
        );
        let sell = RequestSellItem {
            list_id: 382,
            items: Some(vec![SellItemEntry {
                object_id: 7,
                item_id: 57,
                count: 2,
            }]),
        };
        assert_eq!(
            c2s_fields(in_game, &sell.to_bytes()),
            json!({ "list_id": 382, "items": [{ "object_id": 7, "item_id": 57, "count": 2 }] })
        );
        let refund = RequestRefundItem {
            list_id: 382,
            items: Some(vec![0, 3]),
        };
        assert_eq!(
            c2s_fields(in_game, &refund.to_bytes()),
            json!({ "list_id": 382, "items": [0, 3] })
        );
        assert_eq!(c2s_fields(in_game, &RequestBuySellUIClose.to_bytes()), json!({}));
        // A bad list shows as null.
        let bad = RequestBuyItem {
            list_id: 1,
            items: None,
        };
        assert_eq!(
            c2s_fields(in_game, &bad.to_bytes()),
            json!({ "list_id": 1, "items": null })
        );

        // The merchant's list and the sell side share one opcode.
        let list = BuyList::new(
            1000,
            382,
            vec![BuyListProduct {
                item_id: 1835,
                count: 0,
                type2: 5,
                type1: 4,
                body_part: 0,
                enchant: 0,
                price: 25,
            }],
        );
        let fields = s2c_fields(&list.to_bytes());
        assert_eq!(fields["list_id"], 382);
        assert_eq!(fields["products"][0]["price"], 25);
        let sides = ExBuySellList {
            sell_list: vec![SellEntry {
                item: ItemInfo::plain(1, 57, 3),
                price: 1,
            }],
            refund_list: vec![RefundEntry {
                item: ItemInfo::plain(2, 1835, 1),
                index: 0,
                price: 10,
            }],
            done: true,
        };
        let fields = s2c_fields(&sides.to_bytes());
        assert_eq!(fields["sell_list"][0]["price"], 1);
        assert_eq!(fields["refund_list"][0]["item"]["item_id"], 1835);
        assert_eq!(fields["done"], true);
    }

    #[test]
    fn trade_packets_show_their_fields() {
        use l2_protocol_game::client::{AddTradeItem, AnswerTradeRequest, TradeDone, TradeRequest};
        let in_game = ConnectionState::InGame;
        assert_eq!(
            c2s_fields(in_game, &TradeRequest { object_id: 5 }.to_bytes()),
            json!({ "object_id": 5 })
        );
        assert_eq!(
            c2s_fields(in_game, &AnswerTradeRequest { response: 1 }.to_bytes()),
            json!({ "response": 1 })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &AddTradeItem {
                    trade_id: 1,
                    object_id: 2,
                    count: 3
                }
                .to_bytes()
            ),
            json!({ "trade_id": 1, "object_id": 2, "count": 3 })
        );
        assert_eq!(
            c2s_fields(in_game, &TradeDone { response: 0 }.to_bytes()),
            json!({ "response": 0 })
        );

        assert_eq!(
            s2c_fields(&SendTradeRequest { sender_id: 9 }.to_bytes()),
            json!({ "sender_id": 9 })
        );
        assert_eq!(
            s2c_fields(&ServerTradeDone { num: 1 }.to_bytes()),
            json!({ "num": 1 })
        );
        assert_eq!(s2c_fields(&TradeOtherDone.to_bytes()), json!({}));
        let item = TradeItemInfo {
            type1: 4,
            object_id: 77,
            item_id: 57,
            count: 1_000,
            ..TradeItemInfo::default()
        };
        let fields = s2c_fields(&TradeOwnAdd { item: item.clone() }.to_bytes());
        assert_eq!(fields["item"]["item_id"], 57);
        assert_eq!(fields["item"]["count"], 1_000);
        assert_eq!(
            s2c_fields(&TradeUpdate { change: 3, item }.to_bytes())["change"],
            3
        );
        let start = TradeStart {
            partner_object_id: 11,
            items: vec![ItemInfo::plain(1, 57, 5)],
        };
        let fields = s2c_fields(&start.to_bytes());
        assert_eq!(fields["partner_object_id"], 11);
        assert_eq!(fields["items"][0]["count"], 5);
    }
    #[test]
    fn warehouse_packets_show_their_fields() {
        use l2_protocol_game::client::{
            RequestPackageSend, RequestPackageSendableItemList, SendWareHouseDepositList,
            SendWareHouseWithDrawList, WarehouseListEntry,
        };
        use l2_protocol_game::server::{SortedWareHouseWithdrawalList, SortedWarehouseItem};
        let in_game = ConnectionState::InGame;
        let entry = WarehouseListEntry {
            object_id: 4,
            count: 2,
        };
        assert_eq!(
            c2s_fields(
                in_game,
                &SendWareHouseDepositList {
                    items: Some(vec![entry])
                }
                .to_bytes()
            ),
            json!({ "items": [{ "object_id": 4, "count": 2 }] })
        );
        // A list Java drops shows as null.
        assert_eq!(
            c2s_fields(in_game, &SendWareHouseWithDrawList { items: None }.to_bytes()),
            json!({ "items": null })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestPackageSend {
                    object_id: 9,
                    items: Some(vec![entry])
                }
                .to_bytes()
            ),
            json!({ "object_id": 9, "items": [{ "object_id": 4, "count": 2 }] })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestPackageSendableItemList { object_id: 9 }.to_bytes()
            ),
            json!({ "object_id": 9 })
        );

        let plain = WareHouseWithdrawalList {
            wh_type: 4,
            player_adena: 77,
            items: vec![ItemInfo::plain(5, 57, 3)],
        };
        let fields = s2c_fields(&plain.to_bytes());
        assert_eq!(fields["wh_type"], 4);
        assert_eq!(fields["items"][0]["count"], 3);
        // The sorted list has the same opcode and the same byte layout.
        let sorted = SortedWareHouseWithdrawalList {
            wh_type: 1,
            player_adena: 5,
            items: vec![SortedWarehouseItem {
                object_id: 6,
                display_id: 57,
                count: 8,
                time: -1,
                ..SortedWarehouseItem::default()
            }],
        };
        let fields = s2c_fields(&sorted.to_bytes());
        assert_eq!(fields["wh_type"], 1);
        assert_eq!(fields["items"][0]["object_id"], 6);
        assert_eq!(fields["items"][0]["item_id"], 57);
        assert_eq!(fields["items"][0]["time"], -1);
        let to_list = PackageToList {
            players: vec![(12, "Neo".into())],
        };
        assert_eq!(
            s2c_fields(&to_list.to_bytes()),
            json!({ "players": [{ "object_id": 12, "name": "Neo" }] })
        );
        let sendable = PackageSendableList {
            player_object_id: 12,
            adena: 9,
            items: vec![ItemInfo::plain(1, 57, 5)],
        };
        let fields = s2c_fields(&sendable.to_bytes());
        assert_eq!(fields["player_object_id"], 12);
        assert_eq!(fields["items"][0]["item_id"], 57);
        let deposit = WareHouseDepositList {
            wh_type: 1,
            player_adena: 1,
            items: vec![ItemInfo::plain(1, 57, 5)],
        };
        assert_eq!(s2c_fields(&deposit.to_bytes())["items"][0]["count"], 5);
    }

    #[test]
    fn private_store_packets_show_their_fields() {
        use l2_protocol_game::client::{
            RequestPrivateStoreBuy, RequestPrivateStoreManageSell, SellListEntry, SetPrivateStoreListSell,
            SetPrivateStoreMsgSell, StoreBuyRequestEntry,
        };
        let in_game = ConnectionState::InGame;
        assert_eq!(
            c2s_fields(in_game, &RequestPrivateStoreManageSell.to_bytes()),
            json!({})
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &SetPrivateStoreMsgSell {
                    store_msg: "cheap".into()
                }
                .to_bytes()
            ),
            json!({ "store_msg": "cheap" })
        );
        let list = SetPrivateStoreListSell {
            package_sale: true,
            items: Some(vec![SellListEntry {
                item_id: 7,
                count: 2,
                price: 100,
            }]),
        };
        assert_eq!(
            c2s_fields(in_game, &list.to_bytes()),
            json!({ "package_sale": true, "items": [{ "item_id": 7, "count": 2, "price": 100 }] })
        );
        // A list Java drops (count 0) shows as null items.
        let dropped = SetPrivateStoreListSell {
            package_sale: false,
            items: None,
        };
        assert_eq!(
            c2s_fields(in_game, &dropped.to_bytes()),
            json!({ "package_sale": false, "items": null })
        );
        let buy = RequestPrivateStoreBuy {
            store_player_id: 3,
            items: Some(vec![StoreBuyRequestEntry {
                object_id: 4,
                count: 5,
                price: 6,
            }]),
        };
        assert_eq!(
            c2s_fields(in_game, &buy.to_bytes()),
            json!({ "store_player_id": 3, "items": [{ "object_id": 4, "count": 5, "price": 6 }] })
        );

        assert_eq!(
            s2c_fields(
                &PrivateStoreMsgBuy {
                    object_id: 8,
                    message: "wanted".into()
                }
                .to_bytes()
            ),
            json!({ "object_id": 8, "message": "wanted" })
        );
        assert_eq!(
            s2c_fields(
                &ExPrivateStoreSetWholeMsg {
                    object_id: 8,
                    message: "pack".into()
                }
                .to_bytes()
            )["message"],
            "pack"
        );
        let sell = PrivateStoreListSell {
            seller_object_id: 9,
            package_sale: false,
            player_adena: 1_000,
            items: vec![SellStoreEntry {
                item: ItemInfo::plain(5, 57, 3),
                price: 10,
                reference_price: 20,
            }],
        };
        let fields = s2c_fields(&sell.to_bytes());
        assert_eq!(fields["items"][0]["item"]["object_id"], 5);
        assert_eq!(fields["items"][0]["price"], 10);
        let buy_list = PrivateStoreListBuy {
            store_object_id: 9,
            player_adena: 1_000,
            items: vec![BuyStoreEntry {
                item: ItemInfo::plain(5, 57, 3),
                price: 10,
                reference_price: 20,
                count: 4,
            }],
        };
        assert_eq!(s2c_fields(&buy_list.to_bytes())["items"][0]["count"], 4);
    }
    #[test]
    fn pledge_packets_show_their_fields() {
        use l2_protocol_game::client::{
            AllyLeave, RequestJoinPledge, RequestPledgePower, RequestPledgeWarList, RequestSetPledgeCrest,
            RequestStartPledgeWar,
        };
        use l2_protocol_game::server::pledge::{AllianceClanInfo, PledgeMemberOnline};
        let in_game = ConnectionState::InGame;
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestJoinPledge {
                    target: 5,
                    pledge_type: -1
                }
                .to_bytes()
            ),
            json!({ "target": 5, "pledge_type": -1 })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestStartPledgeWar {
                    pledge_name: "Foes".into()
                }
                .to_bytes()
            ),
            json!({ "pledge_name": "Foes" })
        );
        assert_eq!(c2s_fields(in_game, &AllyLeave.to_bytes()), json!({}));
        assert_eq!(
            c2s_fields(in_game, &RequestPledgeWarList { unk1: 0, tab: 1 }.to_bytes()),
            json!({ "unk1": 0, "tab": 1 })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestPledgePower {
                    rank: 2,
                    action: 2,
                    privs: 0x30
                }
                .to_bytes()
            ),
            json!({ "rank": 2, "action": 2, "privs": 48 })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestSetPledgeCrest {
                    length: 2,
                    data: Some(vec![0xAB, 0xCD])
                }
                .to_bytes()
            ),
            json!({ "length": 2, "data": "abcd" })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestSetPledgeCrest {
                    length: 300,
                    data: None
                }
                .to_bytes()
            ),
            json!({ "length": 300, "data": null })
        );

        let ask = AskJoinPledge {
            requestor_object_id: 7,
            sub_pledge_name: Some("Knights".into()),
            pledge_type: 100,
            pledge_name: "Clan".into(),
        };
        assert_eq!(
            s2c_fields(&ask.to_bytes()),
            json!({
                "requestor_object_id": 7,
                "sub_pledge_name": "Knights",
                "pledge_type": 100,
                "pledge_name": "Clan",
            })
        );
        assert_eq!(
            s2c_fields(&JoinPledge { pledge_id: 3 }.to_bytes()),
            json!({ "pledge_id": 3 })
        );
        assert_eq!(
            s2c_fields(
                &PledgeCrest {
                    crest_id: 9,
                    data: vec![1, 2]
                }
                .to_bytes()
            ),
            json!({ "crest_id": 9, "data": "0102" })
        );
        let list = PledgeShowMemberListAll {
            main_or_subpledge: 0,
            clan_id: 3,
            pledge_type: 0,
            clan_name: "Clan".into(),
            leader_name: "Boss".into(),
            crest_id: 0,
            level: 5,
            castle_id: 0,
            hideout_id: 0,
            fort_id: 0,
            rank: 1,
            reputation_score: 100,
            ally_id: 0,
            ally_name: String::new(),
            ally_crest_id: 0,
            at_war: false,
            members: vec![PledgeMember {
                name: "Boss".into(),
                level: 40,
                class_id: 18,
                sex: 0,
                race: 1,
                online_object_id: 11,
                has_sponsor: false,
            }],
        };
        let fields = s2c_fields(&list.to_bytes());
        assert_eq!(fields["clan_name"], "Clan");
        assert_eq!(fields["members"][0]["online_object_id"], 11);
        let update = PledgeShowMemberListUpdate {
            name: "Boss".into(),
            level: 40,
            class_id: 18,
            female: false,
            race: 1,
            online: Some(PledgeMemberOnline {
                object_id: 11,
                pledge_type: 0,
            }),
            has_sponsor: false,
        };
        assert_eq!(s2c_fields(&update.to_bytes())["online"]["object_id"], 11);
        let info = AllianceInfo {
            name: "Ally".into(),
            total: 10,
            online: 4,
            leader_clan: "Clan".into(),
            leader_player: "Boss".into(),
            allies: vec![AllianceClanInfo {
                clan_name: "Clan".into(),
                level: 5,
                leader_name: "Boss".into(),
                total: 10,
                online: 4,
            }],
        };
        assert_eq!(s2c_fields(&info.to_bytes())["allies"][0]["online"], 4);
        assert_eq!(s2c_fields(&PledgeShowMemberListDeleteAll.to_bytes()), json!({}));
    }
    #[test]
    fn summon_packets_show_their_fields() {
        use l2_protocol_game::client::{
            RequestChangePetName, RequestGetItemFromPet, RequestGiveItemToPet, RequestPetGetItem,
            RequestPetUseItem,
        };
        let in_game = ConnectionState::InGame;
        assert_eq!(
            c2s_fields(in_game, &RequestPetGetItem { object_id: 4 }.to_bytes()),
            json!({ "object_id": 4 })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestGetItemFromPet {
                    object_id: 4,
                    amount: 5,
                    unknown: 0
                }
                .to_bytes()
            ),
            json!({ "object_id": 4, "amount": 5, "unknown": 0 })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestGiveItemToPet {
                    object_id: 4,
                    amount: 5
                }
                .to_bytes()
            ),
            json!({ "object_id": 4, "amount": 5 })
        );
        assert_eq!(
            c2s_fields(in_game, &RequestPetUseItem { object_id: 4 }.to_bytes()),
            json!({ "object_id": 4 })
        );
        assert_eq!(
            c2s_fields(in_game, &RequestChangePetName { name: "Rex".into() }.to_bytes()),
            json!({ "name": "Rex" })
        );

        assert_eq!(
            s2c_fields(&PetStatusShow { summon_type: 2 }.to_bytes()),
            json!({ "summon_type": 2 })
        );
        assert_eq!(
            s2c_fields(
                &PetDelete {
                    pet_type: 1,
                    pet_object_id: 9
                }
                .to_bytes()
            ),
            json!({ "pet_type": 1, "pet_object_id": 9 })
        );
        assert_eq!(
            s2c_fields(
                &SetSummonRemainTime {
                    max_time: 1200,
                    remaining_time: 600
                }
                .to_bytes()
            ),
            json!({ "max_time": 1200, "remaining_time": 600 })
        );
        let list = PetItemList {
            items: vec![ItemInfo::plain(1, 57, 7)],
        };
        assert_eq!(s2c_fields(&list.to_bytes())["items"][0]["count"], 7);
        let status = PetStatusUpdate {
            summon_type: 2,
            object_id: 9,
            x: 1,
            y: 2,
            z: 3,
            title: "Owner".into(),
            cur_fed: 100,
            max_fed: 200,
            cur_hp: 10,
            max_hp: 20,
            cur_mp: 30,
            max_mp: 40,
            level: 15,
            exp: 5_000_000_000,
            exp_for_this_level: 4_000_000_000,
            exp_for_next_level: 6_000_000_000,
        };
        let fields = s2c_fields(&status.to_bytes());
        assert_eq!(fields["title"], "Owner");
        assert_eq!(fields["exp"], 5_000_000_000_i64);
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
