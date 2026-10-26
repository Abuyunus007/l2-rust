//! Slot and weight limits of a player's inventory.
// Java: entity/itemcontainer/PlayerInventory.java (validateCapacity, validateWeight,
//       validateCapacityByItemId, getQuestSize)
// Java: entity/itemcontainer/ItemContainer.java (validateWeightByItemId)
//
// Equipping and destroying never check these (Java checks them when something is *added*: loot,
// buying, trading, mail). They are here because the inventory owns the numbers and the cards
// that add items (M4-DROP-01's pickup, shops in M5) call them.

use l2_util::ObjectId;

use super::{LOC_INVENTORY, LOC_PAPERDOLL};
use crate::world::World;

impl World {
    /// Java `PlayerInventory.getQuestSize()`: quest items in the inventory.
    pub fn quest_size(&self, player: ObjectId) -> i32 {
        let Some(p) = self.players.get(&player) else {
            return 0;
        };
        let count = p
            .data
            .items
            .iter()
            .filter(|i| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL)
            .filter(|i| self.data.item(i.item_id).is_some_and(|t| t.quest_item))
            .count();
        i32::try_from(count).unwrap_or(i32::MAX)
    }

    /// Java `PlayerInventory.validateCapacity(slots, questItem)`: `slots` more items fit.
    // NOTE(java-quirk): the condition is `((slots == 0) && !AUTO_LOOT_SLOT_LIMIT) || questItem ?
    // quest : normal`, so asking for 0 slots with `AutoLootSlotLimit = False` is checked against
    // the quest item limit.
    pub fn validate_capacity(&self, player: ObjectId, slots: i64, quest_item: bool) -> bool {
        let Some(view) = self.stats(player) else {
            return false;
        };
        if (slots == 0 && !self.settings.inventory.auto_loot_slot_limit) || quest_item {
            i64::from(self.quest_size(player)) + slots
                <= i64::from(self.settings.inventory_maximum_quest_items)
        } else {
            i64::from(view.non_quest_size()) + slots <= i64::from(view.inventory_limit())
        }
    }

    /// Java `PlayerInventory.validateCapacityByItemId(itemId)`: one slot, unless the item is
    /// stackable and already in the inventory.
    pub fn validate_capacity_by_item_id(&self, player: ObjectId, item_id: i32) -> bool {
        let in_inventory_stackable = self.data.item_stackable(item_id)
            && self.players.get(&player).is_some_and(|p| {
                p.data
                    .items
                    .iter()
                    .any(|i| i.item_id == item_id && (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL))
            });
        let slots = i64::from(!in_inventory_stackable);
        let quest = self.data.item(item_id).is_some_and(|t| t.quest_item);
        self.validate_capacity(player, slots, quest)
    }

    /// Java `PlayerInventory.validateWeight(weight)`: the load plus `weight` stays within the
    /// limit.
    pub fn validate_weight(&self, player: ObjectId, weight: i64) -> bool {
        // Disable weight check for GMs in diet mode (their level must allow transactions).
        if let Some(p) = self.players.get(&player)
            && p.gm.diet
            && self.is_gm(player)
            && self.data.access_level_allows_transaction(p.data.access_level)
        {
            return true;
        }
        let Some(view) = self.stats(player) else {
            return false;
        };
        i64::from(view.current_load()) + weight <= i64::from(view.max_load())
    }

    /// Java `ItemContainer.validateWeightByItemId(itemId, count)`: true for an unknown item.
    pub fn validate_weight_by_item_id(&self, player: ObjectId, item_id: i32, count: i64) -> bool {
        match self.data.item(item_id) {
            None => true,
            Some(t) => self.validate_weight(player, i64::from(t.weight).saturating_mul(count)),
        }
    }
}
