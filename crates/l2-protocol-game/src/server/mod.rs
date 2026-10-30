//! Packets the game server sends to the game client.
//!
//! Same pattern as the client packets (see [`crate::client`]): a struct per packet, `OPCODE` from
//! the generated [`ServerOpcode`] table, `impl Encode` (opcode + fields, Java `writeImpl`) and
//! `impl Decode` (fields after the opcode, for the proxy and test clients). Data that Java reads
//! from the database or from game objects is passed in as plain values: no logic in here.

use l2_net::{PacketError, PacketReader, PacketWriter};

use crate::generated::opcodes::ServerOpcode;

/// Declares a server packet that is nothing but a list of ints, longs and strings in a row (the
/// field types of [`crate::client::WireField`]). Same result as writing the struct, the `OPCODE`
/// const and the `Encode`/`Decode` impls by hand (see `skills.rs`). Put the `// Java:` line above
/// the invocation.
macro_rules! server_packet {
    ($(#[$meta:meta])* $name:ident, $op:ident { $($(#[$fmeta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: $ty,)*
        }

        impl $name {
            pub const OPCODE: $crate::generated::opcodes::ServerOpcode =
                $crate::generated::opcodes::ServerOpcode::$op;
        }

        impl l2_net::Encode for $name {
            fn encode(&self, w: &mut l2_net::PacketWriter) {
                $crate::server::write_opcode(w, Self::OPCODE);
                $($crate::client::WireField::put(&self.$field, w);)*
            }
        }

        impl l2_net::Decode for $name {
            fn decode(r: &mut l2_net::PacketReader) -> Result<Self, l2_net::PacketError> {
                Ok(Self {
                    $($field: <$ty as $crate::client::WireField>::take(r)?,)*
                })
            }
        }
    };
}

/// Like [`server_packet!`] for a packet without fields (a unit struct).
macro_rules! server_unit_packet {
    ($(#[$meta:meta])* $name:ident, $op:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub const OPCODE: $crate::generated::opcodes::ServerOpcode =
                $crate::generated::opcodes::ServerOpcode::$op;
        }

        impl l2_net::Encode for $name {
            fn encode(&self, w: &mut l2_net::PacketWriter) {
                $crate::server::write_opcode(w, Self::OPCODE);
            }
        }

        impl l2_net::Decode for $name {
            fn decode(_r: &mut l2_net::PacketReader) -> Result<Self, l2_net::PacketError> {
                Ok(Self)
            }
        }
    };
}

pub mod admin_support;
pub mod auction;
pub mod boss;
pub mod charscreen;
pub mod chat;
pub mod combat;
pub mod command_channel;
pub mod contacts;
pub mod debug_draw;
pub mod duel;
pub mod effect_packets;
pub mod enchant;
pub mod enchant_skill;
pub mod enterworld;
pub mod entry;
pub mod fence;
pub mod fishing;
pub mod fort;
pub mod friend;
pub mod gm_view;
pub mod gm_windows;
pub mod ground_item;
pub mod handler_windows;
pub mod handshake;
pub mod henna;
pub mod interaction;
pub mod items;
pub mod leftover;
pub mod mail;
pub mod manor;
pub mod map_windows;
pub mod minigame;
pub mod movement;
pub mod multisell;
pub mod olympiad;
pub mod party;
pub mod party_room;
pub mod pccafe;
pub mod pledge;
pub mod quest;
pub mod recipe;
pub mod refine;
pub mod regen;
pub mod residence;
pub mod screen;
pub mod secondary_auth;
pub mod shop;
pub mod siege;
pub mod skills;
pub mod sound;
pub mod ssq;
pub mod store;
pub mod summon;
pub mod system_message;
pub mod territory;
pub mod trade;
pub mod userinfo;
pub mod vehicle;
pub mod visibility;
pub mod warehouse;

