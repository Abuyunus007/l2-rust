//! The CP restoring effects: `CpHeal`, `CpHealOverTime` and `CpHealPercent`.
// Java: dist/game/data/scripts/handlers/skill/effects/CpHeal.java
// Java: dist/game/data/scripts/handlers/skill/effects/CpHealOverTime.java
// Java: dist/game/data/scripts/handlers/skill/effects/CpHealPercent.java

use l2_data::skill::EffectTemplate;

use super::statset::get_double;
use super::vitals::{Vital, is_door};
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `CpHeal`: `power` CP, instantly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CpHeal {
    power: f64,
}

impl CpHeal {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(CpHeal {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for CpHeal {
    fn effect_type(&self) -> EffectType {
        EffectType::Cpheal
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        if world.is_dead(effected) || is_door(world, effected) {
            return;
        }
        // Prevents overheal and negative amount.
        let amount = Vital::Cp.clamp_to_room(world, effected, self.power);
        Vital::Cp.add(world, effected, amount);
        Vital::Cp.send_restored(world, cx.effector, effected, cx.effector != effected, amount);
    }
}

/// Java `CpHealOverTime`: `power * ticksMultiplier` CP every tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CpHealOverTime {
    power: f64,
}

impl CpHealOverTime {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(CpHealOverTime {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for CpHealOverTime {
    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let effected = cx.effected;
        if world.is_dead(effected) {
            return false;
        }
        let mut cp = Vital::Cp.current(world, effected);
        let max_cp = Vital::Cp.max_recoverable(world, effected);
        // Not needed to set the CP and send update packet if player is already at max CP.
        if cp >= max_cp {
            return false;
        }
        cp += self.power * cx.ticks_multiplier(world);
        cp = cp.min(max_cp);
        world.set_current_cp(effected, cp, true);
        // NOTE(java-quirk): always true, where `HealOverTime` returns `skill.isToggle()`.
        true
    }
}

/// Java `CpHealPercent`: `power` percent of the maximum CP, instantly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CpHealPercent {
    power: f64,
}

impl CpHealPercent {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(CpHealPercent {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for CpHealPercent {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        if world.is_dead(effected) || is_door(world, effected) {
            return;
        }
        let full = self.power == 100.0;
        let max_cp = Vital::Cp.max(world, effected);
        let amount = if full {
            max_cp
        } else {
            (max_cp * self.power) / 100.0
        };
        // Prevents overheal and negative amount.
        let amount = Vital::Cp.clamp_to_room(world, effected, amount);
        Vital::Cp.add(world, effected, amount);
        Vital::Cp.send_restored(world, cx.effector, effected, cx.effector != effected, amount);
    }
}
