//! Magic skill formulas: magic damage, mana damage, magic success, magic affected, magic crit.
//!
//! Java: `mechanics/stats/Formulas.java` `calcMagicDam`, `calcManaDam`, `calcMagicSuccess`,
//! `calcMagicAffected`, `calcMCrit`; `entity/actor/stat/CreatureStat.getMCriticalHit`.
//!
//! The "You resisted ..." messages of Java are not sent here: the results say which one applies
//! ([`MagicDamResult`], [`ManaDamResult`]) and the caller sends it.

use super::damage::{pow_int, random_damage_multiplier};
use super::skill::{SkillPower, java_round_f32, npc_level_penalty};
use super::{Rnd, SHIELD_DEFENSE_PERFECT_BLOCK, SHIELD_DEFENSE_SUCCEED};

// ------------------------------------------------------------------ magic crit

/// Java `Formulas.calcMCrit(mRate)`: `mRate > Rnd.get(1000)`. One draw.
pub fn calc_mcrit(m_rate: f64, rng: &mut impl Rnd) -> bool {
    m_rate > f64::from(rng.get_int(1000))
}

/// What `CreatureStat.getMCriticalHit(target, skill)` reads.
#[derive(Debug, Clone, Copy)]
pub struct MCriticalHitInput {
    /// `calcStat(Stat.MCRITICAL_RATE, 1, target, skill)`.
    pub mcritical_rate: f64,
    /// `ClassBalanceConfig.PVP_MAGICAL_SKILL_CRITICAL_CHANCE_MULTIPLIERS` (target playable) or
    /// `PVE_MAGICAL_SKILL_CRITICAL_CHANCE_MULTIPLIERS` of the attacker's class, `Some` only when
    /// `skill != null && target != null && attacker.isPlayable()`.
    pub class_balance_multiplier: Option<f32>,
    /// `PlayerConfig.MAX_MCRIT_RATE`.
    pub max_mcrit_rate: i32,
}

/// Java `CreatureStat.getMCriticalHit(target, skill)`: the rate [`calc_mcrit`] compares.
pub fn calc_m_critical_hit(input: &MCriticalHitInput) -> i32 {
    // NOTE(java-quirk): `(int) calcStat(..) * 10`: the cast comes before the `* 10`.
    let mut val = (input.mcritical_rate as i32).wrapping_mul(10);

    if let Some(multiplier) = input.class_balance_multiplier {
        // `val *= float`: int * float is a float product, narrowed back to int.
        val = (val as f32 * multiplier) as i32;
    }

    val.min(input.max_mcrit_rate)
}

// ------------------------------------------------------------------ magic success

/// What `Formulas.calcMagicSuccess(attacker, target, skill)` reads.
#[derive(Debug, Clone, Copy)]
pub struct MagicSuccessInput<'a> {
    /// `skill.getPower()`.
    pub skill_power: f64,
    /// `skill.getActivateRate()`.
    pub skill_activate_rate: i32,
    /// `PlayerConfig.CALCULATE_MAGIC_SUCCESS_BY_SKILL_MAGIC_LEVEL`.
    pub calculate_magic_success_by_skill_magic_level: bool,
    /// `skill.getMagicLevel()`.
    pub skill_magic_level: i32,
    /// `attacker.getLevel()`.
    pub attacker_level: i32,
    /// `target.getLevel()`.
    pub target_level: i32,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// `target.isRaid()`.
    pub target_is_raid: bool,
    /// `target.isRaidMinion()`.
    pub target_is_raid_minion: bool,
    /// `NpcConfig.MIN_NPC_LEVEL_MAGIC_PENALTY`.
    pub min_npc_level_magic_penalty: i32,
    /// `attacker.asPlayer().getLevel()`, `None` when `asPlayer()` is null.
    pub attacker_player_level: Option<i32>,
    /// `NpcConfig.NPC_SKILL_CHANCE_PENALTY`.
    pub npc_skill_chance_penalty: &'a [f32],
    /// `target.calcStat(Stat.MAGIC_SUCCESS_RES, 1, null, skill)`.
    pub magic_success_res: f64,
}

