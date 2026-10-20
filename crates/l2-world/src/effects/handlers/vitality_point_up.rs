//! The `VitalityPointUp` effect: the target's vitality points go up by `value` (no rates), and its
//! user info is sent again.
// Java: dist/game/data/scripts/handlers/skill/effects/VitalityPointUp.java

use l2_data::skill::EffectTemplate;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `VitalityPointUp`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VitalityPointUp {
    value: f32,
}

impl VitalityPointUp {
    /// Java `VitalityPointUp(attachCond, applyCond, set, params)`: `value` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            value: params::float(template, "value", 0.0)?,
        }))
    }
}

impl EffectHandler for VitalityPointUp {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effected) {
            return;
        }
        world.update_vitality_points(cx.effected, self.value, false, false);
        // Java `updateUserInfo()`: `UserInfo` and `ExBrExtraUserInfo` to the player.
        world.send_user_info(cx.effected);
    }
}
