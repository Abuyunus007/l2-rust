//! Server packets of the mail (M6-MAIL-01): the inbox and outbox lists, a mail opened, the notices
//! and the state changes.
//!
//! Values only, like the other item packets: the world resolves the names, counts the booleans and
//! builds the [`ItemInfo`]s (Java reads them from the `MailMessage` and its `Mail` container).
//! Every Java `writeInt(boolean)` is a `bool` here, written as 1 or 0.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{ItemInfo, read_item, write_item};
use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// `MailMessage.DELETED`: the state `ExChangePostState` reports for a mail that is gone.
pub const POST_STATE_DELETED: i32 = 0;
/// `MailMessage.READED`.
pub const POST_STATE_READED: i32 = 1;
/// `MailMessage.REJECTED`.
pub const POST_STATE_REJECTED: i32 = 2;

fn read_bool(r: &mut PacketReader) -> Result<bool, PacketError> {
    Ok(r.i32()? != 0)
}

// Java: gameserver/network/serverpackets/ExNoticePostArrived.java
/// A new mail arrived, or unread mail is waiting at login (`EX_NOTICE_POST_ARRIVED`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExNoticePostArrived {
    /// The envelope animation of the client.
    pub show_animation: bool,
}

impl ExNoticePostArrived {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExNoticePostArrived;
}

impl Encode for ExNoticePostArrived {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(i32::from(self.show_animation));
    }
}

impl Decode for ExNoticePostArrived {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            show_animation: read_bool(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExNoticePostSent.java
/// The mail was sent (`EX_NOTICE_POST_SENT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExNoticePostSent {
    pub show_animation: bool,
}

impl ExNoticePostSent {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExNoticePostSent;
}

impl Encode for ExNoticePostSent {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(i32::from(self.show_animation));
    }
}

impl Decode for ExNoticePostSent {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            show_animation: read_bool(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExMailArrived.java
/// `EX_MAIL_ARRIVED`: nothing in the Java server sends it, but the class exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExMailArrived;

impl ExMailArrived {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExMailArrived;
}

impl Encode for ExMailArrived {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for ExMailArrived {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExChangePostState.java
/// Mails changed state (`EX_CHANGE_POST_STATE`): read, rejected or deleted. One state for all the
/// listed mails; Java writes it after each id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExChangePostState {
    /// `true` for the inbox, `false` for the outbox.
    pub received_board: bool,
    pub msg_ids: Vec<i32>,
    /// One of the `POST_STATE_*` values.
    pub change_id: i32,
}

impl ExChangePostState {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExChangePostState;
}

impl Encode for ExChangePostState {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(i32::from(self.received_board));
        w.i32(self.msg_ids.len() as i32);
        for id in &self.msg_ids {
            w.i32(*id).i32(self.change_id);
        }
    }
}

impl Decode for ExChangePostState {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let received_board = read_bool(r)?;
        let count = read_count(r, "post count")?;
        let mut msg_ids = Vec::new();
        let mut change_id = 0;
        for _ in 0..count {
            msg_ids.push(r.i32()?);
            change_id = r.i32()?;
        }
        Ok(Self {
            received_board,
            msg_ids,
            change_id,
        })
    }
}

// Java: gameserver/network/serverpackets/ExShowReceivedPostList.java
/// One mail of the inbox list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedPostEntry {
    pub msg_id: i32,
    pub subject: String,
    /// `getSenderName()`: `****` for a mail the system sent.
    pub sender_name: String,
    /// `isLocked()`: the mail asks for a payment.
    pub locked: bool,
    /// `getExpirationSeconds()`.
    pub expiration: i32,
    pub unread: bool,
    pub has_attachments: bool,
    pub returned: bool,
    /// `getSendBySystem()`.
    pub send_by_system: i32,
}