/// Java `Formulas.calcMagicSuccess`: `true` when the magic is not resisted. One draw,
/// `Rnd.get(100)`; none for a skill without power (-1) or with an activate rate over 99.
pub fn calc_magic_success(input: &MagicSuccessInput<'_>, rng: &mut impl Rnd) -> bool {
    if input.skill_power == -1.0 || input.skill_activate_rate > 99 {
        return true;
    }

    let level_ref = if input.calculate_magic_success_by_skill_magic_level && input.skill_magic_level > 0 {
        input.skill_magic_level
    } else {
        input.attacker_level
    };
    let lvl_modifier = pow_int(1.3, input.target_level.wrapping_sub(level_ref));
    let target_modifier = if input.target_is_attackable {
        // The first entry is used from a gap of 3 levels.
        npc_level_penalty(
            input.npc_skill_chance_penalty,
            input.target_level,
            input.attacker_player_level,
            input.min_npc_level_magic_penalty,
            3,
            input.target_is_raid,
            input.target_is_raid_minion,
        ) as f32
    } else {
        1.0
    };

    // General magic resist.
    let res_modifier = input.magic_success_res;
    let rate = 100_i32.wrapping_sub(java_round_f32(
        (lvl_modifier * f64::from(target_modifier) * res_modifier) as f32,
    ));

    rng.get_int(100) < rate
}

// ------------------------------------------------------------------ magic damage

/// How a failed magic check ended for a player attacker (the system message to send).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagicFailure {
    /// The attacker is not a player, or the magic was not resisted.
    None,
    /// The attack went through at half damage ("Your attack has failed", "Drain was only 50
    /// percent successful").
    Half,
    /// Fully resisted, damage is 1 ("C1 has resisted your S2").
    Resisted,
}

/// The `ClassBalanceConfig` multipliers `calcMagicDam` reads (the entry of the class Java
/// indexes with; `float` in Java, 1.0 by default).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagicDamClassBalance {
    /// `PVP_MAGICAL_SKILL_DEFENCE_MULTIPLIERS[target class]`.
    pub pvp_magical_skill_defence: f32,
    /// `PVP_PHYSICAL_SKILL_DEFENCE_MULTIPLIERS[target class]` (non-magic skills).
    pub pvp_physical_skill_defence: f32,
    /// `PVP_MAGICAL_SKILL_CRITICAL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pvp_magical_skill_critical_damage: f32,
    /// `PVE_MAGICAL_SKILL_CRITICAL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pve_magical_skill_critical_damage: f32,
    /// `PVP_MAGICAL_SKILL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pvp_magical_skill_damage: f32,
    /// `PVP_PHYSICAL_SKILL_DAMAGE_MULTIPLIERS[attacker class]` (non-magic skills).
    pub pvp_physical_skill_damage: f32,
    /// `PVE_MAGICAL_SKILL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pve_magical_skill_damage: f32,
}

impl Default for MagicDamClassBalance {
    fn default() -> Self {
        Self {
            pvp_magical_skill_defence: 1.0,
            pvp_physical_skill_defence: 1.0,
            pvp_magical_skill_critical_damage: 1.0,
            pve_magical_skill_critical_damage: 1.0,
            pvp_magical_skill_damage: 1.0,
            pvp_physical_skill_damage: 1.0,
            pve_magical_skill_damage: 1.0,
        }
    }
}

