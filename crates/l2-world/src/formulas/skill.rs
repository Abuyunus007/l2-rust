//! Physical skill formulas: skill damage, blow and backstab, skill crit, blow success, evasion,
//! cast and reuse time, resurrection restore.
//!
//! Java: `mechanics/stats/Formulas.java` `calcPhysDam(.., skill, ..)`, `calcBlowDamage`,
//! `calcBackstabDamage`, `calcCrit(attacker, target, skill)`, `calcBlowSuccess`,
//! `calcPhysicalSkillEvasion`, `calculateSkillResurrectRestorePercent`;
//! `entity/actor/Creature.java` `beginCast` (the pure-number lines);
//! `mechanics/skill/Skill.java` `getPower(creature, target, isPvP, isPvE)`.
//!
//! Same pattern as [`damage`](super::damage): plain inputs named after the Java getters, the
//! [`Rnd`] trait for the draws, quirks marked `NOTE(java-quirk)`.

use super::damage::{level_penalty, random_damage_multiplier};
use super::hit::{CritInput, calc_crit};
use super::speed::calc_atk_spd;
use super::{CalcStat, Position, Rnd, SHIELD_DEFENSE_PERFECT_BLOCK, SHIELD_DEFENSE_SUCCEED};

// ------------------------------------------------------------------ small Java helpers

/// Java `Math.max(double, double)` (a NaN wins, unlike `f64::max`).
pub(crate) fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// Java `Math.min(double, double)` (a NaN wins, unlike `f64::min`).
pub(crate) fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// Java `Math.clamp(double value, double min, double max)` for `min <= max` (Java throws
/// otherwise, which no skill data does): `Math.min(max, Math.max(value, min))`.
pub(crate) fn java_clamp(value: f64, min: f64, max: f64) -> f64 {
    java_min(max, java_max(value, min))
}

/// Java `Math.round(float)`: `floor(a + 0.5)`, saturated to `int`, NaN is 0. The sum is exact in
/// `f64` for every `f32`.
pub(crate) fn java_round_f32(a: f32) -> i32 {
    (f64::from(a) + 0.5).floor() as i32
}

/// The penalty of the `NPC_SKILL_DMG_PENALTY`-style tables: `1` unless the target is a monster of
/// at least `min_npc_level` that is not a raid or raid minion and the attacking player is at least
/// `min_level_diff` levels below it; then `penalties[target - player - (min_level_diff - 1)]` (the
/// last entry past the end).
///
/// The callers check `target.isAttackable()` themselves (Java does it in different places).
pub(crate) fn npc_level_penalty(
    penalties: &[f32],
    target_level: i32,
    attacker_player_level: Option<i32>,
    min_npc_level: i32,
    min_level_diff: i32,
    target_is_raid: bool,
    target_is_raid_minion: bool,
) -> f64 {
    if target_is_raid || target_is_raid_minion || target_level < min_npc_level {
        return 1.0;
    }
    let Some(attacker_level) = attacker_player_level else {
        return 1.0;
    };
    let diff = target_level.wrapping_sub(attacker_level);
    if diff < min_level_diff {
        return 1.0;
    }
    level_penalty(penalties, diff.wrapping_sub(min_level_diff - 1))
}

/// `Skill.getPower(creature, target, isPvP, isPvE)`: the power the damage formulas use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillPower {
    /// `skill.getPower()` (a Java `float`, widened).
    pub power: f64,
    /// `skill.getPower(true, false)`.
    pub pvp_power: f64,
    /// `skill.getPower(false, true)`.
    pub pve_power: f64,
    /// `Some((current HP, max HP))` of the creature a skill with a `DEATH_LINK` (the attacker) or
    /// `PHYSICAL_ATTACK_HP_LINK` (the target) effect looks at; `None` for every other skill and
    /// for the overloads without creatures (`getPower(isPvP, isPvE)`).
    pub hp_link: Option<(f64, i32)>,
}

impl SkillPower {
    /// A skill whose three powers are the same and that has no HP link.
    pub fn flat(power: f64) -> Self {
        Self {
            power,
            pvp_power: power,
            pve_power: power,
            hp_link: None,
        }
    }

    /// Java `getPower(isPvP, isPvE)` (PvE wins over PvP), scaled by the HP link when there is one:
    /// `power * (-((hp * 2) / maxHp) + 2)`.
    pub fn get(&self, is_pvp: bool, is_pve: bool) -> f64 {
        let base = if is_pve {
            self.pve_power
        } else if is_pvp {
            self.pvp_power
        } else {
            self.power
        };
        match self.hp_link {
            Some((hp, max_hp)) => base * (-((hp * 2.0) / f64::from(max_hp)) + 2.0),
            None => base,
        }
    }
}

/// Java `Formulas.calculateSkillResurrectRestorePercent(baseRestorePercent, caster)`;
/// `wit_bonus` is `BaseStat.WIT.calcBonus(caster)`.
pub fn calculate_skill_resurrect_restore_percent(base_restore_percent: f64, wit_bonus: f64) -> f64 {
    if base_restore_percent == 0.0 || base_restore_percent == 100.0 {
        return base_restore_percent;
    }

    let mut restore_percent = base_restore_percent * wit_bonus;
    if (restore_percent - base_restore_percent) > 20.0 {
        restore_percent += 20.0;
    }

    restore_percent = java_max(restore_percent, base_restore_percent);
    restore_percent = java_min(restore_percent, 90.0);

    restore_percent
}

// ------------------------------------------------------------------ physical skill damage

/// The `ClassBalanceConfig` multipliers `calcPhysDam(.., skill != null, ..)` reads (each the
/// entry of the class Java indexes with; `float` in Java, 1.0 by default).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillPhysDamClassBalance {
    /// `PVP_PHYSICAL_SKILL_DEFENCE_MULTIPLIERS[target class]`.
    pub pvp_physical_skill_defence: f32,
    /// `PVP_PHYSICAL_SKILL_CRITICAL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pvp_physical_skill_critical_damage: f32,
    /// `PVE_PHYSICAL_SKILL_CRITICAL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pve_physical_skill_critical_damage: f32,
    /// `PVP_PHYSICAL_SKILL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pvp_physical_skill_damage: f32,
    /// `PVE_PHYSICAL_SKILL_DAMAGE_MULTIPLIERS[attacker class]` (non-bow weapons, playable attacker).
    pub pve_physical_skill_damage: f32,
}

