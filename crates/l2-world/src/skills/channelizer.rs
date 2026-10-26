//! Channeling skills: a creature channels a skill (`SkillChannelizer`), the creatures it is
//! channeled at know who does (`SkillChannelized`).
// Java: mechanics/skill/SkillChannelizer.java
// Java: mechanics/skill/SkillChannelized.java
// Java: entity/actor/Creature.java (isChanneling, getSkillChannelizer, isChannelized,
//       getSkillChannelized)
//
// # Shape
// A cast of a skill with `operateType` CA1/CA5/DA... and a `channelingSkillId` starts the
// channelizer at `beginCast` (`cast.rs`) and stops it when the cast ends (`completeMagicFinalizer`)
// or is aborted (`abortCast`). While it runs, the periodic task (Java: `scheduleAtFixedRate`, here
// a [`CastTimer::ChannelTick`] that schedules its successor first) pays the MP of the tick, lists
// the skill's targets, and gives each one in range the `channelingSkillId` skill at a level equal
// to the number of creatures that channel it at them (the Life Drain / Mass-Wave "stacking").
//
// Both halves live in the [`CastState`](super::cast::CastState) of the creature.
//
// # Quirks kept
// - Every tick adds the channelizer to the [`SkillChannelized`] of *every* listed target, in range
//   or not, and only the targets of the last tick are removed when the channel stops.

use std::collections::BTreeMap;

use l2_data::SkillTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{MagicSkillLaunched, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::cast::CastTimer;
use crate::effects::SkillKey;
use crate::effects::apply::has_negative_effect;
use crate::inventory::handlers::shots::ShotType;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// Java `SkillChannelizer`: the skill a creature channels, its periodic task and the creatures it
/// channels it at (`_channelized`, `None` until the first tick found targets).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Channelizer {
    skill: Option<SkillKey>,
    task: Option<TimerId>,
    channelized: Option<Vec<ObjectId>>,
}

impl Channelizer {
    /// Java `isChanneling()`: the task runs.
    pub fn is_channeling(&self) -> bool {
        self.task.is_some()
    }

    /// Java `hasChannelized()`.
    pub fn has_channelized(&self) -> bool {
        self.channelized.is_some()
    }

    /// Java `getSkill()`.
    pub fn skill(&self) -> Option<SkillKey> {
        self.skill
    }

    /// Java `getChannelized()`.
    pub fn channelized(&self) -> Option<&[ObjectId]> {
        self.channelized.as_deref()
    }

    /// The pending tick timer.
    pub(crate) fn task(&self) -> Option<TimerId> {
        self.task
    }
}

/// Java `SkillChannelized`: who channels which skill at this creature, by the id of the skill
/// that is channeled (`channelingSkillId`). Each list is a set in insertion order (Java:
/// `ConcurrentHashMap` by object id).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillChannelized {
    channelizers: BTreeMap<i32, Vec<ObjectId>>,
}

impl SkillChannelized {
    /// Java `addChannelizer(skillId, channelizer)`.
    pub fn add_channelizer(&mut self, skill_id: i32, channelizer: ObjectId) {
        let list = self.channelizers.entry(skill_id).or_default();
        if !list.contains(&channelizer) {
            list.push(channelizer);
        }
    }

    /// Java `removeChannelizer(skillId, channelizer)`.
    pub fn remove_channelizer(&mut self, skill_id: i32, channelizer: ObjectId) {
        if let Some(list) = self.channelizers.get_mut(&skill_id) {
            list.retain(|c| *c != channelizer);
        }
    }

    /// Java `getChannerlizersSize(skillId)` (sic).
    pub fn channelizers_size(&self, skill_id: i32) -> usize {
        self.channelizers.get(&skill_id).map_or(0, Vec::len)
    }

    /// Java `isChannelized()`: any channelizer is left.
    pub fn is_channelized(&self) -> bool {
        self.channelizers.values().any(|list| !list.is_empty())
    }

    /// The channelizers of `abortChannelization()`; the map is empty afterwards.
    fn take_all(&mut self) -> Vec<ObjectId> {
        let all = self.channelizers.values().flatten().copied().collect();
        self.channelizers.clear();
        all
    }
}

impl World {
    /// Java `Creature.isChanneling()`.
    pub fn is_channeling(&self, id: ObjectId) -> bool {
        self.cast
            .creatures
            .get(&id)
            .is_some_and(|c| c.channelizer.is_channeling())
    }

    /// Java `getSkillChannelizer().hasChannelized()`.
    pub fn has_channelized(&self, id: ObjectId) -> bool {
        self.cast
            .creatures
            .get(&id)
            .is_some_and(|c| c.channelizer.has_channelized())
    }

