//! The number a `Heal` effect restores.
//!
//! Java: `dist/game/data/scripts/handlers/skill/effects/Heal.java` `onStart`.
//! The effect itself (dead / door / invulnerable targets, setting the HP, the system messages) is
//! the handler's job; this is the amount.

use super::magic::calc_mcrit;
use super::skill::{java_max, java_min};
use super::{CalcStat, Rnd};

/// The crystal grade of the effector's active weapon, as `Heal.onStart` tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealWeaponGrade {
    /// `effector.getActiveWeaponInstance() == null`.
    NoWeapon,
    /// `CrystalType.S84`.
    S84,
    /// `CrystalType.S80`.
    S80,
    /// Every other grade.
    Other,
}

/// What `Heal.onStart(effector, effected, skill)` reads for the amount.
#[derive(Clone, Copy)]
pub struct HealInput<'a> {
    /// The effect's `power` parameter.
    pub power: f64,
    /// `skill.isMagic()`.
    pub skill_is_magic: bool,
    /// `skill.isStatic()`: potions and the like heal exactly `power`.
    pub skill_is_static: bool,
    /// `skill.getMpConsume()`.
    pub skill_mp_consume: i32,
    /// `skill.getItemConsumeCount()`.
    pub skill_item_consume_count: i32,
    /// `effector.isChargedShot(ShotType.SPIRITSHOTS)` (combined with `skill.isMagic()` inside).
    pub effector_spiritshot_charged: bool,
    /// `effector.isChargedShot(ShotType.BLESSED_SPIRITSHOTS)` (combined with `skill.isMagic()`).
    pub effector_blessed_spiritshot_charged: bool,
    /// `effector.isPlayer() && effector.asPlayer().isMageClass()`.
    pub effector_is_mage_player: bool,
    /// `effector.isSummon()`.
    pub effector_is_summon: bool,
    /// `effector.isNpc()`.
    pub effector_is_npc: bool,
    /// The crystal grade of `effector.getActiveWeaponInstance()`.
    pub effector_weapon_grade: HealWeaponGrade,
    /// `effector.getMAtk(effector, null)`.
    pub effector_m_atk: f64,
    /// `effected.calcStat(Stat.HEAL_EFFECT, amount, null, null)` (base = the amount).
    pub heal_effect: CalcStat<'a>,
    /// `effector.isPlayable()`.
    pub effector_is_playable: bool,
    /// `ClassBalanceConfig.PLAYER_HEALING_SKILL_MULTIPLIERS[effector class]`.
    pub player_healing_skill_multiplier: f32,
    /// `effector.getMCriticalHit(effected, skill)`.
    pub effector_m_critical_hit: i32,
    /// `effected.getMaxRecoverableHp()`.
    pub effected_max_recoverable_hp: i32,
    /// `effected.getCurrentHp()`.
    pub effected_current_hp: f64,
}