impl Default for SkillPhysDamClassBalance {
    fn default() -> Self {
        Self {
            pvp_physical_skill_defence: 1.0,
            pvp_physical_skill_critical_damage: 1.0,
            pve_physical_skill_critical_damage: 1.0,
            pvp_physical_skill_damage: 1.0,
            pve_physical_skill_damage: 1.0,
        }
    }
}

/// What `Formulas.calcPhysDam(attacker, target, skill, shld, crit, ss)` reads when `skill != null`.
#[derive(Clone, Copy)]
pub struct SkillPhysDamInput<'a> {
    /// `attacker.isPlayable()`.
    pub attacker_is_playable: bool,
    /// `target.isPlayable()`.
    pub target_is_playable: bool,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// Where the attacker stands (`isBehind` first, then `isInFrontOf`).
    pub position: Position,
    /// `attacker.getPAtk(target)`.
    pub p_atk: f64,
    /// `target.getPDef(attacker)`.
    pub p_def: f64,
    /// `target.getShldDef()`.
    pub shld_def: i32,
    /// `target.calcStat(Stat.PVP_PHYS_SKILL_DEF, 1, null, null)`.
    pub pvp_phys_skill_def: f64,
    /// `skill.getPower(attacker, target, isPvP, isPvE)`.
    pub skill_power: SkillPower,
    /// `attacker.calcStat(Stat.CRITICAL_DAMAGE, 1, target, skill)`.
    pub critical_damage: f64,
    /// `attacker.calcStat(Stat.CRITICAL_DAMAGE_POS, 1, target, skill)`.
    pub critical_damage_pos: f64,
    /// `target.calcStat(Stat.DEFENCE_CRITICAL_DAMAGE, 1, target, null)`.
    pub defence_critical_damage: f64,
    /// `attacker.calcStat(Stat.CRITICAL_DAMAGE_ADD, 0, target, skill)`.
    pub critical_damage_add: f64,
    /// `target.calcStat(Stat.DEFENCE_CRITICAL_DAMAGE_ADD, 0, target, skill)`.
    pub defence_critical_damage_add: f64,
    /// `Formulas.calcAttackTraitBonus(attacker, target)`.
    pub attack_trait_bonus: f64,
    /// `PlayerConfig.RANDOMIZE_PHYSICAL_SKILL_DAMAGE`.
    pub randomize_physical_skill_damage: bool,
    /// `attacker.getActiveWeaponItem() != null`.
    pub attacker_has_active_weapon: bool,
    /// `(int) attacker.getStat().calcStat(Stat.RANDOM_DAMAGE, 0)`.
    pub random_damage: i32,
    /// `attacker.getLevel()`.
    pub attacker_level: i32,
    /// `attacker.calcStat(Stat.PVP_PHYS_SKILL_DMG, 1, null, null)`.
    pub pvp_phys_skill_dmg: f64,
    /// `attacker.calcStat(Stat.PHYSICAL_SKILL_POWER, damage, null, null)` (base = the damage).
    pub physical_skill_power: CalcStat<'a>,
    /// `Formulas.calcAttributeBonus(attacker, target, skill)`.
    pub attribute_bonus: f64,
    /// The attacker's active weapon is a bow or a crossbow.
    pub attacker_weapon_is_bow_or_crossbow: bool,
    /// `attacker.calcStat(Stat.PVE_BOW_SKILL_DMG, 1, null, null)`.
    pub pve_bow_skill_dmg: f64,
    /// `attacker.calcStat(Stat.PVE_PHYSICAL_DMG, 1, null, null)`.
    pub pve_physical_dmg: f64,
    /// `target.isRaid()`.
    pub target_is_raid: bool,
    /// `target.isRaidMinion()`.
    pub target_is_raid_minion: bool,
    /// `target.getLevel()`.
    pub target_level: i32,
    /// `attacker.asPlayer().getLevel()`, `None` when `asPlayer()` is null.
    pub attacker_player_level: Option<i32>,
    /// The class balance multipliers.
    pub class_balance: SkillPhysDamClassBalance,
    /// `PlayerConfig.ALT_GAME_SHIELD_BLOCKS`.
    pub alt_game_shield_blocks: bool,
    /// `NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY`.
    pub min_npc_level_dmg_penalty: i32,
    /// `NpcConfig.NPC_SKILL_DMG_PENALTY`.
    pub npc_skill_dmg_penalty: &'a [f32],
}

/// Java `Formulas.calcPhysDam(attacker, target, skill, shld, crit, ss)` for a skill: the damage
/// as a double (callers cast it with `(int)`). `crit` is the result of [`calc_skill_crit`].
///
/// A perfect block returns 1 before any draw. Otherwise the random damage multiplier is drawn
/// twice, or once when `randomize_physical_skill_damage` is off.
pub fn calc_skill_phys_dam(
    input: &SkillPhysDamInput<'_>,
    shld: u8,
    crit: bool,
    ss: bool,
    rng: &mut impl Rnd,
) -> f64 {
    let is_pvp = input.attacker_is_playable && input.target_is_playable;
    let is_pve = input.attacker_is_playable && input.target_is_attackable;
    let proximity_bonus = match input.position {
        Position::Back => 1.2,
        Position::Front => 1.0,
        Position::Side => 1.1,
    };
    let mut damage = input.p_atk;
    let mut defence = input.p_def;

    // Defense bonuses in PvP fight.
    if is_pvp {
        defence *= input.pvp_phys_skill_def;
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
    damage = (damage * f64::from(ss_boost)) + input.skill_power.get(is_pvp, is_pve);
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
            damage *= f64::from(input.class_balance.pvp_physical_skill_critical_damage);
        } else if is_pve {
            damage *= f64::from(input.class_balance.pve_physical_skill_critical_damage);
        }
    } else {
        damage = (76.0 * damage * proximity_bonus) / defence;
    }

    damage *= input.attack_trait_bonus;

    // Weapon random damage.
    if input.randomize_physical_skill_damage {
        damage *= random_damage_multiplier(
            input.attacker_has_active_weapon,
            input.random_damage,
            input.attacker_level,
            rng,
        );
    }

    // NOTE(java-quirk): the multiplier is applied a second time, whatever the config says, so a
    // skill varies by up to twice the weapon's random damage (and draws twice).
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
        damage *= input.pvp_phys_skill_dmg;
        damage *= f64::from(input.class_balance.pvp_physical_skill_damage);
    }

    // Physical skill dmg boost.
    damage = (input.physical_skill_power)(damage);

    damage *= input.attribute_bonus;
    if input.target_is_attackable {
        if input.attacker_weapon_is_bow_or_crossbow {
            damage *= input.pve_bow_skill_dmg;
        } else {
            damage *= input.pve_physical_dmg;

            if input.attacker_is_playable {
                damage *= f64::from(input.class_balance.pve_physical_skill_damage);
            }
        }

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

    damage
}

