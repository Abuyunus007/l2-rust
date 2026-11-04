//! `config/Custom/CustomMailManager.ini`: every `DatabaseQueryDelay` seconds the rows of
//! `custom_mail` whose receiver is online become a mail with the listed items.

use tracing::{info, warn};

use super::misc::CustomMiscTimer;
use crate::mail::{MailMessage, SENDER_TYPE_PLAYER};
use crate::outbox::{ItemRowOp, PersistCommand};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};
use l2_util::ObjectId;

/// The mail container of an item.
const LOC_MAIL: &str = "MAIL";
/// `owner_id` of an item in the mail of the system: the sender id is -1.
const SYSTEM_OWNER: i32 = -1;

/// One row of `custom_mail`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMailEntry {
    /// `date`, as the database prints it: with `receiver`, the key of the delete.
    pub date: String,
    pub receiver: i32,
    pub subject: String,
    pub message: String,
    /// `itemId1 count1 enchant1;itemId2 count2;itemId3 ...`.
    pub items: String,
}

/// Java `ItemEnchantHolder`: `(itemId, count, enchantLevel)`.
type ItemHolder = (i32, i64, i32);

/// `StringUtil.isNumeric`: not empty, only digits.
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

/// Java `String.split(";")`: no separator gives the text, trailing empty pieces are dropped.
fn java_split(text: &str, separator: char) -> Vec<&str> {
    if !text.contains(separator) {
        return vec![text];
    }
    let mut pieces: Vec<&str> = text.split(separator).collect();
    while pieces.last().is_some_and(|p| p.is_empty()) {
        pieces.pop();
    }
    pieces
}

/// The loop over `items.split(";")` of `CustomMailManager`. `None` where Java throws a
/// `NumberFormatException` (a count or enchant level that does not fit its type).
pub(crate) fn parse_items(items: &str) -> Option<Vec<ItemHolder>> {
    let mut holders = Vec::new();
    for piece in java_split(items, ';') {
        if piece.contains(' ') {
            let split = java_split(piece, ' ');
            let item_id = split.first().copied().unwrap_or("");
            let item_count = split.get(1).copied().unwrap_or("");
            let enchant = split.get(2).copied().unwrap_or("0");
            if is_numeric(item_id) && is_numeric(item_count) {
                holders.push((
                    item_id.parse::<i32>().ok()?,
                    item_count.parse::<i64>().ok()?,
                    enchant.parse::<i32>().ok()?,
                ));
            }
        } else if is_numeric(piece) {
            holders.push((piece.parse::<i32>().ok()?, 1, 0));
        }
    }
    Some(holders)
}

impl World {
    /// The start of `CustomMailManager`'s fixed-rate task (initial delay = period).
    pub(crate) fn start_custom_mail(&mut self) {
        if !self.settings.custom_misc.custom_mail_enabled {
            return;
        }
        self.schedule_custom_mail();
        info!("CustomMailManager: Enabled.");
    }

    fn schedule_custom_mail(&mut self) {
        let delay = u64::try_from(self.settings.custom_misc.custom_mail_delay_ms).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        self.timers
            .schedule_at(due, TimerKind::CustomMisc(CustomMiscTimer::CustomMail));
    }

    /// One run of the task: asks for the rows.
    pub(crate) fn on_custom_mail_timer(&mut self) {
        if !self.settings.custom_misc.custom_mail_enabled {
            return;
        }
        self.schedule_custom_mail();
        // A slow read is not asked twice.
        if !self.custom_misc.custom_mail_pending {
            self.custom_misc.custom_mail_pending = true;
            self.outbox.persist(PersistCommand::CustomMailRequest);
        }
    }

    /// The answer of the read (`WorldCommand::CustomMailLoaded`).
    pub(crate) fn on_custom_mail_loaded(&mut self, result: Result<Vec<CustomMailEntry>, ()>) {
        self.custom_misc.custom_mail_pending = false;
        match result {
            Ok(rows) => {
                for row in rows {
                    self.send_custom_mail(row);
                }
            }
            Err(()) => warn!("CustomMailManager: Error reading from database."),
        }
    }

