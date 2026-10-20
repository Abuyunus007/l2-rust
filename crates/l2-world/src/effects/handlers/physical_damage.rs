//! The physical damage effects: `PhysicalDamage`, `PhysicalDamageHpLink`, `PhysicalSoulDamage`
//! and `EnergyDamage`.
// Java: dist/game/data/scripts/handlers/skill/effects/PhysicalDamage.java
// Java: dist/game/data/scripts/handlers/skill/effects/PhysicalDamageHpLink.java
// Java: dist/game/data/scripts/handlers/skill/effects/PhysicalSoulDamage.java
// Java: dist/game/data/scripts/handlers/skill/effects/EnergyDamage.java
//
// The damage, crit, shield and evasion numbers come from `skill_calc.rs` (M5-FORM-02's formulas).
// M5-SHOT-01: every soulshot charge is read as "not charged".

use l2_data::skill::EffectTemplate;
use l2_data::{SkillTemplate, Stat};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::formulas::{Rnd, SHIELD_DEFENSE_PERFECT_BLOCK, SHIELD_DEFENSE_SUCCEED};
use crate::stats::BaseStat;
use crate::world::World;

/// `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` with the skill name, to the effector.
fn send_unsuitable_terms(world: &World, effector: ObjectId, skill: &SkillTemplate) {
    world.send(
        effector,
        &SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms)
            .with(crate::effects::buff_info::skill_name_param(skill)),
    );
}

/// The part of the soul damage effects after the formula: `(int) damage * (1 + souls * 0.048)`
/// with the souls the player holds, at most the skill's `soulMaxConsumeCount`.
fn soul_boosted(world: &World, effector: ObjectId, skill: &SkillTemplate, damage: i32) -> i32 {
    if skill.soul_max_consume > 0 && world.is_player(effector) {
        // Souls Formula (each soul increase +4.8%).
        let charged = world.charged_souls_of(effector);
        let souls = if charged <= skill.soul_max_consume {
            charged
        } else {
            skill.soul_max_consume
        };
        // `damage *= 1 + (souls * 0.048)`: the compound assignment narrows the double to an int.
        return (f64::from(damage) * (1.0 + (f64::from(souls) * 0.048))) as i32;
    }
    damage
}

/// The hit itself the three skill damage effects end with: damage message, HP, the fake death
/// notification and the counter.
fn land_hit(world: &mut World, cx: &EffectCx<'_>, damage: i32, crit: bool) {
    world.send_damage_message(cx.effector, cx.effected, damage, false, crit, false);
    world.reduce_current_hp_by_skill(cx.effected, f64::from(damage), Some(cx.effector), cx.skill);
    world.notify_damage_received(cx.effected, f64::from(damage));
    // Check if damage should be reflected.
    world.calc_damage_reflected_for(cx.effector, cx.effected, cx.skill, crit);
}

/// Java `PhysicalDamage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhysicalDamage;

impl PhysicalDamage {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for PhysicalDamage {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        !world.calc_physical_skill_evasion_for(cx.effector, cx.effected, cx.skill)
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
        if (skill.fly_radius > 0 || skill.fly_type.is_some()) && world.is_movement_disabled(effector) {
            send_unsuitable_terms(world, effector, skill);
            return;
        }
        world.stand_up_from_fake_death_on_damage(effected);

        // M5-SHOT-01: `skill.isPhysical() && effector.isChargedShot(ShotType.SOULSHOTS)`.
        let ss = false;
        let shld = world.calc_shld_use_skill(effector, effected, skill, true);
        // Physical damage critical rate is only affected by STR.
        let mut crit = false;
        if skill.base_crit_rate > 0 {
            crit = world.calc_skill_crit_for(effector, effected, skill);
        }
        let damage = world.calc_skill_phys_dam_for(effector, effected, skill, shld, crit, ss) as i32;
        if damage > 0 {
            land_hit(world, cx, damage, crit);
        } else {
            world.send(
                effector,
                &SystemMessage::new(SystemMessageId::YourAttackHasFailed),
            );
        }
        if skill.is_suicide_attack {
            world.do_die(effector, Some(effector));
        }
    }
}