// ------------------------------------------------------------------ blow and backstab

/// The `ClassBalanceConfig` multipliers `calcBlowDamage` / `calcBackstabDamage` read. For a blow
/// they are the `*_BLOW_SKILL_*` tables, for a backstab the `PVP_PHYSICAL_SKILL_*` ones (it has no
/// PvE entries, those two are ignored). 1.0 by default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlowClassBalance {
    /// `PVP_BLOW_SKILL_DAMAGE_MULTIPLIERS` / `PVP_PHYSICAL_SKILL_DAMAGE_MULTIPLIERS[attacker class]`.
    pub pvp_skill_damage: f32,
    /// `PVP_BLOW_SKILL_DEFENCE_MULTIPLIERS` / `PVP_PHYSICAL_SKILL_DEFENCE_MULTIPLIERS[target class]`.
    pub pvp_skill_defence: f32,
    /// `PVE_BLOW_SKILL_DAMAGE_MULTIPLIERS[attacker class]` (blow only).
    pub pve_skill_damage: f32,
    /// `PVE_BLOW_SKILL_DEFENCE_MULTIPLIERS[target class]` (blow only).
    pub pve_skill_defence: f32,
}

impl Default for BlowClassBalance {
    fn default() -> Self {
        Self {
            pvp_skill_damage: 1.0,
            pvp_skill_defence: 1.0,
            pve_skill_damage: 1.0,
            pve_skill_defence: 1.0,
        }
    }
}

/// What `Formulas.calcBlowDamage` and `Formulas.calcBackstabDamage` read.
#[derive(Debug, Clone, Copy)]
pub struct BlowInput<'a> {
    /// `attacker.isPlayable()`.
    pub attacker_is_playable: bool,
    /// `attacker.isAttackable()` (a blow only).
    pub attacker_is_attackable: bool,
    /// `target.isPlayable()` (a blow's PvP test and its `PVE_BLOW_SKILL_DEFENCE` test).
    pub target_is_playable: bool,
    /// `target.isPlayer()` (a backstab's PvP test).
    pub target_is_player: bool,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// Where the attacker stands: behind +20 %, side +10 %.
    pub position: Position,
    /// `attacker.getPAtk(target)`.
    pub p_atk: f64,
    /// `target.getPDef(attacker)`.
    pub p_def: f64,
    /// `target.getShldDef()`.
    pub shld_def: i32,
    /// `skill.getPower(isPvP, isPvE)` (no HP link in the overload these two use).
    pub skill_power: SkillPower,
    /// `attacker.calcStat(Stat.PVP_PHYS_SKILL_DMG, 1, null, null)`.
    pub pvp_phys_skill_dmg: f64,
    /// `target.calcStat(Stat.PVP_PHYS_SKILL_DEF, 1, null, null)`.
    pub pvp_phys_skill_def: f64,
    /// The class balance multipliers.
    pub class_balance: BlowClassBalance,
    /// `attacker.calcStat(Stat.CRITICAL_DAMAGE, 1, target, skill)`.
    pub critical_damage: f64,
    /// `attacker.calcStat(Stat.CRITICAL_DAMAGE_POS, 1, target, skill)`.
    pub critical_damage_pos: f64,
    /// `target.calcStat(Stat.DEFENCE_CRITICAL_DAMAGE, 1, target, skill)`.
    pub defence_critical_damage: f64,
    /// `attacker.getStat().calcStat(Stat.CRITICAL_DAMAGE_ADD, 0)` for a blow (no target, no
    /// skill), `attacker.calcStat(Stat.CRITICAL_DAMAGE_ADD, 0, target, skill)` for a backstab.
    pub critical_damage_add: f64,
    /// `target.calcStat(Stat.DEFENCE_CRITICAL_DAMAGE_ADD, 0, target, skill)`.
    pub defence_critical_damage_add: f64,
    /// `Formulas.calcWeaponTraitBonus(attacker, target)` (a blow only).
    pub weapon_trait_bonus: f64,
    /// `Formulas.calcGeneralTraitBonus(attacker, target, skill.getTraitType(), false)`.
    pub general_trait_bonus: f64,
    /// `Formulas.calcAttributeBonus(attacker, target, skill)`.
    pub attribute_bonus: f64,
    /// `PlayerConfig.RANDOMIZE_PHYSICAL_SKILL_DAMAGE`.
    pub randomize_physical_skill_damage: bool,
    /// `attacker.getActiveWeaponItem() != null`.
    pub attacker_has_active_weapon: bool,
    /// `(int) attacker.getStat().calcStat(Stat.RANDOM_DAMAGE, 0)`.
    pub random_damage: i32,
    /// `attacker.getLevel()`.
    pub attacker_level: i32,
    /// `target.isRaid()`.
    pub target_is_raid: bool,
    /// `target.isRaidMinion()`.
    pub target_is_raid_minion: bool,
    /// `target.getLevel()`.
    pub target_level: i32,
    /// `attacker.asPlayer().getLevel()`, `None` when `asPlayer()` is null.
    pub attacker_player_level: Option<i32>,
    /// `NpcConfig.MIN_NPC_LEVEL_DMG_PENALTY`.
    pub min_npc_level_dmg_penalty: i32,
    /// `NpcConfig.NPC_SKILL_DMG_PENALTY`.
    pub npc_skill_dmg_penalty: &'a [f32],
}

