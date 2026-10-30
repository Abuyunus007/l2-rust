//! The database parts of the event scripts: the birthday gifts of `CharacterBirthday` and the
//! event items `LongTimeEvent` removes when an event ends (S-LEFT-01).
// Java: dist/game/data/scripts/events/CharacterBirthday/CharacterBirthday.java (onDailyReset),
//       gameserver/mechanics/script/LongTimeEvent.java (destroyItemsOnEnd)
//
// Both are global writes of the Java GS too (D-39), so they follow the side-by-side rules:
// - the birthday query and the mails are a job of the daily reset: only with
//   `DailyResetDatabaseWrites = True` (D-49), else nothing is read and nothing is sent;
// - the `DELETE FROM items WHERE item_id=?` only with `CycleDatabaseWrites = True` (D-53).
// Neither awaits in the tick: the query goes out through the outbox and the answer comes back as
// `WorldCommand::BirthdayGiftsLoaded` (D-15).

use tracing::{info, warn};

use crate::mail::{MailMessage, SENDER_TYPE_ALEGRIA};
use crate::outbox::{ItemRowOp, PersistCommand};
use crate::world::{World, unix_ms_now};

/// The mail container of an item.
const LOC_MAIL: &str = "MAIL";

/// `owner_id` of an item in the mail of the system: the sender id is -1.
const SYSTEM_OWNER: i32 = -1;

/// One character the birthday query found: `(charId, char_name, age)`.
pub type BirthdayRow = (i32, String, i32);

impl World {
    /// The query half of `CharacterBirthday.onDailyReset`: asks the persistence task for the
    /// characters whose birthday came in the last 24 hours. Nothing happens without
    /// `DailyResetDatabaseWrites` (D-49): the Java GS sends the same mails.
    pub fn request_birthday_gifts(&mut self) {
        if !self.settings.daily_reset_database_writes {
            info!("CharacterBirthday: DailyResetDatabaseWrites is off, no birthday gifts are sent (D-49).");
            return;
        }
        // Last 24 hours.
        let since_ms = unix_ms_now() - 24 * 60 * 60 * 1000;
        self.outbox.persist(PersistCommand::BirthdayGiftsRequest { since_ms });
    }

    /// The answer of the birthday query (`WorldCommand::BirthdayGiftsLoaded`): one mail with the
    /// birthday gift for each character found, then Java's "N gifts sent." line.
    pub(crate) fn on_birthday_gifts_loaded(&mut self, result: Result<Vec<BirthdayRow>, ()>) {
        let mut gifts = 0;
        match result {
            Ok(rows) => {
                for (char_id, _name, _age) in rows {
                    if self.send_birthday_gift(char_id) {
                        gifts += 1;
                    }
                }
            }
            // Java: "Error checking birthdays." and the count so far (0).
            Err(()) => warn!("CharacterBirthday: Error checking birthdays."),
        }
        info!("CharacterBirthday {gifts} gifts sent.");
    }

    /// The loop body of `onDailyReset`: the mail from Alegria with `AltBirthdayGift` attached.
    fn send_birthday_gift(&mut self, char_id: i32) -> bool {
        // NOTE(java-quirk): Java replaces "$c1" and "$s1" with `replaceAll`, which takes a regular
        // expression: `$` is the end of the input there, so neither pattern matches anything and
        // the text goes out as the setting has it (with `$c1`/`$s1` in it, if it has them).
        let text = self.scripts.settings.alt_birthday_mail_text.clone();
        let subject = self.scripts.settings.alt_birthday_mail_subject.clone();
        let gift = self.scripts.settings.alt_birthday_gift;

        let Some(template) = self.item_templates().and_then(|t| t.get(gift).cloned()) else {
            warn!("CharacterBirthday: the birthday gift {gift} does not exist.");
            return false;
        };
        let (Some(message_id), Some(item_id)) = (self.allocate_item_id(), self.allocate_item_id()) else {
            return false;
        };
        let now_ms = unix_ms_now();
        let mut message = MailMessage::new_system(
            message_id.0,
            char_id,
            &subject,
            &text,
            SENDER_TYPE_ALEGRIA,
            now_ms,
        );
        // `createAttachments().addItem(ItemProcessType.REWARD, ALT_BIRTHDAY_GIFT, 1, null, null)`.
        let mut item = self.new_item_data(&template, item_id, 1);
        item.loc = LOC_MAIL.to_owned();
        item.loc_data = message.id;
        message.has_attachments = true;
        message.attachments = vec![item.clone()];
        self.outbox.persist(PersistCommand::ItemRows {
            tracked: None,
            ops: vec![ItemRowOp::Insert {
                owner_id: SYSTEM_OWNER,
                item,
            }],
        });
        self.mail_send_message(message);
        true
    }

    /// The database half of `LongTimeEvent.destroyItemsOnEnd` for one item id: `DELETE FROM items
    /// WHERE item_id=?`, the rows of the characters who are not online. Only with
    /// `CycleDatabaseWrites` (D-53): the Java GS deletes the same rows when its event ends.
    pub fn delete_event_item_rows(&mut self, item_id: i32) {
        if !self.scripts.settings.cycle_database_writes {
            return;
        }
        self.outbox.persist(PersistCommand::DeleteItemsOfItemId { item_id });
    }
}
