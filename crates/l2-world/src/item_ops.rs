//! Taking items away from a player (a gatekeeper's fee) and the `InventoryUpdate` that tells the
//! client, with Java's delay.
// Java: entity/actor/Player.java (destroyItemByItemId, reduceAdena, sendInventoryUpdate)
// Java: entity/itemcontainer/{Inventory,ItemContainer}.java (destroyItem, destroyItemByItemId,
//       getInventoryItemCount)
// Java: managers/ItemManager.java (destroyItem)
//
// The items live in `PlayerData::items`; the persistence task stores the difference to what it
// last wrote (a changed count is an `UPDATE`, a spent item a `DELETE`), so changing the list here
// is all the database needs. D-39: Java writes the same rows.
//
// # What is left out
// - `ItemProcessType` logging (`LogItems`), the item audit of GMs: the item log is not ported.
// - Releasing the object id of a destroyed item: ids of items loaded from the database belong to
//   Java's id manager, and the Rust factory only knows its own range.

use l2_data::item::ADENA_ID;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{
    InventoryUpdate, ItemUpdate, StatusUpdate, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use crate::data::ItemStatic;
use crate::enter::{current_load, item_info};
use crate::player::ItemData;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// Java `Player._inventoryUpdate` and its send task.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingInventoryUpdate {
    entries: Vec<ItemUpdate>,
    timer: Option<TimerId>,
}

/// `Player.sendInventoryUpdate` waits 100 ms for more changes.
const INVENTORY_UPDATE_DELAY_MS: u64 = 100;

const LOC_INVENTORY: &str = "INVENTORY";
const LOC_PAPERDOLL: &str = "PAPERDOLL";

impl World {
    /// Java `Inventory.getAdena()`.
    pub fn adena_of(&self, player: ObjectId) -> i64 {
        self.players.get(&player).map_or(0, |p| {
            p.data
                .items
                .iter()
                .find(|i| i.item_id == ADENA_ID && i.loc == LOC_INVENTORY)
                .map_or(0, |i| i.count)
        })
    }

    fn item_static_of(&self, item: &ItemData) -> ItemStatic {
        self.data
            .item(item.item_id)
            .unwrap_or_else(|| ItemStatic::unknown(item.item_id))
    }

    /// `new ItemInfo(item, change)` as the update packet carries it.
    fn update_entry(&self, item: &ItemData, change: i16) -> ItemUpdate {
        ItemUpdate {
            change,
            item: item_info(item, &self.item_static_of(item), unix_ms_now()),
        }
    }

