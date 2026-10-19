//! HP, MP and CP regeneration rates.
//!
//! Java: `mechanics/stats/Formulas.java` `calcHpRegen`, `calcMpRegen`, `calcCpRegen`,
//! `getRegeneratePeriod`. The rates are per regeneration period (3 s).
//!
//! What stays with the caller (M4-REGEN-01) because it needs the world: the Seven Signs festival
//! and siege modifiers, and the clan hall / castle / fort function levels. It hands the resulting
//! factors in as [`HpRegenInput::multipliers`] / [`MpRegenInput::multipliers`], in Java's order.

use super::CalcStat;

/// Java `HP_REGENERATE_PERIOD`: the regeneration task period in milliseconds.
const HP_REGENERATE_PERIOD: i32 = 3000;

/// Java `Formulas.getRegeneratePeriod(creature)`: 3 s for a creature, 5 min for a door.
pub fn get_regenerate_period(is_door: bool) -> i32 {
    if is_door {
        HP_REGENERATE_PERIOD * 100
    } else {
        HP_REGENERATE_PERIOD
    }
}

/// Java's movement bonus branches for a player: `isSitting()`, then `!isMoving()`, then `isRunning()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenPosture {
    /// `isSitting()`: x1.5.
    Sitting,
    /// Not sitting and `!isMoving()`: x1.1.
    Standing,
    /// Moving and `isRunning()`: x0.7.
    Running,
    /// Moving and walking: no bonus.
    Walking,
}

impl RegenPosture {
    /// The multiplier the movement branch applies, `None` when none does.
    fn factor(self) -> Option<f64> {
        match self {
            Self::Sitting => Some(1.5),
            Self::Standing => Some(1.1),
            Self::Running => Some(0.7),
            Self::Walking => None,
        }
    }
}

/// Where the base regeneration of a creature comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegenKind {
    /// `creature.isPlayer()`: the class template value for the level, scaled by the level modifier
    /// and the CON (HP) / MEN (MP) base stat bonus, and the movement bonus.
    Player {
        /// `getTemplate().getBaseHpRegen(level)` / `getBaseMpRegen(level)` (a `double`).
        base_regen: f64,
        /// `creature.getLevelMod()`.
        level_mod: f64,
        /// `BaseStat.CON.calcBonus(creature)` (HP) / `BaseStat.MEN.calcBonus(creature)` (MP).
        stat_bonus: f64,
        /// Sitting, standing, running or walking.
        posture: RegenPosture,
    },
    /// Any other creature that is not a pet: the NPC template value, nothing else.
    Npc {
        /// `getTemplate().getBaseHpReg()` / `getBaseMpReg()` (a `float` in Java).
        base_reg: f32,
    },
    /// `creature.isPet()`: the pet level data times the pet multiplier.
    Pet {
        /// `getPetLevelData().getPetRegenHP()` / `getPetRegenMP()` (a `float`).
        pet_regen: f32,
        /// `NpcConfig.PET_HP_REGEN_MULTIPLIER` / `PET_MP_REGEN_MULTIPLIER`.
        pet_regen_multiplier: f64,
    },
}

/// What `Formulas.calcHpRegen(creature)` reads.
#[derive(Clone, Copy)]
pub struct HpRegenInput<'a> {
    /// The creature kind and its base value.
    pub kind: RegenKind,
    /// `creature.isRaid()`.
    pub is_raid: bool,
    /// `PlayerConfig.HP_REGEN_MULTIPLIER`.
    pub hp_regen_multiplier: f64,
    /// `NpcConfig.RAID_HP_REGEN_MULTIPLIER`.
    pub raid_hp_regen_multiplier: f64,
    /// `ChampionMonstersConfig.CHAMPION_HP_REGEN` when champions are enabled and the creature is
    /// one, otherwise `None`.
    pub champion_hp_regen: Option<f32>,
    /// Players only: the factors Java multiplies in before the movement bonus, in its order
    /// (festival or siege modifier, clan hall, castle, fort function levels).
    pub multipliers: &'a [f64],
    /// Players only: `MotherTreeZone.getHpRegenBonus()` when inside one (added after the product).
    pub mother_tree_hp_regen_bonus: i32,
    /// `creature.calcStat(Stat.REGENERATE_HP_RATE, base, null, null)`.
    pub regenerate_hp_rate: CalcStat<'a>,
}