/// The amount of HP `Heal.onStart` adds to the effected: `power` plus the shot and M. Atk part,
/// the `HEAL_EFFECT` stat, the class balance and x3 on a magic critical, capped by the room left
/// below the maximum recoverable HP and never negative.
///
/// One draw, `Rnd.get(1000)`, for a non-static magic skill; none for a static skill.
pub fn calc_heal_amount(input: &HealInput<'_>, rng: &mut impl Rnd) -> f64 {
    let mut amount = input.power;
    let mut static_shot_bonus = 0.0_f64;
    let mut m_atk_mul = 1_i32;
    let sps = input.skill_is_magic && input.effector_spiritshot_charged;
    let bss = input.skill_is_magic && input.effector_blessed_spiritshot_charged;
    if ((sps || bss) && input.effector_is_mage_player) || input.effector_is_summon {
        // NOTE(java-quirk): a summon always gets the static bonus, with or without a shot.
        static_shot_bonus = f64::from(input.skill_mp_consume); // static bonus for spiritshots
        m_atk_mul = if bss { 4 } else { 2 };
        static_shot_bonus *= if bss { 2.4 } else { 1.0 };
    } else if (sps || bss) && input.effector_is_npc {
        static_shot_bonus = 2.4 * f64::from(input.skill_mp_consume); // always blessed spiritshots
        m_atk_mul = 4;
    } else {
        // No static bonus: grade dynamic bonus.
        match input.effector_weapon_grade {
            HealWeaponGrade::NoWeapon => {}
            HealWeaponGrade::S84 => m_atk_mul = 4,
            HealWeaponGrade::S80 => m_atk_mul = 2,
            HealWeaponGrade::Other => m_atk_mul = 1,
        }

        // Shot dynamic bonus.
        m_atk_mul = if bss { m_atk_mul * 4 } else { m_atk_mul + 1 };
    }

    if !input.skill_is_static {
        amount += static_shot_bonus + (f64::from(m_atk_mul) * input.effector_m_atk).sqrt();
        amount = (input.heal_effect)(amount);

        if input.effector_is_playable && input.skill_item_consume_count <= 0 {
            amount *= f64::from(input.player_healing_skill_multiplier);
        }

        // Heal critic, since CT2.3 Gracia Final.
        if input.skill_is_magic && calc_mcrit(f64::from(input.effector_m_critical_hit), rng) {
            amount *= 3.0;
        }
    }

    // Prevents overheal and negative amount.
    java_max(
        java_min(
            amount,
            f64::from(input.effected_max_recoverable_hp) - input.effected_current_hp,
        ),
        0.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::no_modifiers;
    use crate::formulas::rng::testing::Fixed;

    fn base() -> HealInput<'static> {
        HealInput {
            power: 100.0,
            skill_is_magic: true,
            skill_is_static: false,
            skill_mp_consume: 40,
            skill_item_consume_count: 0,
            effector_spiritshot_charged: false,
            effector_blessed_spiritshot_charged: false,
            effector_is_mage_player: true,
            effector_is_summon: false,
            effector_is_npc: false,
            effector_weapon_grade: HealWeaponGrade::Other,
            effector_m_atk: 100.0,
            heal_effect: &no_modifiers,
            effector_is_playable: true,
            player_healing_skill_multiplier: 1.0,
            effector_m_critical_hit: 100,
            effected_max_recoverable_hp: 10_000,
            effected_current_hp: 1.0,
        }
    }

    fn amount(input: &HealInput<'_>, draws: &[i32]) -> f64 {
        let mut rng = Fixed::new(draws);
        let a = calc_heal_amount(input, &mut rng);
        assert!(rng.exhausted(), "all scripted draws used");
        a
    }

    #[test]
    fn grade_and_shot_dynamic_bonus() {
        // No shots: mAtkMul = 1 + 1 = 2 for a non-S weapon: 100 + sqrt(200).
        assert_eq!(amount(&base(), &[999]), 100.0 + (2.0_f64 * 100.0).sqrt());
        let s84 = HealInput {
            effector_weapon_grade: HealWeaponGrade::S84,
            ..base()
        };
        assert_eq!(amount(&s84, &[999]), 100.0 + (5.0_f64 * 100.0).sqrt());
        let s80 = HealInput {
            effector_weapon_grade: HealWeaponGrade::S80,
            ..base()
        };
        assert_eq!(amount(&s80, &[999]), 100.0 + (3.0_f64 * 100.0).sqrt());
        let none = HealInput {
            effector_weapon_grade: HealWeaponGrade::NoWeapon,
            ..base()
        };
        assert_eq!(amount(&none, &[999]), 100.0 + (2.0_f64 * 100.0).sqrt());
    }

    #[test]
    fn mage_with_a_spiritshot_gets_the_static_bonus() {
        let sps = HealInput {
            effector_spiritshot_charged: true,
            ..base()
        };
        assert_eq!(amount(&sps, &[999]), 100.0 + (40.0 + (2.0_f64 * 100.0).sqrt()));
        let bss = HealInput {
            effector_blessed_spiritshot_charged: true,
            ..base()
        };
        assert_eq!(
            amount(&bss, &[999]),
            100.0 + (40.0 * 2.4 + (4.0_f64 * 100.0).sqrt())
        );
    }

    #[test]
    fn non_mage_players_get_the_grade_bonus_times_four_for_a_blessed_shot() {
        let fighter_bss = HealInput {
            effector_is_mage_player: false,
            effector_blessed_spiritshot_charged: true,
            effector_weapon_grade: HealWeaponGrade::S80,
            ..base()
        };
        // mAtkMul = 2 * 4
        assert_eq!(amount(&fighter_bss, &[999]), 100.0 + (8.0_f64 * 100.0).sqrt());
    }

    #[test]
    fn summons_and_npcs() {
        let summon = HealInput {
            effector_is_summon: true,
            effector_is_mage_player: false,
            ..base()
        };
        // NOTE(java-quirk): no shot needed.
        assert_eq!(amount(&summon, &[999]), 100.0 + (40.0 + (2.0_f64 * 100.0).sqrt()));
        let npc = HealInput {
            effector_is_npc: true,
            effector_is_mage_player: false,
            effector_is_playable: false,
            effector_spiritshot_charged: true,
            ..base()
        };
        assert_eq!(
            amount(&npc, &[999]),
            100.0 + (2.4 * 40.0 + (4.0_f64 * 100.0).sqrt())
        );
        // Without a shot an NPC takes the plain branch.
        let npc_plain = HealInput {
            effector_spiritshot_charged: false,
            ..npc
        };
        assert_eq!(amount(&npc_plain, &[999]), 100.0 + (2.0_f64 * 100.0).sqrt());
    }

    #[test]
    fn magic_crit_triples_and_the_cap_applies() {
        let base_amount = 100.0 + (2.0_f64 * 100.0).sqrt();
        assert_eq!(amount(&base(), &[99]), base_amount * 3.0);
        // 100 is the rate: draw 100 does not crit.
        assert_eq!(amount(&base(), &[100]), base_amount);
        let nearly_full = HealInput {
            effected_current_hp: 9990.0,
            ..base()
        };
        assert_eq!(amount(&nearly_full, &[99]), 10.0);
        let full = HealInput {
            effected_current_hp: 10_000.0,
            ..base()
        };
        assert_eq!(amount(&full, &[0]), 0.0);
        // Over the maximum (a buff ran out): never negative.
        let over = HealInput {
            effected_current_hp: 12_000.0,
            ..base()
        };
        assert_eq!(amount(&over, &[0]), 0.0);
    }

    #[test]
    fn physical_skills_do_not_crit_or_use_shots() {
        let physical = HealInput {
            skill_is_magic: false,
            effector_spiritshot_charged: true,
            ..base()
        };
        // sps is false for a non-magic skill: the plain branch, no draw.
        assert_eq!(amount(&physical, &[]), 100.0 + (2.0_f64 * 100.0).sqrt());
    }

    #[test]
    fn static_skills_heal_exactly_the_power() {
        let potion = HealInput {
            skill_is_static: true,
            skill_is_magic: false,
            ..base()
        };
        assert_eq!(amount(&potion, &[]), 100.0);
        let capped = HealInput {
            effected_current_hp: 9950.0,
            ..potion
        };
        assert_eq!(amount(&capped, &[]), 50.0);
    }

    #[test]
    fn heal_effect_stat_and_class_balance() {
        let boost = |a: f64| a * 1.5;
        let i = HealInput {
            heal_effect: &boost,
            player_healing_skill_multiplier: 0.5,
            ..base()
        };
        let plain = 100.0 + (2.0_f64 * 100.0).sqrt();
        assert_eq!(amount(&i, &[999]), plain * 1.5 * f64::from(0.5_f32));
        // Item skills (scrolls, potions with an item count) skip the class balance.
        let item = HealInput {
            skill_item_consume_count: 1,
            ..i
        };
        assert_eq!(amount(&item, &[999]), plain * 1.5);
    }
}
