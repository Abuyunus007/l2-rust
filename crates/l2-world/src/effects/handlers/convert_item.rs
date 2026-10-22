//! The `ConvertItem` effect: the weapon the player holds turns into its "change weapon" twin
//! (the weapon keeps its enchant level and element).
// Java: dist/game/data/scripts/handlers/skill/effects/ConvertItem.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{InventoryUpdate, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::restoration::set_enchant_level;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::inventory::item_name_param;
use crate::world::World;

/// Java `ConvertItem`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ConvertItem;

impl ConvertItem {
    /// Java `ConvertItem(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ConvertItem))
    }
}

impl EffectHandler for ConvertItem {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let player = cx.effected;
        if world.is_alike_dead(player) || !world.is_player(player) {
            return;
        }
        if world.is_enchanting(player) {
            return;
        }
        let Some(templates) = world.item_templates() else {
            return;
        };
        let Some(change_weapon_id) = world.stats(player).and_then(|v| {
            v.active_weapon_item()
                .and_then(|t| t.weapon())
                .map(|w| w.change_weapon_id)
        }) else {
            return;
        };

        let wpn = world
            .paperdoll_object(player, paperdoll::RHAND)
            .or_else(|| world.paperdoll_object(player, paperdoll::LHAND));
        let Some(wpn) = wpn else {
            return;
        };
        if world.is_item_augmented(wpn) || change_weapon_id == 0 {
            return;
        }
        let new_item_id = change_weapon_id;
        if new_item_id == -1 {
            return;
        }

        let Some(old) = world.inventory_item(player, wpn).cloned() else {
            return;
        };
        let enchant_level = old.enchant_level;
        let elemental = world.item_elemental(wpn);
        let Some(body_part) = templates.get(old.item_id).map(|t| t.body_part) else {
            return;
        };
        let mut unequipped = world.unequip_item_in_body_slot_and_record(player, body_part);
        unequipped.dedup();
        world.send_changed_items(player, &unequipped);
        if unequipped.is_empty() {
            return;
        }

        let mut count = 0;
        for &object_id in &unequipped {
            let Some(item) = world.inventory_item(player, object_id).cloned() else {
                count += 1;
                continue;
            };
            if !templates.get(item.item_id).is_some_and(|t| t.is_weapon()) {
                count += 1;
                continue;
            }
            let message = world.disarmed_message(item.enchant_level, item.item_id, Some(&templates));
            world.send(player, &message);
        }
        if count == unequipped.len() {
            return;
        }

        // `destroyItem(TRANSFER, wpn, player, null)`.
        let Some(destroyed) = world.destroy_inventory_item(player, wpn, old.count) else {
            return;
        };
        // `addItem(TRANSFER, newItemId, 1, player, destroyItem)`.
        let Some(added) = world.inventory_add_item_by_id(player, new_item_id, 1) else {
            return;
        };
        let new_item: ObjectId = added.item.object_id;
        if let Some(elemental) = elemental
            && elemental.element != -1
            && elemental.value != -1
        {
            world.set_item_elemental(new_item, elemental.element, elemental.value);
        }
        set_enchant_level(world, player, new_item, enchant_level);
        let mut changed = Vec::new();
        world.equip_item(player, new_item, &mut changed);

        let name = item_name_param(Some(&templates), new_item_id);
        let message = if enchant_level > 0 {
            SystemMessage::new(SystemMessageId::EquippedS1S2)
                .with(SystemMessageParam::IntNumber(enchant_level))
                .with(name)
        } else {
            SystemMessage::new(SystemMessageId::YouHaveEquippedYourS1).with(name)
        };
        world.send(player, &message);

        // `u.addRemovedItem(destroyItem); u.addItem(newItem);`
        let removed = world.update_entry(&destroyed, InventoryUpdate::REMOVED);
        world.send_inventory_update(player, removed);
        if let Some(now) = world.inventory_item(player, new_item).cloned() {
            let entry = world.update_entry(&now, InventoryUpdate::ADDED);
            world.send_inventory_update(player, entry);
        }
        world.broadcast_user_info(player);
    }
}
