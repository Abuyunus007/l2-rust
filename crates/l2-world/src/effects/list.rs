//! A creature's effect list: the queues of buff infos, the abnormal type stack, the slot limits,
//! the effect flags, the icon updates and the buff finish task.
// Java: entity/actor/holders/creature/EffectList.java (all of it)
// Java: mechanics/skill/BuffFinishTask.java
// Java: entity/actor/Creature.java (stopAllEffects*, stopSkillEffects, stopEffects*,
//       isAffected*, addInvulAgainst/removeInvulAgainst/isInvulAgainst, the abnormal visual
//       effect masks, addBuffInfoTime/removeBuffInfoTime)
// Java: entity/actor/Player.java (stopAllEffects*, updateAndBroadcastStatus)
// Java: entity/actor/stat/CreatureStat.java (getMaxBuffCount, setMaxBuffCount)
//
// Java's queues are `ConcurrentLinkedQueue`s iterated while entries are removed (weakly
// consistent): a loop that removes visits every entry that was there when it started and is
// still there when it is reached. The loops here take the ids first and skip the ones that are
// gone, which visits the same entries.

use std::collections::{BTreeSet, HashMap};

use l2_data::skill::{AbnormalVisualEffect, EffectScope};
use l2_data::stats::AbnormalType;
use l2_protocol_game::server::{AbnormalStatusUpdate, EffectIcon, ShortBuffStatusUpdate};
use l2_util::ObjectId;
use tracing::warn;

use super::buff_info::BuffInfo;
use super::registry::EffectType;
use super::{BuffInfoId, SkillFinishType};
use crate::timer::{TimerId, TimerKind};
use crate::world::World;

/// Java `EffectList`'s icon task waits 300 ms.
const ICON_UPDATE_MS: u64 = 300;
/// Java `Player.updateAndBroadcastStatus` waits 50 ms.
const UPDATE_AND_BROADCAST_STATUS_MS: u64 = 50;
/// `BuffFinishTask` runs every second.
const BUFF_FINISH_PERIOD_TICKS: u64 = 20;
/// Java `CreatureStat.DIVINE_INSPIRATION`: each level is one more buff slot.
const DIVINE_INSPIRATION: i32 = 1405;

/// Which queue of an effect list a skill's infos go to (Java `getEffectList(skill)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListKind {
    Buffs,
    Triggered,
    Dances,
    Toggles,
    Debuffs,
    Passives,
}

impl ListKind {
    /// `getEffects()` and the icon order: buffs, triggered, dances, toggles, debuffs.
    pub const ACTIVE: [ListKind; 5] = [
        ListKind::Buffs,
        ListKind::Triggered,
        ListKind::Dances,
        ListKind::Toggles,
        ListKind::Debuffs,
    ];

    /// The queue of a skill: passive, debuff, triggered, dance, toggle, else buff (in that order
    /// of tests).
    pub(crate) fn of(facts: &super::buff_info::SkillFacts) -> Self {
        if facts.passive {
            ListKind::Passives
        } else if facts.debuff {
            ListKind::Debuffs
        } else if facts.triggered {
            ListKind::Triggered
        } else if facts.dance {
            ListKind::Dances
        } else if facts.toggle {
            ListKind::Toggles
        } else {
            ListKind::Buffs
        }
    }
}

/// One `AbnormalStatusUpdate` entry: `getDisplayId()`, `getDisplayLevel()`, `getTime()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconEntry {
    pub skill_id: i32,
    pub skill_level: i32,
    pub time: i32,
}

/// Java `BuffFinishTask`: the timed infos with the seconds counted so far, and the 1 s task.
#[derive(Debug, Clone, Default)]
pub(crate) struct FinishTask {
    entries: Vec<(BuffInfoId, i32)>,
    timer: Option<TimerId>,
    /// The tick of the running task's next run.
    next_due: u64,
}

/// Java `EffectList` with the effect parts of its `Creature`.
#[derive(Debug, Clone)]
pub struct EffectList {
    buffs: Vec<BuffInfo>,
    triggered: Vec<BuffInfo>,
    dances: Vec<BuffInfo>,
    toggles: Vec<BuffInfo>,
    debuffs: Vec<BuffInfo>,
    passives: Vec<BuffInfo>,
    /// `_stackedEffects`: the info in use for each abnormal type.
    stacked: HashMap<AbnormalType, BuffInfoId>,
    /// `_blockedAbnormalTypes`.
    blocked: BTreeSet<AbnormalType>,
    /// `_shortBuff`.
    pub(crate) short_buff: Option<BuffInfoId>,
    has_buffs_removed_on_any_action: bool,
    has_buffs_removed_on_damage: bool,
    has_debuffs_removed_on_damage: bool,
    effect_flags: u32,
    party_only: bool,
    /// `_hiddenBuffs`.
    hidden_buffs: i32,
    icon_task: Option<TimerId>,
    finish: FinishTask,
    /// `CreatureStat._maxBuffCount`.
    max_buff_count: i32,
    /// `Creature._abnormalVisualEffects`, `_abnormalVisualEffectsSpecial`, `_abnormalVisualEffectsEvent`.
    ave: i32,
    ave_special: i32,
    ave_event: i32,
    /// `Creature._invulAgainst`: skill id to (level, instances).
    invul_against: HashMap<i32, (i32, i32)>,
}

impl EffectList {
    pub(crate) fn new(max_buff_count: i32) -> Self {
        Self {
            buffs: Vec::new(),
            triggered: Vec::new(),
            dances: Vec::new(),
            toggles: Vec::new(),
            debuffs: Vec::new(),
            passives: Vec::new(),
            stacked: HashMap::new(),
            blocked: BTreeSet::new(),
            short_buff: None,
            has_buffs_removed_on_any_action: false,
            has_buffs_removed_on_damage: false,
            has_debuffs_removed_on_damage: false,
            effect_flags: 0,
            party_only: false,
            hidden_buffs: 0,
            icon_task: None,
            finish: FinishTask::default(),
            max_buff_count,
            ave: 0,
            ave_special: 0,
            ave_event: 0,
            invul_against: HashMap::new(),
        }
    }

    /// One queue.
    pub fn queue(&self, kind: ListKind) -> &[BuffInfo] {
        match kind {
            ListKind::Buffs => &self.buffs,
            ListKind::Triggered => &self.triggered,
            ListKind::Dances => &self.dances,
            ListKind::Toggles => &self.toggles,
            ListKind::Debuffs => &self.debuffs,
            ListKind::Passives => &self.passives,
        }
    }

