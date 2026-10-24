//! `RequestSellItem`: selling items to a merchant, into the refund list.
// Java: network/clientpackets/RequestSellItem.java
// Java: entity/itemcontainer/{ItemContainer,PlayerRefund}.java (transferItem, addItem)
// Java: entity/actor/Player.java (checkItemManipulation)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::{RequestSellItem, SellItemEntry};
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::REFUND_LIST_SIZE;
use crate::flood::FloodKind;
use crate::player::ItemData;
use crate::world::World;

/// `ItemProcessType`'s `REFUND` as the `items.loc` column spells it.
pub(super) const LOC_REFUND: &str = "REFUND";

impl World {
    /// Java `Item.isSellable()`: the template, and an augmented item only with
    /// `AltAllowAugmentTrade`.
    // The transmog check (`isEquipable() && getTransmogId() > 0`) needs the appearance system,
    // which does not exist; no item has a transmog id.
    fn item_is_sellable(&self, item: &ItemData, sellable: bool) -> bool {
        if !sellable {
            return false;
        }
        if item.is_augmented() {
            return self.settings.inventory.alt_allow_augment_trade;
        }
        true
    }

    /// Java `Player.hasRefund()`: items are waiting and refunds are on.
    pub(crate) fn has_refund(&self, id: ObjectId) -> bool {
        self.settings.shop.allow_refund && self.players.get(&id).is_some_and(|p| !p.shop.refund.is_empty())
    }

