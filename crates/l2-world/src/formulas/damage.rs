//! Physical damage of a normal attack and the helpers it calls.
//!
//! Java: `mechanics/stats/Formulas.java` `calcPhysDam(attacker, target, null, shld, crit, ss)`,
//! `calcAttributeBonus`, `calcGeneralTraitBonus` / `calcWeaponTraitBonus` / `calcAttackTraitBonus`,
//! `calcLvlBonusMod`; `entity/actor/Creature.getRandomDamageMultiplier`.
//! Skill damage (`skill != null`: skill power, `PHYSICAL_SKILL_POWER`, skill penalties, blows) is M5.

use super::{Position, Rnd, SHIELD_DEFENSE_PERFECT_BLOCK, SHIELD_DEFENSE_SUCCEED};

/// The `ClassBalanceConfig` multipliers `calcPhysDam` reads for a normal attack. Each is the array
/// entry of the class the Java code indexes with (`attacker.asPlayer().getPlayerClass().getId()`,
/// or the target's for the defence one). They are `float` in Java. 1.0 everywhere by default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysDamClassBalance {
    /// `PVP_PHYSICAL_SKILL_DEFENCE_MULTIPLIERS[target class]`, applied to the defence in PvP.
    pub pvp_physical_skill_defence: f32,
    /// `PVP_PHYSICAL_ATTACK_CRITICAL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pvp_physical_attack_critical_damage: f32,
    /// `PVE_PHYSICAL_ATTACK_CRITICAL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pve_physical_attack_critical_damage: f32,
    /// `PVP_PHYSICAL_ATTACK_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pvp_physical_attack_damage: f32,
    /// `PVE_PHYSICAL_ATTACK_DAMAGE_MULTIPLIERS[attacker class]` (non-bow weapons, playable attacker).
    pub pve_physical_attack_damage: f32,
}

impl Default for PhysDamClassBalance {
    fn default() -> Self {
        Self {
            pvp_physical_skill_defence: 1.0,
            pvp_physical_attack_critical_damage: 1.0,
            pve_physical_attack_critical_damage: 1.0,
            pvp_physical_attack_damage: 1.0,
            pve_physical_attack_damage: 1.0,
        }
    }
}