    /// Java `Player.destroyItemByItemId(process, itemId, count, reference, sendMessage)`. A
    /// negative `count` destroys everything.
    pub fn destroy_item_by_item_id(
        &mut self,
        player: ObjectId,
        item_id: i32,
        count: i64,
        send_message: bool,
    ) -> bool {
        if item_id == ADENA_ID {
            return self.reduce_adena(player, count, send_message);
        }
        let Some(items) = self.players.get(&player).map(|p| &p.data.items) else {
            return false;
        };
        let is_inventory = |i: &&ItemData| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL;
        let Some(first) = items
            .iter()
            .filter(is_inventory)
            .find(|i| i.item_id == item_id)
            .cloned()
        else {
            if send_message {
                self.send(player, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            }
            return false;
        };
        let stackable = self.data.item_stackable(item_id);
        let item_count = if stackable {
            first.count
        } else {
            items
                .iter()
                .filter(is_inventory)
                .filter(|i| i.item_id == item_id)
                .map(|i| i.count)
                .sum()
        };
        let remove_count = if count < 0 { item_count } else { count };
        if remove_count <= 0 || item_count < remove_count {
            if send_message {
                self.send(player, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            }
            return false;
        }
        // `Inventory.destroyItem(item, count)` works on the first item only.
        let Some(change) = self.take_from_item(player, first.object_id, remove_count) else {
            if send_message {
                self.send(player, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            }
            return false;
        };

        // Send inventory update packet.
        self.send_inventory_update(player, change);

        // Update current load as well.
        let load = self
            .players
            .get(&player)
            .map_or(0, |p| current_load(&p.data, &*self.data));
        let mut update = StatusUpdate::new(player.0);
        update.add_attribute(status::CUR_LOAD, load);
        self.send(player, &update);

        // Sends message to client if requested.
        if send_message {
            let message = if remove_count > 1 {
                SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                    .with(SystemMessageParam::ItemName(item_id))
                    .with(SystemMessageParam::LongNumber(remove_count))
            } else {
                SystemMessage::new(SystemMessageId::S1HasDisappeared)
                    .with(SystemMessageParam::ItemName(item_id))
            };
            self.send(player, &message);
        }
        true
    }

    /// Java `Player.reduceAdena(process, count, reference, sendMessage)`.
    pub fn reduce_adena(&mut self, player: ObjectId, count: i64, send_message: bool) -> bool {
        if count > self.adena_of(player) {
            if send_message {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                );
            }
            return false;
        }
        if count > 0 {
            let object_id = self.players.get(&player).and_then(|p| {
                p.data
                    .items
                    .iter()
                    .find(|i| i.item_id == ADENA_ID && i.loc == LOC_INVENTORY)
                    .map(|i| i.object_id)
            });
            let Some(change) = object_id.and_then(|object| self.take_from_item(player, object, count)) else {
                return false;
            };
            // Send update packet.
            self.send_inventory_update(player, change);
            if send_message {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::S1AdenaDisappeared)
                        .with(SystemMessageParam::LongNumber(count)),
                );
            }
        }
        true
    }

    /// Java `ItemContainer.destroyItem(item, count)`: `count` less of the item, or the item gone
    /// when it is all of it. `None` when the item has less than `count` (Java returns null).
    /// Returns the update entry for the client.
    fn take_from_item(&mut self, player: ObjectId, object_id: ObjectId, count: i64) -> Option<ItemUpdate> {
        let p = self.players.get_mut(&player)?;
        let index = p.data.items.iter().position(|i| i.object_id == object_id)?;
        let have = p.data.items[index].count;
        if have > count {
            p.data.items[index].count -= count;
            let item = p.data.items[index].clone();
            Some(self.update_entry(&item, InventoryUpdate::MODIFIED))
        } else if have == count {
            let mut item = p.data.items.remove(index);
            // `ItemManager.destroyItem`: the destroyed item has no count left.
            item.count = 0;
            Some(self.update_entry(&item, InventoryUpdate::REMOVED))
        } else {
            None
        }
    }

    /// Java `Player.sendInventoryUpdate(iu)`: the entries join the pending update, which goes out
    /// 100 ms after the last call.
    pub(crate) fn send_inventory_update(&mut self, player: ObjectId, entry: ItemUpdate) {
        let due = self.tick + TimerWheel::ms_to_ticks(INVENTORY_UPDATE_DELAY_MS);
        let Some(p) = self.players.get_mut(&player) else {
            return;
        };
        if let Some(old) = p.pending_inventory.timer.take() {
            self.timers.cancel(old);
        }
        let Some(p) = self.players.get_mut(&player) else {
            return;
        };
        // `Map.putAll`: a later entry for the same object replaces the earlier one.
        match p
            .pending_inventory
            .entries
            .iter_mut()
            .find(|e| e.item.object_id == entry.item.object_id)
        {
            Some(existing) => *existing = entry,
            None => p.pending_inventory.entries.push(entry),
        }
        let timer = self.timers.schedule_at(due, TimerKind::InventoryUpdate(player));
        if let Some(p) = self.players.get_mut(&player) {
            p.pending_inventory.timer = Some(timer);
        }
    }

    /// The send task of `sendInventoryUpdate`.
    pub(crate) fn on_inventory_update_timer(&mut self, player: ObjectId) {
        let Some(p) = self.players.get_mut(&player) else {
            return;
        };
        p.pending_inventory.timer = None;
        let updates = std::mem::take(&mut p.pending_inventory.entries);
        self.send(player, &InventoryUpdate { updates });
    }
}