    /// Java `Creature.isChannelized()`.
    pub fn is_channelized(&self, id: ObjectId) -> bool {
        self.cast
            .creatures
            .get(&id)
            .is_some_and(|c| c.channelized.is_channelized())
    }

    /// Java `getSkillChannelizer().getSkill()`.
    pub fn channeled_skill(&self, id: ObjectId) -> Option<SkillKey> {
        self.cast.creatures.get(&id).and_then(|c| c.channelizer.skill())
    }

    /// Java `getSkillChannelized().getChannerlizersSize(skillId)`.
    pub fn channelizers_size(&self, id: ObjectId, skill_id: i32) -> usize {
        self.cast
            .creatures
            .get(&id)
            .map_or(0, |c| c.channelized.channelizers_size(skill_id))
    }

    /// Java `SkillChannelizer.startChanneling(skill)`.
    pub(crate) fn start_channeling(&mut self, id: ObjectId, skill: &SkillTemplate) {
        // Verify for same status.
        if self.is_channeling(id) {
            warn!("Character: {id:?} is attempting to channel skill but he already does!");
            return;
        }
        // Start channeling.
        let delay = u64::try_from(skill.channeling_tick_initial_delay).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        let task = self
            .timers
            .schedule_at(due, TimerKind::Cast(CastTimer::ChannelTick { channelizer: id }));
        let state = self.cast_state_mut(id);
        state.channelizer.skill = Some(SkillKey::of(skill));
        state.channelizer.task = Some(task);
    }

    /// Java `SkillChannelizer.stopChanneling()`.
    pub(crate) fn stop_channeling(&mut self, id: ObjectId) {
        // Verify for same status.
        if !self.is_channeling(id) {
            warn!("Character: {id:?} is attempting to stop channel skill but he does not!");
            return;
        }
        let state = self.cast_state_mut(id);
        // Cancel the task and unset it.
        let task = state.channelizer.task.take();
        let channelized = state.channelizer.channelized.take();
        let skill = state.channelizer.skill.take();
        if let Some(task) = task {
            self.timers.cancel(task);
        }
        // Cancel target channelization and unset it.
        let channeling_skill_id = skill
            .and_then(|key| {
                self.skill_table()
                    .and_then(|t| t.get(key.id, key.level).map(|s| s.channeling_skill_id))
            })
            .unwrap_or(0);
        for creature in channelized.into_iter().flatten() {
            if let Some(state) = self.cast.creatures.get_mut(&creature) {
                state.channelized.remove_channelizer(channeling_skill_id, id);
            }
        }
    }

    /// Java `SkillChannelized.abortChannelization()` of `id`: everybody who channels a skill at it
    /// stops casting.
    pub(crate) fn abort_channelization(&mut self, id: ObjectId) {
        let channelizers = match self.cast.creatures.get_mut(&id) {
            Some(state) => state.channelized.take_all(),
            None => return,
        };
        for channelizer in channelizers {
            self.abort_cast(channelizer);
        }
    }

    /// The tick timer: Java `ScheduledThreadPoolExecutor.scheduleAtFixedRate` re-arms the task
    /// before the next run, here the successor is armed before `SkillChannelizer.run()` so that an
    /// `abortCast` inside the run cancels it.
    pub(crate) fn on_channel_tick(&mut self, id: ObjectId) {
        let Some(key) = self.channeled_skill(id).filter(|_| self.is_channeling(id)) else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        let interval = u64::try_from(skill.channeling_tick_interval).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(interval);
        let next = self
            .timers
            .schedule_at(due, TimerKind::Cast(CastTimer::ChannelTick { channelizer: id }));
        self.cast_state_mut(id).channelizer.task = Some(next);
        self.run_channelizer(id, skill);
    }

