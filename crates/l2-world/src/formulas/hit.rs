//! Hit, critical hit, shield and attack-break formulas (no skills).
//!
//! Java: `mechanics/stats/Formulas.java` `calcHitMiss`, `calcCrit(attacker, target)`,
//! `calcShldUse(attacker, target)`, `calcAtkBreak`; `data/xml/HitConditionBonusData.getConditionBonus`.
//! The skill variants (`skill != null`) are M5.

use l2_data::HitConditionBonus;

use super::{
    CalcStat, Position, Rnd, SHIELD_DEFENSE_FAILED, SHIELD_DEFENSE_PERFECT_BLOCK, SHIELD_DEFENSE_SUCCEED,
};
use crate::movement::heading_to_degree;

/// Java `HitConditionBonusData.getConditionBonus(attacker, target)`: a multiplier from the
/// attacker's height above the target (`> 50` high, `< -50` low), night and the side it attacks from.
///
/// `attacker_z` and `target_z` are the two `getZ()` values; `bonus` is the loaded
/// `hitConditionBonus.xml`.
pub fn condition_bonus(
    bonus: &HitConditionBonus,
    attacker_z: i32,
    target_z: i32,
    is_night: bool,
    position: Position,
) -> f64 {
    let mut modifier = 100.0_f64;
    let z_diff = attacker_z.wrapping_sub(target_z);

    // High or low ground.
    if z_diff > 50 {
        modifier += f64::from(bonus.high);
    } else if z_diff < -50 {
        modifier += f64::from(bonus.low);
    }

    // Night (no rain support in Java: the rain bonus is never read).
    if is_night {
        modifier += f64::from(bonus.dark);
    }

    // Side. Java asks isBehind first, then isInFrontOf, everything else is the side.
    modifier += f64::from(match position {
        Position::Back => bonus.back,
        Position::Front => bonus.front,
        Position::Side => bonus.side,
    });

    // Can't lower the chance by more than 100 %.
    (modifier / 100.0).max(0.0)
}

/// What `Formulas.calcHitMiss(attacker, target)` reads.
#[derive(Debug, Clone, Copy)]
pub struct HitMissInput<'a> {
    /// `attacker.getAccuracy()`.
    pub accuracy: i32,
    /// `target.getEvasionRate(attacker)`.
    pub evasion_rate: i32,
    /// `attacker.getZ()`.
    pub attacker_z: i32,
    /// `target.getZ()`.
    pub target_z: i32,
    /// `GameTimeTaskManager.isNight()`.
    pub is_night: bool,
    /// Where the attacker stands relative to the target (`isBehind` / `isInFrontOf`).
    pub position: Position,
    /// The loaded `hitConditionBonus.xml` values.
    pub hit_condition_bonus: &'a HitConditionBonus,
}

/// Java `Formulas.calcHitMiss`: `true` when the attack missed (the target evaded).
///
/// One draw: `Rnd.get(1000)`.
pub fn calc_hit_miss(input: &HitMissInput<'_>, rng: &mut impl Rnd) -> bool {
    let mut chance = 80_i32
        .wrapping_add(2_i32.wrapping_mul(input.accuracy.wrapping_sub(input.evasion_rate)))
        .wrapping_mul(10);

    // Get additional bonus from the conditions when you are attacking.
    // NOTE(java-quirk): `chance *= double` casts back to int right here (truncation toward zero).
    let bonus = condition_bonus(
        input.hit_condition_bonus,
        input.attacker_z,
        input.target_z,
        input.is_night,
        input.position,
    );
    chance = (f64::from(chance) * bonus) as i32;

    chance = chance.max(200);
    chance = chance.min(980);

    chance < rng.get_int(1000)
}

/// What `Formulas.calcCrit(attacker, target)` (no skill) reads.
#[derive(Clone, Copy)]
pub struct CritInput<'a> {
    /// `attacker.getStat().calcStat(Stat.CRITICAL_RATE_POS, attacker.getStat().getCriticalHit(target, null), target, null)`.
    pub critical_rate: f64,
    /// `attacker.isPlayable()`.
    pub attacker_is_playable: bool,
    /// `target.isPlayable()`.
    pub target_is_playable: bool,
    /// `target.isAttackable()`.
    pub target_is_attackable: bool,
    /// `ClassBalanceConfig.PVP_PHYSICAL_ATTACK_CRITICAL_CHANCE_MULTIPLIERS[attacker class]` (a `float`, used in PvP).
    pub pvp_physical_attack_critical_chance_multiplier: f32,
    /// `ClassBalanceConfig.PVE_PHYSICAL_ATTACK_CRITICAL_CHANCE_MULTIPLIERS[attacker class]` (used PvE).
    pub pve_physical_attack_critical_chance_multiplier: f32,
    /// `target.getStat().calcStat(Stat.DEFENCE_CRITICAL_RATE, rate, null, null)` (base = the rate).
    pub defence_critical_rate: CalcStat<'a>,
    /// `target.getStat().calcStat(Stat.DEFENCE_CRITICAL_RATE_ADD, 0, null, null)`.
    pub defence_critical_rate_add: f64,
}

