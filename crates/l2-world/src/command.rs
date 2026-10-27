//! What the rest of the server can ask the world to do.

use std::fmt;
use std::net::IpAddr;

use l2_protocol_game::server::AcquireSkillType;

use crate::outbox::PacketSink;
use crate::player::{OnlinePlayer, PlayerData};

/// One client connection, as numbered by the game server's listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

/// In-game client input, already decoded by the session task.
///
/// Later cards add variants as they port packets (movement, actions, …). Anything the world
/// doesn't handle yet arrives as [`ClientInput::Other`] and is ignored at debug level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientInput {
    /// Java `Logout`: store the character, `LeaveWorld`, close the connection.
    Logout,
    /// Java `RequestRestart`: store the character and go back to the character screen.
    Restart,
    /// Java `Say2`. `chat_type` is the number the client sent; the world checks it.
    Say {
        chat_type: i32,
        text: String,
        target: Option<String>,
    },
    /// Java `RequestNetPing`.
    NetPing,
    /// Java `RequestSkillCoolTime`.
    SkillCoolTime,
    /// Java `RequestKeyMapping` (answered with the stored UI key mapping).
    KeyMapping,
    /// Java `MoveToLocation` (the client wants to walk; `movement_mode` 1 = mouse, 0 = keys).
    MoveTo {
        target_x: i32,
        target_y: i32,
        target_z: i32,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        movement_mode: i32,
    },
    /// Java `ValidatePosition`: where the client thinks the player is.
    ValidatePosition { x: i32, y: i32, z: i32, heading: i32 },
    /// Java `CannotMoveAnymore`.
    CannotMoveAnymore { x: i32, y: i32, z: i32, heading: i32 },
    /// Java `Appearing`: the client has loaded the target of a teleport.
    Appearing,
    /// Java `Action`: a click on an object (`action_id` 0 = click, 1 = shift click). The origin
    /// the client sends is read and never used by Java.
    Action { object_id: i32, action_id: i8 },
    /// Java `RequestTargetCanceld` (`unselect` 0 = Escape key).
    TargetCanceled { unselect: i16 },
    /// Java `DlgAnswer`: the answer (1 = yes) to the `ConfirmDlg` with this message id.
    DlgAnswer {
        message_id: i32,
        answer: i32,
        requester_id: i32,
    },
    /// Java `RequestBypassToServer`: the raw command of an html link.
    Bypass { command: String },
    /// Java `RequestLinkHtml`: the raw path of a linked html page.
    LinkHtml { link: String },
    /// Java `SendBypassBuildCmd`: a GM command typed as `//command args`. `command` is the text
    /// after the slashes, already trimmed.
    BuildCmd { command: String },
    /// Java `BypassUserCmd`: a user command such as `/loc` (id 0).
    UserCommand { id: i32 },
    /// Java `AttackRequest` (opcodes `ATTACK` and `ATTACK_REQUEST`): a click to attack. The
    /// client's position and the shift flag are read and never used by Java.
    AttackRequest { object_id: i32, attack_id: i8 },
    /// Java `UseItem` (`ctrl_pressed` is the ctrl key).
    UseItem { object_id: i32, ctrl_pressed: bool },
    /// Java `RequestUnEquipItem`: the raw paperdoll slot number.
    UnEquipItem { paperdoll_slot: i32 },
    /// Java `RequestDestroyItem`.
    DestroyItem { object_id: i32, count: i64 },
    /// Java `RequestDropItem`.
    DropItem {
        object_id: i32,
        count: i64,
        x: i32,
        y: i32,
        z: i32,
    },
    /// Java `RequestItemList`.
    ItemList,
    /// Java `RequestAcquireSkillInfo`: the details of a skill a trainer teaches.
    AcquireSkillInfo {
        id: i32,
        level: i32,
        skill_type: AcquireSkillType,
    },
    /// Java `RequestAcquireSkill`: learn a skill from the trainer. `sub_type` is the squad of a
    /// sub-pledge skill (0 for every other type).
    AcquireSkill {
        id: i32,
        level: i32,
        skill_type: AcquireSkillType,
        sub_type: i32,
    },
    /// Java `RequestRestartPoint`: the raw `_requestedPointType` (0 village, 1 clan hall, 2
    /// castle, 3 fortress, 4 siege headquarters, 5 fixed, 27 jail).
    RestartPoint { point_type: i32 },
    /// Java `RequestMagicSkillUse`: the skill id (or display id) the client asks to use, with
    /// the ctrl (force use) and shift (do not move) keys.
    MagicSkillUse {
        magic_id: i32,
        ctrl_pressed: bool,
        shift_pressed: bool,
    },
    /// Java `RequestAutoSoulShot`: switch the automatic use of a shot on (`shot_type` 1) or off (0).
    AutoSoulShot { item_id: i32, shot_type: i32 },
    /// Java `RequestShortcutReg`: the raw wire values (type id, `slot + page * 12`, id, level,
    /// character type). The world maps them like Java does.
    ShortcutReg {
        type_id: i32,
        slot: i32,
        id: i32,
        level: i32,
        character_type: i32,
    },
    /// Java `RequestShortcutDel`: the wire slot (`slot + page * 12`).
    ShortcutDel { slot: i32 },
    /// Java `TradeRequest`: ask the player with this object id to trade.
    TradeRequest { object_id: i32 },
    /// Java `AnswerTradeRequest` (1 = accept).
    AnswerTradeRequest { response: i32 },
    /// Java `AddTradeItem`: put an item into the trade window.
    AddTradeItem {
        trade_id: i32,
        object_id: i32,
        count: i64,
    },
    /// Java `TradeDone` (1 = confirm, anything else = cancel).
    TradeDone { response: i32 },
    /// Java `MultiSellChoose`: run an entry of the open multisell list `amount` times.
    MultiSellChoose {
        list_id: i32,
        entry_id: i32,
        amount: i64,
    },
    /// The private store packets (M6-STORE-01): `RequestActionUse` for the three store actions,
    /// `RequestPrivateStore*`, `SetPrivateStore*`.
    Store(crate::trade::StoreInput),
    /// The symbol maker packets (L-01): `RequestHenna*`.
    Henna(crate::henna::HennaInput),
    /// Java `RequestVoteNew`: recommend the selected player (`target_id` is the object id the
    /// client thinks it selected).
    VoteNew { target_id: i32 },
    /// Java `RequestActionUse`: a button of the action window (`ctrl_pressed` is `readInt() ==
    /// 1`, `shift_pressed` `readByte() == 1`).
    ActionUse {
        action_id: i32,
        ctrl_pressed: bool,
        shift_pressed: bool,
    },
    /// Java `AnswerCoupleAction`: the answer (1 approve, 0 cancel, -1 refused) to the couple
    /// social action `object_id` asked for (see `actions.rs`).
    AnswerCoupleAction {
        action_id: i32,
        answer: i32,
        object_id: i32,
    },
    /// The clan packets (`RequestJoinPledge`, `RequestPledge*`, ...), see `clan/`.
    Clan(crate::clan::ClanInput),
    /// The packets about the player's pet (`RequestPetGetItem`, `RequestGiveItemToPet`, ...), see
    /// `summon/pet_items.rs`.
    Pet(crate::summon::pet_items::PetInput),
    /// The shop packets (`RequestBuyItem`, `RequestSellItem`, `RequestRefundItem`,
    /// `RequestBuySellUIClose`), see `shop/`.
    Shop(crate::shop::ShopInput),
    /// The warehouse and freight packets (`SendWareHouseDepositList`, ...), see `warehouse/`.
    Warehouse(crate::warehouse::WarehouseInput),
    /// The manor packets (`RequestManorList`, `RequestSetSeed`, `RequestSetCrop`, `RequestBuySeed`,
    /// `RequestProcureCropList`), see `manor/`.
    Manor(crate::manor::ManorInput),
    /// The crafting packets (`RequestRecipeBookOpen`, `RequestRecipeShop*`, ...), see `craft/`.
    Craft(crate::craft::CraftInput),
    /// Java `RequestJoinParty`: invite `name` to the party. `distribution_type_id` is the raw int
    /// of the packet (the world checks it).
    JoinParty { name: String, distribution_type_id: i32 },
    /// Java `RequestAnswerJoinParty`: 1 accept, 0 decline, -1 the client refuses party requests.
    AnswerJoinParty { response: i32 },
    /// Java `RequestFriendInvite`: ask `name` to become a friend.
    FriendInvite { name: String },
    /// Java `RequestAnswerFriendInvite`: 1 accepts the request, anything else refuses.
    AnswerFriendInvite { response: i32 },
    /// Java `RequestFriendList`: the friend list window.
    FriendList,
    /// Java `RequestGmList`: the GM list (`admin/flags.rs`).
    GmList,
    /// Java `GameClient.setClientTracert`: the five hops of the `EnterWorld` packet, sent right
    /// after the enter command (`//tracert`, `//strict_find_dualbox`).
    Tracert { hops: [[u8; 4]; 5] },
    /// Java `RequestFriendDel`: remove the friend called `name`.
    FriendDel { name: String },
    /// Java `RequestSendFriendMsg`: a private message to the friend `receiver`.
    SendFriendMsg { message: String, receiver: String },
    /// Java `RequestBlock`: 0 block and 1 unblock (with `name`), 2 the list, 3 refuse all messages,
    /// 4 accept messages again.
    Block { kind: i32, name: Option<String> },
    /// Java `RequestWithDrawalParty`: leave the party.
    WithdrawParty,
    /// Java `RequestOustPartyMember`: the leader expels `name`.
    OustPartyMember { name: String },
    /// Java `RequestChangePartyLeader`: the leader hands the party to `name`.
    ChangePartyLeader { name: String },
    /// Java `RequestExAskJoinMPCC`: invite the party of `name` into the command channel.
    AskJoinMpcc { name: String },
    /// Java `RequestExAcceptJoinMPCC`: 1 accepts.
    AcceptJoinMpcc { response: i32 },
    /// Java `RequestExOustFromMPCC`: dismiss the party of `name` from the channel.
    OustFromMpcc { name: String },
    /// Java `RequestExMPCCShowPartyMembersInfo`: the members of the party of this leader.
    MpccShowPartyMembersInfo { party_leader_id: i32 },
    /// Java `RequestDuelStart`: challenge the player `name`; `party_duel` is 1 for a party duel.
    DuelStart { name: String, party_duel: i32 },
    /// Java `RequestDuelAnswerStart`: 1 accept, -1 the client refuses duels, else decline.
    DuelAnswerStart { party_duel: i32, response: i32 },
    /// Java `RequestDuelSurrender`.
    DuelSurrender,
    /// The mail packets (`RequestSendPost`, `RequestReceivedPost`, ...), see `mail/`.
    Mail(crate::mail::MailInput),
    /// The enchant window packets (`RequestEnchantItem`, the attribute stone packets, ...), see
    /// `enchant/`.
    Enchant(crate::enchant::EnchantInput),
    /// The augmentation window packets (`RequestRefine`, `RequestConfirmTargetItem`, ...), see
    /// `augment.rs`.
    Augment(crate::augment::AugmentInput),
    /// Java `RequestExFishRanking`: the fishing ranking of the week.
    FishRanking,
    /// The siege window packets (M7-SIEGE-01).
    Siege(crate::siege::SiegeInput),
    /// The fortress packets (`RequestAllFortressInfo`, `RequestFortressSiegeInfo`,
    /// `RequestFortressMapInfo`), see `fort/`.
    Fort(crate::fort::FortInput),
    /// The Territory War packets (`RequestJoinDominionWar`, `RequestDominionInfo`), see
    /// `territory/`.
    Territory(crate::territory::TerritoryInput),
    /// Java `RequestAllAgitInfo`: the list of the auctionable clan halls.
    AllAgitInfo,
    /// The petition packets (`RequestPetition`, `RequestPetitionCancel`,
    /// `RequestPetitionFeedback`), see `petition.rs`.
    Petition(crate::petition::PetitionInput),
    /// Java `SnoopQuit`: the GM stops snooping the player with this object id.
    SnoopQuit { snoop_id: i32 },
    /// A packet the world doesn't handle yet.
    Other { opcode: u8, ex_opcode: Option<u16> },
}