    fn queue_mut(&mut self, kind: ListKind) -> &mut Vec<BuffInfo> {
        match kind {
            ListKind::Buffs => &mut self.buffs,
            ListKind::Triggered => &mut self.triggered,
            ListKind::Dances => &mut self.dances,
            ListKind::Toggles => &mut self.toggles,
            ListKind::Debuffs => &mut self.debuffs,
            ListKind::Passives => &mut self.passives,
        }
    }

    /// Java `getBuffs()`, `getTriggered()`, ...: see [`Self::queue`].
    pub fn buffs(&self) -> &[BuffInfo] {
        &self.buffs
    }

    pub fn passives(&self) -> &[BuffInfo] {
        &self.passives
    }

    /// Java `getEffects()`: every info except the passives, in queue order.
    pub fn effects(&self) -> impl Iterator<Item = &BuffInfo> {
        ListKind::ACTIVE.into_iter().flat_map(|k| self.queue(k).iter())
    }

    /// Every info, the passives last.
    fn all(&self) -> impl Iterator<Item = &BuffInfo> {
        self.effects().chain(self.passives.iter())
    }

    /// The info `id` in any queue.
    pub fn get(&self, id: BuffInfoId) -> Option<&BuffInfo> {
        self.all().find(|i| i.id == id)
    }

    pub(crate) fn get_mut(&mut self, id: BuffInfoId) -> Option<&mut BuffInfo> {
        for kind in ListKind::ACTIVE.into_iter().chain([ListKind::Passives]) {
            if let Some(at) = self.queue(kind).iter().position(|i| i.id == id) {
                return self.queue_mut(kind).get_mut(at);
            }
        }
        None
    }

    /// `queue.remove(info)`: the info, if it still is in that queue.
    fn take(&mut self, kind: ListKind, id: BuffInfoId) -> Option<BuffInfo> {
        let queue = self.queue_mut(kind);
        let at = queue.iter().position(|i| i.id == id)?;
        Some(queue.remove(at))
    }

    fn ids(&self, kind: ListKind) -> Vec<BuffInfoId> {
        self.queue(kind).iter().map(|i| i.id).collect()
    }

    /// Java `isEmpty()`: no info outside the passives.
    pub fn is_empty(&self) -> bool {
        ListKind::ACTIVE.into_iter().all(|k| self.queue(k).is_empty())
    }

    /// Java `getBuffInfoBySkillId(skillId)`: buffs, triggered, dances, toggles, debuffs, then
    /// passives.
    pub fn info_by_skill_id(&self, skill_id: i32) -> Option<&BuffInfo> {
        self.all().find(|i| i.skill.id == skill_id)
    }

    /// Java `isAffectedBySkill(skillId)`.
    pub fn is_affected_by_skill(&self, skill_id: i32) -> bool {
        self.info_by_skill_id(skill_id).is_some()
    }

    /// Java `getBuffInfoByAbnormalType(type)`.
    pub fn info_by_abnormal_type(&self, abnormal_type: AbnormalType) -> Option<&BuffInfo> {
        self.stacked.get(&abnormal_type).and_then(|&id| self.get(id))
    }

    /// Java `isAffectedByAbnormalType(type)`.
    pub fn is_affected_by_abnormal_type(&self, abnormal_type: AbnormalType) -> bool {
        self.stacked.contains_key(&abnormal_type)
    }

    /// Java `getFirstEffect(type)`: the first info (passives excluded) with an effect of `type`.
    pub fn first_effect(&self, effect_type: EffectType) -> Option<&BuffInfo> {
        self.effects()
            .find(|i| i.effects.iter().any(|e| e.handler.effect_type() == effect_type))
    }

    /// Java `isAffected(flag)`.
    pub fn is_affected(&self, flag: u32) -> bool {
        self.effect_flags & flag != 0
    }

    /// Java `getBuffCount()`: the buffs without the hidden ones and the short buff.
    pub fn buff_count(&self) -> i32 {
        if self.buffs.is_empty() {
            return 0;
        }
        len_i32(&self.buffs) - self.hidden_buffs - i32::from(self.short_buff.is_some())
    }

    /// Java `getDanceCount()`.
    pub fn dance_count(&self) -> i32 {
        len_i32(&self.dances)
    }

    /// Java `getTriggeredBuffCount()`.
    pub fn triggered_buff_count(&self) -> i32 {
        len_i32(&self.triggered)
    }

    /// Java `getHiddenBuffsCount()`.
    pub fn hidden_buffs_count(&self) -> i32 {
        self.hidden_buffs
    }

    /// Java `getShortBuff()`.
    pub fn short_buff(&self) -> Option<BuffInfoId> {
        self.short_buff
    }

    /// Java `getBlockedAbnormalTypes()`.
    pub fn blocked_abnormal_types(&self) -> &BTreeSet<AbnormalType> {
        &self.blocked
    }

    /// Java `addBlockedAbnormalTypes(types)`.
    pub fn add_blocked_abnormal_types(&mut self, types: impl IntoIterator<Item = AbnormalType>) {
        self.blocked.extend(types);
    }

    /// Java `removeBlockedAbnormalTypes(types)`: whether the set changed.
    pub fn remove_blocked_abnormal_types(&mut self, types: impl IntoIterator<Item = AbnormalType>) -> bool {
        let mut changed = false;
        for t in types {
            changed |= self.blocked.remove(&t);
        }
        changed
    }

    /// Java `CreatureStat.setMaxBuffCount(count)` (the `EnlargeAbnormalSlot` effect).
    pub fn set_max_buff_count(&mut self, count: i32) {
        self.max_buff_count = count;
    }

    /// Java `Creature.getAbnormalVisualEffects()`, `...Special()`, `...Event()`.
    pub fn abnormal_visual_effects(&self) -> (i32, i32, i32) {
        (self.ave, self.ave_special, self.ave_event)
    }

    /// Java `Creature.hasAbnormalVisualEffect(ave)`.
    pub fn has_abnormal_visual_effect(&self, ave: AbnormalVisualEffect) -> bool {
        let mask = ave.mask();
        let field = if ave.is_event() {
            self.ave_event
        } else if ave.is_special() {
            self.ave_special
        } else {
            self.ave
        };
        field & mask == mask
    }

    /// Java `Creature.startAbnormalVisualEffect(false, aves)`.
    pub fn start_abnormal_visual_effects(&mut self, aves: &[AbnormalVisualEffect]) {
        for ave in aves {
            if ave.is_event() {
                self.ave_event |= ave.mask();
            } else if ave.is_special() {
                self.ave_special |= ave.mask();
            } else {
                self.ave |= ave.mask();
            }
        }
    }

