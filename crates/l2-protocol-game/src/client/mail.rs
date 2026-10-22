//! Client packets of the mail (the post box): writing, reading, deleting, collecting and
//! returning a mail (M6-MAIL-01). All of them are ex packets (`0xD0` + `u16`).
//!
//! Like the warehouse lists, `RequestSendPost` and the two delete packets keep Java's "bad
//! packet" rule: when the counts or the length do not fit, Java stops reading and the handler goes
//! on with what it has. The limit that comes from the configuration (`MaxItemInPacket`) is the
//! world's to compare.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_ex_opcode;
use crate::generated::opcodes::ExClientOpcode;

/// Bytes per attached item in `RequestSendPost`: an int object id and a long count.
pub const ATTACHMENT_BATCH_LENGTH: i32 = 12;

// Java: gameserver/network/clientpackets/RequestPostItemList.java
client_unit_packet!(
    /// The player opens the "attach items" window of a new mail (`REQUEST_POST_ITEM_LIST`,
    /// `0xD0` 0x0065).
    RequestPostItemList,
    ex RequestPostItemList
);

// Java: gameserver/network/clientpackets/RequestReceivedPostList.java
client_unit_packet!(
    /// The player opens the inbox (`REQUEST_RECEIVED_POST_LIST`, `0xD0` 0x0067).
    RequestReceivedPostList,
    ex RequestReceivedPostList
);

// Java: gameserver/network/clientpackets/RequestSentPostList.java
client_unit_packet!(
    /// The player opens the outbox (`REQUEST_SENT_POST_LIST`, `0xD0` 0x006C).
    RequestSentPostList,
    ex RequestSentPostList
);

