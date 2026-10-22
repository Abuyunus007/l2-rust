//! The post box: sending a mail with attachments or a payment request, reading, deleting,
//! collecting, rejecting and cancelling, the 15 day (12 hour for a payment request) expiry and
//! the new-mail notice at login (M6-MAIL-01).
// Java: managers/MailManager.java, taskmanagers/MessageDeletionTaskManager.java
// Java: network/holders/MailMessage.java, entity/itemcontainer/Mail.java
// Java: network/clientpackets/{RequestSendPost,RequestReceivedPost,RequestReceivedPostList,
//       RequestSentPost,RequestSentPostList,RequestPostAttachment,RequestRejectPostAttachment,
//       RequestCancelPostAttachment,RequestDeleteReceivedPost,RequestDeleteSentPost,
//       RequestPostItemList}.java
// Java: network/serverpackets/{ExShowReceivedPostList,ExShowSentPostList,ExReplyReceivedPost,
//       ExReplySentPost,ExReplyPostItemList,ExChangePostState,ExNoticePostArrived,
//       ExNoticePostSent}.java
// Java: data/sql/CharInfoTable.java (the names and access levels of the mail receivers)
//
// # Where the mails live
// Every mail is a [`MailMessage`] in the world ([`MailWorld::messages`]), read at startup like
// Java's `MailManager` constructor (`SELECT * FROM messages ORDER BY expiration`), together with
// its attachments: the `items` rows with `loc = 'MAIL'`, `owner_id` = the sender and `loc_data` =
// the message id. The attachments are a container like the warehouse ones
// ([`ContainerRef::Mail`](crate::ContainerRef)), so `World::move_item` moves items between the
// inventory and a mail and queues the rows as `PersistCommand::ItemRows` (D-39).
//
// NOTE(java-quirk): Java loads the attachments of a message when somebody first asks for them and
// drops them after 15 to 30 minutes (`AttachmentsUnloadTask`). That only saves memory, so they
// stay loaded here.
//
// # Rows (D-39)
// The Java GS reads and writes the same `messages` table, so the statements are Java's, in
// Java's order, as [`MailWrite`]s through `PersistCommand::Mail`: an `INSERT` with the flags
// spelled `'true'`/`'false'`, and one `UPDATE` per flag. A mail one server writes shows on the
// other after the next start (each loads the table once, like Java between two servers).
//
// # Order of the lists
// Java keeps the messages in a `ConcurrentHashMap<Integer, MailMessage>` and the lists
// (`getInbox`, `getOutbox`) iterate it, so the order is the map's (see
// `java_concurrent_map_order`), not the ids' or the dates'. [`MailWorld::order`] is the insertion
// order that function needs (the load order is `ORDER BY expiration`).
//
// # Left out on purpose (each has a comment where it belongs)
// - `BlockList.isInBlockList` (the ignore list is the friend list card's, `// M6-FRIEND:`).
// - The enchant scroll in use (`isEnchanting`, `// M6-ENCHANT:`), `player.setMultiSell(null)`
//   (the multisell state is not kept), the flood protector `canPerformTransaction` of the
//   attachment packets (no flood protectors yet; `canSendMail` is here, see
//   [`MailSettings::send_interval_ticks`]).
// - `ItemProcessType` logging (`LogItems`), the GM audit.
// - `CustomMailManager` and the community board mail (`MailBoard`): scripts of a later card.
//   `MailMessage::new_system` is the constructor they need.
// - Characters created after the server started and not entered yet: `CharInfoTable` finds them
//   with a database query; the world has no database, so they are known once they entered the
//   world ([`World::mail_remember_player`]).

mod expire;
mod names;
mod packets;
mod receive;
mod send;
mod table;

use std::collections::BTreeMap;

use l2_protocol_game::client::RequestSendPost;
use l2_util::ObjectId;

use crate::player::ItemData;
use crate::timer::TimerId;

pub use names::MailCharacter;

