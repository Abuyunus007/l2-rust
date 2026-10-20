//! Java `Formulas.calcCancelStealEffects` and `calcCancelSuccess`: which buffs or debuffs a dispel
//! or a steal takes. Shared by `DispelByCategory` and `StealAbnormal`. M5-FORM-02 does not list
//! these, so they live with the handlers that use them.
// Java: mechanics/stats/Formulas.java (calcCancelStealEffects, calcCancelSuccess)

use l2_data::SkillTemplate;
use l2_util::ObjectId;

use crate::effects::{ListKind, SkillKey};
use crate::formulas::Rnd;
use crate::stats::Stat;
use crate::world::World;

/// One buff or debuff a dispel or steal takes: the info, its skill and the seconds it has left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Canceled {
    pub info: crate::effects::BuffInfoId,
    pub skill: SkillKey,
    pub time: i32,
}

/// Java `Formulas.calcCancelStealEffects(creature, target, skill, slot, rate, max)`. `slot` is
/// `buff` (the buffs, triggered buffs and dances, taken from the newest) or `debuff`; any other
/// slot takes nothing.
// NOTE(java-quirk): Java's `switch (slot)` throws on a missing slot; here a missing slot takes
// nothing.
// NOTE: `canBeStolen()` needs the hero and GM skill lists of `SkillTreeData`, which are not loaded
// (see `l2-data` `skill/template.rs`): those skills count as not hero and not GM.
pub(crate) fn calc_cancel_steal_effects(
    world: &mut World,
    creature: ObjectId,
    target: ObjectId,
    skill: &SkillTemplate,
    slot: Option<&str>,
    rate: i32,
    max: i32,
) -> Vec<Canceled> {
    let Some(table) = world.skill_table() else {
        return Vec::new();
    };
    let max = usize::try_from(max).unwrap_or(0);
    let now = world.tick;
    let Some(list) = world.effect_list(target) else {
        return Vec::new();
    };
    match slot {
        Some("buff") => {
            // Java: buffs, then the triggered buffs, then the dances, taken from the last one.
            let candidates: Vec<Canceled> = [ListKind::Buffs, ListKind::Triggered, ListKind::Dances]
                .into_iter()
                .flat_map(|kind| list.queue(kind).iter())
                .map(|info| Canceled {
                    info: info.id(),
                    skill: info.skill(),
                    time: info.time(now),
                })
                .collect();
            let vuln = world
                .stats(target)
                .map_or(0.0, |v| v.calc_stat(Stat::CancelVuln, 0.0, Some(target), None));
            let prof = world
                .stats(creature)
                .map_or(0.0, |v| v.calc_stat(Stat::CancelProf, 0.0, Some(target), None));
            // Resist Modifier.
            let res_mod = 1.0 + (-(vuln + prof)) / 100.0;
            let final_rate = f64::from(rate) / res_mod;
            let mut canceled = Vec::new();
            for candidate in candidates.into_iter().rev() {
                let Some(info_skill) = table.get(candidate.skill.id, candidate.skill.level) else {
                    continue;
                };
                if !can_be_stolen(info_skill) || !cancel_success(world, info_skill, skill, final_rate as i32)
                {
                    continue;
                }
                canceled.push(candidate);
                if canceled.len() >= max {
                    break;
                }
            }
            canceled
        }
        Some("debuff") => {
            let candidates: Vec<Canceled> = list
                .queue(ListKind::Debuffs)
                .iter()
                .map(|info| Canceled {
                    info: info.id(),
                    skill: info.skill(),
                    time: info.time(now),
                })
                .collect();
            let mut canceled = Vec::new();
            for candidate in candidates.into_iter().rev() {
                let Some(info_skill) = table.get(candidate.skill.id, candidate.skill.level) else {
                    continue;
                };
                if info_skill.is_debuff && info_skill.can_be_dispeled && world.rng().get_int(100) <= rate {
                    canceled.push(candidate);
                    if canceled.len() >= max {
                        break;
                    }
                }
            }
            canceled
        }
        _ => Vec::new(),
    }
}

/// Java `Skill.canBeStolen()`, without the hero and GM checks (see the note above).
fn can_be_stolen(skill: &SkillTemplate) -> bool {
    // Java `CommonSkill.CARAVANS_SECRET_MEDICINE` and `CommonSkill.SERVITOR_SHARE`.
    const CARAVANS_SECRET_MEDICINE: i32 = 2341;
    const SERVITOR_SHARE: i32 = 1557;
    !skill.is_passive()
        && !skill.is_toggle()
        && !skill.is_debuff
        && !(skill.is_static() && skill.id != CARAVANS_SECRET_MEDICINE)
        && skill.can_be_dispeled
        && skill.id != SERVITOR_SHARE
}

/// Java `Formulas.calcCancelSuccess(info, cancelMagicLvl, rate, skill)`: the level bonus of the
/// cancelling skill over the taken one, then the chance clamped to the cancelling skill's
/// minimum and maximum.
// NOTE(java-quirk): Java's `Math.clamp` throws when the minimum is above the maximum; here the
// maximum wins over the minimum and nothing throws. The skill data never has that.
fn cancel_success(world: &mut World, info_skill: &SkillTemplate, skill: &SkillTemplate, rate: i32) -> bool {
    let cancel_magic_lvl = skill.magic_level;
    let factor = if info_skill.magic_level > 0 {
        1.0 + f64::from(cancel_magic_lvl - info_skill.magic_level) / 100.0
    } else {
        1.0
    };
    let chance = (f64::from(rate) * factor) as i32;
    let min = skill.min_chance;
    let max = skill.max_chance;
    let clamped = chance.min(max).max(min);
    world.rng().get_int(100) < clamped
}
