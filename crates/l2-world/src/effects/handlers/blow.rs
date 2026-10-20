//! The blow effects: `FatalBlow`, `Backstab` and `SoulBlow`.
// Java: dist/game/data/scripts/handlers/skill/effects/FatalBlow.java
// Java: dist/game/data/scripts/handlers/skill/effects/Backstab.java
// Java: dist/game/data/scripts/handlers/skill/effects/SoulBlow.java
//
// M5-SHOT-01: every soulshot charge is read as "not charged".

use l2_data::skill::EffectTemplate;

use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::world::World;

/// What the three blows do after the damage is known: the HP, the attack break, the damage
/// message and the counter.
///
/// NOTE(java-quirk): the damage message always says "critical" (`sendDamageMessage(.., false,
/// true, false)`), whatever the roll said.
fn finish_blow(world: &mut World, cx: &EffectCx<'_>, damage: f64) {
    let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
    world.reduce_current_hp_by_skill(effected, damage, Some(effector), skill);
    // `notifyDamageReceived(damage, effector, skill, crit, false)`: only the fake death part
    // exists (the critical flag is for script events).
    world.notify_damage_received(effected, damage);

    // Manage attack or cast break of the target (calculating rate, sending message...)
    world.skill_atk_break(effected, damage);

    if world.is_player(effector) {
        world.send_damage_message(effector, effected, damage as i32, false, true, false);
    }

    // Check if damage should be reflected.
    world.calc_damage_reflected_for(effector, effected, skill, true);
}

/// Java `FatalBlow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FatalBlow;

impl FatalBlow {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for FatalBlow {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        !world.calc_physical_skill_evasion_for(cx.effector, cx.effected, cx.skill)
            && world.calc_blow_success_for(cx.effector, cx.effected, cx.skill)
    }

    fn effect_type(&self) -> EffectType {
        EffectType::PhysicalAttack
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if world.is_alike_dead(effector) {
            return;
        }
        // M5-SHOT-01: `skill.useSoulShot() && effector.isChargedShot(ShotType.SOULSHOTS)`.
        let ss = false;
        let shld = world.calc_shld_use_skill(effector, effected, skill, true);
        let mut damage = world.calc_blow_damage_for(effector, effected, skill, shld, ss);
        let crit = world.calc_skill_crit_for(effector, effected, skill);
        if crit {
            damage *= 2.0;
        }
        finish_blow(world, cx, damage);
    }
}

/// Java `Backstab`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Backstab;

impl Backstab {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for Backstab {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        !world.is_in_front_of(cx.effector, cx.effected)
            && !world.calc_physical_skill_evasion_for(cx.effector, cx.effected, cx.skill)
            && world.calc_blow_success_for(cx.effector, cx.effected, cx.skill)
    }

    fn effect_type(&self) -> EffectType {
        EffectType::PhysicalAttack
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if world.is_alike_dead(effector) {
            return;
        }
        // M5-SHOT-01: `skill.useSoulShot() && effector.isChargedShot(ShotType.SOULSHOTS)`.
        let ss = false;
        let shld = world.calc_shld_use_skill(effector, effected, skill, true);
        let mut damage = world.calc_backstab_damage_for(effector, effected, skill, shld, ss);
        // Crit rate base crit rate for skill, modified with STR bonus.
        if world.calc_skill_crit_for(effector, effected, skill) {
            damage *= 2.0;
        }
        finish_blow(world, cx, damage);
    }
}

/// Java `SoulBlow`: a blow boosted by the souls the caster holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SoulBlow;

impl SoulBlow {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for SoulBlow {
    /// If it is not evaded and the blow lands.
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        !world.calc_physical_skill_evasion_for(cx.effector, cx.effected, cx.skill)
            && world.calc_blow_success_for(cx.effector, cx.effected, cx.skill)
    }

    fn effect_type(&self) -> EffectType {
        EffectType::PhysicalAttack
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if world.is_alike_dead(effector) {
            return;
        }
        // M5-SHOT-01: `skill.useSoulShot() && effector.isChargedShot(ShotType.SOULSHOTS)`.
        let ss = false;
        let shld = world.calc_shld_use_skill(effector, effected, skill, true);
        let mut damage = world.calc_blow_damage_for(effector, effected, skill, shld, ss);
        if skill.soul_max_consume > 0 && world.is_player(effector) {
            // Souls Formula (each soul increase +4.8%).
            let charged = world.charged_souls_of(effector);
            let souls = charged.min(skill.soul_max_consume);
            damage *= 1.0 + (f64::from(souls) * 0.048);
        }
        finish_blow(world, cx, damage);
    }
}