/// `MailMessage.EXPIRATION` (hours): 15 days.
pub const EXPIRATION_HOURS: i64 = 360;
/// `MailMessage.COD_EXPIRATION` (hours).
pub const COD_EXPIRATION_HOURS: i64 = 12;
/// `MessageSenderType.PLAYER.ordinal()`.
pub const SENDER_TYPE_PLAYER: i32 = 0;
/// `MessageSenderType.NEWS.ordinal()`.
pub const SENDER_TYPE_NEWS: i32 = 1;
/// `MessageSenderType.NONE.ordinal()`.
pub const SENDER_TYPE_NONE: i32 = 2;
/// `MessageSenderType.ALEGRIA.ordinal()`.
pub const SENDER_TYPE_ALEGRIA: i32 = 3;

/// The values of `General.ini` and `FloodProtector.ini` the mail reads. `Default` is the Java
/// default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailSettings {
    /// `GeneralConfig.ALLOW_MAIL` (`AllowMail`).
    pub allow_mail: bool,
    /// `GeneralConfig.ALLOW_ATTACHMENTS` (`AllowAttachments`).
    pub allow_attachments: bool,
    /// `FloodProtectorSendMailInterval`, in game ticks of 100 ms: the time between two mails of
    /// a player (`getFloodProtectors().canSendMail()`).
    pub send_interval_ticks: i32,
}

impl Default for MailSettings {
    fn default() -> Self {
        Self {
            allow_mail: true,
            allow_attachments: true,
            send_interval_ticks: 100,
        }
    }
}

/// Java `MailMessage` with its `Mail` container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailMessage {
    /// `_messageId`.
    pub id: i32,
    /// `_senderId`: -1 for a mail the system sent.
    pub sender_id: i32,
    pub receiver_id: i32,
    pub subject: String,
    pub content: String,
    /// `_expiration`: epoch milliseconds.
    pub expiration: i64,
    /// `_reqAdena`: the payment a cash on delivery mail asks for.
    pub req_adena: i64,
    /// `_hasAttachments`.
    pub has_attachments: bool,
    /// `_unread`.
    pub unread: bool,
    /// `_returned`.
    pub returned: bool,
    /// `_sendBySystem`: a `MessageSenderType` ordinal.
    pub send_by_system: i32,
    pub deleted_by_sender: bool,
    pub deleted_by_receiver: bool,
    /// The items of the `Mail` container (`loc = 'MAIL'`, `loc_data` = [`id`](Self::id)).
    pub attachments: Vec<ItemData>,
}

impl MailMessage {
    /// Java `MailMessage(senderId, receiverId, isCod, subject, text, reqAdena)`: a mail a player
    /// writes. `now_ms` is `System.currentTimeMillis()`.
    pub fn new_player_mail(
        id: i32,
        sender_id: i32,
        receiver_id: i32,
        is_cod: bool,
        subject: &str,
        text: &str,
        req_adena: i64,
        now_ms: i64,
    ) -> Self {
        let hours = if is_cod { COD_EXPIRATION_HOURS } else { EXPIRATION_HOURS };
        Self {
            id,
            sender_id,
            receiver_id,
            subject: subject.to_owned(),
            content: text.to_owned(),
            expiration: now_ms.wrapping_add(hours * 3_600_000),
            req_adena,
            has_attachments: false,
            unread: true,
            returned: false,
            send_by_system: SENDER_TYPE_PLAYER,
            deleted_by_sender: false,
            deleted_by_receiver: false,
            attachments: Vec::new(),
        }
    }