fn proximity_bonus(position: Position) -> f64 {
    match position {
        Position::Back => 1.2,
        Position::Front => 1.0,
        Position::Side => 1.1,
    }
}

/// The part of `calcBlowDamage` and `calcBackstabDamage` after the base damage, which is the same
/// (without the weapon trait of a backstab: the caller passes 1 for it).
fn blow_tail(
    input: &BlowInput<'_>,
    base_damage_with_crit: f64,
    critical_add_mod: f64,
    rng: &mut impl Rnd,
) -> f64 {
    // Random weapon damage for physical skills.
    let weapon_mod = if input.randomize_physical_skill_damage {
        random_damage_multiplier(
            input.attacker_has_active_weapon,
            input.random_damage,
            input.attacker_level,
            rng,
        )
    } else {
        1.0
    };

    let penalty_mod = if input.target_is_attackable {
        npc_level_penalty(
            input.npc_skill_dmg_penalty,
            input.target_level,
            input.attacker_player_level,
            input.min_npc_level_dmg_penalty,
            2,
            input.target_is_raid,
            input.target_is_raid_minion,
        )
    } else {
        1.0
    };

    let mut damage = base_damage_with_crit + critical_add_mod + input.defence_critical_damage_add;
    damage *= input.weapon_trait_bonus;
    damage *= input.general_trait_bonus;
    damage *= input.attribute_bonus;
    damage *= weapon_mod;
    damage *= penalty_mod;

    java_max(damage, 1.0)
}

/// `((pos - 1) / 2) + 1`: the positional critical damage a blow and a backstab use.
fn critical_mod_pos(critical_damage_pos: f64) -> f64 {
    ((critical_damage_pos - 1.0) / 2.0) + 1.0
}

/// Java `Formulas.calcBlowDamage(attacker, target, skill, shld, ss)`: a blow skill's damage, at
/// least 1. A perfect block returns 1 before any draw; otherwise one random damage draw (when
/// `randomize_physical_skill_damage` is on).
///
/// The extra critical (`calcCrit` with the skill, x2) of the blow effects is not part of it.
pub fn calc_blow_damage(input: &BlowInput<'_>, shld: u8, ss: bool, rng: &mut impl Rnd) -> f64 {
    let mut defence = input.p_def;

    match shld {
        SHIELD_DEFENSE_SUCCEED => defence += f64::from(input.shld_def),
        SHIELD_DEFENSE_PERFECT_BLOCK => return 1.0,
        _ => {}
    }

    let is_pvp = input.attacker_is_playable && input.target_is_playable;
    let is_pve = input.attacker_is_playable && input.target_is_attackable;
    let power = input.skill_power.get(is_pvp, is_pve);
    let proximity_bonus = proximity_bonus(input.position);
    let ssboost = if ss { 1.458 } else { 1.0 };
    let mut pvp_bonus = 1.0_f64;

    if is_pvp {
        // Damage bonuses in PvP fight.
        pvp_bonus = input.pvp_phys_skill_dmg;
        pvp_bonus *= f64::from(input.class_balance.pvp_skill_damage);

        // Defense bonuses in PvP fight.
        defence *= input.pvp_phys_skill_def;
        defence *= f64::from(input.class_balance.pvp_skill_defence);
    }

    if is_pve {
        pvp_bonus *= f64::from(input.class_balance.pve_skill_damage);
    }

    if input.attacker_is_attackable && input.target_is_playable {
        defence *= f64::from(input.class_balance.pve_skill_defence);
    }

    // Initial damage.
    let base_mod = (77.0 * (power + (input.p_atk * ssboost))) / defence;

    // Critical.
    let critical_mod = input.critical_damage;
    let critical_mod_pos = critical_mod_pos(input.critical_damage_pos);
    let critical_vuln_mod = input.defence_critical_damage;
    let critical_add_mod = (input.critical_damage_add * 6.1 * 77.0) / defence;

    let base = base_mod * critical_mod * critical_mod_pos * critical_vuln_mod * proximity_bonus * pvp_bonus;
    blow_tail(input, base, critical_add_mod, rng)
}

/// Java `Formulas.calcBackstabDamage(attacker, target, skill, shld, ss)`: like a blow, but the
/// soulshot multiplies the whole base damage, the PvP test is `target.isPlayer()`, there is no
/// weapon trait and no PvE class balance. `input.weapon_trait_bonus` must be 1.
pub fn calc_backstab_damage(input: &BlowInput<'_>, shld: u8, ss: bool, rng: &mut impl Rnd) -> f64 {
    let mut defence = input.p_def;

    match shld {
        SHIELD_DEFENSE_SUCCEED => defence += f64::from(input.shld_def),
        SHIELD_DEFENSE_PERFECT_BLOCK => return 1.0,
        _ => {}
    }

    // NOTE(java-quirk): a backstab tests `target.isPlayer()` where a blow tests `isPlayable()`.
    let is_pvp = input.attacker_is_playable && input.target_is_player;
    let is_pve = input.attacker_is_playable && input.target_is_attackable;
    let proximity_bonus = proximity_bonus(input.position);
    let ssboost = if ss { 1.458 } else { 1.0 };
    let mut pvp_bonus = 1.0_f64;

    if is_pvp {
        // Damage bonuses in PvP fight.
        pvp_bonus = input.pvp_phys_skill_dmg;
        pvp_bonus *= f64::from(input.class_balance.pvp_skill_damage);

        // Defense bonuses in PvP fight.
        defence *= input.pvp_phys_skill_def;
        defence *= f64::from(input.class_balance.pvp_skill_defence);
    }

    // Initial damage.
    let base_mod = ((77.0 * (input.skill_power.get(is_pvp, is_pve) + input.p_atk)) / defence) * ssboost;

    // Critical.
    let critical_mod = input.critical_damage;
    let critical_mod_pos = critical_mod_pos(input.critical_damage_pos);
    let critical_vuln_mod = input.defence_critical_damage;
    let critical_add_mod = (input.critical_damage_add * 6.1 * 77.0) / defence;

    let base = base_mod * critical_mod * critical_mod_pos * critical_vuln_mod * proximity_bonus * pvp_bonus;
    blow_tail(input, base, critical_add_mod, rng)
}

// ------------------------------------------------------------------ crit, blow success, evasion

