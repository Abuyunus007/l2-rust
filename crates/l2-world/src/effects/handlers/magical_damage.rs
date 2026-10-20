//! The magical damage effects: `MagicalDamage`, `MagicalDamageByAbnormal`, `MagicalSoulDamage`,
//! `DeathLink`, `HpDrain` and `MagicalDamageMp`.
// Java: dist/game/data/scripts/handlers/skill/effects/MagicalDamage.java
// Java: dist/game/data/scripts/handlers/skill/effects/MagicalDamageByAbnormal.java
// Java: dist/game/data/scripts/handlers/skill/effects/MagicalSoulDamage.java
// Java: dist/game/data/scripts/handlers/skill/effects/DeathLink.java
// Java: dist/game/data/scripts/handlers/skill/effects/HpDrain.java
// Java: dist/game/data/scripts/handlers/skill/effects/MagicalDamageMp.java
//
// The damage numbers come from `skill_calc.rs` (M5-FORM-02's formulas). The resist messages the
// formulas send in Java are sent there too. M5-SHOT-01: every spiritshot charge is read as "not
// charged".

use l2_data::Stat;
use l2_data::skill::EffectTemplate;
use l2_data::stats::AbnormalType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};

use super::params;
use super::skill_calc::MagicFlags;
use crate::effects::registry::{EffectCx, EffectHandler, EffectType};
use crate::formulas::Rnd;
use crate::stats::skills::calc_skill;
use crate::world::World;

/// What the magical damage effects do once `damage > 0`: the attack break, the shield deflect
/// (the damage turns on the caster), the HP and the message.
fn land_magic_hit(world: &mut World, cx: &EffectCx<'_>, damage: i32, mcrit: bool) {
    let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
    // Manage attack or cast break of the target (calculating rate, sending message...)
    world.skill_atk_break(effected, f64::from(damage));

    // Shield Deflect Magic: Reflect all damage on caster.
    let ctx = calc_skill(skill);
    let deflect = world.stats(effected).map_or(0.0, |t| {
        t.calc_stat(Stat::VengeanceSkillMagicDamage, 0.0, Some(effected), Some(&ctx))
    });
    if deflect > f64::from(world.rng().get_int(100)) {
        world.reduce_current_hp_by_skill(effector, f64::from(damage), Some(effected), skill);
        world.notify_damage_received(effector, f64::from(damage));
    } else {
        world.reduce_current_hp_by_skill(effected, f64::from(damage), Some(effector), skill);
        world.notify_damage_received(effected, f64::from(damage));
        world.send_damage_message(effector, effected, damage, mcrit, false, false);
    }
}

/// The rolls every magical damage effect starts with, in Java's order: the magic critical hit,
/// the shield, then the damage (as an `int`) and whether it crit.
fn roll_magic_damage(world: &mut World, cx: &EffectCx<'_>) -> (i32, bool) {
    let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
    // M5-SHOT-01: `skill.useSpiritShot() && effector.isChargedShot(ShotType.SPIRITSHOTS)` and
    // `BLESSED_SPIRITSHOTS`.
    let (sps, bss) = (false, false);
    let mcrit = world.calc_mcrit_for(effector, effected, skill);
    let shld = world.calc_shld_use_skill(effector, effected, skill, true);
    let flags = MagicFlags { sps, bss, mcrit };
    let damage = world.calc_magic_dam_for(effector, effected, skill, shld, flags) as i32;
    (damage, mcrit)
}

/// `damage *= 1 + (chargedSouls * 0.048)` for `MagicalSoulDamage` (`int` compound assignment).
fn soul_boosted(world: &World, cx: &EffectCx<'_>, damage: i32) -> i32 {
    if cx.skill.soul_max_consume > 0 && world.is_player(cx.effector) {
        // Souls Formula (each soul increase +4.8%).
        let souls = world.charged_souls_of(cx.effector).min(cx.skill.soul_max_consume);
        return (f64::from(damage) * (1.0 + (f64::from(souls) * 0.048))) as i32;
    }
    damage
}

/// Java `MagicalDamage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MagicalDamage;

impl MagicalDamage {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for MagicalDamage {
    fn effect_type(&self) -> EffectType {
        EffectType::MagicalAttack
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_alike_dead(cx.effector) {
            return;
        }
        world.stand_up_from_fake_death_on_damage(cx.effected);
        let (damage, mcrit) = roll_magic_damage(world, cx);
        if damage > 0 {
            land_magic_hit(world, cx, damage, mcrit);
        }
        if cx.skill.is_suicide_attack {
            world.do_die(cx.effector, Some(cx.effector));
        }
    }
}

/// Java `MagicalDamageByAbnormal`: the damage grows with the buffs the target holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MagicalDamageByAbnormal;

impl MagicalDamageByAbnormal {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for MagicalDamageByAbnormal {
    fn effect_type(&self) -> EffectType {
        EffectType::MagicalAttack
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_alike_dead(cx.effector) {
            return;
        }
        world.stand_up_from_fake_death_on_damage(cx.effected);
        let (mut damage, mcrit) = roll_magic_damage(world, cx);
        // Each buff increase +30%.
        let buffs = world.effect_list(cx.effected).map_or(0, |l| l.buff_count());
        damage = (f64::from(damage) * (((f64::from(buffs) * 0.3) + 1.3) / 4.0)) as i32;
        if damage > 0 {
            land_magic_hit(world, cx, damage, mcrit);
        }
    }
}

/// Java `MagicalSoulDamage`: magical damage boosted by the souls the caster holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MagicalSoulDamage;

impl MagicalSoulDamage {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for MagicalSoulDamage {
    fn effect_type(&self) -> EffectType {
        EffectType::MagicalAttack
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_alike_dead(cx.effector) {
            return;
        }
        world.stand_up_from_fake_death_on_damage(cx.effected);
        let (damage, mcrit) = roll_magic_damage(world, cx);
        let damage = soul_boosted(world, cx, damage);
        if damage > 0 {
            land_magic_hit(world, cx, damage, mcrit);
        }
    }
}

/// Java `DeathLink`: magical damage that grows as the caster's HP falls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeathLink;

impl DeathLink {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for DeathLink {
    fn effect_type(&self) -> EffectType {
        EffectType::DeathLink
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if world.is_alike_dead(cx.effector) {
            return;
        }
        world.stand_up_from_fake_death_on_damage(cx.effected);
        let (damage, mcrit) = roll_magic_damage(world, cx);
        if damage > 0 {
            land_magic_hit(world, cx, damage, mcrit);
        }
        if cx.skill.is_suicide_attack {
            world.do_die(cx.effector, Some(cx.effector));
        }
    }
}

/// Java `HpDrain`: magical damage that heals the caster by `power` per HP taken.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HpDrain {
    power: f64,
}

impl HpDrain {
    /// Java `HpDrain(attachCond, applyCond, set, params)`: `power` (default 0).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            power: params::double(template, "power", 0.0)?,
        }))
    }
}