pub use admin_support::{AdminForgePacket, ForgePart, PetitionVotePacket, Snoop};
pub use auction::{ExItemAuctionInfo, ItemAuctionNext};
pub use boss::{
    BossRecordEntry, CursedWeaponLocation, ExCursedWeaponList, ExCursedWeaponLocation, ExGetBossRecord,
};
pub use charscreen::{
    CharCreateFail, CharCreateOk, CharDeleteFail, CharDeleteSuccess, CharSelectEntry, CharSelected,
    CharSelectionInfo, NewCharTemplate, NewCharacterSuccess, ServerClose, SsqInfo, active_slot,
    delete_seconds, exp_percent,
};
pub use chat::{ChatType, CreatureSay, LeaveWorld, NetPing, NpcSay, RestartResponse, SayBody, SenderField};
pub use combat::{
    Attack, AttackHit, AutoAttackStart, AutoAttackStop, ChangeMoveType, ChangeWaitType, Die,
    ExUseSharedGroupItem, HITFLAG_CRIT, HITFLAG_MISS, HITFLAG_SHLD, HITFLAG_USESS, Revive, SetupGauge,
};
pub use command_channel::{
    ExAskJoinMpcc, ExCloseMpcc, ExMpccPartyInfoUpdate, ExMpccShowPartyMemberInfo,
    ExMultiPartyCommandChannelInfo, ExOpenMpcc, MpccMember, MpccPartyEntry,
};
pub use contacts::{ExConfirmAddingContact, FriendListExtended, FriendListExtendedEntry};
pub use debug_draw::{ExServerPrimitive, Primitive, PrimitiveColor, ShowBoard};
pub use duel::{ExDuelAskStart, ExDuelEnd, ExDuelReady, ExDuelStart, ExDuelUpdateUserInfo};
pub use effect_packets::{ExSpawnEmitter, FlyToLocation, StartRotation, StopRotation};
pub use enchant::{
    ChooseInventoryItem, EnchantResult, ExAttributeEnchantResult, ExBaseAttributeCancelResult,
    ExChooseInventoryAttributeItem, ExPutEnchantSupportItemResult, ExPutEnchantTargetItemResult,
    ExShowBaseAttributeCancelWindow,
};
pub use enchant_skill::{
    ExEnchantSkillInfo, ExEnchantSkillInfoDetail, ExEnchantSkillList, ExEnchantSkillResult,
};
pub use enterworld::{
    EtcStatusUpdate, ExBasicActionList, ExGetBookMarkInfoPacket, ExShowScreenMessage, ExStorageMaxCount,
    ExUiSetting, ExVoteSystemInfo, FriendList, HennaInfo, QuestList, ShortcutInit, ShortcutRegister,
    SkillCoolTime, SkillList, ValidateLocation,
};
pub use entry::{
    ExBrExtraUserInfo, ExNevitAdventEffect, ExNevitAdventPointInfo, ExNevitAdventTimeChange, ExQuestItemList,
    ExRotation, ExShowContactList, ExVitalityPointInfo, MacroCommandEntry, MacroEntry, MacroList,
};
pub use fence::{ExColosseumFenceInfo, OnEventTrigger};
pub use fishing::{ExFishingEnd, ExFishingHpRegen, ExFishingStart, ExFishingStartCombat};
pub use fort::{ExShowFortressInfo, ExShowFortressMapInfo, ExShowFortressSiegeInfo, FortressInfo};
pub use friend::{FriendAddRequest, FriendPacket, FriendStatusPacket, L2FriendSay};
pub use gm_view::GmViewItemList;
pub use gm_windows::{
    GmHennaInfo, GmSkillEntry, GmViewCharacterInfo, GmViewQuestInfo, GmViewSkillInfo,
    GmViewWarehouseWithdrawList,
};
pub use ground_item::{DropItem, GetItem, SpawnItem};
pub use handler_windows::{
    Dice, ExShowQuestInfo, ShopPreviewList, ShopPreviewProduct, ShowXMasSeal, TutorialCloseHtml,
};
pub use handshake::{KeyPacket, LoginFail};
pub use henna::{
    HennaEquipEntry, HennaEquipList, HennaItemInfo, HennaItemRemoveInfo, HennaPreview, HennaRemoveEntry,
    HennaRemoveList, HennaStatPreview,
};
pub use interaction::{
    ActionFailed, ChairSit, DoorInfo, DoorStatusUpdate, ExAskCoupleAction, MyTargetSelected, NpcHtmlMessage,
    ShowTownMap, SocialAction, StaticObjectInfo, TargetSelected, TargetUnselected,
};
pub use items::{
    InventoryBlock, InventoryUpdate, ItemInfo, ItemList, ItemUpdate, read_inventory_block, read_item,
    read_item_updates, write_inventory_block, write_item, write_item_elemental_and_enchant,
    write_item_updates,
};
pub use leftover::{ExChangeClientEffectInfo, ExRpItemLink, ShopPreviewInfo};
pub use mail::{
    ExChangePostState, ExMailArrived, ExNoticePostArrived, ExNoticePostSent, ExReplyPostItemList,
    ExReplyReceivedPost, ExReplySentPost, ExShowReceivedPostList, ExShowSentPostList, ReceivedPostEntry,
    SentPostEntry,
};
pub use manor::{
    BuyListSeed, CropOffer, CropProcureInfo, CropSettingInfo, ExSendManorList, ExShowCropInfo,
    ExShowCropSetting, ExShowManorDefaultInfo, ExShowProcureCropDetail, ExShowSeedInfo, ExShowSeedSetting,
    ExShowSellCropList, ManorDefaultCrop, PeriodCropSales, PeriodSales, ProcureCastle, SeedForSale,
    SeedProductionInfo, SeedSettingInfo, SellCropItem,
};
pub use map_windows::{ShowCalculator, ShowMiniMap};
pub use minigame::{
    CC_RECORD_FINISH, CC_RECORD_INITIALIZE, CC_RECORD_MAX_PLAYERS, CC_RECORD_UPDATE, CcRecordEntry,
    CubePlayer, ExCubeGameAddPlayer, ExCubeGameChangePoints, ExCubeGameChangeTeam,
    ExCubeGameChangeTimeToStart, ExCubeGameCloseUi, ExCubeGameEnd, ExCubeGameExtendedChangePoints,
    ExCubeGameRemovePlayer, ExCubeGameRequestReady, ExCubeGameTeamList, ExPvpMatchCcMyRecord,
    ExPvpMatchCcRecord, ExPvpMatchCcRetire, ExPvpMatchRecord, ExPvpMatchUserDie, ExSendUiEvent,
    ExStartScenePlayer, PvpMember,
};
pub use movement::{MoveToLocation, MoveToPawn, StopMove, TeleportToLocation};
pub use multisell::{
    MultiSellEntry, MultiSellIngredient, MultiSellItemDetail, MultiSellList, MultiSellProduct,
};
pub use olympiad::{
    ExHeroList, ExOlympiadMatchEnd, ExOlympiadMatchList, ExOlympiadMatchResult, ExOlympiadMode,
    ExOlympiadSpelledInfo, ExOlympiadUserInfo, HeroEntry, ObservationMode, ObservationReturn,
    OlympiadMatchEntry, OlympiadResultEntry, OlympiadSpelledEffect,
};
pub use party::{
    AskJoinParty, ExPartyPetWindowAdd, ExPartyPetWindowDelete, ExPartyPetWindowUpdate, JoinParty,
    PartyDistributionType, PartyMemberLocation, PartyMemberPosition, PartyMemberVitals, PartyPetWindow,
    PartySmallWindowAdd, PartySmallWindowAll, PartySmallWindowDelete, PartySmallWindowDeleteAll,
    PartySmallWindowUpdate, PartySummon, PartyWindowMember,
};
pub use party_room::{
    ExAskJoinPartyRoom, ExAskModifyPartyLooting, ExClosePartyRoom, ExListPartyMatchingWaitingRoom,
    ExManagePartyRoomMember, ExPartyRoomMember, ExRequestChangeNicknameColor, ExSetPartyLooting,
    ListPartyWaiting, PartyRoomInfo, RoomMember, WaitingRoom,
};
pub use pccafe::ExPcCafePointInfo;
pub use pledge::{
    AllianceClanInfo, AllianceInfo, AllyCrest, AskJoinAlly, AskJoinPledge, ExPledgeEmblem,
    ExSubPledgeSkillAdd, GmViewPledgeInfo, JoinPledge, ManagePledgePower, PledgeCrest, PledgeInfo,
    PledgeMember, PledgeMemberOnline, PledgePowerGradeList, PledgeReceiveMemberInfo, PledgeReceivePowerInfo,
    PledgeReceiveSubPledgeCreated, PledgeReceiveWarList, PledgeShowInfoUpdate, PledgeShowMemberListAdd,
    PledgeShowMemberListAll, PledgeShowMemberListDelete, PledgeShowMemberListDeleteAll,
    PledgeShowMemberListUpdate, PledgeSkillEntry, PledgeSkillList, PledgeSkillListAdd, PledgeStatusChanged,
    RankPrivileges, StartPledgeWar, StopPledgeWar, SubPledgeSkill, SurrenderPledgeWar,
};
pub use quest::{
    ExQuestNpcLogList, ExShowQuestMark, NpcQuestHtmlMessage, QuestNpcLogEntry, RadarControl,
    TutorialEnableClientEvent, TutorialShowHtml, TutorialShowQuestionMark,
};
pub use recipe::{
    ManufactureEntry, RecipeBookItemList, RecipeItemMakeInfo, RecipeShopItemInfo, RecipeShopManageList,
    RecipeShopMsg, RecipeShopSellList,
};
pub use refine::{
    ExPutCommissionResultForVariationMake, ExPutIntensiveResultForVariationMake,
    ExPutItemResultForVariationCancel, ExPutItemResultForVariationMake, ExShowVariationCancelWindow,
    ExShowVariationMakeWindow, ExVariationCancelResult, ExVariationResult,
};
pub use regen::ExRegenMax;
pub use residence::{AgitDecoInfo, AgitInfo, ExShowAgitInfo};
pub use screen::{Earthquake, ExRedSky, SpecialCamera, SunRise, SunSet};
pub use secondary_auth::{Ex2ndPasswordAck, Ex2ndPasswordCheck, Ex2ndPasswordVerify};
pub use shop::{BuyList, BuyListProduct, ExBuySellList, RefundEntry, SellEntry};
pub use siege::{
    CastleInfo, DefenderState, ExShowCastleInfo, SiegeAttackerList, SiegeClan, SiegeDefender,
    SiegeDefenderList, SiegeInfo, SiegeOwnerClan, SiegeTime,
};
pub use skills::{
    AbnormalStatusUpdate, AcquireSkillDone, AcquireSkillEntry, AcquireSkillInfo, AcquireSkillList,
    AcquireSkillRequirement, AcquireSkillType, EffectIcon, ExAutoSoulShot, MagicSkillCanceled,
    MagicSkillLaunched, MagicSkillUse, PartySpelled, ShortBuffStatusUpdate, SkillPosition,
};
pub use sound::PlaySound;
pub use ssq::{ExShowSeedMapInfo, SevenSignsView, SsqStatus};
pub use store::{
    BuyStoreEntry, ExPrivateStoreSetWholeMsg, PrivateStoreListBuy, PrivateStoreListSell,
    PrivateStoreManageListBuy, PrivateStoreManageListSell, PrivateStoreMsgBuy, PrivateStoreMsgSell,
    SellStoreEntry, StoreCandidate,
};
pub use summon::{
    ExChangeNpcState, PetDelete, PetInfo, PetInventoryUpdate, PetItemList, PetStatusShow, PetStatusUpdate,
    Ride, SetSummonRemainTime, SummonInfo,
};
pub use system_message::{ConfirmDlg, SystemMessage, SystemMessageParam};
pub use territory::{
    DominionTerritory, ExDominionWarEnd, ExDominionWarStart, ExReplyDominionInfo, ExShowDominionRegistry,
    ExShowOwnthingPos, TerritoryWards, WardPosition,
};
pub use trade::{
    SendTradeRequest, TradeDone, TradeItemInfo, TradeOtherAdd, TradeOtherDone, TradeOwnAdd, TradeStart,
    TradeUpdate,
};
pub use userinfo::UserInfo;
pub use vehicle::{
    AirShipTeleport, ExAirShipInfo, ExAirShipStopMove, ExAirShipTeleportList, ExGetOffAirShip,
    ExGetOnAirShip, ExMoveToLocationAirShip, ExMoveToLocationInAirShip, ExStopMoveAirShip,
    ExStopMoveInAirShip, ExValidateLocationInAirShip, GetOffVehicle, GetOnVehicle, MoveToLocationInVehicle,
    StopMoveInVehicle, ValidateLocationInVehicle, VehicleCheckLocation, VehicleDeparture, VehicleInfo,
    VehicleStarted,
};
pub use visibility::{
    CharInfo, DeleteObject, ExSetCompassZoneCode, NicknameChanged, NpcInfo, RelationChanged, RelationEntry,
    StatusUpdate,
};
pub use warehouse::{
    PackageSendableList, PackageToList, SortedWareHouseWithdrawalList, SortedWarehouseItem,
    WareHouseDepositList, WareHouseWithdrawalList,
};

