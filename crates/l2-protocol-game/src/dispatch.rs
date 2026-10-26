//! Maps `(connection state, opcode)` to a client packet.
//!
//! Java: `gameserver/network/GamePacketHandler.java`. Which packet is accepted in which state
//! comes from the generated tables ([`ClientOpcode::connection_states`] and
//! [`ExClientOpcode::connection_states`]), so this file only lists the packets that already have
//! a struct. Adding a packet = one new [`ClientPacket`] variant + one match arm.

use l2_net::{Decode, PacketError, PacketReader};

use crate::client::MultiSellChoose;
use crate::client::RequestExFishRanking;
use crate::client::{
    Action, Appearing, AttackRequest, AuthLogin, BypassUserCmd, CannotMoveAnymore, CharacterCreate,
    CharacterDelete, CharacterRestore, CharacterSelect, DlgAnswer, EnterWorld, Logout, MoveToLocation,
    NewCharacter, ProtocolVersion, RequestAcquireSkill, RequestAcquireSkillInfo, RequestActionUse,
    RequestAnswerJoinParty, RequestAutoSoulShot, RequestBypassToServer, RequestChangePartyLeader,
    RequestDestroyItem, RequestDispel, RequestDropItem, RequestGotoLobby, RequestItemList, RequestJoinParty,
    RequestKeyMapping, RequestLinkHtml, RequestMagicSkillUse, RequestNetPing, RequestOustPartyMember,
    RequestRestart, RequestRestartPoint, RequestShortCutDel, RequestShortCutReg, RequestSkillCoolTime,
    RequestTargetCanceld, RequestUnEquipItem, RequestVoteNew, RequestWithDrawalParty, Say2,
    SendBypassBuildCmd, UseItem, ValidatePosition,
};
use crate::client::{AddTradeItem, AnswerCoupleAction, AnswerTradeRequest, TradeDone, TradeRequest};
use crate::client::{
    AllyDismiss, AllyLeave, RequestAllyCrest, RequestAllyInfo, RequestAnswerJoinAlly,
    RequestAnswerJoinPledge, RequestDismissAlly, RequestExPledgeCrestLarge, RequestExSetPledgeCrestLarge,
    RequestJoinAlly, RequestJoinPledge, RequestOustPledgeMember, RequestPledgeCrest,
    RequestPledgeExtendedInfo, RequestPledgeInfo, RequestPledgeMemberInfo, RequestPledgeMemberList,
    RequestPledgeMemberPowerInfo, RequestPledgePower, RequestPledgePowerGradeList,
    RequestPledgeReorganizeMember, RequestPledgeSetAcademyMaster, RequestPledgeSetMemberPowerGrade,
    RequestPledgeWarList, RequestReplyStartPledgeWar, RequestReplyStopPledgeWar,
    RequestReplySurrenderPledgeWar, RequestSetAllyCrest, RequestSetPledgeCrest, RequestStartPledgeWar,
    RequestStopPledgeWar, RequestSurrenderPledgeWar, RequestWithdrawalPledge,
};
use crate::client::{
    RequestAllAgitInfo, RequestAllFortressInfo, RequestFortressMapInfo, RequestFortressSiegeInfo,
};
use crate::client::{
    RequestAllCastleInfo, RequestConfirmSiegeWaitingList, RequestJoinSiege, RequestSetCastleSiegeTime,
    RequestSiegeAttackerList, RequestSiegeDefenderList, RequestSiegeInfo,
};
use crate::client::{
    RequestAnswerFriendInvite, RequestBlock, RequestFriendDel, RequestFriendInvite, RequestFriendList,
    RequestSendFriendMsg,
};
use crate::client::{RequestBidItemAuction, RequestInfoItemAuction};
use crate::client::{RequestBuyItem, RequestBuySellUIClose, RequestRefundItem, RequestSellItem};
use crate::client::{
    RequestBuySeed, RequestManorList, RequestProcureCropList, RequestSetCrop, RequestSetSeed,
};
use crate::client::{
    RequestCancelPostAttachment, RequestDeleteReceivedPost, RequestDeleteSentPost, RequestPostAttachment,
    RequestPostItemList, RequestReceivedPost, RequestReceivedPostList, RequestRejectPostAttachment,
    RequestSendPost, RequestSentPost, RequestSentPostList,
};
use crate::client::{
    RequestChangePetName, RequestGetItemFromPet, RequestGiveItemToPet, RequestPetGetItem, RequestPetUseItem,
};
use crate::client::{
    RequestConfirmCancelItem, RequestConfirmGemStone, RequestConfirmRefinerItem, RequestConfirmTargetItem,
    RequestRefine, RequestRefineCancel,
};
use crate::client::{RequestDominionInfo, RequestJoinDominionWar};
use crate::client::{RequestDuelAnswerStart, RequestDuelStart, RequestDuelSurrender};
use crate::client::{
    RequestEnchantItem, RequestExCancelEnchantItem, RequestExEnchantItemAttribute,
    RequestExRemoveItemAttribute, RequestExTryToPutEnchantSupportItem, RequestExTryToPutEnchantTargetItem,
    RequestPackageSend, RequestPackageSendableItemList, SendWareHouseDepositList, SendWareHouseWithDrawList,
};
use crate::client::{
    RequestExAcceptJoinMpcc, RequestExAskJoinMpcc, RequestExMpccShowPartyMembersInfo, RequestExOustFromMpcc,
};
use crate::client::{
    RequestExEnchantSkill, RequestExEnchantSkillInfo, RequestExEnchantSkillInfoDetail,
    RequestExEnchantSkillRouteChange, RequestExEnchantSkillSafe, RequestExEnchantSkillUntrain,
};
use crate::client::{RequestPetition, RequestPetitionCancel, RequestPetitionFeedback, SnoopQuit};
use crate::client::{
    RequestPrivateStoreBuy, RequestPrivateStoreManageBuy, RequestPrivateStoreManageSell,
    RequestPrivateStoreQuitBuy, RequestPrivateStoreQuitSell, RequestPrivateStoreSell, SetPrivateStoreListBuy,
    SetPrivateStoreListSell, SetPrivateStoreMsgBuy, SetPrivateStoreMsgSell, SetPrivateStoreWholeMsg,
};
use crate::client::{
    RequestRecipeBookDestroy, RequestRecipeBookOpen, RequestRecipeItemMakeInfo, RequestRecipeItemMakeSelf,
    RequestRecipeShopListSet, RequestRecipeShopMakeInfo, RequestRecipeShopMakeItem,
    RequestRecipeShopManagePrev, RequestRecipeShopManageQuit, RequestRecipeShopMessageSet,
};
use crate::generated::opcodes::{ClientOpcode, ConnectionState, ExClientOpcode};

