//! Effect landing, skill mastery and abnormal time.
//!
//! Java: `mechanics/stats/Formulas.java` `calcEffectSuccess`, `calcSkillMastery`,
//! `calcEffectAbnormalTime`, `calcProbability`.

use super::Rnd;
use super::skill::java_clamp;

// ------------------------------------------------------------------ effect success

/// What `Formulas.calcEffectSuccess(attacker, target, skill)` reads once the target is known to
/// be able to receive continuous effects (not a door, siege flag or static object: the caller
/// answers those with `false` and no message).
#[derive(Debug, Clone, Copy)]
pub struct EffectSuccessInput {
    /// `skill.isDebuff()`.
    pub skill_is_debuff: bool,
    /// `target.calcStat(Stat.DEBUFF_IMMUNITY, 0, attacker, skill)`.
    pub debuff_immunity: f64,
    /// `skill.getActivateRate()`.
    pub activate_rate: i32,
    /// `target.getSTR()` ... `getWIT()` for `skill.getBasicProperty()`; `None` for
    /// `BaseStat.NONE`, which lands the effect without a roll.
    pub target_base_stat: Option<i32>,
    /// `skill.getMagicLevel()`.
    pub skill_magic_level: i32,
    /// `skill.getLvlBonusRate()`.
    pub skill_lvl_bonus_rate: i32,
    /// `target.getLevel()`.
    pub target_level: i32,
    /// `Formulas.calcAttributeBonus(attacker, target, skill)`.
    pub element_mod: f64,
    /// `Formulas.calcGeneralTraitBonus(attacker, target, skill.getTraitType(), false)`.
    pub trait_mod: f64,
    /// `target.calcStat(skill.isDebuff() ? Stat.DEBUFF_VULN : Stat.BUFF_VULN, 1, null, null)`.
    pub buff_debuff_vuln: f64,
    /// `skill.isMagic()`.
    pub skill_is_magic: bool,
    /// `attacker.getMAtk(null, null)`.
    pub attacker_m_atk: f64,
    /// `attacker.isChargedShot(ShotType.BLESSED_SPIRITSHOTS)`.
    pub attacker_blessed_spiritshot_charged: bool,
    /// `target.getMDef(null, null)`.
    pub target_m_def: f64,
    /// `skill.getMinChance()`.
    pub min_chance: i32,
    /// `skill.getMaxChance()`.
    pub max_chance: i32,
}

/// Java `Formulas.calcEffectSuccess`: `true` when the effect lands. On `false` the attacker is
/// told "C1 has resisted your S2".
///
/// No draw for a debuff the target is immune to, an activate rate of -1 or over 99, or no basic
/// property. Otherwise one `Rnd.get(100)` (also when the trait makes the rate 0).
pub fn calc_effect_success(input: &EffectSuccessInput, rng: &mut impl Rnd) -> bool {
    if input.skill_is_debuff && input.debuff_immunity > 0.0 {
        return false;
    }

    let activate_rate = input.activate_rate;
    let Some(target_base_stat) = input.target_base_stat else {
        return true;
    };
    if activate_rate == -1 || activate_rate > 99 {
        return true;
    }

    let mut magic_level = input.skill_magic_level;
    if magic_level <= -1 {
        magic_level = input.target_level.wrapping_add(3);
    }

    let base_mod = f64::from(
        magic_level
            .wrapping_sub(input.target_level)
            .wrapping_add(3)
            .wrapping_mul(input.skill_lvl_bonus_rate)
            .wrapping_add(activate_rate),
    ) + 30.0
        - f64::from(target_base_stat);
    let element_mod = input.element_mod;
    let trait_mod = input.trait_mod;
    // NOTE(java-quirk): `1 + (stat / 100)` where the stat starts at 1, so a creature without
    // modifiers has 1.01, not 1.
    let buff_debuff_mod = 1.0 + (input.buff_debuff_vuln / 100.0);
    let mut m_atk_mod = 1.0;

    if input.skill_is_magic {
        let m_atk = input.attacker_m_atk;
        let mut val = 0.0;
        if input.attacker_blessed_spiritshot_charged {
            val = m_atk * 3.0; // 3.0 is the blessed spiritshot multiplier.
        }

        val += m_atk;
        val = (val.sqrt() / input.target_m_def) * 11.0;
        m_atk_mod = val;
    }

    let rate = base_mod * element_mod * trait_mod * m_atk_mod * buff_debuff_mod;
    let final_rate = if trait_mod > 0.0 {
        java_clamp(rate, f64::from(input.min_chance), f64::from(input.max_chance))
    } else {
        0.0
    };

    final_rate > f64::from(rng.get_int(100))
}

