//! The HP healing effects: `HealPercent`, `HpByLevel` and `HealOverTime` (`Heal` lives in
//! `heal_skill.rs`, it needs the M5-FORM-02 number).
// Java: dist/game/data/scripts/handlers/skill/effects/HealPercent.java
// Java: dist/game/data/scripts/handlers/skill/effects/HpByLevel.java
// Java: dist/game/data/scripts/handlers/skill/effects/HealOverTime.java

use l2_data::skill::EffectTemplate;
use l2_data::stats::AbnormalType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExRegenMax, SystemMessage, SystemMessageParam};

use super::statset::{get_double, get_int};
use super::vitals::{Vital, is_door};
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// Java `HealPercent`: `power` percent of the maximum HP, instantly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HealPercent {
    power: i32,
}

impl HealPercent {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(HealPercent {
            power: get_int(template, "power", 0)?,
        }))
    }
}

impl EffectHandler for HealPercent {
    fn effect_type(&self) -> EffectType {
        EffectType::Heal
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        if world.is_dead(effected) || is_door(world, effected) {
            return;
        }
        let power = f64::from(self.power);
        let full = power == 100.0;
        let max_hp = Vital::Hp.max(world, effected);
        let amount = if full { max_hp } else { (max_hp * power) / 100.0 };
        // Prevents overheal and negative amount.
        let amount = Vital::Hp.clamp_to_room(world, effected, amount);
        Vital::Hp.add(world, effected, amount);
        Vital::Hp.send_restored(world, cx.effector, effected, cx.effector != effected, amount);
    }
}

/// Java `HpByLevel`: the caster gets `power` HP (up to its maximum). Despite the name the power
/// does not depend on any level.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HpByLevel {
    power: f64,
}

impl HpByLevel {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(HpByLevel {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for HpByLevel {
    fn effect_type(&self) -> EffectType {
        EffectType::Buff
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // NOTE(java-quirk): the HP goes to the effector, not to the effected; it ignores
        // `getMaxRecoverableHp` and the dead flag is left to `setCurrentHp`.
        let effector = cx.effector;
        let current = Vital::Hp.current(world, effector);
        let max_hp = Vital::Hp.max(world, effector);
        let absorb = if current + self.power > max_hp {
            max_hp
        } else {
            current + self.power
        };
        let restored = (absorb - current) as i32;
        world.set_current_hp(effector, absorb, true);
        world.send(
            effector,
            &SystemMessage::new(SystemMessageId::S1HpHasBeenRestored)
                .with(SystemMessageParam::IntNumber(restored)),
        );
    }
}

/// Java `HealOverTime`: `power * ticksMultiplier` HP every tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HealOverTime {
    power: f64,
}

impl HealOverTime {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(HealOverTime {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for HealOverTime {
    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let effected = cx.effected;
        if world.is_dead(effected) || is_door(world, effected) {
            return false;
        }
        let mut hp = Vital::Hp.current(world, effected);
        let max_hp = Vital::Hp.max_recoverable(world, effected);
        // Not needed to set the HP and send update packet if player is already at max HP.
        if hp >= max_hp {
            return false;
        }
        hp += self.power * cx.ticks_multiplier(world);
        hp = hp.min(max_hp);
        world.set_current_hp(effected, hp, true);
        cx.skill.is_toggle()
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_player(cx.effected) && cx.ticks() > 0 && cx.skill.abnormal_type == AbnormalType::HpRecover
        {
            world.send(
                cx.effected,
                &ExRegenMax {
                    time: cx.skill.abnormal_time,
                    tick_interval: cx.ticks(),
                    amount_per_tick: self.power,
                },
            );
        }
    }
}
