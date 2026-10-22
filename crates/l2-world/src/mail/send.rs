//! `RequestSendPost`: the checks, the fee, and moving the attachments into the mail.
// Java: network/clientpackets/RequestSendPost.java

use l2_data::item::ADENA_ID;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::{PostAttachment, RequestSendPost};
use l2_protocol_game::server::SystemMessageParam;
use l2_protocol_game::server::{ExNoticePostSent, InventoryUpdate, ItemUpdate, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::MailMessage;
use crate::inventory::LOC_PAPERDOLL;
use crate::outbox::ItemRowOp;
use crate::player::ItemData;
use crate::warehouse::{ContainerRef, Place};
use crate::world::{World, unix_ms_now};
use crate::zones::ZoneId;

/// `RequestSendPost.MAX_RECV_LENGTH`.
const MAX_RECV_LENGTH: usize = 16;
/// `RequestSendPost.MAX_SUBJ_LENGTH`.
const MAX_SUBJ_LENGTH: usize = 128;
/// `RequestSendPost.MAX_TEXT_LENGTH`.
const MAX_TEXT_LENGTH: usize = 512;
/// `RequestSendPost.MAX_ATTACHMENTS`.
const MAX_ATTACHMENTS: usize = 8;
/// `RequestSendPost.INBOX_SIZE`.
const INBOX_SIZE: usize = 240;
/// `RequestSendPost.OUTBOX_SIZE`.
const OUTBOX_SIZE: usize = 240;
/// `RequestSendPost.MESSAGE_FEE`.
const MESSAGE_FEE: i64 = 100;
/// `RequestSendPost.MESSAGE_FEE_PER_SLOT`: 100 adena message fee + 1000 per each item slot.
const MESSAGE_FEE_PER_SLOT: i64 = 1000;

/// Java `String.length()`: UTF-16 code units.
fn java_length(text: &str) -> usize {
    text.encode_utf16().count()
}

impl World {
    /// `AccessLevel.allowTransaction()` of the player.
    pub(super) fn mail_allows_transaction(&self, player: ObjectId) -> bool {
        self.players
            .get(&player)
            .is_some_and(|p| self.data.access_level_allows_transaction(p.data.access_level))
    }

    /// Java `getClient().getFloodProtectors().canSendMail()`: a GM may always, anybody else once
    /// per interval; every allowed call starts the next interval.
    fn mail_can_send(&mut self, player: ObjectId) -> bool {
        if self.is_gm(player) {
            return true;
        }
        let tick = self.game_ticks();
        if self
            .mail
            .next_send_tick
            .get(&player)
            .is_some_and(|&next| tick < next)
        {
            return false;
        }
        let interval = self.mail.settings.send_interval_ticks;
        // Players that left long ago do not pile up.
        self.mail.next_send_tick.retain(|_, &mut next| next > tick);
        self.mail
            .next_send_tick
            .insert(player, tick.saturating_add(interval));
        true
    }

    /// `player.checkItemManipulation(objectId, count, "attach")` and `item.isTradeable() &&
    /// !item.isEquipped()`: the item when the player may attach this much of it.
    fn mail_attachable_item(&self, player: ObjectId, entry: &PostAttachment) -> Option<ItemData> {
        let item = self.check_item_manipulation(player, ObjectId(entry.object_id), entry.count)?;
        let templates = self.item_templates()?;
        let template = templates.get(item.item_id)?;
        (self.item_is_tradeable(&item, template) && item.loc != LOC_PAPERDOLL).then_some(item)
    }

    fn mail_tell(&self, player: ObjectId, message: SystemMessageId) {
        self.send(player, &SystemMessage::new(message));
    }

    /// Java `RequestSendPost.runImpl`.
    pub(crate) fn on_request_send_post(&mut self, id: ObjectId, request: RequestSendPost) {
        let RequestSendPost {
            receiver,
            mut is_cod,
            subject,
            text,
            mut items,
            mut req_adena,
        } = request;
        if !self.mail.settings.allow_mail || !self.players.contains_key(&id) {
            return;
        }
        // `readImpl` stops at an item count above `MaxItemInPacket`, before it reads the adena.
        if items
            .as_ref()
            .is_some_and(|v| v.len() > self.max_item_in_packet())
        {
            items = None;
            req_adena = 0;
        }

        if !self.mail.settings.allow_attachments {
            items = None;
            is_cod = false;
            req_adena = 0;
        }

        if !self.mail_allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your Access Level.");
            return;
        }

        if !self.is_inside_zone(id, ZoneId::Peace) && items.is_some() {
            self.mail_tell(id, SystemMessageId::YouCannotForwardInANonPeaceZoneLocation);
            return;
        }

        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) {
            self.mail_tell(id, SystemMessageId::YouCannotForwardDuringAnExchange);
            return;
        }

        if self.is_inventory_disabled(id) {
            self.mail_tell(id, SystemMessageId::YouCannotForwardDuringAnExchange);
            return;
        }

        if self.is_enchanting(id) {
            self.mail_tell(
                id,
                SystemMessageId::YouCannotForwardDuringAnItemEnhancementOrAttributeEnhancement,
            );
            return;
        }

        if self.is_in_store_mode(id) {
            self.mail_tell(
                id,
                SystemMessageId::YouCannotForwardBecauseThePrivateShopOrWorkshopIsInProgress,
            );
            return;
        }

        if java_length(&receiver) > MAX_RECV_LENGTH {
            self.mail_tell(id, SystemMessageId::TheAllowedLengthForRecipientExceeded);
            return;
        }

        if java_length(&subject) > MAX_SUBJ_LENGTH {
            self.mail_tell(id, SystemMessageId::TheAllowedLengthForATitleExceeded);
            return;
        }

        if java_length(&text) > MAX_TEXT_LENGTH {
            // Not found message for this.
            self.mail_tell(id, SystemMessageId::TheAllowedLengthForATitleExceeded);
            return;
        }

        if items.as_ref().is_some_and(|v| v.len() > MAX_ATTACHMENTS) {
            self.mail_tell(id, SystemMessageId::ItemSelectionIsPossibleUpTo8);
            return;
        }

        if req_adena < 0 || req_adena > self.settings.ground.max_adena {
            return;
        }

        if is_cod {
            if req_adena == 0 {
                self.mail_tell(
                    id,
                    SystemMessageId::WhenNotEnteringTheAmountForThePaymentRequestYouCannotSendAnyMail,
                );
                return;
            }
            if items.as_ref().is_none_or(Vec::is_empty) {
                self.mail_tell(
                    id,
                    SystemMessageId::ItSAPaymentRequestTransactionPleaseAttachTheItem,
                );
                return;
            }
        }

        let receiver_id = self.mail_char_id_by_name(&receiver);
        if receiver_id <= 0 {
            self.mail_tell(
                id,
                SystemMessageId::WhenTheRecipientDoesnTExistOrTheCharacterHasBeenDeletedSendingMailIsNotPossible,
            );
            return;
        }

        if receiver_id == id.0 {
            self.mail_tell(id, SystemMessageId::YouCannotSendAMailToYourself);
            return;
        }

        let level = self.mail_char_access_level(receiver_id);
        if self.data.access_level(level).is_gm && !self.is_gm(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YourMessageToC1DidNotReachItsRecipientYouCannotSendMailToTheGmStaff,
                )
                .with(SystemMessageParam::Text(receiver)),
            );
            return;
        }

        if self.is_jailed(id)
            && ((self.settings.zones.jail_disable_transaction && items.is_some())
                || self.punishment_settings().jail_disable_chat)
        {
            self.mail_tell(id, SystemMessageId::YouCannotForwardInANonPeaceZoneLocation);
            return;
        }

        // M6-FRIEND: `BlockList.isInBlockList(receiverId, player.getObjectId())` ->
        // C1_HAS_BLOCKED_YOU_YOU_CANNOT_SEND_MAIL_TO_C1 (addString(receiver)).

        if self.mail_outbox_size(id.0) >= OUTBOX_SIZE {
            self.mail_tell(
                id,
                SystemMessageId::TheMailLimit240HasBeenExceededAndThisCannotBeForwarded,
            );
            return;
        }

        if self.mail_inbox_size(receiver_id) >= INBOX_SIZE {
            self.mail_tell(
                id,
                SystemMessageId::TheMailLimit240HasBeenExceededAndThisCannotBeForwarded,
            );
            return;
        }

        if !self.mail_can_send(id) {
            self.mail_tell(
                id,
                SystemMessageId::ThePreviousMailWasForwardedLessThan1MinuteAgoAndThisCannotBeForwarded,
            );
            return;
        }

        // NOTE(java-quirk): the id is taken here, before `removeItems`, and a mail that is not
        // sent keeps it (Java never releases it either).
        let Some(message_id) = self.allocate_item_id() else {
            warn!("No object id for a new mail of {id}.");
            return;
        };
        let msg = MailMessage::new_player_mail(
            message_id.0,
            id.0,
            receiver_id,
            is_cod,
            &subject,
            &text,
            req_adena,
            unix_ms_now(),
        );
        if let Some(msg) = self.mail_remove_items(id, msg, items.as_deref()) {
            // "Should not trade during mail."
            self.clear_multisell(id);
            self.mail_send_message(msg);
            self.send(id, &ExNoticePostSent { show_animation: true });
            self.mail_tell(id, SystemMessageId::MailSuccessfullySent);
        }
    }

    /// Java `RequestSendPost.removeItems(player, msg)`: the checks of the attachments, the fee,
    /// and the move into the mail. The mail when everything worked, `None` after a message to the
    /// player.
    fn mail_remove_items(
        &mut self,
        id: ObjectId,
        mut msg: MailMessage,
        items: Option<&[PostAttachment]>,
    ) -> Option<MailMessage> {
        let mut current_adena = self.adena_of(id);
        let mut fee = MESSAGE_FEE;
        for entry in items.unwrap_or_default() {
            // Check validity of requested item.
            let Some(item) = self.mail_attachable_item(id, entry) else {
                self.mail_tell(
                    id,
                    SystemMessageId::TheItemThatYouReTryingToSendCannotBeForwardedBecauseItIsnTProper,
                );
                return None;
            };
            fee += MESSAGE_FEE_PER_SLOT;
            if item.item_id == ADENA_ID {
                current_adena -= entry.count;
            }
        }

        // Check if enough adena and charge the fee.
        if current_adena < fee || !self.reduce_adena(id, fee, false) {
            self.mail_tell(id, SystemMessageId::YouCannotForwardBecauseYouDonTHaveEnoughAdena);
            return None;
        }

        let Some(items) = items else {
            return Some(msg);
        };
        // `msg.createAttachments()`: the mail gets its container.
        msg.has_attachments = true;
        let message_id = msg.id;
        self.mail.drafts.insert(message_id, msg);
        let container = ContainerRef::Mail {
            message: message_id,
            sender: id.0,
        };

        // Proceed to the transfer.
        let mut ops: Vec<ItemRowOp> = Vec::new();
        let mut entries: Vec<ItemUpdate> = Vec::new();
        let mut done = true;
        for entry in items {
            // Check validity of requested item.
            let Some(old_item) = self.mail_attachable_item(id, entry) else {
                warn!("Error adding attachment for char {id} (olditem == null)");
                done = false;
                break;
            };
            let Some(moved) = self.move_item(
                Place::Inventory(id),
                old_item.object_id,
                entry.count,
                Place::Container(container),
                id,
                &mut ops,
            ) else {
                warn!("Error adding attachment for char {id} (newitem == null)");
                continue;
            };
            let change = if moved.old.count > 0 && !moved.moved_whole {
                InventoryUpdate::MODIFIED
            } else {
                InventoryUpdate::REMOVED
            };
            entries.push(self.update_entry(&moved.old, change));
        }
        self.flush_item_rows(id, ops);
        let msg = self.mail.drafts.remove(&message_id);
        if !done {
            // NOTE(java-quirk): the mail is dropped with the items already moved (their rows say
            // `MAIL` with a message that does not exist) and the fee is gone.
            return None;
        }

        // Send updated item list to the player.
        for entry in entries {
            self.send_inventory_update(id, entry);
        }
        // Update current load status on player.
        self.send_cur_load(id);
        msg
    }
}
