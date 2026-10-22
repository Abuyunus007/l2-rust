//! The expiry of mails: every 10 seconds the mails whose time ran out are deleted and their
//! attachments go back to the sender.
// Java: taskmanagers/MessageDeletionTaskManager.java, entity/itemcontainer/Mail.java (returnToWh,
// Java: deleteMe)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::MailTimer;
use crate::outbox::ItemRowOp;
use crate::timer::{TimerKind, TimerWheel};
use crate::warehouse::{ContainerRef, Place};
use crate::world::{World, unix_ms_now};

/// `scheduleAtFixedRate(this, 10000, 10000)`.
const DELETION_INTERVAL_MS: u64 = 10_000;

/// `ItemLocation.WAREHOUSE`.
const LOC_WAREHOUSE: &str = "WAREHOUSE";

impl World {
    /// Starts the deletion task if it is not running (Java creates the task manager with the
    /// first mail it is told about).
    pub(crate) fn mail_start_deletion_task(&mut self) {
        if self.mail.deletion_timer.is_some() {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(DELETION_INTERVAL_MS);
        let timer = self.timers.schedule_at(due, TimerKind::Mail(MailTimer::Deletion));
        self.mail.deletion_timer = Some(timer);
    }

    /// The timers of `TimerKind::Mail`.
    pub(crate) fn on_mail_timer(&mut self, timer: MailTimer) {
        match timer {
            MailTimer::Deletion => {
                self.mail.deletion_timer = None;
                self.mail_run_deletion();
                self.mail_start_deletion_task();
            }
        }
    }

    /// Java `MessageDeletionTaskManager.run`.
    fn mail_run_deletion(&mut self) {
        let now = unix_ms_now();
        // `PENDING_MESSAGES` holds every mail with its expiration, so the mails in the map whose
        // time passed are the same ones.
        let expired: Vec<i32> = self
            .mail
            .order
            .iter()
            .copied()
            .filter(|id| self.mail.messages.get(id).is_some_and(|m| now > m.expiration))
            .collect();
        for id in expired {
            self.mail_expire(id);
        }
    }

    // D-49: without `MailExpiryDatabaseWrites` the memory is expired like Java does it, but no row is
    // written: the Java GS, which loaded the same mails, deletes the rows and moves the attachments
    // back to the sender on its own schedule, and doing it twice would hand the items out twice.
    fn mail_expire(&mut self, id: i32) {
        let write_rows = self.mail.settings.expiry_database_writes;
        let Some(message) = self.mail.messages.get(&id) else {
            return;
        };
        if message.has_attachments {
            let (sender_id, receiver_id) = (message.sender_id, message.receiver_id);
            let sender = ObjectId(sender_id);
            let receiver = ObjectId(receiver_id);
            let sender_online = self.players.contains_key(&sender);
            let receiver_online = self.players.contains_key(&receiver);
            if sender_online {
                self.mail_return_to_warehouse(id, sender_id, Some(sender), write_rows);
                self.send(
                    sender,
                    &SystemMessage::new(SystemMessageId::TheMailWasReturnedDueToTheExceededWaitingTime),
                );
            } else if sender_id == -1 {
                // Action House / Custom Mail.
                if receiver_online {
                    self.mail_return_to_warehouse(id, sender_id, Some(receiver), write_rows);
                }
            } else {
                self.mail_return_to_warehouse(id, sender_id, None, write_rows);
            }

            // `getAttachments().deleteMe()`: what is left is written once more and forgotten.
            let leftovers: Vec<ItemRowOp> = self
                .mail_attachments(id)
                .unwrap_or_default()
                .iter()
                .map(|item| ItemRowOp::Update {
                    owner_id: sender_id,
                    item: item.clone(),
                })
                .collect();
            if write_rows {
                self.flush_item_rows(sender, leftovers);
            }
            self.mail_remove_attachments_rows(id, write_rows);

            if receiver_online {
                self.send(
                    receiver,
                    &SystemMessage::new(SystemMessageId::TheMailWasReturnedDueToTheExceededWaitingTime),
                );
            }
        }
        self.mail_delete_message_rows(id, write_rows);
    }

    /// Java `Mail.returnToWh(wh)`: every attachment moves to the private warehouse of `owner`, or
    /// (no `owner`: `wh == null`) goes to the `WAREHOUSE` location of the sender's own rows.
    fn mail_return_to_warehouse(
        &mut self,
        id: i32,
        sender_id: i32,
        owner: Option<ObjectId>,
        write_rows: bool,
    ) {
        let container = ContainerRef::Mail {
            message: id,
            sender: sender_id,
        };
        let object_ids: Vec<ObjectId> = self
            .mail_attachments(id)
            .unwrap_or_default()
            .iter()
            .map(|i| i.object_id)
            .collect();
        let mut ops: Vec<ItemRowOp> = Vec::new();
        for object_id in object_ids {
            match owner {
                Some(owner) => {
                    let count = self
                        .mail_attachments(id)
                        .and_then(|items| items.iter().find(|i| i.object_id == object_id))
                        .map_or(0, |i| i.count);
                    self.move_item(
                        Place::Container(container),
                        object_id,
                        count,
                        Place::Container(ContainerRef::Private(owner)),
                        owner,
                        &mut ops,
                    );
                }
                None => {
                    // `item.setItemLocation(ItemLocation.WAREHOUSE)`; the row is written by
                    // `deleteMe` right after.
                    if let Some(item) = self
                        .mail_attachments_mut(id)
                        .and_then(|items| items.iter_mut().find(|i| i.object_id == object_id))
                    {
                        item.loc = LOC_WAREHOUSE.to_string();
                        item.loc_data = 0;
                    }
                }
            }
        }
        if let Some(owner) = owner
            && write_rows
        {
            self.flush_item_rows(owner, ops);
        }
    }
}