/// The inbox (`EX_SHOW_RECEIVED_POST_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowReceivedPostList {
    /// `System.currentTimeMillis() / 1000`.
    pub time: i32,
    pub mails: Vec<ReceivedPostEntry>,
}

impl ExShowReceivedPostList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowReceivedPostList;
}

impl Encode for ExShowReceivedPostList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.time);
        w.i32(self.mails.len() as i32);
        for mail in &self.mails {
            w.i32(mail.msg_id);
            w.string(&mail.subject);
            w.string(&mail.sender_name);
            w.i32(i32::from(mail.locked));
            w.i32(mail.expiration);
            w.i32(i32::from(mail.unread));
            // Java: constant 1.
            w.i32(1);
            w.i32(i32::from(mail.has_attachments));
            w.i32(i32::from(mail.returned));
            w.i32(mail.send_by_system);
            w.i32(0);
        }
    }
}

impl Decode for ExShowReceivedPostList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let time = r.i32()?;
        let count = read_count(r, "mail count")?;
        let mut mails = Vec::new();
        for _ in 0..count {
            let msg_id = r.i32()?;
            let subject = r.string();
            let sender_name = r.string();
            let locked = read_bool(r)?;
            let expiration = r.i32()?;
            let unread = read_bool(r)?;
            r.skip(4)?;
            let has_attachments = read_bool(r)?;
            let returned = read_bool(r)?;
            let send_by_system = r.i32()?;
            r.skip(4)?;
            mails.push(ReceivedPostEntry {
                msg_id,
                subject,
                sender_name,
                locked,
                expiration,
                unread,
                has_attachments,
                returned,
                send_by_system,
            });
        }
        Ok(Self { time, mails })
    }
}

// Java: gameserver/network/serverpackets/ExShowSentPostList.java
/// One mail of the outbox list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentPostEntry {
    pub msg_id: i32,
    pub subject: String,
    /// `getReceiverName()`.
    pub receiver_name: String,
    pub locked: bool,
    /// `getExpirationSeconds()`.
    pub expiration: i32,
    pub unread: bool,
    pub has_attachments: bool,
}

/// The outbox (`EX_SHOW_SENT_POST_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowSentPostList {
    /// `System.currentTimeMillis() / 1000`.
    pub time: i32,
    pub mails: Vec<SentPostEntry>,
}

impl ExShowSentPostList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowSentPostList;
}

impl Encode for ExShowSentPostList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.time);
        w.i32(self.mails.len() as i32);
        for mail in &self.mails {
            w.i32(mail.msg_id);
            w.string(&mail.subject);
            w.string(&mail.receiver_name);
            w.i32(i32::from(mail.locked));
            w.i32(mail.expiration);
            w.i32(i32::from(mail.unread));
            // Java: constant 1.
            w.i32(1);
            w.i32(i32::from(mail.has_attachments));
        }
    }
}

impl Decode for ExShowSentPostList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let time = r.i32()?;
        let count = read_count(r, "mail count")?;
        let mut mails = Vec::new();
        for _ in 0..count {
            let msg_id = r.i32()?;
            let subject = r.string();
            let receiver_name = r.string();
            let locked = read_bool(r)?;
            let expiration = r.i32()?;
            let unread = read_bool(r)?;
            r.skip(4)?;
            let has_attachments = read_bool(r)?;
            mails.push(SentPostEntry {
                msg_id,
                subject,
                receiver_name,
                locked,
                expiration,
                unread,
                has_attachments,
            });
        }
        Ok(Self { time, mails })
    }
}

/// Writes the attachment block both reply packets share: the count, then each item followed by
/// its object id again.
fn write_attachments(w: &mut PacketWriter, items: &[ItemInfo]) {
    w.i32(items.len() as i32);
    for item in items {
        write_item(w, item);
        w.i32(item.object_id);
    }
}

fn read_attachments(r: &mut PacketReader, count: usize) -> Result<Vec<ItemInfo>, PacketError> {
    let mut items = Vec::new();
    for _ in 0..count {
        let item = read_item(r)?;
        r.skip(4)?;
        items.push(item);
    }
    Ok(items)
}

