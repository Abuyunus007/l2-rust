//! The `GiveSp` effect: the caster gets `sp` skill points (no exp), when both sides are players and
//! the target is alive.
// Java: dist/game/data/scripts/handlers/skill/effects/GiveSp.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `GiveSp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GiveSp {
    sp: i32,
}

impl GiveSp {
    /// Java `GiveSp(attachCond, applyCond, set, params)`: `sp` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            sp: params::int(template, "sp", 0)?,
        }))
    }
}

impl EffectHandler for GiveSp {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effector) || !world.is_player(cx.effected) || world.is_alike_dead(cx.effected)
        {
            return;
        }
        // Java `effector.asPlayer().addExpAndSp(0, _sp)`: no bonuses, no vitality.
        world.add_exp_and_sp(cx.effector, 0.0, f64::from(self.sp), false);
    }
}
