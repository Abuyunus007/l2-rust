//! `Hide`: the player becomes invisible and everybody who had it as a target forgets it.
// Java: dist/game/data/scripts/handlers/skill/effects/Hide.java

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `Hide`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Hide;

impl Hide {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Hide))
    }
}

impl EffectHandler for Hide {
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let player = cx.effected;
        if !world.is_player(player) {
            return;
        }
        world.set_invisible(player, true);
        // Java also drops the player's next intention when it is ATTACK (`getNextIntention`);
        // the AI keeps no queued intention here.
        for creature in world.visible_creatures_of(player) {
            if world.creature_target(creature) != Some(player) {
                continue;
            }
            if world.is_attackable_npc(creature) {
                world.aggro_clear(creature);
            }
            world.set_creature_target(creature, None);
            world.abort_attack(creature);
            world.abort_cast_for_effects(creature);
            world.set_intention_idle(creature);
        }
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `!player.inObserverMode()`: nobody observes yet.
        if world.is_player(cx.effected) {
            world.set_invisible(cx.effected, false);
        }
    }
}
