//! `Item.updateDatabase()`: item rows leave the world through the outbox as they change.
// Java: entity/item/instance/Item.java (updateDatabase, updateInDb, insertIntoDb, removeFromDb)
//
// Java writes an item the moment its location or count changes (`LazyItemsUpdate = False`, the
// default), not only when the character is stored. The persistence task picks `UPDATE` or
// `INSERT` by whether it knows the row (`_existsInDb`), so one command covers both. The full
// stores (logout, autosave) still compare the whole item list, which keeps changes Java only
// writes later (a partial destroy writes nothing until the next store) in sync.
//
// NOTE: `LazyItemsUpdate = True` (skip the write unless forced) is not honoured: rows are
// always written at once. The result in the database is the same after the next store.

use l2_util::ObjectId;

use crate::outbox::PersistCommand;
use crate::world::World;

impl World {
    /// Java `item.updateDatabase()` for an item the player still holds: its row as it is now.
    pub(crate) fn update_item_in_db(&mut self, owner: ObjectId, object_id: ObjectId) {
        let Some(item) = self
            .players
            .get(&owner)
            .and_then(|p| p.data.items.iter().find(|i| i.object_id == object_id))
            .cloned()
        else {
            return;
        };
        self.outbox.persist(PersistCommand::ItemChanges {
            owner,
            upserts: vec![item],
            removed: Vec::new(),
        });
    }

    /// Java `item.updateDatabase()` for an item that left the player (`owner 0` / `VOID`):
    /// `removeFromDb`.
    pub(crate) fn remove_item_from_db(&mut self, owner: ObjectId, object_id: ObjectId) {
        self.outbox.persist(PersistCommand::ItemChanges {
            owner,
            upserts: Vec::new(),
            removed: vec![object_id],
        });
    }
}