/// What `Formulas.calcCrit(attacker, target, skill)` (skill != null) reads.
#[derive(Clone, Copy)]
pub struct SkillCritInput<'a> {
    /// `skill.getBaseCritRate()`.
    pub base_crit_rate: i32,
    /// `BaseStat.STR.calcBonus(attacker)`.
    pub str_bonus: f64,
    /// `attacker.isPlayable()`.
    pub attacker_is_playable: bool,
    /// `target.isPlayable()`.
    pub target_is_playable: bool,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// `ClassBalanceConfig.PVP_PHYSICAL_SKILL_CRITICAL_CHANCE_MULTIPLIERS[attacker class]`.
    pub pvp_physical_skill_critical_chance_multiplier: f32,
    /// `ClassBalanceConfig.PVE_PHYSICAL_SKILL_CRITICAL_CHANCE_MULTIPLIERS[attacker class]`.
    pub pve_physical_skill_critical_chance_multiplier: f32,
    /// `target.getStat().calcStat(Stat.DEFENCE_CRITICAL_RATE, rate, null, null)` (base = the rate).
    pub defence_critical_rate: CalcStat<'a>,
    /// `target.getStat().calcStat(Stat.DEFENCE_CRITICAL_RATE_ADD, 0, null, null)`.
    pub defence_critical_rate_add: f64,
}

/// Java `Formulas.calcCrit(attacker, target, skill)` with a skill: `true` for a critical hit.
/// The rate is `baseCritRate * 10 * STR bonus` (not the attacker's critical stat). One draw,
/// `Rnd.get(1000)`.
pub fn calc_skill_crit(input: &SkillCritInput<'_>, rng: &mut impl Rnd) -> bool {
    // `int * int * double`.
    let rate = f64::from(input.base_crit_rate.wrapping_mul(10)) * input.str_bonus;
    // The rest is the normal-attack check with the skill's class balance tables.
    calc_crit(
        &CritInput {
            critical_rate: rate,
            attacker_is_playable: input.attacker_is_playable,
            target_is_playable: input.target_is_playable,
            target_is_attackable: input.target_is_attackable,
            pvp_physical_attack_critical_chance_multiplier: input
                .pvp_physical_skill_critical_chance_multiplier,
            pve_physical_attack_critical_chance_multiplier: input
                .pve_physical_skill_critical_chance_multiplier,
            defence_critical_rate: input.defence_critical_rate,
            defence_critical_rate_add: input.defence_critical_rate_add,
        },
        rng,
    )
}

/// What `Formulas.calcBlowSuccess(creature, target, skill)` reads.
#[derive(Clone, Copy)]
pub struct BlowSuccessInput<'a> {
    /// `skill.getBlowChance()`.
    pub blow_chance: i32,
    /// `BaseStat.DEX.calcBonus(creature)`.
    pub dex_bonus: f64,
    /// Where the creature stands (`isInFrontOf` 1, `isBehind` 2, else 1.5).
    pub position: Position,
    /// `creature.calcStat(Stat.BLOW_RATE, baseRate, target, null)` (base = the rate).
    pub blow_rate: CalcStat<'a>,
}

/// Java `Formulas.calcBlowSuccess`: `Rnd.get(100) < blowChance * dexMod * sideMod` after the
/// `BLOW_RATE` stat. One draw.
pub fn calc_blow_success(input: &BlowSuccessInput<'_>, rng: &mut impl Rnd) -> bool {
    // Apply Position Bonus (unconfirmed values in Java).
    let side_mod = match input.position {
        Position::Front => 1.0,
        Position::Back => 2.0,
        Position::Side => 1.5,
    };
    let base_rate = f64::from(input.blow_chance) * input.dex_bonus * side_mod;
    let rate = (input.blow_rate)(base_rate);
    f64::from(rng.get_int(100)) < rate
}

/// Java `Formulas.calcPhysicalSkillEvasion(creature, target, skill)`: `true` when the target
/// evades. Magic skills and debuffs never draw; otherwise one `Rnd.get(100)` against
/// `target.calcStat(Stat.P_SKILL_EVASION, 0, null, skill)`.
pub fn calc_physical_skill_evasion(
    skill_is_magic: bool,
    skill_is_debuff: bool,
    p_skill_evasion: f64,
    rng: &mut impl Rnd,
) -> bool {
    if skill_is_magic || skill_is_debuff {
        return false;
    }

    f64::from(rng.get_int(100)) < p_skill_evasion
}

// ------------------------------------------------------------------ cast and reuse time

/// What the casting-time lines of `Creature.beginCast` read.
#[derive(Debug, Clone, Copy)]
pub struct SkillCastTimeInput {
    /// `skill.getHitTime()`.
    pub hit_time: i32,
    /// `skill.getCoolTime()`.
    pub cool_time: i32,
    /// `skill.isMagic()`.
    pub skill_is_magic: bool,
    /// `skill.isStatic()`.
    pub skill_is_static: bool,
    /// `skill.isChanneling()`.
    pub skill_is_channeling: bool,
    /// `skill.getChannelingSkillId()`.
    pub channeling_skill_id: i32,
    /// `isChargedShot(SPIRITSHOTS) || isChargedShot(BLESSED_SPIRITSHOTS)`.
    pub spiritshot_charged: bool,
    /// `getMAtkSpd()`.
    pub m_atk_spd: i32,
    /// `getPAtkSpd()`.
    pub p_atk_spd: f64,
}

/// The casting time in milliseconds `Creature.beginCast` computes before it schedules the hit:
/// `hit + cool` through `Formulas.calcAtkSpd` (not for static skills), x0.6 for a magic skill with
/// a spiritshot, then the 550 ms (magic) / 500 ms (not static) floors.
pub fn calc_skill_cast_time(input: &SkillCastTimeInput) -> i32 {
    // Get the Base Casting Time of the Skills.
    let base = input.hit_time.wrapping_add(input.cool_time);
    let mut skill_time = base;
    if !input.skill_is_channeling || input.channeling_skill_id == 0 {
        // Calculate the Casting Time of the "Non-Static" Skills (with caster PAtk/MAtkSpd).
        if !input.skill_is_static {
            skill_time = calc_atk_spd(
                input.skill_is_magic,
                f64::from(skill_time),
                input.m_atk_spd,
                input.p_atk_spd,
            );
        }

        // Calculate the Casting Time of Magic Skills (reduced in 40% if using SPS/BSPS).
        if input.skill_is_magic && input.spiritshot_charged {
            skill_time = (0.6 * f64::from(skill_time)) as i32;
        }
    }

    // Avoid broken Casting Animation.
    // Client can't handle less than 550ms Casting Animation in Magic Skills with more than 550ms base.
    if input.skill_is_magic && base > 550 && skill_time < 550 {
        skill_time = 550;
    }
    // Client can't handle less than 500ms Casting Animation in Physical Skills with 500ms base or more.
    else if !input.skill_is_static && base >= 500 && skill_time < 500 {
        skill_time = 500;
    }

    skill_time
}

