//! Combat, regeneration and skill formulas as pure functions (M4-FORM-01, M5-FORM-02).
//!
//! The skill formulas (M5-FORM-02) are in [`skill`] (physical skills, blows, crit, evasion, cast
//! and reuse time), [`magic`] (magic and mana damage, magic success, magic crit), [`effect`]
//! (effect landing, skill mastery, abnormal time), [`heal`] (the Heal effect's amount) and
//! [`traits`] (trait and element bonuses of skills).
//!
//! Java: `gameserver/mechanics/stats/Formulas.java` (+ `HitConditionBonusData.getConditionBonus`,
//! `Creature.calculateTimeBetweenAttacks` / `calculateReuseTime` / `getRandomDamageMultiplier`).
//!
//! # The pattern (copy it for the formulas of later cards)
//!
//! - A formula is a free function `calc_xxx(&XxxInput, ...) -> result` in the file of its family
//!   ([`hit`], [`damage`], [`speed`], [`regen`]). The name is the Java name in `snake_case`.
//! - Its input is a plain struct of numbers and flags. Every field is named after the Java getter
//!   or config key it replaces (`getAccuracy()` -> `accuracy`, `PlayerConfig.ALT_GAME_SHIELD_BLOCKS`
//!   -> `alt_game_shield_blocks`). The caller (combat / regen code, M4-COMBAT-01, M4-REGEN-01)
//!   fills it from the stat engine; nothing here reads a creature, the world or a config.
//! - Java types are kept: `int` is `i32`, `float` is `f32` (widened with `f64::from` exactly where
//!   Java widens it), `double` is `f64`, `byte` is `u8`. Casts and rounding are as written.
//! - A stat that Java asks with a base that depends on the formula (`calcStat(stat, base, ...)`
//!   with the base computed inside the formula) is passed as a [`CalcStat`] closure. Stats asked
//!   with a fixed base (`calcStat(Stat.CRITICAL_DAMAGE, 1, ...)`) are passed as the plain number.
//! - Every random draw goes through the [`Rnd`] trait in Java's order, so a recorded sequence
//!   of draws (the `rnd` lists of `java-vectors/out/formulas_*.json`) replays exactly.
//!   The world's seedable RNG ([`WorldRng`](crate::WorldRng)) implements [`Rnd`].
//! - Quirks stay: marked `// NOTE(java-quirk):`.
//!
//! Tests: unit tests next to each function, and `tests/formula_vectors.rs` (normal attacks) and
//! `tests/skill_formula_vectors.rs` (skills) for the Java vectors.

pub mod damage;
pub mod effect;
pub mod heal;
pub mod hit;
pub mod magic;
pub mod regen;
pub mod rng;
pub mod skill;
pub mod speed;
pub mod traits;

pub use damage::{
    PhysDamClassBalance, PhysDamInput, TraitInput, calc_attack_trait_bonus, calc_attribute_bonus,
    calc_lvl_bonus_mod, calc_phys_dam, random_damage_multiplier,
};
pub use effect::{
    AbnormalTimeInput, AbnormalTimeResists, EffectSuccessInput, SkillMasteryInput, calc_effect_abnormal_time,
    calc_effect_success, calc_probability, calc_skill_mastery,
};
pub use heal::{HealInput, HealWeaponGrade, calc_heal_amount};
pub use hit::{
    AtkBreakInput, CritInput, HitMissInput, ShieldInput, calc_atk_break, calc_crit, calc_hit_miss,
    calc_shld_use, condition_bonus,
};
pub use magic::{
    MCriticalHitInput, MagicAffectedInput, MagicDamClassBalance, MagicDamInput, MagicDamResult, MagicFailure,
    MagicSuccessInput, ManaDamInput, ManaDamResult, calc_m_critical_hit, calc_magic_affected, calc_magic_dam,
    calc_magic_success, calc_mana_dam, calc_mcrit,
};
pub use regen::{
    CpRegenInput, HpRegenInput, MpRegenInput, RegenKind, RegenPosture, calc_cp_regen, calc_hp_regen,
    calc_mp_regen, get_regenerate_period,
};
pub use rng::Rnd;
pub use skill::{
    BlowClassBalance, BlowInput, BlowSuccessInput, SkillCastTimeInput, SkillCritInput,
    SkillPhysDamClassBalance, SkillPhysDamInput, SkillPower, SkillReuseInput, calc_backstab_damage,
    calc_blow_damage, calc_blow_success, calc_physical_skill_evasion, calc_skill_cast_time, calc_skill_crit,
    calc_skill_phys_dam, calc_skill_reuse_delay, calculate_skill_resurrect_restore_percent,
};
pub use speed::{
    ReuseAttackType, ReuseInput, calc_atk_spd, calc_p_atk_spd, calculate_reuse_time,
    calculate_time_between_attacks,
};
pub use traits::{
    TraitKind, attribute_element, calc_attribute_bonus_skill, calc_general_trait_bonus,
    calc_weapon_trait_bonus,
};

/// Java `Formulas.SHIELD_DEFENSE_FAILED`: no shield defence.
pub const SHIELD_DEFENSE_FAILED: u8 = 0;
/// Java `Formulas.SHIELD_DEFENSE_SUCCEED`: normal shield defence.
pub const SHIELD_DEFENSE_SUCCEED: u8 = 1;
/// Java `Formulas.SHIELD_DEFENSE_PERFECT_BLOCK`: perfect block.
pub const SHIELD_DEFENSE_PERFECT_BLOCK: u8 = 2;

/// Java `Position` (`FRONT`, `SIDE`, `BACK`): where the attacker stands relative to the target,
/// decided by the caller from `isBehind` / `isInFrontOf` (a creature that is neither is on the side).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Front,
    Side,
    Back,
}

/// The stat engine's `Creature.calcStat(stat, base, target, skill)` with the stat, the target and
/// the skill already bound: the formula passes the base value in and gets the folded value out.
/// For a creature without modifiers on that stat it is the identity ([`no_modifiers`]).
pub type CalcStat<'a> = &'a dyn Fn(f64) -> f64;

/// A [`CalcStat`] for a stat nothing modifies.
pub fn no_modifiers(base: f64) -> f64 {
    base
}
