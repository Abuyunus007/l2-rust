//! Moving an item between a player's inventory and a container: `ItemContainer.transferItem`
//! with the warehouse, clan warehouse and freight as one side.
// Java: entity/itemcontainer/ItemContainer.java (transferItem, addItem, destroyItem)
// Java: entity/itemcontainer/PlayerInventory.java (addItem, transferItem)
// Java: entity/item/instance/Item.java (setOwnerId, setItemLocation, updateDatabase, changeCount)
// Java: managers/ItemManager.java (createItem, destroyItem)
//
// The move between two players' inventories is `World::transfer_item` (trade); this is the same
// algorithm with a container on either side, and it queues its rows as `ItemRowOp`s (see the
// module notes in `mod.rs`). The packet handlers collect the ops of one packet and send them as
// one `PersistCommand::ItemRows`.

use l2_data::item::ADENA_ID;
use l2_util::ObjectId;
use tracing::warn;

use super::container::ContainerRef;
use crate::outbox::{ItemRowOp, PersistCommand};
use crate::player::ItemData;
use crate::world::World;

/// One side of a move: the inventory of a player, or a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Inventory(ObjectId),
    Container(ContainerRef),
}

/// What [`World::move_item`] did (Java returns the item in the target; the handlers read the
/// source item after the move as well).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Moved {
    /// The source item after the move: with the count it has left, with count 0 when it was
    /// destroyed, or the item itself (full count, now in the target) when it moved whole.
    pub old: ItemData,
    /// The item in the target (the moved item, the stack it joined or the new stack).
    pub new: ItemData,
    /// Java `oldItem == newItem`: the item object itself changed container.
    pub moved_whole: bool,
}

impl World {
    /// `getOwnerId()` of a place: the character, or the container's owner column.
    fn place_owner(place: Place) -> i32 {
        match place {
            Place::Inventory(player) => player.0,
            Place::Container(container) => container.owner_id(),
        }
    }

    /// Java `getItemByObjectId(objectId)`.
    pub(crate) fn place_item(&self, place: Place, object_id: ObjectId) -> Option<ItemData> {
        match place {
            Place::Inventory(player) => self.inventory_item(player, object_id).cloned(),
            Place::Container(container) => self.container_item(container, object_id).cloned(),
        }
    }

    /// Java `getItemByItemId(itemId)`.
    pub(crate) fn place_item_by_item_id(&self, place: Place, item_id: i32) -> Option<ItemData> {
        match place {
            Place::Inventory(player) => self.inventory_item_by_item_id(player, item_id).cloned(),
            Place::Container(container) => self.container_item_by_item_id(container, item_id).cloned(),
        }
    }

    /// The statement `updateDatabase` runs for a row of this place: an inventory row may not be in
    /// the table yet, a container row is (every item in a container was written when it got there).
    fn row_op(place: Place, item: &ItemData) -> ItemRowOp {
        let owner_id = Self::place_owner(place);
        match place {
            Place::Inventory(_) => ItemRowOp::Write {
                owner_id,
                item: item.clone(),
            },
            Place::Container(_) => ItemRowOp::Update {
                owner_id,
                item: item.clone(),
            },
        }
    }

    /// Takes the item out of its place (`removeItem`): a worn item comes off, its shortcut goes.
    fn place_remove(&mut self, place: Place, object_id: ObjectId) -> Option<ItemData> {
        match place {
            Place::Inventory(player) => self.remove_inventory_item(player, object_id),
            Place::Container(container) => {
                let items = self.container_items_mut(container)?;
                let index = items.iter().position(|i| i.object_id == object_id)?;
                Some(items.remove(index))
            }
        }
    }

    /// `addItem(process, item, actor, reference)` for an item that is not in the place yet: the
    /// new owner and location (a stackable item never gets here, it joins the stack).
    fn place_add_whole(&mut self, place: Place, mut item: ItemData) -> Option<ItemData> {
        match place {
            Place::Inventory(player) => self.inventory_add_item(player, item).map(|added| added.item),
            Place::Container(container) => {
                item.loc = container.base_location().to_string();
                item.loc_data = container.loc_data();
                self.container_items_mut(container)?.push(item.clone());
                Some(item)
            }
        }
    }