    /// Java `Creature.stopAbnormalVisualEffect(false, aves)`.
    pub fn stop_abnormal_visual_effects(&mut self, aves: &[AbnormalVisualEffect]) {
        for ave in aves {
            if ave.is_event() {
                self.ave_event &= !ave.mask();
            } else if ave.is_special() {
                self.ave_special &= !ave.mask();
            } else {
                self.ave &= !ave.mask();
            }
        }
    }

    /// Java `Creature.addInvulAgainst(holder)` (the `ResistSkill` effect).
    pub fn add_invul_against(&mut self, skill_id: i32, skill_level: i32) {
        self.invul_against
            .entry(skill_id)
            .and_modify(|(_, instances)| *instances += 1)
            .or_insert((skill_level, 1));
    }

    /// Java `Creature.removeInvulAgainst(holder)`.
    pub fn remove_invul_against(&mut self, skill_id: i32) {
        if let Some((_, instances)) = self.invul_against.get_mut(&skill_id) {
            *instances -= 1;
            if *instances < 1 {
                self.invul_against.remove(&skill_id);
            }
        }
    }

    /// Java `Creature.isInvulAgainst(skillId, skillLevel)`.
    pub fn is_invul_against(&self, skill_id: i32, skill_level: i32) -> bool {
        self.invul_against
            .get(&skill_id)
            .is_some_and(|&(level, _)| level < 1 || level == skill_level)
    }

    /// Java `doesStack(skill)`: an info of the same abnormal type in the queue of the skill.
    fn does_stack(&self, kind: ListKind, abnormal_type: AbnormalType) -> bool {
        if abnormal_type == AbnormalType::None || self.is_empty() {
            return false;
        }
        self.queue(kind)
            .iter()
            .any(|i| i.facts.abnormal_type == abnormal_type)
    }

    /// Java `updateEffectFlags()`.
    // NOTE(java-quirk): the three flags are only ever set, never cleared, so a creature that once
    // had a "removed on damage" buff keeps scanning its list on every hit.
    fn update_effect_flags(&mut self) {
        let (mut on_action, mut on_damage) = (false, false);
        for kind in [ListKind::Buffs, ListKind::Triggered, ListKind::Toggles] {
            for info in self.queue(kind) {
                on_action |= info.facts.removed_on_any_action_except_move;
                on_damage |= info.facts.removed_on_damage;
            }
        }
        self.has_buffs_removed_on_any_action |= on_action;
        self.has_buffs_removed_on_damage |= on_damage;
        // Dances are not looked at (Java skips their queue here).
        if self.debuffs.iter().any(|i| i.facts.removed_on_damage) {
            self.has_debuffs_removed_on_damage = true;
        }
    }

    /// Java `computeEffectFlags()`: every info outside the passives, in use or not.
    fn compute_effect_flags(&mut self) {
        let mut flags = 0;
        for kind in [
            ListKind::Buffs,
            ListKind::Triggered,
            ListKind::Debuffs,
            ListKind::Dances,
            ListKind::Toggles,
        ] {
            for info in self.queue(kind) {
                for effect in &info.effects {
                    flags |= effect.handler.effect_flags();
                }
            }
        }
        self.effect_flags = flags;
    }

    /// Every timer the list holds (to cancel them when the list goes).
    pub(crate) fn timers(&self) -> Vec<TimerId> {
        let mut timers: Vec<TimerId> = self.all().flat_map(BuffInfo::timers).collect();
        timers.extend(self.icon_task);
        timers.extend(self.finish.timer);
        timers
    }
}

fn len_i32<T>(v: &[T]) -> i32 {
    i32::try_from(v.len()).unwrap_or(i32::MAX)
}

impl World {
    /// Java `CreatureStat.getMaxBuffCount()`: `_maxBuffCount` plus the Divine Inspiration level.
    pub fn max_buff_count(&self, id: ObjectId) -> i32 {
        let base = self
            .effect_list(id)
            .map_or(self.settings.effects.buffs_max_amount, |l| l.max_buff_count);
        base + self.stats(id).map_or(0, |v| v.skill_level(DIVINE_INSPIRATION))
    }

    /// Java `Creature.isAffected(flag)`.
    pub fn is_affected(&self, id: ObjectId, flag: u32) -> bool {
        self.effect_list(id).is_some_and(|l| l.is_affected(flag))
    }

    /// Java `Creature.isAffectedBySkill(skillId)`.
    pub fn is_affected_by_skill(&self, id: ObjectId, skill_id: i32) -> bool {
        self.effect_list(id)
            .is_some_and(|l| l.is_affected_by_skill(skill_id))
    }

    /// Java `Creature.isAffectedByAbnormalType(type)`.
    pub fn is_affected_by_abnormal_type(&self, id: ObjectId, abnormal_type: AbnormalType) -> bool {
        self.effect_list(id)
            .is_some_and(|l| l.is_affected_by_abnormal_type(abnormal_type))
    }

    /// Java `Creature.isInvulAgainst(skillId, skillLevel)`.
    pub fn is_invul_against(&self, id: ObjectId, skill_id: i32, skill_level: i32) -> bool {
        self.effect_list(id)
            .is_some_and(|l| l.is_invul_against(skill_id, skill_level))
    }

    /// Java `Creature.getAbnormalVisualEffects()` and the special and event masks, for the
    /// `UserInfo` / `CharInfo` / `NpcInfo` builders.
    pub fn abnormal_visual_effects(&self, id: ObjectId) -> (i32, i32, i32) {
        self.effect_list(id)
            .map_or((0, 0, 0), EffectList::abnormal_visual_effects)
    }

    /// Java `EffectList.addBlockedAbnormalTypes(types)` (the `BlockAbnormalSlot` effect).
    pub fn add_blocked_abnormal_types(
        &mut self,
        id: ObjectId,
        types: impl IntoIterator<Item = AbnormalType>,
    ) {
        if let Some(list) = self.effect_list_mut(id) {
            list.add_blocked_abnormal_types(types);
        }
    }

    /// Java `EffectList.removeBlockedAbnormalTypes(types)`: whether the set changed.
    pub fn remove_blocked_abnormal_types(
        &mut self,
        id: ObjectId,
        types: impl IntoIterator<Item = AbnormalType>,
    ) -> bool {
        self.effects
            .lists
            .get_mut(&id)
            .is_some_and(|l| l.remove_blocked_abnormal_types(types))
    }