/// Java `Formulas.calcProbability(baseChance, attacker, target, skill)`: the chance of the effects
/// that name a base chance (TargetCancel, Betray, Bluff, ...). One draw, `Rnd.get(100)`.
///
/// `skill_magic_level` is `skill.getMagicLevel()`, `target_int` `target.getINT()`; the two bonuses
/// are as in [`EffectSuccessInput`].
pub fn calc_probability(
    base_chance: f64,
    skill_magic_level: i32,
    target_level: i32,
    target_int: i32,
    attribute_bonus: f64,
    trait_bonus: f64,
    rng: &mut impl Rnd,
) -> bool {
    let chance = ((((f64::from(skill_magic_level) + base_chance) - f64::from(target_level)) + 30.0)
        - f64::from(target_int))
        * attribute_bonus
        * trait_bonus;
    f64::from(rng.get_int(100)) < chance
}

// ------------------------------------------------------------------ skill mastery

/// What `Formulas.calcSkillMastery(actor, skill)` reads.
#[derive(Debug, Clone, Copy)]
pub struct SkillMasteryInput {
    /// `actor.isPlayer()`.
    pub actor_is_player: bool,
    /// `skill.isStatic()`.
    pub skill_is_static: bool,
    /// `actor.getStat().calcStat(Stat.SKILL_MASTERY, 0, null, null)` (cast with `(int)` here).
    pub skill_mastery: f64,
    /// `BaseStat.STR.calcBonus(actor)`.
    pub str_bonus: f64,
    /// `BaseStat.INT.calcBonus(actor)`.
    pub int_bonus: f64,
    /// `actor.getStat().calcStat(Stat.SKILL_MASTERY_RATE, 1, null, null)`.
    pub skill_mastery_rate: f64,
    /// `ClassBalanceConfig.SKILL_MASTERY_CHANCE_MULTIPLIERS[actor class]` (1.0 for a non-player).
    pub class_balance_multiplier: f32,
}

/// Java `Formulas.calcSkillMastery`: `true` when the skill is mastered (double effect time, a
/// quick reuse). Only players, not for static skills, and only with the `SKILL_MASTERY` stat; then
/// one `Rnd.get(100)` against `chance`.
pub fn calc_skill_mastery(input: &SkillMasteryInput, rng: &mut impl Rnd) -> bool {
    // Non players are not affected by Skill Mastery.
    if !input.actor_is_player {
        return false;
    }

    // Static Skills are not affected by Skill Mastery.
    if input.skill_is_static {
        return false;
    }

    let val = input.skill_mastery as i32;
    if val == 0 {
        return false;
    }

    // NOTE(java-quirk): any other stat value than 1 and 4 leaves the chance at 0 but still
    // rolls the draw.
    let mut chance = match val {
        1 => input.str_bonus,
        4 => input.int_bonus,
        _ => 0.0,
    };

    chance *= input.skill_mastery_rate;
    f64::from(rng.get_int(100)) < (chance * f64::from(input.class_balance_multiplier))
}

// ------------------------------------------------------------------ abnormal time