impl EffectHandler for HpDrain {
    fn effect_type(&self) -> EffectType {
        EffectType::HpDrain
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if world.is_alike_dead(effector)
            || world.is_affected_by_abnormal_type(effector, AbnormalType::Invincibility)
            || world.is_affected_by_abnormal_type(effector, AbnormalType::AbnormalInvincibility)
        {
            return;
        }
        // NOTE(java-quirk): no fake death stand-up here, unlike the other magical damage effects.
        let (damage, mcrit) = roll_magic_damage(world, cx);
        let cp = world.core_of(effected).map_or(0, |c| c.cur_cp as i32);
        let hp = world.core_of(effected).map_or(0, |c| c.cur_hp as i32);
        let drain = if cp > 0 {
            if damage < cp { 0 } else { damage - cp }
        } else if damage > hp {
            hp
        } else {
            damage
        };

        let hp_add = self.power * f64::from(drain);
        let current = world.core_of(effector).map_or(0.0, |c| c.cur_hp);
        let max_hp = world.stats(effector).map_or(0.0, |v| f64::from(v.max_hp()));
        let hp_final = if current + hp_add > max_hp {
            max_hp
        } else {
            current + hp_add
        };
        world.set_current_hp(effector, hp_final, true);

        if damage > 0 {
            // Manage attack or cast break of the target (calculating rate, sending message...)
            world.skill_atk_break(effected, f64::from(damage));
            world.send_damage_message(effector, effected, damage, mcrit, false, false);
            world.reduce_current_hp_by_skill(effected, f64::from(damage), Some(effector), skill);
            world.notify_damage_received(effected, f64::from(damage));
        }
    }
}

/// Java `MagicalDamageMp`: damage to the target's MP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MagicalDamageMp;

impl MagicalDamageMp {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self))
    }
}

impl EffectHandler for MagicalDamageMp {
    fn calc_success(&self, world: &mut World, cx: &EffectCx<'_>) -> bool {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if world.is_invul(effected) {
            return false;
        }
        if !world.calc_magic_affected_for(effector, effected, skill) {
            if world.is_player(effector) {
                world.send(
                    effector,
                    &SystemMessage::new(SystemMessageId::YourAttackHasFailed),
                );
            }
            if world.is_player(effected) {
                world.send(
                    effected,
                    &SystemMessage::new(SystemMessageId::C1ResistedC2SMagic)
                        .with(SystemMessageParam::Text(world.creature_name(effected)))
                        .with(SystemMessageParam::Text(world.creature_name(effector))),
                );
            }
            return false;
        }
        true
    }

    fn effect_type(&self) -> EffectType {
        EffectType::MagicalAttack
    }

    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let (effector, effected, skill) = (cx.effector, cx.effected, cx.skill);
        if world.is_alike_dead(effector) {
            return;
        }
        // M5-SHOT-01: the spiritshot charges.
        let (sps, bss) = (false, false);
        // NOTE: the shield is rolled before the magic critical hit here (the other magical
        // effects roll it the other way round).
        let shld = world.calc_shld_use_skill(effector, effected, skill, true);
        let mcrit = world.calc_mcrit_for(effector, effected, skill);
        let flags = MagicFlags { sps, bss, mcrit };
        let damage = world.calc_mana_dam_for(effector, effected, skill, shld, flags);
        let current_mp = world.core_of(effected).map_or(0.0, |c| c.cur_mp);
        let mp = if damage > current_mp { current_mp } else { damage };
        if damage > 0.0 {
            world.stop_effects_on_damage(effected, true);
            let now = world.core_of(effected).map_or(0.0, |c| c.cur_mp);
            world.set_current_mp(effected, now - mp, true);
        }

        if world.is_player(effected) {
            world.send(
                effected,
                &SystemMessage::new(SystemMessageId::S2SMpHasBeenDrainedByC1)
                    .with(SystemMessageParam::Text(world.creature_name(effector)))
                    .with(SystemMessageParam::IntNumber(mp as i32)),
            );
        }
        if world.is_player(effector) {
            world.send(
                effector,
                &SystemMessage::new(SystemMessageId::YourOpponentSMpWasReducedByS1)
                    .with(SystemMessageParam::IntNumber(mp as i32)),
            );
        }
    }
}
