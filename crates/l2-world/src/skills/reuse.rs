//! Skill reuse: the reuse time stamps, the short "disabled" delays and the all-skills lock of a
//! creature, and the `SkillCoolTime` list built from them.
// Java: entity/actor/Creature.java (addTimeStamp, removeTimeStamp, resetTimeStamps,
//       getSkillRemainingReuseTime, hasSkillReuse, getSkillReuseTimeStamp, enableSkill,
//       disableSkill, resetDisabledSkills, isSkillDisabled, disableAllSkills, enableAllSkills,
//       isAllSkillsDisabled)
// Java: entity/actor/holders/creature/TimeStamp.java (the skill constructor, getRemaining,
//       hasNotPassed)
// Java: network/serverpackets/SkillCoolTime.java
//
// # Time
// Java stamps reuse with `System.currentTimeMillis()`. The world stamps it with its own clock,
// `World::world_ms` (50 ms steps), so reuse follows the ticks like every other timer and tests are
// deterministic. What leaves the world (the `SkillCoolTime` packet, the reuse store of
// M5-SAVE-01) is the time *left*, which is the same on both clocks; [`ReuseTimeStamp`] carries it.
//
// # Order
// `_reuseTimeStampsSkills` is a `ConcurrentHashMap<Integer, TimeStamp>` keyed by the reuse hash
// code, and `SkillCoolTime` walks it in its iteration order. Stamps are never removed when they
// pass (only `removeTimeStamp` / `resetTimeStamps` remove them), so the table's size, and with it
// the order, depends on every stamp put so far; [`ReuseState`] keeps them in insertion order and
// [`concurrent_hash_map_order`] replays the buckets.

use std::collections::HashMap;

use l2_data::SkillTemplate;
use l2_protocol_game::server::enterworld::{
    SkillCoolTime, SkillCoolTimeEntry, remaining_seconds, reuse_seconds,
};
use l2_util::ObjectId;

use crate::effects::effect_flag;
use crate::world::World;

/// Java `TimeStamp` of a skill. `stamp_ms` is on the world clock; 0 means "passed".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SkillStamp {
    skill_id: i32,
    skill_level: i32,
    /// `_reuse`: the full reuse delay in milliseconds.
    reuse_ms: i64,
    /// `_stamp`.
    stamp_ms: i64,
}

impl SkillStamp {
    /// `getRemaining()` at `now`.
    fn remaining(&self, now: i64) -> i64 {
        if self.stamp_ms == 0 {
            return 0;
        }
        (self.stamp_ms - now).max(0)
    }

    /// `hasNotPassed()` at `now`.
    fn has_not_passed(&self, now: i64) -> bool {
        self.stamp_ms != 0 && now < self.stamp_ms
    }
}

/// One reuse time stamp as it leaves the world: `SkillCoolTime`, and the reuse store of
/// M5-SAVE-01 (Java `storeEffect` writes `getReuse()` and `getStamp()`, the stamp being "now +
/// remaining" on the wall clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReuseTimeStamp {
    /// The skill's reuse hash code (the map key).
    pub hash_code: i32,
    pub skill_id: i32,
    pub skill_level: i32,
    /// `getReuse()`: the full delay, in milliseconds.
    pub reuse_ms: i64,
    /// `getRemaining()`: milliseconds left, 0 when it passed.
    pub remaining_ms: i64,
}

/// The reuse fields of one creature (Java `Creature._reuseTimeStampsSkills`, `_disabledSkills`,
/// `_allSkillsDisabled`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ReuseState {
    /// By reuse hash code, in insertion order (see the module comment).
    stamps: Vec<(i32, SkillStamp)>,
    /// `_disabledSkills`: reuse hash code to the world ms the skill is enabled again
    /// (`i64::MAX`: until enabled).
    disabled: HashMap<i32, i64>,
    /// `_allSkillsDisabled`.
    pub(crate) all_skills_disabled: bool,
}

impl ReuseState {
    fn stamp(&self, hash: i32) -> Option<&SkillStamp> {
        self.stamps.iter().find(|(h, _)| *h == hash).map(|(_, s)| s)
    }

    /// `_reuseTimeStampsSkills.put(hash, stamp)`: a known key keeps its place.
    fn put(&mut self, hash: i32, stamp: SkillStamp) {
        match self.stamps.iter_mut().find(|(h, _)| *h == hash) {
            Some(entry) => entry.1 = stamp,
            None => self.stamps.push((hash, stamp)),
        }
    }
}