    /// Java `CreatureStat.setMaxBuffCount(count)` (the `EnlargeAbnormalSlot` effect).
    pub fn set_max_buff_count(&mut self, id: ObjectId, count: i32) {
        if let Some(list) = self.effect_list_mut(id) {
            list.set_max_buff_count(count);
        }
    }

    /// Java `Creature.addInvulAgainst(holder)` (the `ResistSkill` effect).
    pub fn add_invul_against(&mut self, id: ObjectId, skill_id: i32, skill_level: i32) {
        if let Some(list) = self.effect_list_mut(id) {
            list.add_invul_against(skill_id, skill_level);
        }
    }

    /// Java `Creature.removeInvulAgainst(holder)`.
    pub fn remove_invul_against(&mut self, id: ObjectId, skill_id: i32) {
        if let Some(list) = self.effects.lists.get_mut(&id) {
            list.remove_invul_against(skill_id);
        }
    }

    /// Java `Creature.startAbnormalVisualEffect(true, aves)`.
    pub fn start_abnormal_visual_effect(&mut self, id: ObjectId, aves: &[AbnormalVisualEffect]) {
        if let Some(list) = self.effect_list_mut(id) {
            list.start_abnormal_visual_effects(aves);
        }
        self.update_abnormal_effect(id);
    }

    /// Java `Creature.stopAbnormalVisualEffect(true, aves)`.
    pub fn stop_abnormal_visual_effect(&mut self, id: ObjectId, aves: &[AbnormalVisualEffect]) {
        if let Some(list) = self.effects.lists.get_mut(&id) {
            list.stop_abnormal_visual_effects(aves);
        }
        self.update_abnormal_effect(id);
    }

    /// The effect handler registry.
    pub fn effect_registry(&self) -> &super::EffectRegistry {
        self.effects.registry()
    }

    /// Whether a passive skill of this id is applied (Java `EffectList.getPassives()`).
    pub fn has_passive(&self, id: ObjectId, skill_id: i32) -> bool {
        self.effect_list(id)
            .is_some_and(|l| l.passives.iter().any(|i| i.skill.id == skill_id))
    }

    // ------------------------------------------------------------------ add

    /// Java `EffectList.add(info)`: stacking, the slot limits, then the info joins its queue and
    /// its effects start. The info's effects were chosen by `applyEffectScope` already.
    pub(crate) fn add_buff_info(&mut self, info: BuffInfo) {
        let owner = info.effected;
        let facts = info.facts;
        let Some(list) = self.effect_list_mut(owner) else {
            return;
        };
        // Support for blocked buff slots.
        if list.blocked.contains(&facts.abnormal_type) {
            return;
        }
        // Passive effects are treated specially.
        if facts.passive {
            self.add_passive_info(info);
            return;
        }
        // Prevent adding and initializing buffs/effects on dead creatures.
        if self.is_dead(owner) {
            return;
        }
        // "The old effect is removed using Map#remove(key) instead of Map#put(key, value) ...
        // because order matters": the new buff goes to the end of its queue.
        if facts.abnormal_type == AbnormalType::None {
            self.stop_skill_effects(owner, SkillFinishType::Normal, info.skill.id);
        } else {
            let stacked = self
                .effect_list(owner)
                .and_then(|l| l.info_by_abnormal_type(facts.abnormal_type))
                .map(|s| (s.id, s.facts));
            if let Some((_, stacked_facts)) = stacked {
                // Skills are only replaced if the incoming buff has greater or equal abnormal
                // level.
                if facts.abnormal_level < stacked_facts.abnormal_level {
                    return;
                }
                if facts.abnormal_instant {
                    // An herb: the lesser buff stays in the list, not in use (no stats, no
                    // ticks), but its finish task keeps running.
                    if stacked_facts.abnormal_instant {
                        self.stop_skill_effects_by_abnormal(
                            owner,
                            SkillFinishType::Normal,
                            facts.abnormal_type,
                        );
                    }
                    let still = self
                        .effect_list(owner)
                        .and_then(|l| l.stacked.get(&facts.abnormal_type).copied());
                    if let Some(hidden) = still {
                        let hidden_info = self.find_info_mut(owner, hidden).map(|i| {
                            i.in_use = false;
                            i.clone()
                        });
                        if let Some(hidden_info) = hidden_info {
                            self.remove_info_stats(&hidden_info);
                            if let Some(list) = self.effects.lists.get_mut(&owner) {
                                list.hidden_buffs += 1;
                            }
                        }
                    }
                } else {
                    // Remove the buff that stacks with the abnormal type.
                    // NOTE(java-quirk): when the stacked one is an herb, the first call removes
                    // the herb (which puts the buff it hid back in use) and the second removes
                    // that buff too.
                    if stacked_facts.abnormal_instant {
                        self.stop_skill_effects_by_abnormal(
                            owner,
                            SkillFinishType::Normal,
                            facts.abnormal_type,
                        );
                    }
                    self.stop_skill_effects_by_abnormal(owner, SkillFinishType::Normal, facts.abnormal_type);
                }
            }
            if let Some(list) = self.effects.lists.get_mut(&owner) {
                list.stacked.insert(facts.abnormal_type, info.id);
            }
        }
        // Select the queue that holds the effects related to this skill.
        let kind = ListKind::of(&facts);
        // Remove first buff when buff list is full.
        let stacks = self
            .effect_list(owner)
            .is_some_and(|l| l.does_stack(kind, facts.abnormal_type));
        if !facts.debuff && !facts.toggle && !facts.seven_signs && !stacks {
            let mut to_remove = {
                let list = self.effect_list(owner);
                if facts.dance {
                    list.map_or(0, EffectList::dance_count) - self.settings.effects.dances_max_amount
                } else if facts.triggered {
                    list.map_or(0, EffectList::triggered_buff_count)
                        - self.settings.effects.triggered_buffs_max_amount
                } else if !facts.healing_potion() {
                    list.map_or(0, EffectList::buff_count) - self.max_buff_count(owner)
                } else {
                    -1
                }
            };
            let ids = self.effect_list(owner).map(|l| l.ids(kind)).unwrap_or_default();
            for id in ids {
                if to_remove < 0 {
                    break;
                }
                let in_use = self.find_info(owner, id).map(|i| i.in_use);
                match in_use {
                    Some(true) => {}
                    _ => continue,
                }
                self.stop_and_remove(owner, true, SkillFinishType::Removed, id, kind);
                to_remove -= 1;
            }
        }
        // After removing old buff (same ID) or stacked buff (same abnormal type), add the buff
        // to the end of the effect list.
        let id = info.id;
        let Some(list) = self.effects.lists.get_mut(&owner) else {
            return;
        };
        list.queue_mut(kind).push(info);
        self.initialize_effects(owner, id);
        self.update_effect_list(owner, true);
    }

