//! The formulas of one normal hit with their inputs taken from the stat engine: what
//! `Formulas.calcHitMiss/calcShldUse/calcCrit/calcPhysDam/calcAtkBreak` read from the creatures.
// Java: mechanics/stats/Formulas.java (calcHitMiss, calcShldUse, calcCrit, calcPhysDam,
//       calcAttackTraitBonus, calcAttributeBonus, calcAtkBreak)
// Java: entity/actor/enums/creature/Position.java (getPosition), interfaces/ILocational.java
//       (isBehind, isInFrontOf)
// Java: entity/actor/Creature.java (doAttackHitSimple: the order of the four calls)
//
// The formulas themselves are M4-FORM-01's pure functions (`crate::formulas`); this file only fills
// their inputs. Every function takes the RNG as an argument (`impl Rnd`), so the Java vectors of
// M4-JV-01 replay through the very code the attack uses (`tests/combat.rs`).

use l2_data::Stat;
use l2_data::item::{ArmorType, WeaponType};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId;

use crate::formulas::{
    AtkBreakInput, CritInput, HitMissInput, PhysDamClassBalance, PhysDamInput, Position, Rnd, ShieldInput,
    calc_atk_break, calc_attack_trait_bonus, calc_crit, calc_hit_miss, calc_phys_dam, calc_shld_use,
};
use crate::movement::heading_from_points;
use crate::stats::{BaseStat, ClassBalance, StatView};
use crate::world::World;

/// What `doAttackHitSimple` rolled for one hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitOutcome {
    pub miss: bool,
    /// [`crate::formulas::SHIELD_DEFENSE_FAILED`] and the other two.
    pub shld: u8,
    pub crit: bool,
    /// `(int) calcPhysDam(...)`, 0 when missed.
    pub damage: i32,
}

/// Java `Position.getPosition(attacker, target)`: where `attacker` stands relative to the way
/// `target` faces (`isBehind` / `isInFrontOf` / the side).
pub fn attacker_position(world: &World, attacker: ObjectId, target: ObjectId) -> Position {
    let (Some(from), Some(to)) = (world.position_of(attacker), world.position_of(target)) else {
        return Position::Front;
    };
    position_from(from.0, from.1, to.0, to.1, world.heading_of(target))
}

/// The arithmetic of `Position.getPosition`.
pub fn position_from(from_x: i32, from_y: i32, to_x: i32, to_y: i32, to_heading: i32) -> Position {
    let heading = to_heading
        .wrapping_sub(heading_from_points(from_x, from_y, to_x, to_y))
        .wrapping_abs();
    // `Integer.toUnsignedLong(heading - 0xA000) <= 0x4000`: the subtraction is an int.
    if (0x2000..=0x6000).contains(&heading) || (heading.wrapping_sub(0xA000) as u32) <= 0x4000 {
        Position::Side
    } else if (heading.wrapping_sub(0x2000) as u32) <= 0xC000 {
        Position::Front
    } else {
        Position::Back
    }
}

/// `getActiveWeaponItem()`'s weapon type, `None` without a weapon.
fn weapon_type(view: &StatView<'_>) -> Option<WeaponType> {
    view.active_weapon_item()
        .and_then(|t| t.weapon())
        .map(|w| w.weapon_type)
}

/// The class id `asPlayer().getPlayerClass().getId()` indexes the class balance tables with.
fn class_id(view: &StatView<'_>) -> i32 {
    view.player().map_or(-1, |p| p.data.class_id)
}

/// Java `Formulas.calcHitMiss(attacker, target)`.
pub fn calc_hit_miss_for(world: &World, attacker: ObjectId, target: ObjectId, rng: &mut impl Rnd) -> bool {
    let (Some(a), Some(t)) = (world.stats(attacker), world.stats(target)) else {
        return true;
    };
    let input = HitMissInput {
        accuracy: a.accuracy(),
        evasion_rate: t.evasion_rate(Some(attacker)),
        attacker_z: world.position_of(attacker).map_or(0, |p| p.2),
        target_z: world.position_of(target).map_or(0, |p| p.2),
        is_night: world.game_clock_night.unwrap_or(false),
        position: attacker_position(world, attacker, target),
        hit_condition_bonus: &world.combat.hit_condition_bonus,
    };
    calc_hit_miss(&input, rng)
}

/// Java `getSecondaryWeaponItem()`: a player's left hand item, an NPC's template left hand.
pub(crate) fn secondary_weapon_item<'w>(view: &StatView<'w>) -> Option<&'w l2_data::ItemTemplate> {
    match (view.player(), view.npc()) {
        (Some(_), _) => view
            .paperdoll(paperdoll::LHAND)
            .and_then(|item| view.stat_data()?.items.get(item.item_id)),
        (None, Some(npc)) if npc.left_hand >= 1 => view.stat_data().and_then(|d| d.items.get(npc.left_hand)),
        _ => None,
    }
}

