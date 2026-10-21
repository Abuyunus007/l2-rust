//! The two formulas of cubics: `calcMagicDam(Cubic, ..)` and `calcCubicSkillSuccess`.
// Java: mechanics/stats/Formulas.java (calcMagicDam (Cubic overload), calcCubicSkillSuccess)
//
// The numbers are pure functions with their inputs in a struct (the pattern of `crate::formulas`);
// the `World` methods below fill the inputs from the creatures, draw in Java's order from the
// world RNG and send the messages Java sends from inside the formulas.

use l2_data::SkillTemplate;
use l2_data::Stat;
use l2_data::skill::BaseStat as SkillBaseStat;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::effects::buff_info::skill_name_param;
use crate::effects::registry::EffectType;
use crate::formulas::skill::{java_clamp, npc_level_penalty};
use crate::formulas::{
    MagicDamResult, MagicFailure, MagicSuccessInput, Rnd, SHIELD_DEFENSE_PERFECT_BLOCK,
    SHIELD_DEFENSE_SUCCEED, SkillPower, calc_lvl_bonus_mod, calc_magic_success,
};
use crate::stats::skills::calc_skill;
use crate::stats::{BaseStat, ClassBalance};
use crate::world::World;

// ------------------------------------------------------------------ calcMagicDam(Cubic, ..)

/// What `Formulas.calcMagicDam(Cubic attacker, Creature target, Skill skill, boolean mcrit,
/// byte shld)` reads.
#[derive(Debug, Clone, Copy)]
pub struct CubicMagicDamInput<'a> {
    /// `target.getMDef(attacker.getOwner(), skill)`.
    pub m_def: f64,
    /// `target.getShldDef()`.
    pub shld_def: i32,
    /// `attacker.getCubicPower()`.
    pub cubic_power: i32,
    /// `skill.getPower(isPvP, isPvE)` (no HP link) with `isPvP = target.isPlayable()` and
    /// `isPvE = target.isAttackable()`: the three powers of the skill.
    pub skill_power: SkillPower,
    /// `target.isPlayable()`.
    pub target_is_playable: bool,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// `PlayerConfig.ALT_GAME_MAGICFAILURES`.
    pub alt_game_magicfailures: bool,
    /// The inputs of the `calcMagicSuccess` calls (the owner attacks).
    pub magic_success: MagicSuccessInput<'a>,
    /// `skill.getMagicLevel()`.
    pub skill_magic_level: i32,
    /// `Formulas.calcAttributeBonus(owner, target, skill)`.
    pub attribute_bonus: f64,
    /// `owner.calcStat(Stat.PVE_MAGICAL_DMG, 1, null, null)`.
    pub pve_magical_dmg: f64,
    /// `ClassBalanceConfig.PVE_MAGICAL_SKILL_DAMAGE_MULTIPLIERS[owner class]`.
    pub pve_magical_skill_damage: f32,
    /// `owner.getLevel()`.
    pub owner_level: i32,
    /// `target.getLevel()`.
    pub target_level: i32,
    pub target_is_raid: bool,
    pub target_is_raid_minion: bool,
    /// `NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY`.
    pub min_npc_level_dmg_penalty: i32,
    /// `NpcConfig.NPC_SKILL_DMG_PENALTY`.
    pub npc_skill_dmg_penalty: &'a [f32],
}