    /// The loop body: a row of an online player becomes a mail.
    fn send_custom_mail(&mut self, row: CustomMailEntry) {
        let player = ObjectId(row.receiver);
        if !self.players.contains_key(&player) {
            return;
        }
        let Some(holders) = parse_items(&row.items) else {
            warn!(
                "CustomMailManager: the items of the mail for {} are not valid: {}",
                row.receiver, row.items
            );
            return;
        };
        let Some(message_id) = self.allocate_item_id() else {
            return;
        };
        let mut msg = MailMessage::new_system(
            message_id.0,
            row.receiver,
            &row.subject,
            &row.message,
            SENDER_TYPE_PLAYER,
            unix_ms_now(),
        );

        if !holders.is_empty() {
            // `msg.createAttachments()`
            msg.has_attachments = true;
            for (item_id, count, enchant) in holders {
                self.custom_mail_add_item(&mut msg, item_id, count, enchant);
            }
        }
        // The rows of the attachments, then the delete of the entry and the message itself.
        let ops: Vec<ItemRowOp> = msg
            .attachments
            .iter()
            .cloned()
            .map(|item| ItemRowOp::Insert {
                owner_id: SYSTEM_OWNER,
                item,
            })
            .collect();
        if !ops.is_empty() {
            self.outbox
                .persist(PersistCommand::ItemRows { tracked: None, ops });
        }
        self.outbox.persist(PersistCommand::CustomMailDelete {
            date: row.date,
            receiver: row.receiver,
        });
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        self.mail_send_message(msg);
        info!("CustomMailManager: Message sent to {name}.");
    }

    /// `attachments.addItem(TRANSFER, itemId, count, null, null)` and `setEnchantLevel`: a
    /// stackable item joins the one of the container, else `count` items are made.
    fn custom_mail_add_item(&mut self, msg: &mut MailMessage, item_id: i32, count: i64, enchant: i32) {
        if count <= 0 {
            return;
        }
        let Some(template) = self.item_templates().and_then(|t| t.get(item_id).cloned()) else {
            warn!("Item doesn't exist so cannot be added. Item ID: {item_id}");
            return;
        };
        let mut last: Option<usize> = None;
        if template.stackable {
            if let Some(index) = msg.attachments.iter().position(|i| i.item_id == item_id) {
                msg.attachments[index].count = msg.attachments[index].count.saturating_add(count);
                last = Some(index);
            }
        }
        if last.is_none() {
            let copies = if template.stackable { 1 } else { count };
            for _ in 0..copies {
                let Some(id) = self.allocate_item_id() else {
                    return;
                };
                let mut item = self.new_item_data(&template, id, if template.stackable { count } else { 1 });
                item.loc = LOC_MAIL.to_owned();
                item.loc_data = msg.id;
                msg.attachments.push(item);
                last = Some(msg.attachments.len() - 1);
            }
        }
        if enchant > 0
            && let Some(index) = last
        {
            msg.attachments[index].enchant_level = enchant;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_item_list_of_a_row() {
        assert_eq!(
            parse_items("57 1000 3;4037"),
            Some(vec![(57, 1000, 3), (4037, 1, 0)])
        );
        // The second piece has a count only: enchant 0.
        assert_eq!(parse_items("57 5"), Some(vec![(57, 5, 0)]));
        // Not numeric pieces are skipped; a trailing `;` is dropped.
        assert_eq!(parse_items("abc;57 x;99 2;"), Some(vec![(99, 2, 0)]));
        assert_eq!(parse_items(""), Some(vec![]));
        // `Integer.parseInt` fails for a number that does not fit.
        assert_eq!(parse_items("99999999999"), None);
    }
}
