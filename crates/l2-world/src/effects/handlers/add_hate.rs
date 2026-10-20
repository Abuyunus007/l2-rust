//! The `AddHate` effect: an attackable NPC's hate for the caster grows by `power` (or shrinks
//! when it is negative).
// Java: dist/game/data/scripts/handlers/skill/effects/AddHate.java

use l2_data::skill::EffectTemplate;

use super::hate::reduce_hate;
use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `AddHate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AddHate {
    power: f64,
}

impl AddHate {
    /// Java `AddHate(attachCond, applyCond, set, params)`: `power` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: params::double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for AddHate {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_attackable_npc(cx.effected) {
            return;
        }
        let val = self.power;
        if val > 0.0 {
            world.add_damage_hate(cx.effected, cx.effector, 0, i64::from(val as i32));
        } else if val < 0.0 {
            // NOTE(java-quirk): `reduceHate` adds its amount, so a negative power raises the hate
            // by `-power` (the call reads as a reduction, the effect does the opposite).
            reduce_hate(world, cx.effected, cx.effector, i64::from((-val) as i32));
        }
    }
}
