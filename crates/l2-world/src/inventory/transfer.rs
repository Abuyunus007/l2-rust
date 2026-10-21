//! Moving items from one player to another: `ItemContainer.transferItem` (trade, private stores)
//! and the manipulation checks of `Player` (`checkItemManipulation`, `validateItemManipulation`,
//! `Item.isTradeable`).
// Java: entity/itemcontainer/{ItemContainer,PlayerInventory}.java (transferItem, addItem)
// Java: entity/actor/Player.java (checkItemManipulation, validateItemManipulation)
// Java: entity/item/instance/Item.java (isTradeable, updateDatabase)
// Java: managers/ItemManager.java (createItem, destroyItem)
//
// # Database (D-39: the Java GS shares the rows)
// Java moves an item by changing `owner_id`/`loc` of its row and calling `updateDatabase(true)`,
// i.e. one `UPDATE items SET owner_id=?,count=?,loc=?,loc_data=?,...`. The persistence task keeps
// the rows of each character separately (`PersistCommand::ItemChanges` has one owner), so the same
// end state is reached with two commands in this order: the row leaves the old owner (a `DELETE`,
// because that owner's baseline knows it) and the new owner gets the whole row (an `INSERT` with
// the new `owner_id`). The commands go through one queue, so nobody sees the row missing, and a
// later full store of either character cannot delete or duplicate it. A partial move is an
// `UPDATE` of the source row and an `UPDATE` (stack merge) or `INSERT` (new stack, new id from the
// Rust range) of the target row, as in Java.
// NOTE: `item_variables` rows of the item are not modelled by the Rust server (they are not read),
// so the `DELETE` of the old row drops them; Java's `UPDATE` keeps them. Only matters for items
// that carry variables (appearance stones, ...).

use l2_data::item::ItemTemplate;
use l2_protocol_game::server::InventoryUpdate;
use l2_util::ObjectId;

use crate::player::ItemData;
use crate::world::World;

/// What `World::transfer_item` did (Java returns the new item; the packets of `TradeList` also
/// read the old one after the move).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Transferred {
    /// The source item after the move: with the count it has left, with count 0 when it was
    /// destroyed, or the item itself (full count, now in the other inventory) when it moved whole.
    pub old: ItemData,
    /// The item in the target inventory (the moved item, the stack it joined or the new stack).
    pub new: ItemData,
    /// Java `oldItem == newItem`: the item object itself changed owner.
    pub moved_whole: bool,
}

impl World {
    /// Java `Item.isTradeable()`: the template, and for an augmented item `AltAllowAugmentTrade`.
    // The transmog check (`isEquipable() && getTransmogId() > 0`) needs the appearance system
    // (item_variables), which does not exist; no item has a transmog id.
    pub(crate) fn item_is_tradeable(&self, item: &ItemData, template: &ItemTemplate) -> bool {
        if !template.tradeable {
            return false;
        }
        if item.is_augmented() {
            return self.settings.inventory.alt_allow_augment_trade;
        }
        true
    }

    /// Java `Player.validateItemManipulation(objectId, process)`: the item is in the inventory and
    /// is not the enchant scroll in use.
    // The control item of a summoned pet, the saddle of a mount and a cursed weapon refuse here as
    // well; none of them exists yet (summons are a later M6 card, mounts and cursed weapons are
    // not ported).
    pub(crate) fn validate_item_manipulation(&self, player: ObjectId, object_id: ObjectId) -> bool {
        self.inventory_item(player, object_id).is_some()
            && self.active_enchant_item(player) != Some(object_id)
    }

    /// Java `Player.checkItemManipulation(objectId, count, action)`: the item if the player owns
    /// it and may move `count` of it.
    pub(crate) fn check_item_manipulation(
        &self,
        player: ObjectId,
        object_id: ObjectId,
        count: i64,
    ) -> Option<ItemData> {
        let item = self.inventory_item(player, object_id)?;
        let stackable = self.data.item_stackable(item.item_id);
        if count < 0 || (count > 1 && !stackable) {
            return None;
        }
        if count > item.count {
            return None;
        }
        // The enchant scroll in use.
        if self.active_enchant_item(player) == Some(object_id) {
            return None;
        }
        // We cannot put a weapon with augmentation in a warehouse (or trade it) while casting
        // (possible exploit).
        if item.is_augmented() && (self.is_casting_now(player) || self.is_casting_simultaneously_now(player))
        {
            return None;
        }
        Some(item.clone())
    }

