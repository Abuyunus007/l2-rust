//! Putting items into an inventory: `Inventory.addItem` (stack merge, the `ADDED` / `MODIFIED`
//! flag), `Player.addItem`, `Player.addAdena`, auto loot, and the drop of what does not fit
//! (`Player.dropItem`).
// Java: entity/itemcontainer/{ItemContainer,PlayerInventory}.java (addItem, addAdena,
//       validateCapacity(Item), getInventoryItemCount)
// Java: entity/actor/Player.java (addItem, addAdena, dropItem, doAutoLoot)
// Java: entity/item/instance/Item.java (changeCount, setOwnerId, setItemLocation)
// Java: managers/ItemManager.java (createItem, destroyItem)
//
// # Rows and the database
// An added item is a row of `PlayerData::items` with `loc` `INVENTORY`. Java does not write it at
// once (`setOwnerId` and `addItem` only mark it dirty); the next store compares the item list and
// inserts it, so nothing is sent to the outbox here. A stack that grows is the same: `changeCount`
// writes nothing until the next store.
//
// # Left out on purpose
// - `ItemProcessType` logging (`LogItems`), the GM audit, `ON_PLAYER_ITEM_ADD` (D-05).
// - Cursed weapons, territory wards and combat flags that activate when picked up (M5).
// - Herbs: an `ExImmediateEffect` item casts its skills instead of entering the inventory.
// - Parties: `distributeItem` for loot and pickups (M5).

use l2_data::item::{ADENA_ID, ItemTemplate};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{InventoryUpdate, StatusUpdate, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{LOC_INVENTORY, LOC_PAPERDOLL, is_equipable, item_name_param};
use crate::effects::SkillKey;
use crate::enter::current_load;
use crate::ground_item::GroundItem;
use crate::player::ItemData;
use crate::world::World;

/// An item that was added: its state now and what the client is told (`Item.getLastChange()`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AddedItem {
    pub item: ItemData,
    /// `InventoryUpdate::ADDED` (a new row) or `InventoryUpdate::MODIFIED` (a stack that grew).
    pub change: i16,
}

fn in_inventory(item: &ItemData) -> bool {
    item.loc == LOC_INVENTORY || item.loc == LOC_PAPERDOLL
}

impl World {
    /// Java `ItemContainer.getItemByItemId(itemId)` on a player's inventory: the first row.
    pub(crate) fn inventory_item_by_item_id(&self, player: ObjectId, item_id: i32) -> Option<&ItemData> {
        self.players
            .get(&player)?
            .data
            .items
            .iter()
            .find(|i| i.item_id == item_id && in_inventory(i))
    }