/// Java `Formulas.calcCrit(attacker, target)`: `true` for a critical hit. One draw: `Rnd.get(1000)`.
pub fn calc_crit(input: &CritInput<'_>, rng: &mut impl Rnd) -> bool {
    let mut rate = input.critical_rate;

    let is_pvp = input.attacker_is_playable && input.target_is_playable;
    if is_pvp {
        rate *= f64::from(input.pvp_physical_attack_critical_chance_multiplier);
    } else if input.attacker_is_playable && input.target_is_attackable {
        // isPvE
        rate *= f64::from(input.pve_physical_attack_critical_chance_multiplier);
    }

    ((input.defence_critical_rate)(rate) + input.defence_critical_rate_add) > f64::from(rng.get_int(1000))
}

/// What `Formulas.calcShldUse(attacker, target)` (no skill) reads.
#[derive(Debug, Clone, Copy)]
pub struct ShieldInput {
    /// `target.getSecondaryWeaponItem()` is an `Armor` that is not a sigil.
    pub target_has_shield: bool,
    /// `target.calcStat(Stat.SHIELD_RATE, 0, attacker, null)`.
    pub shield_rate: f64,
    /// `BaseStat.DEX.calcBonus(target)`.
    pub dex_bonus: f64,
    /// `target.calcStat(Stat.SHIELD_DEFENCE_ANGLE, 0, null, null)`.
    pub shield_defence_angle: f64,
    /// `target.getX()`.
    pub target_x: i32,
    /// `target.getY()`.
    pub target_y: i32,
    /// `target.getHeading()`.
    pub target_heading: i32,
    /// `attacker.getX()`.
    pub attacker_x: i32,
    /// `attacker.getY()`.
    pub attacker_y: i32,
    /// The attacker's active weapon is a bow (`WeaponType.BOW`; crossbows don't count in Java).
    pub attacker_weapon_is_bow: bool,
    /// `PlayerConfig.ALT_PERFECT_SHLD_BLOCK`.
    pub alt_perfect_shld_block: i32,
}

/// Java `LocationUtil.calculateAngleFrom(originX, originY, targetX, targetY)`: degrees in [0, 360).
fn calculate_angle_from(origin_x: i32, origin_y: i32, target_x: i32, target_y: i32) -> f64 {
    let mut angle = f64::from(target_y.wrapping_sub(origin_y))
        .atan2(f64::from(target_x.wrapping_sub(origin_x)))
        .to_degrees();
    if angle < 0.0 {
        angle += 360.0;
    }
    angle
}

/// Java `Formulas.calcShldUse(attacker, target)`: [`SHIELD_DEFENSE_FAILED`],
/// [`SHIELD_DEFENSE_SUCCEED`] or [`SHIELD_DEFENSE_PERFECT_BLOCK`].
///
/// No draw when the target has no shield, no shield rate or the attacker is outside the shield
/// angle; otherwise one `Rnd.get(100)` for the perfect-block test and, only when that fails, a
/// second one. (The "your shield defence succeeded" system messages are the caller's job.)
pub fn calc_shld_use(input: &ShieldInput, rng: &mut impl Rnd) -> u8 {
    if !input.target_has_shield {
        return SHIELD_DEFENSE_FAILED;
    }

    let mut shld_rate = input.shield_rate * input.dex_bonus;
    if shld_rate <= 1e-6 {
        return SHIELD_DEFENSE_FAILED;
    }

    let degreeside = (input.shield_defence_angle as i32).wrapping_add(120);
    if degreeside < 360 {
        let direction =
            calculate_angle_from(input.target_x, input.target_y, input.attacker_x, input.attacker_y);
        if (direction - heading_to_degree(input.target_heading)).abs() > f64::from(degreeside / 2) {
            return SHIELD_DEFENSE_FAILED;
        }
    }

    let mut shld_success = SHIELD_DEFENSE_FAILED;

    // If attacker use bow and target wear shield, shield block rate is multiplied by 1.3 (30%).
    if input.attacker_weapon_is_bow {
        shld_rate *= 1.3;
    }

    if shld_rate > 0.0 && (100 - input.alt_perfect_shld_block) < rng.get_int(100) {
        shld_success = SHIELD_DEFENSE_PERFECT_BLOCK;
    } else if shld_rate > f64::from(rng.get_int(100)) {
        shld_success = SHIELD_DEFENSE_SUCCEED;
    }

    shld_success
}