/// What `Formulas.calcMagicDam(attacker, target, skill, shld, sps, bss, mcrit)` reads.
#[derive(Debug, Clone, Copy)]
pub struct MagicDamInput<'a> {
    /// `attacker.isPlayable()`.
    pub attacker_is_playable: bool,
    /// `attacker.isPlayer()`.
    pub attacker_is_player: bool,
    /// `target.isPlayable()`.
    pub target_is_playable: bool,
    /// `target.isPlayer()`.
    pub target_is_player: bool,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// `target.getMDef(attacker, skill)`.
    pub m_def: f64,
    /// `target.getShldDef()`.
    pub shld_def: i32,
    /// `attacker.getMAtk(target, skill)`.
    pub m_atk: f64,
    /// `skill.isMagic()`.
    pub skill_is_magic: bool,
    /// `skill.getPower(attacker, target, isPvP, isPvE)`.
    pub skill_power: SkillPower,
    /// `target.calcStat(Stat.PVP_MAGICAL_DEF, 1, null, null)`.
    pub pvp_magical_def: f64,
    /// `target.calcStat(Stat.PVP_PHYS_SKILL_DEF, 1, null, null)`.
    pub pvp_phys_skill_def: f64,
    /// `PlayerConfig.ALT_GAME_MAGICFAILURES`.
    pub alt_game_magicfailures: bool,
    /// The inputs of the `calcMagicSuccess` calls.
    pub magic_success: MagicSuccessInput<'a>,
    /// `attacker.getLevel()`.
    pub attacker_level: i32,
    /// `attacker.calcStat(Stat.MAGIC_CRIT_DMG, 1, null, null)`.
    pub magic_crit_dmg: f64,
    /// `PlayerConfig.RANDOMIZE_MAGICAL_SKILL_DAMAGE`.
    pub randomize_magical_skill_damage: bool,
    /// `attacker.getActiveWeaponItem() != null`.
    pub attacker_has_active_weapon: bool,
    /// `(int) attacker.getStat().calcStat(Stat.RANDOM_DAMAGE, 0)`.
    pub random_damage: i32,
    /// `attacker.calcStat(Stat.PVP_MAGICAL_DMG, 1, null, null)`.
    pub pvp_magical_dmg: f64,
    /// `attacker.calcStat(Stat.PVP_PHYS_SKILL_DMG, 1, null, null)`.
    pub pvp_phys_skill_dmg: f64,
    /// `Formulas.calcAttributeBonus(attacker, target, skill)`.
    pub attribute_bonus: f64,
    /// `attacker.calcStat(Stat.PVE_MAGICAL_DMG, 1, null, null)`.
    pub pve_magical_dmg: f64,
    /// `attacker.asPlayer().getLevel()`, `None` when `asPlayer()` is null.
    pub attacker_player_level: Option<i32>,
    /// `target.isRaid()`.
    pub target_is_raid: bool,
    /// `target.isRaidMinion()`.
    pub target_is_raid_minion: bool,
    /// `target.getLevel()`.
    pub target_level: i32,
    /// `NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY`.
    pub min_npc_level_dmg_penalty: i32,
    /// `NpcConfig.NPC_SKILL_DMG_PENALTY`.
    pub npc_skill_dmg_penalty: &'a [f32],
    /// The class balance multipliers.
    pub class_balance: MagicDamClassBalance,
}

/// The result of [`calc_magic_dam`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagicDamResult {
    /// The damage as a double (the effect casts it with `(int)`).
    pub damage: f64,
    /// The first `calcMagicSuccess` failed (with `ALT_GAME_MAGICFAILURES`): a player target is
    /// told "You resisted C1's magic" (drain: "You resisted C1's drain").
    pub failed: bool,
    /// What happened to a player attacker's damage when the check failed.
    pub failure: MagicFailure,
}

/// Java `Formulas.calcMagicDam(attacker, target, skill, shld, sps, bss, mcrit)`.
///
/// A perfect block returns 1 before any draw. With `alt_game_magicfailures` the first draw is
/// `Rnd.get(100)` of `calcMagicSuccess`; a failed check for a player attacker draws it again
/// (see the quirk below); the weapon random multiplier follows.
pub fn calc_magic_dam(
    input: &MagicDamInput<'_>,
    shld: u8,
    sps: bool,
    bss: bool,
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

    let mut m_atk = input.m_atk;
    let is_pvp = input.attacker_is_playable && input.target_is_playable;
    let is_pve = input.attacker_is_playable && input.target_is_attackable;

    // PvP bonuses for defense.
    if is_pvp {
        if input.skill_is_magic {
            m_def *= input.pvp_magical_def;
            m_def *= f64::from(input.class_balance.pvp_magical_skill_defence);
        } else {
            m_def *= input.pvp_phys_skill_def;
            m_def *= f64::from(input.class_balance.pvp_physical_skill_defence);
        }
    }

    // Bonus Spirit shot.
    m_atk *= if bss {
        4.0
    } else if sps {
        2.0
    } else {
        1.0
    };

    // MDAM Formula.
    let mut damage = ((91.0 * m_atk.sqrt()) / m_def) * input.skill_power.get(is_pvp, is_pve);

    let mut failed = false;
    let mut failure = MagicFailure::None;

    // Failure calculation.
    if input.alt_game_magicfailures && !calc_magic_success(&input.magic_success, rng) {
        failed = true;
        if input.attacker_is_player {
            // NOTE(java-quirk): the check is rolled a second time to decide between "half
            // damage" and "fully resisted", so a failure that was a 1-in-N event turns into the
            // half damage case with the chance of a success.
            if calc_magic_success(&input.magic_success, rng)
                && input.target_level.wrapping_sub(input.attacker_level) <= 9
            {
                failure = MagicFailure::Half;
                damage /= 2.0;
            } else {
                failure = MagicFailure::Resisted;
                damage = 1.0;
            }
        }
    } else if mcrit {
        damage *= if input.attacker_is_player && input.target_is_player {
            2.5
        } else {
            3.0
        };
        damage *= input.magic_crit_dmg;

        if input.attacker_is_playable {
            damage *= f64::from(if input.target_is_playable {
                input.class_balance.pvp_magical_skill_critical_damage
            } else {
                input.class_balance.pve_magical_skill_critical_damage
            });
        }
    }

    // Weapon random damage.
    if input.randomize_magical_skill_damage {
        damage *= random_damage_multiplier(
            input.attacker_has_active_weapon,
            input.random_damage,
            input.attacker_level,
            rng,
        );
    }

    // PvP bonuses for damage.
    if is_pvp {
        if input.skill_is_magic {
            damage *= f64::from(input.class_balance.pvp_magical_skill_damage);
            damage *= input.pvp_magical_dmg;
        } else {
            damage *= f64::from(input.class_balance.pvp_physical_skill_damage);
            damage *= input.pvp_phys_skill_dmg;
        }
    }

    damage *= input.attribute_bonus;

    if input.target_is_attackable {
        damage *= input.pve_magical_dmg;

        if input.attacker_player_level.is_some() {
            damage *= f64::from(input.class_balance.pve_magical_skill_damage);

            damage *= npc_level_penalty(
                input.npc_skill_dmg_penalty,
                input.target_level,
                input.attacker_player_level,
                input.min_npc_level_dmg_penalty,
                2,
                input.target_is_raid,
                input.target_is_raid_minion,
            );
        }
    }

    MagicDamResult {
        damage,
        failed,
        failure,
    }
}