/// The iteration order of a `ConcurrentHashMap<Integer, V>` holding `keys` (inserted in this
/// order, no duplicates): by bucket `spread(h) & (n - 1)` of the final table size, then insertion
/// order within a bucket.
// Java: the table starts at 16 and doubles as soon as the count reaches `sizeCtl` (`n - n/4`),
// one entry earlier than `HashMap`. A bucket that would become a tree (8 entries in a table of 64)
// is far beyond the number of reuse stamps a character has.
pub(crate) fn concurrent_hash_map_order(keys: &[i32]) -> Vec<usize> {
    let mut capacity: usize = 16;
    while keys.len() >= capacity - capacity / 4 {
        capacity *= 2;
    }
    let bucket = |k: i32| {
        let h = k as u32;
        (((h ^ (h >> 16)) & 0x7fff_ffff) as usize) & (capacity - 1)
    };
    let mut indexed: Vec<(usize, usize)> = keys.iter().enumerate().map(|(i, &k)| (bucket(k), i)).collect();
    indexed.sort_unstable();
    indexed.into_iter().map(|(_, i)| i).collect()
}

impl World {
    fn reuse_state(&self, id: ObjectId) -> Option<&ReuseState> {
        self.cast.creatures.get(&id).map(|c| &c.reuse)
    }

    fn reuse_state_mut(&mut self, id: ObjectId) -> &mut ReuseState {
        &mut self.cast_state_mut(id).reuse
    }

    /// Java `Creature.addTimeStamp(skill, reuse)`: the skill is in reuse for `reuse_ms`.
    pub fn add_time_stamp(&mut self, id: ObjectId, skill: &SkillTemplate, reuse_ms: i64) {
        let now = self.world_ms();
        let stamp = SkillStamp {
            skill_id: skill.id,
            skill_level: skill.level,
            reuse_ms,
            stamp_ms: if reuse_ms != 0 { now + reuse_ms } else { 0 },
        };
        self.reuse_state_mut(id).put(skill.hash_code(), stamp);
    }

    /// Java `Creature.addTimeStamp(skill, reuse, systime)` as `restoreEffects` uses it: the stamp
    /// ends `remaining_ms` from now (M5-SAVE-01 turns Java's wall-clock stamp into the time
    /// left).
    pub fn restore_time_stamp(
        &mut self,
        id: ObjectId,
        skill: &SkillTemplate,
        reuse_ms: i64,
        remaining_ms: i64,
    ) {
        let now = self.world_ms();
        let stamp = SkillStamp {
            skill_id: skill.id,
            skill_level: skill.level,
            reuse_ms,
            stamp_ms: if remaining_ms > 0 { now + remaining_ms } else { 0 },
        };
        self.reuse_state_mut(id).put(skill.hash_code(), stamp);
    }

    /// Java `Creature.removeTimeStamp(skill)`.
    pub fn remove_time_stamp(&mut self, id: ObjectId, skill: &SkillTemplate) {
        if let Some(state) = self.cast.creatures.get_mut(&id) {
            let hash = skill.hash_code();
            state.reuse.stamps.retain(|(h, _)| *h != hash);
        }
    }

    /// Java `Creature.resetTimeStamps()`.
    pub fn reset_time_stamps(&mut self, id: ObjectId) {
        if let Some(state) = self.cast.creatures.get_mut(&id) {
            state.reuse.stamps.clear();
        }
    }

    /// Java `Creature.getSkillRemainingReuseTime(hashCode)`: milliseconds left, -1 without a stamp.
    pub fn skill_remaining_reuse_time(&self, id: ObjectId, hash_code: i32) -> i64 {
        let now = self.world_ms();
        self.reuse_state(id)
            .and_then(|r| r.stamp(hash_code))
            .map_or(-1, |s| s.remaining(now))
    }

    /// Java `Creature.hasSkillReuse(hashCode)`.
    pub fn has_skill_reuse(&self, id: ObjectId, hash_code: i32) -> bool {
        let now = self.world_ms();
        self.reuse_state(id)
            .and_then(|r| r.stamp(hash_code))
            .is_some_and(|s| s.has_not_passed(now))
    }

    /// The reuse stamps of a creature in Java's map order, passed ones included (Java keeps them
    /// until they are removed). For M5-SAVE-01's store.
    pub fn skill_reuse_time_stamps(&self, id: ObjectId) -> Vec<ReuseTimeStamp> {
        let now = self.world_ms();
        let Some(state) = self.reuse_state(id) else {
            return Vec::new();
        };
        let keys: Vec<i32> = state.stamps.iter().map(|(h, _)| *h).collect();
        concurrent_hash_map_order(&keys)
            .into_iter()
            .map(|i| {
                let (hash_code, s) = &state.stamps[i];
                ReuseTimeStamp {
                    hash_code: *hash_code,
                    skill_id: s.skill_id,
                    skill_level: s.skill_level,
                    reuse_ms: s.reuse_ms,
                    remaining_ms: s.remaining(now),
                }
            })
            .collect()
    }