    /// The passive branch of `EffectList.add`.
    fn add_passive_info(&mut self, info: BuffInfo) {
        let owner = info.effected;
        // "Passive effects don't need stack type!"
        if info.facts.abnormal_type != AbnormalType::None {
            let name = self
                .skill_table()
                .and_then(|s| s.get(info.skill.id, info.skill.level).map(|t| t.name.clone()))
                .unwrap_or_default();
            warn!(
                "Passive {name}({}/{}) with abnormal type: {}!",
                info.skill.id,
                info.skill.level,
                info.facts.abnormal_type.name()
            );
        }
        // Check for passive skill conditions.
        if !self.check_skill_condition(info.effector, info.effected, info.skill) {
            return;
        }
        // Removes the old stats from the creature if the skill was present.
        let old: Vec<BuffInfo> = match self.effects.lists.get_mut(&owner) {
            Some(list) => {
                let (old, keep) = std::mem::take(&mut list.passives)
                    .into_iter()
                    .partition(|b| b.skill.id == info.skill.id);
                list.passives = keep;
                old
            }
            None => return,
        };
        for mut b in old {
            b.in_use = false;
            self.remove_info_stats(&b);
        }
        let id = info.id;
        if let Some(list) = self.effects.lists.get_mut(&owner) {
            list.passives.push(info);
        }
        // Initialize effects.
        self.initialize_effects(owner, id);
    }

    // ------------------------------------------------------------------ remove

    /// Java `EffectList.stopAndRemove(broadcast, type, info, queue)`: the info leaves the queue
    /// (once: a second call finds nothing), its effects stop, and an herb that ends puts the buff
    /// it hid back in use. `false` when the info was not in the queue.
    pub(crate) fn stop_and_remove(
        &mut self,
        owner: ObjectId,
        broadcast: bool,
        finish_type: SkillFinishType,
        id: BuffInfoId,
        kind: ListKind,
    ) -> bool {
        // "Already removed means already stopped, stopping again would strip a newer buff of
        // the same skill."
        let Some(mut info) = self.effects.lists.get_mut(&owner).and_then(|l| l.take(kind, id)) else {
            return false;
        };
        // Stop the buff effects.
        self.stop_info_effects(&mut info, finish_type, broadcast);
        if let Some(list) = self.effects.lists.get_mut(&owner) {
            if !info.in_use {
                // A hidden buff that ends.
                list.hidden_buffs -= 1;
            } else if list.stacked.get(&info.facts.abnormal_type) == Some(&info.id) {
                list.stacked.remove(&info.facts.abnormal_type);
            }
        }
        // If it's an herb that ends, check if there are hidden buffs.
        if info.facts.abnormal_instant {
            let hidden = self.effect_list(owner).and_then(|l| {
                l.buffs
                    .iter()
                    .find(|b| b.facts.abnormal_type == info.facts.abnormal_type && !b.in_use)
                    .map(|b| b.id)
            });
            if let Some(hidden) = hidden {
                let back = self.find_info_mut(owner, hidden).map(|b| {
                    b.in_use = true;
                    b.clone()
                });
                if let Some(back) = back {
                    self.add_info_stats(&back);
                    if let Some(list) = self.effects.lists.get_mut(&owner) {
                        list.stacked.insert(back.facts.abnormal_type, back.id);
                        list.hidden_buffs -= 1;
                    }
                }
            }
        }
        if finish_type != SkillFinishType::Removed {
            self.apply_end_scope(&info);
        }
        true
    }

    /// `stopAndRemove(info)` with the info's own queue and `REMOVED`.
    fn stop_and_remove_info(
        &mut self,
        owner: ObjectId,
        broadcast: bool,
        finish_type: SkillFinishType,
        id: BuffInfoId,
    ) {
        let Some(kind) = self.find_info(owner, id).map(|i| ListKind::of(&i.facts)) else {
            return;
        };
        self.stop_and_remove(owner, broadcast, finish_type, id, kind);
    }

    /// Java `EffectList.remove(type, info)`.
    pub fn remove_effect(&mut self, owner: ObjectId, finish_type: SkillFinishType, id: BuffInfoId) {
        self.stop_and_remove_info(owner, true, finish_type, id);
        self.update_effect_list(owner, true);
    }

    /// Java `Creature.stopSkillEffects(type, skillId)` (`EffectList.stopSkillEffects`).
    pub fn stop_skill_effects(&mut self, owner: ObjectId, finish_type: SkillFinishType, skill_id: i32) {
        let info = self
            .effect_list(owner)
            .and_then(|l| l.info_by_skill_id(skill_id))
            .map(|i| i.id);
        if let Some(info) = info {
            self.remove_effect(owner, finish_type, info);
        }
        // `Servitor.stopSkillEffects`: the buff is not kept for the servitor's next summon either.
        if self.is_servitor(owner) {
            self.servitor_forget_saved_skill(owner, skill_id);
        } else if self.is_pet(owner) {
            // `Pet.stopSkillEffects`: `SummonEffectTable.removePetEffects`.
            self.pet_forget_saved_skill(owner, skill_id);
        }
    }

    /// Java `EffectList.stopSkillEffects(removeType, abnormalType)`: whether there was one.
    pub fn stop_skill_effects_by_abnormal(
        &mut self,
        owner: ObjectId,
        finish_type: SkillFinishType,
        abnormal_type: AbnormalType,
    ) -> bool {
        let old = self
            .effects
            .lists
            .get_mut(&owner)
            .and_then(|l| l.stacked.remove(&abnormal_type));
        match old {
            Some(old) => {
                self.remove_effect(owner, finish_type, old);
                true
            }
            None => false,
        }
    }

    /// Stops every info of `kinds` that `keep` does not keep, with `stopAndRemove(broadcast,
    /// info, queue)`.
    fn stop_kinds(
        &mut self,
        owner: ObjectId,
        kinds: &[ListKind],
        broadcast: bool,
        finish_type: SkillFinishType,
        stop: impl Fn(&BuffInfo) -> bool,
    ) {
        for &kind in kinds {
            let ids = self.effect_list(owner).map(|l| l.ids(kind)).unwrap_or_default();
            for id in ids {
                if self.find_info(owner, id).is_some_and(&stop) {
                    self.stop_and_remove(owner, broadcast, finish_type, id, kind);
                }
            }
        }
    }