/// Java `Formulas.calcMagicDam(Cubic attacker, Creature target, Skill skill, boolean mcrit, byte
/// shld)`.
///
/// A perfect block returns 1 before any draw. With `alt_game_magicfailures` the first draw is the
/// `Rnd.get(100)` of `calcMagicSuccess`; a failed check draws it a second time (see the quirks).
// NOTE(java-quirk): the damage is `91 * ((cubicPower + skillPower) / mDef)`: the cubic's power is
// added to the skill's and not rooted like a creature's M. Atk (`91 * sqrt(mAtk) / mDef * power`).
// There is no weapon random damage, no PvP bonus and no class balance but the PvE damage one, and
// a critical hit is a plain `* 3`.
pub fn calc_cubic_magic_dam(
    input: &CubicMagicDamInput<'_>,
    shld: u8,
    mcrit: bool,
    rng: &mut impl Rnd,
) -> MagicDamResult {
    let mut m_def = input.m_def;
    match shld {
        SHIELD_DEFENSE_SUCCEED => m_def += f64::from(input.shld_def),
        SHIELD_DEFENSE_PERFECT_BLOCK => {
            return MagicDamResult {
                damage: 1.0,
                failed: false,
                failure: MagicFailure::None,
            };
        }
        _ => {}
    }

    // Cubics MDAM Formula (similar to PDAM formula, but using 91 instead of 70, also resisted by
    // mDef).
    let m_atk = f64::from(input.cubic_power);
    let power = input
        .skill_power
        .get(input.target_is_playable, input.target_is_attackable);
    let mut damage = 91.0 * ((m_atk + power) / m_def);

    // Failure calculation.
    let mut failed = false;
    let mut failure = MagicFailure::None;
    if input.alt_game_magicfailures && !calc_magic_success(&input.magic_success, rng) {
        failed = true;
        // NOTE(java-quirk): the check is rolled a second time to decide between "half damage" and
        // "fully resisted", and the level gap is the target's level minus the skill's magic
        // level (a creature's formula uses the attacker's level).
        if calc_magic_success(&input.magic_success, rng)
            && input.target_level.wrapping_sub(input.skill_magic_level) <= 9
        {
            failure = MagicFailure::Half;
            damage /= 2.0;
        } else {
            failure = MagicFailure::Resisted;
            damage = 1.0;
        }
    } else if mcrit {
        damage *= 3.0;
    }

    damage *= input.attribute_bonus;

    if input.target_is_attackable {
        damage *= input.pve_magical_dmg;
        damage *= f64::from(input.pve_magical_skill_damage);
        damage *= npc_level_penalty(
            input.npc_skill_dmg_penalty,
            input.target_level,
            Some(input.owner_level),
            input.min_npc_level_dmg_penalty,
            2,
            input.target_is_raid,
            input.target_is_raid_minion,
        );
    }

    MagicDamResult {
        damage,
        failed,
        failure,
    }
}

// ------------------------------------------------------------------ calcCubicSkillSuccess

/// The checks of `Formulas.calcCubicSkillSuccess` before the reflection roll: `Some(answer)` when
/// they decide, `None` when the reflection comes next.
pub fn cubic_success_before_reflection(
    skill_is_debuff: bool,
    skill_power: f32,
    debuff_immunity: f64,
    shld: u8,
) -> Option<bool> {
    if skill_is_debuff {
        if skill_power == -1.0 {
            return Some(true);
        } else if debuff_immunity > 0.0 {
            return Some(false);
        }
    }
    // Perfect Shield Block.
    if shld == SHIELD_DEFENSE_PERFECT_BLOCK {
        return Some(false);
    }
    None
}

/// What `Formulas.calcCubicSkillSuccess` reads for the landing rate (after the reflection).
#[derive(Debug, Clone, Copy)]
pub struct CubicSuccessInput {
    /// `skill.getActivateRate()`.
    pub activate_rate: i32,
    /// `skill.getBasicProperty() == BaseStat.NONE`.
    pub basic_property_none: bool,
    /// `skill.getPower()`.
    pub skill_power: f32,
    /// `skill.getBasicProperty().calcBonus(target)`.
    pub stat_mod: f64,
    /// `Formulas.calcGeneralTraitBonus(owner, target, skill.getTraitType(), false)`.
    pub res_mod: f64,
    /// `Formulas.calcLvlBonusMod(owner, target, skill)`.
    pub lvl_bonus_mod: f64,
    /// `Formulas.calcAttributeBonus(owner, target, skill)`.
    pub element_mod: f64,
    /// `skill.getMinChance()`.
    pub min_chance: i32,
    /// `skill.getMaxChance()`.
    pub max_chance: i32,
}

