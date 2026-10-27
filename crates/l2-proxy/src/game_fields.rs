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
use l2_protocol_game::server::auction::ExItemAuctionInfo;
use l2_protocol_game::server::boss::{ExCursedWeaponList, ExCursedWeaponLocation, ExGetBossRecord};
use l2_protocol_game::server::charscreen::{
    CharCreateFail, CharCreateOk, CharDeleteFail, CharDeleteSuccess, CharSelected, CharSelectionInfo,
    NewCharacterSuccess, ServerClose, SsqInfo,
};
use l2_protocol_game::server::chat::{LeaveWorld, NetPing, RestartResponse};
use l2_protocol_game::server::combat::{
    Attack, AttackHit, AutoAttackStart, AutoAttackStop, ChangeMoveType, ChangeWaitType, Die,
    ExUseSharedGroupItem, Revive,
};
use l2_protocol_game::server::command_channel::{
    ExAskJoinMpcc, ExCloseMpcc, ExMpccPartyInfoUpdate, ExMpccShowPartyMemberInfo,
    ExMultiPartyCommandChannelInfo, ExOpenMpcc,
};
use l2_protocol_game::server::debug_draw::{ExServerPrimitive, ShowBoard};
use l2_protocol_game::server::duel::{
    ExDuelAskStart, ExDuelEnd, ExDuelReady, ExDuelStart, ExDuelUpdateUserInfo,
};
use l2_protocol_game::server::effect_packets::FlyToLocation;
use l2_protocol_game::server::enchant::{
    ChooseInventoryItem, EnchantResult, ExAttributeEnchantResult, ExBaseAttributeCancelResult,
    ExChooseInventoryAttributeItem, ExPutEnchantSupportItemResult, ExPutEnchantTargetItemResult,
    ExShowBaseAttributeCancelWindow,
};
use l2_protocol_game::server::enchant_skill::{
    ExEnchantSkillInfo, ExEnchantSkillInfoDetail, ExEnchantSkillList, ExEnchantSkillResult,
};
use l2_protocol_game::server::enterworld::{
    EtcStatusUpdate, ExBasicActionList, ExGetBookMarkInfoPacket, ExShowScreenMessage, ExStorageMaxCount,
    ExUiSetting, ExVoteSystemInfo, FriendList, HennaInfo, QuestList, ShortcutEntry, ShortcutInit,
    ShortcutRegister, SkillCoolTime, SkillList, ValidateLocation,
};
use l2_protocol_game::server::entry::{
    ExBrExtraUserInfo, ExNevitAdventPointInfo, ExNevitAdventTimeChange, ExQuestItemList, ExRotation,
    ExShowContactList, MacroCommandEntry, MacroEntry, MacroList,
};
use l2_protocol_game::server::fishing::{
    ExFishingEnd, ExFishingHpRegen, ExFishingStart, ExFishingStartCombat,
};
use l2_protocol_game::server::fort::{ExShowFortressInfo, ExShowFortressMapInfo, ExShowFortressSiegeInfo};
use l2_protocol_game::server::friend::{FriendAddRequest, FriendPacket, FriendStatusPacket, L2FriendSay};
use l2_protocol_game::server::gm_view::GmViewItemList;
use l2_protocol_game::server::gm_windows::{
    GmHennaInfo, GmViewCharacterInfo, GmViewQuestInfo, GmViewSkillInfo, GmViewWarehouseWithdrawList,
};
use l2_protocol_game::server::ground_item::{DropItem, GetItem, SpawnItem};
use l2_protocol_game::server::handshake::{KeyPacket, LoginFail};
use l2_protocol_game::server::henna::{
    HennaEquipList, HennaItemInfo, HennaItemRemoveInfo, HennaPreview, HennaRemoveList,
};
use l2_protocol_game::server::interaction::{
    ActionFailed, DoorInfo, DoorStatusUpdate, MyTargetSelected, NpcHtmlMessage, StaticObjectInfo,
    TargetSelected, TargetUnselected,
};
use l2_protocol_game::server::items::{InventoryBlock, ItemInfo, ItemList};
use l2_protocol_game::server::leftover::{ExRpItemLink, ShopPreviewInfo};
use l2_protocol_game::server::mail::{
    ExChangePostState, ExMailArrived, ExNoticePostArrived, ExNoticePostSent, ExReplyPostItemList,
    ExReplyReceivedPost, ExReplySentPost, ExShowReceivedPostList, ExShowSentPostList,
};
use l2_protocol_game::server::manor::{
    BuyListSeed, ExSendManorList, ExShowCropInfo, ExShowCropSetting, ExShowManorDefaultInfo,
    ExShowProcureCropDetail, ExShowSeedInfo, ExShowSeedSetting, ExShowSellCropList,
};
use l2_protocol_game::server::minigame::{
    ExCubeGameAddPlayer, ExCubeGameChangePoints, ExCubeGameChangeTeam, ExCubeGameChangeTimeToStart,
    ExCubeGameCloseUi, ExCubeGameEnd, ExCubeGameExtendedChangePoints, ExCubeGameRemovePlayer,
    ExCubeGameRequestReady, ExCubeGameTeamList, ExPvpMatchCcMyRecord, ExPvpMatchCcRecord, ExPvpMatchCcRetire,
    ExPvpMatchRecord, ExPvpMatchUserDie, ExSendUiEvent, ExStartScenePlayer,
};
use l2_protocol_game::server::movement::{MoveToLocation, MoveToPawn, StopMove, TeleportToLocation};
use l2_protocol_game::server::multisell::{MultiSellItemDetail, MultiSellList};
use l2_protocol_game::server::olympiad::{
    ExHeroList, ExOlympiadMatchEnd, ExOlympiadMatchList, ExOlympiadMatchResult, ExOlympiadMode,
    ExOlympiadSpelledInfo, ExOlympiadUserInfo, ObservationMode, ObservationReturn,
};
use l2_protocol_game::server::party::{
    AskJoinParty, ExPartyPetWindowAdd, ExPartyPetWindowDelete, ExPartyPetWindowUpdate, JoinParty,
    PartyMemberPosition, PartyMemberVitals, PartyPetWindow, PartySmallWindowAdd, PartySmallWindowAll,
    PartySmallWindowDelete, PartySmallWindowDeleteAll, PartySmallWindowUpdate, PartySummon,
};
use l2_protocol_game::server::party_room::{
    ExAskJoinPartyRoom, ExAskModifyPartyLooting, ExClosePartyRoom, ExListPartyMatchingWaitingRoom,
    ExManagePartyRoomMember, ExPartyRoomMember, ExRequestChangeNicknameColor, ExSetPartyLooting,
    ListPartyWaiting, PartyRoomInfo,
};
use l2_protocol_game::server::pccafe::ExPcCafePointInfo;
use l2_protocol_game::server::pledge::{
    AllianceInfo, AllyCrest, AskJoinAlly, AskJoinPledge, ExPledgeEmblem, ExSubPledgeSkillAdd,
    GmViewPledgeInfo, JoinPledge, ManagePledgePower, PledgeCrest, PledgeInfo, PledgeMember,
    PledgePowerGradeList, PledgeReceiveMemberInfo, PledgeReceivePowerInfo, PledgeReceiveSubPledgeCreated,
    PledgeReceiveWarList, PledgeShowInfoUpdate, PledgeShowMemberListAdd, PledgeShowMemberListAll,
    PledgeShowMemberListDelete, PledgeShowMemberListDeleteAll, PledgeShowMemberListUpdate, PledgeSkillList,
    PledgeSkillListAdd, PledgeStatusChanged, StartPledgeWar, StopPledgeWar, SurrenderPledgeWar,
};
use l2_protocol_game::server::recipe::{
    RecipeBookItemList, RecipeItemMakeInfo, RecipeShopItemInfo, RecipeShopManageList, RecipeShopMsg,
    RecipeShopSellList,
};
use l2_protocol_game::server::refine::{
    ExPutCommissionResultForVariationMake, ExPutIntensiveResultForVariationMake,
    ExPutItemResultForVariationCancel, ExPutItemResultForVariationMake, ExShowVariationCancelWindow,
    ExShowVariationMakeWindow, ExVariationCancelResult, ExVariationResult,
};
use l2_protocol_game::server::residence::{AgitDecoInfo, ExShowAgitInfo};
use l2_protocol_game::server::screen::{Earthquake, ExRedSky, SpecialCamera, SunRise, SunSet};
use l2_protocol_game::server::shop::{BuyList, ExBuySellList};
use l2_protocol_game::server::siege::{
    ExShowCastleInfo, SiegeAttackerList, SiegeClan, SiegeDefenderList, SiegeInfo, SiegeTime,
};
use l2_protocol_game::server::skills::{
    AbnormalStatusUpdate, AcquireSkillDone, AcquireSkillInfo, AcquireSkillList, EffectIcon, ExAutoSoulShot,
    MagicSkillCanceled, MagicSkillLaunched, MagicSkillUse, PartySpelled, ShortBuffStatusUpdate,
    SkillPosition,
};
use l2_protocol_game::server::sound::PlaySound;
use l2_protocol_game::server::ssq::{ExShowSeedMapInfo, SsqScore, SsqStatus, SsqStatusBody};
use l2_protocol_game::server::store::{
    BuyStoreEntry, ExPrivateStoreSetWholeMsg, PrivateStoreListBuy, PrivateStoreListSell,
    PrivateStoreManageListBuy, PrivateStoreManageListSell, PrivateStoreMsgBuy, PrivateStoreMsgSell,
    SellStoreEntry, StoreCandidate,
};
use l2_protocol_game::server::summon::{
    PetDelete, PetInfo, PetInventoryUpdate, PetItemList, PetStatusShow, PetStatusUpdate, Ride,
    SetSummonRemainTime,
};
use l2_protocol_game::server::system_message::{ConfirmDlg, SystemMessage, SystemMessageParam};
use l2_protocol_game::server::territory::{
    ExDominionWarEnd, ExDominionWarStart, ExReplyDominionInfo, ExShowDominionRegistry, ExShowOwnthingPos,
};
use l2_protocol_game::server::trade::{
    SendTradeRequest, TradeDone as ServerTradeDone, TradeItemInfo, TradeOtherAdd, TradeOtherDone,
    TradeOwnAdd, TradeStart, TradeUpdate,
};
use l2_protocol_game::server::userinfo::UserInfo;
use l2_protocol_game::server::vehicle::{
    ExAirShipInfo, ExAirShipStopMove, ExAirShipTeleportList, ExGetOffAirShip, ExGetOnAirShip,
    ExMoveToLocationAirShip, ExMoveToLocationInAirShip, ExStopMoveInAirShip, ExValidateLocationInAirShip,
    GetOffVehicle, GetOnVehicle, MoveToLocationInVehicle, StopMoveInVehicle, ValidateLocationInVehicle,
    VehicleCheckLocation, VehicleDeparture, VehicleInfo, VehicleStarted,
};
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
        // L-04: bookmarks, secondary password (never shown), contacts, hardware (the MAC address
        // is never shown), PC cafe coupon, GameGuard.
        ClientPacket::RequestBookMarkSlotInfo(_) | ClientPacket::RequestChangeBookMarkSlot(_) => json!({}),
        ClientPacket::RequestSaveBookMarkSlot(p) => object!(p => name, icon, tag),
        ClientPacket::RequestModifyBookMarkSlot(p) => object!(p => id, name, icon, tag),
        ClientPacket::RequestDeleteBookMarkSlot(p) => object!(p => id),
        ClientPacket::RequestTeleportBookMark(p) => object!(p => id),
        ClientPacket::RequestEx2ndPasswordCheck(_) => json!({}),
        ClientPacket::RequestEx2ndPasswordVerify(_) => json!({ "password": REDACTED }),
        ClientPacket::RequestEx2ndPasswordReq(p) => {
            json!({ "change_pass": p.change_pass, "password": REDACTED, "new_password": p.new_password.as_ref().map(|_| REDACTED) })
        }
        ClientPacket::RequestExShowContactList(_) | ClientPacket::RequestExFriendListExtended(_) => json!({}),
        ClientPacket::RequestExAddContactToContactList(p) => object!(p => name),
        ClientPacket::RequestExDeleteContactFromContactList(p) => object!(p => name),
        ClientPacket::RequestHardWareInfo(p) => json!({
            "mac_address": REDACTED, "windows_platform_id": p.windows_platform_id,
            "windows_major_version": p.windows_major_version, "windows_minor_version": p.windows_minor_version,
            "windows_build_number": p.windows_build_number, "directx_version": p.directx_version,
            "directx_revision": p.directx_revision, "cpu_name": p.cpu_name, "cpu_speed": p.cpu_speed,
            "cpu_core_count": p.cpu_core_count, "vga_count": p.vga_count, "vga_pcx_speed": p.vga_pcx_speed,
            "phys_memory": [p.phys_memory_slot1, p.phys_memory_slot2, p.phys_memory_slot3],
            "video_memory": p.video_memory, "vga_version": p.vga_version, "vga_name": p.vga_name,
            "vga_driver_version": p.vga_driver_version,
        }),
        ClientPacket::RequestPCCafeCouponUse(p) => object!(p => coupon),
        ClientPacket::GameGuardReply(p) => json!({ "reply": hex::encode(p.reply) }),
        ClientPacket::RequestHennaItemList(p) => object!(p => unknown),
        ClientPacket::RequestHennaItemInfo(p) => object!(p => symbol_id),
        ClientPacket::RequestHennaEquip(p) => object!(p => symbol_id),
        ClientPacket::RequestHennaRemoveList(p) => object!(p => unknown),
        ClientPacket::RequestHennaItemRemoveInfo(p) => object!(p => symbol_id),
        ClientPacket::RequestHennaRemove(p) => object!(p => symbol_id),
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
        ClientPacket::RequestPetition(p) => object!(p => content, kind),
        ClientPacket::RequestPetitionCancel(_) => json!({}),
        ClientPacket::RequestPetitionFeedback(p) => object!(p => unknown, rate, message),
        ClientPacket::SnoopQuit(p) => object!(p => snoop_id),
        ClientPacket::AnswerCoupleAction(p) => object!(p => action_id, answer, object_id),
        ClientPacket::DlgAnswer(p) => object!(p => message_id, answer, requester_id),
        ClientPacket::RequestJoinParty(p) => object!(p => name, party_distribution_type_id),
        ClientPacket::RequestAnswerJoinParty(p) => object!(p => response),
        ClientPacket::RequestWithDrawalParty(_) => json!({}),
        ClientPacket::RequestOustPartyMember(p) => object!(p => name),
        ClientPacket::RequestFriendInvite(p) => object!(p => name),
        ClientPacket::RequestAnswerFriendInvite(p) => object!(p => response),
        ClientPacket::RequestFriendList(_) => json!({}),
        ClientPacket::RequestFriendDel(p) => object!(p => name),
        ClientPacket::RequestSendFriendMsg(p) => object!(p => message, receiver),
        ClientPacket::RequestBlock(p) => object!(p => kind, name),
        ClientPacket::RequestChangePartyLeader(p) => object!(p => name),
        ClientPacket::RequestPartyMatchConfig(p) => object!(p => auto, location, level),
        ClientPacket::RequestPartyMatchList(p) => {
            object!(p => room_id, members_max, min_level, max_level, loot, room_title)
        }
        ClientPacket::RequestPartyMatchDetail(p) => object!(p => room_id, unk1, unk2, unk3),
        ClientPacket::RequestOustFromPartyRoom(p) => object!(p => char_id),
        ClientPacket::RequestDismissPartyRoom(p) => object!(p => room_id, data2),
        ClientPacket::RequestWithdrawPartyRoom(p) => object!(p => room_id, unk1),
        ClientPacket::RequestAskJoinPartyRoom(p) => object!(p => name),
        ClientPacket::AnswerJoinPartyRoom(p) => object!(p => answer),
        ClientPacket::RequestListPartyMatchingWaitingRoom(p) => {
            object!(p => page, min_level, max_level, mode)
        }
        ClientPacket::RequestExitPartyMatchingWaitingRoom(_) => json!({}),
        ClientPacket::RequestPartyLootModification(p) => object!(p => distribution_type_id),
        ClientPacket::AnswerPartyLootModification(p) => object!(p => answer),
        ClientPacket::RequestGiveNickName(p) => object!(p => target, title),
        ClientPacket::RequestChangeNicknameColor(p) => object!(p => color_num, title, item_object_id),
        ClientPacket::RequestResetNickname(_) => json!({}),
        ClientPacket::RequestGmList(_) => json!({}),
        ClientPacket::RequestGmCommand(p) => object!(p => target_name, command),
        ClientPacket::RequestExAskJoinMpcc(p) => object!(p => name),
        ClientPacket::RequestExAcceptJoinMpcc(p) => object!(p => response),
        ClientPacket::RequestExOustFromMpcc(p) => object!(p => name),
        ClientPacket::RequestExMpccShowPartyMembersInfo(p) => object!(p => party_leader_id),
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
        ClientPacket::RequestRecipeBookOpen(p) => object!(p => recipe_type),
        ClientPacket::RequestRecipeBookDestroy(p) => object!(p => recipe_id),
        ClientPacket::RequestRecipeItemMakeInfo(p) => object!(p => id),
        ClientPacket::RequestRecipeItemMakeSelf(p) => object!(p => id),
        ClientPacket::RequestRecipeShopMessageSet(p) => object!(p => name),
        ClientPacket::RequestRecipeShopListSet(p) => json!({
            "items": p.items.as_ref().map(|v| {
                v.iter().map(|i| object!(i => recipe_id, cost)).collect::<Vec<_>>()
            }),
        }),
        ClientPacket::RequestRecipeShopManageQuit(_) => json!({}),
        ClientPacket::RequestRecipeShopMakeInfo(p) => object!(p => player_object_id, recipe_id),
        ClientPacket::RequestRecipeShopMakeItem(p) => object!(p => id, recipe_id, unknown),
        ClientPacket::RequestRecipeShopManagePrev(_) => json!({}),
        ClientPacket::MultiSellChoose(p) => {
            object!(p => list_id, entry_id, amount, unk1, unk2, unk3, elemental)
        }
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
        ClientPacket::RequestDuelSurrender(_) | ClientPacket::RequestExFishRanking(_) => json!({}),
        ClientPacket::RequestPostItemList(_)
        | ClientPacket::RequestReceivedPostList(_)
        | ClientPacket::RequestSentPostList(_) => json!({}),
        ClientPacket::RequestSendPost(p) => json!({
            "receiver": p.receiver,
            "is_cod": p.is_cod,
            "subject": p.subject,
            "text": p.text,
            "items": p.items.as_ref().map(|v| {
                v.iter().map(|i| object!(i => object_id, count)).collect::<Vec<_>>()
            }),
            "req_adena": p.req_adena,
        }),
        ClientPacket::RequestReceivedPost(p) => object!(p => msg_id),
        ClientPacket::RequestSentPost(p) => object!(p => msg_id),
        ClientPacket::RequestPostAttachment(p) => object!(p => msg_id),
        ClientPacket::RequestRejectPostAttachment(p) => object!(p => msg_id),
        ClientPacket::RequestCancelPostAttachment(p) => object!(p => msg_id),
        ClientPacket::RequestDeleteReceivedPost(p) => json!({ "msg_ids": p.msg_ids }),
        ClientPacket::RequestDeleteSentPost(p) => json!({ "msg_ids": p.msg_ids }),
        ClientPacket::RequestEnchantItem(p) => object!(p => object_id),
        ClientPacket::RequestExTryToPutEnchantTargetItem(p) => object!(p => object_id),
        ClientPacket::RequestExTryToPutEnchantSupportItem(p) => {
            object!(p => support_object_id, enchant_object_id)
        }
        ClientPacket::RequestExCancelEnchantItem(_) => json!({}),
        ClientPacket::RequestExEnchantItemAttribute(p) => object!(p => object_id),
        ClientPacket::RequestExRemoveItemAttribute(p) => object!(p => object_id, element),
        ClientPacket::RequestExEnchantSkillInfo(p) => object!(p => skill_id, skill_level),
        ClientPacket::RequestExEnchantSkillInfoDetail(p) => object!(p => kind, skill_id, skill_level),
        ClientPacket::RequestExEnchantSkill(p) => object!(p => skill_id, skill_level),
        ClientPacket::RequestExEnchantSkillSafe(p) => object!(p => skill_id, skill_level),
        ClientPacket::RequestExEnchantSkillUntrain(p) => object!(p => skill_id, skill_level),
        ClientPacket::RequestExEnchantSkillRouteChange(p) => object!(p => skill_id, skill_level),
        ClientPacket::RequestConfirmTargetItem(p) => object!(p => object_id),
        ClientPacket::RequestConfirmRefinerItem(p) => {
            object!(p => target_item_object_id, refiner_item_object_id)
        }
        ClientPacket::RequestConfirmGemStone(p) => {
            object!(p => target_item_object_id, refiner_item_object_id, gemstone_item_object_id, gemstone_count)
        }
        ClientPacket::RequestRefine(p) => {
            object!(p => target_item_object_id, refiner_item_object_id, gemstone_item_object_id, gemstone_count)
        }
        ClientPacket::RequestConfirmCancelItem(p) => object!(p => object_id),
        ClientPacket::RequestRefineCancel(p) => object!(p => target_item_object_id),
        ClientPacket::RequestSiegeInfo(_) => json!({}),
        ClientPacket::RequestSiegeAttackerList(p) => object!(p => castle_id),
        ClientPacket::RequestSiegeDefenderList(p) => object!(p => castle_id),
        ClientPacket::RequestJoinSiege(p) => object!(p => castle_id, is_attacker, is_joining),
        ClientPacket::RequestConfirmSiegeWaitingList(p) => object!(p => castle_id, clan_id, approved),
        ClientPacket::RequestSetCastleSiegeTime(p) => object!(p => castle_id, time),
        ClientPacket::RequestAllCastleInfo(_)
        | ClientPacket::RequestAllFortressInfo(_)
        | ClientPacket::RequestAllAgitInfo(_)
        | ClientPacket::RequestFortressSiegeInfo(_)
        | ClientPacket::RequestManorList(_)
        | ClientPacket::RequestDominionInfo(_) => json!({}),
        ClientPacket::RequestFortressMapInfo(p) => object!(p => fortress_id),
        ClientPacket::RequestBuySeed(p) => json!({
            "manor_id": p.manor_id,
            "items": p.items.as_ref().map(|v| v.iter().map(|i| object!(i => item_id, count)).collect::<Vec<_>>()),
        }),
        ClientPacket::RequestProcureCropList(p) => json!({
            "items": p.items.as_ref().map(|v| {
                v.iter()
                    .map(|i| object!(i => object_id, item_id, manor_id, count))
                    .collect::<Vec<_>>()
            }),
        }),
        ClientPacket::RequestSetSeed(p) => json!({
            "manor_id": p.manor_id,
            "items": p.items.iter().map(|i| object!(i => item_id, sales, price)).collect::<Vec<_>>(),
        }),
        ClientPacket::RequestSetCrop(p) => json!({
            "manor_id": p.manor_id,
            "items": p.items.iter()
                .map(|i| object!(i => item_id, sales, price, reward_type))
                .collect::<Vec<_>>(),
        }),
        ClientPacket::RequestJoinDominionWar(p) => object!(p => territory_id, is_clan, is_joining),
        // L-02
        ClientPacket::RequestMakeMacro(p) => {
            let commands: Vec<Value> = p
                .commands
                .iter()
                .map(|c| object!(c => entry, kind, d1, d2, command))
                .collect();
            json!({
                "id": p.id, "name": p.name, "descr": p.descr, "acronym": p.acronym, "icon": p.icon,
                "commands": commands,
            })
        }
        ClientPacket::RequestDeleteMacro(p) => object!(p => id),
        ClientPacket::RequestCrystallizeItem(p) => object!(p => object_id, count),
        ClientPacket::RequestPreviewItem(p) => object!(p => unknown, list_id, items),
        ClientPacket::RequestExRqItemLink(p) => object!(p => object_id),
        ClientPacket::RequestSaveKeyMapping(p) => {
            json!({ "ui_key_mapping": p.ui_key_mapping.as_ref().map(hex::encode) })
        }
        ClientPacket::RequestSaveInventoryOrder(p) => json!({
            "order": p.order.iter().map(|(object_id, order)| json!({ "object_id": object_id, "order": order }))
                .collect::<Vec<_>>(),
        }),
        ClientPacket::RequestExMagicSkillUseGround(p) => {
            object!(p => x, y, z, skill_id, ctrl_pressed, shift_pressed)
        }
        ClientPacket::RequestShowMiniMap(_)
        | ClientPacket::RequestRecordInfo(_)
        | ClientPacket::RequestSkillList(_) => json!({}),
        ClientPacket::MoveWithDelta(p) => object!(p => dx, dy, dz),
        ClientPacket::StartRotating(p) => object!(p => degree, side),
        ClientPacket::FinishRotating(p) => object!(p => degree, unknown),
        ClientPacket::RequestBidItemAuction(p) => object!(p => instance_id, bid),
        ClientPacket::RequestInfoItemAuction(p) => object!(p => instance_id),
        ClientPacket::ObserverReturn(_)
        | ClientPacket::RequestOlympiadMatchList(_)
        | ClientPacket::RequestOlympiadObserverEnd(_)
        | ClientPacket::RequestExOlympiadMatchListRefresh(_)
        | ClientPacket::RequestSeedPhase(_)
        | ClientPacket::RequestCursedWeaponList(_)
        | ClientPacket::RequestCursedWeaponLocation(_) => json!({}),
        ClientPacket::RequestWriteHeroWords(p) => object!(p => hero_words),
        ClientPacket::RequestSsqStatus(p) => object!(p => page),
        ClientPacket::RequestGetBossRecord(p) => object!(p => boss_id),
        ClientPacket::RequestGetOnVehicle(p) => object!(p => boat_id, x, y, z),
        ClientPacket::RequestGetOffVehicle(p) => object!(p => boat_id, x, y, z),
        ClientPacket::RequestMoveToLocationInVehicle(p) => object!(
            p => boat_id, target_x, target_y, target_z, origin_x, origin_y, origin_z
        ),
        ClientPacket::CannotMoveAnymoreInVehicle(p) => object!(p => boat_id, x, y, z, heading),
        ClientPacket::RequestExCubeGameChangeTeam(p) => object!(p => arena, team),
        ClientPacket::RequestExCubeGameReadyAnswer(p) => object!(p => arena, answer),
        ClientPacket::EndScenePlayer(p) => object!(p => movie_id),
        ClientPacket::MoveToLocationAirShip(p) => object!(p => command, param1, param2),
        ClientPacket::MoveToLocationInAirShip(p) => object!(
            p => ship_id, target_x, target_y, target_z, origin_x, origin_y, origin_z
        ),
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
        ServerOpcode::FriendAddRequest => decoded::<FriendAddRequest>(&mut r, friend_add_request_fields),
        ServerOpcode::L2Friend => decoded::<FriendPacket>(&mut r, friend_packet_fields),
        ServerOpcode::L2FriendStatus => decoded::<FriendStatusPacket>(&mut r, friend_status_fields),
        ServerOpcode::L2FriendSay => decoded::<L2FriendSay>(&mut r, l2_friend_say_fields),
        ServerOpcode::ValidateLocation => decoded::<ValidateLocation>(&mut r, validate_location_fields),
        ServerOpcode::QuestList => decoded::<QuestList>(&mut r, quest_list_fields),
        ServerOpcode::SkillCoolTime => decoded::<SkillCoolTime>(&mut r, skill_cool_time_fields),
        ServerOpcode::HennaInfo => decoded::<HennaInfo>(&mut r, henna_info_fields),
        ServerOpcode::HennaEquipList => decoded::<HennaEquipList>(&mut r, henna_equip_list_fields),
        ServerOpcode::HennaUnequipList => decoded::<HennaRemoveList>(&mut r, henna_remove_list_fields),
        ServerOpcode::HennaItemInfo => decoded::<HennaItemInfo>(&mut r, |p| henna_preview_fields(&p.0)),
        ServerOpcode::HennaUnequipInfo => {
            decoded::<HennaItemRemoveInfo>(&mut r, |p| henna_preview_fields(&p.0))
        }
        ServerOpcode::EtcStatusUpdate => decoded::<EtcStatusUpdate>(&mut r, etc_status_fields),
        ServerOpcode::ExStorageMaxCount => decoded::<ExStorageMaxCount>(&mut r, storage_max_count_fields),
        ServerOpcode::ExBasicActionList => decoded::<ExBasicActionList>(&mut r, basic_action_list_fields),
        ServerOpcode::ExVoteSystemInfo => decoded::<ExVoteSystemInfo>(&mut r, vote_system_info_fields),
        ServerOpcode::ExGetBookmarkInfo => decoded::<ExGetBookMarkInfoPacket>(&mut r, bookmark_info_fields),
        ServerOpcode::ExUiSetting => decoded::<ExUiSetting>(&mut r, ui_setting_fields),
        ServerOpcode::ShopPreviewInfo => decoded::<ShopPreviewInfo>(&mut r, shop_preview_info_fields),
        ServerOpcode::ExRpItemLink => decoded::<ExRpItemLink>(&mut r, rp_item_link_fields),
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
        ServerOpcode::ExConfirmAddingPostFriend => {
            decoded::<l2_protocol_game::server::ExConfirmAddingContact>(&mut r, |p| object!(p => name, added))
        }
        ServerOpcode::FriendList => decoded::<l2_protocol_game::server::FriendListExtended>(&mut r, |p| {
            let friends: Vec<Value> = p
                .friends
                .iter()
                .map(|f| object!(f => object_id, name, online, class_id, level))
                .collect();
            json!({ "friends": friends })
        }),
        ServerOpcode::Ex2ndPasswordCheck => {
            decoded::<l2_protocol_game::server::Ex2ndPasswordCheck>(&mut r, |p| object!(p => window_type))
        }
        ServerOpcode::Ex2ndPasswordVerify => decoded::<l2_protocol_game::server::Ex2ndPasswordVerify>(
            &mut r,
            |p| object!(p => mode, wrong_attempts),
        ),
        ServerOpcode::Ex2ndPasswordAck => {
            decoded::<l2_protocol_game::server::Ex2ndPasswordAck>(&mut r, |p| object!(p => response))
        }
        ServerOpcode::ExBrExtraUserInfo => decoded::<ExBrExtraUserInfo>(&mut r, br_extra_user_info_fields),
        ServerOpcode::ExNavitAdventPointInfo => decoded::<ExNevitAdventPointInfo>(&mut r, nevit_point_fields),
        ServerOpcode::ExPcCafePointInfo => decoded::<ExPcCafePointInfo>(&mut r, pc_cafe_point_info_fields),
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
        ServerOpcode::GmViewCharacterInfo => {
            decoded::<GmViewCharacterInfo>(&mut r, gm_view_character_info_fields)
        }
        ServerOpcode::GmHennaInfo => decoded::<GmHennaInfo>(&mut r, gm_henna_info_fields),
        ServerOpcode::GmViewSkillInfo => decoded::<GmViewSkillInfo>(&mut r, gm_view_skill_info_fields),
        ServerOpcode::GmViewQuestInfo => decoded::<GmViewQuestInfo>(&mut r, gm_view_quest_info_fields),
        ServerOpcode::GmViewWarehouseWithdrawList => {
            decoded::<GmViewWarehouseWithdrawList>(&mut r, gm_view_warehouse_fields)
        }
        ServerOpcode::PartyRoomInfo => decoded::<PartyRoomInfo>(&mut r, party_room_info_fields),
        ServerOpcode::ListPartyWaiting => decoded::<ListPartyWaiting>(&mut r, list_party_waiting_fields),
        ServerOpcode::ExPartyRoomMember => decoded::<ExPartyRoomMember>(&mut r, party_room_member_fields),
        ServerOpcode::ExManagePartyRoomMember => {
            decoded::<ExManagePartyRoomMember>(&mut r, manage_party_room_member_fields)
        }
        ServerOpcode::ExClosePartyRoom => decoded::<ExClosePartyRoom>(&mut r, no_fields),
        ServerOpcode::ExAskJoinPartyRoom => decoded::<ExAskJoinPartyRoom>(&mut r, ask_join_party_room_fields),
        ServerOpcode::ExListPartyMatchingWaitingRoom => {
            decoded::<ExListPartyMatchingWaitingRoom>(&mut r, list_matching_waiting_room_fields)
        }
        ServerOpcode::ExAskModifyPartyLooting => {
            decoded::<ExAskModifyPartyLooting>(&mut r, ask_modify_party_looting_fields)
        }
        ServerOpcode::ExSetPartyLooting => decoded::<ExSetPartyLooting>(&mut r, set_party_looting_fields),
        ServerOpcode::ExRequestChangeNicknameColor => {
            decoded::<ExRequestChangeNicknameColor>(&mut r, request_change_nickname_color_fields)
        }
        ServerOpcode::ExOpenMpcc => decoded::<ExOpenMpcc>(&mut r, no_fields),
        ServerOpcode::ExCloseMpcc => decoded::<ExCloseMpcc>(&mut r, no_fields),
        ServerOpcode::ExAskJoinMpcc => decoded::<ExAskJoinMpcc>(&mut r, ask_join_mpcc_fields),
        ServerOpcode::ExMpccPartyInfoUpdate => {
            decoded::<ExMpccPartyInfoUpdate>(&mut r, mpcc_party_info_update_fields)
        }
        ServerOpcode::ExMpccShowPartyMemberInfo => {
            decoded::<ExMpccShowPartyMemberInfo>(&mut r, mpcc_show_party_member_info_fields)
        }
        ServerOpcode::ExMultiPartyCommandChannelInfo => {
            decoded::<ExMultiPartyCommandChannelInfo>(&mut r, multi_party_command_channel_info_fields)
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
        ServerOpcode::MultiSellList => decoded::<MultiSellList>(&mut r, multi_sell_list_fields),
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
        ServerOpcode::Earthquake => decoded::<Earthquake>(&mut r, earthquake_fields),
        ServerOpcode::Sunrise => decoded::<SunRise>(&mut r, no_fields),
        ServerOpcode::Sunset => decoded::<SunSet>(&mut r, no_fields),
        ServerOpcode::ExRedSky => decoded::<ExRedSky>(&mut r, ex_red_sky_fields),
        ServerOpcode::SpecialCamera => decoded::<SpecialCamera>(&mut r, special_camera_fields),
        ServerOpcode::ExServerPrimitive => decoded::<ExServerPrimitive>(&mut r, server_primitive_fields),
        ServerOpcode::ShowBoard => decoded::<ShowBoard>(&mut r, show_board_fields),
        ServerOpcode::AgitDecoInfo => decoded::<AgitDecoInfo>(&mut r, agit_deco_info_fields),
        ServerOpcode::ChooseInventoryItem => {
            decoded::<ChooseInventoryItem>(&mut r, choose_inventory_item_fields)
        }
        ServerOpcode::EnchantResult => decoded::<EnchantResult>(&mut r, enchant_result_fields),
        ServerOpcode::ExPutEnchantTargetItemResult => {
            decoded::<ExPutEnchantTargetItemResult>(&mut r, put_enchant_target_fields)
        }
        ServerOpcode::ExPutEnchantSupportItemResult => {
            decoded::<ExPutEnchantSupportItemResult>(&mut r, put_enchant_support_fields)
        }
        ServerOpcode::ExAttributeEnchantResult => {
            decoded::<ExAttributeEnchantResult>(&mut r, attribute_enchant_result_fields)
        }
        ServerOpcode::ExBaseAttributeCancelResult => {
            decoded::<ExBaseAttributeCancelResult>(&mut r, base_attribute_cancel_result_fields)
        }
        ServerOpcode::ExChooseInventoryAttributeItem => {
            decoded::<ExChooseInventoryAttributeItem>(&mut r, choose_inventory_attribute_fields)
        }
        ServerOpcode::ExShowVariationMakeWindow => decoded::<ExShowVariationMakeWindow>(&mut r, no_fields),
        ServerOpcode::ExShowVariationCancelWindow => {
            decoded::<ExShowVariationCancelWindow>(&mut r, no_fields)
        }
        ServerOpcode::ExPutItemResultForVariationMake => {
            decoded::<ExPutItemResultForVariationMake>(&mut r, put_item_variation_make_fields)
        }
        ServerOpcode::ExPutIntensiveResultForVariationMake => {
            decoded::<ExPutIntensiveResultForVariationMake>(&mut r, put_intensive_variation_make_fields)
        }
        ServerOpcode::ExPutCommissionResultForVariationMake => {
            decoded::<ExPutCommissionResultForVariationMake>(&mut r, put_commission_variation_make_fields)
        }
        ServerOpcode::ExVariationResult => decoded::<ExVariationResult>(&mut r, variation_result_fields),
        ServerOpcode::ExPutItemResultForVariationCancel => {
            decoded::<ExPutItemResultForVariationCancel>(&mut r, put_item_variation_cancel_fields)
        }
        ServerOpcode::ExVariationCancelResult => {
            decoded::<ExVariationCancelResult>(&mut r, variation_cancel_result_fields)
        }
        ServerOpcode::ExShowBaseAttributeCancelWindow => {
            decoded::<ExShowBaseAttributeCancelWindow>(&mut r, base_attribute_cancel_window_fields)
        }
        ServerOpcode::ExEnchantSkillInfo => decoded::<ExEnchantSkillInfo>(&mut r, enchant_skill_info_fields),
        ServerOpcode::ExEnchantSkillInfoDetail => {
            decoded::<ExEnchantSkillInfoDetail>(&mut r, enchant_skill_info_detail_fields)
        }
        ServerOpcode::ExEnchantSkillList => decoded::<ExEnchantSkillList>(&mut r, enchant_skill_list_fields),
        ServerOpcode::ExEnchantSkillResult => {
            decoded::<ExEnchantSkillResult>(&mut r, enchant_skill_result_fields)
        }
        ServerOpcode::ExDuelAskStart => decoded::<ExDuelAskStart>(&mut r, duel_ask_start_fields),
        ServerOpcode::ExDuelReady => decoded::<ExDuelReady>(&mut r, duel_ready_fields),
        ServerOpcode::ExDuelStart => decoded::<ExDuelStart>(&mut r, duel_start_fields),
        ServerOpcode::ExDuelEnd => decoded::<ExDuelEnd>(&mut r, duel_end_fields),
        ServerOpcode::ExDuelUpdateUserInfo => {
            decoded::<ExDuelUpdateUserInfo>(&mut r, duel_update_user_info_fields)
        }
        ServerOpcode::ExFishingStart => decoded::<ExFishingStart>(
            &mut r,
            |p| object!(p => object_id, fish_type, x, y, z, night_lure),
        ),
        ServerOpcode::ExFishingStartCombat => decoded::<ExFishingStartCombat>(
            &mut r,
            |p| object!(p => object_id, time, hp, mode, lure_type, deceptive_mode),
        ),
        ServerOpcode::ExFishingHpRegen => decoded::<ExFishingHpRegen>(
            &mut r,
            |p| object!(p => object_id, time, fish_hp, hp_mode, good_use, anim, penalty, hp_bar_color),
        ),
        ServerOpcode::ExFishingEnd => decoded::<ExFishingEnd>(&mut r, |p| object!(p => object_id, win)),
        ServerOpcode::PetDelete => decoded::<PetDelete>(&mut r, pet_delete_fields),
        ServerOpcode::Ride => decoded::<Ride>(&mut r, ride_fields),
        ServerOpcode::RecipeBookItemList => {
            decoded::<RecipeBookItemList>(&mut r, |p| object!(p => is_common, max_mp, recipes))
        }
        ServerOpcode::RecipeItemMakeInfo => {
            decoded::<RecipeItemMakeInfo>(&mut r, |p| object!(p => id, is_common, cur_mp, max_mp, success))
        }
        ServerOpcode::RecipeShopItemInfo => {
            decoded::<RecipeShopItemInfo>(&mut r, |p| object!(p => object_id, recipe_id, cur_mp, max_mp))
        }
        ServerOpcode::RecipeShopManageList => decoded::<RecipeShopManageList>(&mut r, |p| {
            let items: Vec<Value> = p.items.iter().map(|i| object!(i => recipe_id, cost)).collect();
            json!({
                "seller_id": p.seller_id,
                "adena": p.adena,
                "is_common": p.is_common,
                "recipes": p.recipes,
                "items": items,
            })
        }),
        ServerOpcode::RecipeShopMsg => decoded::<RecipeShopMsg>(&mut r, |p| object!(p => object_id, message)),
        ServerOpcode::RecipeShopSellList => decoded::<RecipeShopSellList>(&mut r, |p| {
            let items: Vec<Value> = p.items.iter().map(|i| object!(i => recipe_id, cost)).collect();
            json!({
                "manufacturer_id": p.manufacturer_id,
                "cur_mp": p.cur_mp,
                "max_mp": p.max_mp,
                "buyer_adena": p.buyer_adena,
                "items": items,
            })
        }),
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
        ServerOpcode::ExNoticePostArrived => {
            decoded::<ExNoticePostArrived>(&mut r, |p| object!(p => show_animation))
        }
        ServerOpcode::ExNoticePostSent => {
            decoded::<ExNoticePostSent>(&mut r, |p| object!(p => show_animation))
        }
        ServerOpcode::ExMailArrived => decoded::<ExMailArrived>(&mut r, |_| json!({})),
        ServerOpcode::CastleSiegeInfo => decoded::<SiegeInfo>(&mut r, siege_info_fields),
        ServerOpcode::CastleSiegeAttackerList => {
            decoded::<SiegeAttackerList>(&mut r, siege_attacker_list_fields)
        }
        ServerOpcode::CastleSiegeDefenderList => {
            decoded::<SiegeDefenderList>(&mut r, siege_defender_list_fields)
        }
        ServerOpcode::ExShowCastleInfo => decoded::<ExShowCastleInfo>(&mut r, |p| {
            json!({
                "castles": p.castles.iter().map(|c| {
                    object!(c => residence_id, owner_name, tax_percent, siege_date_secs)
                }).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExShowAgitInfo => decoded::<ExShowAgitInfo>(&mut r, |p| {
            json!({
                "halls": p.halls.iter().map(|h| {
                    object!(h => hall_id, owner_clan_name, leader_name, grade_below_one)
                }).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExShowFortressInfo => decoded::<ExShowFortressInfo>(&mut r, |p| {
            json!({
                "forts": p.forts.iter().map(|f| {
                    object!(f => residence_id, owner_clan_name, siege_in_progress, owned_time)
                }).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExShowFortressSiegeInfo => {
            decoded::<ExShowFortressSiegeInfo>(&mut r, |p| object!(p => fort_id, barracks_total, barracks))
        }
        ServerOpcode::ExShowFortressMapInfo => decoded::<ExShowFortressMapInfo>(
            &mut r,
            |p| object!(p => fort_id, siege_in_progress, barracks_total, barracks),
        ),
        ServerOpcode::ExShowManorDefaultInfo => {
            decoded::<ExShowManorDefaultInfo>(&mut r, manor_default_fields)
        }
        ServerOpcode::ExShowSeedInfo => decoded::<ExShowSeedInfo>(&mut r, seed_info_fields),
        ServerOpcode::ExShowCropInfo => decoded::<ExShowCropInfo>(&mut r, crop_info_fields),
        ServerOpcode::ExShowSeedSetting => decoded::<ExShowSeedSetting>(&mut r, seed_setting_fields),
        ServerOpcode::ExShowCropSetting => decoded::<ExShowCropSetting>(&mut r, crop_setting_fields),
        ServerOpcode::ExShowProcureCropDetail => decoded::<ExShowProcureCropDetail>(&mut r, |p| {
            json!({
                "crop_id": p.crop_id,
                "castles": p.castles.iter().map(|c| {
                    object!(c => castle_id, amount, price, reward)
                }).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExShowSellCropList => decoded::<ExShowSellCropList>(&mut r, sell_crop_list_fields),
        ServerOpcode::ExSendManorList => decoded::<ExSendManorList>(&mut r, |p| {
            json!({
                "castles": p.castles.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::BuyListSeed => decoded::<BuyListSeed>(&mut r, |p| {
            json!({
                "money": p.money,
                "manor_id": p.manor_id,
                "seeds": p.seeds.iter().map(|s| {
                    object!(s => seed_id, amount, price)
                }).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExDominionWarStart => {
            decoded::<ExDominionWarStart>(&mut r, |p| object!(p => object_id, territory_id, is_disguised))
        }
        ServerOpcode::ExDominionWarEnd => decoded::<ExDominionWarEnd>(&mut r, no_fields),
        ServerOpcode::ExShowDominionRegistry => {
            decoded::<ExShowDominionRegistry>(&mut r, dominion_registry_fields)
        }
        ServerOpcode::ExReplyDominionInfo => decoded::<ExReplyDominionInfo>(&mut r, |p| {
            json!({
                "territories": p.territories.iter().map(|t| {
                    object!(t => territory_id, name, owner_clan_name, ward_ids, war_start_secs)
                }).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExShowOwnthingPos => decoded::<ExShowOwnthingPos>(&mut r, |p| {
            json!({
                "wards": p.wards.iter().map(|w| object!(w => territory_id, x, y, z)).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExItemAuctionInfo => decoded::<ExItemAuctionInfo>(&mut r, item_auction_fields),
        ServerOpcode::ExChangePostState => {
            decoded::<ExChangePostState>(&mut r, |p| object!(p => received_board, msg_ids, change_id))
        }
        ServerOpcode::ExShowReceivedPostList => {
            decoded::<ExShowReceivedPostList>(&mut r, show_received_post_list_fields)
        }
        ServerOpcode::ExShowSentPostList => decoded::<ExShowSentPostList>(&mut r, show_sent_post_list_fields),
        ServerOpcode::ExReplyReceivedPost => {
            decoded::<ExReplyReceivedPost>(&mut r, reply_received_post_fields)
        }
        ServerOpcode::ExReplySentPost => decoded::<ExReplySentPost>(&mut r, reply_sent_post_fields),
        ServerOpcode::ExReplyPostItemList => decoded::<ExReplyPostItemList>(
            &mut r,
            |p| json!({ "items": p.items.iter().map(item_fields).collect::<Vec<_>>() }),
        ),
        ServerOpcode::Snoop => decoded::<l2_protocol_game::server::Snoop>(&mut r, |p| {
            json!({
                "convo_id": p.convo_id,
                "name": p.name,
                "chat_type": p.chat_type.client_id(),
                "speaker": p.speaker,
                "message": p.message,
            })
        }),
        ServerOpcode::PetitionVote => {
            decoded::<l2_protocol_game::server::PetitionVotePacket>(&mut r, no_fields)
        }
        // M8: Olympiad, hero, Seven Signs, boats and air ships, Cube game, boss records.
        ServerOpcode::ExOlympiadMatchEnd => decoded::<ExOlympiadMatchEnd>(&mut r, no_fields),
        ServerOpcode::ExOlympiadMode => decoded::<ExOlympiadMode>(&mut r, |p| object!(p => mode)),
        // M8-OLY-01: the observer mode.
        ServerOpcode::ObserverStart => decoded::<ObservationMode>(&mut r, |p| object!(p => x, y, z)),
        ServerOpcode::ObserverEnd => decoded::<ObservationReturn>(&mut r, |p| object!(p => x, y, z)),
        ServerOpcode::ExReceiveOlympiad => receive_olympiad_fields(raw.get(start..).unwrap_or_default()),
        ServerOpcode::ExOlympiadUserInfo => decoded::<ExOlympiadUserInfo>(
            &mut r,
            |p| object!(p => side, object_id, name, class_id, cur_hp, max_hp, cur_cp, max_cp),
        ),
        ServerOpcode::ExOlympiadSpelledInfo => decoded::<ExOlympiadSpelledInfo>(&mut r, |p| {
            json!({
                "player_id": p.player_id,
                "effects": p.effects.iter().map(|e| object!(e => skill_id, level, time)).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExHeroList => decoded::<ExHeroList>(&mut r, |p| {
            json!({
                "heroes": p.heroes.iter().map(|h| object!(
                    h => name, class_id, clan_name, clan_crest, ally_name, ally_crest, count
                )).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::SsqStatus => decoded::<SsqStatus>(&mut r, ssq_status_fields),
        ServerOpcode::ExShowSeedMapInfo => decoded::<ExShowSeedMapInfo>(&mut r, |p| {
            json!({
                "entries": p.entries.iter().map(|e| object!(e => x, y, z, state)).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::VehicleInfo => {
            decoded::<VehicleInfo>(&mut r, |p| object!(p => object_id, x, y, z, heading))
        }
        ServerOpcode::VehicleDeparture => decoded::<VehicleDeparture>(
            &mut r,
            |p| object!(p => object_id, move_speed, rotation_speed, x, y, z),
        ),
        ServerOpcode::VehicleStart => decoded::<VehicleStarted>(&mut r, |p| object!(p => object_id, state)),
        ServerOpcode::VehicleCheckLocation => {
            decoded::<VehicleCheckLocation>(&mut r, |p| object!(p => object_id, x, y, z, heading))
        }
        ServerOpcode::GetOnVehicle => {
            decoded::<GetOnVehicle>(&mut r, |p| object!(p => object_id, boat_object_id, x, y, z))
        }
        ServerOpcode::GetOffVehicle => {
            decoded::<GetOffVehicle>(&mut r, |p| object!(p => object_id, boat_object_id, x, y, z))
        }
        ServerOpcode::MoveToLocationInVehicle => decoded::<MoveToLocationInVehicle>(
            &mut r,
            |p| object!(p => object_id, boat_id, destination_x, destination_y, destination_z, origin_x, origin_y, origin_z),
        ),
        ServerOpcode::StopMoveInVehicle => {
            decoded::<StopMoveInVehicle>(&mut r, |p| object!(p => object_id, boat_id, x, y, z, heading))
        }
        ServerOpcode::ValidateLocationInVehicle => decoded::<ValidateLocationInVehicle>(
            &mut r,
            |p| object!(p => object_id, boat_object_id, x, y, z, heading),
        ),
        ServerOpcode::ExAirshipInfo => decoded::<ExAirShipInfo>(&mut r, |p| {
            object!(p => ship_object_id, x, y, z, heading, captain_id, move_speed, rotation_speed,
                helm_object_id, helm_points, fuel, max_fuel)
        }),
        ServerOpcode::ExAirshipTeleportList => decoded::<ExAirShipTeleportList>(&mut r, |p| {
            json!({
                "dock_id": p.dock_id,
                "teleports": p.teleports.iter().map(|t| object!(t => index, fuel, x, y, z)).collect::<Vec<_>>(),
            })
        }),
        // Both air ship stop packets share `EX_STOP_MOVE_AIRSHIP` and five ints, so the wire cannot
        // tell them apart; the proxy shows the player layout.
        ServerOpcode::ExStopMoveAirship => {
            decoded::<ExAirShipStopMove>(&mut r, |p| object!(p => air_ship_id, player_id, x, y, z))
        }
        ServerOpcode::ExStopMoveInAirship => decoded::<ExStopMoveInAirShip>(
            &mut r,
            |p| object!(p => player_id, ship_object_id, x, y, z, heading),
        ),
        ServerOpcode::ExValidateLocationInAirship => decoded::<ExValidateLocationInAirShip>(
            &mut r,
            |p| object!(p => player_id, ship_id, x, y, z, heading),
        ),
        ServerOpcode::ExGetOnAirship => {
            decoded::<ExGetOnAirShip>(&mut r, |p| object!(p => player_id, air_ship_id, x, y, z))
        }
        ServerOpcode::ExGetOffAirship => {
            decoded::<ExGetOffAirShip>(&mut r, |p| object!(p => player_id, air_ship_id, x, y, z))
        }
        ServerOpcode::ExMoveToLocationAirship => decoded::<ExMoveToLocationAirShip>(
            &mut r,
            |p| object!(p => object_id, destination_x, destination_y, destination_z, x, y, z),
        ),
        ServerOpcode::ExMoveToLocationInAirship => decoded::<ExMoveToLocationInAirShip>(
            &mut r,
            |p| object!(p => object_id, air_ship_id, destination_x, destination_y, destination_z, heading),
        ),
        ServerOpcode::ExBlockUpSetList => block_list_fields(raw.get(start..).unwrap_or_default()),
        ServerOpcode::ExBlockUpSetState => block_state_fields(raw.get(start..).unwrap_or_default()),
        ServerOpcode::ExPvpMatchRecord => decoded::<ExPvpMatchRecord>(&mut r, pvp_record_fields),
        ServerOpcode::ExPvpMatchUserDie => {
            decoded::<ExPvpMatchUserDie>(&mut r, |p| object!(p => red_kills, blue_kills))
        }
        ServerOpcode::ExPvpMatchCcRecord => decoded::<ExPvpMatchCcRecord>(&mut r, |p| {
            json!({
                "state": p.state,
                "players": p.players.iter().map(|e| object!(e => name, points)).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExPvpMatchCcMyRecord => {
            decoded::<ExPvpMatchCcMyRecord>(&mut r, |p| object!(p => points))
        }
        ServerOpcode::ExPvpMatchCcRetire => decoded::<ExPvpMatchCcRetire>(&mut r, |_| json!({})),
        ServerOpcode::ExStartScenePlayer => decoded::<ExStartScenePlayer>(&mut r, |p| object!(p => movie_id)),
        ServerOpcode::ExSendUiEvent => decoded::<ExSendUiEvent>(
            &mut r,
            |p| object!(p => object_id, hide, count_up, start_time, end_time, npc_string_id, params),
        ),
        ServerOpcode::ExGetBossRecord => decoded::<ExGetBossRecord>(&mut r, |p| {
            json!({
                "ranking": p.ranking,
                "total_points": p.total_points,
                "records": p.records.iter().map(|x| object!(x => boss_id, points)).collect::<Vec<_>>(),
            })
        }),
        ServerOpcode::ExCursedWeaponList => decoded::<ExCursedWeaponList>(&mut r, |p| object!(p => ids)),
        ServerOpcode::ExCursedWeaponLocation => decoded::<ExCursedWeaponLocation>(&mut r, |p| {
            json!({
                "locations": p.locations.iter().map(|l| object!(l => id, activated, x, y, z)).collect::<Vec<_>>(),
            })
        }),
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

fn friend_add_request_fields(p: &FriendAddRequest) -> Value {
    object!(p => requestor_name)
}

fn friend_packet_fields(p: &FriendPacket) -> Value {
    object!(p => added, object_id, name, online)
}

fn friend_status_fields(p: &FriendStatusPacket) -> Value {
    object!(p => online, name, object_id)
}

fn l2_friend_say_fields(p: &L2FriendSay) -> Value {
    object!(p => sender, receiver, message)
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

fn henna_equip_list_fields(p: &HennaEquipList) -> Value {
    let entries: Vec<Value> = p
        .entries
        .iter()
        .map(|e| object!(e => dye_id, dye_item_id, wear_count, wear_fee, allowed))
        .collect();
    json!({ "adena": p.adena, "total": p.total, "entries": entries })
}

fn henna_remove_list_fields(p: &HennaRemoveList) -> Value {
    let entries: Vec<Value> = p
        .entries
        .iter()
        .map(|e| object!(e => dye_id, dye_item_id, cancel_count, cancel_fee))
        .collect();
    json!({ "adena": p.adena, "count": p.count, "entries": entries })
}

/// `HennaItemInfo` and `HennaItemRemoveInfo`: the six stats as `[current, after]` pairs.
fn henna_preview_fields(p: &HennaPreview) -> Value {
    let stat = |s: &l2_protocol_game::server::henna::HennaStatPreview| json!([s.now, s.after]);
    json!({
        "dye_id": p.dye_id, "dye_item_id": p.dye_item_id, "count": p.count, "fee": p.fee,
        "allowed": p.allowed, "adena": p.adena,
        "int": stat(&p.int), "str": stat(&p.str), "con": stat(&p.con),
        "men": stat(&p.men), "dex": stat(&p.dex), "wit": stat(&p.wit),
    })
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

fn shop_preview_info_fields(p: &ShopPreviewInfo) -> Value {
    json!({ "item_ids": p.item_ids })
}

fn rp_item_link_fields(p: &ExRpItemLink) -> Value {
    json!({ "item": item_fields(&p.item) })
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

/// The first int of a body with several meanings (the Cube game lists and states, the Olympiad
/// result or match list), which picks the packet. `None` when the body is shorter than that.
fn peek_sub(body: &[u8]) -> Option<i32> {
    PacketReader::new(body).i32().ok()
}

fn unknown_sub(sub: Option<i32>) -> Value {
    json!({ "unknown_sub": sub })
}

fn receive_olympiad_fields(body: &[u8]) -> Value {
    let sub = peek_sub(body);
    let mut r = PacketReader::new(body);
    match sub {
        Some(0) => decoded::<ExOlympiadMatchList>(&mut r, |p| {
            json!({
                "games": p.games.iter().map(|g| object!(
                    g => stadium_id, game_kind, state, player_1, player_2
                )).collect::<Vec<_>>(),
            })
        }),
        Some(1) => decoded::<ExOlympiadMatchResult>(&mut r, match_result_fields),
        _ => unknown_sub(sub),
    }
}

fn match_result_fields(p: &ExOlympiadMatchResult) -> Value {
    json!({
        "tie": p.tie,
        "winner_name": p.winner_name,
        "win_team": p.win_team,
        "lose_team": p.lose_team,
        "winners": p.winners.iter().map(|e| object!(
            e => name, clan_name, clan_id, class_id, damage, current_points, diff_points
        )).collect::<Vec<_>>(),
        "losers": p.losers.iter().map(|e| object!(
            e => name, clan_name, clan_id, class_id, damage, current_points, diff_points
        )).collect::<Vec<_>>(),
    })
}

fn block_list_fields(body: &[u8]) -> Value {
    let sub = peek_sub(body);
    let mut r = PacketReader::new(body);
    match sub {
        Some(0) => decoded::<ExCubeGameTeamList>(&mut r, cube_team_list_fields),
        Some(1) => decoded::<ExCubeGameAddPlayer>(&mut r, |p| object!(p => red_team, object_id, name)),
        Some(2) => decoded::<ExCubeGameRemovePlayer>(&mut r, |p| object!(p => red_team, object_id)),
        Some(3) => decoded::<ExCubeGameChangeTimeToStart>(&mut r, |p| object!(p => seconds)),
        Some(4) => decoded::<ExCubeGameRequestReady>(&mut r, no_fields),
        Some(5) => decoded::<ExCubeGameChangeTeam>(&mut r, |p| object!(p => object_id, from_red_team)),
        Some(-1) => decoded::<ExCubeGameCloseUi>(&mut r, no_fields),
        _ => unknown_sub(sub),
    }
}

fn block_state_fields(body: &[u8]) -> Value {
    let sub = peek_sub(body);
    let mut r = PacketReader::new(body);
    match sub {
        Some(0) => decoded::<ExCubeGameExtendedChangePoints>(
            &mut r,
            |p| object!(p => time_left, blue_points, red_points, red_team, object_id, player_points),
        ),
        Some(1) => decoded::<ExCubeGameEnd>(&mut r, |p| object!(p => red_team_win)),
        Some(2) => {
            decoded::<ExCubeGameChangePoints>(&mut r, |p| object!(p => time_left, blue_points, red_points))
        }
        _ => unknown_sub(sub),
    }
}

fn cube_team_list_fields(p: &ExCubeGameTeamList) -> Value {
    json!({
        "room": p.room,
        "blue": p.blue.iter().map(|c| object!(c => object_id, name)).collect::<Vec<_>>(),
        "red": p.red.iter().map(|c| object!(c => object_id, name)).collect::<Vec<_>>(),
    })
}

fn pvp_record_fields(p: &ExPvpMatchRecord) -> Value {
    json!({
        "record_type": p.record_type,
        "winner_team": p.winner_team,
        "blue_kills": p.blue_kills,
        "red_kills": p.red_kills,
        "blue": p.blue.iter().map(|m| object!(m => name, kills, deaths)).collect::<Vec<_>>(),
        "red": p.red.iter().map(|m| object!(m => name, kills, deaths)).collect::<Vec<_>>(),
    })
}

fn score_fields(score: &SsqScore) -> Value {
    object!(score => stone_score, festival_score, total_score, percent)
}

fn ssq_status_fields(p: &SsqStatus) -> Value {
    let body = match &p.body {
        SsqStatusBody::None => json!({}),
        SsqStatusBody::Cycle(c) => json!({
            "cycle": c.cycle,
            "period_message": c.period_message,
            "deadline_message": c.deadline_message,
            "player_cabal": c.player_cabal,
            "player_seal": c.player_seal,
            "stone_contrib": c.stone_contrib,
            "adena_collect": c.adena_collect,
            "dusk": score_fields(&c.dusk),
            "dawn": score_fields(&c.dawn),
        }),
        SsqStatusBody::Festivals(list) => json!(
            list.iter()
                .map(|f| object!(
                    f => level_score, dusk_score, dusk_members, dawn_score, dawn_members
                ))
                .collect::<Vec<_>>()
        ),
        SsqStatusBody::Seals(seals) => json!({
            "retain_limit": seals.retain_limit,
            "claim_limit": seals.claim_limit,
            "rows": seals.rows.iter().map(|r| object!(r => seal, owner, dusk_percent, dawn_percent)).collect::<Vec<_>>(),
        }),
        SsqStatusBody::Predicted(predicted) => json!({
            "winner": predicted.winner,
            "seals": predicted.seals.iter().map(|s| object!(s => seal, owner, message)).collect::<Vec<_>>(),
        }),
    };
    json!({ "page": p.page, "period": p.period, "body": body })
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

fn pc_cafe_point_info_fields(p: &ExPcCafePointInfo) -> Value {
    object!(p => points, add_point, period_type, remain_time, point_type, time)
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

fn ask_join_mpcc_fields(p: &ExAskJoinMpcc) -> Value {
    object!(p => requestor_name)
}

fn gm_view_character_info_fields(p: &GmViewCharacterInfo) -> Value {
    object!(
        p => x, y, z, heading, object_id, name, race, is_female, class_id, level, exp, exp_percent, str, dex, con,
        int, wit, men, max_hp, current_hp, max_mp, current_mp, sp, current_load, max_load, pk_kills,
        paperdoll_object_ids, paperdoll_item_ids, paperdoll_augmentation_ids, talisman_slots, can_equip_cloak,
        p_atk, p_atk_spd, p_def, evasion, accuracy, critical, m_atk, m_atk_spd, m_def, pvp_flag, karma, run_speed,
        walk_speed, swim_run_speed, swim_walk_speed, fly_run_speed, fly_walk_speed, move_multiplier,
        attack_speed_multiplier, collision_radius, collision_height, hair_style, hair_color, face, is_gm, title,
        clan_id, clan_crest_id, ally_id, mount_type, private_store_type, has_dwarven_craft, pvp_kills,
        recom_left, recom_have, max_cp, current_cp, running, pledge_class, noble, hero, name_color, title_color,
        attack_element, attack_element_value, defense, fame, vitality_points
    )
}

fn gm_henna_info_fields(p: &GmHennaInfo) -> Value {
    object!(p => int, str, con, men, dex, wit, dye_ids)
}

fn gm_view_skill_info_fields(p: &GmViewSkillInfo) -> Value {
    let skills: Vec<Value> = p
        .skills
        .iter()
        .map(|s| object!(s => passive, display_level, display_id, disabled, enchantable))
        .collect();
    json!({ "player_name": p.player_name, "skills": skills })
}

fn gm_view_quest_info_fields(p: &GmViewQuestInfo) -> Value {
    let quests: Vec<Value> = p
        .quests
        .iter()
        .map(|(quest_id, cond)| json!({ "quest_id": quest_id, "cond": cond }))
        .collect();
    json!({ "player_name": p.player_name, "quests": quests })
}

fn gm_view_warehouse_fields(p: &GmViewWarehouseWithdrawList) -> Value {
    let items: Vec<Value> = p.items.iter().map(item_fields).collect();
    json!({ "owner_name": p.owner_name, "adena": p.adena, "items": items })
}

fn party_room_info_fields(p: &PartyRoomInfo) -> Value {
    object!(p => room_id, max_members, min_level, max_level, loot_type, location, title)
}

fn list_party_waiting_fields(p: &ListPartyWaiting) -> Value {
    let rooms: Vec<Value> = p
        .rooms
        .iter()
        .map(|room| {
            let members: Vec<Value> = room
                .members
                .iter()
                .map(|(class_id, name)| json!({ "class_id": class_id, "name": name }))
                .collect();
            json!({
                "room_id": room.room_id, "title": room.title, "location": room.location,
                "min_level": room.min_level, "max_level": room.max_level,
                "max_members": room.max_members, "owner_name": room.owner_name, "members": members,
            })
        })
        .collect();
    json!({ "rooms": rooms })
}

fn party_room_member_fields(p: &ExPartyRoomMember) -> Value {
    let members: Vec<Value> = p
        .members
        .iter()
        .map(|m| object!(m => object_id, name, class_id, level, location, role))
        .collect();
    json!({ "mode": p.mode, "members": members })
}

fn manage_party_room_member_fields(p: &ExManagePartyRoomMember) -> Value {
    json!({ "mode": p.mode, "member": object!(p.member => object_id, name, class_id, level, location, role) })
}

fn ask_join_party_room_fields(p: &ExAskJoinPartyRoom) -> Value {
    object!(p => requestor_name)
}

fn list_matching_waiting_room_fields(p: &ExListPartyMatchingWaitingRoom) -> Value {
    let members: Vec<Value> = p
        .members
        .iter()
        .map(|(name, class_id, level)| json!({ "name": name, "class_id": class_id, "level": level }))
        .collect();
    json!({ "page": p.page, "members": members })
}

fn ask_modify_party_looting_fields(p: &ExAskModifyPartyLooting) -> Value {
    json!({ "requestor": p.requestor, "distribution_type": p.distribution_type.id() })
}

fn request_change_nickname_color_fields(p: &ExRequestChangeNicknameColor) -> Value {
    object!(p => item_object_id)
}

fn set_party_looting_fields(p: &ExSetPartyLooting) -> Value {
    json!({ "result": p.result, "distribution_type": p.distribution_type.id() })
}

fn mpcc_party_info_update_fields(p: &ExMpccPartyInfoUpdate) -> Value {
    object!(p => leader_name, leader_object_id, member_count, mode)
}

fn mpcc_show_party_member_info_fields(p: &ExMpccShowPartyMemberInfo) -> Value {
    let members: Vec<Value> = p
        .members
        .iter()
        .map(|m| object!(m => name, object_id, class_id))
        .collect();
    json!({ "members": members })
}

fn multi_party_command_channel_info_fields(p: &ExMultiPartyCommandChannelInfo) -> Value {
    let parties: Vec<Value> = p
        .parties
        .iter()
        .map(|e| object!(e => leader_name, leader_object_id, member_count))
        .collect();
    let mut fields = object!(p => leader_name, channel_loot, member_count);
    fields["parties"] = json!(parties);
    fields
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

fn multi_sell_detail_fields(d: &MultiSellItemDetail) -> Value {
    object!(d => enchant, augment_id, attack_element, attack_element_power, element_defense)
}

fn multi_sell_list_fields(p: &MultiSellList) -> Value {
    let entries: Vec<Value> = p
        .entries
        .iter()
        .map(|e| {
            let products: Vec<Value> = e
                .products
                .iter()
                .map(|i| {
                    let mut v = object!(i => item_id, body_part, type2, count);
                    v["detail"] = multi_sell_detail_fields(&i.detail);
                    v
                })
                .collect();
            let ingredients: Vec<Value> = e
                .ingredients
                .iter()
                .map(|i| {
                    let mut v = object!(i => item_id, type2, count);
                    v["detail"] = multi_sell_detail_fields(&i.detail);
                    v
                })
                .collect();
            json!({
                "entry_id": e.entry_id,
                "stackable": e.stackable,
                "products": products,
                "ingredients": ingredients,
            })
        })
        .collect();
    json!({ "list_id": p.list_id, "page": p.page, "finished": p.finished, "entries": entries })
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

fn put_item_variation_make_fields(p: &ExPutItemResultForVariationMake) -> Value {
    object!(p => item_object_id, item_id)
}

fn put_intensive_variation_make_fields(p: &ExPutIntensiveResultForVariationMake) -> Value {
    object!(p => refiner_item_object_id, life_stone_id, gemstone_item_id, gemstone_count)
}

fn put_commission_variation_make_fields(p: &ExPutCommissionResultForVariationMake) -> Value {
    object!(p => gemstone_object_id, item_id, gemstone_count)
}

fn variation_result_fields(p: &ExVariationResult) -> Value {
    object!(p => stat12, stat34, result)
}

fn put_item_variation_cancel_fields(p: &ExPutItemResultForVariationCancel) -> Value {
    object!(p => item_object_id, item_id, aug1, aug2, price)
}

fn variation_cancel_result_fields(p: &ExVariationCancelResult) -> Value {
    object!(p => result)
}

fn choose_inventory_item_fields(p: &ChooseInventoryItem) -> Value {
    object!(p => item_id)
}

fn enchant_result_fields(p: &EnchantResult) -> Value {
    object!(p => result, crystal, count)
}

fn put_enchant_target_fields(p: &ExPutEnchantTargetItemResult) -> Value {
    object!(p => result)
}

fn put_enchant_support_fields(p: &ExPutEnchantSupportItemResult) -> Value {
    object!(p => result)
}

fn attribute_enchant_result_fields(p: &ExAttributeEnchantResult) -> Value {
    object!(p => result)
}

fn base_attribute_cancel_result_fields(p: &ExBaseAttributeCancelResult) -> Value {
    object!(p => object_id, attribute)
}

fn choose_inventory_attribute_fields(p: &ExChooseInventoryAttributeItem) -> Value {
    object!(p => item_id, elements, max_level)
}

fn base_attribute_cancel_window_fields(p: &ExShowBaseAttributeCancelWindow) -> Value {
    let items: Vec<Value> = p
        .items
        .iter()
        .map(|&(object_id, price)| json!({ "object_id": object_id, "price": price }))
        .collect();
    json!({ "items": items })
}

fn enchant_skill_info_fields(p: &ExEnchantSkillInfo) -> Value {
    object!(p => skill_id, level, max_enchanted, routes)
}

fn enchant_skill_info_detail_fields(p: &ExEnchantSkillInfoDetail) -> Value {
    object!(p => kind, skill_id, skill_level, sp, chance, adena, book_id, book_required)
}

fn enchant_skill_list_fields(p: &ExEnchantSkillList) -> Value {
    let skills: Vec<Value> = p
        .skills
        .iter()
        .map(|&(id, level)| json!({ "id": id, "level": level }))
        .collect();
    json!({ "kind": p.kind, "skills": skills })
}

fn enchant_skill_result_fields(p: &ExEnchantSkillResult) -> Value {
    object!(p => enchanted)
}

fn play_sound_fields(p: &PlaySound) -> Value {
    object!(p => unknown1, sound_file, unknown3, unknown4, unknown5, unknown6, unknown7, unknown8)
}

fn earthquake_fields(p: &Earthquake) -> Value {
    object!(p => x, y, z, intensity, duration)
}

fn ex_red_sky_fields(p: &ExRedSky) -> Value {
    object!(p => duration)
}

fn special_camera_fields(p: &SpecialCamera) -> Value {
    object!(p => id, force, angle1, angle2, time, duration, rel_yaw, rel_pitch, is_wide, rel_angle, unk)
}

fn server_primitive_fields(p: &ExServerPrimitive) -> Value {
    let entry = |e: &l2_protocol_game::server::Primitive| {
        json!({
            "name": e.name,
            "color": e.color.0,
            "name_colored": e.name_colored,
            "x": e.x, "y": e.y, "z": e.z,
            "end": e.end.map(|(x, y, z)| json!({ "x": x, "y": y, "z": z })),
        })
    };
    json!({
        "name": p.name,
        "x": p.x, "y": p.y, "z": p.z,
        "points": p.points().iter().map(entry).collect::<Vec<_>>(),
        "lines": p.lines().iter().map(entry).collect::<Vec<_>>(),
    })
}

fn show_board_fields(p: &ShowBoard) -> Value {
    object!(p => show, content)
}

fn agit_deco_info_fields(p: &AgitDecoInfo) -> Value {
    object!(p => hall_id, fireplace, carpet_statue, chandelier, mirror, crystal, curtain, magic_curtain, flag, front_platform, item_create)
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

fn ride_fields(p: &Ride) -> Value {
    object!(p => object_id, mounted, ride_type, ride_npc_id, x, y, z)
}

fn pet_delete_fields(p: &PetDelete) -> Value {
    object!(p => pet_type, pet_object_id)
}

fn set_summon_remain_time_fields(p: &SetSummonRemainTime) -> Value {
    object!(p => max_time, remaining_time)
}

fn siege_clan_fields(c: &SiegeClan) -> Value {
    object!(c => clan_id, name, leader_name, crest_id, ally_id, ally_name, ally_crest_id)
}

fn siege_info_fields(p: &SiegeInfo) -> Value {
    let time = match &p.time {
        SiegeTime::Date(date) => json!({ "date": date }),
        SiegeTime::Hours(hours) => json!({ "hours": hours }),
    };
    let owner = p
        .owner
        .as_ref()
        .map(|o| object!(o => name, leader_name, ally_id, ally_name));
    let mut fields = object!(p => residence_id, viewer_is_owner_leader, owner_id, now_secs);
    fields["owner"] = json!(owner);
    fields["time"] = time;
    fields
}

fn siege_attacker_list_fields(p: &SiegeAttackerList) -> Value {
    json!({
        "residence_id": p.residence_id,
        "clans": p.clans.iter().map(|c| c.as_ref().map(siege_clan_fields)).collect::<Vec<_>>(),
    })
}

fn siege_defender_list_fields(p: &SiegeDefenderList) -> Value {
    json!({
        "castle_id": p.castle_id,
        "clans": p.clans.iter().map(|d| {
            d.as_ref().map(|d| json!({ "clan": siege_clan_fields(&d.clan), "state": d.state.id() }))
        }).collect::<Vec<_>>(),
    })
}

fn manor_default_fields(p: &ExShowManorDefaultInfo) -> Value {
    json!({
        "hide_buttons": p.hide_buttons,
        "crops": p.crops.iter().map(|c| {
            object!(c => crop_id, level, seed_price, crop_price, reward_1, reward_2)
        }).collect::<Vec<_>>(),
    })
}

fn seed_info_fields(p: &ExShowSeedInfo) -> Value {
    json!({
        "hide_buttons": p.hide_buttons,
        "manor_id": p.manor_id,
        "seeds": p.seeds.iter().map(|s| {
            object!(s => seed_id, left, started, price, level, reward_1, reward_2)
        }).collect::<Vec<_>>(),
    })
}

fn crop_info_fields(p: &ExShowCropInfo) -> Value {
    json!({
        "hide_buttons": p.hide_buttons,
        "manor_id": p.manor_id,
        "crops": p.crops.iter().map(|c| {
            object!(c => crop_id, left, started, price, reward, level, reward_1, reward_2)
        }).collect::<Vec<_>>(),
    })
}

fn seed_setting_fields(p: &ExShowSeedSetting) -> Value {
    json!({
        "manor_id": p.manor_id,
        "seeds": p.seeds.iter().map(|s| json!({
            "seed_id": s.seed_id,
            "level": s.level,
            "reward_1": s.reward_1,
            "reward_2": s.reward_2,
            "sale_limit": s.sale_limit,
            "reference_price": s.reference_price,
            "min_price": s.min_price,
            "max_price": s.max_price,
            "current": object!(s.current => amount, price),
            "next": object!(s.next => amount, price),
        })).collect::<Vec<_>>(),
    })
}

fn crop_setting_fields(p: &ExShowCropSetting) -> Value {
    json!({
        "manor_id": p.manor_id,
        "crops": p.crops.iter().map(|c| json!({
            "crop_id": c.crop_id,
            "level": c.level,
            "reward_1": c.reward_1,
            "reward_2": c.reward_2,
            "sale_limit": c.sale_limit,
            "min_price": c.min_price,
            "max_price": c.max_price,
            "current": object!(c.current => amount, price, reward),
            "next": object!(c.next => amount, price, reward),
        })).collect::<Vec<_>>(),
    })
}

fn sell_crop_list_fields(p: &ExShowSellCropList) -> Value {
    json!({
        "manor_id": p.manor_id,
        "items": p.items.iter().map(|i| json!({
            "object_id": i.object_id,
            "item_id": i.item_id,
            "seed_level": i.seed_level,
            "reward_1": i.reward_1,
            "reward_2": i.reward_2,
            "offer": i.offer.as_ref().map(|o| object!(o => amount, price, reward)),
            "my_crops": i.my_crops,
        })).collect::<Vec<_>>(),
    })
}

fn dominion_registry_fields(p: &ExShowDominionRegistry) -> Value {
    let mut fields = object!(
        p => castle_id, owner_clan_name, owner_leader_name, owner_ally_name, clan_requests,
        merc_requests, war_time, current_time, clan_registered, merc_registered
    );
    fields["territories"] = p
        .territories
        .iter()
        .map(|t| object!(t => territory_id, ward_ids))
        .collect::<Vec<_>>()
        .into();
    fields
}

fn item_auction_fields(p: &ExItemAuctionInfo) -> Value {
    let mut fields = object!(p => refresh, instance_id, bid, time_remaining);
    fields["item"] = item_fields(&p.item);
    fields["next"] = json!(p.next.as_ref().map(|n| {
        let mut next = object!(n => init_bid, start_secs);
        next["item"] = item_fields(&n.item);
        next
    }));
    fields
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

fn show_received_post_list_fields(p: &ExShowReceivedPostList) -> Value {
    json!({
        "time": p.time,
        "mails": p.mails.iter().map(|m| object!(
            m => msg_id, subject, sender_name, locked, expiration, unread, has_attachments, returned,
            send_by_system
        )).collect::<Vec<_>>(),
    })
}

fn show_sent_post_list_fields(p: &ExShowSentPostList) -> Value {
    json!({
        "time": p.time,
        "mails": p.mails.iter().map(|m| object!(
            m => msg_id, subject, receiver_name, locked, expiration, unread, has_attachments
        )).collect::<Vec<_>>(),
    })
}

fn reply_received_post_fields(p: &ExReplyReceivedPost) -> Value {
    json!({
        "msg_id": p.msg_id,
        "locked": p.locked,
        "sender_name": p.sender_name,
        "subject": p.subject,
        "content": p.content,
        "items": p.items.iter().map(item_fields).collect::<Vec<_>>(),
        "req_adena": p.req_adena,
        "has_attachments": p.has_attachments,
        "send_by_system": p.send_by_system,
    })
}

fn reply_sent_post_fields(p: &ExReplySentPost) -> Value {
    json!({
        "msg_id": p.msg_id,
        "locked": p.locked,
        "receiver_name": p.receiver_name,
        "subject": p.subject,
        "content": p.content,
        "items": p.items.iter().map(item_fields).collect::<Vec<_>>(),
        "req_adena": p.req_adena,
        "send_by_system": p.send_by_system,
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
        ServerOpcode::GmViewCharacterInfo,
        ServerOpcode::GmHennaInfo,
        ServerOpcode::GmViewSkillInfo,
        ServerOpcode::GmViewQuestInfo,
        ServerOpcode::GmViewWarehouseWithdrawList,
        ServerOpcode::PartyRoomInfo,
        ServerOpcode::ListPartyWaiting,
        ServerOpcode::ExPartyRoomMember,
        ServerOpcode::ExManagePartyRoomMember,
        ServerOpcode::ExClosePartyRoom,
        ServerOpcode::ExAskJoinPartyRoom,
        ServerOpcode::ExListPartyMatchingWaitingRoom,
        ServerOpcode::ExAskModifyPartyLooting,
        ServerOpcode::ExSetPartyLooting,
        ServerOpcode::ExRequestChangeNicknameColor,
        ServerOpcode::UserInfo,
        ServerOpcode::ShortCutInit,
        ServerOpcode::ShortCutRegister,
        ServerOpcode::SkillList,
        ServerOpcode::SystemMessage,
        ServerOpcode::RestartResponse,
        ServerOpcode::NetPing,
        ServerOpcode::LogOutOk,
        ServerOpcode::L2FriendList,
        ServerOpcode::FriendAddRequest,
        ServerOpcode::L2Friend,
        ServerOpcode::L2FriendStatus,
        ServerOpcode::L2FriendSay,
        ServerOpcode::ValidateLocation,
        ServerOpcode::QuestList,
        ServerOpcode::SkillCoolTime,
        ServerOpcode::HennaInfo,
        ServerOpcode::HennaEquipList,
        ServerOpcode::HennaUnequipList,
        ServerOpcode::HennaItemInfo,
        ServerOpcode::HennaUnequipInfo,
        ServerOpcode::EtcStatusUpdate,
        ServerOpcode::ExStorageMaxCount,
        ServerOpcode::ExBasicActionList,
        ServerOpcode::ExVoteSystemInfo,
        ServerOpcode::ExGetBookmarkInfo,
        ServerOpcode::ExConfirmAddingPostFriend,
        ServerOpcode::FriendList,
        ServerOpcode::Ex2ndPasswordCheck,
        ServerOpcode::Ex2ndPasswordVerify,
        ServerOpcode::Ex2ndPasswordAck,
        ServerOpcode::ExUiSetting,
        ServerOpcode::ShopPreviewInfo,
        ServerOpcode::ExRpItemLink,
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
        ServerOpcode::ExPcCafePointInfo,
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
        ServerOpcode::ExOpenMpcc,
        ServerOpcode::ExCloseMpcc,
        ServerOpcode::ExAskJoinMpcc,
        ServerOpcode::ExMpccPartyInfoUpdate,
        ServerOpcode::ExMpccShowPartyMemberInfo,
        ServerOpcode::ExMultiPartyCommandChannelInfo,
        ServerOpcode::ExBuySellList,
        ServerOpcode::TradeRequest,
        ServerOpcode::TradeStart,
        ServerOpcode::TradeOwnAdd,
        ServerOpcode::TradeOtherAdd,
        ServerOpcode::TradeUpdate,
        ServerOpcode::TradePressOtherOk,
        ServerOpcode::TradeDone,
        ServerOpcode::MultiSellList,
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
        ServerOpcode::Earthquake,
        ServerOpcode::Sunrise,
        ServerOpcode::Sunset,
        ServerOpcode::ExRedSky,
        ServerOpcode::SpecialCamera,
        ServerOpcode::ExServerPrimitive,
        ServerOpcode::ShowBoard,
        ServerOpcode::Snoop,
        ServerOpcode::PetitionVote,
        ServerOpcode::AgitDecoInfo,
        ServerOpcode::ExShowAgitInfo,
        ServerOpcode::ChooseInventoryItem,
        ServerOpcode::EnchantResult,
        ServerOpcode::ExPutEnchantTargetItemResult,
        ServerOpcode::ExPutEnchantSupportItemResult,
        ServerOpcode::ExAttributeEnchantResult,
        ServerOpcode::ExBaseAttributeCancelResult,
        ServerOpcode::ExChooseInventoryAttributeItem,
        ServerOpcode::ExShowVariationMakeWindow,
        ServerOpcode::ExShowVariationCancelWindow,
        ServerOpcode::ExPutItemResultForVariationMake,
        ServerOpcode::ExPutIntensiveResultForVariationMake,
        ServerOpcode::ExPutCommissionResultForVariationMake,
        ServerOpcode::ExVariationResult,
        ServerOpcode::ExPutItemResultForVariationCancel,
        ServerOpcode::ExVariationCancelResult,
        ServerOpcode::ExShowBaseAttributeCancelWindow,
        ServerOpcode::ExEnchantSkillInfo,
        ServerOpcode::ExEnchantSkillInfoDetail,
        ServerOpcode::ExEnchantSkillList,
        ServerOpcode::ExEnchantSkillResult,
        ServerOpcode::ExDuelAskStart,
        ServerOpcode::ExDuelReady,
        ServerOpcode::ExDuelStart,
        ServerOpcode::ExDuelEnd,
        ServerOpcode::ExDuelUpdateUserInfo,
        ServerOpcode::ExFishingStart,
        ServerOpcode::ExFishingStartCombat,
        ServerOpcode::ExFishingHpRegen,
        ServerOpcode::ExFishingEnd,
        ServerOpcode::PetDelete,
        ServerOpcode::Ride,
        ServerOpcode::SetSummonRemainTime,
        ServerOpcode::PetItemList,
        ServerOpcode::PetInventoryUpdate,
        ServerOpcode::WarehouseDepositList,
        ServerOpcode::WarehouseWithdrawList,
        ServerOpcode::PackageToList,
        ServerOpcode::PackageSendableList,
        ServerOpcode::ExNoticePostArrived,
        ServerOpcode::ExNoticePostSent,
        ServerOpcode::ExMailArrived,
        ServerOpcode::CastleSiegeInfo,
        ServerOpcode::CastleSiegeAttackerList,
        ServerOpcode::CastleSiegeDefenderList,
        ServerOpcode::ExShowCastleInfo,
        ServerOpcode::ExShowFortressInfo,
        ServerOpcode::ExShowFortressSiegeInfo,
        ServerOpcode::ExShowFortressMapInfo,
        ServerOpcode::ExShowManorDefaultInfo,
        ServerOpcode::ExShowSeedInfo,
        ServerOpcode::ExShowCropInfo,
        ServerOpcode::ExShowSeedSetting,
        ServerOpcode::ExShowCropSetting,
        ServerOpcode::ExShowProcureCropDetail,
        ServerOpcode::ExShowSellCropList,
        ServerOpcode::BuyListSeed,
        ServerOpcode::ExSendManorList,
        ServerOpcode::ExDominionWarStart,
        ServerOpcode::ExDominionWarEnd,
        ServerOpcode::ExShowDominionRegistry,
        ServerOpcode::ExReplyDominionInfo,
        ServerOpcode::ExShowOwnthingPos,
        ServerOpcode::ExItemAuctionInfo,
        ServerOpcode::ExChangePostState,
        ServerOpcode::ExShowReceivedPostList,
        ServerOpcode::ExShowSentPostList,
        ServerOpcode::ExReplyReceivedPost,
        ServerOpcode::ExReplySentPost,
        ServerOpcode::ExReplyPostItemList,
        ServerOpcode::RecipeBookItemList,
        ServerOpcode::RecipeItemMakeInfo,
        ServerOpcode::RecipeShopItemInfo,
        ServerOpcode::RecipeShopManageList,
        ServerOpcode::RecipeShopMsg,
        ServerOpcode::RecipeShopSellList,
        ServerOpcode::ExOlympiadMatchEnd,
        ServerOpcode::ExOlympiadMode,
        ServerOpcode::ObserverStart,
        ServerOpcode::ObserverEnd,
        ServerOpcode::ExReceiveOlympiad,
        ServerOpcode::ExOlympiadUserInfo,
        ServerOpcode::ExOlympiadSpelledInfo,
        ServerOpcode::ExHeroList,
        ServerOpcode::SsqStatus,
        ServerOpcode::ExShowSeedMapInfo,
        ServerOpcode::VehicleInfo,
        ServerOpcode::VehicleDeparture,
        ServerOpcode::VehicleStart,
        ServerOpcode::VehicleCheckLocation,
        ServerOpcode::GetOnVehicle,
        ServerOpcode::GetOffVehicle,
        ServerOpcode::MoveToLocationInVehicle,
        ServerOpcode::StopMoveInVehicle,
        ServerOpcode::ValidateLocationInVehicle,
        ServerOpcode::ExAirshipInfo,
        ServerOpcode::ExAirshipTeleportList,
        ServerOpcode::ExStopMoveAirship,
        ServerOpcode::ExStopMoveInAirship,
        ServerOpcode::ExValidateLocationInAirship,
        ServerOpcode::ExGetOnAirship,
        ServerOpcode::ExGetOffAirship,
        ServerOpcode::ExMoveToLocationAirship,
        ServerOpcode::ExMoveToLocationInAirship,
        ServerOpcode::ExBlockUpSetList,
        ServerOpcode::ExBlockUpSetState,
        ServerOpcode::ExPvpMatchRecord,
        ServerOpcode::ExPvpMatchUserDie,
        ServerOpcode::ExPvpMatchCcRecord,
        ServerOpcode::ExPvpMatchCcMyRecord,
        ServerOpcode::ExPvpMatchCcRetire,
        ServerOpcode::ExStartScenePlayer,
        ServerOpcode::ExSendUiEvent,
        ServerOpcode::ExGetBossRecord,
        ServerOpcode::ExCursedWeaponList,
        ServerOpcode::ExCursedWeaponLocation,
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

    /// L-02: the macro, crystallize, wear shop and key mapping packets show their fields.
    #[test]
    fn leftover_packets_show_their_fields() {
        use l2_protocol_game::client::{
            MacroCommand, RequestCrystallizeItem, RequestMakeMacro, RequestPreviewItem,
            RequestSaveKeyMapping, StartRotating,
        };
        use l2_protocol_game::server::{ExRpItemLink, ShopPreviewInfo, items::ItemInfo};
        let make = RequestMakeMacro {
            id: 0,
            name: "Heal".into(),
            descr: "d".into(),
            acronym: "HL".into(),
            icon: 3,
            commands: vec![MacroCommand {
                entry: 1,
                kind: 3,
                d1: 0,
                d2: 0,
                command: "/sit".into(),
            }],
        };
        assert_eq!(
            c2s_fields(ConnectionState::InGame, &make.to_bytes()),
            json!({
                "id": 0, "name": "Heal", "descr": "d", "acronym": "HL", "icon": 3,
                "commands": [{ "entry": 1, "kind": 3, "d1": 0, "d2": 0, "command": "/sit" }],
            })
        );
        assert_eq!(
            c2s_fields(
                ConnectionState::InGame,
                &RequestCrystallizeItem {
                    object_id: 5,
                    count: 1
                }
                .to_bytes()
            ),
            json!({ "object_id": 5, "count": 1 })
        );
        assert_eq!(
            c2s_fields(
                ConnectionState::InGame,
                &RequestPreviewItem {
                    unknown: 0,
                    list_id: 100,
                    items: Some(vec![2, 5]),
                }
                .to_bytes()
            ),
            json!({ "unknown": 0, "list_id": 100, "items": [2, 5] })
        );
        assert_eq!(
            c2s_fields(
                ConnectionState::InGame,
                &RequestSaveKeyMapping {
                    ui_key_mapping: Some(vec![1, 255]),
                }
                .to_bytes()
            ),
            json!({ "ui_key_mapping": "01ff" })
        );
        assert_eq!(
            c2s_fields(
                ConnectionState::InGame,
                &StartRotating { degree: 9, side: 1 }.to_bytes()
            ),
            json!({ "degree": 9, "side": 1 })
        );
        let preview = ShopPreviewInfo::from_slots(&[(6, 2376)]);
        assert_eq!(s2c_fields(&preview.to_bytes())["item_ids"][10], json!(2376));
        let link = ExRpItemLink {
            item: ItemInfo::plain(7, 57, 3),
        };
        assert_eq!(s2c_fields(&link.to_bytes())["item"]["count"], json!(3));
    }

    #[test]
    fn agit_deco_info_shows_its_fields() {
        let packet = AgitDecoInfo {
            hall_id: 36,
            fireplace: 2,
            mirror: 1,
            ..AgitDecoInfo::default()
        };
        let fields = s2c_fields(&packet.to_bytes());
        assert_eq!(fields["hall_id"], 36);
        assert_eq!(fields["fireplace"], 2);
        assert_eq!(fields["mirror"], 1);
        assert_eq!(fields["item_create"], 0);
    }

    #[test]
    fn the_clan_hall_list_shows_its_fields() {
        use l2_protocol_game::client::RequestAllAgitInfo;
        use l2_protocol_game::server::residence::AgitInfo;
        assert_eq!(
            c2s_fields(ConnectionState::InGame, &RequestAllAgitInfo.to_bytes()),
            json!({})
        );
        let packet = ExShowAgitInfo {
            halls: vec![
                AgitInfo {
                    hall_id: 22,
                    owner_clan_name: String::new(),
                    leader_name: String::new(),
                    grade_below_one: true,
                },
                AgitInfo {
                    hall_id: 36,
                    owner_clan_name: "Wolves".to_owned(),
                    leader_name: "Fenrir".to_owned(),
                    grade_below_one: false,
                },
            ],
        };
        let fields = s2c_fields(&packet.to_bytes());
        assert_eq!(fields["halls"][0]["hall_id"], 22);
        assert_eq!(fields["halls"][0]["grade_below_one"], true);
        assert_eq!(fields["halls"][1]["owner_clan_name"], "Wolves");
        assert_eq!(fields["halls"][1]["leader_name"], "Fenrir");
    }

    #[test]
    fn multisell_packets_show_their_fields() {
        use l2_protocol_game::client::MultiSellChoose;
        use l2_protocol_game::server::multisell::{
            MultiSellEntry, MultiSellIngredient, MultiSellItemDetail, MultiSellList, MultiSellProduct,
        };
        let choose = MultiSellChoose::new(1235, 300_001, 25);
        assert_eq!(
            c2s_fields(ConnectionState::InGame, &choose.to_bytes()),
            json!({
                "list_id": 1235, "entry_id": 300_001, "amount": 25,
                "unk1": 0, "unk2": 0, "unk3": 0, "elemental": [0, 0, 0, 0, 0, 0, 0, 0],
            })
        );
        let list = MultiSellList {
            list_id: 7,
            page: 1,
            finished: true,
            entries: vec![MultiSellEntry {
                entry_id: 100_000,
                stackable: true,
                products: vec![MultiSellProduct {
                    item_id: 22,
                    body_part: 1,
                    type2: 1,
                    count: 2,
                    detail: MultiSellItemDetail {
                        enchant: 3,
                        ..MultiSellItemDetail::default()
                    },
                }],
                ingredients: vec![MultiSellIngredient {
                    item_id: 57,
                    type2: 4,
                    count: 2100,
                    detail: MultiSellItemDetail::default(),
                }],
            }],
        };
        let fields = s2c_fields(&list.to_bytes());
        assert_eq!(fields["list_id"], 7);
        assert_eq!(fields["finished"], true);
        assert_eq!(fields["entries"][0]["entry_id"], 100_000);
        assert_eq!(fields["entries"][0]["products"][0]["detail"]["enchant"], 3);
        assert_eq!(fields["entries"][0]["products"][0]["count"], 2);
        assert_eq!(fields["entries"][0]["ingredients"][0]["item_id"], 57);
        assert_eq!(fields["entries"][0]["ingredients"][0]["count"], 2100);
    }

    #[test]
    fn enchant_skill_packets_show_their_fields() {
        use l2_protocol_game::client::{RequestExEnchantSkill, RequestExEnchantSkillInfoDetail};
        assert_eq!(
            c2s_fields(
                ConnectionState::InGame,
                &RequestExEnchantSkillInfoDetail {
                    kind: 3,
                    skill_id: 1,
                    skill_level: 205
                }
                .to_bytes()
            ),
            json!({ "kind": 3, "skill_id": 1, "skill_level": 205 })
        );
        assert_eq!(
            c2s_fields(
                ConnectionState::InGame,
                &RequestExEnchantSkill {
                    skill_id: 1,
                    skill_level: 101
                }
                .to_bytes()
            ),
            json!({ "skill_id": 1, "skill_level": 101 })
        );
        let info = ExEnchantSkillInfo {
            skill_id: 1,
            level: 101,
            max_enchanted: false,
            routes: vec![101, 201],
        };
        assert_eq!(
            s2c_fields(&info.to_bytes()),
            json!({ "skill_id": 1, "level": 101, "max_enchanted": false, "routes": [101, 201] })
        );
        let detail = ExEnchantSkillInfoDetail {
            kind: 1,
            skill_id: 1,
            skill_level: 102,
            sp: 5,
            chance: 80,
            adena: 7,
            book_id: 9627,
            book_required: true,
        };
        assert_eq!(s2c_fields(&detail.to_bytes())["book_id"], 9627);
        assert_eq!(
            s2c_fields(&ExEnchantSkillResult { enchanted: true }.to_bytes()),
            json!({ "enchanted": true })
        );
        let list = ExEnchantSkillList {
            kind: 2,
            skills: vec![(1, 101)],
        };
        assert_eq!(
            s2c_fields(&list.to_bytes()),
            json!({ "kind": 2, "skills": [{ "id": 1, "level": 101 }] })
        );
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
    fn party_room_and_gm_window_packets_show_their_fields() {
        use l2_protocol_game::client::{RequestGmCommand, RequestPartyMatchList, RequestResetNickname};
        use l2_protocol_game::server::party_room::{ListPartyWaiting, RoomMember, WaitingRoom};
        use l2_protocol_game::server::{
            ExPartyRoomMember, ExSetPartyLooting, GmViewQuestInfo, PartyDistributionType,
        };

        let fields = s2c_fields(
            &ExPartyRoomMember {
                mode: 1,
                members: vec![RoomMember {
                    object_id: 7,
                    name: "Alpha".into(),
                    class_id: 10,
                    level: 40,
                    location: 6,
                    role: 1,
                }],
            }
            .to_bytes(),
        );
        assert_eq!(fields["mode"], 1);
        assert_eq!(fields["members"][0]["name"], "Alpha");
        assert_eq!(fields["members"][0]["role"], 1);

        let fields = s2c_fields(
            &ListPartyWaiting {
                rooms: vec![WaitingRoom {
                    room_id: 3,
                    title: "Cruma".into(),
                    location: 0,
                    min_level: 1,
                    max_level: 80,
                    max_members: 9,
                    owner_name: "Alpha".into(),
                    members: vec![(10, "Alpha".into())],
                }],
            }
            .to_bytes(),
        );
        assert_eq!(fields["rooms"][0]["title"], "Cruma");
        assert_eq!(fields["rooms"][0]["members"][0]["class_id"], 10);

        let fields = s2c_fields(
            &ExSetPartyLooting {
                result: 1,
                distribution_type: PartyDistributionType::ByTurn,
            }
            .to_bytes(),
        );
        assert_eq!(
            (fields["result"].clone(), fields["distribution_type"].clone()),
            (json!(1), json!(3))
        );

        let fields = s2c_fields(
            &GmViewQuestInfo {
                player_name: "Target".into(),
                quests: vec![],
            }
            .to_bytes(),
        );
        assert_eq!(fields["player_name"], "Target");
        assert_eq!(fields["quests"], json!([]));

        let in_game = ConnectionState::InGame;
        let fields = c2s_fields(
            in_game,
            &RequestPartyMatchList {
                room_id: 0,
                members_max: 9,
                min_level: 1,
                max_level: 80,
                loot: 1,
                room_title: "Cruma".into(),
            }
            .to_bytes(),
        );
        assert_eq!(fields["room_title"], "Cruma");
        let fields = c2s_fields(
            in_game,
            &RequestGmCommand {
                target_name: "Target".into(),
                command: 5,
            }
            .to_bytes(),
        );
        assert_eq!(
            (fields["target_name"].clone(), fields["command"].clone()),
            (json!("Target"), json!(5))
        );
        assert_eq!(c2s_fields(in_game, &RequestResetNickname.to_bytes()), json!({}));
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

        let equip = HennaEquipList {
            adena: 5,
            total: 4,
            entries: vec![l2_protocol_game::server::henna::HennaEquipEntry {
                dye_id: 3,
                dye_item_id: 4445,
                wear_count: 10,
                wear_fee: 37000,
                allowed: true,
            }],
        };
        let fields = s2c_fields(&equip.to_bytes());
        assert_eq!(fields["total"], 4);
        assert_eq!(fields["entries"][0]["wear_fee"], 37000);
        let mut preview = HennaPreview::default();
        preview.int.now = 40;
        preview.int.after = 41;
        let fields = s2c_fields(&HennaItemInfo(preview).to_bytes());
        assert_eq!(fields["int"], json!([40, 41]));
        assert_eq!(
            c2s_fields(
                l2_protocol_game::generated::opcodes::ConnectionState::InGame,
                &l2_protocol_game::client::RequestHennaEquip { symbol_id: 6 }.to_bytes()
            ),
            json!({ "symbol_id": 6 })
        );

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

    /// M7-ADMIN-10: the petition and snoop packets.
    #[test]
    fn petition_and_snoop_packets_show_their_fields() {
        use ConnectionState::InGame;
        use l2_protocol_game::client::{
            RequestPetition, RequestPetitionCancel, RequestPetitionFeedback, SnoopQuit,
        };
        use l2_protocol_game::server::{ChatType, PetitionVotePacket, Snoop};
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestPetition {
                    content: "stuck".into(),
                    kind: 3
                }
                .to_bytes()
            ),
            json!({ "content": "stuck", "kind": 3 })
        );
        assert_eq!(c2s_fields(InGame, &RequestPetitionCancel.to_bytes()), json!({}));
        assert_eq!(
            c2s_fields(
                InGame,
                &RequestPetitionFeedback {
                    unknown: 0,
                    rate: 4,
                    message: "thanks".into()
                }
                .to_bytes()
            ),
            json!({ "unknown": 0, "rate": 4, "message": "thanks" })
        );
        assert_eq!(
            c2s_fields(InGame, &SnoopQuit { snoop_id: 77 }.to_bytes()),
            json!({ "snoop_id": 77 })
        );
        let snoop = Snoop {
            convo_id: 5,
            name: "Target".into(),
            chat_type: ChatType::Whisper,
            speaker: "Friend".into(),
            message: "hi".into(),
        };
        assert_eq!(
            s2c(&snoop.to_bytes()).unwrap(),
            json!({
                "convo_id": 5,
                "name": "Target",
                "chat_type": 2,
                "speaker": "Friend",
                "message": "hi",
            })
        );
        assert_eq!(s2c(&PetitionVotePacket.to_bytes()).unwrap(), json!({}));
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
            s2c_fields(&ExPcCafePointInfo::new(100, -5, 0).to_bytes()),
            json!({
                "points": 100, "add_point": -5, "period_type": 1, "remain_time": 42,
                "point_type": 3, "time": 0
            })
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
    fn m7_packets_show_their_fields() {
        use l2_protocol_game::client::{RequestJoinSiege, RequestSetSeed, SeedSale};
        use l2_protocol_game::server::auction::ExItemAuctionInfo;
        use l2_protocol_game::server::fort::ExShowFortressSiegeInfo;
        use l2_protocol_game::server::siege::{SiegeInfo, SiegeTime};
        use l2_protocol_game::server::territory::ExDominionWarStart;
        let in_game = ConnectionState::InGame;
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestJoinSiege {
                    castle_id: 5,
                    is_attacker: 1,
                    is_joining: 0,
                }
                .to_bytes()
            ),
            json!({ "castle_id": 5, "is_attacker": 1, "is_joining": 0 })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestSetSeed {
                    manor_id: 9,
                    items: vec![SeedSale {
                        item_id: 4,
                        sales: 100,
                        price: 200,
                    }],
                }
                .to_bytes()
            ),
            json!({
                "manor_id": 9,
                "items": [{ "item_id": 4, "sales": 100, "price": 200 }],
            })
        );
        assert_eq!(
            s2c_fields(
                &SiegeInfo {
                    residence_id: 21,
                    viewer_is_owner_leader: true,
                    owner_id: 0,
                    owner: None,
                    now_secs: 1000,
                    time: SiegeTime::Hours(vec![11, 12]),
                }
                .to_bytes()
            ),
            json!({
                "residence_id": 21,
                "viewer_is_owner_leader": true,
                "owner_id": 0,
                "owner": null,
                "now_secs": 1000,
                "time": { "hours": [11, 12] },
            })
        );
        assert_eq!(
            s2c_fields(&ExShowFortressSiegeInfo::new(2, 4, 3, 1).to_bytes()),
            json!({ "fort_id": 2, "barracks_total": 4, "barracks": [2] })
        );
        assert_eq!(
            s2c_fields(
                &ExDominionWarStart {
                    object_id: 1000,
                    territory_id: 85,
                    is_disguised: false,
                }
                .to_bytes()
            ),
            json!({ "object_id": 1000, "territory_id": 85, "is_disguised": false })
        );
        let auction = s2c_fields(
            &ExItemAuctionInfo {
                refresh: false,
                instance_id: 2,
                bid: 500,
                time_remaining: 60,
                item: ItemInfo::plain(268_435_457, 57, 5),
                next: None,
            }
            .to_bytes(),
        );
        assert_eq!(auction["instance_id"], json!(2));
        assert_eq!(auction["bid"], json!(500));
        assert_eq!(auction["refresh"], json!(false));
        assert_eq!(auction["next"], Value::Null);
    }

    #[test]
    fn command_channel_packets_show_their_fields() {
        use l2_protocol_game::client::{
            RequestExAcceptJoinMpcc, RequestExAskJoinMpcc, RequestExMpccShowPartyMembersInfo,
            RequestExOustFromMpcc,
        };
        use l2_protocol_game::server::command_channel::{MpccMember, MpccPartyEntry};

        let in_game = ConnectionState::InGame;
        assert_eq!(
            c2s_fields(in_game, &RequestExAskJoinMpcc { name: "Eve".into() }.to_bytes()),
            json!({ "name": "Eve" })
        );
        assert_eq!(
            c2s_fields(in_game, &RequestExAcceptJoinMpcc { response: 1 }.to_bytes()),
            json!({ "response": 1 })
        );
        assert_eq!(
            c2s_fields(in_game, &RequestExOustFromMpcc { name: "Eve".into() }.to_bytes()),
            json!({ "name": "Eve" })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestExMpccShowPartyMembersInfo { party_leader_id: 7 }.to_bytes()
            ),
            json!({ "party_leader_id": 7 })
        );

        assert_eq!(s2c_fields(&ExOpenMpcc.to_bytes()), json!({}));
        assert_eq!(s2c_fields(&ExCloseMpcc.to_bytes()), json!({}));
        assert_eq!(
            s2c_fields(
                &ExAskJoinMpcc {
                    requestor_name: "Ann".into()
                }
                .to_bytes()
            ),
            json!({ "requestor_name": "Ann" })
        );
        assert_eq!(
            s2c_fields(
                &ExMpccPartyInfoUpdate {
                    leader_name: "Ann".into(),
                    leader_object_id: 5,
                    member_count: 3,
                    mode: 1
                }
                .to_bytes()
            ),
            json!({ "leader_name": "Ann", "leader_object_id": 5, "member_count": 3, "mode": 1 })
        );
        assert_eq!(
            s2c_fields(
                &ExMpccShowPartyMemberInfo {
                    members: vec![MpccMember {
                        name: "Ann".into(),
                        object_id: 5,
                        class_id: 88
                    }]
                }
                .to_bytes()
            ),
            json!({ "members": [{ "name": "Ann", "object_id": 5, "class_id": 88 }] })
        );
        assert_eq!(
            s2c_fields(
                &ExMultiPartyCommandChannelInfo {
                    leader_name: "Ann".into(),
                    channel_loot: 0,
                    member_count: 4,
                    parties: vec![MpccPartyEntry {
                        leader_name: "Ann".into(),
                        leader_object_id: 5,
                        member_count: 4
                    }]
                }
                .to_bytes()
            ),
            json!({
                "leader_name": "Ann",
                "channel_loot": 0,
                "member_count": 4,
                "parties": [{ "leader_name": "Ann", "leader_object_id": 5, "member_count": 4 }]
            })
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
    fn recipe_packets_show_their_fields() {
        use l2_protocol_game::client::{
            RecipeShopListEntry, RequestRecipeBookOpen, RequestRecipeShopListSet, RequestRecipeShopMakeItem,
        };
        use l2_protocol_game::server::recipe::ManufactureEntry;
        let in_game = ConnectionState::InGame;
        assert_eq!(
            c2s_fields(in_game, &RequestRecipeBookOpen { recipe_type: 0 }.to_bytes()),
            json!({ "recipe_type": 0 })
        );
        assert_eq!(
            c2s_fields(
                in_game,
                &RequestRecipeShopMakeItem {
                    id: 4,
                    recipe_id: 9,
                    unknown: 0
                }
                .to_bytes()
            ),
            json!({ "id": 4, "recipe_id": 9, "unknown": 0 })
        );
        let list = RequestRecipeShopListSet {
            items: Some(vec![RecipeShopListEntry {
                recipe_id: 3,
                cost: 50,
            }]),
        };
        assert_eq!(
            c2s_fields(in_game, &list.to_bytes()),
            json!({ "items": [{ "recipe_id": 3, "cost": 50 }] })
        );
        let book = RecipeBookItemList {
            is_common: false,
            max_mp: 70,
            recipes: vec![5, 6],
        };
        assert_eq!(
            s2c_fields(&book.to_bytes()),
            json!({ "is_common": false, "max_mp": 70, "recipes": [5, 6] })
        );
        let sell = RecipeShopSellList {
            manufacturer_id: 1,
            cur_mp: 2,
            max_mp: 3,
            buyer_adena: 4,
            items: vec![ManufactureEntry {
                recipe_id: 7,
                cost: 8,
            }],
        };
        assert_eq!(s2c_fields(&sell.to_bytes())["items"][0]["cost"], 8);
        let msg = RecipeShopMsg {
            object_id: 1,
            message: "Arrows".into(),
        };
        assert_eq!(s2c_fields(&msg.to_bytes())["message"], "Arrows");
    }

    #[test]
    fn fishing_packets_show_their_fields() {
        use l2_protocol_game::client::RequestExFishRanking;
        use l2_protocol_game::server::fishing::{
            ExFishingEnd, ExFishingHpRegen, ExFishingStart, ExFishingStartCombat,
        };
        assert_eq!(
            c2s_fields(ConnectionState::InGame, &RequestExFishRanking.to_bytes()),
            json!({})
        );
        let start = ExFishingStart {
            object_id: 7,
            fish_type: 4,
            x: 1,
            y: 2,
            z: 3,
            night_lure: true,
        };
        assert_eq!(
            s2c_fields(&start.to_bytes()),
            json!({ "object_id": 7, "fish_type": 4, "x": 1, "y": 2, "z": 3, "night_lure": true })
        );
        let combat = ExFishingStartCombat {
            object_id: 7,
            time: 24,
            hp: 100,
            mode: 1,
            lure_type: 2,
            deceptive_mode: 0,
        };
        assert_eq!(s2c_fields(&combat.to_bytes())["lure_type"], 2);
        let regen = ExFishingHpRegen {
            object_id: 7,
            time: 23,
            fish_hp: 90,
            hp_mode: 1,
            good_use: 1,
            anim: 2,
            penalty: 5,
            hp_bar_color: 1,
        };
        assert_eq!(s2c_fields(&regen.to_bytes())["penalty"], 5);
        let end = ExFishingEnd {
            object_id: 7,
            win: true,
        };
        assert_eq!(
            s2c_fields(&end.to_bytes()),
            json!({ "object_id": 7, "win": true })
        );
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
                &Ride {
                    object_id: 9,
                    mounted: true,
                    ride_type: 2,
                    ride_npc_id: 12_621,
                    x: -1,
                    y: 2,
                    z: -3
                }
                .to_bytes()
            ),
            json!({
                "object_id": 9, "mounted": true, "ride_type": 2, "ride_npc_id": 12_621,
                "x": -1, "y": 2, "z": -3
            })
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

    /// L-04: the bookmark packets show their fields, the secondary password and the MAC address
    /// of the hardware report are never shown.
    #[test]
    fn l04_packets_show_their_fields_and_hide_secrets() {
        use l2_protocol_game::client::{
            RequestEx2ndPasswordReq, RequestEx2ndPasswordVerify, RequestSaveBookMarkSlot,
            RequestTeleportBookMark,
        };
        use l2_protocol_game::generated::opcodes::ConnectionState::{Authenticated, InGame};
        use l2_protocol_game::server::{Ex2ndPasswordVerify, ExConfirmAddingContact};
        let save = RequestSaveBookMarkSlot {
            name: "Home".into(),
            icon: 2,
            tag: "t".into(),
        };
        assert_eq!(
            c2s_fields(InGame, &save.to_bytes()),
            json!({ "name": "Home", "icon": 2, "tag": "t" })
        );
        assert_eq!(
            c2s_fields(InGame, &RequestTeleportBookMark { id: 3 }.to_bytes()),
            json!({ "id": 3 })
        );
        let verify = RequestEx2ndPasswordVerify {
            password: "123457".into(),
        };
        let shown = c2s_fields(Authenticated, &verify.to_bytes());
        assert_eq!(shown, json!({ "password": REDACTED }));
        let req = RequestEx2ndPasswordReq {
            change_pass: 2,
            password: "123457".into(),
            new_password: Some("654321".into()),
        };
        let shown = c2s_fields(Authenticated, &req.to_bytes()).to_string();
        assert!(!shown.contains("123457") && !shown.contains("654321"), "{shown}");
        let fields = s2c_fields(
            &Ex2ndPasswordVerify {
                mode: 1,
                wrong_attempts: 2,
            }
            .to_bytes(),
        );
        assert_eq!(fields, json!({ "mode": 1, "wrong_attempts": 2 }));
        let fields = s2c_fields(
            &ExConfirmAddingContact {
                name: "Buddy".into(),
                added: true,
            }
            .to_bytes(),
        );
        assert_eq!(fields, json!({ "name": "Buddy", "added": true }));
    }
}