    /// Java `MailMessage(receiverId, subject, content, sendBySystem)`: a system mail (the sender
    /// id is -1 and the sender already "deleted" it).
    pub fn new_system(
        id: i32,
        receiver_id: i32,
        subject: &str,
        content: &str,
        send_by_system: i32,
        now_ms: i64,
    ) -> Self {
        Self {
            id,
            sender_id: -1,
            receiver_id,
            subject: subject.to_owned(),
            content: content.to_owned(),
            expiration: now_ms.wrapping_add(EXPIRATION_HOURS * 3_600_000),
            req_adena: 0,
            has_attachments: false,
            unread: true,
            returned: false,
            send_by_system,
            deleted_by_sender: true,
            deleted_by_receiver: false,
            attachments: Vec::new(),
        }
    }

    /// Java `MailMessage(MailMessage msg)`: the mail that gives the attachments of `msg` back to
    /// its sender. The attachments move to the new mail (`msg.removeAttachments()`,
    /// `setNewMessageId`), `msg` keeps none.
    // NOTE(java-quirk): the returned mail has `NONE` as the sender type (2), so the client shows
    // "****" as its sender and it cannot be returned again.
    pub fn new_returned(id: i32, original: &mut MailMessage, now_ms: i64) -> Self {
        let mut attachments = std::mem::take(&mut original.attachments);
        original.has_attachments = false;
        for item in &mut attachments {
            item.loc_data = id;
        }
        Self {
            id,
            sender_id: original.sender_id,
            receiver_id: original.sender_id,
            subject: String::new(),
            content: String::new(),
            expiration: now_ms.wrapping_add(EXPIRATION_HOURS * 3_600_000),
            req_adena: 0,
            has_attachments: true,
            unread: true,
            returned: true,
            send_by_system: SENDER_TYPE_NONE,
            deleted_by_sender: true,
            deleted_by_receiver: false,
            attachments,
        }
    }

    /// Java `isLocked()`: the mail asks for a payment.
    pub fn is_locked(&self) -> bool {
        self.req_adena > 0
    }

    /// Java `getExpirationSeconds()`.
    pub fn expiration_seconds(&self) -> i32 {
        (self.expiration / 1000) as i32
    }
}

/// What the startup reads for the post box: `MailManager`'s and `CharInfoTable`'s constructors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailLoad {
    /// `SELECT * FROM messages ORDER BY expiration`, each with its attachments.
    pub messages: Vec<MailMessage>,
    /// `SELECT charId, char_name, accesslevel FROM characters`.
    pub characters: Vec<MailCharacter>,
}

/// The mail packets as the session hands them to the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailInput {
    /// Java `RequestPostItemList`.
    PostItemList,
    /// Java `RequestSendPost`.
    Send(RequestSendPost),
    /// Java `RequestReceivedPostList`.
    ReceivedPostList,
    /// Java `RequestSentPostList`.
    SentPostList,
    /// Java `RequestReceivedPost`.
    ReceivedPost { msg_id: i32 },
    /// Java `RequestSentPost`.
    SentPost { msg_id: i32 },
    /// Java `RequestPostAttachment`.
    PostAttachment { msg_id: i32 },
    /// Java `RequestRejectPostAttachment`.
    RejectPostAttachment { msg_id: i32 },
    /// Java `RequestCancelPostAttachment`.
    CancelPostAttachment { msg_id: i32 },
    /// Java `RequestDeleteReceivedPost`; `None` is a list Java drops.
    DeleteReceivedPost { msg_ids: Option<Vec<i32>> },
    /// Java `RequestDeleteSentPost`; `None` is a list Java drops.
    DeleteSentPost { msg_ids: Option<Vec<i32>> },
}

/// One write of the `messages` table with Java's statement (`PersistCommand::Mail`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailWrite {
    /// `MailManager.sendMessage`: `INSERT INTO messages ...`.
    Insert(Box<MailMessage>),
    /// `markAsReadInDb`.
    MarkRead(i32),
    /// `markAsDeletedBySenderInDb`.
    MarkDeletedBySender(i32),
    /// `markAsDeletedByReceiverInDb`.
    MarkDeletedByReceiver(i32),
    /// `removeAttachmentsInDb`.
    RemoveAttachments(i32),
    /// `deleteMessageInDb` (the database half).
    Delete(i32),
}