/// The rest of `Formulas.calcCubicSkillSuccess` after the reflection did not turn the skill
/// around: `true` without a draw for a skill that always lands, else `Rnd.get(100) < rate`.
pub fn calc_cubic_success_rate(input: &CubicSuccessInput, rng: &mut impl Rnd) -> bool {
    let activate_rate = input.activate_rate;
    if activate_rate == -1 || activate_rate > 99 || input.basic_property_none {
        return true;
    }

    // Calculate BaseRate.
    let base_rate = if activate_rate > 0 {
        f64::from(activate_rate)
    } else {
        f64::from(input.skill_power)
    };
    let mut rate = base_rate / input.stat_mod;
    // Resist Modifier, Lvl Bonus Modifier, Element Modifier.
    rate *= input.res_mod;
    rate *= input.lvl_bonus_mod;
    rate *= input.element_mod;
    // Add Matk/Mdef Bonus (TODO: Pending)

    // Check the Rate Limits.
    let final_rate = java_clamp(rate, f64::from(input.min_chance), f64::from(input.max_chance));
    f64::from(rng.get_int(100)) < final_rate
}

// ------------------------------------------------------------------ the world's side

/// `BaseStat.calcBonus(target)` for the stat a skill names (`BaseStat.NONE` never gets here).
fn basic_property_bonus(target: &crate::stats::StatView<'_>, property: SkillBaseStat) -> f64 {
    match property {
        SkillBaseStat::Str => target.bonus(BaseStat::Str),
        SkillBaseStat::Dex => target.bonus(BaseStat::Dex),
        SkillBaseStat::Con => target.bonus(BaseStat::Con),
        SkillBaseStat::Int => target.bonus(BaseStat::Int),
        SkillBaseStat::Men => target.bonus(BaseStat::Men),
        SkillBaseStat::Wit => target.bonus(BaseStat::Wit),
        SkillBaseStat::None => 1.0,
    }
}

impl World {
    /// Java `Formulas.calcMagicDam(Cubic attacker, target, skill, mcrit, shld)` with the resist
    /// messages it sends. The damage as a double (the caller casts it with `(int)`).
    pub(crate) fn calc_cubic_magic_dam(
        &mut self,
        owner: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        cubic_power: i32,
        mcrit: bool,
        shld: u8,
    ) -> f64 {
        let is_drain = self.skill_has_effect_type(skill, &[EffectType::HpDrain]);
        let ctx = calc_skill(skill);
        let result = self.with_rng(|world, rng| {
            let (Some(a), Some(t)) = (world.stats(owner), world.stats(target)) else {
                return None;
            };
            let combat = &world.settings.combat;
            let cb = &world.settings.stats.class_balance;
            let raid_minion = t
                .npc()
                .and_then(|n| n.leader)
                .is_some_and(|leader| world.is_raid(leader));
            let input = CubicMagicDamInput {
                m_def: t.m_def(Some(owner), Some(&ctx)),
                shld_def: t.shld_def(),
                cubic_power,
                skill_power: SkillPower {
                    power: f64::from(skill.power),
                    pvp_power: f64::from(skill.pvp_power),
                    pve_power: f64::from(skill.pve_power),
                    hp_link: None,
                },
                target_is_playable: t.is_playable(),
                target_is_attackable: t.is_attackable(),
                alt_game_magicfailures: combat.alt_game_magicfailures,
                magic_success: Self::magic_success_input(world, &a, &t, skill),
                skill_magic_level: skill.magic_level,
                attribute_bonus: world.calc_attribute_bonus_for_effect(owner, target, skill),
                pve_magical_dmg: a.calc(Stat::PveMagicalDmg, 1.0),
                pve_magical_skill_damage: ClassBalance::of(
                    &cb.pve_magical_skill_damage,
                    a.player().map_or(-1, |p| p.data.class_id),
                ),
                owner_level: a.level(),
                target_level: t.level(),
                target_is_raid: t.is_raid(),
                target_is_raid_minion: raid_minion,
                min_npc_level_dmg_penalty: combat.min_npc_level_dmg_penalty,
                npc_skill_dmg_penalty: &combat.npc_skill_dmg_penalty,
            };
            Some(calc_cubic_magic_dam(&input, shld, mcrit, rng))
        });
        let Some(MagicDamResult {
            damage,
            failed,
            failure,
        }) = result
        else {
            return 0.0;
        };
        // The messages Java sends from inside the formula, in its order.
        if failed {
            match failure {
                MagicFailure::Half => {
                    let id = if is_drain {
                        SystemMessageId::DrainWasOnly50PercentSuccessful
                    } else {
                        SystemMessageId::YourAttackHasFailed
                    };
                    self.send(owner, &SystemMessage::new(id));
                }
                MagicFailure::Resisted => {
                    self.send(
                        owner,
                        &SystemMessage::new(SystemMessageId::C1HasResistedYourS2)
                            .with(SystemMessageParam::Text(self.creature_name(target)))
                            .with(skill_name_param(skill)),
                    );
                }
                MagicFailure::None => {}
            }
            if self.is_player(target) {
                let id = if is_drain {
                    SystemMessageId::YouResistedC1SDrain
                } else {
                    SystemMessageId::YouResistedC1SMagic
                };
                self.send(
                    target,
                    &SystemMessage::new(id).with(SystemMessageParam::Text(self.creature_name(owner))),
                );
            }
        }
        damage
    }

