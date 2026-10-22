//! `RequestRefundItem`: buying back what the player sold.
// Java: network/clientpackets/RequestRefundItem.java
// Java: entity/itemcontainer/{ItemContainer,PlayerInventory}.java (transferItem, addItem)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::RequestRefundItem;
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use crate::world::World;

impl World {
    /// Java `PlayerRefund.transferItem(TRANSFER, objectId, Long.MAX_VALUE, player.getInventory(),
    /// ...)`: one refunded item goes back into the inventory. `false` when it was not there.
    fn transfer_from_refund(&mut self, id: ObjectId, object_id: ObjectId) -> bool {
        let Some(position) = self
            .players
            .get(&id)
            .and_then(|p| p.shop.refund.iter().position(|i| i.object_id == object_id))
        else {
            return false;
        };
        let Some(templates) = self.item_templates() else {
            return false;
        };
        let source = self.players.get(&id).map(|p| p.shop.refund[position].clone());
        let Some(source) = source else {
            return false;
        };
        let stackable = templates.get(source.item_id).is_some_and(|t| t.stackable);
        let target = if stackable {
            self.inventory_item_by_item_id(id, source.item_id).cloned()
        } else {
            None
        };

        // If possible, move entire item object.
        if target.is_none() && !stackable {
            let Some(mut item) = self.players.get_mut(&id).map(|p| p.shop.refund.remove(position)) else {
                return false;
            };
            item.loc = crate::inventory::LOC_INVENTORY.to_string();
            item.loc_data = 0;
            let object_id = item.object_id;
            if self.inventory_add_item(id, item).is_none() {
                return false;
            }
            // `sourceitem.updateDatabase(true)`: the row is back (as `INVENTORY`).
            self.update_item_in_db(id, object_id);
            self.refresh_weight(id);
            return true;
        }

        // Otherwise the refunded item is destroyed and its count goes to the inventory.
        if let Some(player) = self.players.get_mut(&id) {
            player.shop.refund.remove(position);
        }
        self.release_item_id(source.object_id);
        let count = source.count;
        let target_id = match target {
            // If possible, only update counts.
            Some(existing) => {
                self.change_item_count(id, existing.object_id, count);
                Some(existing.object_id)
            }
            // Otherwise add new item (the inventory tells the client).
            None => self
                .inventory_add_item_by_id(id, source.item_id, count)
                .map(|added| added.item.object_id),
        };
        if let Some(target_id) = target_id {
            self.update_item_in_db(id, target_id);
        }
        self.refresh_weight(id);
        true
    }

    /// Java `RequestRefundItem.runImpl`.
    pub(crate) fn on_request_refund_item(&mut self, id: ObjectId, request: RequestRefundItem) {
        if !self.players.contains_key(&id) {
            return;
        }
        // FOLLOW-UP: the flood protector `canPerformTransaction` ("You are using refund too fast.").

        let list_id = request.list_id;
        let items: Option<Vec<i32>> = request.items.filter(|v| v.len() <= self.max_item_in_packet());
        let Some(indexes) = items.filter(|_| self.has_refund(id)) else {
            self.send(id, &ActionFailed);
            return;
        };

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

        let mut weight: i64 = 0;
        let mut adena: i64 = 0;
        let mut slots: i64 = 0;

        let refund: Vec<_> = self
            .players
            .get(&id)
            .map(|p| p.shop.refund.clone())
            .unwrap_or_default();
        let mut object_ids: Vec<ObjectId> = Vec::with_capacity(indexes.len());
        for (i, &idx) in indexes.iter().enumerate() {
            let Some(item) = usize::try_from(idx).ok().and_then(|n| refund.get(n)) else {
                self.shop_illegal_action(id, "sent invalid refund index");
                return;
            };

            // check for duplicates - indexes
            if indexes[i + 1..].contains(&idx) {
                self.shop_illegal_action(id, "sent duplicate refund index");
                return;
            }

            let Some(template) = templates.get(item.item_id) else {
                self.send(id, &ActionFailed);
                return;
            };
            object_ids.push(item.object_id);

            // second check for duplicates - object ids
            if object_ids[..i].contains(&item.object_id) {
                self.shop_illegal_action(id, "has duplicate items in refund list");
                return;
            }

            let count = item.count;
            weight = weight.wrapping_add(count.wrapping_mul(i64::from(template.weight)));
            adena = adena.wrapping_add(count.wrapping_mul(i64::from(template.price / 2)));
            if !template.stackable {
                slots = slots.wrapping_add(count);
            } else if self.inventory_item_by_item_id(id, template.id).is_none() {
                slots += 1;
            }
        }

        // No GM exemption here, unlike buying.
        if weight > i64::from(i32::MAX) || weight < 0 || !self.validate_weight(id, weight) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
            self.send(id, &ActionFailed);
            return;
        }

        if slots > i64::from(i32::MAX) || slots < 0 || !self.validate_capacity(id, slots, false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            self.send(id, &ActionFailed);
            return;
        }

        if !self.settings.shop.merchant_zero_sell_price && (adena < 0 || !self.reduce_adena(id, adena, false))
        {
            self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
            self.send(id, &ActionFailed);
            return;
        }

        for object_id in object_ids {
            if !self.transfer_from_refund(id, object_id) {
                warn!("Error refunding object for char {id} (newitem == null)");
            }
        }

        // Update current load status on player.
        self.send_cur_load(id);
        self.send_ex_buy_sell_list(id, true);
    }
}