    /// `Item.changeCount(process, count, ...)`: adena stops at `MaxAdena`, nothing goes below 0.
    fn place_change_count(&mut self, place: Place, object_id: ObjectId, count: i64) -> Option<ItemData> {
        match place {
            Place::Inventory(player) => self.change_item_count(player, object_id, count),
            Place::Container(container) => {
                let max_adena = self.settings.ground.max_adena;
                let item = self
                    .container_items_mut(container)?
                    .iter_mut()
                    .find(|i| i.object_id == object_id)?;
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
        }
    }

    /// `removeItem` + `ItemManager.destroyItem`: the whole stack goes, its id is released. The
    /// inventory side deletes its row itself; a container row is deleted by the caller's op.
    fn place_destroy(&mut self, place: Place, object_id: ObjectId, count: i64) -> Option<ItemData> {
        match place {
            Place::Inventory(player) => self.destroy_inventory_item(player, object_id, count),
            Place::Container(_) => {
                let mut removed = self.place_remove(place, object_id)?;
                removed.count = 0;
                removed.loc = "VOID".to_string();
                removed.loc_data = 0;
                self.release_item_id(object_id);
                Some(removed)
            }
        }
    }

    /// `addItem(process, itemId, count, actor, reference)` for an item the place does not hold
    /// yet: a new item with a new object id (D-39: the Rust range).
    fn place_create(&mut self, place: Place, item_id: i32, count: i64, actor: ObjectId) -> Option<ItemData> {
        match place {
            Place::Inventory(player) => self.create_transferred_item(player, item_id, count, actor),
            Place::Container(container) => {
                let templates = self.item_templates()?;
                let Some(template) = templates.get(item_id) else {
                    warn!("Invalid ItemId ({item_id}) requested by {actor}");
                    return None;
                };
                let id = self.allocate_item_id()?;
                let mut data = self.new_item_data(template, id, if template.stackable { count } else { 1 });
                data.loc = container.base_location().to_string();
                data.loc_data = container.loc_data();
                data.enchant_level = template.default_enchant_level;
                self.container_items_mut(container)?.push(data.clone());
                Some(data)
            }
        }
    }

    /// Java `ItemContainer.transferItem(process, objectId, count, target, actor, reference)`
    /// between two places. The rows to write are appended to `ops`. `None` when Java returns
    /// `null`: the item is not in `from`, or a count of 0 found nothing to return.
    pub(crate) fn move_item(
        &mut self,
        from: Place,
        object_id: ObjectId,
        count: i64,
        to: Place,
        actor: ObjectId,
        ops: &mut Vec<ItemRowOp>,
    ) -> Option<Moved> {
        let source = self.place_item(from, object_id)?;
        let stackable = self.data.item_stackable(source.item_id);
        let target = if stackable {
            self.place_item_by_item_id(to, source.item_id)
        } else {
            None
        };

        // Check if requested quantity is available.
        let count = count.min(source.count);

        // NOTE(java-quirk): the withdraw packet accepts a count of 0. Nothing is moved; Java
        // returns the item `addItem(itemId, 0)` finds in the target, which is the stack of a
        // stackable item, or any item with the same template id, or `null` (the handler then
        // stops the whole withdraw).
        if count == 0 {
            let existing = self.place_item_by_item_id(to, source.item_id)?;
            return Some(Moved {
                old: source,
                new: existing,
                moved_whole: false,
            });
        }

        let moved = if source.count == count && target.is_none() && !stackable {
            // If possible, move entire item object.
            let old = self.place_remove(from, object_id)?;
            let new = self.place_add_whole(to, old.clone())?;
            ops.push(Self::row_op_with_owner(from, to, &new));
            Moved {
                old,
                new,
                moved_whole: true,
            }
        } else {
            let (old, destroyed) = if source.count > count {
                // If possible, only update counts.
                (self.place_change_count(from, object_id, -count)?, false)
            } else {
                // Otherwise destroy old item.
                (self.place_destroy(from, object_id, source.count)?, true)
            };
            // `sourceitem.updateDatabase(true)`: the count, or the row gone.
            if !destroyed {
                ops.push(Self::row_op(from, &old));
            } else if matches!(from, Place::Container(_)) {
                ops.push(ItemRowOp::Delete { object_id });
            }
            let (new, joined) = match target {
                // If possible, only update counts.
                Some(existing) => (self.place_change_count(to, existing.object_id, count)?, true),
                // Otherwise add new item.
                None => (self.place_create(to, source.item_id, count, actor)?, false),
            };
            // `targetitem.updateDatabase()`.
            ops.push(if joined {
                Self::row_op(to, &new)
            } else {
                ItemRowOp::Insert {
                    owner_id: Self::place_owner(to),
                    item: new.clone(),
                }
            });
            Moved {
                old,
                new,
                moved_whole: false,
            }
        };

        if let Place::Inventory(player) = from {
            self.refresh_weight(player);
        }
        if let Place::Inventory(player) = to {
            self.refresh_weight(player);
        }
        Some(moved)
    }

    /// The row of an item that moved whole: the statement of the place it came from (an inventory
    /// row may not exist yet), with the owner of the place it went to.
    fn row_op_with_owner(from: Place, to: Place, item: &ItemData) -> ItemRowOp {
        let owner_id = Self::place_owner(to);
        match from {
            Place::Inventory(_) => ItemRowOp::Write {
                owner_id,
                item: item.clone(),
            },
            Place::Container(_) => ItemRowOp::Update {
                owner_id,
                item: item.clone(),
            },
        }
    }

    /// Sends the ops of one packet to the persistence task (`tracked` is the player at the
    /// keyboard, whose inventory rows the task keeps a baseline of).
    pub(crate) fn flush_item_rows(&mut self, tracked: ObjectId, ops: Vec<ItemRowOp>) {
        if ops.is_empty() {
            return;
        }
        self.outbox.persist(PersistCommand::ItemRows {
            tracked: Some(tracked),
            ops,
        });
    }
}
