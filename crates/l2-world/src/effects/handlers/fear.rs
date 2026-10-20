//! Fear and its instant sibling: the creature runs away from the effector.
// Java: dist/game/data/scripts/handlers/skill/effects/Fear.java
// Java: dist/game/data/scripts/handlers/skill/effects/RunAway.java

use l2_data::npc::NpcRace;
use l2_data::skill::EffectTemplate;

use super::status_params::int_or;
use crate::formulas::Rnd;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType, effect_flag};
use crate::world::World;

/// Java `Fear`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Fear;

impl Fear {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Fear))
    }
}

impl EffectHandler for Fear {
    /// Players, summons, and attackables that are no defenders, fort commanders, siege flags or
    /// siege weapons.
    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        let Some(view) = world.stats(cx.effected) else {
            return false;
        };
        view.is_player()
            || (view.is_attackable()
                && !view.npc_template().is_some_and(|t| {
                    matches!(t.npc_type.as_str(), "Defender" | "FortCommander" | "SiegeFlag")
                        || t.race == NpcRace::SiegeWeapon
                }))
    }

    fn effect_flags(&self) -> u32 {
        effect_flag::FEAR
    }

    fn effect_type(&self) -> EffectType {
        EffectType::Fear
    }

    /// Java `getTicks()`: five, whatever the skill says.
    fn ticks(&self, _template_ticks: i32) -> i32 {
        5
    }

    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        world.notify_action_afraid(cx.effected, cx.effector, false);
        false
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_casting_now_for_effects(cx.effected) && world.can_abort_cast_for_effects(cx.effected) {
            world.abort_cast_for_effects(cx.effected);
        }
        world.notify_action_afraid(cx.effected, cx.effector, true);
    }
}

/// Java `RunAway`: an instant fear for attackable NPCs, with a chance and a duration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunAway {
    power: i32,
    time: i32,
}

impl RunAway {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(RunAway {
            power: int_or(template, "power", 0)?,
            time: int_or(template, "time", 0)?,
        }))
    }
}

impl EffectHandler for RunAway {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_attackable_npc(cx.effected) {
            return;
        }
        if world.rng().get_int(100) > self.power {
            return;
        }
        if world.is_casting_now_for_effects(cx.effected) && world.can_abort_cast_for_effects(cx.effected) {
            world.abort_cast_for_effects(cx.effected);
        }
        // `getAI()` makes the AI of the NPC first.
        world.wake_ai(cx.effected);
        world.set_fear_time(cx.effected, self.time);
        world.notify_action_afraid(cx.effected, cx.effector, true);
    }
}