/// What `Formulas.calcPhysDam(attacker, target, null, shld, crit, ss)` reads.
#[derive(Debug, Clone, Copy)]
pub struct PhysDamInput<'a> {
    /// `attacker.isPlayable()`.
    pub attacker_is_playable: bool,
    /// `target.isPlayable()`.
    pub target_is_playable: bool,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// Where the attacker stands: behind +20 %, side +10 % (`isBehind` first, then `isInFrontOf`).
    pub position: Position,
    /// `attacker.getPAtk(target)`.
    pub p_atk: f64,
    /// `target.getPDef(attacker)`.
    pub p_def: f64,
    /// `target.getShldDef()`.
    pub shld_def: i32,
    /// `target.calcStat(Stat.PVP_PHYSICAL_DEF, 1, null, null)`.
    pub pvp_physical_def: f64,
    /// `attacker.calcStat(Stat.CRITICAL_DAMAGE, 1, target, null)`.
    pub critical_damage: f64,
    /// `attacker.calcStat(Stat.CRITICAL_DAMAGE_POS, 1, target, null)`.
    pub critical_damage_pos: f64,
    /// `target.calcStat(Stat.DEFENCE_CRITICAL_DAMAGE, 1, target, null)`.
    pub defence_critical_damage: f64,
    /// `attacker.calcStat(Stat.CRITICAL_DAMAGE_ADD, 0, target, null)`.
    pub critical_damage_add: f64,
    /// `target.calcStat(Stat.DEFENCE_CRITICAL_DAMAGE_ADD, 0, target, null)`.
    pub defence_critical_damage_add: f64,
    /// `Formulas.calcAttackTraitBonus(attacker, target)`, see [`calc_attack_trait_bonus`].
    pub attack_trait_bonus: f64,
    /// `attacker.getActiveWeaponItem() != null`.
    pub attacker_has_active_weapon: bool,
    /// `(int) attacker.getStat().calcStat(Stat.RANDOM_DAMAGE, 0)`.
    pub random_damage: i32,
    /// `attacker.getLevel()`.
    pub attacker_level: i32,
    /// `attacker.calcStat(Stat.PVP_PHYSICAL_DMG, 1, null, null)`.
    pub pvp_physical_dmg: f64,
    /// Attack attribute value used by `calcAttributeBonus` (`attacker.getAttackElementValue(attacker.getAttackElement())`).
    pub attack_attribute: i32,
    /// Defence attribute value of the target for the same element.
    pub defence_attribute: i32,
    /// The attacker's active weapon is a bow or a crossbow.
    pub attacker_weapon_is_bow_or_crossbow: bool,
    /// `attacker.calcStat(Stat.PVE_BOW_DMG, 1, null, null)`.
    pub pve_bow_dmg: f64,
    /// `attacker.calcStat(Stat.PVE_PHYSICAL_DMG, 1, null, null)`.
    pub pve_physical_dmg: f64,
    /// `target.isRaid()`.
    pub target_is_raid: bool,
    /// `target.isRaidMinion()`.
    pub target_is_raid_minion: bool,
    /// `target.getLevel()`.
    pub target_level: i32,
    /// `attacker.asPlayer().getLevel()`, `None` when `asPlayer()` is null (a plain monster).
    pub attacker_player_level: Option<i32>,
    /// The class balance multipliers.
    pub class_balance: PhysDamClassBalance,
    /// `PlayerConfig.ALT_GAME_SHIELD_BLOCKS`.
    pub alt_game_shield_blocks: bool,
    /// `PlayerConfig.RANDOMIZE_AUTO_ATTACK_DAMAGE`.
    pub randomize_auto_attack_damage: bool,
    /// `NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY`.
    pub min_npc_level_dmg_penalty: i32,
    /// `NpcConfig.NPC_DMG_PENALTY`.
    pub npc_dmg_penalty: &'a [f32],
    /// `NpcConfig.NPC_CRIT_DMG_PENALTY`.
    pub npc_crit_dmg_penalty: &'a [f32],
}

/// Java `Creature.getRandomDamageMultiplier()`: `1 + Rnd.get(-r, r) / 100`, where `r` is the
/// weapon's `RANDOM_DAMAGE` stat or, without a weapon, `5 + (int) sqrt(level)`.
///
/// No draw when `r` is 0 (`Rnd.get(0, 0)` returns its origin).
pub fn random_damage_multiplier(
    has_active_weapon: bool,
    random_damage: i32,
    level: i32,
    rng: &mut impl Rnd,
) -> f64 {
    let random = if has_active_weapon {
        random_damage
    } else {
        5_i32.wrapping_add(f64::from(level).sqrt() as i32)
    };

    1.0 + f64::from(rng.get_int_range(0_i32.wrapping_sub(random), random)) / 100.0
}

/// Java `MathUtil.pow(double, int)` (exponentiation by squaring, the same rounding as Java).
fn pow_int(mut base: f64, mut exp: i32) -> f64 {
    if exp == 0 {
        return 1.0;
    }
    if exp < 0 {
        return 1.0 / pow_int(base, exp.wrapping_neg());
    }
    let mut result = 1.0;
    while exp != 0 {
        if (exp & 1) != 0 {
            result *= base;
        }
        base *= base;
        exp >>= 1;
    }
    result
}