/// Java `PhysicalDamageHpLink`: the power grows as the target's HP falls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhysicalDamageHpLink;

impl PhysicalDamageHpLink {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for PhysicalDamageHpLink {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        !world.calc_physical_skill_evasion_for(cx.effector, cx.effected, cx.skill)
    }

    fn effect_type(&self) -> EffectType {
        EffectType::PhysicalAttackHpLink
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if world.is_alike_dead(effector) {
            return;
        }
        // NOTE(java-quirk): unlike `PhysicalDamage`, any skill of this effect needs a free mover
        // (no fly check), and it neither stands a fake dead target up nor handles suicide skills.
        if world.is_movement_disabled(effector) {
            send_unsuitable_terms(world, effector, skill);
            return;
        }
        let shld = world.calc_shld_use_skill(effector, effected, skill, true);
        // Physical damage critical rate is only affected by STR.
        let mut crit = false;
        if skill.base_crit_rate > 0 {
            crit = world.calc_skill_crit_for(effector, effected, skill);
        }
        // M5-SHOT-01: `skill.isPhysical() && effector.isChargedShot(ShotType.SOULSHOTS)`.
        let ss = false;
        let damage = world.calc_skill_phys_dam_for(effector, effected, skill, shld, crit, ss) as i32;
        if damage > 0 {
            land_hit(world, cx, damage, crit);
        } else {
            world.send(
                effector,
                &SystemMessage::new(SystemMessageId::YourAttackHasFailed),
            );
        }
    }
}

/// Java `PhysicalSoulDamage`: physical damage boosted by the souls the caster holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhysicalSoulDamage;

impl PhysicalSoulDamage {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for PhysicalSoulDamage {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        !world.calc_physical_skill_evasion_for(cx.effector, cx.effected, cx.skill)
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
        if (skill.fly_radius > 0 || skill.fly_type.is_some()) && world.is_movement_disabled(effector) {
            send_unsuitable_terms(world, effector, skill);
            return;
        }
        world.stand_up_from_fake_death_on_damage(effected);

        // M5-SHOT-01: `skill.isPhysical() && effector.isChargedShot(ShotType.SOULSHOTS)`.
        let ss = false;
        let shld = world.calc_shld_use_skill(effector, effected, skill, true);
        // Physical damage critical rate is only affected by STR.
        let mut crit = false;
        if skill.base_crit_rate > 0 {
            crit = world.calc_skill_crit_for(effector, effected, skill);
        }
        // NOTE(java-quirk): the formula gets `crit = false`; a critical hit doubles the damage
        // after the souls, not inside the formula.
        let mut damage = world.calc_skill_phys_dam_for(effector, effected, skill, shld, false, ss) as i32;
        damage = soul_boosted(world, effector, skill, damage);
        if crit {
            damage = damage.wrapping_mul(2);
        }
        if damage > 0 {
            land_hit(world, cx, damage, crit);
        } else {
            world.send(
                effector,
                &SystemMessage::new(SystemMessageId::YourAttackHasFailed),
            );
        }
        if skill.is_suicide_attack {
            world.do_die(effector, Some(effector));
        }
    }
}

/// Java `EnergyDamage`: the damage of the force skills; its formula is in the effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EnergyDamage {
    power: f64,
    critical_chance: i32,
    ignore_shield_defence: bool,
}

impl EnergyDamage {
    /// Java `EnergyDamage(attachCond, applyCond, set, params)`: `power` (0), `criticalChance` (0)
    /// and `ignoreShieldDefence` (false).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: params::double(template, "power", 0.0)?,
            critical_chance: params::int(template, "criticalChance", 0)?,
            ignore_shield_defence: params::boolean(template, "ignoreShieldDefence", false)?,
        }))
    }
}

