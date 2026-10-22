//! The deals of the private stores: a customer buys from a sell store or package sale
//! (`RequestPrivateStoreBuy`, `TradeList.privateStoreBuy`) or sells to a buy store
//! (`RequestPrivateStoreSell`, `TradeList.privateStoreSell`).
// Java: network/clientpackets/{RequestPrivateStoreBuy,RequestPrivateStoreSell}.java
// Java: network/holders/{TradeList,RequestTrade}.java (privateStoreBuy, privateStoreSell)
// Java: entity/itemcontainer/{PlayerInventory,ItemContainer}.java (reduceAdena, addAdena,
//       transferItem)
//
// # How Java's code is mirrored
// `privateStoreBuy` / `privateStoreSell` are methods of the store's `TradeList` and change both
// inventories. Here the list of the store owner is copied out of the player, worked on, and put
// back when the deal is over (the same in every way out, so a `lock()` is not lost), and the
// inventories are changed through the `World` like everywhere else.
//
// # Java quirks that are kept
// See the `NOTE(java-quirk)` comments: the inverted `RequestTrade.equals`, the item id of a
// buy request that is always 0, and the adena item that is gone when a customer pays with all
// of its adena.

use l2_data::item::ADENA_ID;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::{StoreBuyRequestEntry, StoreSellRequestEntry};
use l2_protocol_game::server::{
    ActionFailed, InventoryUpdate, ItemUpdate, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use super::TradeList;
use super::store::PrivateStoreType;
use crate::dialog::INTERACTION_DISTANCE;
use crate::flood::FloodKind;
use crate::inventory::item_name_param;
use crate::world::World;

/// Java `RequestTrade`: one line of a customer's request. A buy request has no item id (Java's
/// three-argument constructor leaves it 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RequestTrade {
    object_id: i32,
    item_id: i32,
    count: i64,
    price: i64,
}

/// What `TradeList.privateStoreBuy` answers: 0 is ok, 1 canceled (no adena, bad price...), 2
/// failed (an item error), and Java's exception when the buyer paid with every adena it had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuyResult {
    Ok,
    Canceled,
    Failed,
    /// A Java `NullPointerException` or `ArithmeticException`: nothing else happens, the client
    /// gets no answer.
    Exception,
}

/// The iteration order of a Java `HashSet` whose elements hash to `key`: by bucket
/// `(h ^ (h >>> 16)) & (capacity - 1)` of the final table (16 at first, doubled while the set holds
/// more than three quarters of it), then insertion order.
fn hash_set_order<T>(entries: Vec<T>, key: impl Fn(&T) -> i32) -> Vec<T> {
    let mut capacity: usize = 16;
    while entries.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let bucket = |entry: &T| {
        let h = key(entry) as u32;
        ((h ^ (h >> 16)) as usize) & (capacity - 1)
    };
    let mut indexed: Vec<(usize, usize, T)> = entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| (bucket(&entry), index, entry))
        .collect();
    indexed.sort_by_key(|(bucket, index, _)| (*bucket, *index));
    indexed.into_iter().map(|(_, _, entry)| entry).collect()
}

impl World {
    /// `World.getPlayer(id) != null` and the checks both customer packets make before the deal:
    /// the store is within `INTERACTION_DISTANCE` and in the same instance (or the customer is in
    /// the multiverse, -1).
    fn store_in_reach(&self, customer: ObjectId, store: ObjectId) -> bool {
        if self
            .distance_3d(customer, store)
            .is_none_or(|d| d >= f64::from(INTERACTION_DISTANCE))
        {
            return false;
        }
        let instance = |o: ObjectId| self.objects.get(&o).map_or(0, |e| e.instance_id);
        !(instance(customer) != instance(store) && instance(customer) != -1)
    }

    // ------------------------------------------------------------------ RequestPrivateStoreBuy