    fn has_kind(&self, owner: ObjectId, kind: ListKind) -> bool {
        self.effect_list(owner).is_some_and(|l| !l.queue(kind).is_empty())
    }

    /// Java `EffectList.stopAllEffects()` (`Creature.stopAllEffects`; a player also
    /// `updateAndBroadcastStatus()`).
    pub fn stop_all_effects(&mut self, owner: ObjectId) {
        self.stop_all_buffs(owner, false, true);
        self.stop_all_dances(owner, false);
        self.stop_all_toggles(owner, false);
        self.stop_all_debuffs(owner, false);
        if let Some(list) = self.effects.lists.get_mut(&owner) {
            list.stacked.clear();
        }
        // Update effect flags, icons and ave.
        self.update_effect_list(owner, true);
        self.update_abnormal_effect(owner);
        self.update_and_broadcast_status(owner);
    }

    /// The effect part of Java `Creature.doDie` for an NPC: an attackable whose spawn respawns
    /// loses its effects (`stopAllEffects()`), one that does not respawn every effect
    /// (`stopAllEffectsWithoutExclusions(true, true)`), any other NPC those that do not last
    /// through death.
    pub(crate) fn stop_effects_on_death(&mut self, id: ObjectId) {
        if self.stats(id).is_some_and(|v| v.is_attackable()) {
            let respawns = self
                .npcs
                .get(&id)
                .and_then(|n| n.spawn)
                .is_some_and(|spawn| self.is_respawn_enabled(spawn));
            if respawns {
                self.stop_all_effects(id);
            } else {
                self.stop_all_effects_without_exclusions(id, true, true);
            }
        } else {
            self.stop_all_effects_except_those_that_last_through_death(id);
        }
    }

    /// Java `stopAllEffectsExceptThoseThatLastThroughDeath()` (a player also
    /// `updateAndBroadcastStatus()`).
    pub fn stop_all_effects_except_those_that_last_through_death(&mut self, owner: ObjectId) {
        let update = self.stop_queues_unless(owner, |i| i.facts.stay_after_death);
        self.update_effect_list(owner, update);
        self.update_and_broadcast_status(owner);
    }

    /// Java `Player.stopAllEffectsNotStayOnSubclassChange()`.
    pub fn stop_all_effects_not_stay_on_subclass_change(&mut self, owner: ObjectId) {
        let update = self.stop_queues_unless(owner, |i| i.facts.stay_on_subclass_change);
        self.update_effect_list(owner, update);
        self.update_and_broadcast_status(owner);
    }

    /// The loop of the two methods above: buffs, triggered, debuffs, dances, toggles; `update`
    /// is true when any of those queues had something.
    fn stop_queues_unless(&mut self, owner: ObjectId, keep: impl Fn(&BuffInfo) -> bool) -> bool {
        let mut update = false;
        for kind in [
            ListKind::Buffs,
            ListKind::Triggered,
            ListKind::Debuffs,
            ListKind::Dances,
            ListKind::Toggles,
        ] {
            if self.has_kind(owner, kind) {
                self.stop_kinds(owner, &[kind], true, SkillFinishType::Removed, |i| !keep(i));
                update = true;
            }
        }
        update
    }

    /// Java `EffectList.stopAllEffectsWithoutExclusions(update, broadcast)`: passives too.
    pub fn stop_all_effects_without_exclusions(&mut self, owner: ObjectId, update: bool, broadcast: bool) {
        self.stop_kinds(
            owner,
            &[
                ListKind::Buffs,
                ListKind::Triggered,
                ListKind::Dances,
                ListKind::Toggles,
                ListKind::Debuffs,
                ListKind::Passives,
            ],
            broadcast,
            SkillFinishType::Removed,
            |_| true,
        );
        // Update stats, effect flags and icons.
        if update {
            self.update_effect_list(owner, broadcast);
        }
    }

    /// Java `EffectList.stopAllBuffs(update, triggered)`.
    pub fn stop_all_buffs(&mut self, owner: ObjectId, update: bool, triggered: bool) {
        self.stop_kinds(
            owner,
            &[ListKind::Buffs],
            update,
            SkillFinishType::Removed,
            |_| true,
        );
        if triggered {
            self.stop_kinds(
                owner,
                &[ListKind::Triggered],
                update,
                SkillFinishType::Removed,
                |_| true,
            );
        }
        self.update_effect_list(owner, update);
    }

    /// Java `EffectList.stopAllToggles(update)` (`stopAllToggles()` is `update = true`).
    pub fn stop_all_toggles(&mut self, owner: ObjectId, update: bool) {
        if !self.has_kind(owner, ListKind::Toggles) {
            return;
        }
        self.stop_kinds(
            owner,
            &[ListKind::Toggles],
            update,
            SkillFinishType::Removed,
            |_| true,
        );
        self.update_effect_list(owner, update);
    }

    /// Java `EffectList.stopAllDances(update)`.
    pub fn stop_all_dances(&mut self, owner: ObjectId, update: bool) {
        if !self.has_kind(owner, ListKind::Dances) {
            return;
        }
        self.stop_kinds(
            owner,
            &[ListKind::Dances],
            update,
            SkillFinishType::Removed,
            |_| true,
        );
        self.update_effect_list(owner, update);
    }

    /// Java `EffectList.stopAllDebuffs(update)`.
    pub fn stop_all_debuffs(&mut self, owner: ObjectId, update: bool) {
        if !self.has_kind(owner, ListKind::Debuffs) {
            return;
        }
        self.stop_kinds(
            owner,
            &[ListKind::Debuffs],
            update,
            SkillFinishType::Removed,
            |_| true,
        );
        self.update_effect_list(owner, update);
    }

    /// Java `EffectList.stopEffects(type)` (`Creature.stopEffects`).
    // NOTE(java-quirk): Java calls `stopAndRemove(info)` once per matching effect of an info;
    // the second call finds the info gone and does nothing.
    pub fn stop_effects(&mut self, owner: ObjectId, effect_type: EffectType) {
        let mut update = false;
        for kind in ListKind::ACTIVE {
            if !self.has_kind(owner, kind) {
                continue;
            }
            let ids = self.effect_list(owner).map(|l| l.ids(kind)).unwrap_or_default();
            for id in ids {
                let matches = self
                    .find_info(owner, id)
                    .is_some_and(|i| i.effects.iter().any(|e| e.handler.effect_type() == effect_type));
                if matches {
                    self.stop_and_remove_info(owner, true, SkillFinishType::Removed, id);
                }
            }
            update = true;
        }
        self.update_effect_list(owner, update);
    }