// Java: gameserver/network/serverpackets/ExReplyReceivedPost.java
/// A mail of the inbox opened (`EX_REPLY_RECEIVED_POST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExReplyReceivedPost {
    pub msg_id: i32,
    pub locked: bool,
    pub sender_name: String,
    pub subject: String,
    pub content: String,
    pub items: Vec<ItemInfo>,
    pub req_adena: i64,
    /// `hasAttachments()`: Java writes the flag, not whether `items` has anything.
    pub has_attachments: bool,
    pub send_by_system: i32,
}

impl ExReplyReceivedPost {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExReplyReceivedPost;
}

impl Encode for ExReplyReceivedPost {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.msg_id);
        w.i32(i32::from(self.locked));
        // "Unknown" in Java.
        w.i32(0);
        w.string(&self.sender_name);
        w.string(&self.subject);
        w.string(&self.content);
        write_attachments(w, &self.items);
        w.i64(self.req_adena);
        w.i32(i32::from(self.has_attachments));
        w.i32(self.send_by_system);
    }
}

impl Decode for ExReplyReceivedPost {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let msg_id = r.i32()?;
        let locked = read_bool(r)?;
        r.skip(4)?;
        let sender_name = r.string();
        let subject = r.string();
        let content = r.string();
        let count = read_count(r, "attachment count")?;
        let items = read_attachments(r, count)?;
        let req_adena = r.i64()?;
        let has_attachments = read_bool(r)?;
        let send_by_system = r.i32()?;
        Ok(Self {
            msg_id,
            locked,
            sender_name,
            subject,
            content,
            items,
            req_adena,
            has_attachments,
            send_by_system,
        })
    }
}

// Java: gameserver/network/serverpackets/ExReplySentPost.java
/// A mail of the outbox opened (`EX_REPLY_SENT_POST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExReplySentPost {
    pub msg_id: i32,
    pub locked: bool,
    pub receiver_name: String,
    pub subject: String,
    pub content: String,
    pub items: Vec<ItemInfo>,
    pub req_adena: i64,
    /// NOTE(java-quirk): Java writes it only after a non-empty item list; the decoder gives 0
    /// when there are no items.
    pub send_by_system: i32,
}

impl ExReplySentPost {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExReplySentPost;
}

impl Encode for ExReplySentPost {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.msg_id);
        w.i32(i32::from(self.locked));
        w.string(&self.receiver_name);
        w.string(&self.subject);
        w.string(&self.content);
        write_attachments(w, &self.items);
        w.i64(self.req_adena);
        if !self.items.is_empty() {
            w.i32(self.send_by_system);
        }
    }
}

impl Decode for ExReplySentPost {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let msg_id = r.i32()?;
        let locked = read_bool(r)?;
        let receiver_name = r.string();
        let subject = r.string();
        let content = r.string();
        let count = read_count(r, "attachment count")?;
        let items = read_attachments(r, count)?;
        let req_adena = r.i64()?;
        let send_by_system = if items.is_empty() { 0 } else { r.i32()? };
        Ok(Self {
            msg_id,
            locked,
            receiver_name,
            subject,
            content,
            items,
            req_adena,
            send_by_system,
        })
    }
}

// Java: gameserver/network/serverpackets/ExReplyPostItemList.java
/// The items the player can attach to a new mail (`EX_REPLY_POST_ITEM_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExReplyPostItemList {
    pub items: Vec<ItemInfo>,
}

impl ExReplyPostItemList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExReplyPostItemList;
}

impl Encode for ExReplyPostItemList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.items.len() as i32);
        for item in &self.items {
            write_item(w, item);
        }
    }
}

impl Decode for ExReplyPostItemList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "item count")?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(read_item(r)?);
        }
        Ok(Self { items })
    }
}
