//! The exchange of a trade: weight and slot checks, the item moves and what the two players are
//! told. The item moves (`transfer_items`, `trade_items_weight`, `trade_items_slots`) are what the
//! private stores (STORE-01) use as well.
// Java: network/holders/TradeList.java (doExchange, TransferItems, countItemsSlots,
//       calcItemsWeight)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{InventoryUpdate, ItemUpdate, SystemMessage};
use l2_util::ObjectId;

use super::{TradeItem, TradeList};
use crate::world::World;

impl World {
    /// Java `TradeList.calcItemsWeight()`: the weight of everything on the list.
    pub(crate) fn trade_items_weight(&self, list: &TradeList) -> i64 {
        let mut weight: i64 = 0;
        for item in &list.items {
            // `ItemData.getTemplate(...) == null`: skipped.
            let Some(template) = self.data.item(item.item.item_id) else {
                continue;
            };
            weight = weight.wrapping_add(item.count.wrapping_mul(i64::from(template.weight)));
        }
        weight.min(i64::from(i32::MAX))
    }

    /// Java `TradeList.countItemsSlots(partner)`: the slots the list needs in `partner`'s
    /// inventory.
    pub(crate) fn trade_items_slots(&self, list: &TradeList, partner: ObjectId) -> i64 {
        // Java's `int slots` and a `long` count: the sum is cut to an int each time.
        let mut slots: i32 = 0;
        for item in &list.items {
            if self.data.item(item.item.item_id).is_none() {
                continue;
            }
            if !self.data.item_stackable(item.item.item_id) {
                slots = slots.wrapping_add(item.count as i32);
            } else if self
                .inventory_item_by_item_id(partner, item.item.item_id)
                .is_none()
            {
                slots = slots.wrapping_add(1);
            }
        }
        i64::from(slots)
    }

    /// Java `TradeList.TransferItems(partner, ownerIU, partnerIU)`: every item of `items` goes
    /// from `owner` to `partner`. The two `InventoryUpdate`s collect what changed on each side.
    /// Stops at the first item that cannot move (Java returns `false`; the callers ignore it).
    pub(crate) fn transfer_items(
        &mut self,
        owner: ObjectId,
        items: &[TradeItem],
        partner: ObjectId,
        owner_iu: &mut Vec<ItemUpdate>,
        partner_iu: &mut Vec<ItemUpdate>,
    ) -> bool {
        for titem in items {
            if self.inventory_item(owner, titem.object_id).is_none() {
                return false;
            }
            let Some(moved) = self.transfer_item(owner, titem.object_id, titem.count, partner, owner) else {
                return false;
            };
            // Add changes to inventory update packets.
            if moved.old.count > 0 && !moved.moved_whole {
                owner_iu.push(self.update_entry(&moved.old, InventoryUpdate::MODIFIED));
            } else {
                owner_iu.push(self.update_entry(&moved.old, InventoryUpdate::REMOVED));
            }
            if moved.new.count > titem.count {
                partner_iu.push(self.update_entry(&moved.new, InventoryUpdate::MODIFIED));
            } else {
                partner_iu.push(self.update_entry(&moved.new, InventoryUpdate::ADDED));
            }
        }
        true
    }

    /// Java `TradeList.doExchange(partnerList)`, run by the list of `owner` (who confirmed
    /// second) with `partner`'s list (who confirmed first).
    pub(crate) fn do_exchange(&mut self, owner: ObjectId, partner: ObjectId) {
        let own_list = self.players.get(&owner).and_then(|p| p.trade.active.clone());
        let partner_list = self.players.get(&partner).and_then(|p| p.trade.active.clone());
        let (Some(own_list), Some(partner_list)) = (own_list, partner_list) else {
            return;
        };
        let mut success = false;

        // Check weight and slots.
        if !self.validate_weight(owner, self.trade_items_weight(&partner_list))
            || !self.validate_weight(partner, self.trade_items_weight(&own_list))
        {
            self.send(
                partner,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
            self.send(
                owner,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
        } else if !self.validate_capacity(owner, self.trade_items_slots(&partner_list, owner), false)
            || !self.validate_capacity(partner, self.trade_items_slots(&own_list, partner), false)
        {
            self.send(partner, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            self.send(owner, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
        } else {
            // Prepare inventory update packet.
            let mut owner_iu = Vec::new();
            let mut partner_iu = Vec::new();

            // Transfer items. The partner's items come first.
            self.transfer_items(
                partner,
                &partner_list.items,
                owner,
                &mut partner_iu,
                &mut owner_iu,
            );
            self.transfer_items(owner, &own_list.items, partner, &mut owner_iu, &mut partner_iu);
            for entry in owner_iu {
                self.send_inventory_update(owner, entry);
            }
            for entry in partner_iu {
                self.send_inventory_update(partner, entry);
            }

            // The client drops part of the update when the trade window closes, so the full list
            // is sent as well.
            self.send_item_list(owner, false);
            self.send_item_list(partner, false);

            // Update current load as well.
            self.send_cur_load(owner);
            self.send_cur_load(partner);

            success = true;
        }

        // Finish the trade.
        self.on_trade_finish(partner, success);
        self.on_trade_finish(owner, success);
    }
}