/// The resistance factors of a debuff's duration.
#[derive(Debug, Clone, Copy)]
pub struct AbnormalTimeResists {
    /// `skill.getBasicProperty().calcBonus(target)` (1 for `BaseStat.NONE`).
    pub stat_mod: f64,
    /// `Formulas.calcGeneralTraitBonus(caster, target, skill.getTraitType(), false)`.
    pub res_mod: f64,
    /// `Formulas.calcLvlBonusMod(caster, target, skill)`.
    pub lvl_bonus_mod: f64,
    /// `Formulas.calcAttributeBonus(caster, target, skill)`.
    pub element_mod: f64,
}

/// What `Formulas.calcEffectAbnormalTime(caster, target, skill)` reads.
#[derive(Debug, Clone, Copy)]
pub struct AbnormalTimeInput {
    /// `skill.isPassive() || skill.isToggle()`: the time is -1.
    pub skill_is_passive_or_toggle: bool,
    /// `skill.getAbnormalTime()`.
    pub skill_abnormal_time: i32,
    /// `target != null && target.isServitor() && skill.isAbnormalInstant()`: the herb buff of a
    /// servitor lasts half.
    pub halve_for_servitor: bool,
    /// The caster's inputs of `calcSkillMastery`.
    pub mastery: SkillMasteryInput,
    /// `Some` when `caster != null && target != null && skill.isDebuff() &&
    /// GeneralConfig.DEBUFF_DURATION_USES_RESISTS`.
    pub resists: Option<AbnormalTimeResists>,
}