    /// Java `RequestPrivateStoreBuy.runImpl`.
    pub(super) fn on_request_private_store_buy(
        &mut self,
        id: ObjectId,
        store: ObjectId,
        items: Option<Vec<StoreBuyRequestEntry>>,
    ) {
        // Java drops a list with more than `MAX_ITEM_IN_PACKET` items already when it reads it.
        let max = usize::try_from(self.settings.store.max_item_in_packet).unwrap_or(0);
        let Some(items) = items.filter(|list| list.len() <= max) else {
            self.send(id, &ActionFailed);
            return;
        };

        if !self.flood_allows(id, FloodKind::Transaction) {
            self.send_message(id, "You are buying items too fast.");
            return;
        }

        if !self.players.contains_key(&store) {
            return;
        }
        // `isCursedWeaponEquipped()`: cursed weapons are not ported.
        if !self.store_in_reach(id, store) {
            return;
        }
        let store_type = self.private_store_type(store);
        if !matches!(store_type, PrivateStoreType::Sell | PrivateStoreType::PackageSell) {
            return;
        }
        if !self.allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your Access Level.");
            self.send(id, &ActionFailed);
            return;
        }

        // NOTE(java-quirk): the request is collected in a `HashSet` whose `equals` is inverted
        // (see `RequestPrivateStoreBuy`), so every line is kept, in hash order.
        let requests = hash_set_order(
            items
                .iter()
                .map(|e| RequestTrade {
                    object_id: e.object_id,
                    // NOTE(java-quirk): `new RequestTrade(objectId, cnt, price)` has no item id.
                    item_id: 0,
                    count: e.count,
                    price: e.price,
                })
                .collect(),
            |r| r.object_id,
        );

        let store_item_count = self.sell_list_copy(store).item_count();
        if store_type == PrivateStoreType::PackageSell && store_item_count > requests.len() {
            let text = format!(
                "[RequestPrivateStoreBuy] {} tried to buy less items than sold by package-sell, ban this player for bot usage!",
                self.player_text(id)
            );
            self.store_illegal_action(id, &text);
            return;
        }

        match self.private_store_buy(store, id, requests) {
            BuyResult::Ok => {}
            BuyResult::Canceled => {
                self.send(id, &ActionFailed);
                return;
            }
            BuyResult::Failed => {
                self.send(id, &ActionFailed);
                warn!(
                    "PrivateStore buy has failed due to invalid list or request. Player: {}, Private store of: {}",
                    self.store_name_of(id),
                    self.store_name_of(store)
                );
                return;
            }
            BuyResult::Exception => return,
        }

        // Update offline trade record, if realtime saving is enabled: offline trade is not ported.

