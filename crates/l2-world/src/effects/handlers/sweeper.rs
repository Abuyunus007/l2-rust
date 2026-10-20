//! The `Sweeper` effect: the spoiler takes the SPOIL items of a dead monster.
// Java: dist/game/data/scripts/handlers/skill/effects/Sweeper.java
// Java: entity/actor/Player.java (addItem with `ItemProcessType.SWEEP`)

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Sweeper`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Sweeper;

impl Sweeper {
    /// Java `Sweeper(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Sweeper))
    }
}

impl EffectHandler for Sweeper {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effector) || !world.is_attackable_npc(cx.effected) {
            return;
        }
        if !world.check_spoil_owner(cx.effected, cx.effector) {
            return;
        }
        let loot = world.spoil_loot_items(cx.effected);
        if !world.check_inventory_slots_and_weight(cx.effector, &loot) {
            return;
        }
        if let Some(items) = world.take_sweep(cx.effected) {
            for item in items {
                if world.is_in_party(cx.effector) {
                    world.party_distribute_item(cx.effector, item.id, item.count, true, cx.effected);
                } else {
                    world.add_swept_item(cx.effector, item.id, item.count);
                }
            }
        }
    }
}
