//! `SilentMove`: the toggle that hides the creature from monsters while it drains MP.
// Java: dist/game/data/scripts/handlers/skill/effects/SilentMove.java

use l2_data::skill::EffectTemplate;

use super::status_params::double_or;
use crate::effects::registry::{EffectCx, EffectHandler, effect_flag};
use crate::world::World;

/// Java `SilentMove`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SilentMove {
    power: f64,
}

impl SilentMove {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(SilentMove {
            power: double_or(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for SilentMove {
    fn effect_flags(&self) -> u32 {
        effect_flag::SILENT_MOVE
    }

    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let effected = cx.effected;
        if world.is_dead(effected) {
            return false;
        }
        let mana_dam = self.power * cx.ticks_multiplier(world);
        let current_mp = world.core_of(effected).map_or(0.0, |c| c.cur_mp);
        if mana_dam > current_mp {
            world.send_lack_of_mp(effected);
            return false;
        }
        world.reduce_mp(effected, mana_dam);
        cx.skill.is_toggle()
    }
}