        if self.sell_list_copy(store).item_count() == 0 {
            self.set_private_store_type(store, PrivateStoreType::None);
            self.broadcast_user_info(store);
        }
    }

    fn store_name_of(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Java `TradeList.privateStoreBuy(player, items)` on the sell list of `owner`.
    fn private_store_buy(
        &mut self,
        owner: ObjectId,
        player: ObjectId,
        mut items: Vec<RequestTrade>,
    ) -> BuyResult {
        let mut list = self.sell_list_copy(owner);
        let result = self.private_store_buy_on(&mut list, player, &mut items);
        if let Some(slot) = self.sell_list_mut(owner) {
            *slot = list;
        }
        result
    }

    fn private_store_buy_on(
        &mut self,
        list: &mut TradeList,
        player: ObjectId,
        items: &mut [RequestTrade],
    ) -> BuyResult {
        let owner = list.owner;
        if list.locked {
            return BuyResult::Canceled;
        }
        if !self.validate_trade_list(list) {
            list.lock();
            return BuyResult::Canceled;
        }
        if !self.players.contains_key(&owner) || !self.players.contains_key(&player) {
            return BuyResult::Canceled;
        }

        let max_adena = self.settings.ground.max_adena;
        let templates = self.item_templates();
        let mut slots: i32 = 0;
        let mut weight: i32 = 0;
        let mut total_price: i64 = 0;
        for item in items.iter_mut() {
            let mut found = false;
            if let Some(listed) = list.items.iter().find(|t| t.object_id.0 == item.object_id) {
                if listed.price == item.price {
                    if listed.count < item.count {
                        item.count = listed.count;
                    }
                    found = true;
                }
            }

            // Item with this objectId and price not found in tradelist.
            if !found {
                if list.packaged {
                    let text = format!(
                        "[TradeList.privateStoreBuy()] {} tried to cheat the package sell and buy only a part of the package! Ban this player for bot usage!",
                        self.player_text(player)
                    );
                    self.store_illegal_action(player, &text);
                    return BuyResult::Failed;
                }
                item.count = 0;
                continue;
            }

            // Check for overflow in the single item.
            let Some(limit) = max_adena.checked_div(item.count) else {
                // `MAX_ADENA / 0`: Java's `ArithmeticException`.
                return BuyResult::Exception;
            };
            if limit < item.price {
                // Private store attempting to overflow - disable it.
                list.lock();
                return BuyResult::Canceled;
            }

            total_price = total_price.wrapping_add(item.count.wrapping_mul(item.price));

            // Check for overflow of the total price.
            if max_adena < total_price || total_price < 0 {
                // Private store attempting to overflow - disable it.
                list.lock();
                return BuyResult::Canceled;
            }

            // Check if requested item is available for manipulation.
            let old_item = self.check_item_manipulation(owner, ObjectId(item.object_id), item.count);
            let tradeable = old_item.as_ref().is_some_and(|old| {
                templates
                    .as_ref()
                    .and_then(|t| t.get(old.item_id))
                    .is_some_and(|t| self.item_is_tradeable(old, t))
            });
            if !tradeable {
                // Private store sell invalid item - disable it.
                list.lock();
                return BuyResult::Failed;
            }

            // NOTE(java-quirk): `item.getItemId()` of a buy request is 0, which is no template, so
            // neither the weight nor the slots of what the customer buys are ever counted (the
            // checks below are made with 0 and 0).
            let Some(template) = templates.as_ref().and_then(|t| t.get(item.item_id)) else {
                continue;
            };
            // `int weight += long`.
            weight =
                i64::from(weight).wrapping_add(item.count.wrapping_mul(i64::from(template.weight))) as i32;
            if !template.stackable {
                slots = i64::from(slots).wrapping_add(item.count) as i32;
            } else if self.inventory_item_by_item_id(player, item.item_id).is_none() {
                slots = slots.wrapping_add(1);
            }
        }

        if total_price > self.adena_of(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
            );
            return BuyResult::Canceled;
        }
        if !self.validate_weight(player, i64::from(weight)) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
            return BuyResult::Canceled;
        }
        if !self.validate_capacity(player, i64::from(slots), false) {
            self.send(player, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            return BuyResult::Canceled;
        }

        // `playerInventory.reduceAdena(BUY, totalPrice, ...)` is false for a price of 0.
        if self.reduce_adena_quietly(player, total_price).is_none() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
            );
            return BuyResult::Canceled;
        }

        // Prepare inventory update packets.
        let mut owner_iu: Vec<ItemUpdate> = Vec::new();
        let mut player_iu: Vec<ItemUpdate> = Vec::new();
        // NOTE(java-quirk): `playerInventory.getAdenaInstance()` is null once the adena item is
        // used up, and `playerIU.addItem(null)` throws. A customer who pays with every adena it
        // has loses it: the exception ends the packet before the seller is paid or an item moves.
        let Some(adena_item) = self.inventory_item_by_item_id(player, ADENA_ID).cloned() else {
            warn!(
                "{}: paid every adena in a private store purchase; Java's NullPointerException ends the deal here.",
                self.store_name_of(player)
            );
            return BuyResult::Exception;
        };
        player_iu.push(self.update_entry(&adena_item, InventoryUpdate::MODIFIED));
        // `ownerInventory.addAdena(SELL, totalPrice, _owner, player)`: the owner is told by the
        // inventory itself.
        self.inventory_add_item_by_id(owner, ADENA_ID, total_price);

        let mut ok = true;
        // Transfer items.
        for item in items.iter() {
            if item.count == 0 {
                continue;
            }

            // Check if requested item is available for manipulation.
            if self
                .check_item_manipulation(owner, ObjectId(item.object_id), item.count)
                .is_none()
            {
                // Should not happens - validation already done.
                list.lock();
                ok = false;
                break;
            }

            // Proceed with item transfer.
            let Some(moved) = self.transfer_item(owner, ObjectId(item.object_id), item.count, player, owner)
            else {
                ok = false;
                break;
            };

            list.remove_item(item.object_id, -1, item.count);

            // Add changes to inventory update packets.
            if moved.old.count > 0 && !moved.moved_whole {
                owner_iu.push(self.update_entry(&moved.old, InventoryUpdate::MODIFIED));
            } else {
                owner_iu.push(self.update_entry(&moved.old, InventoryUpdate::REMOVED));
            }
            if moved.new.count > item.count {
                player_iu.push(self.update_entry(&moved.new, InventoryUpdate::MODIFIED));
            } else {
                player_iu.push(self.update_entry(&moved.new, InventoryUpdate::ADDED));
            }

            // Send messages about the transaction to both players.
            let stackable = self.data.item_stackable(moved.new.item_id);
            let name = item_name_param(templates.as_deref(), moved.new.item_id);
            let owner_name = self.store_name_of(owner);
            let player_name = self.store_name_of(player);
            let (to_owner, to_player) = if stackable {
                (
                    SystemMessage::new(SystemMessageId::C1PurchasedS3S2S)
                        .with(SystemMessageParam::Text(player_name))
                        .with(name.clone())
                        .with(SystemMessageParam::LongNumber(item.count)),
                    SystemMessage::new(SystemMessageId::YouHavePurchasedS3S2SFromC1)
                        .with(SystemMessageParam::Text(owner_name))
                        .with(name)
                        .with(SystemMessageParam::LongNumber(item.count)),
                )
            } else {
                (
                    SystemMessage::new(SystemMessageId::C1PurchasedS2)
                        .with(SystemMessageParam::Text(player_name))
                        .with(name.clone()),
                    SystemMessage::new(SystemMessageId::YouHavePurchasedS2FromC1)
                        .with(SystemMessageParam::Text(owner_name))
                        .with(name),
                )
            };
            self.send(owner, &to_owner);
            self.send(player, &to_player);
        }

        // Send inventory update packet.
        for entry in owner_iu {
            self.send_inventory_update(owner, entry);
        }
        for entry in player_iu {
            self.send_inventory_update(player, entry);
        }
        if ok { BuyResult::Ok } else { BuyResult::Failed }
    }

    /// Java `PlayerInventory.reduceAdena(process, count, actor, reference)`: takes the adena
    /// without a message or an `InventoryUpdate` (`count > 0` only). Returns the adena item after
    /// the change (count 0 when it is gone), `None` when nothing was taken.
    fn reduce_adena_quietly(&mut self, player: ObjectId, count: i64) -> Option<crate::player::ItemData> {
        if count <= 0 {
            return None;
        }
        let adena = self.inventory_item_by_item_id(player, ADENA_ID)?.object_id;
        self.destroy_inventory_item(player, adena, count)
    }

    // ------------------------------------------------------------------ RequestPrivateStoreSell

    /// Java `RequestPrivateStoreSell.runImpl`.
    pub(super) fn on_request_private_store_sell(
        &mut self,
        id: ObjectId,
        store: ObjectId,
        items: Option<Vec<StoreSellRequestEntry>>,
    ) {
        let max = usize::try_from(self.settings.store.max_item_in_packet).unwrap_or(0);
        let Some(items) = items.filter(|list| list.len() <= max) else {
            self.send(id, &ActionFailed);
            return;
        };

        if !self.flood_allows(id, FloodKind::Transaction) {
            self.send_message(id, "You are selling items too fast.");
            return;
        }

        if !self.players.contains_key(&store) {
            return;
        }
        if !self.store_in_reach(id, store) {
            return;
        }
        if self.private_store_type(store) != PrivateStoreType::Buy {
            return;
        }
        // `isCursedWeaponEquipped()`: cursed weapons are not ported.
        if !self.allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your Access Level.");
            self.send(id, &ActionFailed);
            return;
        }

        let mut requests: Vec<RequestTrade> = items
            .iter()
            .map(|e| RequestTrade {
                object_id: e.object_id,
                item_id: e.item_id,
                count: e.count,
                price: e.price,
            })
            .collect();
        if !self.private_store_sell(store, id, &mut requests) {
            self.send(id, &ActionFailed);
            warn!(
                "PrivateStore sell has failed due to invalid list or request. Player: {}, Private store of: {}",
                self.store_name_of(id),
                self.store_name_of(store)
            );
            return;
        }

        // Update offline trade record, if realtime saving is enabled: offline trade is not ported.

        if self.buy_list_copy(store).item_count() == 0 {
            self.set_private_store_type(store, PrivateStoreType::None);
            self.broadcast_user_info(store);
        }
    }

    /// Java `TradeList.privateStoreSell(player, items)` on the buy list of `owner`.
    fn private_store_sell(&mut self, owner: ObjectId, player: ObjectId, items: &mut [RequestTrade]) -> bool {
        let mut list = self.buy_list_copy(owner);
        let result = self.private_store_sell_on(&mut list, player, items);
        if let Some(slot) = self.buy_list_mut(owner) {
            *slot = list;
        }
        result
    }

    fn private_store_sell_on(
        &mut self,
        list: &mut TradeList,
        player: ObjectId,
        items: &mut [RequestTrade],
    ) -> bool {
        let owner = list.owner;
        if list.locked || !self.players.contains_key(&owner) || !self.players.contains_key(&player) {
            return false;
        }

        let max_adena = self.settings.ground.max_adena;
        let templates = self.item_templates();
        let mut ok = false;
        // Prepare inventory update packet.
        let mut owner_iu: Vec<ItemUpdate> = Vec::new();
        let mut player_iu: Vec<ItemUpdate> = Vec::new();
        let mut total_price: i64 = 0;
        for item in items.iter_mut() {
            // Searching item in tradelist using itemId.
            let mut found = false;
            if let Some(listed) = list.items.iter().find(|t| t.item.item_id == item.item_id) {
                // Price should be the same.
                if listed.price == item.price {
                    // If requesting more than available - decrease count.
                    if listed.count < item.count {
                        item.count = listed.count;
                    }
                    found = item.count > 0;
                }
            }

            // Not found any item in the tradelist with same itemId and price. Maybe another player
            // already sold this item?
            if !found {
                continue;
            }

            // Check for overflow in the single item (`item.count >= 1` here).
            if max_adena
                .checked_div(item.count)
                .is_none_or(|limit| limit < item.price)
            {
                list.lock();
                break;
            }

            let new_total = total_price.wrapping_add(item.count.wrapping_mul(item.price));

            // Check for overflow of the total price.
            if max_adena < new_total || new_total < 0 {
                list.lock();
                break;
            }

            if self.adena_of(owner) < new_total {
                continue;
            }

            // Check if requested item is available for manipulation.
            let mut object_id = ObjectId(item.object_id);
            let mut old_item = self.check_item_manipulation(player, object_id, item.count);
            // Private store - buy use same objectId for buying several non-stackable items.
            if old_item.is_none() {
                // Searching other items using same itemId.
                let Some(other) = self.inventory_item_by_item_id(player, item.item_id).cloned() else {
                    continue;
                };
                object_id = other.object_id;
                old_item = self.check_item_manipulation(player, object_id, item.count);
            }
            let Some(old_item) = old_item else {
                continue;
            };

            if old_item.item_id != item.item_id {
                let text = format!("{} is cheating with sell items", self.player_text(player));
                self.store_illegal_action(player, &text);
                return false;
            }

            let tradeable = templates
                .as_ref()
                .and_then(|t| t.get(old_item.item_id))
                .is_some_and(|t| self.item_is_tradeable(&old_item, t));
            if !tradeable {
                continue;
            }

            // Proceed with item transfer.
            let Some(moved) = self.transfer_item(player, object_id, item.count, owner, player) else {
                continue;
            };

            list.remove_item(-1, item.item_id, item.count);
            ok = true;

            // Increase total price only after successful transaction.
            total_price = new_total;

            // Add changes to inventory update packets.
            if moved.old.count > 0 && !moved.moved_whole {
                player_iu.push(self.update_entry(&moved.old, InventoryUpdate::MODIFIED));
            } else {
                player_iu.push(self.update_entry(&moved.old, InventoryUpdate::REMOVED));
            }
            if moved.new.count > item.count {
                owner_iu.push(self.update_entry(&moved.new, InventoryUpdate::MODIFIED));
            } else {
                owner_iu.push(self.update_entry(&moved.new, InventoryUpdate::ADDED));
            }

            // Send messages about the transaction to both players.
            let stackable = self.data.item_stackable(moved.new.item_id);
            let name = item_name_param(templates.as_deref(), moved.new.item_id);
            let owner_name = self.store_name_of(owner);
            let player_name = self.store_name_of(player);
            let (to_owner, to_player) = if stackable {
                (
                    SystemMessage::new(SystemMessageId::YouHavePurchasedS3S2SFromC1)
                        .with(SystemMessageParam::Text(player_name))
                        .with(name.clone())
                        .with(SystemMessageParam::LongNumber(item.count)),
                    SystemMessage::new(SystemMessageId::C1PurchasedS3S2S)
                        .with(SystemMessageParam::Text(owner_name))
                        .with(name)
                        .with(SystemMessageParam::LongNumber(item.count)),
                )
            } else {
                (
                    SystemMessage::new(SystemMessageId::YouHavePurchasedS2FromC1)
                        .with(SystemMessageParam::Text(player_name))
                        .with(name.clone()),
                    SystemMessage::new(SystemMessageId::C1PurchasedS2)
                        .with(SystemMessageParam::Text(owner_name))
                        .with(name),
                )
            };
            self.send(owner, &to_owner);
            self.send(player, &to_player);
        }

        if total_price > 0 {
            // Transfer adena.
            if total_price > self.adena_of(owner) {
                // Should not happens, just a precaution.
                return false;
            }

            // `ownerIU.addItem(adenaItem)` reads the item after the change: the last change of an
            // adena item that is used up is REMOVED.
            if let Some(after) = self.reduce_adena_quietly(owner, total_price) {
                let change = if after.count == 0 {
                    InventoryUpdate::REMOVED
                } else {
                    InventoryUpdate::MODIFIED
                };
                owner_iu.push(self.update_entry(&after, change));
            }
            if let Some(added) = self.inventory_add_item_by_id(player, ADENA_ID, total_price) {
                player_iu.push(self.update_entry(&added.item, added.change));
            }
        }

        if ok {
            // Send inventory update packet.
            for entry in owner_iu {
                self.send_inventory_update(owner, entry);
            }
            for entry in player_iu {
                self.send_inventory_update(player, entry);
            }
        }

        ok
    }
}

#[cfg(test)]
mod tests {
    use super::hash_set_order;

    #[test]
    fn hash_set_order_follows_the_buckets_and_keeps_duplicates() {
        // 16 buckets: 17 -> 1, 33 -> 1 (insertion order inside a bucket), 2 -> 2, 16 -> 0.
        let ordered = hash_set_order(vec![2, 17, 33, 16, 17], |k| *k);
        assert_eq!(ordered, vec![16, 17, 33, 17, 2]);
    }

    #[test]
    fn hash_set_order_grows_the_table() {
        // 13 entries do not fit in 16 buckets (threshold 12): 32 buckets, so 16 and 1 part.
        let mut keys: Vec<i32> = (0..11).map(|n| n + 100).collect();
        keys.push(16);
        keys.push(1);
        let ordered = hash_set_order(keys, |k| *k);
        let one = ordered.iter().position(|k| *k == 1);
        let sixteen = ordered.iter().position(|k| *k == 16);
        assert!(one < sixteen);
    }
}
