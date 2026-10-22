//! `MailManager`: the table of mails, who has what in the inbox and outbox, and the changes with
//! their rows.
// Java: managers/MailManager.java, network/holders/MailMessage.java (markAsRead,
// Java: setDeletedBySender, setDeletedByReceiver, removeAttachments)

use l2_protocol_game::server::ExNoticePostArrived;
use l2_util::ObjectId;

use super::{MailMessage, MailWrite};
use crate::inventory::java_concurrent_map_order;
use crate::outbox::PersistCommand;
use crate::player::ItemData;
use crate::world::World;

impl World {
    /// Java `MailManager.getMessage(msgId)`.
    pub(crate) fn mail_message(&self, id: i32) -> Option<&MailMessage> {
        self.mail.messages.get(&id)
    }

    /// The `Mail` container of a message (a draft included), see [`ContainerRef::Mail`].
    ///
    /// [`ContainerRef::Mail`]: crate::ContainerRef
    pub(crate) fn mail_attachments(&self, id: i32) -> Option<&[ItemData]> {
        self.mail
            .messages
            .get(&id)
            .or_else(|| self.mail.drafts.get(&id))
            .map(|m| m.attachments.as_slice())
    }

    pub(crate) fn mail_attachments_mut(&mut self, id: i32) -> Option<&mut Vec<ItemData>> {
        if self.mail.messages.contains_key(&id) {
            return self.mail.messages.get_mut(&id).map(|m| &mut m.attachments);
        }
        self.mail.drafts.get_mut(&id).map(|m| &mut m.attachments)
    }

    /// The message ids in the order a `ConcurrentHashMap` iterates them.
    fn mail_ids_in_map_order(&self) -> Vec<i32> {
        java_concurrent_map_order(&self.mail.order)
    }

    /// Java `MailManager.hasUnreadPost(player)`.
    // NOTE(java-quirk): a mail the receiver deleted without reading it still counts.
    pub(crate) fn mail_has_unread_post(&self, receiver: ObjectId) -> bool {
        self.mail
            .messages
            .values()
            .any(|m| m.receiver_id == receiver.0 && m.unread)
    }

    /// Java `MailManager.getInboxSize(objectId)`.
    pub(crate) fn mail_inbox_size(&self, object_id: i32) -> usize {
        self.mail
            .messages
            .values()
            .filter(|m| m.receiver_id == object_id && !m.deleted_by_receiver)
            .count()
    }

    /// Java `MailManager.getOutboxSize(objectId)`.
    pub(crate) fn mail_outbox_size(&self, object_id: i32) -> usize {
        self.mail
            .messages
            .values()
            .filter(|m| m.sender_id == object_id && !m.deleted_by_sender)
            .count()
    }

    /// Java `MailManager.getInbox(objectId)`.
    pub(crate) fn mail_inbox(&self, object_id: i32) -> Vec<&MailMessage> {
        self.mail_ids_in_map_order()
            .into_iter()
            .filter_map(|id| self.mail.messages.get(&id))
            .filter(|m| m.receiver_id == object_id && !m.deleted_by_receiver)
            .collect()
    }

    /// Java `MailManager.getOutbox(objectId)`.
    pub(crate) fn mail_outbox(&self, object_id: i32) -> Vec<&MailMessage> {
        self.mail_ids_in_map_order()
            .into_iter()
            .filter_map(|id| self.mail.messages.get(&id))
            .filter(|m| m.sender_id == object_id && !m.deleted_by_sender)
            .collect()
    }

    pub(crate) fn mail_persist(&mut self, write: MailWrite) {
        self.outbox.persist(PersistCommand::Mail(write));
    }

    /// Java `MailManager.sendMessage(msg)`: the mail is stored, the receiver (when online) is told
    /// and the deletion task learns its expiration.
    pub(crate) fn mail_send_message(&mut self, msg: MailMessage) {
        let (id, receiver) = (msg.id, msg.receiver_id);
        self.mail_persist(MailWrite::Insert(Box::new(msg.clone())));
        if self.mail.messages.insert(id, msg).is_none() {
            self.mail.order.push(id);
        }
        if self.players.contains_key(&ObjectId(receiver)) {
            self.send(
                ObjectId(receiver),
                &ExNoticePostArrived {
                    show_animation: true,
                },
            );
        }
        self.mail_start_deletion_task();
    }

    /// Java `MailMessage.markAsRead()`.
    pub(crate) fn mail_mark_as_read(&mut self, id: i32) {
        let Some(msg) = self.mail.messages.get_mut(&id) else {
            return;
        };
        if !msg.unread {
            return;
        }
        msg.unread = false;
        self.mail_persist(MailWrite::MarkRead(id));
    }

    /// Java `MailMessage.setDeletedBySender()`.
    pub(crate) fn mail_set_deleted_by_sender(&mut self, id: i32) {
        let Some(msg) = self.mail.messages.get_mut(&id) else {
            return;
        };
        if msg.deleted_by_sender {
            return;
        }
        msg.deleted_by_sender = true;
        if msg.deleted_by_receiver {
            self.mail_delete_message(id);
        } else {
            self.mail_persist(MailWrite::MarkDeletedBySender(id));
        }
    }

    /// Java `MailMessage.setDeletedByReceiver()`.
    pub(crate) fn mail_set_deleted_by_receiver(&mut self, id: i32) {
        let Some(msg) = self.mail.messages.get_mut(&id) else {
            return;
        };
        if msg.deleted_by_receiver {
            return;
        }
        msg.deleted_by_receiver = true;
        if msg.deleted_by_sender {
            self.mail_delete_message(id);
        } else {
            self.mail_persist(MailWrite::MarkDeletedByReceiver(id));
        }
    }

    /// Java `MailMessage.removeAttachments()`: the container is forgotten (its items went
    /// somewhere else, or are gone) and the flag is cleared in the table.
    pub(crate) fn mail_remove_attachments(&mut self, id: i32) {
        let Some(msg) = self.mail.messages.get_mut(&id) else {
            return;
        };
        // `_attachments == null`: nothing was loaded, nothing to remove.
        if !msg.has_attachments {
            return;
        }
        msg.attachments.clear();
        msg.has_attachments = false;
        self.mail_persist(MailWrite::RemoveAttachments(id));
    }

    /// Java `MailManager.deleteMessageInDb(msgId)`: the row goes, the mail leaves the map and its
    /// id is given back.
    pub(crate) fn mail_delete_message(&mut self, id: i32) {
        self.mail_persist(MailWrite::Delete(id));
        if self.mail.messages.remove(&id).is_some() {
            self.mail.order.retain(|&m| m != id);
        }
        self.release_item_id(ObjectId(id));
    }
}
