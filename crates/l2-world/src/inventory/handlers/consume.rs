//! Using up an item by its object id.
// Java: entity/actor/Player.java (destroyItem(process, objectId, count, ...) and
//       destroyItem(process, item, count, ...))

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{InventoryUpdate, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::inventory::{LOC_PAPERDOLL, item_name_param};
use crate::world::World;

impl World {
    /// Java `Player.destroyItem(process, objectId, count, reference, sendMessage)`: `count` less
    /// of the item, or the item gone when it is all of it. The client gets the
    /// `InventoryUpdate`, the load and (with `send_message`) "$s1 has disappeared".
    pub(crate) fn destroy_item_by_object_id(
        &mut self,
        id: ObjectId,
        object_id: ObjectId,
        count: i64,
        send_message: bool,
    ) -> bool {
        let fail = |world: &Self| {
            if send_message {
                world.send(id, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            }
            false
        };
        let Some(item) = self.inventory_item(id, object_id).cloned() else {
            return fail(self);
        };
        if item.count < count {
            return fail(self);
        }

        let was_equipped = item.loc == LOC_PAPERDOLL;
        let Some(removed) = self.destroy_inventory_item(id, object_id, count) else {
            return fail(self);
        };

        // Send inventory update packet.
        let change = if removed.count == 0 {
            InventoryUpdate::REMOVED
        } else {
            InventoryUpdate::MODIFIED
        };
        let entry = self.update_entry(&removed, change);
        self.send_inventory_update(id, entry);

        if was_equipped && removed.count == 0 {
            // The client leaves the item in the paperdoll when the last one is used up, so the
            // full list is sent as well.
            self.send_item_list(id, false);
        }

        // Update current load as well.
        self.send_cur_load(id);

        // Sends message to client if requested.
        if send_message {
            let templates = self.item_templates();
            let name = item_name_param(templates.as_deref(), removed.item_id);
            let message = if count > 1 {
                SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                    .with(name)
                    .with(SystemMessageParam::LongNumber(count))
            } else {
                SystemMessage::new(SystemMessageId::S1HasDisappeared).with(name)
            };
            self.send(id, &message);
        }
        true
    }
}