    /// Java `ItemContainer.transferItem(process, objectId, count, target, actor, reference)` between
    /// two player inventories. `actor` is the player Java passes (the one who gives the item; the
    /// private stores pass the seller or the buyer).
    ///
    /// Returns `None` when the item is not in `from`'s inventory.
    pub(crate) fn transfer_item(
        &mut self,
        from: ObjectId,
        object_id: ObjectId,
        count: i64,
        to: ObjectId,
        actor: ObjectId,
    ) -> Option<Transferred> {
        let source = self.inventory_item(from, object_id)?.clone();
        let stackable = self.data.item_stackable(source.item_id);
        let mut target = if stackable {
            self.inventory_item_by_item_id(to, source.item_id).cloned()
        } else {
            None
        };

        // Check if requested quantity is available.
        let count = count.min(source.count);

        let old;
        let new;
        let moved_whole;
        // If possible, move entire item object.
        if source.count == count && target.is_none() && !stackable {
            let moved = self.remove_inventory_item(from, object_id)?;
            new = self.inventory_add_item(to, moved.clone())?.item;
            old = moved;
            moved_whole = true;
        } else {
            moved_whole = false;
            if source.count > count {
                // If possible, only update counts.
                old = self.change_item_count(from, object_id, -count)?;
            } else {
                // Otherwise destroy old item (`ItemManager.destroyItem` deletes the row).
                old = self.destroy_inventory_item(from, object_id, source.count)?;
            }
            new = if let Some(existing) = target.take() {
                // If possible, only update counts.
                self.change_item_count(to, existing.object_id, count)?
            } else {
                // Otherwise add new item.
                self.create_transferred_item(to, source.item_id, count, actor)?
            };
        }

        // Updates database: `sourceitem.updateDatabase(true)`, then the target item if it is another
        // object.
        if moved_whole {
            // The same row, a new owner (see the module notes).
            self.remove_item_from_db(from, object_id);
            self.update_item_in_db(to, new.object_id);
        } else {
            if old.count > 0 {
                self.update_item_in_db(from, object_id);
            }
            self.update_item_in_db(to, new.object_id);
        }

        // An augmented item is never worn while it is traded (`isAvailable` / unequipped by the
        // move), so Java's `getAugmentation().removeBonus(actor)` has nothing to remove.
        self.refresh_weight(from);
        self.refresh_weight(to);
        Some(Transferred {
            old,
            new,
            moved_whole,
        })
    }

    /// `ItemContainer.addItem(process, itemId, count, actor, reference)` for an item that is not in
    /// the target yet (a new object id from the Rust range, D-39), with `PlayerInventory`'s
    /// additions.
    // NOTE(java-quirk): `PlayerInventory.addItem(process, itemId, count, actor, reference)` sends
    // the `InventoryUpdate` and the load to `actor`, not to the inventory's owner. A trade passes
    // the player who gives the item, so the giver is told about an item that is in the other
    // player's inventory. The full `ItemList` that the exchange sends afterwards replaces it.
    pub(crate) fn create_transferred_item(
        &mut self,
        to: ObjectId,
        item_id: i32,
        count: i64,
        actor: ObjectId,
    ) -> Option<ItemData> {
        let templates = self.item_templates()?;
        let Some(template) = templates.get(item_id) else {
            tracing::warn!("Invalid ItemId ({item_id}) requested by {actor}");
            return None;
        };
        let id = self.allocate_item_id()?;
        let mut data = self.new_item_data(template, id, if template.stackable { count } else { 1 });
        data.loc = super::LOC_INVENTORY.to_string();
        data.loc_data = 0;
        data.enchant_level = template.default_enchant_level;
        self.players.get_mut(&to)?.data.items.push(data.clone());
        if template.has_skills() {
            self.send_skill_list(to);
        }
        self.refresh_weight(to);

        // Send inventory update packet (to the actor, see above).
        let change = if template.stackable && data.count > count {
            InventoryUpdate::MODIFIED
        } else {
            InventoryUpdate::ADDED
        };
        let entry = self.update_entry(&data, change);
        self.send_inventory_update(actor, entry);
        // Update current load as well.
        self.send_cur_load(actor);
        Some(data)
    }
}
