//! The MP restoring effects: `ManaHeal`, `ManaHealByLevel`, `ManaHealOverTime` and
//! `ManaHealPercent`.
// Java: dist/game/data/scripts/handlers/skill/effects/ManaHeal.java
// Java: dist/game/data/scripts/handlers/skill/effects/ManaHealByLevel.java
// Java: dist/game/data/scripts/handlers/skill/effects/ManaHealOverTime.java
// Java: dist/game/data/scripts/handlers/skill/effects/ManaHealPercent.java

use l2_data::Stat;
use l2_data::skill::EffectTemplate;

use super::statset::get_double;
use super::vitals::{Vital, is_door};
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// `effected.calcStat(Stat.MANA_CHARGE, amount, null, null)`.
fn mana_charge(world: &World, id: l2_util::ObjectId, amount: f64) -> f64 {
    world.stats(id).map_or(amount, |view| {
        view.calc_stat(Stat::ManaCharge, amount, None, None)
    })
}

/// Java `ManaHeal`: `power` MP (scaled by the target's mana charge stat unless the skill is
/// static).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ManaHeal {
    power: f64,
}

impl ManaHeal {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ManaHeal {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for ManaHeal {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        if world.is_dead(effected) || is_door(world, effected) || world.is_invul(effected) {
            return;
        }
        let mut amount = self.power;
        if !cx.skill.is_static() {
            amount = mana_charge(world, effected, amount);
        }
        // Prevents overheal and negative amount.
        let amount = Vital::Mp.clamp_to_room(world, effected, amount);
        Vital::Mp.add(world, effected, amount);
        Vital::Mp.send_restored(world, cx.effector, effected, cx.effector != effected, amount);
    }
}

/// Java `ManaHealByLevel`: like `ManaHeal`, but a target far above the skill's magic level gets
/// less.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ManaHealByLevel {
    power: f64,
}

impl ManaHealByLevel {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ManaHealByLevel {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for ManaHealByLevel {
    fn effect_type(&self) -> EffectType {
        EffectType::ManahealByLevel
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        if world.is_dead(effected) || is_door(world, effected) || world.is_invul(effected) {
            return;
        }
        let mut amount = self.power;
        // Recharged MP influenced by difference between target level and skill level.
        // If target is within 5 levels or lower then skill level there's no penalty.
        // NOTE(java-quirk): unlike `ManaHeal`, the mana charge stat applies to a static skill too.
        amount = mana_charge(world, effected, amount);
        let level = world.stats(effected).map_or(0, |view| view.level());
        if level > cx.skill.magic_level {
            let level_diff = level - cx.skill.magic_level;
            // If target is too high compared to skill level, the amount of recharged mp gradually
            // decreases.
            amount = match level_diff {
                6 => amount * 0.9,
                7 => amount * 0.8,
                8 => amount * 0.7,
                9 => amount * 0.6,
                10 => amount * 0.5,
                11 => amount * 0.4,
                12 => amount * 0.3,
                13 => amount * 0.2,
                14 => amount * 0.1,
                d if d >= 15 => 0.0,
                _ => amount,
            };
        }
        // Prevents overheal and negative amount.
        let amount = Vital::Mp.clamp_to_room(world, effected, amount);
        Vital::Mp.add(world, effected, amount);
        Vital::Mp.send_restored(world, cx.effector, effected, cx.effector != effected, amount);
    }
}

/// Java `ManaHealOverTime`: `power * ticksMultiplier` MP every tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ManaHealOverTime {
    power: f64,
}

impl ManaHealOverTime {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ManaHealOverTime {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for ManaHealOverTime {
    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let effected = cx.effected;
        if world.is_dead(effected) {
            return false;
        }
        let mut mp = Vital::Mp.current(world, effected);
        let max_mp = Vital::Mp.max_recoverable(world, effected);
        // Not needed to set the MP and send update packet if player is already at max MP.
        // NOTE(java-quirk): `HealOverTime` and `CpHealOverTime` return false here (which ends a
        // toggle); this one returns true.
        if mp >= max_mp {
            return true;
        }
        mp += self.power * cx.ticks_multiplier(world);
        mp = mp.min(max_mp);
        world.set_current_mp(effected, mp, true);
        cx.skill.is_toggle()
    }
}

/// Java `ManaHealPercent`: `power` percent of the maximum MP, instantly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ManaHealPercent {
    power: f64,
}

impl ManaHealPercent {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ManaHealPercent {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for ManaHealPercent {
    fn effect_type(&self) -> EffectType {
        EffectType::ManahealPercent
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        if world.is_dead(effected) || is_door(world, effected) {
            return;
        }
        let full = self.power == 100.0;
        let max_mp = Vital::Mp.max(world, effected);
        let amount = if full {
            max_mp
        } else {
            (max_mp * self.power) / 100.0
        };
        // Prevents overheal and negative amount.
        let amount = Vital::Mp.clamp_to_room(world, effected, amount);
        Vital::Mp.add(world, effected, amount);
        Vital::Mp.send_restored(world, cx.effector, effected, cx.effector != effected, amount);
    }
}