    /// Java `new SkillCoolTime(player)`: the stamps that have not passed, in the map's order.
    pub fn skill_cool_time(&self, id: ObjectId) -> SkillCoolTime {
        let entries = self
            .skill_reuse_time_stamps(id)
            .into_iter()
            .filter(|ts| ts.remaining_ms > 0)
            .map(|ts| SkillCoolTimeEntry {
                skill_id: ts.skill_id,
                level: ts.skill_level,
                reuse_seconds: reuse_seconds(ts.reuse_ms, ts.remaining_ms),
                remaining_seconds: remaining_seconds(ts.remaining_ms),
            })
            .collect();
        SkillCoolTime { entries }
    }

    /// Java `Creature.enableSkill(skill)`.
    pub fn enable_skill(&mut self, id: ObjectId, skill: &SkillTemplate) {
        if let Some(state) = self.cast.creatures.get_mut(&id) {
            state.reuse.disabled.remove(&skill.hash_code());
        }
    }

    /// Java `Creature.disableSkill(skill, delay)`: a delay of 0 or less disables it "forever".
    pub fn disable_skill(&mut self, id: ObjectId, skill: &SkillTemplate, delay_ms: i64) {
        let until = if delay_ms > 0 {
            self.world_ms() + delay_ms
        } else {
            i64::MAX
        };
        self.reuse_state_mut(id).disabled.insert(skill.hash_code(), until);
    }

    /// Java `Creature.resetDisabledSkills()`.
    pub fn reset_disabled_skills(&mut self, id: ObjectId) {
        if let Some(state) = self.cast.creatures.get_mut(&id) {
            state.reuse.disabled.clear();
        }
    }

    /// Java `Creature.isSkillDisabled(skill)`: every skill is locked, the skill is in reuse, or it
    /// was disabled for a while.
    pub fn is_skill_disabled(&mut self, id: ObjectId, skill: &SkillTemplate) -> bool {
        if self.is_all_skills_disabled(id) {
            return true;
        }
        let hash = skill.hash_code();
        if self.has_skill_reuse(id, hash) {
            return true;
        }
        let now = self.world_ms();
        let Some(state) = self.cast.creatures.get_mut(&id) else {
            return false;
        };
        let Some(&stamp) = state.reuse.disabled.get(&hash) else {
            return false;
        };
        if stamp < now {
            state.reuse.disabled.remove(&hash);
            return false;
        }
        true
    }

    /// Java `Creature.disableAllSkills()`.
    pub fn disable_all_skills(&mut self, id: ObjectId) {
        self.reuse_state_mut(id).all_skills_disabled = true;
    }

    /// Java `Creature.enableAllSkills()`.
    pub fn enable_all_skills(&mut self, id: ObjectId) {
        if let Some(state) = self.cast.creatures.get_mut(&id) {
            state.reuse.all_skills_disabled = false;
        }
    }

    /// Java `Creature.isAllSkillsDisabled()`: the lock, or stunned, asleep or paralyzed.
    // NOTE: Java's `isParalyzed()` also reads `_isParalyzed` (`setParalyzed`, used by scripts and
    // olympiad); nothing sets it in the world, so only the effect flag counts.
    pub fn is_all_skills_disabled(&self, id: ObjectId) -> bool {
        self.reuse_state(id).is_some_and(|r| r.all_skills_disabled)
            || self.is_affected(
                id,
                effect_flag::STUNNED | effect_flag::SLEEP | effect_flag::PARALYZED,
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_hash_map_order_buckets_by_spread_hash() {
        // 1177 * 1021 + 1 = 1201718: (1201718 ^ 18) & 15 = 6 ^ 2 = 4;
        // 1040 * 1021 + 3 = 1061843: (1061843 ^ 16) & 15 = 3.
        let keys = [1_201_718, 1_061_843];
        assert_eq!(concurrent_hash_map_order(&keys), vec![1, 0]);
    }

    #[test]
    fn concurrent_hash_map_grows_one_entry_earlier_than_hash_map() {
        // 12 keys: a HashMap stays at 16 buckets, a ConcurrentHashMap has 32. Keys 1 and 17 share
        // bucket 1 of 16 but not of 32.
        let mut keys: Vec<i32> = (100..110).collect();
        keys.push(17);
        keys.push(1);
        let order = concurrent_hash_map_order(&keys);
        let pos = |k: i32| order.iter().position(|&i| keys[i] == k).unwrap();
        assert!(pos(1) < pos(17));
    }

    #[test]
    fn stamp_remaining_and_passed() {
        let s = SkillStamp {
            skill_id: 1,
            skill_level: 1,
            reuse_ms: 1000,
            stamp_ms: 5000,
        };
        assert_eq!(s.remaining(4000), 1000);
        assert!(s.has_not_passed(4999));
        assert!(!s.has_not_passed(5000));
        assert_eq!(s.remaining(6000), 0);
    }
}