// ------------------------------------------------------------------ mana damage

/// What `Formulas.calcManaDam(attacker, target, skill, shld, sps, bss, mcrit)` reads.
#[derive(Debug, Clone, Copy)]
pub struct ManaDamInput<'a> {
    /// `attacker.getMAtk(target, skill)`.
    pub m_atk: f64,
    /// `target.getMDef(attacker, skill)`.
    pub m_def: f64,
    /// `target.getShldDef()`.
    pub shld_def: i32,
    /// `target.getMaxMp()`.
    pub target_max_mp: i32,
    /// `skill.getPower(attacker, target, isPvP, isPvE)`.
    pub skill_power: SkillPower,
    /// `attacker.isPlayable()`.
    pub attacker_is_playable: bool,
    /// `attacker.isPlayer()`.
    pub attacker_is_player: bool,
    /// `target.isPlayable()`.
    pub target_is_playable: bool,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// `Formulas.calcGeneralTraitBonus(attacker, target, skill.getTraitType(), false)`.
    pub general_trait_bonus: f64,
    /// `attacker.calcStat(Stat.PVE_MAGICAL_DMG, 1, null, null)`.
    pub pve_magical_dmg: f64,
    /// `attacker.asPlayer().getLevel()`, `None` when `asPlayer()` is null.
    pub attacker_player_level: Option<i32>,
    /// `ClassBalanceConfig.PVE_MAGICAL_SKILL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pve_magical_skill_damage: f32,
    /// `target.isRaid()`.
    pub target_is_raid: bool,
    /// `target.isRaidMinion()`.
    pub target_is_raid_minion: bool,
    /// `target.getLevel()`.
    pub target_level: i32,
    /// `NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY`.
    pub min_npc_level_dmg_penalty: i32,
    /// `NpcConfig.NPC_SKILL_DMG_PENALTY`.
    pub npc_skill_dmg_penalty: &'a [f32],
    /// `PlayerConfig.ALT_GAME_MAGICFAILURES`.
    pub alt_game_magicfailures: bool,
    /// The inputs of the `calcMagicSuccess` call.
    pub magic_success: MagicSuccessInput<'a>,
}

/// The result of [`calc_mana_dam`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManaDamResult {
    /// The mana damage as a double.
    pub damage: f64,
    /// `calcMagicSuccess` failed (with `ALT_GAME_MAGICFAILURES`): a player attacker's damage was
    /// halved ("Damage is decreased because C1 resisted C2's magic"), and a player target is told
    /// "C1 weakly resisted C2's magic".
    pub resisted: bool,
}