/// Java `Formulas.calcAttributeBonus` for a normal attack (`skill == null`):
/// diff > 0 gives `min(1.025 + sqrt(diff^3 / 2) * 0.0001, 1.25)`, diff < 0 the mirrored
/// `max(0.975 - sqrt(|diff|^3 / 2) * 0.0001, 0.75)`, 0 gives 1.
pub fn calc_attribute_bonus(attack_attribute: i32, defence_attribute: i32) -> f64 {
    let diff = attack_attribute.wrapping_sub(defence_attribute);
    if diff > 0 {
        (1.025 + ((pow_int(f64::from(diff), 3) / 2.0).sqrt() * 0.0001)).min(1.25)
    } else if diff < 0 {
        (0.975 - ((pow_int(f64::from(diff.wrapping_neg()), 3) / 2.0).sqrt() * 0.0001)).max(0.75)
    } else {
        1.0
    }
}

/// One weakness trait (`TraitType.getType() == 2`) as `calcGeneralTraitBonus(attacker, target,
/// traitType, true)` sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraitInput {
    /// `target.getStat().isTraitInvul(traitType)`.
    pub target_invul: bool,
    /// `attacker.getStat().hasAttackTrait(traitType)`.
    pub attacker_has_attack_trait: bool,
    /// `target.getStat().hasDefenceTrait(traitType)`.
    pub target_has_defence_trait: bool,
    /// `attacker.getStat().getAttackTrait(traitType)` (a `float` in Java).
    pub attack_trait: f32,
    /// `target.getStat().getDefenceTrait(traitType)` (a `float` in Java).
    pub defence_trait: f32,
}

/// Java `Formulas.calcGeneralTraitBonus` for a type-2 trait with `ignoreResistance = true`.
fn general_trait_bonus(t: &TraitInput) -> f64 {
    if t.target_invul {
        return 0.0;
    }
    if !t.attacker_has_attack_trait || !t.target_has_defence_trait {
        return 1.0;
    }
    // float - float stays a float in Java, only the `+ 1.0` widens it.
    let result = f64::from(t.attack_trait - t.defence_trait) + 1.0;
    result.clamp(0.05, 2.0)
}

/// Java `Formulas.calcAttackTraitBonus(attacker, target)`.
///
/// `weapon_defence_trait` is `target.getStat().getDefenceTraits()[attacker.getAttackType().getTraitType().ordinal()]`
/// (a `float`); `weaknesses` are the type-2 traits in `TraitType.values()` order.
pub fn calc_attack_trait_bonus(weapon_defence_trait: f32, weaknesses: &[TraitInput]) -> f64 {
    // calcWeaponTraitBonus
    let weapon_trait_bonus = 1.0 - (f64::from(weapon_defence_trait) - 1.0);
    if weapon_trait_bonus == 0.0 {
        return 0.0;
    }

    let mut weakness_bonus = 1.0_f64;
    for t in weaknesses {
        weakness_bonus *= general_trait_bonus(t);
        if weakness_bonus == 0.0 {
            return 0.0;
        }
    }

    (weapon_trait_bonus * weakness_bonus).clamp(0.05, 2.0)
}

/// Java `Formulas.calcLvlBonusMod(attacker, target, skill)`.
///
/// `skill_magic_level` is `skill.getMagicLevel()`, used instead of the attacker's level when it is
/// above 0; `skill_lvl_bonus_rate` is `skill.getLvlBonusRate()`.
pub fn calc_lvl_bonus_mod(
    skill_magic_level: i32,
    skill_lvl_bonus_rate: i32,
    attacker_level: i32,
    target_level: i32,
) -> f64 {
    let attacker_lvl = if skill_magic_level > 0 {
        skill_magic_level
    } else {
        attacker_level
    };
    let skill_level_bonus_rate_mod = 1.0 + (f64::from(skill_lvl_bonus_rate) / 100.0);
    let lvl_mod = 1.0 + (f64::from(attacker_lvl.wrapping_sub(target_level)) / 100.0);
    skill_level_bonus_rate_mod * lvl_mod
}

