//! The damage that does not come from an attack formula: `DamOverTime`, `DamOverTimePercent`,
//! `ManaDamOverTime` and `CpDamPercent`.
// Java: dist/game/data/scripts/handlers/skill/effects/DamOverTime.java
// Java: dist/game/data/scripts/handlers/skill/effects/DamOverTimePercent.java
// Java: dist/game/data/scripts/handlers/skill/effects/ManaDamOverTime.java
// Java: dist/game/data/scripts/handlers/skill/effects/CpDamPercent.java

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;

use super::statset::{get_boolean, get_double};
use super::vitals::Vital;
use crate::effects::SkillKey;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// The tick of `DamOverTime` and `DamOverTimePercent`, which differ only in the damage.
fn damage_tick(world: &mut World, cx: &EffectCx<'_>, can_kill: bool, damage_of: impl Fn(f64) -> f64) -> bool {
    let effected = cx.effected;
    if world.is_dead(effected) {
        return false;
    }
    let current_hp = Vital::Hp.current(world, effected);
    let mut damage = damage_of(current_hp);
    if damage >= (current_hp - 1.0) {
        if cx.skill.is_toggle() {
            world.send(
                effected,
                &SystemMessage::new(SystemMessageId::YourSkillHasBeenCanceledDueToLackOfHp),
            );
            return false;
        }
        // For DOT skills that will not kill effected player.
        if !can_kill {
            // Fix for players dying by DOTs if HP < 1 since reduceCurrentHP method will kill them.
            if current_hp <= 1.0 {
                return cx.skill.is_toggle();
            }
            damage = current_hp - 1.0;
        }
    }
    world.reduce_current_hp_by_dot(effected, damage, Some(cx.effector), cx.skill);
    world.notify_damage_received_by(
        effected,
        damage,
        Some(cx.effector),
        Some(SkillKey::of(cx.skill)),
        false,
        true,
    );
    cx.skill.is_toggle()
}

/// Java `DamOverTime`: `power * ticksMultiplier` damage every tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DamOverTime {
    can_kill: bool,
    power: f64,
}

impl DamOverTime {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(DamOverTime {
            can_kill: get_boolean(template, "canKill", false),
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for DamOverTime {
    fn effect_type(&self) -> EffectType {
        EffectType::DmgOverTime
    }

    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let damage = self.power * cx.ticks_multiplier(world);
        damage_tick(world, cx, self.can_kill, |_| damage)
    }
}

/// Java `DamOverTimePercent`: `power * ticksMultiplier` times the current HP, every tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DamOverTimePercent {
    can_kill: bool,
    power: f64,
}

impl DamOverTimePercent {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(DamOverTimePercent {
            can_kill: get_boolean(template, "canKill", false),
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for DamOverTimePercent {
    fn effect_type(&self) -> EffectType {
        EffectType::DmgOverTimePercent
    }

    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let ticks = cx.ticks_multiplier(world);
        // `getCurrentHp() * _power * getTicksMultiplier()`, left to right.
        damage_tick(world, cx, self.can_kill, |hp| hp * self.power * ticks)
    }
}

/// Java `ManaDamOverTime`: `power * ticksMultiplier` MP every tick.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ManaDamOverTime {
    power: f64,
}

impl ManaDamOverTime {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(ManaDamOverTime {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for ManaDamOverTime {
    fn on_action_time(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let effected = cx.effected;
        if world.is_dead(effected) {
            return false;
        }
        let mana_dam = self.power * cx.ticks_multiplier(world);
        if mana_dam > Vital::Mp.current(world, effected) && cx.skill.is_toggle() {
            world.send(
                effected,
                &SystemMessage::new(SystemMessageId::YourSkillWasDeactivatedDueToLackOfMp),
            );
            return false;
        }
        world.reduce_mp(effected, mana_dam);
        cx.skill.is_toggle()
    }
}

/// Java `CpDamPercent`: takes `power` percent of the target's current CP.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CpDamPercent {
    power: f64,
}

impl CpDamPercent {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(CpDamPercent {
            power: get_double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for CpDamPercent {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        if !world.is_player(effected) {
            return;
        }
        // M5-EFX-02: a fake-death target stands up here (`stopFakeDeath`).
        let damage = ((Vital::Cp.current(world, effected) * self.power) / 100.0) as i32;
        // Manage attack or cast break of the target (calculating rate, sending message).
        world.skill_atk_break(effected, f64::from(damage));
        if damage > 0 {
            let cp = Vital::Cp.current(world, effected);
            world.set_current_cp(effected, cp - f64::from(damage), true);
        }
    }
}