/// A decoded client packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientPacket {
    ProtocolVersion(ProtocolVersion),
    AuthLogin(AuthLogin),
    NewCharacter(NewCharacter),
    CharacterCreate(CharacterCreate),
    CharacterDelete(CharacterDelete),
    CharacterRestore(CharacterRestore),
    CharacterSelect(CharacterSelect),
    RequestGotoLobby(RequestGotoLobby),
    EnterWorld(EnterWorld),
    RequestKeyMapping(RequestKeyMapping),
    Say2(Say2),
    Logout(Logout),
    RequestRestart(RequestRestart),
    RequestNetPing(RequestNetPing),
    RequestSkillCoolTime(RequestSkillCoolTime),
    Action(Action),
    RequestTargetCanceld(RequestTargetCanceld),
    RequestBypassToServer(RequestBypassToServer),
    RequestLinkHtml(RequestLinkHtml),
    SendBypassBuildCmd(SendBypassBuildCmd),
    BypassUserCmd(BypassUserCmd),
    MoveToLocation(MoveToLocation),
    ValidatePosition(ValidatePosition),
    CannotMoveAnymore(CannotMoveAnymore),
    Appearing(Appearing),
    /// `ATTACK` (0x01) and `ATTACK_REQUEST` (0x32): Java's `AttackRequest` for both.
    AttackRequest(AttackRequest),
    RequestActionUse(RequestActionUse),
    AnswerCoupleAction(AnswerCoupleAction),
    RequestRestartPoint(RequestRestartPoint),
    UseItem(UseItem),
    RequestUnEquipItem(RequestUnEquipItem),
    RequestDropItem(RequestDropItem),
    RequestDestroyItem(RequestDestroyItem),
    RequestItemList(RequestItemList),
    RequestMagicSkillUse(RequestMagicSkillUse),
    RequestAcquireSkill(RequestAcquireSkill),
    RequestAcquireSkillInfo(RequestAcquireSkillInfo),
    RequestShortCutReg(RequestShortCutReg),
    RequestShortCutDel(RequestShortCutDel),
    DlgAnswer(DlgAnswer),
    /// `EX_REQUEST_AUTO_SOULSHOT` (`0xD0` 0x000D), an ex packet.
    RequestAutoSoulShot(RequestAutoSoulShot),
    /// `EX_REQUEST_DISPEL` (`0xD0` 0x004B), an ex packet.
    RequestDispel(RequestDispel),
    /// `REQUEST_NEW_VOTE_SYSTEM` (`0xD0` 0x007E), an ex packet.
    RequestVoteNew(RequestVoteNew),
    RequestJoinParty(RequestJoinParty),
    RequestAnswerJoinParty(RequestAnswerJoinParty),
    RequestWithDrawalParty(RequestWithDrawalParty),
    RequestOustPartyMember(RequestOustPartyMember),
    /// `REQUEST_FRIEND_INVITE` (0x77).
    RequestFriendInvite(RequestFriendInvite),
    /// `REQUEST_ANSWER_FRIEND_INVITE` (0x78).
    RequestAnswerFriendInvite(RequestAnswerFriendInvite),
    /// `REQUEST_FRIEND_LIST` (0x79).
    RequestFriendList(RequestFriendList),
    /// `REQUEST_FRIEND_DEL` (0x7A).
    RequestFriendDel(RequestFriendDel),
    /// `REQUEST_SEND_FRIEND_MSG` (0x6B).
    RequestSendFriendMsg(RequestSendFriendMsg),
    /// `REQUEST_BLOCK` (0xA9).
    RequestBlock(RequestBlock),
    /// `REQUEST_CHANGE_PARTY_LEADER` (`0xD0` 0x000C), an ex packet.
    RequestChangePartyLeader(RequestChangePartyLeader),
    /// `REQUEST_EX_ASK_JOIN_MPCC` (`0xD0` 0x0006), an ex packet.
    RequestExAskJoinMpcc(RequestExAskJoinMpcc),
    /// `REQUEST_EX_ACCEPT_JOIN_MPCC` (`0xD0` 0x0007), an ex packet.
    RequestExAcceptJoinMpcc(RequestExAcceptJoinMpcc),
    /// `REQUEST_EX_OUST_FROM_MPCC` (`0xD0` 0x0008), an ex packet.
    RequestExOustFromMpcc(RequestExOustFromMpcc),
    /// `REQUEST_EX_MPCC_SHOW_PARTY_MEMBERS_INFO` (`0xD0` 0x002D), an ex packet.
    RequestExMpccShowPartyMembersInfo(RequestExMpccShowPartyMembersInfo),
    RequestBuyItem(RequestBuyItem),
    RequestSellItem(RequestSellItem),
    /// `REQUEST_REFUND_ITEM` (`0xD0` 0x0075), an ex packet.
    RequestRefundItem(RequestRefundItem),
    /// `REQUEST_BUI_SELL_UI_CLOSE` (`0xD0` 0x0076), an ex packet.
    RequestBuySellUIClose(RequestBuySellUIClose),
    TradeRequest(TradeRequest),
    AnswerTradeRequest(AnswerTradeRequest),
    AddTradeItem(AddTradeItem),
    /// The client's `TRADE_DONE` (confirm or cancel the trade window).
    TradeDone(TradeDone),
    /// `MULTI_SELL_CHOOSE`: the player picks an entry of the open multisell list.
    MultiSellChoose(MultiSellChoose),
    RequestPrivateStoreManageSell(RequestPrivateStoreManageSell),
    RequestPrivateStoreManageBuy(RequestPrivateStoreManageBuy),
    RequestPrivateStoreQuitSell(RequestPrivateStoreQuitSell),
    RequestPrivateStoreQuitBuy(RequestPrivateStoreQuitBuy),
    SetPrivateStoreMsgSell(SetPrivateStoreMsgSell),
    SetPrivateStoreMsgBuy(SetPrivateStoreMsgBuy),
    SetPrivateStoreListSell(SetPrivateStoreListSell),
    SetPrivateStoreListBuy(SetPrivateStoreListBuy),
    RequestPrivateStoreBuy(RequestPrivateStoreBuy),
    RequestPrivateStoreSell(RequestPrivateStoreSell),
    /// `SET_PRIVATE_STORE_WHOLE_MSG` (`0xD0` 0x0049), an ex packet.
    SetPrivateStoreWholeMsg(SetPrivateStoreWholeMsg),
    SendWareHouseDepositList(SendWareHouseDepositList),
    SendWareHouseWithDrawList(SendWareHouseWithDrawList),
    RequestPackageSend(RequestPackageSend),
    RequestPackageSendableItemList(RequestPackageSendableItemList),
    // M6-CRAFT-01: 0xB9 and 0xBD have no Java class (see `client/recipe.rs`).
    RequestRecipeBookOpen(RequestRecipeBookOpen),
    RequestRecipeBookDestroy(RequestRecipeBookDestroy),
    RequestRecipeItemMakeInfo(RequestRecipeItemMakeInfo),
    RequestRecipeItemMakeSelf(RequestRecipeItemMakeSelf),
    RequestRecipeShopMessageSet(RequestRecipeShopMessageSet),
    RequestRecipeShopListSet(RequestRecipeShopListSet),
    RequestRecipeShopManageQuit(RequestRecipeShopManageQuit),
    RequestRecipeShopMakeInfo(RequestRecipeShopMakeInfo),
    RequestRecipeShopMakeItem(RequestRecipeShopMakeItem),
    RequestRecipeShopManagePrev(RequestRecipeShopManagePrev),
    RequestJoinPledge(RequestJoinPledge),
    RequestAnswerJoinPledge(RequestAnswerJoinPledge),
    RequestWithdrawalPledge(RequestWithdrawalPledge),
    RequestOustPledgeMember(RequestOustPledgeMember),
    RequestPledgeMemberList(RequestPledgeMemberList),
    RequestPledgeInfo(RequestPledgeInfo),
    RequestPledgeExtendedInfo(RequestPledgeExtendedInfo),
    RequestPledgeCrest(RequestPledgeCrest),
    /// `RequestExPledgeCrestLarge` (`0xD0` ex packet).
    RequestExPledgeCrestLarge(RequestExPledgeCrestLarge),
    /// `RequestPledgePowerGradeList` (`0xD0` ex packet).
    RequestPledgePowerGradeList(RequestPledgePowerGradeList),
    /// `RequestPledgeMemberPowerInfo` (`0xD0` ex packet).
    RequestPledgeMemberPowerInfo(RequestPledgeMemberPowerInfo),
    /// `RequestPledgeMemberInfo` (`0xD0` ex packet).
    RequestPledgeMemberInfo(RequestPledgeMemberInfo),
    /// `RequestPledgeWarList` (`0xD0` ex packet).
    RequestPledgeWarList(RequestPledgeWarList),
    /// `RequestPledgeSetAcademyMaster` (`0xD0` ex packet).
    RequestPledgeSetAcademyMaster(RequestPledgeSetAcademyMaster),
    /// `RequestPledgeSetMemberPowerGrade` (`0xD0` ex packet).
    RequestPledgeSetMemberPowerGrade(RequestPledgeSetMemberPowerGrade),
    /// `RequestPledgeReorganizeMember` (`0xD0` ex packet).
    RequestPledgeReorganizeMember(RequestPledgeReorganizeMember),
    RequestPledgePower(RequestPledgePower),
    RequestSetPledgeCrest(RequestSetPledgeCrest),
    /// `RequestExSetPledgeCrestLarge` (`0xD0` ex packet).
    RequestExSetPledgeCrestLarge(RequestExSetPledgeCrestLarge),
    RequestSetAllyCrest(RequestSetAllyCrest),
    RequestStartPledgeWar(RequestStartPledgeWar),
    RequestReplyStartPledgeWar(RequestReplyStartPledgeWar),
    RequestStopPledgeWar(RequestStopPledgeWar),
    RequestReplyStopPledgeWar(RequestReplyStopPledgeWar),
    RequestSurrenderPledgeWar(RequestSurrenderPledgeWar),
    RequestReplySurrenderPledgeWar(RequestReplySurrenderPledgeWar),
    RequestJoinAlly(RequestJoinAlly),
    RequestAnswerJoinAlly(RequestAnswerJoinAlly),
    AllyLeave(AllyLeave),
    AllyDismiss(AllyDismiss),
    RequestDismissAlly(RequestDismissAlly),
    RequestAllyCrest(RequestAllyCrest),
    RequestAllyInfo(RequestAllyInfo),
    RequestPetGetItem(RequestPetGetItem),
    RequestGetItemFromPet(RequestGetItemFromPet),
    RequestGiveItemToPet(RequestGiveItemToPet),
    RequestPetUseItem(RequestPetUseItem),
    RequestChangePetName(RequestChangePetName),
    /// `REQUEST_PETITION` (0x89).
    RequestPetition(RequestPetition),
    /// `REQUEST_PETITION_CANCEL` (0x8A).
    RequestPetitionCancel(RequestPetitionCancel),
    /// `REQUEST_PETITION_FEEDBACK` (0xC9).
    RequestPetitionFeedback(RequestPetitionFeedback),
    /// `SNOOP_QUIT` (0xB4).
    SnoopQuit(SnoopQuit),
    RequestDuelStart(RequestDuelStart),
    RequestDuelAnswerStart(RequestDuelAnswerStart),
    RequestDuelSurrender(RequestDuelSurrender),
    /// `0xD0` 0x0018, an ex packet.
    RequestExFishRanking(RequestExFishRanking),
    /// The mail packets (`0xD0` 0x0065 to 0x006F), ex packets.
    RequestPostItemList(RequestPostItemList),
    RequestSendPost(RequestSendPost),
    RequestReceivedPostList(RequestReceivedPostList),
    RequestDeleteReceivedPost(RequestDeleteReceivedPost),
    RequestReceivedPost(RequestReceivedPost),
    RequestPostAttachment(RequestPostAttachment),
    RequestRejectPostAttachment(RequestRejectPostAttachment),
    RequestSentPostList(RequestSentPostList),
    RequestDeleteSentPost(RequestDeleteSentPost),
    RequestSentPost(RequestSentPost),
    RequestCancelPostAttachment(RequestCancelPostAttachment),
    RequestEnchantItem(RequestEnchantItem),
    RequestExTryToPutEnchantTargetItem(RequestExTryToPutEnchantTargetItem),
    RequestExTryToPutEnchantSupportItem(RequestExTryToPutEnchantSupportItem),
    RequestExCancelEnchantItem(RequestExCancelEnchantItem),
    RequestExEnchantItemAttribute(RequestExEnchantItemAttribute),
    RequestExRemoveItemAttribute(RequestExRemoveItemAttribute),
    RequestExEnchantSkillInfo(RequestExEnchantSkillInfo),
    RequestExEnchantSkillInfoDetail(RequestExEnchantSkillInfoDetail),
    RequestExEnchantSkill(RequestExEnchantSkill),
    RequestExEnchantSkillSafe(RequestExEnchantSkillSafe),
    RequestExEnchantSkillUntrain(RequestExEnchantSkillUntrain),
    RequestExEnchantSkillRouteChange(RequestExEnchantSkillRouteChange),
    RequestConfirmTargetItem(RequestConfirmTargetItem),
    RequestConfirmRefinerItem(RequestConfirmRefinerItem),
    RequestConfirmGemStone(RequestConfirmGemStone),
    RequestRefine(RequestRefine),
    RequestConfirmCancelItem(RequestConfirmCancelItem),
    RequestRefineCancel(RequestRefineCancel),
    /// `REQUEST_SIEGE_INFO` (0xAA).
    RequestSiegeInfo(RequestSiegeInfo),
    /// `REQUEST_SIEGE_ATTACKER_LIST` (0xAB).
    RequestSiegeAttackerList(RequestSiegeAttackerList),
    /// `REQUEST_SIEGE_DEFENDER_LIST` (0xAC).
    RequestSiegeDefenderList(RequestSiegeDefenderList),
    /// `REQUEST_JOIN_SIEGE` (0xAD).
    RequestJoinSiege(RequestJoinSiege),
    /// `REQUEST_CONFIRM_SIEGE_WAITING_LIST` (0xAE).
    RequestConfirmSiegeWaitingList(RequestConfirmSiegeWaitingList),
    /// `REQUEST_SET_CASTLE_SIEGE_TIME` (0xAF).
    RequestSetCastleSiegeTime(RequestSetCastleSiegeTime),
    /// `REQUEST_BUY_SEED` (0xC5).
    RequestBuySeed(RequestBuySeed),
    /// Java would accept this plain packet in this state, but it has no struct here yet.
    /// The body is left unread.
    Unported(ClientOpcode),
    /// The same for ex packets (`0xD0` + `u16`). The ex id is consumed, the body is left unread.
    UnportedEx(ExClientOpcode),
    /// `REQUEST_ALL_CASTLE_INFO` (`0xD0` 0x003C), an ex packet.
    RequestAllCastleInfo(RequestAllCastleInfo),
    /// `REQUEST_ALL_FORTRESS_INFO` (`0xD0` 0x003D), an ex packet.
    RequestAllFortressInfo(RequestAllFortressInfo),
    RequestAllAgitInfo(RequestAllAgitInfo),
    /// `REQUEST_FORTRESS_SIEGE_INFO` (`0xD0` 0x003F), an ex packet.
    RequestFortressSiegeInfo(RequestFortressSiegeInfo),
    /// `REQUEST_FORTRESS_MAP_INFO` (`0xD0` 0x0048), an ex packet.
    RequestFortressMapInfo(RequestFortressMapInfo),
    /// `REQUEST_MANOR_LIST` (`0xD0` 0x0001), an ex packet.
    RequestManorList(RequestManorList),
    /// `REQUEST_PROCEDURE_CROP_LIST` (`0xD0` 0x0002), an ex packet (Java `RequestProcureCropList`).
    RequestProcureCropList(RequestProcureCropList),
    /// `REQUEST_SET_SEED` (`0xD0` 0x0003), an ex packet.
    RequestSetSeed(RequestSetSeed),
    /// `REQUEST_SET_CROP` (`0xD0` 0x0004), an ex packet.
    RequestSetCrop(RequestSetCrop),
    /// `REQUEST_EX_JOIN_DOMINION_WAR` (`0xD0` 0x0057), an ex packet.
    RequestJoinDominionWar(RequestJoinDominionWar),
    /// `REQUEST_EX_DOMINION_INFO` (`0xD0` 0x0058), an ex packet.
    RequestDominionInfo(RequestDominionInfo),
    /// `REQUEST_BID_ITEM_AUCTION` (`0xD0` 0x0039), an ex packet.
    RequestBidItemAuction(RequestBidItemAuction),
    /// `REQUEST_INFO_ITEM_AUCTION` (`0xD0` 0x003A), an ex packet.
    RequestInfoItemAuction(RequestInfoItemAuction),
}

