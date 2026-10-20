//! `Disarm`: the player's weapon comes off and goes back on when the effect ends.
// Java: dist/game/data/scripts/handlers/skill/effects/Disarm.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::server::charscreen::paperdoll;

use crate::effects::registry::{EffectCx, EffectHandler, effect_flag};
use crate::inventory::body_part_of_worn_item;
use crate::world::World;

/// Java `Disarm`. Java keeps the disarmed item of each player in a static map; here it is
/// `CreatureControl::disarmed_item`, so the handler itself holds no state.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Disarm;

impl Disarm {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Disarm))
    }
}

impl EffectHandler for Disarm {
    /// `effected.isPlayer()`.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        world.is_player(cx.effected)
    }

    fn effect_flags(&self) -> u32 {
        effect_flag::DISARMED
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let player = cx.effected;
        if !world.is_player(player) {
            return;
        }
        let Some(weapon) = world.paperdoll_object(player, paperdoll::RHAND) else {
            return;
        };
        let Some(item) = world.inventory_item(player, weapon).cloned() else {
            return;
        };
        let templates = world.item_templates();
        let Some(template) = templates.as_deref().and_then(|t| t.get(item.item_id)) else {
            return;
        };
        let Some(part) = body_part_of_worn_item(&item, template) else {
            return;
        };
        let mut changed = Vec::new();
        world.unequip_item_in_body_slot(player, part, &mut changed);
        if let Some(entry) = world.modified_entry(player, weapon) {
            world.send_inventory_update(player, entry);
        }
        world.broadcast_user_info(player);
        if let Some(control) = world.control_mut(player) {
            control.disarmed_item = Some(weapon);
        }
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        let player = cx.effected;
        if !world.is_player(player) {
            return;
        }
        let Some(item) = world.control_mut(player).and_then(|c| c.disarmed_item.take()) else {
            return;
        };
        if world.inventory_item(player, item).is_none() {
            return;
        }
        let mut changed = Vec::new();
        world.equip_item(player, item, &mut changed);
        if let Some(entry) = world.modified_entry(player, item) {
            world.send_inventory_update(player, entry);
        }
    }
}