/// Java `Formulas.calcManaDam`. A perfect block returns 1 before any draw; one `Rnd.get(100)` for
/// the magic check with `alt_game_magicfailures`. The "magic critical hit" message of `mcrit` is
/// the caller's.
pub fn calc_mana_dam(
    input: &ManaDamInput<'_>,
    shld: u8,
    sps: bool,
    bss: bool,
    mcrit: bool,
    rng: &mut impl Rnd,
) -> ManaDamResult {
    // Formula: (SQR(M.Atk)*Power*(Target Max MP/97))/M.Def
    let mut m_atk = input.m_atk;
    let mut m_def = input.m_def;
    let is_pvp = input.attacker_is_playable && input.target_is_playable;
    let is_pve = input.attacker_is_playable && input.target_is_attackable;
    let mp = f64::from(input.target_max_mp);

    match shld {
        SHIELD_DEFENSE_SUCCEED => m_def += f64::from(input.shld_def),
        SHIELD_DEFENSE_PERFECT_BLOCK => {
            return ManaDamResult {
                damage: 1.0,
                resisted: false,
            };
        }
        _ => {}
    }

    // Bonus Spiritshot.
    m_atk *= if bss {
        4.0
    } else if sps {
        2.0
    } else {
        1.0
    };

    let mut damage = (m_atk.sqrt() * input.skill_power.get(is_pvp, is_pve) * (mp / 97.0)) / m_def;
    damage *= input.general_trait_bonus;

    if input.target_is_attackable {
        damage *= input.pve_magical_dmg;

        if input.attacker_player_level.is_some() {
            damage *= f64::from(input.pve_magical_skill_damage);

            damage *= npc_level_penalty(
                input.npc_skill_dmg_penalty,
                input.target_level,
                input.attacker_player_level,
                input.min_npc_level_dmg_penalty,
                2,
                input.target_is_raid,
                input.target_is_raid_minion,
            );
        }
    }

    // Failure calculation.
    let mut resisted = false;
    if input.alt_game_magicfailures && !calc_magic_success(&input.magic_success, rng) {
        resisted = true;
        if input.attacker_is_player {
            damage /= 2.0;
        }
    }

    if mcrit {
        damage *= 3.0;
    }

    ManaDamResult { damage, resisted }
}

// ------------------------------------------------------------------ magic affected

/// What `Formulas.calcMagicAffected(actor, target, skill)` reads.
#[derive(Debug, Clone, Copy)]
pub struct MagicAffectedInput {
    /// `skill.isActive()`.
    pub skill_is_active: bool,
    /// `skill.hasNegativeEffect()`.
    pub skill_has_negative_effect: bool,
    /// `skill.isDebuff()`.
    pub skill_is_debuff: bool,
    /// `target.getMDef(actor, skill)`.
    pub m_def: f64,
    /// `actor.getMAtk(target, skill)`.
    pub m_atk: f64,
    /// `Formulas.calcGeneralTraitBonus(actor, target, skill.getTraitType(), false)`.
    pub trait_bonus: f64,
    /// `target.calcStat(Stat.DEBUFF_IMMUNITY, 0, null, skill)`.
    pub debuff_immunity: f64,
}