// Java: gameserver/network/clientpackets/RequestReceivedPost.java
client_packet!(
    /// The player opens a mail of the inbox (`REQUEST_RECEIVED_POST`, `0xD0` 0x0069).
    RequestReceivedPost,
    ex RequestReceivedPost {
        msg_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestSentPost.java
client_packet!(
    /// The player opens a mail of the outbox (`REQUEST_SENT_POST`, `0xD0` 0x006E).
    RequestSentPost,
    ex RequestSentPost {
        msg_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestPostAttachment.java
client_packet!(
    /// The player takes the attachments of a mail it received (`REQUEST_POST_ATTACHMENT`,
    /// `0xD0` 0x006A).
    RequestPostAttachment,
    ex RequestPostAttachment {
        msg_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestRejectPostAttachment.java
client_packet!(
    /// The player sends the attachments of a mail it received back
    /// (`REQUEST_REJECT_POST_ATTACHMENT`, `0xD0` 0x006B).
    RequestRejectPostAttachment,
    ex RequestRejectPostAttachment {
        msg_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestCancelPostAttachment.java
client_packet!(
    /// The sender takes back a mail whose attachments the receiver has not taken
    /// (`REQUEST_CANCEL_SENT_POST`, `0xD0` 0x006F; the Java class is `RequestCancelPostAttachment`).
    RequestCancelPostAttachment,
    ex RequestCancelSentPost {
        msg_id: i32,
    }
);

/// One attached item of a [`RequestSendPost`] (Java's private `AttachmentItem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostAttachment {
    /// The object id of the item in the inventory.
    pub object_id: i32,
    pub count: i64,
}

// Java: gameserver/network/clientpackets/RequestSendPost.java
/// The player sends a mail (`REQUEST_SEND_POST`, `0xD0` 0x0066).
///
/// NOTE(java-quirk): `readImpl` returns early on a bad item count, a length that does not fit or
/// an item below 1, and `runImpl` still sends the mail, without attachments. That is `items:
/// None` with `req_adena` 0 here (a negative `req_adena` also drops the items but keeps the
/// value, and the handler then returns). A count above `MaxItemInPacket` is the world's check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSendPost {
    /// The name of the receiver.
    pub receiver: String,
    /// `readInt() != 0`: cash on delivery.
    pub is_cod: bool,
    pub subject: String,
    pub text: String,
    /// `None`: Java's `_items == null`.
    pub items: Option<Vec<PostAttachment>>,
    /// The adena the receiver has to pay (cash on delivery).
    pub req_adena: i64,
}

impl RequestSendPost {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestSendPost;
}

impl Encode for RequestSendPost {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.string(&self.receiver);
        w.i32(i32::from(self.is_cod));
        w.string(&self.subject);
        w.string(&self.text);
        w.i32(self.items.as_ref().map_or(0, |v| v.len() as i32));
        for item in self.items.iter().flatten() {
            w.i32(item.object_id).i64(item.count);
        }
        w.i64(self.req_adena);
    }
}

impl Decode for RequestSendPost {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let receiver = r.string();
        let is_cod = r.i32()? != 0;
        let subject = r.string();
        let text = r.string();
        let mut packet = Self {
            receiver,
            is_cod,
            subject,
            text,
            items: None,
            req_adena: 0,
        };

        let attach_count = r.i32()?;
        // Java computes `(attachCount * BATCH_LENGTH) + 8` in `int`.
        let expected = attach_count.wrapping_mul(ATTACHMENT_BATCH_LENGTH).wrapping_add(8);
        if attach_count < 0 || i64::from(expected) != r.remaining() as i64 {
            return Ok(packet);
        }
        if attach_count > 0 {
            let mut items = Vec::new();
            for _ in 0..attach_count {
                let item = PostAttachment {
                    object_id: r.i32()?,
                    count: r.i64()?,
                };
                if item.object_id < 1 || item.count < 1 {
                    return Ok(packet);
                }
                items.push(item);
            }
            packet.items = Some(items);
        }
        packet.req_adena = r.i64()?;
        if packet.req_adena < 0 {
            packet.items = None;
        }
        Ok(packet)
    }
}

/// Java's shared reading of the two delete packets: `count >= 1` and `count * 4 == remaining()`,
/// else the list is dropped (`None`). A count above `MaxItemInPacket` is the world's check.
fn read_message_ids(r: &mut PacketReader) -> Result<Option<Vec<i32>>, PacketError> {
    let count = r.i32()?;
    if count <= 0 || i64::from(count.wrapping_mul(4)) != r.remaining() as i64 {
        return Ok(None);
    }
    let mut ids = Vec::new();
    for _ in 0..count {
        ids.push(r.i32()?);
    }
    Ok(Some(ids))
}

/// Writes a message id list: `None` (a list Java drops) is the count `0`, which is dropped the
/// same way.
fn write_message_ids(w: &mut PacketWriter, ids: &Option<Vec<i32>>) {
    w.i32(ids.as_ref().map_or(0, |v| v.len() as i32));
    for id in ids.iter().flatten() {
        w.i32(*id);
    }
}

// Java: gameserver/network/clientpackets/RequestDeleteReceivedPost.java
/// The player deletes mails of the inbox (`REQUEST_DELETE_RECEIVED_POST`, `0xD0` 0x0068).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestDeleteReceivedPost {
    /// `None`: Java's `_msgIds == null`.
    pub msg_ids: Option<Vec<i32>>,
}

impl RequestDeleteReceivedPost {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestDeleteReceivedPost;
}

impl Encode for RequestDeleteReceivedPost {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        write_message_ids(w, &self.msg_ids);
    }
}

impl Decode for RequestDeleteReceivedPost {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            msg_ids: read_message_ids(r)?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestDeleteSentPost.java
/// The player deletes mails of the outbox (`REQUEST_DELETE_SENT_POST`, `0xD0` 0x006D).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestDeleteSentPost {
    /// `None`: Java's `_msgIds == null`.
    pub msg_ids: Option<Vec<i32>>,
}

impl RequestDeleteSentPost {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestDeleteSentPost;
}

impl Encode for RequestDeleteSentPost {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        write_message_ids(w, &self.msg_ids);
    }
}

impl Decode for RequestDeleteSentPost {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            msg_ids: read_message_ids(r)?,
        })
    }
}