/// Java `Formulas.calcHpRegen`: HP regained per period.
pub fn calc_hp_regen(input: &HpRegenInput<'_>) -> f64 {
    let mut init = match input.kind {
        RegenKind::Player { base_regen, .. } => base_regen,
        RegenKind::Npc { base_reg } => f64::from(base_reg),
        // The template value is read first and then replaced below, like Java.
        RegenKind::Pet { .. } => 0.0,
    };
    let mut hp_regen_multiplier = if input.is_raid {
        input.raid_hp_regen_multiplier
    } else {
        input.hp_regen_multiplier
    };
    let mut hp_regen_bonus = 0.0_f64;

    if let Some(champion) = input.champion_hp_regen {
        hp_regen_multiplier *= f64::from(champion);
    }

    match input.kind {
        RegenKind::Player {
            level_mod,
            stat_bonus,
            posture,
            ..
        } => {
            for factor in input.multipliers {
                hp_regen_multiplier *= factor;
            }

            // Mother Tree effect is calculated at last.
            hp_regen_bonus += f64::from(input.mother_tree_hp_regen_bonus);

            // Calculate Movement bonus.
            if let Some(factor) = posture.factor() {
                hp_regen_multiplier *= factor;
            }

            // Add CON bonus.
            init *= level_mod * stat_bonus;
        }
        RegenKind::Pet {
            pet_regen,
            pet_regen_multiplier,
        } => {
            init = f64::from(pet_regen) * pet_regen_multiplier;
        }
        RegenKind::Npc { .. } => {}
    }

    // NOTE(java-quirk): a non-raid NPC uses the *player* multiplier (HpRegenMultiplier).
    ((input.regenerate_hp_rate)(init.max(1.0)) * hp_regen_multiplier) + hp_regen_bonus
}

/// What `Formulas.calcMpRegen(creature)` reads.
#[derive(Clone, Copy)]
pub struct MpRegenInput<'a> {
    /// The creature kind and its base value (`stat_bonus` is the MEN bonus).
    pub kind: RegenKind,
    /// `creature.isRaid()`.
    pub is_raid: bool,
    /// `PlayerConfig.MP_REGEN_MULTIPLIER`.
    pub mp_regen_multiplier: f64,
    /// `NpcConfig.RAID_MP_REGEN_MULTIPLIER`.
    pub raid_mp_regen_multiplier: f64,
    /// Players only: the factors Java multiplies in before the movement bonus, in its order
    /// (festival modifier, clan hall, castle, fort function levels).
    pub multipliers: &'a [f64],
    /// Players only: `MotherTreeZone.getMpRegenBonus()` when inside one.
    pub mother_tree_mp_regen_bonus: i32,
    /// `creature.calcStat(Stat.REGENERATE_MP_RATE, base, null, null)`.
    pub regenerate_mp_rate: CalcStat<'a>,
}

/// Java `Formulas.calcMpRegen`: MP regained per period.
pub fn calc_mp_regen(input: &MpRegenInput<'_>) -> f64 {
    let mut init = match input.kind {
        RegenKind::Player { base_regen, .. } => base_regen,
        RegenKind::Npc { base_reg } => f64::from(base_reg),
        RegenKind::Pet { .. } => 0.0,
    };
    let mut mp_regen_multiplier = if input.is_raid {
        input.raid_mp_regen_multiplier
    } else {
        input.mp_regen_multiplier
    };
    let mut mp_regen_bonus = 0.0_f64;

    match input.kind {
        RegenKind::Player {
            level_mod,
            stat_bonus,
            posture,
            ..
        } => {
            // Mother Tree effect is calculated at last.
            mp_regen_bonus += f64::from(input.mother_tree_mp_regen_bonus);

            for factor in input.multipliers {
                mp_regen_multiplier *= factor;
            }

            // Calculate Movement bonus.
            if let Some(factor) = posture.factor() {
                mp_regen_multiplier *= factor;
            }

            // Add MEN bonus.
            init *= level_mod * stat_bonus;
        }
        RegenKind::Pet {
            pet_regen,
            pet_regen_multiplier,
        } => {
            init = f64::from(pet_regen) * pet_regen_multiplier;
        }
        RegenKind::Npc { .. } => {}
    }

    ((input.regenerate_mp_rate)(init.max(1.0)) * mp_regen_multiplier) + mp_regen_bonus
}

/// What `Formulas.calcCpRegen(player)` reads.
#[derive(Clone, Copy)]
pub struct CpRegenInput<'a> {
    /// `getTemplate().getBaseCpRegen(level)`.
    pub base_cp_regen: f64,
    /// `player.getLevelMod()`.
    pub level_mod: f64,
    /// `BaseStat.CON.calcBonus(player)`.
    pub con_bonus: f64,
    /// Sitting, standing, running or walking.
    pub posture: RegenPosture,
    /// `PlayerConfig.CP_REGEN_MULTIPLIER`.
    pub cp_regen_multiplier: f64,
    /// `player.calcStat(Stat.REGENERATE_CP_RATE, base, null, null)`.
    pub regenerate_cp_rate: CalcStat<'a>,
}