/// Reads an int count, which Java never writes negative. Anything negative is an error.
pub(crate) fn read_count(r: &mut PacketReader, field: &'static str) -> Result<usize, PacketError> {
    let count = r.i32()?;
    usize::try_from(count).map_err(|_| PacketError::invalid(field, format!("negative count {count}")))
}

/// Writes a server packet's id the way Java `ServerPackets.writeId` does: the id byte, then the
/// `u16` ex id for ex packets (id byte `0xFE`).
///
/// NOTE(java-quirk): Java writes the ex id only when it is greater than 0, so `ExDummy`
/// (`0xFE`, ex id 0) is the single byte `0xFE`.
pub fn write_opcode(w: &mut PacketWriter, opcode: ServerOpcode) {
    w.u8(opcode.id());
    match opcode.ex_id() {
        Some(ex) if ex > 0 => {
            w.u16(ex);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opcode_bytes(opcode: ServerOpcode) -> Vec<u8> {
        let mut w = PacketWriter::new();
        write_opcode(&mut w, opcode);
        w.into_bytes()
    }

    #[test]
    fn plain_and_ex_ids() {
        assert_eq!(opcode_bytes(ServerOpcode::VersionCheck), [0x2E]);
        assert_eq!(opcode_bytes(ServerOpcode::ServerClose), [0x20]);
        // 0xFE + u16 ex id, little endian: EX_RESTART_CLIENT is 0x48.
        assert_eq!(opcode_bytes(ServerOpcode::ExRestartClient), [0xFE, 0x48, 0x00]);
    }

    #[test]
    fn ex_dummy_is_one_byte() {
        assert_eq!(ServerOpcode::ExDummy.ex_id(), Some(0));
        assert_eq!(opcode_bytes(ServerOpcode::ExDummy), [0xFE]);
    }

    #[test]
    fn every_opcode_writes_one_or_three_bytes() {
        for op in ServerOpcode::ALL {
            let bytes = opcode_bytes(op);
            let expected = if op.ex_id().is_some_and(|ex| ex > 0) { 3 } else { 1 };
            assert_eq!(bytes.len(), expected, "{}", op.name());
            assert_eq!(bytes[0], op.id());
        }
    }
}