    /// Java `EffectList.stopEffectsOnAction()`: the buffs "removed on any action except move".
    pub fn stop_effects_on_action(&mut self, owner: ObjectId) {
        if !self
            .effect_list(owner)
            .is_some_and(|l| l.has_buffs_removed_on_any_action)
        {
            return;
        }
        let update = self.stop_queues_if(
            owner,
            |i| i.facts.removed_on_any_action_except_move,
            &[
                ListKind::Buffs,
                ListKind::Triggered,
                ListKind::Debuffs,
                ListKind::Dances,
                ListKind::Toggles,
            ],
        );
        self.update_effect_list(owner, update);
    }

    /// Java `EffectList.stopEffectsOnDamage(awake)`.
    pub fn stop_effects_on_damage(&mut self, owner: ObjectId, awake: bool) {
        if !awake {
            return;
        }
        let (buffs, debuffs) = self.effect_list(owner).map_or((false, false), |l| {
            (l.has_buffs_removed_on_damage, l.has_debuffs_removed_on_damage)
        });
        let mut update = false;
        if buffs {
            update |= self.stop_queues_if(
                owner,
                |i| i.facts.removed_on_damage,
                &[
                    ListKind::Buffs,
                    ListKind::Triggered,
                    ListKind::Dances,
                    ListKind::Toggles,
                ],
            );
        }
        if debuffs {
            update |= self.stop_queues_if(owner, |i| i.facts.removed_on_damage, &[ListKind::Debuffs]);
        }
        self.update_effect_list(owner, update);
    }

    fn stop_queues_if(
        &mut self,
        owner: ObjectId,
        stop: impl Fn(&BuffInfo) -> bool,
        kinds: &[ListKind],
    ) -> bool {
        let mut update = false;
        for &kind in kinds {
            if self.has_kind(owner, kind) {
                self.stop_kinds(owner, &[kind], true, SkillFinishType::Removed, &stop);
                update = true;
            }
        }
        update
    }

    /// Java `EffectList.forEach(function, dances)`: `f` runs for every info (dances only when
    /// asked); any `true` updates the flags and icons.
    pub fn for_each_effect(
        &mut self,
        owner: ObjectId,
        dances: bool,
        mut f: impl FnMut(&mut World, BuffInfoId) -> bool,
    ) {
        let mut update = false;
        for kind in ListKind::ACTIVE {
            if kind == ListKind::Dances && !dances {
                continue;
            }
            let ids = self.effect_list(owner).map(|l| l.ids(kind)).unwrap_or_default();
            for id in ids {
                update |= f(self, id);
            }
        }
        self.update_effect_list(owner, update);
    }

    // ------------------------------------------------------------------ icons and flags

    /// Java `Creature.updateEffectIcons(partyOnly)` (`EffectList.updateEffectIcons`).
    pub fn update_effect_icons(&mut self, owner: ObjectId, party_only: bool) {
        // Java's list exists from the constructor on; a member who never had an effect still
        // sends its (empty) icons to a party it joins.
        if let Some(list) = self.effect_list_mut(owner)
            && party_only
        {
            list.party_only = true;
        }
        self.update_effect_list(owner, true);
    }

    /// Java `updateEffectList(update)`: the icons (300 ms later), then the effect flags.
    pub(crate) fn update_effect_list(&mut self, owner: ObjectId, update: bool) {
        if !update {
            return;
        }
        self.schedule_effect_icons(owner);
        if let Some(list) = self.effects.lists.get_mut(&owner) {
            list.compute_effect_flags();
        }
    }

    /// The first half of Java's `updateEffectIcons()`: the sticky flags, then (for players) the
    /// icon task, one at a time.
    fn schedule_effect_icons(&mut self, owner: ObjectId) {
        let is_playable = self.players.contains_key(&owner) || self.is_summon(owner);
        let due = self.ticks_from_now(ICON_UPDATE_MS);
        let Some(list) = self.effects.lists.get_mut(&owner) else {
            return;
        };
        list.update_effect_flags();
        if !is_playable || list.icon_task.is_some() {
            return;
        }
        list.icon_task = Some(self.timers.schedule_at(due, TimerKind::EffectIcons(owner)));
    }

    /// The icon task of `updateEffectIcons()`: the `AbnormalStatusUpdate` (unless only the party
    /// needed an update), the short buff of a healing potion.
    pub(crate) fn on_effect_icons_timer(&mut self, owner: ObjectId) {
        if !self.effect_owner_present(owner) {
            return;
        }
        let Some(list) = self.effects.lists.get_mut(&owner) else {
            return;
        };
        list.icon_task = None;
        // A summon's icons go to its owner as `PartySpelled` (`summon/info.rs`).
        if self.is_summon(owner) {
            self.send_summon_party_spelled(owner);
            return;
        }
        let Some(list) = self.effects.lists.get_mut(&owner) else {
            return;
        };
        let send_asu = if list.party_only {
            list.party_only = false;
            false
        } else {
            true
        };
        // The olympiad packet (`ExOlympiadSpelledInfo`) is M8; the party's `PartySpelled` follows
        // the `AbnormalStatusUpdate` below.
        let potions: Vec<BuffInfoId> = list
            .buffs
            .iter()
            .filter(|i| i.facts.healing_potion())
            .map(|i| i.id)
            .collect();
        for potion in potions {
            self.short_buff_status_update(owner, Some(potion));
        }
        if send_asu {
            let icons = self.effect_icons(owner);
            self.send_abnormal_status_update(owner, &icons);
        }
        // `_owner.isInParty()`: the whole party sees the icons in the party window (a summon's
        // `PartySpelled` is `summon/party.rs`).
        if let Some(party) = self.party_of(owner) {
            let spelled = self.party_spelled(owner);
            self.broadcast_party(party, &spelled);
        }
        // `ExOlympiadSpelledInfo` to the observers of a running battle (M8-OLY-01).
        self.olympiad_spelled_info(owner);
    }

    /// What the `AbnormalStatusUpdate` of `owner` lists now: every info in use except healing
    /// potions, buffs first, then triggered, dances, toggles and debuffs.
    pub fn effect_icons(&self, owner: ObjectId) -> Vec<IconEntry> {
        let Some(list) = self.effect_list(owner) else {
            return Vec::new();
        };
        list.effects()
            .filter(|i| i.in_use && !i.facts.healing_potion())
            .map(|i| IconEntry {
                skill_id: i.facts.display_id,
                skill_level: i.facts.display_level,
                time: i.time(self.tick),
            })
            .collect()
    }