/// Why a player left the world. The session task gets this through [`PacketSink::left_world`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveReason {
    Logout,
    /// Back to the character screen; the connection stays.
    Restart,
    Disconnected,
    Kicked,
    ServerShutdown,
}

/// A question for the admin endpoint. The answer goes to the callback (called on the world
/// thread, so it must only hand the value over, e.g. into a channel).
pub enum AdminQuery {
    OnlinePlayers(Box<dyn FnOnce(Vec<OnlinePlayer>) + Send>),
}

impl fmt::Debug for AdminQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OnlinePlayers(_) => f.write_str("OnlinePlayers(..)"),
        }
    }
}

/// Everything that can enter the world.
pub enum WorldCommand {
    /// The session loaded the character from the database; put it into the world.
    EnterWorld {
        session: SessionId,
        player: Box<PlayerData>,
        sink: Box<dyn PacketSink>,
        /// The client's address, from its connection. `None` when the session doesn't know it.
        /// Only the admin status shows it.
        ip: Option<IpAddr>,
    },
    /// An in-game packet from the client of `session`.
    Input {
        session: SessionId,
        input: ClientInput,
    },
    /// The connection is gone (the character is stored, no packets are sent).
    Disconnected {
        session: SessionId,
    },
    /// The login server asked to kick this account.
    Kick {
        account: String,
    },
    /// `CharacterSelect`: "Disconnect offline trader." The character is stored and removed when it
    /// is an offline trader; `done` hears whether it was (and the final store is queued). Called
    /// before the character is loaded, so the load sees the trader's last state.
    ReleaseOfflineTrader {
        object_id: l2_util::ObjectId,
        done: Box<dyn FnOnce(bool) + Send>,
    },
    /// The login server kicked an account that has no connection: its offline traders are stored
    /// and deleted; `done` hears whether there was one (then the final store frees the account at
    /// the login server, D-52).
    ReleaseOfflineAccount {
        account: String,
        done: Box<dyn FnOnce(bool) + Send>,
    },
    Admin(AdminQuery),
    /// A path worker finished a search (M3-GEO-03). `requester` and `request_id` are the ones
    /// of the `l2_geo::PathRequest`; the movement code (M3-MOVE-01) matches them against the
    /// creature's current request and drops stale answers.
    PathFound {
        requester: u64,
        request_id: u64,
        result: l2_geo::PathResult,
    },
    /// The answer to `PersistCommand::PetRowRequest`: the rows of the collar, or `Err` when the
    /// read failed (the pet is then not summoned).
    PetRowLoaded {
        owner: l2_util::ObjectId,
        control_object_id: l2_util::ObjectId,
        result: Result<crate::summon::pet::PetRowData, ()>,
    },
    /// The answer to `PersistCommand::MailAttachmentsRequest`: the rows still in the mail, or `Err`
    /// when the read failed (the attachments are then not handed out, M6-SYNC-01).
    MailAttachmentsLoaded {
        player: l2_util::ObjectId,
        message_id: i32,
        result: Result<Vec<crate::player::ItemData>, ()>,
    },
    /// The answer to `PersistCommand::ClanWarehouseRequest`: the `CLANWH` rows of the clan, or
    /// `Err` when the read failed (the window is then not opened, M6-SYNC-01).
    ClanWarehouseLoaded {
        player: l2_util::ObjectId,
        clan_id: i32,
        result: Result<Vec<crate::player::ItemData>, ()>,
    },
    /// The answer to `PersistCommand::CrestIdsRequest`: the used crest ids, or `Err` when the read
    /// failed (the crest is then not created, M7-FU-03).
    CrestIdsLoaded {
        ticket: u32,
        result: Result<Vec<i32>, ()>,
    },
    /// The `INSERT` of a crest failed (the primary key is taken); the world retries once with a
    /// fresh read of the used ids (M7-FU-03).
    CrestInsertFailed {
        crest_id: i32,
    },
    /// The answer to `PersistCommand::ResidenceRereadRequest`: the residence's rows, or `Err` when
    /// the read failed (the follow-up `then` is then refused, M7-RES-01).
    ResidenceReread {
        result: Result<crate::residence::ResidenceRereadRows, ()>,
        then: crate::residence::ResidenceReadThen,
    },
    /// The answer to `PersistCommand::OfflinePetRowsRequest`: the pet items and inventory rows of
    /// an owner who is offline, or `Err` when the read failed (M6-FU-09).
    OfflinePetRowsLoaded {
        owner: l2_util::ObjectId,
        control_object_id: l2_util::ObjectId,
        result: Result<crate::summon::pet_offline::OfflinePetRows, ()>,
    },
    /// The answer to `PersistCommand::BlockListRequest`: the block list rows of the receiver, or
    /// `Err` when the read failed (the mail is then not sent, M6-FU-09).
    BlockListLoaded {
        sender: l2_util::ObjectId,
        owner: i32,
        result: Result<Vec<i32>, ()>,
    },
    /// The answer to `PersistCommand::Announcement(Insert)`: the id the database gave the row, or
    /// `Err` when the INSERT failed (`//announces add`, see `admin/announce.rs`).
    AnnouncementStored {
        ticket: u32,
        result: Result<i32, ()>,
    },
    /// The answer to `ControlRequest::ReloadHtml` (`//reload html`, see `admin/reload.rs`).
    HtmlReloaded {
        requester: l2_util::ObjectId,
        path: Option<String>,
        result: crate::admin::reload::HtmlReloadResult,
    },
    /// The answer to `ControlRequest::LoadHtmlFile` (`//loadhtml`, see `admin/page.rs`): the text
    /// of the file, `None` when there is no such html file.
    HtmlFileLoaded {
        requester: l2_util::ObjectId,
        path: String,
        content: Option<String>,
    },
    /// The answer to `ControlRequest::ReloadSpawns` (`//respawnall`, see `admin/spawn_admin.rs`):
    /// the spawn files, read again.
    SpawnsReloaded {
        spawns: std::sync::Arc<l2_data::SpawnData>,
    },
    /// The login server registered the game server under this name (Java
    /// `LoginServerThread._serverName`, shown by `//server_login`).
    LoginServerRegistered {
        name: String,
    },
    /// Installs the path search service (the pool starts after the world thread, so it cannot be
    /// given to the world before it runs).
    SetPathRequests(Box<dyn crate::movement::PathRequests>),
    /// Store everyone, tell their sessions, then call `done` and stop the world thread.
    Shutdown {
        done: Box<dyn FnOnce() + Send>,
    },
}

