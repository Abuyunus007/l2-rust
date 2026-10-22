//! The packets that show the post box: the lists, a mail opened, the items that can be attached.
// Java: network/serverpackets/{ExShowReceivedPostList,ExShowSentPostList,ExReplyReceivedPost,
// Java: ExReplySentPost,ExReplyPostItemList}.java, network/holders/MailMessage.java
// Java: (getSenderName, getReceiverName)

use l2_protocol_game::server::{
    ExReplyPostItemList, ExReplyReceivedPost, ExReplySentPost, ExShowReceivedPostList, ExShowSentPostList,
    ItemInfo, ReceivedPostEntry, SentPostEntry,
};
use l2_util::ObjectId;
use tracing::warn;

use super::MailMessage;
use crate::world::{World, unix_ms_now};

/// What `getSenderName` gives for a mail the system sent.
const SYSTEM_SENDER_NAME: &str = "****";

impl World {
    /// Java `MailMessage.getSenderName()`.
    fn mail_sender_name(&self, msg: &MailMessage) -> String {
        if msg.send_by_system != 0 {
            return SYSTEM_SENDER_NAME.to_owned();
        }
        self.mail_char_name(msg.sender_id).unwrap_or_default()
    }

    /// Java `MailMessage.getReceiverName()`.
    fn mail_receiver_name(&self, msg: &MailMessage) -> String {
        self.mail_char_name(msg.receiver_id).unwrap_or_default()
    }

    /// The items of a mail the way `ExReplyReceivedPost` and `ExReplySentPost` list them: empty
    /// (with a warning) when the mail says it has attachments and the container is empty.
    fn mail_attachment_infos(&self, msg: &MailMessage) -> Vec<ItemInfo> {
        if !msg.has_attachments {
            return Vec::new();
        }
        if msg.attachments.is_empty() {
            warn!("Message {} has attachments but itemcontainer is empty.", msg.id);
            return Vec::new();
        }
        let now_ms = unix_ms_now();
        msg.attachments
            .iter()
            .map(|item| self.warehouse_item_info(item, now_ms))
            .collect()
    }

    /// Java `new ExShowReceivedPostList(objectId)`.
    pub(crate) fn mail_received_list(&self, object_id: i32) -> ExShowReceivedPostList {
        ExShowReceivedPostList {
            time: (unix_ms_now() / 1000) as i32,
            mails: self
                .mail_inbox(object_id)
                .into_iter()
                .map(|msg| ReceivedPostEntry {
                    msg_id: msg.id,
                    subject: msg.subject.clone(),
                    sender_name: self.mail_sender_name(msg),
                    locked: msg.is_locked(),
                    expiration: msg.expiration_seconds(),
                    unread: msg.unread,
                    has_attachments: msg.has_attachments,
                    returned: msg.returned,
                    send_by_system: msg.send_by_system,
                })
                .collect(),
        }
    }

    /// Java `new ExShowSentPostList(objectId)`.
    pub(crate) fn mail_sent_list(&self, object_id: i32) -> ExShowSentPostList {
        ExShowSentPostList {
            time: (unix_ms_now() / 1000) as i32,
            mails: self
                .mail_outbox(object_id)
                .into_iter()
                .map(|msg| SentPostEntry {
                    msg_id: msg.id,
                    subject: msg.subject.clone(),
                    receiver_name: self.mail_receiver_name(msg),
                    locked: msg.is_locked(),
                    expiration: msg.expiration_seconds(),
                    unread: msg.unread,
                    has_attachments: msg.has_attachments,
                })
                .collect(),
        }
    }

    /// Java `new ExReplyReceivedPost(msg)`.
    pub(crate) fn mail_reply_received(&self, msg: &MailMessage) -> ExReplyReceivedPost {
        ExReplyReceivedPost {
            msg_id: msg.id,
            locked: msg.is_locked(),
            sender_name: self.mail_sender_name(msg),
            subject: msg.subject.clone(),
            content: msg.content.clone(),
            items: self.mail_attachment_infos(msg),
            req_adena: msg.req_adena,
            has_attachments: msg.has_attachments,
            send_by_system: msg.send_by_system,
        }
    }

    /// Java `new ExReplySentPost(msg)`.
    pub(crate) fn mail_reply_sent(&self, msg: &MailMessage) -> ExReplySentPost {
        ExReplySentPost {
            msg_id: msg.id,
            locked: msg.is_locked(),
            receiver_name: self.mail_receiver_name(msg),
            subject: msg.subject.clone(),
            content: msg.content.clone(),
            items: self.mail_attachment_infos(msg),
            req_adena: msg.req_adena,
            send_by_system: msg.send_by_system,
        }
    }

    /// Java `new ExReplyPostItemList(player)`: `getAvailableItems(true, false, false)`.
    pub(crate) fn mail_post_item_list(&self, player: ObjectId) -> ExReplyPostItemList {
        let now_ms = unix_ms_now();
        ExReplyPostItemList {
            items: self
                .available_items(player, true, false, false)
                .iter()
                .map(|item| self.warehouse_item_info(item, now_ms))
                .collect(),
        }
    }
}