/// Java `Formulas.calcMagicAffected`: whether an area magic skill reaches the target.
/// One `Rnd.nextGaussian()`, none for a debuff the target is immune to.
pub fn calc_magic_affected(input: &MagicAffectedInput, rng: &mut impl Rnd) -> bool {
    // TODO: CHECK/FIX THIS FORMULA UP!! (Java's own comment.)
    let mut defence = 0.0;
    if input.skill_is_active && input.skill_has_negative_effect {
        defence = input.m_def;
    }

    let attack = 2.0 * input.m_atk * input.trait_bonus;
    let mut d = (attack - defence) / (attack + defence);

    if input.skill_is_debuff && input.debuff_immunity > 0.0 {
        return false;
    }

    d += 0.5 * rng.next_gaussian();
    d > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::rng::testing::Fixed;
    use crate::formulas::rng::testing_gaussian::Gaussian;

    const SKILL_PENALTY: [f32; 4] = [0.8, 0.7, 0.65, 0.62];
    const CHANCE_PENALTY: [f32; 4] = [2.5, 3.0, 3.25, 3.5];

    fn success() -> MagicSuccessInput<'static> {
        MagicSuccessInput {
            skill_power: 100.0,
            skill_activate_rate: -1,
            calculate_magic_success_by_skill_magic_level: true,
            skill_magic_level: 74,
            attacker_level: 76,
            target_level: 76,
            target_is_attackable: true,
            target_is_raid: false,
            target_is_raid_minion: false,
            min_npc_level_magic_penalty: 78,
            attacker_player_level: Some(76),
            npc_skill_chance_penalty: &CHANCE_PENALTY,
            magic_success_res: 1.0,
        }
    }

    #[test]
    fn magic_success_rate() {
        // 1.3^2 = 1.69 -> round 2 -> rate 98: draws 0..97 succeed.
        let i = success();
        assert!(calc_magic_success(&i, &mut Fixed::new(&[97])));
        assert!(!calc_magic_success(&i, &mut Fixed::new(&[98])));
    }

    #[test]
    fn magic_success_without_a_draw() {
        let mut i = success();
        i.skill_power = -1.0;
        assert!(calc_magic_success(&i, &mut Fixed::new(&[])));
        i.skill_power = 10.0;
        i.skill_activate_rate = 100;
        assert!(calc_magic_success(&i, &mut Fixed::new(&[])));
    }

    #[test]
    fn magic_success_level_reference_and_penalty() {
        let mut i = success();
        i.calculate_magic_success_by_skill_magic_level = false;
        // Attacker level 76 against 76: modifier 1 -> rate 99.
        assert!(calc_magic_success(&i, &mut Fixed::new(&[98])));
        assert!(!calc_magic_success(&i, &mut Fixed::new(&[99])));
        // A monster of level 80 above the minimum, 4 levels over the player: index 4 - 2 = 2.
        i.target_level = 80;
        i.attacker_level = 80;
        i.attacker_player_level = Some(76);
        // 1.0 * 3.25 -> round 3 -> rate 97.
        assert!(calc_magic_success(&i, &mut Fixed::new(&[96])));
        assert!(!calc_magic_success(&i, &mut Fixed::new(&[97])));
        // The resist stat multiplies.
        i.magic_success_res = 2.0;
        // 3.25 * 2 = 6.5 -> round 7 -> rate 93.
        assert!(calc_magic_success(&i, &mut Fixed::new(&[92])));
        assert!(!calc_magic_success(&i, &mut Fixed::new(&[93])));
    }

    #[test]
    fn mcrit() {
        assert!(calc_mcrit(100.5, &mut Fixed::new(&[100])));
        assert!(!calc_mcrit(100.0, &mut Fixed::new(&[100])));
    }

    #[test]
    fn m_critical_hit_rate() {
        let base = MCriticalHitInput {
            mcritical_rate: 6.9,
            class_balance_multiplier: None,
            max_mcrit_rate: 200,
        };
        // (int) 6.9 = 6, x 10.
        assert_eq!(calc_m_critical_hit(&base), 60);
        let with_class = MCriticalHitInput {
            class_balance_multiplier: Some(1.5),
            ..base
        };
        assert_eq!(calc_m_critical_hit(&with_class), 90);
        let capped = MCriticalHitInput {
            mcritical_rate: 100.0,
            ..base
        };
        assert_eq!(calc_m_critical_hit(&capped), 200);
    }

    fn dam() -> MagicDamInput<'static> {
        MagicDamInput {
            attacker_is_playable: true,
            attacker_is_player: true,
            target_is_playable: false,
            target_is_player: false,
            target_is_attackable: true,
            m_def: 100.0,
            shld_def: 0,
            m_atk: 10_000.0,
            skill_is_magic: true,
            skill_power: SkillPower::flat(10.0),
            pvp_magical_def: 1.0,
            pvp_phys_skill_def: 1.0,
            alt_game_magicfailures: true,
            magic_success: success(),
            attacker_level: 76,
            magic_crit_dmg: 1.0,
            randomize_magical_skill_damage: true,
            attacker_has_active_weapon: true,
            random_damage: 0,
            pvp_magical_dmg: 1.0,
            pvp_phys_skill_dmg: 1.0,
            attribute_bonus: 1.0,
            pve_magical_dmg: 1.0,
            attacker_player_level: Some(76),
            target_is_raid: false,
            target_is_raid_minion: false,
            target_level: 76,
            min_npc_level_dmg_penalty: 78,
            npc_skill_dmg_penalty: &SKILL_PENALTY,
            class_balance: MagicDamClassBalance::default(),
        }
    }

    fn run(input: &MagicDamInput<'_>, shots: (bool, bool, bool), draws: &[i32]) -> MagicDamResult {
        let mut rng = Fixed::new(draws);
        let r = calc_magic_dam(input, 0, shots.0, shots.1, shots.2, &mut rng);
        assert!(rng.exhausted(), "all scripted draws used");
        r
    }

    #[test]
    fn plain_damage_and_shots() {
        // (91 * 100 / 100) * 10 = 910
        let r = run(&dam(), (false, false, false), &[0]);
        assert_eq!(r.damage, 910.0);
        assert!(!r.failed);
        // sps x2 on the m.atk, bss x4: sqrt(20000), sqrt(40000) = 200.
        assert_eq!(
            run(&dam(), (true, false, false), &[0]).damage,
            (91.0 * 20_000_f64.sqrt() / 100.0) * 10.0
        );
        assert_eq!(run(&dam(), (true, true, false), &[0]).damage, 91.0 * 2.0 * 10.0);
    }

    #[test]
    fn a_failed_check_for_a_player_rolls_again() {
        let i = dam();
        // First draw fails (98), second also fails: damage 1.
        let r = run(&i, (false, false, false), &[98, 98]);
        assert_eq!(
            (r.damage, r.failed, r.failure),
            (1.0, true, MagicFailure::Resisted)
        );
        // Second succeeds and the level gap is 0 <= 9: half damage.
        let r = run(&i, (false, false, false), &[98, 0]);
        assert_eq!((r.damage, r.failed, r.failure), (455.0, true, MagicFailure::Half));
        // Second succeeds but the target is 10 levels higher: resisted.
        let mut far = i;
        far.target_level = 86;
        far.magic_success.target_level = 86;
        far.magic_success.skill_magic_level = 86;
        // No player level, so no penalties: 1.3^0 = 1 -> rate 99 -> a draw of 99 fails.
        far.attacker_player_level = None;
        far.magic_success.attacker_player_level = None;
        let r = run(&far, (false, false, false), &[99, 0]);
        assert_eq!(r.failure, MagicFailure::Resisted);
        assert_eq!(r.damage, 1.0);
    }

    #[test]
    fn a_failed_check_for_an_npc_attacker_draws_once_and_keeps_the_damage() {
        let mut i = dam();
        i.attacker_is_player = false;
        i.attacker_is_playable = false;
        i.attacker_player_level = None;
        // NOTE(java-quirk): the magic crit is skipped after a failed check as well (else-if).
        let r = run(&i, (false, false, true), &[98]);
        assert_eq!((r.damage, r.failed, r.failure), (910.0, true, MagicFailure::None));
    }

    #[test]
    fn magic_crit_multipliers() {
        let mut i = dam();
        i.magic_crit_dmg = 1.5;
        i.class_balance.pve_magical_skill_critical_damage = 2.0;
        // 910 * 3 (PvE) * 1.5 * 2.
        assert_eq!(
            run(&i, (false, false, true), &[0]).damage,
            910.0 * 3.0 * 1.5 * 2.0
        );
        // Player against player: 2.5 and the PvP table.
        i.target_is_playable = true;
        i.target_is_player = true;
        i.target_is_attackable = false;
        i.class_balance.pvp_magical_skill_critical_damage = 0.5;
        assert_eq!(
            run(&i, (false, false, true), &[0]).damage,
            910.0 * 2.5 * 1.5 * 0.5
        );
    }

    #[test]
    fn pvp_uses_the_magical_or_physical_skill_stats() {
        let mut i = dam();
        i.target_is_playable = true;
        i.target_is_player = true;
        i.target_is_attackable = false;
        i.pvp_magical_def = 2.0;
        i.pvp_magical_dmg = 3.0;
        i.pvp_phys_skill_def = 4.0;
        i.pvp_phys_skill_dmg = 5.0;
        // Magic skill: defence x2, damage x3.
        assert_eq!(
            run(&i, (false, false, false), &[0]).damage,
            ((91.0 * 100.0 / 200.0) * 10.0) * 3.0
        );
        // Non-magic skill: defence x4, damage x5.
        i.skill_is_magic = false;
        assert_eq!(
            run(&i, (false, false, false), &[0]).damage,
            ((91.0 * 100.0 / 400.0) * 10.0) * 5.0
        );
    }

    #[test]
    fn perfect_block_and_shield() {
        let i = dam();
        let mut rng = Fixed::new(&[]);
        assert_eq!(calc_magic_dam(&i, 2, false, false, true, &mut rng).damage, 1.0);
        let mut s = i;
        s.shld_def = 100;
        let mut rng = Fixed::new(&[0]);
        // m.def 200: half.
        assert_eq!(calc_magic_dam(&s, 1, false, false, false, &mut rng).damage, 455.0);
    }

    #[test]
    fn pve_penalty_and_class_balance() {
        let mut i = dam();
        i.target_level = 80;
        i.magic_success.target_level = 80;
        i.pve_magical_dmg = 2.0;
        i.class_balance.pve_magical_skill_damage = 0.5;
        // The check passes with a low draw (1.3^6 * 3.25 rounds to 16: rate 84). The player is
        // level 76, the monster 80: damage penalty index 4 - 1 = 3 -> 0.62.
        let r = run(&i, (false, false, false), &[0]);
        assert_eq!(r.damage, 910.0 * 2.0 * f64::from(0.5_f32) * f64::from(0.62_f32));
    }

    #[test]
    fn magic_failures_off_never_draws_for_success() {
        let mut i = dam();
        i.alt_game_magicfailures = false;
        i.randomize_magical_skill_damage = false;
        assert_eq!(run(&i, (false, false, false), &[]).damage, 910.0);
    }

    fn mana() -> ManaDamInput<'static> {
        ManaDamInput {
            m_atk: 10_000.0,
            m_def: 100.0,
            shld_def: 0,
            target_max_mp: 970,
            skill_power: SkillPower::flat(10.0),
            attacker_is_playable: true,
            attacker_is_player: true,
            target_is_playable: false,
            target_is_attackable: true,
            general_trait_bonus: 1.0,
            pve_magical_dmg: 1.0,
            attacker_player_level: Some(76),
            pve_magical_skill_damage: 1.0,
            target_is_raid: false,
            target_is_raid_minion: false,
            target_level: 76,
            min_npc_level_dmg_penalty: 78,
            npc_skill_dmg_penalty: &SKILL_PENALTY,
            alt_game_magicfailures: true,
            magic_success: success(),
        }
    }

    #[test]
    fn mana_damage() {
        let mut rng = Fixed::new(&[0]);
        // sqrt(10000) * 10 * (970 / 97) / 100 = 100 * 10 * 10 / 100 = 100
        let r = calc_mana_dam(&mana(), 0, false, false, false, &mut rng);
        assert_eq!((r.damage, r.resisted), (100.0, false));
        // A resisted check halves a player's damage; a magic crit triples it.
        let mut rng = Fixed::new(&[98]);
        let r = calc_mana_dam(&mana(), 0, false, false, true, &mut rng);
        assert_eq!((r.damage, r.resisted), (150.0, true));
        // An NPC attacker keeps the damage.
        let mut npc = mana();
        npc.attacker_is_player = false;
        npc.attacker_player_level = None;
        let mut rng = Fixed::new(&[98]);
        let r = calc_mana_dam(&npc, 0, false, false, false, &mut rng);
        assert_eq!((r.damage, r.resisted), (100.0, true));
        // Perfect block.
        let mut rng = Fixed::new(&[]);
        assert_eq!(calc_mana_dam(&mana(), 2, true, true, true, &mut rng).damage, 1.0);
    }

    #[test]
    fn magic_affected() {
        let input = MagicAffectedInput {
            skill_is_active: true,
            skill_has_negative_effect: true,
            skill_is_debuff: false,
            m_def: 100.0,
            m_atk: 100.0,
            trait_bonus: 1.0,
            debuff_immunity: 0.0,
        };
        // attack 200, defence 100: d = 1/3; the gaussian must not be below -2/3.
        assert!(calc_magic_affected(&input, &mut Gaussian(-0.6)));
        assert!(!calc_magic_affected(&input, &mut Gaussian(-0.7)));
        // A buff has no defence: d = 1.
        let buff = MagicAffectedInput {
            skill_has_negative_effect: false,
            ..input
        };
        assert!(calc_magic_affected(&buff, &mut Gaussian(-1.9)));
        // A debuff with immunity returns before the draw.
        let immune = MagicAffectedInput {
            skill_is_debuff: true,
            debuff_immunity: 1.0,
            ..input
        };
        assert!(!calc_magic_affected(&immune, &mut Fixed::new(&[])));
    }
}