/// What `Formulas.calcAtkBreak(target, dmg)` reads about the target.
#[derive(Clone, Copy)]
pub struct AtkBreakInput<'a> {
    /// `target.isChanneling()`.
    pub is_channeling: bool,
    /// `target.isCastingNow()`.
    pub is_casting_now: bool,
    /// `target.isAttackingNow()`.
    pub is_attacking_now: bool,
    /// The target's active weapon is a bow (`WeaponType.BOW`; crossbows don't count in Java).
    pub active_weapon_is_bow: bool,
    /// `target.isRaid()`.
    pub is_raid: bool,
    /// `target.isInvul()`.
    pub is_invul: bool,
    /// `BaseStat.MEN.calcBonus(target)`.
    pub men_bonus: f64,
    /// `target.calcStat(Stat.ATTACK_CANCEL, rate, null, null)` (base = the rate).
    pub attack_cancel: CalcStat<'a>,
    /// `PlayerConfig.ALT_GAME_CANCEL_CAST`.
    pub alt_game_cancel_cast: bool,
    /// `PlayerConfig.ALT_GAME_CANCEL_BOW`.
    pub alt_game_cancel_bow: bool,
}

/// Java `Formulas.calcAtkBreak`: `true` when the hit interrupts the target's cast or bow attack.
///
/// One draw, `Rnd.get(100)`, and none when the target can't be interrupted.
pub fn calc_atk_break(input: &AtkBreakInput<'_>, dmg: f64, rng: &mut impl Rnd) -> bool {
    if input.is_channeling {
        return false;
    }

    let mut init = 0.0_f64;

    if input.alt_game_cancel_cast && input.is_casting_now {
        init = 15.0;
    }

    if input.alt_game_cancel_bow && input.is_attacking_now && input.active_weapon_is_bow {
        init = 15.0;
    }

    if input.is_raid || input.is_invul || init <= 0.0 {
        return false; // No attack break
    }

    // Chance of break is higher with higher dmg.
    init += (13.0 * dmg).sqrt();

    // Chance is affected by target MEN.
    init -= (input.men_bonus * 100.0) - 100.0;

    // Calculate all modifiers for ATTACK_CANCEL.
    let mut rate = (input.attack_cancel)(init);

    // Adjust the rate to be between 1 and 99.
    rate = rate.clamp(1.0, 99.0);

    f64::from(rng.get_int(100)) < rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::no_modifiers;
    use crate::formulas::rng::testing::Fixed;

    const BONUS: HitConditionBonus = HitConditionBonus {
        front: 0,
        side: 5,
        back: 10,
        high: 3,
        low: -3,
        dark: -10,
        rain: 0,
    };

    #[test]
    fn condition_bonus_adds_height_night_and_side() {
        let b = |az, tz, night, pos| condition_bonus(&BONUS, az, tz, night, pos);
        assert_eq!(b(0, 0, false, Position::Front), 1.0);
        assert_eq!(b(0, 0, false, Position::Side), 1.05);
        assert_eq!(b(0, 0, false, Position::Back), 1.1);
        // Exactly 50 apart is still level ground.
        assert_eq!(b(50, 0, false, Position::Front), 1.0);
        assert_eq!(b(51, 0, false, Position::Front), 1.03);
        assert_eq!(b(-51, 0, false, Position::Front), 0.97);
        assert_eq!(b(0, 0, true, Position::Front), 0.9);
    }

    #[test]
    fn condition_bonus_never_below_zero() {
        let harsh = HitConditionBonus { dark: -500, ..BONUS };
        assert_eq!(condition_bonus(&harsh, 0, 0, true, Position::Front), 0.0);
    }

    fn hit_input(accuracy: i32, evasion_rate: i32) -> HitMissInput<'static> {
        HitMissInput {
            accuracy,
            evasion_rate,
            attacker_z: 0,
            target_z: 0,
            is_night: false,
            position: Position::Front,
            hit_condition_bonus: &BONUS,
        }
    }

    #[test]
    fn hit_miss_chance_is_clamped_to_200_980() {
        // Equal accuracy and evasion: (80 + 0) * 10 = 800, a draw of 799 hits, 800 misses.
        assert!(!calc_hit_miss(&hit_input(30, 30), &mut Fixed::new(&[799])));
        assert!(!calc_hit_miss(&hit_input(30, 30), &mut Fixed::new(&[800])));
        assert!(calc_hit_miss(&hit_input(30, 30), &mut Fixed::new(&[801])));
        // Huge accuracy: capped at 980.
        assert!(!calc_hit_miss(&hit_input(500, 0), &mut Fixed::new(&[979])));
        assert!(calc_hit_miss(&hit_input(500, 0), &mut Fixed::new(&[981])));
        // Huge evasion: floor of 200.
        assert!(!calc_hit_miss(&hit_input(0, 500), &mut Fixed::new(&[200])));
        assert!(calc_hit_miss(&hit_input(0, 500), &mut Fixed::new(&[201])));
    }

    #[test]
    fn hit_miss_chance_truncates_after_the_bonus() {
        // 800 * 1.05 = 840 (the double product may land just below: Java truncates it).
        let mut input = hit_input(30, 30);
        input.position = Position::Side;
        let expected = (800.0_f64 * 1.05) as i32;
        assert!(!calc_hit_miss(&input, &mut Fixed::new(&[expected])));
        assert!(calc_hit_miss(&input, &mut Fixed::new(&[expected + 1])));
    }

    fn crit_input(rate: f64) -> CritInput<'static> {
        CritInput {
            critical_rate: rate,
            attacker_is_playable: true,
            target_is_playable: false,
            target_is_attackable: true,
            pvp_physical_attack_critical_chance_multiplier: 1.0,
            pve_physical_attack_critical_chance_multiplier: 1.0,
            defence_critical_rate: &no_modifiers,
            defence_critical_rate_add: 0.0,
        }
    }

    #[test]
    fn crit_is_rate_greater_than_the_draw() {
        assert!(calc_crit(&crit_input(44.0), &mut Fixed::new(&[43])));
        assert!(!calc_crit(&crit_input(44.0), &mut Fixed::new(&[44])));
    }

    #[test]
    fn crit_uses_the_class_balance_for_pve_and_pvp() {
        let mut pve = crit_input(100.0);
        pve.pve_physical_attack_critical_chance_multiplier = 0.5;
        assert!(calc_crit(&pve, &mut Fixed::new(&[49])));
        assert!(!calc_crit(&pve, &mut Fixed::new(&[50])));
        let mut pvp = crit_input(100.0);
        pvp.target_is_playable = true;
        pvp.pvp_physical_attack_critical_chance_multiplier = 2.0;
        assert!(calc_crit(&pvp, &mut Fixed::new(&[199])));
        assert!(!calc_crit(&pvp, &mut Fixed::new(&[200])));
        // A monster attacking: neither multiplier.
        let mut mob = crit_input(100.0);
        mob.attacker_is_playable = false;
        mob.pve_physical_attack_critical_chance_multiplier = 0.0;
        assert!(calc_crit(&mob, &mut Fixed::new(&[99])));
    }

    #[test]
    fn crit_applies_the_target_defence_to_the_rate() {
        let halve = |rate: f64| rate * 0.5;
        let mut input = crit_input(100.0);
        input.defence_critical_rate = &halve;
        input.defence_critical_rate_add = 10.0;
        // 100 * 0.5 + 10 = 60
        assert!(calc_crit(&input, &mut Fixed::new(&[59])));
        assert!(!calc_crit(&input, &mut Fixed::new(&[60])));
    }

    fn shield_input() -> ShieldInput {
        // Target at the origin facing +x (heading 0 is 0 degrees), attacker straight ahead.
        ShieldInput {
            target_has_shield: true,
            shield_rate: 20.0,
            dex_bonus: 1.0,
            shield_defence_angle: 0.0,
            target_x: 0,
            target_y: 0,
            target_heading: 0,
            attacker_x: 40,
            attacker_y: 0,
            attacker_weapon_is_bow: false,
            alt_perfect_shld_block: 10,
        }
    }

    #[test]
    fn shield_without_shield_or_rate_draws_nothing() {
        let mut none = shield_input();
        none.target_has_shield = false;
        assert_eq!(calc_shld_use(&none, &mut Fixed::new(&[])), SHIELD_DEFENSE_FAILED);
        let mut no_rate = shield_input();
        no_rate.shield_rate = 0.0;
        assert_eq!(
            calc_shld_use(&no_rate, &mut Fixed::new(&[])),
            SHIELD_DEFENSE_FAILED
        );
    }

    #[test]
    fn shield_outcomes() {
        // Perfect block needs a first draw above 100 - 10 = 90.
        assert_eq!(
            calc_shld_use(&shield_input(), &mut Fixed::new(&[91])),
            SHIELD_DEFENSE_PERFECT_BLOCK
        );
        // First draw 90 is not above 90; second draw below the 20 % rate succeeds.
        assert_eq!(
            calc_shld_use(&shield_input(), &mut Fixed::new(&[90, 19])),
            SHIELD_DEFENSE_SUCCEED
        );
        assert_eq!(
            calc_shld_use(&shield_input(), &mut Fixed::new(&[90, 20])),
            SHIELD_DEFENSE_FAILED
        );
    }

    #[test]
    fn shield_bow_multiplies_the_rate_by_1_3() {
        let mut input = shield_input();
        input.attacker_weapon_is_bow = true;
        // 20 * 1.3 = 26: a second draw of 25 succeeds, 26 does not.
        assert_eq!(
            calc_shld_use(&input, &mut Fixed::new(&[0, 25])),
            SHIELD_DEFENSE_SUCCEED
        );
        assert_eq!(
            calc_shld_use(&input, &mut Fixed::new(&[0, 26])),
            SHIELD_DEFENSE_FAILED
        );
    }

    #[test]
    fn shield_only_covers_the_front_arc() {
        // Angle 0 + 120 = 120 degrees: 60 either side of the facing.
        let mut behind = shield_input();
        behind.attacker_x = -40;
        assert_eq!(
            calc_shld_use(&behind, &mut Fixed::new(&[])),
            SHIELD_DEFENSE_FAILED
        );
        // A shield-defence-angle bonus of 240 gives 360 degrees: all around.
        behind.shield_defence_angle = 240.0;
        assert_eq!(
            calc_shld_use(&behind, &mut Fixed::new(&[91])),
            SHIELD_DEFENSE_PERFECT_BLOCK
        );
    }

    fn break_input(cast: bool) -> AtkBreakInput<'static> {
        AtkBreakInput {
            is_channeling: false,
            is_casting_now: cast,
            is_attacking_now: false,
            active_weapon_is_bow: false,
            is_raid: false,
            is_invul: false,
            men_bonus: 1.0,
            attack_cancel: &no_modifiers,
            alt_game_cancel_cast: true,
            alt_game_cancel_bow: false,
        }
    }

    #[test]
    fn atk_break_only_for_casting_targets() {
        assert!(!calc_atk_break(&break_input(false), 100.0, &mut Fixed::new(&[])));
        // 15 + sqrt(13 * 100) = 51.05..., men bonus 1.0 -> no change: a draw of 51 breaks it, 52 not.
        assert!(calc_atk_break(&break_input(true), 100.0, &mut Fixed::new(&[51])));
        assert!(!calc_atk_break(&break_input(true), 100.0, &mut Fixed::new(&[52])));
    }

    #[test]
    fn atk_break_rate_is_clamped_and_blocked_for_raid_invul_channeling() {
        // Huge damage: rate capped at 99.
        assert!(calc_atk_break(&break_input(true), 1.0e9, &mut Fixed::new(&[98])));
        assert!(!calc_atk_break(&break_input(true), 1.0e9, &mut Fixed::new(&[99])));
        let mut raid = break_input(true);
        raid.is_raid = true;
        assert!(!calc_atk_break(&raid, 100.0, &mut Fixed::new(&[])));
        let mut invul = break_input(true);
        invul.is_invul = true;
        assert!(!calc_atk_break(&invul, 100.0, &mut Fixed::new(&[])));
        let mut channel = break_input(true);
        channel.is_channeling = true;
        assert!(!calc_atk_break(&channel, 100.0, &mut Fixed::new(&[])));
    }

    #[test]
    fn atk_break_bow_needs_the_config_and_a_bow() {
        let mut input = break_input(false);
        input.is_attacking_now = true;
        input.active_weapon_is_bow = true;
        assert!(!calc_atk_break(&input, 100.0, &mut Fixed::new(&[])));
        input.alt_game_cancel_bow = true;
        assert!(calc_atk_break(&input, 100.0, &mut Fixed::new(&[0])));
        input.active_weapon_is_bow = false;
        assert!(!calc_atk_break(&input, 100.0, &mut Fixed::new(&[])));
    }
}