    /// Java `PlayerRefund.addItem(Item)`: the item joins the list; with more than 12 the first
    /// one is destroyed.
    // NOTE(java-quirk): Java removes `_items.stream().findFirst()` of a hash set, i.e. an item of
    // no particular age (it can be the one just added). The oldest one goes here.
    fn refund_add(&mut self, id: ObjectId, item: ItemData) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.shop.refund.push(item);
        if player.shop.refund.len() > REFUND_LIST_SIZE {
            let removed = player.shop.refund.remove(0);
            // `ItemManager.destroyItem(REFUND, ...)` releases the object id.
            self.release_item_id(removed.object_id);
        }
    }

    /// Java `Player.clearRefund()` / `PlayerRefund.deleteMe()`: every sold item is destroyed.
    pub(crate) fn clear_refund(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let items = std::mem::take(&mut player.shop.refund);
        for item in items {
            self.release_item_id(item.object_id);
        }
    }

    /// Java `PlayerInventory.transferItem(TRANSFER, objectId, count, player.getRefund(), ...)`
    /// for an item of the player. `false` when the item could not be moved.
    fn transfer_to_refund(&mut self, id: ObjectId, object_id: ObjectId, count: i64) -> bool {
        let Some(source) = self.inventory_item(id, object_id).cloned() else {
            return false;
        };
        let Some(templates) = self.item_templates() else {
            return false;
        };
        let Some(template) = templates.get(source.item_id) else {
            return false;
        };
        let stackable = template.stackable;
        let target_present = stackable
            && self
                .players
                .get(&id)
                .is_some_and(|p| p.shop.refund.iter().any(|i| i.item_id == source.item_id));

        // Check if requested quantity is available.
        let count = count.min(source.count);

        // If possible, move entire item object.
        if source.count == count && !target_present && !stackable {
            let Some(mut moved) = self.remove_inventory_item(id, object_id) else {
                return false;
            };
            moved.loc = LOC_REFUND.to_string();
            moved.loc_data = 0;
            self.refund_add(id, moved);
            // The row leaves the inventory (see the module notes: no `REFUND` row is written).
            self.remove_item_from_db(id, object_id);
            self.refresh_weight(id);
            return true;
        }

        // A new refund item needs an id before anything changes.
        let new_id = if target_present {
            None
        } else {
            match self.allocate_item_id() {
                Some(new_id) => Some(new_id),
                None => return false,
            }
        };

        let remaining = if source.count > count {
            // If possible, only update counts.
            self.change_item_count(id, object_id, -count)
        } else {
            // Otherwise destroy old item.
            self.destroy_inventory_item(id, object_id, source.count)
        };
        let Some(remaining) = remaining else {
            return false;
        };

        match new_id {
            // If possible, only update counts.
            None => {
                if let Some(player) = self.players.get_mut(&id)
                    && let Some(existing) = player
                        .shop
                        .refund
                        .iter_mut()
                        .find(|i| i.item_id == source.item_id)
                {
                    existing.count = existing.count.saturating_add(count);
                }
            }
            // Otherwise add new item.
            Some(new_id) => {
                let mut created =
                    self.new_item_data(template, new_id, if template.stackable { count } else { 1 });
                created.loc = LOC_REFUND.to_string();
                created.loc_data = 0;
                created.enchant_level = template.default_enchant_level;
                self.refund_add(id, created);
            }
        }

        // `sourceitem.updateDatabase(true)`
        if remaining.count > 0 {
            self.update_item_in_db(id, object_id);
        }
        self.refresh_weight(id);
        true
    }

    /// Java `RequestSellItem.runImpl`.
    pub(crate) fn on_request_sell_item(&mut self, id: ObjectId, request: RequestSellItem) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.flood_allows(id, FloodKind::Transaction) {
            self.send_message(id, "You are selling too fast.");
            return;
        }

        let list_id = request.list_id;
        // (The decoder already drops an entry below 1; the world input can come from elsewhere.)
        let items: Option<Vec<SellItemEntry>> = request.items.filter(|v| {
            v.len() <= self.max_item_in_packet()
                && v.iter()
                    .all(|e| e.object_id >= 1 && e.item_id >= 1 && e.count >= 1)
        });
        let Some(items) = items else {
            self.send(id, &ActionFailed);
            return;
        };

        // Alt game - Karma punishment.
        if !self.settings.talk.karma_player_can_shop
            && self.players.get(&id).is_some_and(|p| p.data.karma > 0)
        {
            self.send(id, &ActionFailed);
            return;
        }

        let Ok(merchant) = self.shop_merchant(id, list_id) else {
            return;
        };

        let Some(list) = self.shop.lists.as_ref().and_then(|l| l.get(list_id)).cloned() else {
            self.shop_illegal_action(id, &format!("sent a false BuyList list_id {list_id}"));
            return;
        };

        if let Some(npc) = merchant {
            let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
            if !list.is_npc_allowed(npc_id) {
                self.send(id, &ActionFailed);
                return;
            }
        }

        let Some(templates) = self.item_templates() else {
            self.send(id, &ActionFailed);
            return;
        };
        let max_adena = self.settings.ground.max_adena;

        let mut total_price: i64 = 0;
        // Proceed the sell
        for entry in &items {
            let object_id = ObjectId(entry.object_id);
            let Some(item) = self.check_item_manipulation(id, object_id, entry.count) else {
                continue;
            };
            let Some(template) = templates.get(item.item_id) else {
                continue;
            };
            if !self.item_is_sellable(&item, template.sellable) {
                continue;
            }

            let price = i64::from(template.price / 2);
            total_price = total_price.wrapping_add(price.wrapping_mul(entry.count));
            if (max_adena / entry.count) < price || total_price > max_adena {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouHaveExceededYourOutOfPocketAdenaLimit),
                );
                return;
            }

            if self.settings.shop.allow_refund {
                if !self.transfer_to_refund(id, object_id, entry.count) {
                    warn!("{id}: could not move item {} to the refund list", entry.object_id);
                }
            } else {
                self.destroy_inventory_item(id, object_id, entry.count);
            }
        }

        if !self.settings.shop.merchant_zero_sell_price {
            self.add_adena(id, total_price, false);
        }

        // Update current load as well.
        self.send_cur_load(id);
        self.send_ex_buy_sell_list(id, true);
        self.send(id, &SystemMessage::new(SystemMessageId::TheTransactionIsComplete));
    }
}