    /// Java `Formulas.calcCubicSkillSuccess(Cubic attacker, target, skill, shld)`: whether the
    /// effects of a cubic's skill land. The reflection of a debuff is rolled before the rate.
    pub(crate) fn calc_cubic_skill_success(
        &mut self,
        owner: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        shld: u8,
    ) -> bool {
        let ctx = calc_skill(skill);
        let immunity = self
            .stats(target)
            .map_or(0.0, |t| t.calc_stat(Stat::DebuffImmunity, 0.0, None, Some(&ctx)));
        if let Some(answer) = cubic_success_before_reflection(skill.is_debuff, skill.power, immunity, shld) {
            return answer;
        }
        // If target reflect this skill then the effect will fail.
        if self.calc_buff_debuff_reflection(target, skill) {
            return false;
        }
        let (stat_mod, lvl_bonus_mod) = {
            let (Some(a), Some(t)) = (self.stats(owner), self.stats(target)) else {
                return false;
            };
            let stat_mod = if skill.basic_property == SkillBaseStat::None {
                1.0
            } else {
                basic_property_bonus(&t, skill.basic_property)
            };
            (
                stat_mod,
                calc_lvl_bonus_mod(skill.magic_level, skill.lvl_bonus_rate, a.level(), t.level()),
            )
        };
        let input = CubicSuccessInput {
            activate_rate: skill.activate_rate,
            basic_property_none: skill.basic_property == SkillBaseStat::None,
            skill_power: skill.power,
            stat_mod,
            res_mod: self.calc_general_trait_bonus(owner, target, skill.trait_type, false),
            lvl_bonus_mod,
            element_mod: self.calc_attribute_bonus_for_effect(owner, target, skill),
            min_chance: skill.min_chance,
            max_chance: skill.max_chance,
        };
        self.with_rng(|_, rng| calc_cubic_success_rate(&input, rng))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::rng::testing::Fixed as Scripted;

    /// A monster of level 40 hit by a level 40 owner with the shipped defaults of
    /// `NpcConfig` / `PlayerConfig` (no penalty table, no failures).
    fn input<'a>(penalty: &'a [f32]) -> CubicMagicDamInput<'a> {
        CubicMagicDamInput {
            m_def: 500.0,
            shld_def: 200,
            cubic_power: 1000,
            skill_power: SkillPower {
                power: 1500.0,
                pvp_power: 1500.0,
                pve_power: 1500.0,
                hp_link: None,
            },
            target_is_playable: false,
            target_is_attackable: true,
            alt_game_magicfailures: false,
            magic_success: MagicSuccessInput {
                skill_power: 1500.0,
                skill_activate_rate: -1,
                calculate_magic_success_by_skill_magic_level: false,
                skill_magic_level: 40,
                attacker_level: 40,
                target_level: 40,
                target_is_attackable: true,
                target_is_raid: false,
                target_is_raid_minion: false,
                min_npc_level_magic_penalty: 78,
                attacker_player_level: Some(40),
                npc_skill_chance_penalty: &[],
                magic_success_res: 1.0,
            },
            skill_magic_level: 40,
            attribute_bonus: 1.0,
            pve_magical_dmg: 1.0,
            pve_magical_skill_damage: 1.0,
            owner_level: 40,
            target_level: 40,
            target_is_raid: false,
            target_is_raid_minion: false,
            min_npc_level_dmg_penalty: 78,
            npc_skill_dmg_penalty: penalty,
        }
    }

    #[test]
    fn the_damage_adds_the_cubic_power_to_the_skill_power() {
        // 91 * ((1000 + 1500) / 500) = 455.
        let mut rng = Scripted::new(&[]);
        let result = calc_cubic_magic_dam(&input(&[]), 0, false, &mut rng);
        assert_eq!(result.damage, 455.0);
        assert!(!result.failed);
        assert!(rng.exhausted(), "no draw without ALT_GAME_MAGICFAILURES");
        // A critical hit is `* 3`.
        let mut rng = Scripted::new(&[]);
        assert_eq!(
            calc_cubic_magic_dam(&input(&[]), 0, true, &mut rng).damage,
            1365.0
        );
    }

    #[test]
    fn a_shield_adds_to_the_defence_and_a_perfect_block_does_one_point() {
        let mut rng = Scripted::new(&[]);
        // mDef 700: 91 * (2500 / 700).
        let shielded = calc_cubic_magic_dam(&input(&[]), SHIELD_DEFENSE_SUCCEED, false, &mut rng);
        assert_eq!(shielded.damage, 91.0 * (2500.0 / 700.0));
        let blocked = calc_cubic_magic_dam(&input(&[]), SHIELD_DEFENSE_PERFECT_BLOCK, true, &mut rng);
        assert_eq!(blocked.damage, 1.0);
    }

    #[test]
    fn pve_damage_stat_class_balance_and_level_penalty_apply_to_monsters() {
        let penalty = [0.5_f32, 0.25];
        let mut i = input(&penalty);
        i.pve_magical_dmg = 1.2;
        i.pve_magical_skill_damage = 1.5;
        // The target is 3 levels above the owner and the penalty starts at level 40:
        // index 3 - 1 = 2 is past the end, so the last entry (0.25) is used.
        i.target_level = 43;
        i.min_npc_level_dmg_penalty = 40;
        let mut rng = Scripted::new(&[]);
        let result = calc_cubic_magic_dam(&i, 0, false, &mut rng);
        assert_eq!(result.damage, 455.0 * 1.2 * f64::from(1.5_f32) * 0.25);
        // A player target is not attackable: none of it applies, and a playable target reads the
        // PvP power.
        let mut i = input(&penalty);
        i.target_is_attackable = false;
        i.target_is_playable = true;
        i.skill_power.pvp_power = 500.0;
        // 91 * ((1000 + 500) / 500) = 273.
        let mut rng = Scripted::new(&[]);
        assert_eq!(calc_cubic_magic_dam(&i, 0, false, &mut rng).damage, 273.0);
    }

    #[test]
    fn a_failed_magic_check_halves_or_resists_with_the_skill_level_gap() {
        // `calcMagicSuccess` draws once (and not at all for a skill with an activate rate over
        // 99). Power 1500 is not -1, the rate -1: level gap 0 -> modifier 1, rate 100 - 1 = 99.
        let mut i = input(&[]);
        i.alt_game_magicfailures = true;
        i.magic_success.target_is_attackable = false;
        // First draw 99 (not below 99: failed), second draw 0 (succeeds): half damage.
        let mut rng = Scripted::new(&[99, 0]);
        let result = calc_cubic_magic_dam(&i, 0, true, &mut rng);
        assert!(result.failed);
        assert_eq!(result.failure, MagicFailure::Half);
        // The critical hit is not applied after a failure.
        assert_eq!(result.damage, 455.0 / 2.0);
        // The second draw fails as well: resisted, 1 point of damage.
        let mut rng = Scripted::new(&[99, 99]);
        let result = calc_cubic_magic_dam(&i, 0, false, &mut rng);
        assert_eq!((result.failure, result.damage), (MagicFailure::Resisted, 1.0));
        // The gap is the target's level minus the skill's magic level (10 > 9): resisted even
        // though the second draw succeeds.
        i.target_level = 50;
        i.magic_success.target_level = 40;
        let mut rng = Scripted::new(&[99, 0]);
        let result = calc_cubic_magic_dam(&i, 0, false, &mut rng);
        assert_eq!(result.failure, MagicFailure::Resisted);
    }

    #[test]
    fn success_before_the_reflection_follows_java() {
        // A debuff with power -1 always lands; an immune target resists; a perfect block fails.
        assert_eq!(cubic_success_before_reflection(true, -1.0, 5.0, 2), Some(true));
        assert_eq!(cubic_success_before_reflection(true, 10.0, 5.0, 0), Some(false));
        assert_eq!(cubic_success_before_reflection(false, 10.0, 5.0, 0), None);
        assert_eq!(cubic_success_before_reflection(false, 10.0, 0.0, 2), Some(false));
        assert_eq!(cubic_success_before_reflection(true, 10.0, 0.0, 0), None);
    }

    fn rate_input() -> CubicSuccessInput {
        CubicSuccessInput {
            activate_rate: 40,
            basic_property_none: false,
            skill_power: 80.0,
            stat_mod: 1.25,
            res_mod: 1.0,
            lvl_bonus_mod: 1.1,
            element_mod: 1.0,
            min_chance: 10,
            max_chance: 90,
        }
    }

    #[test]
    fn the_landing_rate_is_base_over_stat_times_the_modifiers_clamped() {
        // 40 / 1.25 * 1.1 = 35.2: a draw of 35 lands, 36 does not.
        let mut rng = Scripted::new(&[35, 36]);
        assert!(calc_cubic_success_rate(&rate_input(), &mut rng));
        assert!(!calc_cubic_success_rate(&rate_input(), &mut rng));
        // Without an activate rate the skill's power is the base: 80 / 1.25 * 1.1 = 70.4.
        let mut i = rate_input();
        i.activate_rate = 0;
        let mut rng = Scripted::new(&[70, 71]);
        assert!(calc_cubic_success_rate(&i, &mut rng));
        assert!(!calc_cubic_success_rate(&i, &mut rng));
        // The clamp: a huge rate is held at the maximum (a draw of 90 fails), a tiny one at the
        // minimum.
        i.lvl_bonus_mod = 100.0;
        let mut rng = Scripted::new(&[89, 90]);
        assert!(calc_cubic_success_rate(&i, &mut rng));
        assert!(!calc_cubic_success_rate(&i, &mut rng));
        i.lvl_bonus_mod = 0.0001;
        let mut rng = Scripted::new(&[9, 10]);
        assert!(calc_cubic_success_rate(&i, &mut rng));
        assert!(!calc_cubic_success_rate(&i, &mut rng));
    }

    #[test]
    fn a_skill_that_always_lands_draws_nothing() {
        for (rate, none) in [(-1, false), (100, false), (40, true)] {
            let mut i = rate_input();
            i.activate_rate = rate;
            i.basic_property_none = none;
            let mut rng = Scripted::new(&[]);
            assert!(calc_cubic_success_rate(&i, &mut rng), "rate {rate} none {none}");
            assert!(rng.exhausted());
        }
    }
}