/// Java `Formulas.calcEffectAbnormalTime`: the seconds an effect lasts (-1 for passives and
/// toggles). Draws like [`calc_skill_mastery`].
pub fn calc_effect_abnormal_time(input: &AbnormalTimeInput, rng: &mut impl Rnd) -> i32 {
    let mut time = if input.skill_is_passive_or_toggle {
        -1
    } else {
        input.skill_abnormal_time
    };

    // An herb buff will affect both master and servitor, but the buff duration will be half of
    // the normal duration. If a servitor is not summoned, the master will receive the full buff
    // duration.
    if input.halve_for_servitor {
        time /= 2;
    }

    // If the skill is a mastery skill, the effect will last twice the default time.
    if calc_skill_mastery(&input.mastery, rng) {
        time = time.wrapping_mul(2);
    }

    // Debuffs Duration Affected by Resistances.
    if let Some(r) = &input.resists
        && time > 0
    {
        let t = f64::from(time);
        time = java_clamp(
            (t * r.res_mod * r.lvl_bonus_mod * r.element_mod) / r.stat_mod,
            t * 0.5,
            t,
        )
        .ceil() as i32;
    }

    time
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::rng::testing::Fixed;

    fn success() -> EffectSuccessInput {
        EffectSuccessInput {
            skill_is_debuff: true,
            debuff_immunity: 0.0,
            activate_rate: 40,
            target_base_stat: Some(40),
            skill_magic_level: 76,
            skill_lvl_bonus_rate: 2,
            target_level: 76,
            element_mod: 1.0,
            trait_mod: 1.0,
            buff_debuff_vuln: 0.0,
            skill_is_magic: false,
            attacker_m_atk: 0.0,
            attacker_blessed_spiritshot_charged: false,
            target_m_def: 1.0,
            min_chance: 10,
            max_chance: 90,
        }
    }

    #[test]
    fn effect_rate_from_levels_and_base_stat() {
        // ((76 - 76) + 3) * 2 + 40 + 30 - 40 = 36
        let i = success();
        assert!(calc_effect_success(&i, &mut Fixed::new(&[35])));
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[36])));
    }

    #[test]
    fn rate_is_clamped_between_min_and_max_chance() {
        let mut i = success();
        i.target_base_stat = Some(500);
        // Rate negative -> 10.
        assert!(calc_effect_success(&i, &mut Fixed::new(&[9])));
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[10])));
        i.target_base_stat = Some(0);
        i.activate_rate = 99;
        // 6 + 99 + 30 = 135 -> 90.
        assert!(calc_effect_success(&i, &mut Fixed::new(&[89])));
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[90])));
    }

    #[test]
    fn no_draw_cases() {
        let mut i = success();
        i.debuff_immunity = 1.0;
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[])));
        // Immunity only counts for debuffs.
        i.skill_is_debuff = false;
        assert!(calc_effect_success(&i, &mut Fixed::new(&[35])));
        let mut i = success();
        i.activate_rate = -1;
        assert!(calc_effect_success(&i, &mut Fixed::new(&[])));
        i.activate_rate = 100;
        assert!(calc_effect_success(&i, &mut Fixed::new(&[])));
        i.activate_rate = 40;
        i.target_base_stat = None;
        assert!(calc_effect_success(&i, &mut Fixed::new(&[])));
    }

    #[test]
    fn invulnerable_to_the_trait_still_draws_and_never_lands() {
        let mut i = success();
        i.trait_mod = 0.0;
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[0])));
    }

    #[test]
    fn magic_skills_use_the_attackers_m_atk_over_the_targets_m_def() {
        let mut i = success();
        i.skill_is_magic = true;
        i.attacker_m_atk = 400.0;
        i.target_m_def = 200.0;
        // sqrt(400) / 200 * 11 = 1.1 -> 36 * 1.1 = 39.6000..
        let rate = 36.0 * ((400.0_f64.sqrt() / 200.0) * 11.0);
        assert!(calc_effect_success(&i, &mut Fixed::new(&[39])));
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[40])));
        assert!(rate > 39.0 && rate < 40.0);
        // A blessed spiritshot makes val = 3 * 400 + 400 = 1600 -> sqrt 40 / 200 * 11 = 2.2.
        i.attacker_blessed_spiritshot_charged = true;
        assert!(calc_effect_success(&i, &mut Fixed::new(&[78])));
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[80])));
    }

    #[test]
    fn negative_magic_level_uses_the_target_level_plus_three() {
        let mut i = success();
        i.skill_magic_level = -1;
        // (76 + 3 - 76 + 3) * 2 = 12 -> 12 + 40 + 30 - 40 = 42
        assert!(calc_effect_success(&i, &mut Fixed::new(&[41])));
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[42])));
    }

    #[test]
    fn debuff_vulnerability_has_a_one_percent_base() {
        let mut i = success();
        i.buff_debuff_vuln = 100.0;
        // 36 * 2 = 72
        assert!(calc_effect_success(&i, &mut Fixed::new(&[71])));
        assert!(!calc_effect_success(&i, &mut Fixed::new(&[72])));
    }

    #[test]
    fn probability() {
        // ((74 + 30) - 76 + 30) - 21 = 37
        assert!(calc_probability(
            30.0,
            74,
            76,
            21,
            1.0,
            1.0,
            &mut Fixed::new(&[36])
        ));
        assert!(!calc_probability(
            30.0,
            74,
            76,
            21,
            1.0,
            1.0,
            &mut Fixed::new(&[37])
        ));
        assert!(!calc_probability(
            30.0,
            74,
            76,
            21,
            1.0,
            0.0,
            &mut Fixed::new(&[0])
        ));
    }

    fn mastery() -> SkillMasteryInput {
        SkillMasteryInput {
            actor_is_player: true,
            skill_is_static: false,
            skill_mastery: 1.0,
            str_bonus: 1.5,
            int_bonus: 0.5,
            skill_mastery_rate: 2.0,
            class_balance_multiplier: 1.0,
        }
    }

    #[test]
    fn mastery_chance_is_the_stat_bonus_times_the_rate() {
        // STR bonus 1.5 * 2 = 3 (percent).
        assert!(calc_skill_mastery(&mastery(), &mut Fixed::new(&[2])));
        assert!(!calc_skill_mastery(&mastery(), &mut Fixed::new(&[3])));
        let int = SkillMasteryInput {
            skill_mastery: 4.0,
            ..mastery()
        };
        assert!(calc_skill_mastery(&int, &mut Fixed::new(&[0])));
        assert!(!calc_skill_mastery(&int, &mut Fixed::new(&[1])));
        let balanced = SkillMasteryInput {
            class_balance_multiplier: 2.0,
            ..mastery()
        };
        assert!(calc_skill_mastery(&balanced, &mut Fixed::new(&[5])));
        assert!(!calc_skill_mastery(&balanced, &mut Fixed::new(&[6])));
    }

    #[test]
    fn mastery_without_a_draw() {
        let none = |m: SkillMasteryInput| calc_skill_mastery(&m, &mut Fixed::new(&[]));
        assert!(!none(SkillMasteryInput {
            actor_is_player: false,
            ..mastery()
        }));
        assert!(!none(SkillMasteryInput {
            skill_is_static: true,
            ..mastery()
        }));
        assert!(!none(SkillMasteryInput {
            skill_mastery: 0.0,
            ..mastery()
        }));
        // A mastery stat of 2 or 3 rolls against a chance of 0.
        assert!(!calc_skill_mastery(
            &SkillMasteryInput {
                skill_mastery: 2.0,
                ..mastery()
            },
            &mut Fixed::new(&[0])
        ));
    }

    fn abnormal() -> AbnormalTimeInput {
        AbnormalTimeInput {
            skill_is_passive_or_toggle: false,
            skill_abnormal_time: 20,
            halve_for_servitor: false,
            mastery: SkillMasteryInput {
                skill_mastery: 0.0,
                ..mastery()
            },
            resists: None,
        }
    }

    #[test]
    fn abnormal_time_basics() {
        let mut rng = Fixed::new(&[]);
        assert_eq!(calc_effect_abnormal_time(&abnormal(), &mut rng), 20);
        let toggle = AbnormalTimeInput {
            skill_is_passive_or_toggle: true,
            ..abnormal()
        };
        assert_eq!(calc_effect_abnormal_time(&toggle, &mut rng), -1);
        let herb = AbnormalTimeInput {
            halve_for_servitor: true,
            skill_abnormal_time: 21,
            ..abnormal()
        };
        assert_eq!(calc_effect_abnormal_time(&herb, &mut rng), 10);
    }

    #[test]
    fn mastery_doubles_the_time() {
        let m = AbnormalTimeInput {
            mastery: mastery(),
            ..abnormal()
        };
        assert_eq!(calc_effect_abnormal_time(&m, &mut Fixed::new(&[0])), 40);
        assert_eq!(calc_effect_abnormal_time(&m, &mut Fixed::new(&[99])), 20);
    }

    #[test]
    fn resistances_shorten_a_debuff_down_to_half() {
        let r = |res_mod: f64| AbnormalTimeInput {
            resists: Some(AbnormalTimeResists {
                stat_mod: 1.0,
                res_mod,
                lvl_bonus_mod: 1.0,
                element_mod: 1.0,
            }),
            ..abnormal()
        };
        let mut rng = Fixed::new(&[]);
        assert_eq!(calc_effect_abnormal_time(&r(0.75), &mut rng), 15);
        // Not below half, never above the base time, rounded up.
        assert_eq!(calc_effect_abnormal_time(&r(0.1), &mut rng), 10);
        assert_eq!(calc_effect_abnormal_time(&r(2.0), &mut rng), 20);
        assert_eq!(calc_effect_abnormal_time(&r(0.76), &mut rng), 16);
        // A time of 0 or less is left alone.
        let zero = AbnormalTimeInput {
            skill_abnormal_time: 0,
            ..r(0.5)
        };
        assert_eq!(calc_effect_abnormal_time(&zero, &mut rng), 0);
    }
}