/// Java `Formulas.calcCpRegen`: CP regained per period.
pub fn calc_cp_regen(input: &CpRegenInput<'_>) -> f64 {
    // With CON bonus. NOTE(java-quirk): unlike HP and MP this multiplies left to right
    // ((base * levelMod) * bonus), which can differ in the last bit.
    let init = input.base_cp_regen * input.level_mod * input.con_bonus;
    let mut cp_regen_multiplier = input.cp_regen_multiplier;
    if let Some(factor) = input.posture.factor() {
        cp_regen_multiplier *= factor;
    }

    (input.regenerate_cp_rate)(init.max(1.0)) * cp_regen_multiplier
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::no_modifiers;

    fn player(posture: RegenPosture) -> RegenKind {
        RegenKind::Player {
            base_regen: 2.0,
            level_mod: 1.5,
            stat_bonus: 2.0,
            posture,
        }
    }

    fn hp(kind: RegenKind) -> HpRegenInput<'static> {
        HpRegenInput {
            kind,
            is_raid: false,
            hp_regen_multiplier: 1.0,
            raid_hp_regen_multiplier: 2.0,
            champion_hp_regen: None,
            multipliers: &[],
            mother_tree_hp_regen_bonus: 0,
            regenerate_hp_rate: &no_modifiers,
        }
    }

    #[test]
    fn period() {
        assert_eq!(get_regenerate_period(false), 3000);
        assert_eq!(get_regenerate_period(true), 300_000);
    }

    #[test]
    fn player_hp_regen_by_posture() {
        // init = 2 * (1.5 * 2) = 6.
        assert_eq!(calc_hp_regen(&hp(player(RegenPosture::Walking))), 6.0);
        assert_eq!(calc_hp_regen(&hp(player(RegenPosture::Sitting))), 6.0 * 1.5);
        assert_eq!(calc_hp_regen(&hp(player(RegenPosture::Standing))), 6.0 * 1.1);
        assert_eq!(calc_hp_regen(&hp(player(RegenPosture::Running))), 6.0 * 0.7);
    }

    #[test]
    fn hp_regen_factors_and_bonus() {
        let mut input = hp(player(RegenPosture::Walking));
        let factors = [1.5, 2.0];
        input.multipliers = &factors;
        input.mother_tree_hp_regen_bonus = 4;
        input.champion_hp_regen = Some(2.0);
        // 6 * ((1 * 2) * 1.5 * 2) + 4
        assert_eq!(calc_hp_regen(&input), 6.0 * 6.0 + 4.0);
        let double = |base: f64| base * 2.0;
        input.regenerate_hp_rate = &double;
        assert_eq!(calc_hp_regen(&input), 12.0 * 6.0 + 4.0);
    }

    #[test]
    fn base_is_at_least_one_before_the_stat() {
        let kind = RegenKind::Npc { base_reg: 0.25 };
        let seen = std::cell::Cell::new(0.0);
        let record = |base: f64| {
            seen.set(base);
            base
        };
        let mut input = hp(kind);
        input.regenerate_hp_rate = &record;
        assert_eq!(calc_hp_regen(&input), 1.0);
        assert_eq!(seen.get(), 1.0);
    }

    #[test]
    fn npc_uses_the_player_multiplier_unless_raid() {
        let mut input = hp(RegenKind::Npc { base_reg: 8.0 });
        input.hp_regen_multiplier = 3.0;
        assert_eq!(calc_hp_regen(&input), 24.0);
        input.is_raid = true;
        assert_eq!(calc_hp_regen(&input), 16.0);
    }

    #[test]
    fn pet_regen_replaces_the_base() {
        let input = hp(RegenKind::Pet {
            pet_regen: 4.0,
            pet_regen_multiplier: 0.5,
        });
        assert_eq!(calc_hp_regen(&input), 2.0);
        // 0.1f32 widens to 0.10000000149...
        let input = hp(RegenKind::Pet {
            pet_regen: 0.1,
            pet_regen_multiplier: 100.0,
        });
        assert_eq!(calc_hp_regen(&input), f64::from(0.1_f32) * 100.0);
    }

    #[test]
    fn mp_regen() {
        let input = MpRegenInput {
            kind: player(RegenPosture::Sitting),
            is_raid: false,
            mp_regen_multiplier: 1.0,
            raid_mp_regen_multiplier: 5.0,
            multipliers: &[2.0],
            mother_tree_mp_regen_bonus: 1,
            regenerate_mp_rate: &no_modifiers,
        };
        // 6 * (1 * 2 * 1.5) + 1
        assert_eq!(calc_mp_regen(&input), 6.0 * 3.0 + 1.0);
        let npc = MpRegenInput {
            kind: RegenKind::Npc { base_reg: 3.0 },
            is_raid: true,
            ..input
        };
        assert_eq!(calc_mp_regen(&npc), 15.0);
    }

    #[test]
    fn cp_regen() {
        let input = CpRegenInput {
            base_cp_regen: 2.0,
            level_mod: 1.5,
            con_bonus: 2.0,
            posture: RegenPosture::Standing,
            cp_regen_multiplier: 2.0,
            regenerate_cp_rate: &no_modifiers,
        };
        assert_eq!(calc_cp_regen(&input), 6.0 * (2.0 * 1.1));
    }
}
