//! `FakeDeath`: the player plays dead.
// Java: dist/game/data/scripts/handlers/skill/effects/FakeDeath.java

use l2_data::skill::EffectTemplate;

use super::status_params::double_or;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `FakeDeath`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FakeDeath {
    power: f64,
}

impl FakeDeath {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(FakeDeath {
            power: double_or(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for FakeDeath {
    fn effect_type(&self) -> EffectType {
        EffectType::FakeDeath
    }

    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let effected = cx.effected;
        if world.is_dead(effected) {
            return false;
        }
        let mana_dam = self.power * cx.ticks_multiplier(world);
        let current_mp = world.core_of(effected).map_or(0.0, |c| c.cur_mp);
        if mana_dam > current_mp && cx.skill.is_toggle() {
            world.send_lack_of_mp(effected);
            return false;
        }
        world.reduce_mp(effected, mana_dam);
        cx.skill.is_toggle()
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `stopFakeDeath(false)` without the effect removal: the player gets up, the grace
        // period against monsters starts, and everybody sees it stand.
        world.stop_fake_death(cx.effected, false);
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.start_fake_death(cx.effected);
    }
}