/// `penalties[level_diff]`, or the last entry when `level_diff` is past the end (Java's three
/// `NPC_*_DMG_PENALTY` lookups). An empty array would throw in Java; here it changes nothing.
fn level_penalty(penalties: &[f32], level_diff: i32) -> f64 {
    let Some(last) = penalties.last() else {
        return 1.0;
    };
    let index = usize::try_from(level_diff).unwrap_or(0);
    f64::from(*penalties.get(index).unwrap_or(last))
}

/// Java `Formulas.calcPhysDam(attacker, target, null, shld, crit, ss)`: the damage of a normal
/// attack as a double (callers cast it with `(int)`).
///
/// `shld` is the result of `calc_shld_use`; a perfect block returns 1 before any draw. Otherwise
/// the random damage multiplier is drawn **twice** (see the quirk below), or once when
/// `randomize_auto_attack_damage` is off.
pub fn calc_phys_dam(input: &PhysDamInput<'_>, shld: u8, crit: bool, ss: bool, rng: &mut impl Rnd) -> f64 {
    let is_pvp = input.attacker_is_playable && input.target_is_playable;
    let is_pve = input.attacker_is_playable && input.target_is_attackable;
    let proximity_bonus = match input.position {
        Position::Back => 1.2,  // Behind: +20%
        Position::Front => 1.0, //
        Position::Side => 1.1,  // Side: +10%
    };
    let mut damage = input.p_atk;
    let mut defence = input.p_def;

    // Defense bonuses in PvP fight.
    if is_pvp {
        // NOTE(java-quirk): a normal attack (`skill == null`) still uses the *skill* defence
        // multiplier of the class balance config here.
        defence *= input.pvp_physical_def;
        defence *= f64::from(input.class_balance.pvp_physical_skill_defence);
    }

    match shld {
        SHIELD_DEFENSE_SUCCEED => {
            if !input.alt_game_shield_blocks {
                defence += f64::from(input.shld_def);
            }
        }
        SHIELD_DEFENSE_PERFECT_BLOCK => return 1.0,
        _ => {}
    }

    // Add soulshot boost.
    let ss_boost = if ss { 2 } else { 1 };
    damage *= f64::from(ss_boost);
    if crit {
        // H5 Damage Formula.
        damage = 2.0
            * input.critical_damage
            * input.critical_damage_pos
            * input.defence_critical_damage
            * ((76.0 * damage * proximity_bonus) / defence);
        damage += (input.critical_damage_add * 77.0) / defence;
        damage += input.defence_critical_damage_add;

        if is_pvp {
            damage *= f64::from(input.class_balance.pvp_physical_attack_critical_damage);
        } else if is_pve {
            damage *= f64::from(input.class_balance.pve_physical_attack_critical_damage);
        }
    } else {
        damage = (76.0 * damage * proximity_bonus) / defence;
    }

    damage *= input.attack_trait_bonus;

    // Weapon random damage.
    if input.randomize_auto_attack_damage {
        damage *= random_damage_multiplier(
            input.attacker_has_active_weapon,
            input.random_damage,
            input.attacker_level,
            rng,
        );
    }

    // NOTE(java-quirk): the multiplier is applied a second time, whatever the config says, so a
    // normal attack varies by up to twice the weapon's random damage (and draws twice).
    damage *= random_damage_multiplier(
        input.attacker_has_active_weapon,
        input.random_damage,
        input.attacker_level,
        rng,
    );
    if shld > 0 && input.alt_game_shield_blocks {
        damage -= f64::from(input.shld_def);
        if damage < 0.0 {
            damage = 0.0;
        }
    }

    if damage > 0.0 && damage < 1.0 {
        damage = 1.0;
    } else if damage < 0.0 {
        damage = 0.0;
    }

    // Dmg bonuses in PvP fight.
    if is_pvp {
        damage *= input.pvp_physical_dmg;
        damage *= f64::from(input.class_balance.pvp_physical_attack_damage);
    }

    damage *= calc_attribute_bonus(input.attack_attribute, input.defence_attribute);
    if input.target_is_attackable {
        if input.attacker_weapon_is_bow_or_crossbow {
            damage *= input.pve_bow_dmg;
        } else {
            damage *= input.pve_physical_dmg;

            if input.attacker_is_playable {
                damage *= f64::from(input.class_balance.pve_physical_attack_damage);
            }
        }

        if !input.target_is_raid
            && !input.target_is_raid_minion
            && input.target_level >= input.min_npc_level_dmg_penalty
        {
            if let Some(attacker_level) = input.attacker_player_level {
                if input.target_level.wrapping_sub(attacker_level) >= 2 {
                    let level_diff = input.target_level.wrapping_sub(attacker_level).wrapping_sub(1);
                    damage *= if crit {
                        level_penalty(input.npc_crit_dmg_penalty, level_diff)
                    } else {
                        level_penalty(input.npc_dmg_penalty, level_diff)
                    };
                }
            }
        }
    }

    damage
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::rng::testing::Fixed;

    const NPC_DMG: [f32; 4] = [0.7, 0.6, 0.6, 0.55];
    const NPC_CRIT: [f32; 4] = [0.75, 0.65, 0.6, 0.58];

    /// A level 40 player hitting a level 40 monster with a weapon without random damage.
    fn input() -> PhysDamInput<'static> {
        PhysDamInput {
            attacker_is_playable: true,
            target_is_playable: false,
            target_is_attackable: true,
            position: Position::Front,
            p_atk: 100.0,
            p_def: 76.0,
            shld_def: 0,
            pvp_physical_def: 1.0,
            critical_damage: 1.0,
            critical_damage_pos: 1.0,
            defence_critical_damage: 1.0,
            critical_damage_add: 0.0,
            defence_critical_damage_add: 0.0,
            attack_trait_bonus: 1.0,
            attacker_has_active_weapon: true,
            random_damage: 0,
            attacker_level: 40,
            pvp_physical_dmg: 1.0,
            attack_attribute: 0,
            defence_attribute: 0,
            attacker_weapon_is_bow_or_crossbow: false,
            pve_bow_dmg: 1.0,
            pve_physical_dmg: 1.0,
            target_is_raid: false,
            target_is_raid_minion: false,
            target_level: 40,
            attacker_player_level: Some(40),
            class_balance: PhysDamClassBalance::default(),
            alt_game_shield_blocks: false,
            randomize_auto_attack_damage: true,
            min_npc_level_dmg_penalty: 78,
            npc_dmg_penalty: &NPC_DMG,
            npc_crit_dmg_penalty: &NPC_CRIT,
        }
    }

    fn dmg(input: &PhysDamInput<'_>, shld: u8, crit: bool, ss: bool, draws: &[i32]) -> f64 {
        let mut rng = Fixed::new(draws);
        let d = calc_phys_dam(input, shld, crit, ss, &mut rng);
        assert!(rng.exhausted(), "all scripted draws used");
        d
    }

    #[test]
    fn plain_hit_is_76_p_atk_over_p_def() {
        // 76 * 100 / 76 = 100; no random damage, so the multiplier draws nothing.
        assert_eq!(dmg(&input(), 0, false, false, &[]), 100.0);
        assert_eq!(dmg(&input(), 0, false, true, &[]), 200.0);
    }

    #[test]
    fn position_bonus() {
        let mut i = input();
        i.position = Position::Back;
        assert_eq!(dmg(&i, 0, false, false, &[]), (76.0 * 100.0 * 1.2) / 76.0);
        i.position = Position::Side;
        assert_eq!(dmg(&i, 0, false, false, &[]), (76.0 * 100.0 * 1.1) / 76.0);
    }

    #[test]
    fn crit_doubles_with_the_stat_factors() {
        let mut i = input();
        i.critical_damage = 1.25;
        i.critical_damage_add = 76.0;
        i.defence_critical_damage_add = 3.0;
        // 2 * 1.25 * (76 * 100 / 76) = 250; + 76 * 77 / 76 = 77; + 3.
        assert_eq!(dmg(&i, 0, true, false, &[]), 250.0 + 77.0 + 3.0);
    }

    #[test]
    fn perfect_block_is_one_before_any_draw() {
        assert_eq!(dmg(&input(), 2, true, true, &[]), 1.0);
    }

    #[test]
    fn shield_adds_to_defence_or_subtracts_from_damage() {
        let mut i = input();
        i.shld_def = 24;
        // defence 76 + 24 = 100: 76 * 100 / 100 = 76.
        assert_eq!(dmg(&i, 1, false, false, &[]), 76.0);
        i.alt_game_shield_blocks = true;
        // Config on: damage 100 - 24 instead.
        assert_eq!(dmg(&i, 1, false, false, &[]), 76.0);
        // Not blocked: nothing is subtracted.
        assert_eq!(dmg(&i, 0, false, false, &[]), 100.0);
        // Never below zero.
        i.shld_def = 1000;
        assert_eq!(dmg(&i, 1, false, false, &[]), 0.0);
    }

    #[test]
    fn random_damage_is_drawn_twice() {
        let mut i = input();
        i.random_damage = 10;
        // NOTE(java-quirk) of calc_phys_dam: two draws, each 1 + r/100.
        let d = dmg(&i, 0, false, false, &[10, -10]);
        assert_eq!(d, 100.0 * 1.1 * 0.9);
        // With the config off only the unconditional draw is left.
        i.randomize_auto_attack_damage = false;
        assert_eq!(dmg(&i, 0, false, false, &[5]), 100.0 * 1.05);
    }

    #[test]
    fn without_weapon_random_damage_comes_from_the_level() {
        // 5 + (int) sqrt(40) = 11: Rnd.get(-11, 11).
        assert_eq!(
            random_damage_multiplier(false, 99, 40, &mut Fixed::new(&[11])),
            1.11
        );
        // With a weapon the stat decides; 0 draws nothing.
        assert_eq!(random_damage_multiplier(true, 0, 40, &mut Fixed::new(&[])), 1.0);
    }

    #[test]
    fn tiny_damage_rounds_up_to_one() {
        let mut i = input();
        i.p_atk = 0.5;
        assert_eq!(dmg(&i, 0, false, false, &[]), 1.0);
        i.p_atk = 0.0;
        assert_eq!(dmg(&i, 0, false, false, &[]), 0.0);
    }

    #[test]
    fn pvp_uses_the_pvp_stats_and_class_balance() {
        let mut i = input();
        i.target_is_playable = true;
        i.target_is_attackable = false;
        i.pvp_physical_def = 2.0;
        i.pvp_physical_dmg = 1.5;
        i.class_balance.pvp_physical_skill_defence = 0.5;
        i.class_balance.pvp_physical_attack_damage = 2.0;
        // defence 76 * 2 * 0.5 = 76; damage 100 * 1.5 * 2 = 300.
        assert_eq!(dmg(&i, 0, false, false, &[]), 300.0);
    }

    #[test]
    fn pve_bow_and_physical_multipliers() {
        let mut i = input();
        i.pve_physical_dmg = 1.5;
        i.class_balance.pve_physical_attack_damage = 2.0;
        assert_eq!(dmg(&i, 0, false, false, &[]), 300.0);
        i.attacker_weapon_is_bow_or_crossbow = true;
        i.pve_bow_dmg = 1.2;
        assert_eq!(dmg(&i, 0, false, false, &[]), 100.0 * 1.2);
    }

    #[test]
    fn level_penalty_for_high_level_monsters() {
        let mut i = input();
        i.target_level = 80;
        i.attacker_player_level = Some(76);
        // Difference 4 -> index 3.
        assert_eq!(dmg(&i, 0, false, false, &[]), 100.0 * f64::from(0.55_f32));
        assert_eq!(dmg(&i, 0, true, false, &[]), 100.0 * 2.0 * f64::from(0.58_f32));
        // A bigger gap uses the last entry; a gap of 1 or 2 levels is not penalised.
        i.attacker_player_level = Some(40);
        assert_eq!(dmg(&i, 0, false, false, &[]), 100.0 * f64::from(0.55_f32));
        i.attacker_player_level = Some(79);
        assert_eq!(dmg(&i, 0, false, false, &[]), 100.0);
        // Below the minimum monster level, raids and non-players: no penalty.
        i.attacker_player_level = Some(70);
        i.target_is_raid = true;
        assert_eq!(dmg(&i, 0, false, false, &[]), 100.0);
        i.target_is_raid = false;
        i.attacker_player_level = None;
        assert_eq!(dmg(&i, 0, false, false, &[]), 100.0);
        i.attacker_player_level = Some(70);
        i.target_level = 77;
        assert_eq!(dmg(&i, 0, false, false, &[]), 100.0);
    }

    #[test]
    fn attribute_bonus() {
        assert_eq!(calc_attribute_bonus(0, 0), 1.0);
        assert_eq!(calc_attribute_bonus(20, 20), 1.0);
        // diff 4: 1.025 + sqrt(64 / 2) * 0.0001
        assert_eq!(calc_attribute_bonus(24, 20), 1.025 + (32.0_f64.sqrt() * 0.0001));
        assert_eq!(calc_attribute_bonus(20, 24), 0.975 - (32.0_f64.sqrt() * 0.0001));
        // Capped.
        assert_eq!(calc_attribute_bonus(100_000, 0), 1.25);
        assert_eq!(calc_attribute_bonus(0, 100_000), 0.75);
    }

    #[test]
    fn trait_bonus() {
        let none = TraitInput {
            target_invul: false,
            attacker_has_attack_trait: false,
            target_has_defence_trait: false,
            attack_trait: 1.0,
            defence_trait: 1.0,
        };
        assert_eq!(calc_attack_trait_bonus(1.0, &[none, none]), 1.0);
        // A weapon resistance of 1.25 (defence trait) leaves 0.75.
        assert_eq!(calc_attack_trait_bonus(1.25, &[]), 0.75);
        // A fully immune weapon type gives 0, whatever the weaknesses.
        assert_eq!(calc_attack_trait_bonus(2.0, &[]), 0.0);
        let weak = TraitInput {
            attacker_has_attack_trait: true,
            target_has_defence_trait: true,
            attack_trait: 1.5,
            defence_trait: 1.0,
            ..none
        };
        assert_eq!(calc_attack_trait_bonus(1.0, &[weak]), f64::from(0.5_f32) + 1.0);
        let invul = TraitInput {
            target_invul: true,
            ..none
        };
        assert_eq!(calc_attack_trait_bonus(1.0, &[weak, invul]), 0.0);
        // Clamped to [0.05, 2].
        let huge = TraitInput {
            attack_trait: 9.0,
            ..weak
        };
        assert_eq!(calc_attack_trait_bonus(1.0, &[huge]), 2.0);
        let tiny = TraitInput {
            attack_trait: -9.0,
            ..weak
        };
        assert_eq!(calc_attack_trait_bonus(1.0, &[tiny]), 0.05);
    }

    #[test]
    fn lvl_bonus_mod() {
        // (1 + 2/100) * (1 + (23 - 1)/100)
        assert_eq!(calc_lvl_bonus_mod(23, 2, 1, 1), 1.02 * 1.22);
        // Magic level 0: the attacker's level.
        assert_eq!(calc_lvl_bonus_mod(0, 2, 76, 80), 1.02 * 0.96);
    }
}
