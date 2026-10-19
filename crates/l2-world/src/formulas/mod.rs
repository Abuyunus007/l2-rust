//! Combat and regeneration formulas as pure functions (M4-FORM-01).
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
//! Tests: unit tests next to each function, and `tests/formula_vectors.rs` for the Java vectors.

pub mod damage;
pub mod hit;
pub mod regen;
pub mod rng;
pub mod speed;

pub use damage::{
    PhysDamClassBalance, PhysDamInput, TraitInput, calc_attack_trait_bonus, calc_attribute_bonus,
    calc_lvl_bonus_mod, calc_phys_dam, random_damage_multiplier,
};
pub use hit::{
    AtkBreakInput, CritInput, HitMissInput, ShieldInput, calc_atk_break, calc_crit, calc_hit_miss,
    calc_shld_use, condition_bonus,
};
pub use regen::{
    CpRegenInput, HpRegenInput, MpRegenInput, RegenKind, RegenPosture, calc_cp_regen, calc_hp_regen,
    calc_mp_regen, get_regenerate_period,
};
pub use rng::Rnd;
pub use speed::{
    ReuseAttackType, ReuseInput, calc_atk_spd, calc_p_atk_spd, calculate_reuse_time,
    calculate_time_between_attacks,
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