    /// Java `ItemContainer.getInventoryItemCount(itemId, -1)`: the count of a stackable item, else
    /// how many there are.
    pub(crate) fn inventory_item_count(&self, player: ObjectId, item_id: i32) -> i64 {
        let Some(p) = self.players.get(&player) else {
            return 0;
        };
        let stackable = self.data.item_stackable(item_id);
        let mut count = 0;
        for item in p
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id && in_inventory(i))
        {
            if stackable {
                return item.count;
            }
            count += 1;
        }
        count
    }

    /// Java `PlayerInventory.validateCapacity(Item item)`: a stackable item that is already there
    /// (and is not a herb) needs no slot.
    pub(crate) fn validate_capacity_for_item(&self, player: ObjectId, template: &ItemTemplate) -> bool {
        let slots = i64::from(
            !template.stackable
                || (self.inventory_item_count(player, template.id) <= 0 && !template.ex_immediate_effect),
        );
        self.validate_capacity(player, slots, template.quest_item)
    }

    /// Java `Item.changeCount(process, count, ...)`: adena stops at `MaxAdena`, nothing goes
    /// below 0.
    pub(super) fn change_item_count(
        &mut self,
        player: ObjectId,
        object_id: ObjectId,
        count: i64,
    ) -> Option<ItemData> {
        let max_adena = self.settings.ground.max_adena;
        let p = self.players.get_mut(&player)?;
        let item = p.data.items.iter_mut().find(|i| i.object_id == object_id)?;
        if count == 0 {
            return Some(item.clone());
        }
        let max = if item.item_id == ADENA_ID {
            max_adena
        } else {
            i64::MAX
        };
        if count > 0 && item.count > max.wrapping_sub(count) {
            item.count = max;
        } else {
            item.count = item.count.wrapping_add(count);
        }
        if item.count < 0 {
            item.count = 0;
        }
        Some(item.clone())
    }

    /// Java `Inventory.addItem(process, item, actor, reference)` for an item that exists already
    /// (a pickup): a stack joins the one in the inventory and the new item is destroyed, else it
    /// becomes a row of the inventory.
    pub(crate) fn inventory_add_item(&mut self, player: ObjectId, mut item: ItemData) -> Option<AddedItem> {
        let old = self.inventory_item_by_item_id(player, item.item_id).cloned();
        let result = match old {
            // If stackable item is found in inventory just add to current quantity.
            Some(old) if self.data.item_stackable(old.item_id) => {
                let merged = self.change_item_count(player, old.object_id, item.count)?;
                // And destroys the item (`ItemManager.destroyItem` releases its id).
                self.release_item_id(item.object_id);
                AddedItem {
                    item: merged,
                    change: InventoryUpdate::MODIFIED,
                }
            }
            // If item hasn't be found in inventory, create new one.
            _ => {
                item.loc = LOC_INVENTORY.to_string();
                item.loc_data = 0;
                self.players.get_mut(&player)?.data.items.push(item.clone());
                // `Inventory.addItem`: an item with skills refreshes the skill window.
                let has_skills = self
                    .item_templates()
                    .as_deref()
                    .and_then(|t| t.get(item.item_id))
                    .is_some_and(|t| t.has_skills());
                if has_skills {
                    self.send_skill_list(player);
                }
                AddedItem {
                    item,
                    change: InventoryUpdate::ADDED,
                }
            }
        };
        self.refresh_weight(player);
        Some(result)
    }

    /// Java `Inventory.addItem(process, itemId, count, actor, reference)` with
    /// `PlayerInventory`'s additions: the items are created here (new ids from the factory), the
    /// client gets the `InventoryUpdate` and the load.
    pub(crate) fn inventory_add_item_by_id(
        &mut self,
        player: ObjectId,
        item_id: i32,
        count: i64,
    ) -> Option<AddedItem> {
        let templates = self.item_templates()?;
        let multiple = self.settings.ground.multiple_item_drop;
        let old = self.inventory_item_by_item_id(player, item_id).cloned();
        let mut extras = Vec::new();
        let mut last: Option<AddedItem> = None;
        match old {
            // If stackable item is found in inventory just add to current quantity.
            Some(old) if self.data.item_stackable(old.item_id) => {
                let merged = self.change_item_count(player, old.object_id, count)?;
                last = Some(AddedItem {
                    item: merged,
                    change: InventoryUpdate::MODIFIED,
                });
            }
            // If item hasn't be found in inventory, create new one.
            _ => {
                let mut i: i64 = 0;
                while i < count {
                    let Some(template) = templates.get(item_id) else {
                        warn!("Invalid ItemId ({item_id}) requested by {player}");
                        return None;
                    };
                    let id = self.allocate_item_id()?;
                    let mut data =
                        self.new_item_data(template, id, if template.stackable { count } else { 1 });
                    data.loc = LOC_INVENTORY.to_string();
                    data.loc_data = 0;
                    data.enchant_level = template.default_enchant_level;
                    self.players.get_mut(&player)?.data.items.push(data.clone());
                    if template.has_skills() {
                        self.send_skill_list(player);
                    }
                    // Add additional items to InventoryUpdate.
                    if count > 1 && i < count - 1 {
                        extras.push(self.update_entry(&data, InventoryUpdate::ADDED));
                    }
                    last = Some(AddedItem {
                        item: data,
                        change: InventoryUpdate::ADDED,
                    });
                    i += 1;
                    // If stackable, end loop as entire count is included in 1 instance of item.
                    if template.stackable || !multiple {
                        break;
                    }
                }
                // If additional items where created send InventoryUpdate.
                if count > 1 && !self.data.item_stackable(item_id) {
                    for entry in extras {
                        self.send_inventory_update(player, entry);
                    }
                }
            }
        }
        self.refresh_weight(player);
        let added = last?;

        // `PlayerInventory.addItem`: send inventory update packet; update current load as well.
        let stackable = self.data.item_stackable(added.item.item_id);
        let change = if stackable && added.item.count > count {
            InventoryUpdate::MODIFIED
        } else {
            InventoryUpdate::ADDED
        };
        let entry = self.update_entry(&added.item, change);
        self.send_inventory_update(player, entry);
        self.send_cur_load(player);
        Some(added)
    }

    /// The `InventoryUpdate` and `CUR_LOAD` that follow `Player.addItem(Item)`.
    fn send_added_item(&mut self, player: ObjectId, added: &AddedItem) {
        let entry = self.update_entry(&added.item, added.change);
        self.send_inventory_update(player, entry);
        let Some(p) = self.players.get(&player) else {
            return;
        };
        let mut update = StatusUpdate::new(player.0);
        update.add_attribute(status::CUR_LOAD, current_load(&p.data, &*self.data));
        self.send(player, &update);
    }

    /// Java `Player.addItem(process, item, reference, sendMessage)`: adds an item that exists
    /// (a pickup) and tells the client. Returns what happened to it.
    pub(crate) fn add_item_object(
        &mut self,
        player: ObjectId,
        item: ItemData,
        send_message: bool,
    ) -> Option<AddedItem> {
        if item.count <= 0 {
            return None;
        }
        let templates = self.item_templates();
        let template = templates.as_deref().and_then(|t| t.get(item.item_id)).cloned();
        // Sends message to client if requested.
        if send_message {
            let name = item_name_param(templates.as_deref(), item.item_id);
            let message = if item.count > 1 {
                SystemMessage::new(SystemMessageId::YouHaveObtainedS2S1)
                    .with(name)
                    .with(SystemMessageParam::LongNumber(item.count))
            } else if item.enchant_level > 0 {
                SystemMessage::new(SystemMessageId::YouHaveObtainedAS1S2)
                    .with(SystemMessageParam::IntNumber(item.enchant_level))
                    .with(name)
            } else {
                SystemMessage::new(SystemMessageId::YouHaveObtainedS1).with(name)
            };
            self.send(player, &message);
        }

        // Add the item to inventory.
        let added = self.inventory_add_item(player, item)?;

        // Send inventory update packet; update current load as well.
        self.send_added_item(player, &added);

        // If over capacity, drop the item.
        let (dropable, quest_item, stackable) = template
            .as_ref()
            .map_or((true, false, false), |t| (t.dropable, t.quest_item, t.stackable));
        if !self.is_gm(player)
            && !self.validate_capacity(player, 0, quest_item)
            && dropable
            && (!stackable || added.change != InventoryUpdate::MODIFIED)
        {
            self.drop_overflow_item(player, added.item.object_id, true);
        }
        // Cursed weapons, combat flags and territory wards (M5).
        Some(added)
    }

    /// Java `Player.addItem(process, itemId, count, reference, sendMessage)` (no enchant level):
    /// creates the items. `None` for an unknown item, a herb (it casts its skills at once) or when
    /// no id could be allocated.
    pub fn add_item(
        &mut self,
        player: ObjectId,
        item_id: i32,
        count: i64,
        send_message: bool,
    ) -> Option<ObjectId> {
        if count <= 0 {
            return None;
        }
        let templates = self.item_templates()?;
        let Some(template) = templates.get(item_id) else {
            tracing::error!("Item doesn't exist so cannot be added. Item ID: {item_id}");
            return None;
        };
        // Sends message to client if requested (not for a herb while a spell is being cast).
        if send_message && (!self.is_casting_now(player) || !template.ex_immediate_effect) {
            let name = item_name_param(Some(&templates), item_id);
            let message = if count > 1 {
                SystemMessage::new(SystemMessageId::YouHaveObtainedS2S1)
                    .with(name)
                    .with(SystemMessageParam::LongNumber(count))
            } else {
                SystemMessage::new(SystemMessageId::YouHaveObtainedS1).with(name)
            };
            self.send(player, &message);
        }

        // Auto-use herbs.
        if template.ex_immediate_effect && template.is_etc_item() {
            let keys: Vec<SkillKey> = template
                .skills
                .item_skills
                .iter()
                .flatten()
                .map(|s| SkillKey::new(s.id, s.level))
                .collect();
            for key in keys {
                self.do_simultaneous_cast(player, key);
            }
            self.broadcast_info(player);
            return None;
        }

        // Add the item to inventory.
        let created = self.inventory_add_item_by_id(player, item_id, count)?;

        // If over capacity, drop the item.
        if !self.is_gm(player)
            && !self.validate_capacity(player, 0, template.quest_item)
            && template.dropable
            && (!template.stackable || created.change != InventoryUpdate::MODIFIED)
        {
            self.drop_overflow_item(player, created.item.object_id, false);
        }
        Some(created.item.object_id)
    }

    /// Java `Player.addAdena(process, count, reference, sendMessage)`: adena up to `MaxAdena`.
    pub fn add_adena(&mut self, player: ObjectId, count: i64, send_message: bool) {
        let current = self.adena_of(player);
        let limit_remaining = self.settings.ground.max_adena.wrapping_sub(current);
        let amount_to_add = count;
        let count = if count > limit_remaining {
            limit_remaining
        } else {
            count
        };

        if send_message {
            let message = if count == 0 && amount_to_add > 0 {
                // Failed adding adena.
                SystemMessage::new(SystemMessageId::YouHaveFailedToEarnS1Adena)
                    .with(SystemMessageParam::LongNumber(amount_to_add))
            } else {
                // Inform player how much adena they received.
                SystemMessage::new(SystemMessageId::YouHaveEarnedS1Adena)
                    .with(SystemMessageParam::LongNumber(count))
            };
            self.send(player, &message);
        }

        if count > 0 {
            // `PlayerInventory.addAdena`.
            // NOTE(java-quirk): that already sends an `InventoryUpdate` and the load; the code below
            // sends the adena item again (or the whole item list for the first adena).
            self.inventory_add_item_by_id(player, ADENA_ID, count);
            // Send update packet.
            if count == self.adena_of(player) {
                self.send_item_list(player, false);
            } else if let Some(adena) = self.inventory_item_by_item_id(player, ADENA_ID).cloned() {
                let entry = self.update_entry(&adena, InventoryUpdate::MODIFIED);
                self.send_inventory_update(player, entry);
            }
        }

        // If the player has reached the adena limit, inform them.
        if self.adena_of(player) == self.settings.ground.max_adena && send_message {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveExceededYourOutOfPocketAdenaLimit),
            );
        }
    }

    /// Java `Player.doAutoLoot(target, itemId, itemCount)`: the loot goes straight into the killer's
    /// inventory, or is shared by the party (`party/loot.rs`).
    pub(crate) fn do_auto_loot(&mut self, player: ObjectId, npc: ObjectId, item_id: i32, count: i64) {
        self.do_auto_loot_for(player, npc, item_id, count);
    }

    // ------------------------------------------------------------------ dropping

    /// `Player.dropItem(DROP, item, null, true[, true])` for an item that does not fit: it leaves
    /// the inventory and lands within 25 of the player. The row was never written (the item was
    /// just added), so there is nothing to delete.
    fn drop_overflow_item(&mut self, player: ObjectId, object_id: ObjectId, protect_item: bool) {
        self.player_drop_item_object(player, object_id, protect_item, false);
    }

    /// Java `Player.dropItem(process, item, reference, sendMessage = true, protectItem)`: the item
    /// leaves the inventory (`Inventory.dropItem(item)`: owner 0, location `VOID`, the row deleted
    /// when `in_db`) and lands within 25 of the player. Returns `false` when the item is not in
    /// the inventory (the player hears `INCORRECT_ITEM_COUNT_2`).
    pub(crate) fn player_drop_item_object(
        &mut self,
        player: ObjectId,
        object_id: ObjectId,
        protect_item: bool,
        in_db: bool,
    ) -> bool {
        let Some(mut dropped) = self.remove_inventory_item(player, object_id) else {
            self.send(player, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            return false;
        };
        if in_db {
            self.remove_item_from_db(player, object_id);
        }
        self.refresh_weight(player);
        dropped.loc = crate::ground_item::LOC_VOID.to_string();
        dropped.loc_data = 0;
        let Some((px, py, pz)) = self.position_of(player) else {
            return false;
        };
        let x = (px + self.rng().below(50)) - 25;
        let y = (py + self.rng().below(50)) - 25;
        let item_id = dropped.item_id;
        let entry = self.update_entry(&dropped, InventoryUpdate::REMOVED);
        self.player_drop_to_ground(player, dropped, x, y, pz + 20, protect_item);

        // Send inventory update packet; update current load as well.
        self.send_inventory_update(player, entry);
        self.send_cur_load(player);

        // Sends message to client if requested.
        let templates = self.item_templates();
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveDroppedS1)
                .with(item_name_param(templates.as_deref(), item_id)),
        );
        true
    }

    /// The part `Player.dropItem` has in common for both of its versions, after the item left the
    /// inventory: `dropMe`, the auto destroy list, the `_protected` flag and the retail drop
    /// protection.
    pub(crate) fn player_drop_to_ground(
        &mut self,
        player: ObjectId,
        dropped: ItemData,
        x: i32,
        y: i32,
        z: i32,
        protect_item: bool,
    ) -> Option<ObjectId> {
        let id = dropped.object_id;
        let item_id = dropped.item_id;
        let equipable = self
            .item_templates()
            .as_deref()
            .and_then(|t| t.get(item_id))
            .is_some_and(is_equipable);
        let settings = self.settings.ground.clone();
        let mut item = GroundItem::new(dropped);
        // Retail drop protection.
        if protect_item {
            self.protect_item(&mut item, player);
        }
        if !self.drop_me(item, Some(player), x, y, z) {
            self.release_item_id(id);
            return None;
        }
        if settings.autodestroy_item_after > 0
            && settings.destroy_dropped_player_item
            && !settings.is_protected_item(item_id)
            && ((equipable && settings.destroy_equipable_player_item) || !equipable)
        {
            self.add_to_auto_destroy(id);
        }
        // Protection against auto destroy dropped item.
        if let Some(item) = self.ground_item_mut(id) {
            item.protected = if settings.destroy_dropped_player_item {
                equipable && !settings.destroy_equipable_player_item
            } else {
                true
            };
        }
        Some(id)
    }
}
