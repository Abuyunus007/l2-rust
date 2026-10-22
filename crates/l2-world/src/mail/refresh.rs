//! Fresh mail rows for a database two servers share (M6-SYNC-01, D-39).
// Java: managers/MailManager.java (the map the mails live in), entity/itemcontainer/Mail.java
// Java: (restore: `WHERE owner_id=? AND loc='MAIL' AND loc_data=?`)
//
// The Java GS and this server each load `messages` and the MAIL rows of `items` once at their
// start. A mail the Java GS received after that, or an attachment one of its players took, is
// unknown here, and handing the same attachment out again would duplicate the items. Two reads
// close the gap without a database wait inside a tick (D-15):
// - the session reads the character's mails at `CharacterSelect` ([`MailRefresh`]) and
//   [`World::mail_refresh_player`] replaces this server's copies of exactly those messages;
// - `RequestPostAttachment` asks for the attachment rows of the message
//   ([`PersistCommand::MailAttachmentsRequest`], answered by `WorldCommand::MailAttachmentsLoaded`)
//   and goes on with them.
// The persistence task reads in the order it writes, so an answer includes every write this server
// queued before the request.

use std::collections::BTreeSet;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::MailMessage;
use crate::outbox::PersistCommand;
use crate::player::ItemData;
use crate::world::World;

/// `ItemLocation.MAIL` as the `items.loc` column spells it.
const LOC_MAIL: &str = "MAIL";

/// Which attachment packet waits for the rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttachmentAction {
    /// `RequestPostAttachment`.
    Take,
    /// `RequestRejectPostAttachment`.
    Reject,
    /// `RequestCancelPostAttachment`.
    Cancel,
}

/// The mails of one character as the database has them (sender or receiver), with their
/// attachments (`PlayerProfile::mail_refresh`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailRefresh {
    pub messages: Vec<MailMessage>,
}

impl World {
    /// The mail part of entering the world: the copies of the character's messages are replaced by
    /// what the session read. A message the database no longer has is dropped (the Java GS
    /// deleted or expired it), a new one is added and the attachments of the others are replaced.
    ///
    /// Messages this server made itself since it started are left alone: the Java GS cannot have
    /// changed them, and their rows may not be written yet. The ids of dropped messages are not
    /// given back to the id factory, they were never this server's.
    pub(crate) fn mail_refresh_player(&mut self, id: ObjectId) {
        let Some(refresh) = self
            .players
            .get_mut(&id)
            .and_then(|p| p.data.profile.mail_refresh.take())
        else {
            return;
        };
        let character = id.0;
        let loaded: BTreeSet<i32> = refresh.messages.iter().map(|m| m.id).collect();
        let stale: Vec<i32> = self
            .mail
            .order
            .iter()
            .copied()
            .filter(|mail_id| {
                !loaded.contains(mail_id)
                    && !self.mail.local_ids.contains(mail_id)
                    && self
                        .mail
                        .messages
                        .get(mail_id)
                        .is_some_and(|m| m.sender_id == character || m.receiver_id == character)
            })
            .collect();
        for mail_id in &stale {
            self.mail.messages.remove(mail_id);
        }
        self.mail.order.retain(|mail_id| !stale.contains(mail_id));

        let (mut added, mut replaced) = (0usize, 0usize);
        for mut message in refresh.messages {
            if self.mail.local_ids.contains(&message.id) {
                continue;
            }
            // `Item.restoreFromDb` returns null for a template the datapack does not know.
            self.drop_unknown_items(&mut message.attachments);
            let mail_id = message.id;
            if self.mail.messages.insert(mail_id, message).is_none() {
                self.mail.order.push(mail_id);
                added += 1;
            } else {
                replaced += 1;
            }
        }
        if !stale.is_empty() || added > 0 {
            info!(
                "Mail: refreshed the post box of character {character}: {added} added, {replaced} replaced, {} dropped.",
                stale.len()
            );
        }
        // The deletion task has to know the mails the Java GS made, too.
        self.mail_start_deletion_task();
    }

    /// Asks for the attachment rows of `message_id` before `player` takes them. A request that is
    /// already out is replaced (the answer of the first one is dropped as stale).
    pub(crate) fn mail_request_attachments(
        &mut self,
        player: ObjectId,
        message_id: i32,
        sender_id: i32,
        action: AttachmentAction,
    ) {
        self.mail.pending_attachments.insert(player, (message_id, action));
        self.outbox.persist(PersistCommand::MailAttachmentsRequest {
            player,
            message_id,
            sender_id,
        });
    }

    /// `WorldCommand::MailAttachmentsLoaded`: the attachments of the message are replaced by the
    /// rows that are still in the mail, and `RequestPostAttachment` goes on. A failed read hands
    /// nothing out and never falls back to the copy in memory.
    pub(crate) fn on_mail_attachments_loaded(
        &mut self,
        player: ObjectId,
        message_id: i32,
        result: Result<Vec<ItemData>, ()>,
    ) {
        let action = match self.mail.pending_attachments.get(&player) {
            Some(&(pending, action)) if pending == message_id => action,
            _ => return,
        };
        self.mail.pending_attachments.remove(&player);
        if !self.players.contains_key(&player) {
            return;
        }
        let Ok(mut rows) = result else {
            warn!(
                "Mail: could not read the attachments of message {message_id} for {player}; nothing is handed out."
            );
            self.send(player, &ActionFailed);
            return;
        };
        // Only the rows the message still owns: `loc = 'MAIL'` and `loc_data` = the message id.
        // Anything else moved away (the Java GS gave it out, or returned it in a new message).
        rows.retain(|i| i.loc == LOC_MAIL && i.loc_data == message_id);
        self.drop_unknown_items(&mut rows);
        let Some(message) = self.mail.messages.get_mut(&message_id) else {
            return;
        };
        message.attachments = rows;
        if message.attachments.is_empty() {
            // Taken elsewhere already: the Java GS wrote the flag, nothing to write here.
            message.has_attachments = false;
            if action == AttachmentAction::Cancel {
                // Java's answer to a sent mail without attachments.
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouCannotCancelSentMailSinceTheRecipientReceivedIt),
                );
            }
            return;
        }
        match action {
            AttachmentAction::Take => self.post_attachment(player, message_id, true),
            AttachmentAction::Reject => self.reject_attachment(player, message_id, true),
            AttachmentAction::Cancel => self.cancel_attachment(player, message_id, true),
        }
    }
}