impl fmt::Debug for WorldCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnterWorld { session, player, .. } => {
                write!(f, "EnterWorld({session:?}, {})", player.name)
            }
            Self::Input { session, input } => write!(f, "Input({session:?}, {input:?})"),
            Self::Disconnected { session } => write!(f, "Disconnected({session:?})"),
            Self::Kick { account } => write!(f, "Kick({account})"),
            Self::ReleaseOfflineAccount { account, .. } => write!(f, "ReleaseOfflineAccount({account})"),
            Self::ReleaseOfflineTrader { object_id, .. } => write!(f, "ReleaseOfflineTrader({object_id})"),
            Self::Admin(query) => write!(f, "Admin({query:?})"),
            Self::PathFound {
                requester,
                request_id,
                result,
            } => write!(
                f,
                "PathFound({requester}, {request_id}, {})",
                if result.path.is_ok() { "path" } else { "no path" }
            ),
            Self::PetRowLoaded {
                owner,
                control_object_id,
                result,
            } => write!(
                f,
                "PetRowLoaded({owner}, {control_object_id}, {})",
                if result.is_ok() { "ok" } else { "failed" }
            ),
            Self::MailAttachmentsLoaded {
                player,
                message_id,
                result,
            } => write!(
                f,
                "MailAttachmentsLoaded({player}, {message_id}, {})",
                if result.is_ok() { "ok" } else { "failed" }
            ),
            Self::CrestIdsLoaded { ticket, result } => write!(
                f,
                "CrestIdsLoaded({ticket}, {})",
                if result.is_ok() { "ok" } else { "failed" }
            ),
            Self::CrestInsertFailed { crest_id } => write!(f, "CrestInsertFailed({crest_id})"),
            Self::ClanWarehouseLoaded {
                player,
                clan_id,
                result,
            } => write!(
                f,
                "ClanWarehouseLoaded({player}, {clan_id}, {})",
                if result.is_ok() { "ok" } else { "failed" }
            ),
            Self::ResidenceReread { result, then } => write!(
                f,
                "ResidenceReread({then:?}, {})",
                if result.is_ok() { "ok" } else { "failed" }
            ),
            Self::OfflinePetRowsLoaded {
                owner,
                control_object_id,
                result,
            } => write!(
                f,
                "OfflinePetRowsLoaded({owner}, {control_object_id}, {})",
                if result.is_ok() { "ok" } else { "failed" }
            ),
            Self::BlockListLoaded {
                sender,
                owner,
                result,
            } => write!(
                f,
                "BlockListLoaded({sender}, {owner}, {})",
                if result.is_ok() { "ok" } else { "failed" }
            ),
            Self::AnnouncementStored { ticket, result } => write!(
                f,
                "AnnouncementStored({ticket}, {})",
                if result.is_ok() { "ok" } else { "failed" }
            ),
            Self::HtmlReloaded {
                requester,
                path,
                result,
            } => {
                write!(f, "HtmlReloaded({requester}, {path:?}, {result:?})")
            }
            Self::HtmlFileLoaded {
                requester,
                path,
                content,
            } => write!(
                f,
                "HtmlFileLoaded({requester}, {path:?}, {})",
                if content.is_some() { "found" } else { "missing" }
            ),
            Self::SpawnsReloaded { spawns } => write!(f, "SpawnsReloaded({} spawns)", spawns.len()),
            Self::LoginServerRegistered { name } => write!(f, "LoginServerRegistered({name})"),
            Self::SetPathRequests(_) => f.write_str("SetPathRequests"),
            Self::Shutdown { .. } => f.write_str("Shutdown"),
        }
    }
}