    /// Java `EffectList.shortBuffStatusUpdate(info)`: a player's short buff (the potion gauge),
    /// set or reset.
    pub(crate) fn short_buff_status_update(&mut self, owner: ObjectId, info: Option<BuffInfoId>) {
        if !self.players.contains_key(&owner) {
            return;
        }
        let now = self.tick;
        let Some(list) = self.effects.lists.get_mut(&owner) else {
            return;
        };
        list.short_buff = info;
        let entry = info.and_then(|id| list.get(id)).map(|i| IconEntry {
            skill_id: i.skill.id,
            skill_level: i.skill.level,
            time: i.time(now),
        });
        self.send_short_buff_status_update(owner, entry);
    }

    // ------------------------------------------------------------------ BuffFinishTask

    /// Java `Creature.addBuffInfoTime(info)` -> `BuffFinishTask.addBuffInfo`: the info is counted
    /// down once a second from now on; the task starts at once (Java: delay 0, here the next
    /// tick) if it does not run.
    pub(crate) fn add_buff_info_time(&mut self, owner: ObjectId, id: BuffInfoId) {
        let next = self.tick + 1;
        let Some(list) = self.effects.lists.get_mut(&owner) else {
            return;
        };
        let finish = &mut list.finish;
        // `_buffInfos.put(info, new AtomicInteger())`.
        match finish.entries.iter_mut().find(|(i, _)| *i == id) {
            Some(entry) => entry.1 = 0,
            None => finish.entries.push((id, 0)),
        }
        if finish.timer.is_none() {
            finish.next_due = next;
            finish.timer = Some(self.timers.schedule_at(next, TimerKind::BuffFinish(owner)));
        }
    }

    /// Java `Creature.removeBuffInfoTime(info)` -> `BuffFinishTask.removeBuffInfo`: the task
    /// stops when nothing is left to count.
    pub(crate) fn remove_buff_info_time(&mut self, owner: ObjectId, id: BuffInfoId) {
        let Some(list) = self.effects.lists.get_mut(&owner) else {
            return;
        };
        let finish = &mut list.finish;
        finish.entries.retain(|(i, _)| *i != id);
        if finish.entries.is_empty()
            && let Some(timer) = finish.timer.take()
        {
            self.timers.cancel(timer);
        }
    }

    /// One run of `BuffFinishRunnable`: every counted info one second older; those past their
    /// abnormal time end (`remove(NORMAL, info)`).
    // NOTE: Java walks a `ConcurrentHashMap` (no fixed order) and ends the infos on pool
    // threads; here they end in the order they were added, right after the run.
    pub(crate) fn on_buff_finish_timer(&mut self, owner: ObjectId) {
        if !self.effect_owner_present(owner) {
            return;
        }
        let Some(list) = self.effects.lists.get_mut(&owner) else {
            return;
        };
        list.finish.timer = None;
        let mut expired = Vec::new();
        let entries = std::mem::take(&mut list.finish.entries);
        let mut kept = Vec::with_capacity(entries.len());
        for (id, mut count) in entries {
            let Some(abnormal_time) = list.get(id).map(|i| i.abnormal_time) else {
                // Not in the list any more: nothing to finish.
                continue;
            };
            count += 1;
            if count > abnormal_time {
                expired.push(id);
            } else {
                kept.push((id, count));
            }
        }
        list.finish.entries = kept;
        if !list.finish.entries.is_empty() {
            let due = list.finish.next_due + BUFF_FINISH_PERIOD_TICKS;
            list.finish.next_due = due;
            list.finish.timer = Some(self.timers.schedule_at(due, TimerKind::BuffFinish(owner)));
        }
        // "Stop this exact buff once, a newer buff of the same skill must not be affected."
        for id in expired {
            self.remove_effect(owner, SkillFinishType::Normal, id);
        }
    }

    // ------------------------------------------------------------------ Player

    /// Java `Player.updateAndBroadcastStatus()`: 50 ms later the weight and grade penalties are
    /// checked and the player's info goes out (one pending task at a time).
    pub(crate) fn update_and_broadcast_status(&mut self, id: ObjectId) {
        // `Summon.stopAllEffects()` & co. end with `updateAndBroadcastStatus(1)`, at once.
        if self.is_summon(id) {
            self.update_and_broadcast_summon_status(id, 1);
            return;
        }
        if !self.players.contains_key(&id) || self.effects.status_tasks.contains_key(&id) {
            return;
        }
        let due = self.ticks_from_now(UPDATE_AND_BROADCAST_STATUS_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::UpdateAndBroadcastStatus(id));
        self.effects.status_tasks.insert(id, timer);
    }

    pub(crate) fn on_update_and_broadcast_status_timer(&mut self, id: ObjectId) {
        self.effects.status_tasks.remove(&id);
        if !self.players.contains_key(&id) {
            return;
        }
        self.refresh_overloaded(id);
        self.refresh_expertise_penalty(id);
        self.broadcast_user_info(id);
    }

    /// Sends the `AbnormalStatusUpdate` of `icons` to the player.
    // Java builds the packet in the icon task and writes it at once, so every icon added (all in
    // use) is written; the count quirk of the packet never shows here.
    fn send_abnormal_status_update(&self, owner: ObjectId, icons: &[IconEntry]) {
        let mut asu = AbnormalStatusUpdate::default();
        for icon in icons {
            asu.add_skill(
                EffectIcon {
                    display_id: icon.skill_id,
                    display_level: icon.skill_level,
                    time: icon.time,
                    in_use: true,
                },
                false,
            );
        }
        self.send(owner, &asu);
    }

    /// Sends `ShortBuffStatusUpdate` (the entry) or `RESET_SHORT_BUFF` (`None`) to the player.
    fn send_short_buff_status_update(&self, owner: ObjectId, entry: Option<IconEntry>) {
        let packet = entry.map_or(ShortBuffStatusUpdate::RESET_SHORT_BUFF, |e| {
            ShortBuffStatusUpdate {
                skill_id: e.skill_id,
                skill_level: e.skill_level,
                duration: e.time,
            }
        });
        self.send(owner, &packet);
    }

    /// `Skill.applyEffectScope(END, info, true, false)` for an info that ended.
    fn apply_end_scope(&mut self, info: &BuffInfo) {
        self.apply_effect_scope_instant(EffectScope::End, info.effector, info.effected, info.skill);
    }
}