/// The timers of `TimerKind::Mail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailTimer {
    /// Java `MessageDeletionTaskManager.run`, every 10 seconds.
    Deletion,
}

/// The post box and the names it needs (see the module notes).
#[derive(Default)]
pub(crate) struct MailWorld {
    pub settings: MailSettings,
    /// `MailManager._messages`.
    pub messages: BTreeMap<i32, MailMessage>,
    /// The ids of `messages` in the order they were put in the map.
    pub order: Vec<i32>,
    /// A mail being written: it has its `Mail` container before `sendMessage` puts it in the map
    /// (`RequestSendPost.removeItems`). A draft that is never sent is dropped, its rows stay in
    /// the table like in Java.
    pub drafts: BTreeMap<i32, MailMessage>,
    /// `CharInfoTable._names` and `_accessLevels`.
    pub names: BTreeMap<i32, (String, i32)>,
    /// The ids by lowercase name (`getIdByName` ignores the case).
    pub ids_by_name: BTreeMap<String, i32>,
    /// The game tick each player may send the next mail at (the flood protector `canSendMail`).
    pub next_send_tick: BTreeMap<ObjectId, i32>,
    /// The timer of the 10 s deletion task.
    pub deletion_timer: Option<TimerId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    #[test]
    fn a_player_mail_lives_15_days_and_a_payment_request_12_hours() {
        let mail = MailMessage::new_player_mail(7, 1, 2, false, "s", "t", 0, NOW);
        assert_eq!(mail.expiration, NOW + 360 * 3_600_000);
        let cod = MailMessage::new_player_mail(8, 1, 2, true, "s", "t", 500, NOW);
        assert_eq!(cod.expiration, NOW + 12 * 3_600_000);
        assert!(cod.is_locked());
        assert!(!mail.is_locked());
        assert!(mail.unread && !mail.returned && !mail.has_attachments);
        assert_eq!((mail.deleted_by_sender, mail.deleted_by_receiver), (false, false));
        assert_eq!(mail.send_by_system, 0);
    }

    #[test]
    fn a_system_mail_comes_from_nobody_and_is_gone_for_the_sender() {
        let mail = MailMessage::new_system(9, 2, "News", "Text", SENDER_TYPE_NEWS, NOW);
        assert_eq!(mail.sender_id, -1);
        assert!(mail.deleted_by_sender);
        assert!(!mail.deleted_by_receiver);
        assert_eq!(mail.send_by_system, 1);
        assert_eq!(mail.expiration, NOW + 360 * 3_600_000);
        assert_eq!(mail.req_adena, 0);
    }

    #[test]
    fn a_returned_mail_takes_the_attachments_and_goes_to_the_sender() {
        let item = ItemData {
            object_id: ObjectId(100),
            item_id: 57,
            count: 10,
            loc: "MAIL".into(),
            loc_data: 7,
            ..Default::default()
        };
        let mut original = MailMessage::new_player_mail(7, 1, 2, false, "s", "t", 0, NOW);
        original.has_attachments = true;
        original.attachments = vec![item];
        let returned = MailMessage::new_returned(11, &mut original, NOW);
        assert!(!original.has_attachments);
        assert!(original.attachments.is_empty());
        assert_eq!((returned.sender_id, returned.receiver_id), (1, 1));
        assert!(returned.has_attachments && returned.returned && returned.unread);
        assert!(returned.deleted_by_sender && !returned.deleted_by_receiver);
        assert_eq!((returned.subject.as_str(), returned.content.as_str()), ("", ""));
        // NOTE(java-quirk): `MessageSenderType.NONE`.
        assert_eq!(returned.send_by_system, 2);
        assert_eq!(returned.attachments.len(), 1);
        assert_eq!(returned.attachments[0].loc_data, 11);
        assert_eq!(returned.expiration, NOW + 360 * 3_600_000);
    }
}
