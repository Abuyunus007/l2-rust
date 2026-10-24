//! A dead pet whose corpse decays while its owner is offline (M6-FU-09): the pet's items go back
//! to the owner, the collar is destroyed and the `pets` row goes, in the database.
// Java: entity/actor/instance/Pet.java (deleteMe, destroyControlItem)
// Java: entity/itemcontainer/{PetInventory.transferItemsToOwner,ItemContainer.transferItem,destroyItem}.java
//
// # Why this exists
// A dead pet stays when its owner logs out (`Player.deleteMe` only unsummons a living one). Java's
// `Summon` keeps a reference to its `Player` object, which stays in memory after the logout, so
// when the corpse decays `Pet.deleteMe(_owner)` moves the items into that object's inventory and
// destroys the collar there, and every `Item.updateDatabase` writes the rows. The world has no
// object of the offline owner, so the rows come from the database (D-15: the read goes out through
// the outbox and the answer is a world command, `WorldCommand::OfflinePetRowsLoaded`) and the
// same moves are made on them here.
//
// # Left out
// - The `Player` object of Java can be stale (its inventory is the one of the logout) and the
//   character may log in again before the decay; then the new session's items are the truth
//   and the online code runs (see [`World::on_offline_pet_rows_loaded`]).

use l2_util::ObjectId;
use tracing::warn;

use super::pet_items::{LOC_PET, LOC_PET_EQUIP};
use crate::outbox::{ItemRowOp, PersistCommand};
use crate::player::ItemData;
use crate::world::World;

const LOC_INVENTORY: &str = "INVENTORY";
const LOC_PAPERDOLL: &str = "PAPERDOLL";

/// The rows of an offline owner that the decayed pet's items and collar touch.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OfflinePetRows {
    /// `loc` `PET` and `PET_EQUIP` rows of the owner (`PetInventory.restore`).
    pub pet_items: Vec<ItemData>,
    /// `loc` `INVENTORY` and `PAPERDOLL` rows of the owner (`PlayerInventory.restore`).
    pub inventory: Vec<ItemData>,
}

impl World {
    /// The part of `Pet.deleteMe(owner)` an owner who is not in the world still gets: the
    /// database is asked for the rows the moves need.
    pub(crate) fn pet_decayed_offline(&mut self, owner: ObjectId, control: ObjectId) {
        self.outbox.persist(PersistCommand::OfflinePetRowsRequest {
            owner,
            control_object_id: control,
        });
    }

    /// `WorldCommand::OfflinePetRowsLoaded`: `transferItemsToOwner`, `destroyControlItem` and
    /// `DELETE FROM pets` on the rows. A failed read changes nothing at all (D-50: no guessing
    /// from memory), so the collar, its items and its row stay as they were.
    pub(crate) fn on_offline_pet_rows_loaded(
        &mut self,
        owner: ObjectId,
        control: ObjectId,
        result: Result<OfflinePetRows, ()>,
    ) {
        // The owner logged in again while the database answered: what it holds now is the truth
        // (its items were read at the login), so the moves are the online ones.
        if self.players.contains_key(&owner) {
            self.pet_transfer_items_to_owner(owner);
            self.pet_destroy_control_item(owner, control);
            return;
        }
        let Ok(rows) = result else {
            warn!(
                "Could not read the rows of {owner} for its decayed pet (collar {control}); its items and collar stay."
            );
            return;
        };
        let ops = self.offline_pet_ops(owner, control, rows);
        if !ops.is_empty() {
            self.outbox
                .persist(PersistCommand::ItemRows { tracked: None, ops });
        }
        // `DELETE FROM pets WHERE item_obj_id = ?`.
        self.pet_row_deleted(control);
    }

    /// The row writes of `transferItemsToOwner()` followed by `destroyItem(control)` on the
    /// owner's inventory rows.
    fn offline_pet_ops(
        &mut self,
        owner: ObjectId,
        control: ObjectId,
        rows: OfflinePetRows,
    ) -> Vec<ItemRowOp> {
        let OfflinePetRows {
            pet_items,
            mut inventory,
        } = rows;
        let mut ops = Vec::new();
        for source in pet_items
            .into_iter()
            .filter(|i| i.loc == LOC_PET || i.loc == LOC_PET_EQUIP)
        {
            // `ItemContainer.transferItem(.., source, count, ownerInventory, ..)` with the whole
            // count.
            let stackable = self.data.item_stackable(source.item_id);
            let target = stackable
                .then(|| inventory.iter().position(|i| i.item_id == source.item_id))
                .flatten();
            if !stackable {
                // "If possible, move entire item object": a worn pet item comes off.
                let mut moved = source;
                moved.loc = LOC_INVENTORY.to_string();
                moved.loc_data = 0;
                ops.push(ItemRowOp::Update {
                    owner_id: owner.0,
                    item: moved.clone(),
                });
                inventory.push(moved);
                continue;
            }
            // Otherwise the old item is destroyed and the count goes to the owner's stack or to
            // a new item.
            let count = source.count;
            let source_id = source.object_id;
            match target {
                Some(index) => {
                    let stack = &mut inventory[index];
                    stack.count = stack.count.wrapping_add(count);
                    ops.push(ItemRowOp::Update {
                        owner_id: owner.0,
                        item: stack.clone(),
                    });
                }
                None => {
                    let created = self.item_templates().and_then(|templates| {
                        let template = templates.get(source.item_id)?;
                        let id = self.allocate_item_id()?;
                        let mut data = self.new_item_data(template, id, count);
                        data.loc = LOC_INVENTORY.to_string();
                        data.loc_data = 0;
                        data.enchant_level = template.default_enchant_level;
                        Some(data)
                    });
                    let Some(created) = created else {
                        // The source row stays where it is rather than losing the count.
                        warn!(
                            "Could not make an item for {owner}'s stack of {}.",
                            source.item_id
                        );
                        continue;
                    };
                    ops.push(ItemRowOp::Insert {
                        owner_id: owner.0,
                        item: created.clone(),
                    });
                    inventory.push(created);
                }
            }
            ops.push(ItemRowOp::Delete { object_id: source_id });
        }

        // `destroyControlItem(owner, false)`: the collar (count 1) leaves the inventory.
        if let Some(index) = inventory
            .iter()
            .position(|i| i.object_id == control && (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL))
        {
            ops.push(ItemRowOp::Delete { object_id: control });
            inventory.remove(index);
        } else {
            warn!("Couldn't destroy pet control item for {owner} pet: {control}");
        }
        ops
    }
}