/// Java `target.getSecondaryWeaponItem()` is an `Armor` that is not a sigil.
fn has_shield(view: &StatView<'_>) -> bool {
    secondary_weapon_item(view)
        .and_then(|t| t.armor())
        .is_some_and(|a| a.armor_type != ArmorType::Sigil)
}

/// Java `Formulas.calcShldUse(attacker, target)` without its system messages (the attack sends
/// them, see `World::send_shield_message`).
pub fn calc_shld_use_for(world: &World, attacker: ObjectId, target: ObjectId, rng: &mut impl Rnd) -> u8 {
    let (Some(a), Some(t)) = (world.stats(attacker), world.stats(target)) else {
        return crate::formulas::SHIELD_DEFENSE_FAILED;
    };
    let (Some(ap), Some(tp)) = (world.position_of(attacker), world.position_of(target)) else {
        return crate::formulas::SHIELD_DEFENSE_FAILED;
    };
    let target_has_shield = has_shield(&t);
    let input = ShieldInput {
        target_has_shield,
        // Only read when there is a shield (Java computes it after the item check).
        shield_rate: if target_has_shield {
            t.calc_stat(Stat::ShieldRate, 0.0, Some(attacker), None)
        } else {
            0.0
        },
        dex_bonus: if target_has_shield {
            t.bonus(BaseStat::Dex)
        } else {
            1.0
        },
        shield_defence_angle: if target_has_shield {
            t.calc(Stat::ShieldDefenceAngle, 0.0)
        } else {
            0.0
        },
        target_x: tp.0,
        target_y: tp.1,
        target_heading: world.heading_of(target),
        attacker_x: ap.0,
        attacker_y: ap.1,
        attacker_weapon_is_bow: weapon_type(&a) == Some(WeaponType::Bow),
        alt_perfect_shld_block: world.settings.combat.alt_perfect_shld_block,
    };
    calc_shld_use(&input, rng)
}

/// Java `Formulas.calcCrit(attacker, target)`.
pub fn calc_crit_for(world: &World, attacker: ObjectId, target: ObjectId, rng: &mut impl Rnd) -> bool {
    let (Some(a), Some(t)) = (world.stats(attacker), world.stats(target)) else {
        return false;
    };
    let critical_rate = a.calc_stat(
        Stat::CriticalRatePos,
        f64::from(a.critical_hit(Some(target), None)),
        Some(target),
        None,
    );
    let cb = &world.settings.stats.class_balance;
    let class = class_id(&a);
    let defence = |rate: f64| t.calc_stat(Stat::DefenceCriticalRate, rate, None, None);
    let input = CritInput {
        critical_rate,
        attacker_is_playable: a.is_playable(),
        target_is_playable: t.is_playable(),
        target_is_attackable: t.is_attackable(),
        pvp_physical_attack_critical_chance_multiplier: ClassBalance::of(
            &cb.pvp_physical_attack_critical_chance,
            class,
        ),
        pve_physical_attack_critical_chance_multiplier: ClassBalance::of(
            &cb.pve_physical_attack_critical_chance,
            class,
        ),
        defence_critical_rate: &defence,
        defence_critical_rate_add: t.calc(Stat::DefenceCriticalRateAdd, 0.0),
    };
    calc_crit(&input, rng)
}