    /// Java `SkillChannelizer.run()`.
    fn run_channelizer(&mut self, id: ObjectId, skill: &SkillTemplate) {
        if skill.mp_per_channeling > 0 {
            // Validate mana per tick.
            let cur_mp = self.core_of(id).map_or(0.0, |c| c.cur_mp);
            if cur_mp < f64::from(skill.mp_per_channeling) {
                if self.is_player(id) {
                    self.send(
                        id,
                        &SystemMessage::new(SystemMessageId::YourSkillWasDeactivatedDueToLackOfMp),
                    );
                }
                self.abort_cast(id);
                return;
            }
            // Reduce mana per tick.
            self.reduce_mp(id, f64::from(skill.mp_per_channeling));
        }

        // Apply channeling skills on the targets.
        if skill.channeling_skill_id <= 0 {
            return;
        }
        let Some(table) = self.skill_table() else {
            return;
        };
        if table.get(skill.channeling_skill_id, 1).is_none() {
            warn!(
                "SkillChannelizer: skill {}-{} couldn't find effect id skill: {} !",
                skill.id, skill.level, skill.channeling_skill_id
            );
            self.abort_cast(id);
            return;
        }

        let ctx = self.cast_target_context(id);
        let result = self.skill_target_list_current(skill, id, false, &ctx);
        self.apply_target_result(id, skill, &result);
        let mut target_list = Vec::new();
        for target in result.targets {
            if self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
                target_list.push(target);
                self.cast_state_mut(target)
                    .channelized
                    .add_channelizer(skill.channeling_skill_id, id);
            }
        }
        if target_list.is_empty() {
            return;
        }
        self.cast_state_mut(id).channelizer.channelized = Some(target_list.clone());

        for creature in target_list {
            if !self.check_if_in_range(skill.effect_range, id, creature, true) {
                continue;
            }
            if !self.can_see_target(id, creature) {
                continue;
            }
            let max_level = table.max_level(skill.channeling_skill_id);
            let size = self.channelizers_size(creature, skill.channeling_skill_id);
            let level = i32::try_from(size).unwrap_or(i32::MAX).min(max_level);
            if level == 0 {
                continue;
            }
            let applied = self
                .effect_list(creature)
                .and_then(|list| list.info_by_skill_id(skill.channeling_skill_id))
                .map(|info| info.skill.level);
            if applied.is_none_or(|applied_level| applied_level < level) {
                let Some(channeling_skill) = table.get(skill.channeling_skill_id, level) else {
                    warn!(
                        "SkillChannelizer: Non existent channeling skill requested: {}",
                        skill.id
                    );
                    self.abort_cast(id);
                    return;
                };
                // Update PvP status.
                if self.is_playable(creature) && self.is_player(id) && has_negative_effect(channeling_skill) {
                    self.update_pvp_status_target(id, creature);
                }
                self.apply_effects(id, creature, SkillKey::of(channeling_skill));

                // Reduce shots.
                if self.skill_uses_spirit_shot(skill) {
                    let shot = if self.is_charged_shot(id, ShotType::BlessedSpiritshots) {
                        ShotType::BlessedSpiritshots
                    } else {
                        ShotType::Spiritshots
                    };
                    self.set_charged_shot(id, shot, false);
                } else {
                    self.set_charged_shot(id, ShotType::Soulshots, false);
                }
                // Shots are re-charged every cast.
                let (soul, spirit) = (
                    self.skill_uses_soul_shot(skill),
                    self.skill_uses_spirit_shot(skill),
                );
                self.recharge_shots(id, soul, spirit);
            }
            // `broadcastSkillPacket(packet, target)` is a plain broadcast around the channelizer.
            self.broadcast_creature_packet(
                id,
                &MagicSkillLaunched {
                    creature_id: id.0,
                    skill_id: skill.id,
                    skill_level: skill.level,
                    target_ids: vec![creature.0],
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SkillChannelized`: a channelizer is a set per skill id, the size counts the casters, and
    /// `isChannelized` is true while any is left.
    #[test]
    fn channelized_counts_distinct_channelizers_per_skill() {
        let mut c = SkillChannelized::default();
        assert!(!c.is_channelized());
        assert_eq!(c.channelizers_size(1419), 0);
        c.add_channelizer(1419, ObjectId(1));
        c.add_channelizer(1419, ObjectId(1));
        c.add_channelizer(1419, ObjectId(2));
        c.add_channelizer(1420, ObjectId(1));
        assert_eq!(c.channelizers_size(1419), 2);
        assert_eq!(c.channelizers_size(1420), 1);
        assert!(c.is_channelized());
        c.remove_channelizer(1419, ObjectId(1));
        assert_eq!(c.channelizers_size(1419), 1);
        c.remove_channelizer(1419, ObjectId(2));
        c.remove_channelizer(1420, ObjectId(1));
        assert!(!c.is_channelized());
    }

    /// `abortChannelization`: everybody who channels at the creature, then the map is empty.
    #[test]
    fn abort_takes_every_channelizer_and_clears() {
        let mut c = SkillChannelized::default();
        c.add_channelizer(1, ObjectId(7));
        c.add_channelizer(2, ObjectId(8));
        let mut all = c.take_all();
        all.sort_by_key(|o| o.0);
        assert_eq!(all, [ObjectId(7), ObjectId(8)]);
        assert!(!c.is_channelized());
    }
}
