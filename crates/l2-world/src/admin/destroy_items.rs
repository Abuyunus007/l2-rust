//! `//destroy_items` and `//destroy_all_items` (and the `destroyitems`/`destroyallitems` spellings):
//! the GM's inventory items are destroyed, the worn ones only with the word `all`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminDestroyItems.java
//
// The destroy is `PlayerInventory.destroyItem` of the whole item, which takes a worn item off
// first (`remove_inventory_item`). The removed entries go out in one `InventoryUpdate`.

use l2_protocol_game::server::{InventoryUpdate, ItemUpdate};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException};
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL};
use crate::summon::pet_items::LOC_PET_EQUIP;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminDestroyItems.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_destroy_items", admin_destroy_items),
        ("admin_destroy_all_items", admin_destroy_items),
        ("admin_destroyitems", admin_destroy_items),
        ("admin_destroyallitems", admin_destroy_items),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminDestroyItems.onCommand`: every item of the inventory (`getItems()`, the inventory
/// and the worn items) except the worn ones unless the command contains `all`.
fn admin_destroy_items(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let all = command.contains("all");
    let Some(player) = world.players.get(&gm) else {
        return Ok(());
    };
    let items: Vec<_> = player
        .data
        .items
        .iter()
        .filter(|i| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL || i.loc == LOC_PET_EQUIP)
        .filter(|i| all || !(i.loc == LOC_PAPERDOLL || i.loc == LOC_PET_EQUIP))
        .cloned()
        .collect();

    let mut updates: Vec<ItemUpdate> = Vec::new();
    for item in items {
        // `iu.addRemovedItem(item)` is read before the item is destroyed.
        updates.push(world.update_entry(&item, InventoryUpdate::REMOVED));
        world.destroy_inventory_item(gm, item.object_id, item.count);
    }
    world.send_inventory_update_now(gm, updates);
    Ok(())
}
