//! The `Restoration` effect: the playable gets `itemCount` of `itemId` (enchanted to
//! `itemEnchantmentLevel` when that is above zero).
// Java: dist/game/data/scripts/handlers/skill/effects/Restoration.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::warn;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Restoration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Restoration {
    item_id: i32,
    item_count: i64,
    item_enchantment_level: i32,
}

impl Restoration {
    /// Java `Restoration(attachCond, applyCond, set, params)`: `itemId` and `itemEnchantmentLevel`
    /// (default 0), `itemCount` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            item_id: params::int(template, "itemId", 0)?,
            item_count: params::long(template, "itemCount", 0)?,
            item_enchantment_level: params::int(template, "itemEnchantmentLevel", 0)?,
        }))
    }
}

impl EffectHandler for Restoration {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.stats(cx.effected).is_some_and(|v| v.is_playable()) {
            return;
        }
        if self.item_id <= 0 || self.item_count <= 0 {
            world.send(
                cx.effected,
                &SystemMessage::new(SystemMessageId::ThereWasNothingFoundInside),
            );
            warn!(
                "Restoration effect with wrong item Id/count: {}/{}!",
                self.item_id, self.item_count
            );
            return;
        }
        if world.is_player(cx.effected) {
            if let Some(object_id) = world.add_item(cx.effected, self.item_id, self.item_count, true)
                && self.item_enchantment_level > 0
            {
                set_enchant_level(world, cx.effected, object_id, self.item_enchantment_level);
            }
        } else if world.is_pet(cx.effected) {
            // The pet gets the item in its own inventory and the owner its list.
            world.pet_restoration_item(
                cx.effected,
                self.item_id,
                self.item_count,
                self.item_enchantment_level,
            );
        }
    }
}

/// Java `Item.setEnchantLevel(level)` on an item of `player`. Like Java, nothing is sent.
pub(crate) fn set_enchant_level(world: &mut World, player: ObjectId, object_id: ObjectId, level: i32) {
    if let Some(item) = world
        .players
        .get_mut(&player)
        .and_then(|p| p.data.items.iter_mut().find(|i| i.object_id == object_id))
    {
        item.enchant_level = level;
    }
}