impl EffectHandler for EnergyDamage {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        // TODO: Verify this on retail (Java's note).
        !world.calc_physical_skill_evasion_for(cx.effector, cx.effected, cx.skill)
    }

    fn effect_type(&self) -> EffectType {
        EffectType::PhysicalAttack
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (attacker, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if !world.is_player(attacker) {
            return;
        }
        let (mut attack, mut defence) = match (world.stats(attacker), world.stats(effected)) {
            (Some(a), Some(t)) => (a.p_atk(Some(effected)), t.p_def(Some(attacker))),
            _ => return,
        };
        if !self.ignore_shield_defence {
            let shield = world.calc_shld_use_skill(attacker, effected, skill, true);
            match shield {
                SHIELD_DEFENSE_SUCCEED => {
                    defence += world.stats(effected).map_or(0.0, |t| f64::from(t.shld_def()))
                }
                SHIELD_DEFENSE_PERFECT_BLOCK => defence = -1.0,
                _ => {}
            }
        }

        let mut damage = 1.0_f64;
        let mut critical = false;
        if defence != -1.0 {
            let (weapon_defence_trait, _) = world.attack_trait_inputs(attacker, effected);
            let damage_multiplier = crate::formulas::calc_weapon_trait_bonus(weapon_defence_trait)
                * world.calc_attribute_bonus_for_effect(attacker, effected, skill)
                * world.calc_general_trait_bonus(attacker, effected, skill.trait_type, true);
            // M5-SHOT-01: `skill.useSoulShot() && attacker.isChargedShot(ShotType.SOULSHOTS)`.
            let ss_boost = 1.0;
            let (bow, effected_is_player, str_bonus) = match (world.stats(attacker), world.stats(effected)) {
                (Some(a), Some(t)) => (
                    matches!(
                        a.active_weapon_item()
                            .and_then(|i| i.weapon())
                            .map(|w| w.weapon_type),
                        Some(l2_data::item::WeaponType::Bow | l2_data::item::WeaponType::CrossBow)
                    ),
                    t.is_player(),
                    a.bonus(BaseStat::Str),
                ),
                _ => return,
            };
            let weapon_type_boost = if bow { 70.0 } else { 77.0 };
            // The charge count should be the count before casting the skill, but it is reduced
            // before the effects run: add what the skill consumes.
            let energy_charges_boost =
                ((f64::from(world.charges_of(attacker) + skill.charge_consume) - 1.0) * 0.2) + 1.0;
            attack += self.power;
            attack *= ss_boost;
            attack *= energy_charges_boost;
            attack *= weapon_type_boost;
            if effected_is_player {
                defence *= world
                    .stats(effected)
                    .map_or(1.0, |t| t.calc(Stat::PvpPhysSkillDef, 1.0));
            }
            // M5-CFG: `PVP/PVE_ENERGY_SKILL_DEFENCE_MULTIPLIERS` (1.0) for a playable target.

            damage = attack / defence;
            damage *= damage_multiplier;
            if effected_is_player {
                let attacker_view = world.stats(attacker);
                damage *= attacker_view
                    .as_ref()
                    .map_or(1.0, |a| a.calc(Stat::PvpPhysSkillDmg, 1.0));
                damage = attacker_view
                    .as_ref()
                    .map_or(damage, |a| a.calc(Stat::PhysicalSkillPower, damage));
            }

            critical = (str_bonus * f64::from(self.critical_chance)) > (world.rng().next_double() * 100.0);
            if critical {
                damage *= 2.0;
                // Critical Boost: "True Damage" from your Passive 193 (Critical Power).
                let ctx = crate::stats::skills::calc_skill(skill);
                let crit_add = world.stats(attacker).map_or(0.0, |a| {
                    a.calc_stat(Stat::CriticalDamageAdd, 0.0, Some(effected), Some(&ctx))
                });
                damage += crit_add; // Absolute (applies to both mobs and players).
                // (optional retail) Additive resistance of the target.
                damage += world.stats(effected).map_or(0.0, |t| {
                    t.calc_stat(Stat::DefenceCriticalDamageAdd, 0.0, Some(effected), Some(&ctx))
                });
            }
            // M5-CFG: `PVP/PVE_ENERGY_SKILL_DAMAGE_MULTIPLIERS` (1.0) for a playable attacker.
        }

        if damage > 0.0 {
            world.send_damage_message(attacker, effected, damage as i32, false, critical, false);
            world.reduce_current_hp_by_skill(effected, damage, Some(attacker), skill);
            world.notify_damage_received(effected, damage);
            // Check if damage should be reflected.
            world.calc_damage_reflected_for(attacker, effected, skill, critical);
        }
    }
}