/// Decodes the body of a client packet whose `opcode` byte was already read from `reader`.
///
/// Returns `Ok(None)` when Java ignores the packet: unknown opcode, or an opcode that is not
/// allowed in `state` (Java checks the state before reading any field). Returns `Err` when the
/// opcode and state are fine but the body is too short. For `0xD0` the dispatcher reads the
/// `u16` ex id itself, as Java does.
pub fn decode_client(
    state: ConnectionState,
    opcode: u8,
    reader: &mut PacketReader,
) -> Result<Option<ClientPacket>, PacketError> {
    let Some(opcode) = ClientOpcode::from_id(opcode) else {
        return Ok(None);
    };
    if !opcode.connection_states().contains(&state) {
        return Ok(None);
    }
    if opcode == ClientOpcode::ExPacket {
        return decode_ex_client(state, reader);
    }
    let packet = match opcode {
        ProtocolVersion::OPCODE => ClientPacket::ProtocolVersion(ProtocolVersion::decode(reader)?),
        AuthLogin::OPCODE => ClientPacket::AuthLogin(AuthLogin::decode(reader)?),
        NewCharacter::OPCODE => ClientPacket::NewCharacter(NewCharacter::decode(reader)?),
        CharacterCreate::OPCODE => ClientPacket::CharacterCreate(CharacterCreate::decode(reader)?),
        CharacterDelete::OPCODE => ClientPacket::CharacterDelete(CharacterDelete::decode(reader)?),
        CharacterRestore::OPCODE => ClientPacket::CharacterRestore(CharacterRestore::decode(reader)?),
        CharacterSelect::OPCODE => ClientPacket::CharacterSelect(CharacterSelect::decode(reader)?),
        EnterWorld::OPCODE => ClientPacket::EnterWorld(EnterWorld::decode(reader)?),
        Say2::OPCODE => ClientPacket::Say2(Say2::decode(reader)?),
        Logout::OPCODE => ClientPacket::Logout(Logout::decode(reader)?),
        RequestRestart::OPCODE => ClientPacket::RequestRestart(RequestRestart::decode(reader)?),
        RequestNetPing::OPCODE => ClientPacket::RequestNetPing(RequestNetPing::decode(reader)?),
        RequestSkillCoolTime::OPCODE => {
            ClientPacket::RequestSkillCoolTime(RequestSkillCoolTime::decode(reader)?)
        }
        Action::OPCODE => ClientPacket::Action(Action::decode(reader)?),
        RequestTargetCanceld::OPCODE => {
            ClientPacket::RequestTargetCanceld(RequestTargetCanceld::decode(reader)?)
        }
        RequestBypassToServer::OPCODE => {
            ClientPacket::RequestBypassToServer(RequestBypassToServer::decode(reader)?)
        }
        RequestLinkHtml::OPCODE => ClientPacket::RequestLinkHtml(RequestLinkHtml::decode(reader)?),
        SendBypassBuildCmd::OPCODE => ClientPacket::SendBypassBuildCmd(SendBypassBuildCmd::decode(reader)?),
        BypassUserCmd::OPCODE => ClientPacket::BypassUserCmd(BypassUserCmd::decode(reader)?),
        MoveToLocation::OPCODE => ClientPacket::MoveToLocation(MoveToLocation::decode(reader)?),
        ValidatePosition::OPCODE => ClientPacket::ValidatePosition(ValidatePosition::decode(reader)?),
        CannotMoveAnymore::OPCODE => ClientPacket::CannotMoveAnymore(CannotMoveAnymore::decode(reader)?),
        Appearing::OPCODE => ClientPacket::Appearing(Appearing::decode(reader)?),
        AttackRequest::OPCODE | AttackRequest::OPCODE_REQUEST => {
            ClientPacket::AttackRequest(AttackRequest::decode(reader)?)
        }
        RequestActionUse::OPCODE => ClientPacket::RequestActionUse(RequestActionUse::decode(reader)?),
        RequestRestartPoint::OPCODE => {
            ClientPacket::RequestRestartPoint(RequestRestartPoint::decode(reader)?)
        }
        UseItem::OPCODE => ClientPacket::UseItem(UseItem::decode(reader)?),
        RequestUnEquipItem::OPCODE => ClientPacket::RequestUnEquipItem(RequestUnEquipItem::decode(reader)?),
        RequestDropItem::OPCODE => ClientPacket::RequestDropItem(RequestDropItem::decode(reader)?),
        RequestDestroyItem::OPCODE => ClientPacket::RequestDestroyItem(RequestDestroyItem::decode(reader)?),
        RequestItemList::OPCODE => ClientPacket::RequestItemList(RequestItemList::decode(reader)?),
        RequestMagicSkillUse::OPCODE => {
            ClientPacket::RequestMagicSkillUse(RequestMagicSkillUse::decode(reader)?)
        }
        RequestAcquireSkill::OPCODE => {
            ClientPacket::RequestAcquireSkill(RequestAcquireSkill::decode(reader)?)
        }
        RequestAcquireSkillInfo::OPCODE => {
            ClientPacket::RequestAcquireSkillInfo(RequestAcquireSkillInfo::decode(reader)?)
        }
        RequestShortCutReg::OPCODE => ClientPacket::RequestShortCutReg(RequestShortCutReg::decode(reader)?),
        RequestShortCutDel::OPCODE => ClientPacket::RequestShortCutDel(RequestShortCutDel::decode(reader)?),
        DlgAnswer::OPCODE => ClientPacket::DlgAnswer(DlgAnswer::decode(reader)?),
        RequestJoinParty::OPCODE => ClientPacket::RequestJoinParty(RequestJoinParty::decode(reader)?),
        RequestAnswerJoinParty::OPCODE => {
            ClientPacket::RequestAnswerJoinParty(RequestAnswerJoinParty::decode(reader)?)
        }
        RequestWithDrawalParty::OPCODE => {
            ClientPacket::RequestWithDrawalParty(RequestWithDrawalParty::decode(reader)?)
        }
        RequestOustPartyMember::OPCODE => {
            ClientPacket::RequestOustPartyMember(RequestOustPartyMember::decode(reader)?)
        }
        RequestFriendInvite::OPCODE => {
            ClientPacket::RequestFriendInvite(RequestFriendInvite::decode(reader)?)
        }
        RequestAnswerFriendInvite::OPCODE => {
            ClientPacket::RequestAnswerFriendInvite(RequestAnswerFriendInvite::decode(reader)?)
        }
        RequestFriendList::OPCODE => ClientPacket::RequestFriendList(RequestFriendList::decode(reader)?),
        RequestFriendDel::OPCODE => ClientPacket::RequestFriendDel(RequestFriendDel::decode(reader)?),
        RequestSendFriendMsg::OPCODE => {
            ClientPacket::RequestSendFriendMsg(RequestSendFriendMsg::decode(reader)?)
        }
        RequestBlock::OPCODE => ClientPacket::RequestBlock(RequestBlock::decode(reader)?),
        RequestPetition::OPCODE => ClientPacket::RequestPetition(RequestPetition::decode(reader)?),
        RequestPetitionCancel::OPCODE => {
            ClientPacket::RequestPetitionCancel(RequestPetitionCancel::decode(reader)?)
        }
        RequestPetitionFeedback::OPCODE => {
            ClientPacket::RequestPetitionFeedback(RequestPetitionFeedback::decode(reader)?)
        }
        SnoopQuit::OPCODE => ClientPacket::SnoopQuit(SnoopQuit::decode(reader)?),
        RequestBuyItem::OPCODE => ClientPacket::RequestBuyItem(RequestBuyItem::decode(reader)?),
        RequestSellItem::OPCODE => ClientPacket::RequestSellItem(RequestSellItem::decode(reader)?),
        TradeRequest::OPCODE => ClientPacket::TradeRequest(TradeRequest::decode(reader)?),
        AnswerTradeRequest::OPCODE => ClientPacket::AnswerTradeRequest(AnswerTradeRequest::decode(reader)?),
        AddTradeItem::OPCODE => ClientPacket::AddTradeItem(AddTradeItem::decode(reader)?),
        TradeDone::OPCODE => ClientPacket::TradeDone(TradeDone::decode(reader)?),
        MultiSellChoose::OPCODE => ClientPacket::MultiSellChoose(MultiSellChoose::decode(reader)?),
        RequestPrivateStoreManageSell::OPCODE => {
            ClientPacket::RequestPrivateStoreManageSell(RequestPrivateStoreManageSell::decode(reader)?)
        }
        RequestPrivateStoreManageBuy::OPCODE => {
            ClientPacket::RequestPrivateStoreManageBuy(RequestPrivateStoreManageBuy::decode(reader)?)
        }
        RequestPrivateStoreQuitSell::OPCODE => {
            ClientPacket::RequestPrivateStoreQuitSell(RequestPrivateStoreQuitSell::decode(reader)?)
        }
        RequestPrivateStoreQuitBuy::OPCODE => {
            ClientPacket::RequestPrivateStoreQuitBuy(RequestPrivateStoreQuitBuy::decode(reader)?)
        }
        SetPrivateStoreMsgSell::OPCODE => {
            ClientPacket::SetPrivateStoreMsgSell(SetPrivateStoreMsgSell::decode(reader)?)
        }
        SetPrivateStoreMsgBuy::OPCODE => {
            ClientPacket::SetPrivateStoreMsgBuy(SetPrivateStoreMsgBuy::decode(reader)?)
        }
        SetPrivateStoreListSell::OPCODE => {
            ClientPacket::SetPrivateStoreListSell(SetPrivateStoreListSell::decode(reader)?)
        }
        SetPrivateStoreListBuy::OPCODE => {
            ClientPacket::SetPrivateStoreListBuy(SetPrivateStoreListBuy::decode(reader)?)
        }
        RequestPrivateStoreBuy::OPCODE => {
            ClientPacket::RequestPrivateStoreBuy(RequestPrivateStoreBuy::decode(reader)?)
        }
        RequestPrivateStoreSell::OPCODE => {
            ClientPacket::RequestPrivateStoreSell(RequestPrivateStoreSell::decode(reader)?)
        }
        SendWareHouseDepositList::OPCODE => {
            ClientPacket::SendWareHouseDepositList(SendWareHouseDepositList::decode(reader)?)
        }
        SendWareHouseWithDrawList::OPCODE => {
            ClientPacket::SendWareHouseWithDrawList(SendWareHouseWithDrawList::decode(reader)?)
        }
        RequestPackageSend::OPCODE => ClientPacket::RequestPackageSend(RequestPackageSend::decode(reader)?),
        RequestPackageSendableItemList::OPCODE => {
            ClientPacket::RequestPackageSendableItemList(RequestPackageSendableItemList::decode(reader)?)
        }
        RequestRecipeBookOpen::OPCODE => {
            ClientPacket::RequestRecipeBookOpen(RequestRecipeBookOpen::decode(reader)?)
        }
        RequestRecipeBookDestroy::OPCODE => {
            ClientPacket::RequestRecipeBookDestroy(RequestRecipeBookDestroy::decode(reader)?)
        }
        RequestRecipeItemMakeInfo::OPCODE => {
            ClientPacket::RequestRecipeItemMakeInfo(RequestRecipeItemMakeInfo::decode(reader)?)
        }
        RequestRecipeItemMakeSelf::OPCODE => {
            ClientPacket::RequestRecipeItemMakeSelf(RequestRecipeItemMakeSelf::decode(reader)?)
        }
        RequestRecipeShopMessageSet::OPCODE => {
            ClientPacket::RequestRecipeShopMessageSet(RequestRecipeShopMessageSet::decode(reader)?)
        }
        RequestRecipeShopListSet::OPCODE => {
            ClientPacket::RequestRecipeShopListSet(RequestRecipeShopListSet::decode(reader)?)
        }
        RequestRecipeShopManageQuit::OPCODE => {
            ClientPacket::RequestRecipeShopManageQuit(RequestRecipeShopManageQuit::decode(reader)?)
        }
        RequestRecipeShopMakeInfo::OPCODE => {
            ClientPacket::RequestRecipeShopMakeInfo(RequestRecipeShopMakeInfo::decode(reader)?)
        }
        RequestRecipeShopMakeItem::OPCODE => {
            ClientPacket::RequestRecipeShopMakeItem(RequestRecipeShopMakeItem::decode(reader)?)
        }
        RequestRecipeShopManagePrev::OPCODE => {
            ClientPacket::RequestRecipeShopManagePrev(RequestRecipeShopManagePrev::decode(reader)?)
        }
        RequestJoinPledge::OPCODE => ClientPacket::RequestJoinPledge(RequestJoinPledge::decode(reader)?),
        RequestAnswerJoinPledge::OPCODE => {
            ClientPacket::RequestAnswerJoinPledge(RequestAnswerJoinPledge::decode(reader)?)
        }
        RequestWithdrawalPledge::OPCODE => {
            ClientPacket::RequestWithdrawalPledge(RequestWithdrawalPledge::decode(reader)?)
        }
        RequestOustPledgeMember::OPCODE => {
            ClientPacket::RequestOustPledgeMember(RequestOustPledgeMember::decode(reader)?)
        }
        RequestPledgeMemberList::OPCODE => {
            ClientPacket::RequestPledgeMemberList(RequestPledgeMemberList::decode(reader)?)
        }
        RequestPledgeInfo::OPCODE => ClientPacket::RequestPledgeInfo(RequestPledgeInfo::decode(reader)?),
        RequestPledgeExtendedInfo::OPCODE => {
            ClientPacket::RequestPledgeExtendedInfo(RequestPledgeExtendedInfo::decode(reader)?)
        }
        RequestPledgeCrest::OPCODE => ClientPacket::RequestPledgeCrest(RequestPledgeCrest::decode(reader)?),
        RequestPledgePower::OPCODE => ClientPacket::RequestPledgePower(RequestPledgePower::decode(reader)?),
        RequestSetPledgeCrest::OPCODE => {
            ClientPacket::RequestSetPledgeCrest(RequestSetPledgeCrest::decode(reader)?)
        }
        RequestSetAllyCrest::OPCODE => {
            ClientPacket::RequestSetAllyCrest(RequestSetAllyCrest::decode(reader)?)
        }
        RequestStartPledgeWar::OPCODE => {
            ClientPacket::RequestStartPledgeWar(RequestStartPledgeWar::decode(reader)?)
        }
        RequestReplyStartPledgeWar::OPCODE => {
            ClientPacket::RequestReplyStartPledgeWar(RequestReplyStartPledgeWar::decode(reader)?)
        }
        RequestStopPledgeWar::OPCODE => {
            ClientPacket::RequestStopPledgeWar(RequestStopPledgeWar::decode(reader)?)
        }
        RequestReplyStopPledgeWar::OPCODE => {
            ClientPacket::RequestReplyStopPledgeWar(RequestReplyStopPledgeWar::decode(reader)?)
        }
        RequestSurrenderPledgeWar::OPCODE => {
            ClientPacket::RequestSurrenderPledgeWar(RequestSurrenderPledgeWar::decode(reader)?)
        }
        RequestReplySurrenderPledgeWar::OPCODE => {
            ClientPacket::RequestReplySurrenderPledgeWar(RequestReplySurrenderPledgeWar::decode(reader)?)
        }
        RequestJoinAlly::OPCODE => ClientPacket::RequestJoinAlly(RequestJoinAlly::decode(reader)?),
        RequestAnswerJoinAlly::OPCODE => {
            ClientPacket::RequestAnswerJoinAlly(RequestAnswerJoinAlly::decode(reader)?)
        }
        AllyLeave::OPCODE => ClientPacket::AllyLeave(AllyLeave::decode(reader)?),
        AllyDismiss::OPCODE => ClientPacket::AllyDismiss(AllyDismiss::decode(reader)?),
        RequestDismissAlly::OPCODE => ClientPacket::RequestDismissAlly(RequestDismissAlly::decode(reader)?),
        RequestAllyCrest::OPCODE => ClientPacket::RequestAllyCrest(RequestAllyCrest::decode(reader)?),
        RequestAllyInfo::OPCODE => ClientPacket::RequestAllyInfo(RequestAllyInfo::decode(reader)?),
        RequestPetGetItem::OPCODE => ClientPacket::RequestPetGetItem(RequestPetGetItem::decode(reader)?),
        RequestGetItemFromPet::OPCODE => {
            ClientPacket::RequestGetItemFromPet(RequestGetItemFromPet::decode(reader)?)
        }
        RequestGiveItemToPet::OPCODE => {
            ClientPacket::RequestGiveItemToPet(RequestGiveItemToPet::decode(reader)?)
        }
        RequestPetUseItem::OPCODE => ClientPacket::RequestPetUseItem(RequestPetUseItem::decode(reader)?),
        RequestChangePetName::OPCODE => {
            ClientPacket::RequestChangePetName(RequestChangePetName::decode(reader)?)
        }
        RequestEnchantItem::OPCODE => ClientPacket::RequestEnchantItem(RequestEnchantItem::decode(reader)?),
        RequestSiegeInfo::OPCODE => ClientPacket::RequestSiegeInfo(RequestSiegeInfo::decode(reader)?),
        RequestSiegeAttackerList::OPCODE => {
            ClientPacket::RequestSiegeAttackerList(RequestSiegeAttackerList::decode(reader)?)
        }
        RequestSiegeDefenderList::OPCODE => {
            ClientPacket::RequestSiegeDefenderList(RequestSiegeDefenderList::decode(reader)?)
        }
        RequestJoinSiege::OPCODE => ClientPacket::RequestJoinSiege(RequestJoinSiege::decode(reader)?),
        RequestConfirmSiegeWaitingList::OPCODE => {
            ClientPacket::RequestConfirmSiegeWaitingList(RequestConfirmSiegeWaitingList::decode(reader)?)
        }
        RequestSetCastleSiegeTime::OPCODE => {
            ClientPacket::RequestSetCastleSiegeTime(RequestSetCastleSiegeTime::decode(reader)?)
        }
        RequestBuySeed::OPCODE => ClientPacket::RequestBuySeed(RequestBuySeed::decode(reader)?),
        other => ClientPacket::Unported(other),
    };
    Ok(Some(packet))
}

