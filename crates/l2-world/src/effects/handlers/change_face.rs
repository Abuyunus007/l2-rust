//! The `ChangeFace` effect: the caster's face becomes `value` (its user info is sent again).
// Java: dist/game/data/scripts/handlers/skill/effects/ChangeFace.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `ChangeFace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChangeFace {
    value: i32,
}

impl ChangeFace {
    /// Java `ChangeFace(attachCond, applyCond, set, params)`: `value` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            value: params::int(template, "value", 0)?,
        }))
    }
}

impl EffectHandler for ChangeFace {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effector) || !world.is_player(cx.effected) || world.is_alike_dead(cx.effected)
        {
            return;
        }
        if let Some(player) = world.players.get_mut(&cx.effector) {
            player.data.face = self.value;
        }
        world.broadcast_user_info(cx.effector);
    }
}
