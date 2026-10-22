//! Everything after a mail was sent: the lists, opening a mail, deleting, taking the attachments
//! (with the payment of a cash on delivery mail), returning them and cancelling a sent mail.
// Java: network/clientpackets/{RequestReceivedPost,RequestReceivedPostList,RequestSentPost,
// Java: RequestSentPostList,RequestPostAttachment,RequestRejectPostAttachment,
// Java: RequestCancelPostAttachment,RequestDeleteReceivedPost,RequestDeleteSentPost,
// Java: RequestPostItemList}.java

use l2_data::item::ADENA_ID;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessageParam;
use l2_protocol_game::server::mail::{POST_STATE_DELETED, POST_STATE_READED, POST_STATE_REJECTED};
use l2_protocol_game::server::{ExChangePostState, InventoryUpdate, ItemUpdate, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::refresh::AttachmentAction;
use super::{MailInput, MailMessage, MailWrite};
use crate::flood::FloodKind;
use crate::inventory::LOC_INVENTORY;
use crate::outbox::ItemRowOp;
use crate::player::ItemData;
use crate::warehouse::{ContainerRef, Place};
use crate::world::{World, unix_ms_now};
use crate::zones::ZoneId;

impl World {
    /// Dispatches the packets of `ClientInput::Mail`.
    pub(crate) fn on_mail_input(&mut self, id: ObjectId, input: MailInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            MailInput::PostItemList => self.on_request_post_item_list(id),
            MailInput::Send(request) => self.on_request_send_post(id, request),
            MailInput::ReceivedPostList => {
                if self.mail.settings.allow_mail {
                    let list = self.mail_received_list(id.0);
                    self.send(id, &list);
                }
            }
            MailInput::SentPostList => {
                if self.mail.settings.allow_mail {
                    let list = self.mail_sent_list(id.0);
                    self.send(id, &list);
                }
            }
            MailInput::ReceivedPost { msg_id } => self.on_request_received_post(id, msg_id),
            MailInput::SentPost { msg_id } => self.on_request_sent_post(id, msg_id),
            MailInput::PostAttachment { msg_id } => self.on_request_post_attachment(id, msg_id),
            MailInput::RejectPostAttachment { msg_id } => self.on_request_reject_post_attachment(id, msg_id),
            MailInput::CancelPostAttachment { msg_id } => self.on_request_cancel_post_attachment(id, msg_id),
            MailInput::DeleteReceivedPost { msg_ids } => self.on_request_delete_post(id, msg_ids, true),
            MailInput::DeleteSentPost { msg_ids } => self.on_request_delete_post(id, msg_ids, false),
        }
    }

    fn mail_tell_id(&self, player: ObjectId, message: SystemMessageId) {
        self.send(player, &SystemMessage::new(message));
    }

    /// `PunishmentManager.handleIllegalPlayerAction(player, player + " <what>", DEFAULT_PUNISH)`;
    /// `Player.toString()` is `Player:<name>[<object id>]`.
    fn mail_illegal_action(&mut self, id: ObjectId, what: &str) {
        let name = self.players.get(&id).map_or("?", |p| p.data.name.as_str());
        let message = format!("Player:{name}[{}] {what}", id.0);
        let punishment = self.default_punish();
        self.handle_illegal_player_action(id, &message, punishment);
    }

    /// The `peace zone` text three of the packets share.
    fn mail_peace_zone_attachments_message(&self, id: ObjectId) {
        self.mail_tell_id(
            id,
            SystemMessageId::YouCannotReceiveOrSendMailWithAttachedItemsInNonPeaceZoneRegions,
        );
    }

    /// Java `RequestPostItemList.runImpl`.
    fn on_request_post_item_list(&mut self, id: ObjectId) {
        if !self.mail.settings.allow_mail || !self.mail.settings.allow_attachments {
            return;
        }
        if !self.is_inside_zone(id, ZoneId::Peace) {
            self.mail_peace_zone_attachments_message(id);
            return;
        }
        let list = self.mail_post_item_list(id);
        self.send(id, &list);
    }

    /// Java `RequestReceivedPost.runImpl`.
    fn on_request_received_post(&mut self, id: ObjectId, msg_id: i32) {
        if !self.mail.settings.allow_mail {
            return;
        }
        let Some(msg) = self.mail_message(msg_id) else {
            return;
        };
        let (has_attachments, receiver_id, deleted) =
            (msg.has_attachments, msg.receiver_id, msg.deleted_by_receiver);
        if !self.is_inside_zone(id, ZoneId::Peace) && has_attachments {
            self.mail_peace_zone_attachments_message(id);
            return;
        }
        if receiver_id != id.0 {
            self.mail_illegal_action(id, "tried to receive not own post!");
            return;
        }
        if deleted {
            return;
        }
        if let Some(msg) = self.mail_message(msg_id) {
            let reply = self.mail_reply_received(msg);
            self.send(id, &reply);
        }
        self.send(
            id,
            &ExChangePostState {
                received_board: true,
                msg_ids: vec![msg_id],
                change_id: POST_STATE_READED,
            },
        );
        self.mail_mark_as_read(msg_id);
    }

    /// Java `RequestSentPost.runImpl`.
    fn on_request_sent_post(&mut self, id: ObjectId, msg_id: i32) {
        if !self.mail.settings.allow_mail {
            return;
        }
        let Some(msg) = self.mail_message(msg_id) else {
            return;
        };
        if !self.is_inside_zone(id, ZoneId::Peace) && msg.has_attachments {
            self.mail_peace_zone_attachments_message(id);
            return;
        }
        if msg.sender_id != id.0 {
            self.mail_illegal_action(id, "tried to read not own post!");
            return;
        }
        if msg.deleted_by_sender {
            return;
        }
        let reply = self.mail_reply_sent(msg);
        self.send(id, &reply);
    }

    /// Java `RequestDeleteReceivedPost.runImpl` (`received`) and `RequestDeleteSentPost.runImpl`.
    fn on_request_delete_post(&mut self, id: ObjectId, msg_ids: Option<Vec<i32>>, received: bool) {
        // `readImpl` drops a count above `MaxItemInPacket`.
        let Some(msg_ids) = msg_ids.filter(|v| v.len() <= self.max_item_in_packet()) else {
            return;
        };
        if !self.mail.settings.allow_mail {
            return;
        }
        if !self.is_inside_zone(id, ZoneId::Peace) {
            self.mail_peace_zone_attachments_message(id);
            return;
        }
        for &msg_id in &msg_ids {
            let Some(msg) = self.mail_message(msg_id) else {
                continue;
            };
            let (owner, gone) = if received {
                (msg.receiver_id, msg.deleted_by_receiver)
            } else {
                (msg.sender_id, msg.deleted_by_sender)
            };
            if owner != id.0 {
                self.mail_illegal_action(id, "tried to delete not own post!");
                return;
            }
            // NOTE(java-quirk): this `return` ends the whole request without an answer, the mails
            // before it are already deleted.
            if msg.has_attachments || gone {
                return;
            }
            if received {
                self.mail_set_deleted_by_receiver(msg_id);
            } else {
                self.mail_set_deleted_by_sender(msg_id);
            }
        }
        self.send(
            id,
            &ExChangePostState {
                received_board: received,
                msg_ids,
                change_id: POST_STATE_DELETED,
            },
        );
    }

    /// The slots and the weight the attachments need in the inventory of `player`
    /// (`RequestPostAttachment` and `RequestCancelPostAttachment`).
    // NOTE(java-quirk): `int weight` and `int slots` take `long * int` and `long` sums, which wrap.
    fn mail_slots_and_weight(&self, player: ObjectId, items: &[ItemData]) -> (i64, i64) {
        let templates = self.item_templates();
        let (mut weight, mut slots) = (0i32, 0i32);
        for item in items {
            let unit = templates
                .as_ref()
                .and_then(|t| t.get(item.item_id))
                .map_or(0, |t| i64::from(t.weight));
            weight = (i64::from(weight).wrapping_add(item.count.wrapping_mul(unit))) as i32;
            if !self.data.item_stackable(item.item_id) {
                slots = (i64::from(slots).wrapping_add(item.count)) as i32;
            } else if self.inventory_item_by_item_id(player, item.item_id).is_none() {
                slots = slots.wrapping_add(1);
            }
        }
        (i64::from(slots), i64::from(weight))
    }

    /// The loop that moves every attachment of a mail into the inventory of `player`, shared by
    /// `RequestPostAttachment` and `RequestCancelPostAttachment`. `false` when a move failed
    /// (Java returns on the spot).
    fn mail_take_attachments(&mut self, player: ObjectId, msg_id: i32, sender: i32) -> bool {
        let container = ContainerRef::Mail {
            message: msg_id,
            sender,
        };
        let items: Vec<(ObjectId, i32, i64)> = self
            .mail_attachments(msg_id)
            .unwrap_or_default()
            .iter()
            .map(|i| (i.object_id, i.item_id, i.count))
            .collect();
        let mut ops: Vec<ItemRowOp> = Vec::new();
        let mut entries: Vec<ItemUpdate> = Vec::new();
        let mut ok = true;
        for (object_id, item_id, count) in items {
            let Some(moved) = self.move_item(
                Place::Container(container),
                object_id,
                count,
                Place::Inventory(player),
                player,
                &mut ops,
            ) else {
                ok = false;
                break;
            };
            let change = if self.data.item_stackable(moved.new.item_id) && moved.new.count > count {
                InventoryUpdate::MODIFIED
            } else {
                InventoryUpdate::ADDED
            };
            entries.push(self.update_entry(&moved.new, change));
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveAcquiredS2S1)
                    .with(SystemMessageParam::ItemName(item_id))
                    .with(SystemMessageParam::LongNumber(count)),
            );
        }
        self.flush_item_rows(player, ops);
        if !ok {
            return false;
        }
        // Send updated item list to the player.
        for entry in entries {
            self.send_inventory_update(player, entry);
        }
        // Send full list to avoid duplicates.
        self.send_item_list(player, false);
        true
    }

    /// Java `RequestPostAttachment.runImpl`, up to the point where it reads the attachments: the
    /// rows are read again first (M6-SYNC-01, see `refresh.rs`) and the answer calls
    /// [`post_attachment`](Self::post_attachment) with `refreshed`.
    fn on_request_post_attachment(&mut self, id: ObjectId, msg_id: i32) {
        self.post_attachment(id, msg_id, false);
    }

    /// Java `RequestPostAttachment.runImpl`. `refreshed` is false for the packet and true when the
    /// attachment rows were read again; the checks run in both passes, because the state of the
    /// player may have changed while the database answered.
    pub(super) fn post_attachment(&mut self, id: ObjectId, msg_id: i32, refreshed: bool) {
        if !self.mail.settings.allow_mail || !self.mail.settings.allow_attachments {
            return;
        }
        // The second pass (`refreshed`) is the same request: the interval was started by the first.
        if !refreshed && !self.flood_allows(id, FloodKind::Transaction) {
            return;
        }
        if !self.mail_allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your Access Level");
            return;
        }
        if !self.is_inside_zone(id, ZoneId::Peace) {
            self.mail_tell_id(id, SystemMessageId::YouCannotReceiveInANonPeaceZoneLocation);
            return;
        }
        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) {
            self.mail_tell_id(id, SystemMessageId::YouCannotReceiveDuringAnExchange);
            return;
        }
        if self.is_enchanting(id) {
            self.mail_tell_id(
                id,
                SystemMessageId::YouCannotReceiveDuringAnItemEnhancementOrAttributeEnhancement,
            );
            return;
        }
        if self.is_in_store_mode(id) {
            self.mail_tell_id(
                id,
                SystemMessageId::YouCannotReceiveBecauseThePrivateShopOrWorkshopIsInProgress,
            );
            return;
        }
        let Some(msg) = self.mail_message(msg_id) else {
            return;
        };
        if msg.receiver_id != id.0 {
            self.mail_illegal_action(id, "tried to get not own attachment!");
            return;
        }
        if !msg.has_attachments {
            return;
        }
        let (sender_id, adena) = (msg.sender_id, msg.req_adena);
        // Java also checks that every item of the container belongs to the sender, sits in the
        // `MAIL` location and carries this message's id (`handleIllegalPlayerAction`
        // "wrong item"). The container is built from exactly those rows, so nothing can fail.
        let items = msg.attachments.clone();
        if !refreshed {
            // The Java GS may have given the attachments out already (D-39): ask for the rows.
            self.mail_request_attachments(id, msg_id, sender_id, AttachmentAction::Take);
            return;
        }
        let (slots, weight) = self.mail_slots_and_weight(id, &items);

        // Item Max Limit Check.
        if !self.validate_capacity(id, slots, false) {
            self.mail_tell_id(id, SystemMessageId::YouCouldNotReceiveBecauseYourInventoryIsFull);
            return;
        }
        // Weight limit Check.
        if !self.validate_weight(id, weight) {
            self.mail_tell_id(id, SystemMessageId::YouCouldNotReceiveBecauseYourInventoryIsFull);
            return;
        }

        if adena > 0 && !self.reduce_adena(id, adena, true) {
            self.mail_tell_id(id, SystemMessageId::YouCannotReceiveBecauseYouDonTHaveEnoughAdena);
            return;
        }

        // Proceed to the transfer.
        if !self.mail_take_attachments(id, msg_id, sender_id) {
            return;
        }
        self.mail_remove_attachments(msg_id);
        // Update current load status on player.
        self.send_cur_load(id);

        let player_name = self
            .players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let sender = ObjectId(sender_id);
        let sender_online = self.players.contains_key(&sender);
        if adena > 0 {
            if sender_online {
                self.add_adena(sender, adena, false);
                self.send(
                    sender,
                    &SystemMessage::new(SystemMessageId::S2HasMadeAPaymentOfS1AdenaPerYourPaymentRequestMail)
                        .with(SystemMessageParam::LongNumber(adena))
                        .with(SystemMessageParam::Text(player_name)),
                );
            } else {
                self.mail_pay_offline_sender(id, sender, adena);
            }
        } else if sender_online {
            self.send(
                sender,
                &SystemMessage::new(SystemMessageId::S1AcquiredTheAttachedItemToYourMail)
                    .with(SystemMessageParam::Text(player_name)),
            );
        }

        self.send(
            id,
            &ExChangePostState {
                received_board: true,
                msg_ids: vec![msg_id],
                change_id: POST_STATE_READED,
            },
        );
        self.mail_tell_id(id, SystemMessageId::MailSuccessfullyReceived);
        let list = self.mail_received_list(id.0);
        self.send(id, &list);
    }

    /// The payment of a cash on delivery mail when the sender is offline: a new adena item in the
    /// sender's inventory (`ItemManager.createItem`, `setOwnerId`, `updateDatabase(true)`).
    // NOTE(java-quirk): it is a new stack, not added to the adena the sender already has; the
    // inventory shows two stacks until something merges them.
    fn mail_pay_offline_sender(&mut self, receiver: ObjectId, sender: ObjectId, adena: i64) {
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(ADENA_ID) else {
            return;
        };
        let Some(object_id) = self.allocate_item_id() else {
            return;
        };
        let mut item = self.new_item_data(template, object_id, adena);
        item.loc = LOC_INVENTORY.to_string();
        item.loc_data = 0;
        self.flush_item_rows(
            receiver,
            vec![ItemRowOp::Insert {
                owner_id: sender.0,
                item,
            }],
        );
    }

    /// Java `RequestRejectPostAttachment.runImpl`.
    fn on_request_reject_post_attachment(&mut self, id: ObjectId, msg_id: i32) {
        self.reject_attachment(id, msg_id, false);
    }

    /// Java `RequestRejectPostAttachment.runImpl`; `refreshed` as in
    /// [`post_attachment`](Self::post_attachment).
    pub(super) fn reject_attachment(&mut self, id: ObjectId, msg_id: i32, refreshed: bool) {
        if !self.mail.settings.allow_mail || !self.mail.settings.allow_attachments {
            return;
        }
        // The second pass (`refreshed`) is the same request: the interval was started by the first.
        if !refreshed && !self.flood_allows(id, FloodKind::Transaction) {
            return;
        }
        if !self.is_inside_zone(id, ZoneId::Peace) {
            self.mail_peace_zone_attachments_message(id);
            return;
        }
        let Some(msg) = self.mail_message(msg_id) else {
            return;
        };
        if msg.receiver_id != id.0 {
            self.mail_illegal_action(id, "tried to reject not own attachment!");
            return;
        }
        if !msg.has_attachments || msg.send_by_system != 0 {
            return;
        }
        let sender_id = msg.sender_id;
        if !refreshed {
            // The returned mail takes the attachments along: ask what the database has first.
            self.mail_request_attachments(id, msg_id, sender_id, AttachmentAction::Reject);
            return;
        }

        // `new MailMessage(msg)`.
        let Some(new_id) = self.allocate_item_id() else {
            warn!("No object id for the returned mail {msg_id}.");
            return;
        };
        let Some(original) = self.mail.messages.get_mut(&msg_id) else {
            return;
        };
        let returned = MailMessage::new_returned(new_id.0, original, unix_ms_now());
        self.mail_persist(MailWrite::RemoveAttachments(msg_id));
        let ops: Vec<ItemRowOp> = returned
            .attachments
            .iter()
            .map(|item| ItemRowOp::Update {
                owner_id: returned.sender_id,
                item: item.clone(),
            })
            .collect();
        self.flush_item_rows(id, ops);
        self.mail_send_message(returned);

        self.mail_tell_id(id, SystemMessageId::MailSuccessfullyReturned);
        self.send(
            id,
            &ExChangePostState {
                received_board: true,
                msg_ids: vec![msg_id],
                change_id: POST_STATE_REJECTED,
            },
        );
        let sender = ObjectId(sender_id);
        if self.players.contains_key(&sender) {
            let name = self
                .players
                .get(&id)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            self.send(
                sender,
                &SystemMessage::new(SystemMessageId::S1ReturnedTheMail).with(SystemMessageParam::Text(name)),
            );
        }
    }

    /// Java `RequestCancelPostAttachment.runImpl`.
    fn on_request_cancel_post_attachment(&mut self, id: ObjectId, msg_id: i32) {
        self.cancel_attachment(id, msg_id, false);
    }

    /// Java `RequestCancelPostAttachment.runImpl`; `refreshed` as in
    /// [`post_attachment`](Self::post_attachment).
    pub(super) fn cancel_attachment(&mut self, id: ObjectId, msg_id: i32, refreshed: bool) {
        if !self.mail.settings.allow_mail || !self.mail.settings.allow_attachments {
            return;
        }
        // The second pass (`refreshed`) is the same request: the interval was started by the first.
        if !refreshed && !self.flood_allows(id, FloodKind::Transaction) {
            return;
        }
        let Some(msg) = self.mail_message(msg_id) else {
            return;
        };
        if msg.sender_id != id.0 {
            self.mail_illegal_action(id, "tried to cancel not own post!");
            return;
        }
        if !self.is_inside_zone(id, ZoneId::Peace) {
            self.mail_tell_id(id, SystemMessageId::YouCannotCancelInANonPeaceZoneLocation);
            return;
        }
        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) {
            self.mail_tell_id(id, SystemMessageId::YouCannotCancelDuringAnExchange);
            return;
        }
        if self.is_enchanting(id) {
            self.mail_tell_id(
                id,
                SystemMessageId::YouCannotCancelDuringAnItemEnhancementOrAttributeEnhancement,
            );
            return;
        }
        if self.is_in_store_mode(id) {
            self.mail_tell_id(
                id,
                SystemMessageId::YouCannotCancelBecauseThePrivateShopOrWorkshopIsInProgress,
            );
            return;
        }
        if !msg.has_attachments || msg.attachments.is_empty() {
            self.mail_tell_id(
                id,
                SystemMessageId::YouCannotCancelSentMailSinceTheRecipientReceivedIt,
            );
            return;
        }
        let (receiver_id, sender_id) = (msg.receiver_id, msg.sender_id);
        // Java's checks that the items are the sender's, in the `MAIL` location and of this
        // message cannot fail here (see `on_request_post_attachment`).
        let items = msg.attachments.clone();
        if !refreshed {
            self.mail_request_attachments(id, msg_id, sender_id, AttachmentAction::Cancel);
            return;
        }
        let (slots, weight) = self.mail_slots_and_weight(id, &items);

        if !self.validate_capacity(id, slots, false) {
            self.mail_tell_id(
                id,
                SystemMessageId::YouCouldNotCancelReceiptBecauseYourInventoryIsFull,
            );
            return;
        }
        if !self.validate_weight(id, weight) {
            self.mail_tell_id(
                id,
                SystemMessageId::YouCouldNotCancelReceiptBecauseYourInventoryIsFull,
            );
            return;
        }

        // Proceed to the transfer.
        if !self.mail_take_attachments(id, msg_id, sender_id) {
            return;
        }
        self.mail_remove_attachments(msg_id);
        // Update current load status on player.
        self.send_cur_load(id);

        let receiver = ObjectId(receiver_id);
        if self.players.contains_key(&receiver) {
            let name = self
                .players
                .get(&id)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            self.send(
                receiver,
                &SystemMessage::new(SystemMessageId::S1CanceledTheSentMail)
                    .with(SystemMessageParam::Text(name)),
            );
            self.send(
                receiver,
                &ExChangePostState {
                    received_board: true,
                    msg_ids: vec![msg_id],
                    change_id: POST_STATE_DELETED,
                },
            );
        }

        self.mail_delete_message(msg_id);

        self.send(
            id,
            &ExChangePostState {
                received_board: false,
                msg_ids: vec![msg_id],
                change_id: POST_STATE_DELETED,
            },
        );
        self.mail_tell_id(id, SystemMessageId::MailSuccessfullyCancelled);
    }
}