/// The part after `0xD0`: the `u16` ex id and the body.
fn decode_ex_client(
    state: ConnectionState,
    reader: &mut PacketReader,
) -> Result<Option<ClientPacket>, PacketError> {
    let Some(ex) = ExClientOpcode::from_id(reader.u16()?) else {
        return Ok(None);
    };
    if !ex.connection_states().contains(&state) {
        return Ok(None);
    }
    // TODO(M2-GPROTO-*): Java reads a `u32` sub id right after the ex id of 0x51 (bookmarks) to
    // pick the packet. Whoever ports the bookmark packets handles it here.
    let packet = match ex {
        RequestGotoLobby::EX_OPCODE => ClientPacket::RequestGotoLobby(RequestGotoLobby::decode(reader)?),
        RequestKeyMapping::EX_OPCODE => ClientPacket::RequestKeyMapping(RequestKeyMapping::decode(reader)?),
        RequestAutoSoulShot::EX_OPCODE => {
            ClientPacket::RequestAutoSoulShot(RequestAutoSoulShot::decode(reader)?)
        }
        RequestDispel::EX_OPCODE => ClientPacket::RequestDispel(RequestDispel::decode(reader)?),
        RequestVoteNew::EX_OPCODE => ClientPacket::RequestVoteNew(RequestVoteNew::decode(reader)?),
        AnswerCoupleAction::EX_OPCODE => {
            ClientPacket::AnswerCoupleAction(AnswerCoupleAction::decode(reader)?)
        }
        RequestChangePartyLeader::EX_OPCODE => {
            ClientPacket::RequestChangePartyLeader(RequestChangePartyLeader::decode(reader)?)
        }
        RequestExAskJoinMpcc::EX_OPCODE => {
            ClientPacket::RequestExAskJoinMpcc(RequestExAskJoinMpcc::decode(reader)?)
        }
        RequestExAcceptJoinMpcc::EX_OPCODE => {
            ClientPacket::RequestExAcceptJoinMpcc(RequestExAcceptJoinMpcc::decode(reader)?)
        }
        RequestExOustFromMpcc::EX_OPCODE => {
            ClientPacket::RequestExOustFromMpcc(RequestExOustFromMpcc::decode(reader)?)
        }
        RequestExMpccShowPartyMembersInfo::EX_OPCODE => ClientPacket::RequestExMpccShowPartyMembersInfo(
            RequestExMpccShowPartyMembersInfo::decode(reader)?,
        ),
        RequestRefundItem::EX_OPCODE => ClientPacket::RequestRefundItem(RequestRefundItem::decode(reader)?),
        RequestBuySellUIClose::EX_OPCODE => {
            ClientPacket::RequestBuySellUIClose(RequestBuySellUIClose::decode(reader)?)
        }
        SetPrivateStoreWholeMsg::EX_OPCODE => {
            ClientPacket::SetPrivateStoreWholeMsg(SetPrivateStoreWholeMsg::decode(reader)?)
        }
        RequestExPledgeCrestLarge::EX_OPCODE => {
            ClientPacket::RequestExPledgeCrestLarge(RequestExPledgeCrestLarge::decode(reader)?)
        }
        RequestPledgePowerGradeList::EX_OPCODE => {
            ClientPacket::RequestPledgePowerGradeList(RequestPledgePowerGradeList::decode(reader)?)
        }
        RequestPledgeMemberPowerInfo::EX_OPCODE => {
            ClientPacket::RequestPledgeMemberPowerInfo(RequestPledgeMemberPowerInfo::decode(reader)?)
        }
        RequestPledgeMemberInfo::EX_OPCODE => {
            ClientPacket::RequestPledgeMemberInfo(RequestPledgeMemberInfo::decode(reader)?)
        }
        RequestPledgeWarList::EX_OPCODE => {
            ClientPacket::RequestPledgeWarList(RequestPledgeWarList::decode(reader)?)
        }
        RequestPledgeSetAcademyMaster::EX_OPCODE => {
            ClientPacket::RequestPledgeSetAcademyMaster(RequestPledgeSetAcademyMaster::decode(reader)?)
        }
        RequestPledgeSetMemberPowerGrade::EX_OPCODE => {
            ClientPacket::RequestPledgeSetMemberPowerGrade(RequestPledgeSetMemberPowerGrade::decode(reader)?)
        }
        RequestPledgeReorganizeMember::EX_OPCODE => {
            ClientPacket::RequestPledgeReorganizeMember(RequestPledgeReorganizeMember::decode(reader)?)
        }
        RequestExSetPledgeCrestLarge::EX_OPCODE => {
            ClientPacket::RequestExSetPledgeCrestLarge(RequestExSetPledgeCrestLarge::decode(reader)?)
        }
        RequestDuelStart::EX_OPCODE => ClientPacket::RequestDuelStart(RequestDuelStart::decode(reader)?),
        RequestDuelAnswerStart::EX_OPCODE => {
            ClientPacket::RequestDuelAnswerStart(RequestDuelAnswerStart::decode(reader)?)
        }
        RequestDuelSurrender::EX_OPCODE => {
            ClientPacket::RequestDuelSurrender(RequestDuelSurrender::decode(reader)?)
        }
        RequestExFishRanking::EX_OPCODE => {
            ClientPacket::RequestExFishRanking(RequestExFishRanking::decode(reader)?)
        }
        RequestPostItemList::EX_OPCODE => {
            ClientPacket::RequestPostItemList(RequestPostItemList::decode(reader)?)
        }
        RequestSendPost::EX_OPCODE => ClientPacket::RequestSendPost(RequestSendPost::decode(reader)?),
        RequestReceivedPostList::EX_OPCODE => {
            ClientPacket::RequestReceivedPostList(RequestReceivedPostList::decode(reader)?)
        }
        RequestDeleteReceivedPost::EX_OPCODE => {
            ClientPacket::RequestDeleteReceivedPost(RequestDeleteReceivedPost::decode(reader)?)
        }
        RequestReceivedPost::EX_OPCODE => {
            ClientPacket::RequestReceivedPost(RequestReceivedPost::decode(reader)?)
        }
        RequestPostAttachment::EX_OPCODE => {
            ClientPacket::RequestPostAttachment(RequestPostAttachment::decode(reader)?)
        }
        RequestRejectPostAttachment::EX_OPCODE => {
            ClientPacket::RequestRejectPostAttachment(RequestRejectPostAttachment::decode(reader)?)
        }
        RequestSentPostList::EX_OPCODE => {
            ClientPacket::RequestSentPostList(RequestSentPostList::decode(reader)?)
        }
        RequestDeleteSentPost::EX_OPCODE => {
            ClientPacket::RequestDeleteSentPost(RequestDeleteSentPost::decode(reader)?)
        }
        RequestSentPost::EX_OPCODE => ClientPacket::RequestSentPost(RequestSentPost::decode(reader)?),
        RequestCancelPostAttachment::EX_OPCODE => {
            ClientPacket::RequestCancelPostAttachment(RequestCancelPostAttachment::decode(reader)?)
        }
        RequestExTryToPutEnchantTargetItem::EX_OPCODE => ClientPacket::RequestExTryToPutEnchantTargetItem(
            RequestExTryToPutEnchantTargetItem::decode(reader)?,
        ),
        RequestExTryToPutEnchantSupportItem::EX_OPCODE => ClientPacket::RequestExTryToPutEnchantSupportItem(
            RequestExTryToPutEnchantSupportItem::decode(reader)?,
        ),
        RequestExCancelEnchantItem::EX_OPCODE => {
            ClientPacket::RequestExCancelEnchantItem(RequestExCancelEnchantItem::decode(reader)?)
        }
        RequestExEnchantItemAttribute::EX_OPCODE => {
            ClientPacket::RequestExEnchantItemAttribute(RequestExEnchantItemAttribute::decode(reader)?)
        }
        RequestExRemoveItemAttribute::EX_OPCODE => {
            ClientPacket::RequestExRemoveItemAttribute(RequestExRemoveItemAttribute::decode(reader)?)
        }
        RequestExEnchantSkillInfo::EX_OPCODE => {
            ClientPacket::RequestExEnchantSkillInfo(RequestExEnchantSkillInfo::decode(reader)?)
        }
        RequestExEnchantSkillInfoDetail::EX_OPCODE => {
            ClientPacket::RequestExEnchantSkillInfoDetail(RequestExEnchantSkillInfoDetail::decode(reader)?)
        }
        RequestExEnchantSkill::EX_OPCODE => {
            ClientPacket::RequestExEnchantSkill(RequestExEnchantSkill::decode(reader)?)
        }
        RequestExEnchantSkillSafe::EX_OPCODE => {
            ClientPacket::RequestExEnchantSkillSafe(RequestExEnchantSkillSafe::decode(reader)?)
        }
        RequestExEnchantSkillUntrain::EX_OPCODE => {
            ClientPacket::RequestExEnchantSkillUntrain(RequestExEnchantSkillUntrain::decode(reader)?)
        }
        RequestExEnchantSkillRouteChange::EX_OPCODE => {
            ClientPacket::RequestExEnchantSkillRouteChange(RequestExEnchantSkillRouteChange::decode(reader)?)
        }
        RequestConfirmTargetItem::EX_OPCODE => {
            ClientPacket::RequestConfirmTargetItem(RequestConfirmTargetItem::decode(reader)?)
        }
        RequestConfirmRefinerItem::EX_OPCODE => {
            ClientPacket::RequestConfirmRefinerItem(RequestConfirmRefinerItem::decode(reader)?)
        }
        RequestConfirmGemStone::EX_OPCODE => {
            ClientPacket::RequestConfirmGemStone(RequestConfirmGemStone::decode(reader)?)
        }
        RequestRefine::EX_OPCODE => ClientPacket::RequestRefine(RequestRefine::decode(reader)?),
        RequestConfirmCancelItem::EX_OPCODE => {
            ClientPacket::RequestConfirmCancelItem(RequestConfirmCancelItem::decode(reader)?)
        }
        RequestRefineCancel::EX_OPCODE => {
            ClientPacket::RequestRefineCancel(RequestRefineCancel::decode(reader)?)
        }
        RequestAllCastleInfo::EX_OPCODE => {
            ClientPacket::RequestAllCastleInfo(RequestAllCastleInfo::decode(reader)?)
        }
        RequestAllFortressInfo::EX_OPCODE => {
            ClientPacket::RequestAllFortressInfo(RequestAllFortressInfo::decode(reader)?)
        }
        RequestAllAgitInfo::EX_OPCODE => {
            ClientPacket::RequestAllAgitInfo(RequestAllAgitInfo::decode(reader)?)
        }
        RequestFortressSiegeInfo::EX_OPCODE => {
            ClientPacket::RequestFortressSiegeInfo(RequestFortressSiegeInfo::decode(reader)?)
        }
        RequestFortressMapInfo::EX_OPCODE => {
            ClientPacket::RequestFortressMapInfo(RequestFortressMapInfo::decode(reader)?)
        }
        RequestManorList::EX_OPCODE => ClientPacket::RequestManorList(RequestManorList::decode(reader)?),
        RequestProcureCropList::EX_OPCODE => {
            ClientPacket::RequestProcureCropList(RequestProcureCropList::decode(reader)?)
        }
        RequestSetSeed::EX_OPCODE => ClientPacket::RequestSetSeed(RequestSetSeed::decode(reader)?),
        RequestSetCrop::EX_OPCODE => ClientPacket::RequestSetCrop(RequestSetCrop::decode(reader)?),
        RequestJoinDominionWar::EX_OPCODE => {
            ClientPacket::RequestJoinDominionWar(RequestJoinDominionWar::decode(reader)?)
        }
        RequestDominionInfo::EX_OPCODE => {
            ClientPacket::RequestDominionInfo(RequestDominionInfo::decode(reader)?)
        }
        RequestBidItemAuction::EX_OPCODE => {
            ClientPacket::RequestBidItemAuction(RequestBidItemAuction::decode(reader)?)
        }
        RequestInfoItemAuction::EX_OPCODE => {
            ClientPacket::RequestInfoItemAuction(RequestInfoItemAuction::decode(reader)?)
        }
        other => ClientPacket::UnportedEx(other),
    };
    Ok(Some(packet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::skills::AcquireSkillType;
    use ConnectionState::{Authenticated, Connected, Entering, InGame};
    use l2_net::Encode;

    /// A body long enough for every ported packet (all fields zero; strings are empty).
    /// EnterWorld needs 104 bytes.
    const ZEROS: [u8; 128] = [0; 128];
    /// The id byte of ex packets.
    const EX_PACKET: u8 = 0xD0;

    fn decode(state: ConnectionState, opcode: u8, body: &[u8]) -> Result<Option<ClientPacket>, PacketError> {
        decode_client(state, opcode, &mut PacketReader::new(body))
    }

    #[test]
    fn every_opcode_in_every_state_follows_the_generated_table() {
        for opcode in 0..=u8::MAX {
            for &state in ConnectionState::ALL {
                let known = ClientOpcode::from_id(opcode);
                let accepted = known.is_some_and(|op| op.connection_states().contains(&state));
                // 0xD0 reads its ex id from the body: 0xFFFF is no ex packet, so it is ignored.
                let body = [0xFF; 2].iter().chain(&ZEROS).copied().collect::<Vec<u8>>();
                let result = decode(state, opcode, &body).unwrap();
                if opcode == EX_PACKET {
                    assert!(result.is_none(), "{state:?}");
                } else {
                    assert_eq!(result.is_some(), accepted, "opcode {opcode:#04x} in {state:?}");
                }
            }
        }
    }

    #[test]
    fn every_ex_opcode_in_every_state_follows_the_generated_table() {
        for ex in ExClientOpcode::ALL {
            for &state in ConnectionState::ALL {
                let mut body = ex.id().to_le_bytes().to_vec();
                body.extend(ZEROS);
                let result = decode(state, EX_PACKET, &body).unwrap();
                assert_eq!(
                    result.is_some(),
                    ex.connection_states().contains(&state),
                    "{} in {state:?}",
                    ex.name()
                );
            }
        }
    }

    #[test]
    fn states_match_java_for_the_ported_packets() {
        // GamePacketHandler + ClientPackets.java: handshake in CONNECTED, character screen in
        // AUTHENTICATED.
        let ported = [
            (0x0E, Connected),
            (0x2B, Connected),
            (0x0C, Authenticated),
            (0x0D, Authenticated),
            (0x12, Authenticated),
            (0x13, Authenticated),
            (0x7B, Authenticated),
            (0x11, Entering),
        ];
        for (opcode, home) in ported {
            for &state in ConnectionState::ALL {
                let result = decode(state, opcode, &ZEROS).unwrap();
                assert_eq!(
                    result.is_some(),
                    state == home,
                    "opcode {opcode:#04x} in {state:?}"
                );
            }
        }
        // RequestGotoLobby is ex id 0x0036, AUTHENTICATED only.
        for &state in ConnectionState::ALL {
            assert_eq!(
                decode(state, EX_PACKET, &[0x36, 0x00]).unwrap().is_some(),
                state == Authenticated,
                "{state:?}"
            );
        }
        // RequestKeyMapping is ex id 0x0021, ENTERING and IN_GAME.
        for &state in ConnectionState::ALL {
            assert_eq!(
                decode(state, EX_PACKET, &[0x21, 0x00]).unwrap().is_some(),
                matches!(state, Entering | InGame),
                "{state:?}"
            );
        }
        // Logout is accepted in AUTHENTICATED and IN_GAME; chat, restart and net ping in IN_GAME.
        for &state in ConnectionState::ALL {
            assert_eq!(
                decode(state, 0x00, &ZEROS).unwrap().is_some(),
                matches!(state, Authenticated | InGame),
                "{state:?}"
            );
            for opcode in [0x49, 0x57, 0xB1] {
                assert_eq!(
                    decode(state, opcode, &ZEROS).unwrap().is_some(),
                    state == InGame,
                    "opcode {opcode:#04x} in {state:?}"
                );
            }
        }
    }

    #[test]
    fn decodes_the_right_packet() {
        assert_eq!(
            decode(Connected, 0x0E, &[0x11, 0x01, 0, 0]).unwrap(),
            Some(ClientPacket::ProtocolVersion(ProtocolVersion { version: 273 }))
        );
        assert_eq!(
            decode(Authenticated, 0x0D, &[1, 0, 0, 0]).unwrap(),
            Some(ClientPacket::CharacterDelete(CharacterDelete { char_slot: 1 }))
        );
        assert_eq!(
            decode(Authenticated, 0x7B, &[2, 0, 0, 0]).unwrap(),
            Some(ClientPacket::CharacterRestore(CharacterRestore { char_slot: 2 }))
        );
        assert_eq!(
            decode(Authenticated, 0x13, &[]).unwrap(),
            Some(ClientPacket::NewCharacter(NewCharacter))
        );
        // Ex packet: the dispatcher consumes the u16 ex id.
        assert_eq!(
            decode(Authenticated, 0xD0, &[0x36, 0x00]).unwrap(),
            Some(ClientPacket::RequestGotoLobby(RequestGotoLobby))
        );
        assert_eq!(
            decode(InGame, 0x49, &[0x41, 0, 0, 0, 2, 0, 0, 0, 0x42, 0, 0, 0]).unwrap(),
            Some(ClientPacket::Say2(Say2 {
                text: "A".into(),
                chat_type: 2,
                target: Some("B".into()),
            }))
        );
        assert_eq!(
            decode(InGame, 0x57, &[]).unwrap(),
            Some(ClientPacket::RequestRestart(RequestRestart))
        );
        // What the encoders write decodes to the same packet (opcode bytes stripped).
        let select = CharacterSelect {
            char_slot: 3,
            unk1: 1,
            unk2: 2,
            unk3: 3,
            unk4: 4,
        };
        let bytes = select.to_bytes();
        assert_eq!(
            decode(Authenticated, bytes[0], &bytes[1..]).unwrap(),
            Some(ClientPacket::CharacterSelect(select))
        );
    }

    #[test]
    fn known_but_unported_packets_are_reported() {
        // RequestPartyMatchConfig (0x7F, party matching) is accepted in IN_GAME, but has no struct
        // yet.
        assert_eq!(
            decode(InGame, 0x7F, &[]).unwrap(),
            Some(ClientPacket::Unported(ClientOpcode::RequestPartyMatchConfig))
        );
        // RequestProcedureCropList (ex id 0x0002, Java RequestProcureCropList) is ported; an ex
        // packet that has no struct still consumes its id and leaves the body alone.
        let mut r = PacketReader::new(&[0x01, 0x00, 0xAA]);
        let result = decode_client(InGame, 0xD0, &mut r).unwrap();
        assert_eq!(result, Some(ClientPacket::RequestManorList(RequestManorList)));
        assert_eq!(r.remaining(), 1);
        // A count of 0 is Java's bad list: the body is read (4 bytes) and the packet is dropped.
        let mut r = PacketReader::new(&[0x02, 0x00, 0, 0, 0, 0]);
        let result = decode_client(InGame, 0xD0, &mut r).unwrap();
        assert_eq!(
            result,
            Some(ClientPacket::RequestProcureCropList(RequestProcureCropList {
                items: None
            }))
        );
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn attack_request_has_both_java_ids() {
        // Java ClientPackets: ATTACK (0x01) and ATTACK_REQUEST (0x32) both use AttackRequest.
        // object id, three zero origin ints, then the attack id byte (1 = shift-click).
        let mut body = 0x4433_2211_i32.to_le_bytes().to_vec();
        body.extend([0; 12]);
        body.push(1);
        let expected = Some(ClientPacket::AttackRequest(AttackRequest {
            object_id: 0x4433_2211,
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            attack_id: 1,
        }));
        for id in [0x01, 0x32] {
            assert_eq!(decode(InGame, id, &body).unwrap(), expected, "opcode {id:#04x}");
        }
        // Not in the other states, like the rest of the table.
        assert_eq!(decode(Authenticated, 0x01, &body).unwrap(), None);
    }

    #[test]
    fn combat_and_item_packets_decode_in_game() {
        assert_eq!(
            decode(InGame, 0x14, &[]).unwrap(),
            Some(ClientPacket::RequestItemList(RequestItemList))
        );
        assert_eq!(
            decode(InGame, 0x19, &[7, 0, 0, 0, 1, 0, 0, 0]).unwrap(),
            Some(ClientPacket::UseItem(UseItem {
                object_id: 7,
                ctrl_pressed: true,
            }))
        );
        assert_eq!(
            decode(InGame, 0x7D, &[5, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestRestartPoint(RequestRestartPoint {
                requested_point_type: 5,
            }))
        );
        // The state check comes first: RequestRestartPoint is IN_GAME only.
        assert_eq!(decode(Authenticated, 0x7D, &[5, 0, 0, 0]).unwrap(), None);
    }

    #[test]
    fn skill_and_soulshot_packets_decode_in_game() {
        assert_eq!(
            decode(InGame, 0x39, &[0x99, 0x04, 0, 0, 2, 0, 0, 0, 1]).unwrap(),
            Some(ClientPacket::RequestMagicSkillUse(RequestMagicSkillUse {
                magic_id: 0x499,
                // Java: any non-zero int is "pressed" for ctrl and any non-zero byte for shift.
                ctrl_pressed: true,
                shift_pressed: true,
            }))
        );
        assert_eq!(
            decode(InGame, 0x7C, &[1, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAcquireSkill(RequestAcquireSkill {
                id: 1,
                level: 2,
                skill_type: AcquireSkillType::Pledge,
                sub_type: 0,
            }))
        );
        // The squad id is only read for SUBPLEDGE (type 3).
        assert_eq!(
            decode(InGame, 0x7C, &[1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 5, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAcquireSkill(RequestAcquireSkill {
                id: 1,
                level: 2,
                skill_type: AcquireSkillType::Subpledge,
                sub_type: 5,
            }))
        );
        // An unknown learn type is a bad packet, not a panic.
        assert!(decode(InGame, 0x7C, &[1, 0, 0, 0, 2, 0, 0, 0, 9, 0, 0, 0]).is_err());
        assert_eq!(
            decode(InGame, 0x73, &[4, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAcquireSkillInfo(RequestAcquireSkillInfo {
                id: 4,
                level: 1,
                skill_type: AcquireSkillType::Transform,
            }))
        );
        assert_eq!(
            decode(
                InGame,
                0x3D,
                &[2, 0, 0, 0, 14, 0, 0, 0, 0xB4, 0x04, 0, 0, 7, 0, 0, 0, 1, 0, 0, 0]
            )
            .unwrap(),
            Some(ClientPacket::RequestShortCutReg(RequestShortCutReg {
                type_id: 2,
                slot: 14,
                id: 1204,
                level: 7,
                character_type: 1,
            }))
        );
        assert_eq!(
            decode(InGame, 0x3F, &[13, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestShortCutDel(RequestShortCutDel { slot: 13 }))
        );
        // The shortcut bar only exists in game.
        assert_eq!(decode(Authenticated, 0x3F, &[13, 0, 0, 0]).unwrap(), None);
        // Ex packets: the dispatcher consumes the ex id (0x000D soulshot, 0x004B dispel).
        assert_eq!(
            decode(InGame, 0xD0, &[0x0D, 0x00, 7, 0, 0, 0, 1, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAutoSoulShot(RequestAutoSoulShot {
                item_id: 7,
                shot_type: RequestAutoSoulShot::ON,
            }))
        );
        assert_eq!(
            decode(InGame, 0xD0, &[0x4B, 0x00, 9, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestDispel(RequestDispel {
                object_id: 9,
                skill_id: 3,
                skill_level: 2,
            }))
        );
        assert_eq!(
            decode(InGame, 0xD0, &[0x7E, 0x00, 9, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestVoteNew(RequestVoteNew { target_id: 9 }))
        );
        assert_eq!(decode(Entering, 0xD0, &[0x7E, 0x00, 9, 0, 0, 0]).unwrap(), None);
        // The state check comes first: these are IN_GAME only.
        assert_eq!(decode(Authenticated, 0x39, &ZEROS).unwrap(), None);
        assert_eq!(decode(Entering, 0xD0, &[0x4B, 0x00]).unwrap(), None);
    }

    #[test]
    fn shop_packets_decode_in_game_only() {
        use crate::client::{BuyItemEntry, SellItemEntry};
        let buy = RequestBuyItem {
            list_id: 382,
            items: Some(vec![BuyItemEntry {
                item_id: 1835,
                count: 5,
            }]),
        };
        let sell = RequestSellItem {
            list_id: 382,
            items: Some(vec![SellItemEntry {
                object_id: 7,
                item_id: 57,
                count: 1,
            }]),
        };
        let refund = RequestRefundItem {
            list_id: 382,
            items: Some(vec![0]),
        };
        for &state in ConnectionState::ALL {
            let in_game = state == InGame;
            let got = decode(state, 0x40, &buy.to_bytes()[1..]).unwrap();
            assert_eq!(
                got,
                in_game.then(|| ClientPacket::RequestBuyItem(buy.clone())),
                "{state:?}"
            );
            let got = decode(state, 0x37, &sell.to_bytes()[1..]).unwrap();
            assert_eq!(
                got,
                in_game.then(|| ClientPacket::RequestSellItem(sell.clone())),
                "{state:?}"
            );
            let got = decode(state, EX_PACKET, &refund.to_bytes()[1..]).unwrap();
            assert_eq!(
                got,
                in_game.then(|| ClientPacket::RequestRefundItem(refund.clone())),
                "{state:?}"
            );
            let got = decode(state, EX_PACKET, &RequestBuySellUIClose.to_bytes()[1..]).unwrap();
            assert_eq!(
                got,
                in_game.then_some(ClientPacket::RequestBuySellUIClose(RequestBuySellUIClose)),
                "{state:?}"
            );
        }
    }

    #[test]
    fn friend_and_block_packets_decode_in_game_only() {
        // "Hi" as UTF-16LE with the terminator, then the receiver "A".
        let send_msg = [b'H', 0, b'i', 0, 0, 0, b'A', 0, 0, 0];
        assert_eq!(
            decode(InGame, 0x6B, &send_msg).unwrap(),
            Some(ClientPacket::RequestSendFriendMsg(RequestSendFriendMsg {
                message: "Hi".into(),
                receiver: "A".into(),
            }))
        );
        assert_eq!(
            decode(InGame, 0x77, &[b'A', 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestFriendInvite(RequestFriendInvite {
                name: "A".into()
            }))
        );
        assert_eq!(
            decode(InGame, 0x78, &[1, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestAnswerFriendInvite(
                RequestAnswerFriendInvite { response: 1 }
            ))
        );
        assert_eq!(
            decode(InGame, 0x79, &[]).unwrap(),
            Some(ClientPacket::RequestFriendList(RequestFriendList))
        );
        assert_eq!(
            decode(InGame, 0x7A, &[b'A', 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestFriendDel(RequestFriendDel {
                name: "A".into()
            }))
        );
        assert_eq!(
            decode(InGame, 0xA9, &[4, 0, 0, 0]).unwrap(),
            Some(ClientPacket::RequestBlock(RequestBlock { kind: 4, name: None }))
        );
        // Not accepted before the game.
        for opcode in [0x6B, 0x77, 0x78, 0x79, 0x7A, 0xA9] {
            assert_eq!(
                decode(Authenticated, opcode, &ZEROS).unwrap(),
                None,
                "{opcode:#04x}"
            );
        }
    }

    #[test]
    fn unknown_opcodes_are_ignored() {
        assert_eq!(decode(Connected, 0x02, &ZEROS).unwrap(), None); // unused in the Java table
        assert_eq!(decode(Authenticated, 0xFF, &ZEROS).unwrap(), None);
        assert_eq!(decode(Authenticated, 0xD0, &[0xFF, 0xFF]).unwrap(), None);
    }

    #[test]
    fn short_bodies() {
        // Right state, body too short: an error.
        for (state, opcode, len) in [
            (Connected, 0x2B, 3),
            (Authenticated, 0x0C, 4),
            (Authenticated, 0x0D, 3),
            (Authenticated, 0x12, 17),
            (Authenticated, 0x7B, 0),
            (Entering, 0x11, 103),
            (Authenticated, 0xD0, 1),
            (InGame, 0x49, 2),
            (InGame, 0x3D, 19),
            (InGame, 0x3F, 3),
        ] {
            assert!(
                decode(state, opcode, &ZEROS[..len]).is_err(),
                "opcode {opcode:#04x}"
            );
        }
        // Wrong state: the body is never read, so no error (Java checks the state first).
        assert_eq!(decode(InGame, 0x0D, &[]).unwrap(), None);
        assert_eq!(decode(Connected, 0x12, &[]).unwrap(), None);
        // ProtocolVersion tolerates any length (Java quirk), even an empty one.
        assert_eq!(
            decode(Connected, 0x0E, &[]).unwrap(),
            Some(ClientPacket::ProtocolVersion(ProtocolVersion { version: 0 }))
        );
    }
}