/// What the reuse-delay lines of `Creature.beginCast` read.
#[derive(Debug, Clone, Copy)]
pub struct SkillReuseInput {
    /// `skill.getReuseDelay()`.
    pub reuse_delay: i32,
    /// `skill.isStaticReuse()`.
    pub static_reuse: bool,
    /// `skill.isStatic()`.
    pub skill_is_static: bool,
    /// `skill.isMagic()`.
    pub skill_is_magic: bool,
    /// `skill.isPhysical()`.
    pub skill_is_physical: bool,
    /// `calcStat(Stat.MAGIC_REUSE_RATE, 1, null, null)`.
    pub magic_reuse_rate: f64,
    /// `calcStat(Stat.P_REUSE, 1, null, null)`.
    pub p_reuse: f64,
    /// `calcStat(Stat.DANCE_REUSE, 1, null, null)`.
    pub dance_reuse: f64,
    /// `ClassBalanceConfig.SKILL_REUSE_MULTIPLIERS[class]` when the caster is playable.
    pub skill_reuse_multiplier: Option<f32>,
}

/// The reuse delay in milliseconds `Creature.beginCast` sets (before the skill mastery roll that
/// turns a delay above 10 into 100, see `calc_skill_mastery`).
pub fn calc_skill_reuse_delay(input: &SkillReuseInput) -> i32 {
    // Calculate the Reuse Time of the Skill.
    let mut reuse_delay = if input.static_reuse || input.skill_is_static {
        input.reuse_delay
    } else if input.skill_is_magic {
        (f64::from(input.reuse_delay) * input.magic_reuse_rate) as i32
    } else if input.skill_is_physical {
        (f64::from(input.reuse_delay) * input.p_reuse) as i32
    } else {
        (f64::from(input.reuse_delay) * input.dance_reuse) as i32
    };

    if let Some(multiplier) = input.skill_reuse_multiplier {
        // `int * float`: a float product, narrowed back.
        reuse_delay = (reuse_delay as f32 * multiplier) as i32;
    }

    reuse_delay
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::no_modifiers;
    use crate::formulas::rng::testing::Fixed;

    const SKILL_PENALTY: [f32; 4] = [0.8, 0.7, 0.65, 0.62];

    fn phys() -> SkillPhysDamInput<'static> {
        SkillPhysDamInput {
            attacker_is_playable: true,
            target_is_playable: false,
            target_is_attackable: true,
            position: Position::Front,
            p_atk: 100.0,
            p_def: 76.0,
            shld_def: 0,
            pvp_phys_skill_def: 1.0,
            skill_power: SkillPower::flat(0.0),
            critical_damage: 1.0,
            critical_damage_pos: 1.0,
            defence_critical_damage: 1.0,
            critical_damage_add: 0.0,
            defence_critical_damage_add: 0.0,
            attack_trait_bonus: 1.0,
            randomize_physical_skill_damage: true,
            attacker_has_active_weapon: true,
            random_damage: 0,
            attacker_level: 40,
            pvp_phys_skill_dmg: 1.0,
            physical_skill_power: &no_modifiers,
            attribute_bonus: 1.0,
            attacker_weapon_is_bow_or_crossbow: false,
            pve_bow_skill_dmg: 1.0,
            pve_physical_dmg: 1.0,
            target_is_raid: false,
            target_is_raid_minion: false,
            target_level: 40,
            attacker_player_level: Some(40),
            class_balance: SkillPhysDamClassBalance::default(),
            alt_game_shield_blocks: false,
            min_npc_level_dmg_penalty: 78,
            npc_skill_dmg_penalty: &SKILL_PENALTY,
        }
    }

    fn phys_dam(input: &SkillPhysDamInput<'_>, shld: u8, crit: bool, ss: bool, draws: &[i32]) -> f64 {
        let mut rng = Fixed::new(draws);
        let d = calc_skill_phys_dam(input, shld, crit, ss, &mut rng);
        assert!(rng.exhausted(), "all scripted draws used");
        d
    }

    #[test]
    fn skill_power_is_added_to_the_boosted_p_atk() {
        let mut i = phys();
        i.skill_power = SkillPower::flat(76.0);
        // (100 + 76) * 76 / 76; with a soulshot (100 * 2 + 76).
        assert_eq!(phys_dam(&i, 0, false, false, &[]), 176.0);
        assert_eq!(phys_dam(&i, 0, false, true, &[]), 276.0);
    }

    #[test]
    fn skill_damage_draws_twice_or_once() {
        let mut i = phys();
        i.random_damage = 10;
        assert_eq!(phys_dam(&i, 0, false, false, &[10, -10]), 100.0 * 1.1 * 0.9);
        i.randomize_physical_skill_damage = false;
        assert_eq!(phys_dam(&i, 0, false, false, &[5]), 100.0 * 1.05);
    }

    #[test]
    fn perfect_block_is_one_before_any_draw() {
        assert_eq!(phys_dam(&phys(), 2, true, true, &[]), 1.0);
    }

    #[test]
    fn skill_crit_uses_the_skill_balance_tables() {
        let mut i = phys();
        i.critical_damage_add = 76.0;
        i.class_balance.pve_physical_skill_critical_damage = 0.5;
        // (2 * 100 + 77) * 0.5
        assert_eq!(phys_dam(&i, 0, true, false, &[]), 138.5);
    }

    #[test]
    fn physical_skill_power_scales_the_damage() {
        let mut i = phys();
        let boost = |d: f64| d * 1.2;
        i.physical_skill_power = &boost;
        assert_eq!(phys_dam(&i, 0, false, false, &[]), 100.0 * 1.2);
    }

    #[test]
    fn skill_penalty_for_high_level_monsters() {
        let mut i = phys();
        i.target_level = 80;
        i.attacker_player_level = Some(76);
        // Difference 4 -> index 3.
        assert_eq!(phys_dam(&i, 0, false, false, &[]), 100.0 * f64::from(0.62_f32));
        // Bow skills use the bow stat and the same penalty.
        i.attacker_weapon_is_bow_or_crossbow = true;
        i.pve_bow_skill_dmg = 2.0;
        assert_eq!(
            phys_dam(&i, 0, false, false, &[]),
            100.0 * 2.0 * f64::from(0.62_f32)
        );
    }

    #[test]
    fn skill_power_pvp_pve_and_hp_link() {
        let p = SkillPower {
            power: 10.0,
            pvp_power: 20.0,
            pve_power: 30.0,
            hp_link: None,
        };
        assert_eq!(p.get(false, false), 10.0);
        assert_eq!(p.get(true, false), 20.0);
        assert_eq!(p.get(false, true), 30.0);
        assert_eq!(p.get(true, true), 30.0);
        // Half HP: x (-(1000 * 2 / 2000) + 2) = x 1; full HP: x 0; nearly dead: almost x 2.
        let link = |hp| SkillPower {
            hp_link: Some((hp, 2000)),
            ..SkillPower::flat(10.0)
        };
        assert_eq!(link(1000.0).get(false, false), 10.0);
        assert_eq!(link(2000.0).get(false, false), 0.0);
        assert_eq!(link(0.0).get(false, false), 20.0);
    }

    fn blow() -> BlowInput<'static> {
        BlowInput {
            attacker_is_playable: true,
            attacker_is_attackable: false,
            target_is_playable: false,
            target_is_player: false,
            target_is_attackable: true,
            position: Position::Front,
            p_atk: 100.0,
            p_def: 100.0,
            shld_def: 0,
            skill_power: SkillPower::flat(1000.0),
            pvp_phys_skill_dmg: 1.0,
            pvp_phys_skill_def: 1.0,
            class_balance: BlowClassBalance::default(),
            critical_damage: 1.0,
            critical_damage_pos: 1.0,
            defence_critical_damage: 1.0,
            critical_damage_add: 0.0,
            defence_critical_damage_add: 0.0,
            weapon_trait_bonus: 1.0,
            general_trait_bonus: 1.0,
            attribute_bonus: 1.0,
            randomize_physical_skill_damage: true,
            attacker_has_active_weapon: true,
            random_damage: 0,
            attacker_level: 40,
            target_is_raid: false,
            target_is_raid_minion: false,
            target_level: 40,
            attacker_player_level: Some(40),
            min_npc_level_dmg_penalty: 78,
            npc_skill_dmg_penalty: &SKILL_PENALTY,
        }
    }

    #[test]
    fn blow_and_backstab_differ_in_the_soulshot_and_pvp_test() {
        let i = blow();
        let mut rng = Fixed::new(&[]);
        // 77 * (1000 + 100) / 100 = 847
        assert_eq!(calc_blow_damage(&i, 0, false, &mut rng), 847.0);
        // Blow: the soulshot multiplies only the p.atk; backstab: the whole base.
        assert_eq!(
            calc_blow_damage(&i, 0, true, &mut rng),
            (77.0 * (1000.0 + 100.0 * 1.458)) / 100.0
        );
        assert_eq!(calc_backstab_damage(&i, 0, true, &mut rng), 847.0 * 1.458);
        // 1 on a perfect block, at least 1 otherwise.
        assert_eq!(calc_blow_damage(&i, 2, true, &mut rng), 1.0);
        let mut weak = i;
        weak.skill_power = SkillPower::flat(0.0);
        weak.p_atk = 0.0;
        assert_eq!(calc_blow_damage(&weak, 0, false, &mut rng), 1.0);
        // The backstab's PvP test is isPlayer.
        let mut pvp = i;
        pvp.target_is_playable = true;
        pvp.target_is_player = false;
        pvp.target_is_attackable = false;
        pvp.pvp_phys_skill_dmg = 2.0;
        assert_eq!(calc_backstab_damage(&pvp, 0, false, &mut rng), 847.0);
        assert_eq!(calc_blow_damage(&pvp, 0, false, &mut rng), 847.0 * 2.0);
    }

    #[test]
    fn blow_weapon_random_draw_is_one_and_optional() {
        let mut i = blow();
        i.random_damage = 5;
        let mut rng = Fixed::new(&[5]);
        assert_eq!(calc_blow_damage(&i, 0, false, &mut rng), 847.0 * 1.05);
        assert!(rng.exhausted());
        i.randomize_physical_skill_damage = false;
        let mut rng = Fixed::new(&[]);
        assert_eq!(calc_blow_damage(&i, 0, false, &mut rng), 847.0);
    }

    #[test]
    fn blow_positional_critical() {
        let mut i = blow();
        i.critical_damage_pos = 1.5;
        let mut rng = Fixed::new(&[]);
        // ((1.5 - 1) / 2) + 1 = 1.25
        assert_eq!(calc_blow_damage(&i, 0, false, &mut rng), 847.0 * 1.25);
    }

    #[test]
    fn skill_crit_rate_is_base_times_ten_times_str() {
        let input = SkillCritInput {
            base_crit_rate: 15,
            str_bonus: 1.2,
            attacker_is_playable: true,
            target_is_playable: false,
            target_is_attackable: true,
            pvp_physical_skill_critical_chance_multiplier: 1.0,
            pve_physical_skill_critical_chance_multiplier: 1.0,
            defence_critical_rate: &no_modifiers,
            defence_critical_rate_add: 0.0,
        };
        // 15 * 10 * 1.2 = 180: draw 179 crits, draw 180 does not.
        assert!(calc_skill_crit(&input, &mut Fixed::new(&[179])));
        assert!(!calc_skill_crit(&input, &mut Fixed::new(&[180])));
    }

    #[test]
    fn blow_success_side_modifier() {
        let input = |position| BlowSuccessInput {
            blow_chance: 20,
            dex_bonus: 1.0,
            position,
            blow_rate: &no_modifiers,
        };
        // 20 front, 40 behind, 30 side.
        assert!(calc_blow_success(&input(Position::Front), &mut Fixed::new(&[19])));
        assert!(!calc_blow_success(
            &input(Position::Front),
            &mut Fixed::new(&[20])
        ));
        assert!(calc_blow_success(&input(Position::Back), &mut Fixed::new(&[39])));
        assert!(!calc_blow_success(&input(Position::Back), &mut Fixed::new(&[40])));
        assert!(calc_blow_success(&input(Position::Side), &mut Fixed::new(&[29])));
        assert!(!calc_blow_success(&input(Position::Side), &mut Fixed::new(&[30])));
    }

    #[test]
    fn evasion_never_draws_for_magic_and_debuffs() {
        let mut rng = Fixed::new(&[]);
        assert!(!calc_physical_skill_evasion(true, false, 100.0, &mut rng));
        assert!(!calc_physical_skill_evasion(false, true, 100.0, &mut rng));
        assert!(calc_physical_skill_evasion(
            false,
            false,
            30.0,
            &mut Fixed::new(&[29])
        ));
        assert!(!calc_physical_skill_evasion(
            false,
            false,
            30.0,
            &mut Fixed::new(&[30])
        ));
    }

    #[test]
    fn resurrect_restore_percent() {
        assert_eq!(calculate_skill_resurrect_restore_percent(0.0, 1.5), 0.0);
        assert_eq!(calculate_skill_resurrect_restore_percent(100.0, 0.1), 100.0);
        // Never below the base, never above 90.
        assert_eq!(calculate_skill_resurrect_restore_percent(50.0, 0.5), 50.0);
        assert_eq!(calculate_skill_resurrect_restore_percent(80.0, 1.5), 90.0);
        // A gain over 20 points gets +20.
        assert_eq!(calculate_skill_resurrect_restore_percent(30.0, 2.0), 60.0 + 20.0);
        assert_eq!(calculate_skill_resurrect_restore_percent(30.0, 1.5), 45.0);
    }

    #[test]
    fn java_helpers() {
        assert!(java_max(f64::NAN, 1.0).is_nan());
        assert!(java_min(1.0, f64::NAN).is_nan());
        assert_eq!(java_clamp(5.0, 1.0, 3.0), 3.0);
        assert_eq!(java_clamp(-5.0, 1.0, 3.0), 1.0);
        assert_eq!(java_round_f32(2.5), 3);
        assert_eq!(java_round_f32(-2.5), -2);
        assert_eq!(java_round_f32(2.4), 2);
        assert_eq!(java_round_f32(f32::NAN), 0);
        assert_eq!(java_round_f32(3.0e10), i32::MAX);
    }

    fn cast() -> SkillCastTimeInput {
        SkillCastTimeInput {
            hit_time: 4000,
            cool_time: 0,
            skill_is_magic: true,
            skill_is_static: false,
            skill_is_channeling: false,
            channeling_skill_id: 0,
            spiritshot_charged: false,
            m_atk_spd: 213,
            p_atk_spd: 417.0,
        }
    }

    #[test]
    fn cast_time() {
        // (4000 / 213) * 333 = 6253 (the vector of Wind Strike for a duelist).
        assert_eq!(calc_skill_cast_time(&cast()), 6253);
        // x0.6 with a spiritshot.
        let sps = SkillCastTimeInput {
            spiritshot_charged: true,
            ..cast()
        };
        assert_eq!(calc_skill_cast_time(&sps), 3751);
        // A static skill keeps hit + cool.
        let s = SkillCastTimeInput {
            skill_is_static: true,
            ..cast()
        };
        assert_eq!(calc_skill_cast_time(&s), 4000);
        // Floors: 550 for magic over 550 ms, 500 for other skills from 500 ms.
        let quick = SkillCastTimeInput {
            hit_time: 600,
            m_atk_spd: 10_000,
            spiritshot_charged: true,
            ..cast()
        };
        assert_eq!(calc_skill_cast_time(&quick), 550);
        let quick_physical = SkillCastTimeInput {
            hit_time: 500,
            skill_is_magic: false,
            p_atk_spd: 100_000.0,
            ..cast()
        };
        assert_eq!(calc_skill_cast_time(&quick_physical), 500);
        let tiny = SkillCastTimeInput {
            hit_time: 400,
            skill_is_magic: false,
            p_atk_spd: 100_000.0,
            ..cast()
        };
        assert_eq!(calc_skill_cast_time(&tiny), 1);
    }

    #[test]
    fn channeling_skills_with_a_channel_id_skip_the_speed_formula() {
        let c = SkillCastTimeInput {
            skill_is_channeling: true,
            channeling_skill_id: 5,
            spiritshot_charged: true,
            ..cast()
        };
        assert_eq!(calc_skill_cast_time(&c), 4000);
        let no_id = SkillCastTimeInput {
            channeling_skill_id: 0,
            ..c
        };
        assert_eq!(calc_skill_cast_time(&no_id), 3751);
    }

    #[test]
    fn reuse_delay() {
        let base = SkillReuseInput {
            reuse_delay: 1200,
            static_reuse: false,
            skill_is_static: false,
            skill_is_magic: true,
            skill_is_physical: false,
            magic_reuse_rate: 0.5,
            p_reuse: 0.25,
            dance_reuse: 2.0,
            skill_reuse_multiplier: None,
        };
        assert_eq!(calc_skill_reuse_delay(&base), 600);
        let physical = SkillReuseInput {
            skill_is_magic: false,
            skill_is_physical: true,
            ..base
        };
        assert_eq!(calc_skill_reuse_delay(&physical), 300);
        let dance = SkillReuseInput {
            skill_is_magic: false,
            ..base
        };
        assert_eq!(calc_skill_reuse_delay(&dance), 2400);
        let static_reuse = SkillReuseInput {
            static_reuse: true,
            ..base
        };
        assert_eq!(calc_skill_reuse_delay(&static_reuse), 1200);
        let balanced = SkillReuseInput {
            skill_reuse_multiplier: Some(0.5),
            ..base
        };
        assert_eq!(calc_skill_reuse_delay(&balanced), 300);
    }
}