/// Java `Formulas.calcPhysDam(attacker, target, null, shld, crit, ss)`.
pub fn calc_phys_dam_for(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    shld: u8,
    crit: bool,
    ss: bool,
    rng: &mut impl Rnd,
) -> f64 {
    let (Some(a), Some(t)) = (world.stats(attacker), world.stats(target)) else {
        return 0.0;
    };
    let cb = &world.settings.stats.class_balance;
    let combat = &world.settings.combat;
    let (attacker_class, target_class) = (class_id(&a), class_id(&t));
    let weapon = weapon_type(&a);
    let element = a.attack_element();
    let input = PhysDamInput {
        attacker_is_playable: a.is_playable(),
        target_is_playable: t.is_playable(),
        target_is_attackable: t.is_attackable(),
        position: attacker_position(world, attacker, target),
        p_atk: a.p_atk(Some(target)),
        p_def: t.p_def(Some(attacker)),
        shld_def: t.shld_def(),
        pvp_physical_def: t.calc(Stat::PvpPhysicalDef, 1.0),
        critical_damage: a.calc_stat(Stat::CriticalDamage, 1.0, Some(target), None),
        critical_damage_pos: a.calc_stat(Stat::CriticalDamagePos, 1.0, Some(target), None),
        defence_critical_damage: t.calc_stat(Stat::DefenceCriticalDamage, 1.0, Some(target), None),
        critical_damage_add: a.calc_stat(Stat::CriticalDamageAdd, 0.0, Some(target), None),
        defence_critical_damage_add: t.calc_stat(Stat::DefenceCriticalDamageAdd, 0.0, Some(target), None),
        // The attack and defence traits come from the effects `AttackTrait` and `DefenceTrait`.
        attack_trait_bonus: {
            let (weapon_defence_trait, weaknesses) = world.attack_trait_inputs(attacker, target);
            calc_attack_trait_bonus(weapon_defence_trait, &weaknesses)
        },
        attacker_has_active_weapon: a.active_weapon_item().is_some(),
        random_damage: a.calc(Stat::RandomDamage, 0.0) as i32,
        attacker_level: a.level(),
        pvp_physical_dmg: a.calc(Stat::PvpPhysicalDmg, 1.0),
        attack_attribute: a.attack_element_value(element),
        defence_attribute: t.defense_element_value(element),
        attacker_weapon_is_bow_or_crossbow: matches!(weapon, Some(WeaponType::Bow | WeaponType::CrossBow)),
        pve_bow_dmg: a.calc(Stat::PveBowDmg, 1.0),
        pve_physical_dmg: a.calc(Stat::PvePhysicalDmg, 1.0),
        target_is_raid: t.is_raid(),
        // Java `isRaidMinion()`: a minion whose leader is a raid boss.
        target_is_raid_minion: t
            .npc()
            .and_then(|n| n.leader)
            .is_some_and(|leader| world.is_raid(leader)),
        target_level: t.level(),
        attacker_player_level: a.player().map(|p| p.core.level),
        class_balance: PhysDamClassBalance {
            pvp_physical_skill_defence: ClassBalance::of(&cb.pvp_physical_skill_defence, target_class),
            pvp_physical_attack_critical_damage: ClassBalance::of(
                &cb.pvp_physical_attack_critical_damage,
                attacker_class,
            ),
            pve_physical_attack_critical_damage: ClassBalance::of(
                &cb.pve_physical_attack_critical_damage,
                attacker_class,
            ),
            pvp_physical_attack_damage: ClassBalance::of(&cb.pvp_physical_attack_damage, attacker_class),
            pve_physical_attack_damage: ClassBalance::of(&cb.pve_physical_attack_damage, attacker_class),
        },
        alt_game_shield_blocks: combat.alt_game_shield_blocks,
        randomize_auto_attack_damage: combat.randomize_auto_attack_damage,
        min_npc_level_dmg_penalty: combat.min_npc_level_dmg_penalty,
        npc_dmg_penalty: &combat.npc_dmg_penalty,
        npc_crit_dmg_penalty: &combat.npc_crit_dmg_penalty,
    };
    calc_phys_dam(&input, shld, crit, ss, rng)
}

/// The rolls of `Creature.doAttackHitSimple` in its order: hit or miss, then (if not missed)
/// shield, critical hit and the damage (cast to `int`), and `attackpercent` applied as Java does.
pub fn calc_hit_simple(
    world: &World,
    attacker: ObjectId,
    target: ObjectId,
    attack_percent: f64,
    ss: bool,
    rng: &mut impl Rnd,
) -> HitOutcome {
    let miss = calc_hit_miss_for(world, attacker, target, rng);
    let mut outcome = HitOutcome {
        miss,
        shld: 0,
        crit: false,
        damage: 0,
    };
    if !miss {
        outcome.shld = calc_shld_use_for(world, attacker, target, rng);
        outcome.crit = calc_crit_for(world, attacker, target, rng);
        outcome.damage =
            calc_phys_dam_for(world, attacker, target, outcome.shld, outcome.crit, ss, rng) as i32;
        if attack_percent != 100.0 {
            outcome.damage = ((f64::from(outcome.damage) * attack_percent) / 100.0) as i32;
        }
    }
    outcome
}

/// Java `Formulas.calcAtkBreak(target, dmg)`.
pub fn calc_atk_break_for(world: &World, target: ObjectId, dmg: f64, rng: &mut impl Rnd) -> bool {
    let Some(t) = world.stats(target) else {
        return false;
    };
    let cancel = |rate: f64| t.calc(Stat::AttackCancel, rate);
    let input = AtkBreakInput {
        // M5: channeling and casting.
        is_channeling: false,
        is_casting_now: false,
        is_attacking_now: world.is_attacking_now(target),
        active_weapon_is_bow: weapon_type(&t) == Some(WeaponType::Bow),
        is_raid: t.is_raid(),
        is_invul: world.is_invul(target),
        men_bonus: t.bonus(BaseStat::Men),
        attack_cancel: &cancel,
        alt_game_cancel_cast: world.settings.combat.alt_game_cancel_cast,
        alt_game_cancel_bow: world.settings.combat.alt_game_cancel_bow,
    };
    calc_atk_break(&input, dmg, rng)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The geometry of `formulas_attack_hit.json`: the target at (100000, 100000) facing heading
    /// 16384, the attacker 40 units away in front, on the side and behind.
    #[test]
    fn position_matches_the_vector_geometry() {
        let t = (100_000, 100_000, 16_384);
        assert_eq!(position_from(100_022, 100_033, t.0, t.1, t.2), Position::Front);
        assert_eq!(position_from(100_040, 100_000, t.0, t.1, t.2), Position::Side);
        assert_eq!(position_from(99_978, 99_967, t.0, t.1, t.2), Position::Back);
    }
}
