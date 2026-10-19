//! One applied skill: its effects, time, ticks and stat functions.
// Java: mechanics/skill/BuffInfo.java (all of it), mechanics/effects/{EffectTickTask,
//       EffectTaskInfo}.java, mechanics/effects/AbstractEffect.java (getStatFuncs)
// Java: mechanics/stats/functions/FuncTemplate.java (getFunc: the attach condition)
// Java: entity/actor/Creature.java (start/stopAbnormalVisualEffect, updateAbnormalEffect)
//
// Java's `GameTimeTaskManager` game ticks (100 ms) measure a buff's age (`getTime`); here the
// world tick (50 ms) does: `(now - start) / 20` whole seconds, the same number.

use std::sync::Arc;

use l2_data::SkillTemplate;
use l2_data::skill::EffectScope;
use l2_data::stats::AbnormalType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::registry::{EffectCx, EffectHandler};
use super::{BuffInfoId, SkillFinishType, SkillKey};
use crate::stats::calculators::{Func, FuncOwner};
use crate::stats::conditions::{self, CondEnv};
use crate::stats::{Stat, skills::calc_skill};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// World ticks per second (Java `GameTimeTaskManager.TICKS_PER_SECOND` counts 100 ms ticks).
const TICKS_PER_SECOND: u64 = 20;

/// Java `EffectTickTask`: which effect of which buff info ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectTickTask {
    pub effected: ObjectId,
    pub info: BuffInfoId,
    /// The index in the info's effects.
    pub effect: usize,
}

/// Java `EffectTaskInfo`'s scheduled future: a fixed-rate task (`scheduleAtFixedRate(task,
/// period, period)`), run `runs` times so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TickTask {
    pub timer: TimerId,
    pub start_tick: u64,
    pub period_ms: u64,
    pub runs: u64,
}

impl TickTask {
    /// The tick of run `n` (1 based).
    fn due(&self, n: u64) -> u64 {
        tick_due(self.start_tick, self.period_ms, n)
    }
}

/// A fixed-rate task started at `start_tick` runs for the `n`th time (1 based) `n * period`
/// later, rounded up to a tick (and never twice in one tick).
fn tick_due(start_tick: u64, period_ms: u64, n: u64) -> u64 {
    let tick_ms = crate::world::TICK.as_millis() as u64;
    start_tick + n.saturating_mul(period_ms).div_ceil(tick_ms).max(n)
}

/// One continuous effect of a buff info (Java's `AbstractEffect` in `BuffInfo._effects`, with its
/// tick task from `_tasks`).
#[derive(Debug, Clone)]
pub struct ActiveEffect {
    /// Where the template is: `skill.effects.get(scope)[index]`.
    pub scope: EffectScope,
    pub index: usize,
    /// The shared handler of the template.
    pub handler: Arc<dyn EffectHandler>,
    pub(crate) tick: Option<TickTask>,
}

impl ActiveEffect {
    /// The owner of this effect's stat functions.
    pub(crate) fn func_owner(&self, skill: SkillKey) -> FuncOwner {
        FuncOwner::Effect {
            skill_id: skill.id,
            skill_level: skill.level,
            scope: self.scope,
            index: u16::try_from(self.index).unwrap_or(u16::MAX),
        }
    }
}

/// The skill fields the effect list reads, copied from the template when the info is made
/// (Java reads them from the `Skill` object, which never changes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SkillFacts {
    pub abnormal_type: AbnormalType,
    pub abnormal_level: i32,
    pub abnormal_instant: bool,
    pub passive: bool,
    pub toggle: bool,
    pub debuff: bool,
    pub dance: bool,
    pub triggered: bool,
    pub seven_signs: bool,
    pub stay_after_death: bool,
    pub stay_on_subclass_change: bool,
    pub removed_on_any_action_except_move: bool,
    pub removed_on_damage: bool,
    pub display_id: i32,
    pub display_level: i32,
}

impl SkillFacts {
    pub(crate) fn of(skill: &SkillTemplate) -> Self {
        Self {
            abnormal_type: skill.abnormal_type,
            abnormal_level: skill.abnormal_level,
            abnormal_instant: skill.abnormal_instant,
            passive: skill.is_passive(),
            toggle: skill.is_toggle(),
            debuff: skill.is_debuff,
            dance: skill.is_dance(),
            triggered: skill.is_triggered_skill,
            seven_signs: skill.is_seven_signs,
            stay_after_death: skill.stay_after_death,
            stay_on_subclass_change: skill.stay_on_subclass_change,
            removed_on_any_action_except_move: skill.removed_on_any_action_except_move,
            removed_on_damage: skill.removed_on_damage,
            display_id: skill.display_id,
            display_level: skill.display_level,
        }
    }

    /// Java `Skill.isHealingPotionSkill()`.
    pub(crate) fn healing_potion(&self) -> bool {
        self.abnormal_type == AbnormalType::HpRecover
    }
}

/// Java `BuffInfo`: one skill applied to one creature.
#[derive(Debug, Clone)]
pub struct BuffInfo {
    pub(crate) id: BuffInfoId,
    pub(crate) effector: ObjectId,
    pub(crate) effected: ObjectId,
    pub(crate) skill: SkillKey,
    pub(crate) facts: SkillFacts,
    pub(crate) effects: Vec<ActiveEffect>,
    pub(crate) abnormal_time: i32,
    pub(crate) period_start_tick: u64,
    pub(crate) finish_type: SkillFinishType,
    pub(crate) in_use: bool,
}

impl BuffInfo {
    /// `new BuffInfo(effector, effected, skill)` with the abnormal time already calculated
    /// (`Formulas.calcEffectAbnormalTime`, see `apply.rs`).
    pub(crate) fn new(
        id: BuffInfoId,
        effector: ObjectId,
        effected: ObjectId,
        skill: &SkillTemplate,
        abnormal_time: i32,
        now: u64,
    ) -> Self {
        Self {
            id,
            effector,
            effected,
            skill: SkillKey::of(skill),
            facts: SkillFacts::of(skill),
            effects: Vec::new(),
            abnormal_time,
            period_start_tick: now,
            finish_type: SkillFinishType::Normal,
            in_use: true,
        }
    }

    pub fn id(&self) -> BuffInfoId {
        self.id
    }

    /// Java `getEffector()`.
    pub fn effector(&self) -> ObjectId {
        self.effector
    }

    /// Java `getEffected()`.
    pub fn effected(&self) -> ObjectId {
        self.effected
    }

    /// Java `getSkill()`, as id and level.
    pub fn skill(&self) -> SkillKey {
        self.skill
    }

    /// The abnormal type of the skill.
    pub fn abnormal_type(&self) -> AbnormalType {
        self.facts.abnormal_type
    }

    /// Java `isAbnormalType(type)`.
    pub fn is_abnormal_type(&self, abnormal_type: AbnormalType) -> bool {
        self.facts.abnormal_type == abnormal_type
    }

    /// Java `getEffects()`.
    pub fn effects(&self) -> &[ActiveEffect] {
        &self.effects
    }

    /// Java `getAbnormalTime()`: seconds (`-1` for passives and toggles).
    pub fn abnormal_time(&self) -> i32 {
        self.abnormal_time
    }

    /// Java `setAbnormalTime(abnormalTime)`.
    pub fn set_abnormal_time(&mut self, abnormal_time: i32) {
        self.abnormal_time = abnormal_time;
    }

    /// Java `getPeriodStartTicks()`, in world ticks.
    pub fn period_start_tick(&self) -> u64 {
        self.period_start_tick
    }

    /// Java `getTime()`: the seconds left at world tick `now`.
    // NOTE(java-quirk): a passive or toggle (abnormal time -1) counts down below -1; the client
    // shows no time for a negative value.
    pub fn time(&self, now: u64) -> i32 {
        let elapsed = now.saturating_sub(self.period_start_tick) / TICKS_PER_SECOND;
        self.abnormal_time
            .saturating_sub(i32::try_from(elapsed).unwrap_or(i32::MAX))
    }

    /// Java `isRemoved()`.
    pub fn is_removed(&self) -> bool {
        self.finish_type == SkillFinishType::Removed
    }

    /// Java `isInUse()`: `false` while a stronger herb of the same abnormal type hides it.
    pub fn is_in_use(&self) -> bool {
        self.in_use
    }

    /// Java `addEffect(effect)`.
    pub(crate) fn add_effect(&mut self, scope: EffectScope, index: usize, handler: Arc<dyn EffectHandler>) {
        self.effects.push(ActiveEffect {
            scope,
            index,
            handler,
            tick: None,
        });
    }

    /// Every timer the info holds.
    pub(crate) fn timers(&self) -> impl Iterator<Item = TimerId> + '_ {
        self.effects.iter().filter_map(|e| e.tick.map(|t| t.timer))
    }
}

/// Java `SystemMessage.addSkillName(skill)`: the name as text when the display id differs.
pub(crate) fn skill_name_param(skill: &SkillTemplate) -> SystemMessageParam {
    if skill.id != skill.display_id {
        SystemMessageParam::Text(skill.name.clone())
    } else {
        SystemMessageParam::SkillName {
            id: skill.id,
            level: skill.level,
        }
    }
}

impl World {
    /// Sends a system message with the skill's name to a player (Java `sendPacket` reaches only
    /// players, through their connection).
    pub(crate) fn send_skill_message(&self, to: ObjectId, id: SystemMessageId, skill: &SkillTemplate) {
        // A player that is leaving is out of `players` already but still has its connection.
        self.send(to, &SystemMessage::new(id).with(skill_name_param(skill)));
    }

    /// Java `AbstractEffect.getStatFuncs(caster, target, skill)`: the functions of one effect
    /// template, those whose attach condition holds for `(effector, effected)`.
    pub(crate) fn effect_stat_funcs(
        &self,
        effector: ObjectId,
        effected: ObjectId,
        skill: &SkillTemplate,
        effect: &ActiveEffect,
    ) -> Vec<Func> {
        let Some(template) = skill.effects.get(effect.scope).get(effect.index) else {
            return Vec::new();
        };
        let owner = effect.func_owner(SkillKey::of(skill));
        let ctx = calc_skill(skill);
        // The caster's view tests the attach conditions. A caster that left the world fails every
        // conditioned function (Java would still test the stale object).
        let caster = self.stats(effector);
        template
            .funcs
            .iter()
            .filter(|t| match t.attach_cond.as_deref() {
                None => true,
                Some(cond) => caster.as_ref().is_some_and(|view| {
                    conditions::test(
                        cond,
                        &CondEnv {
                            actor: view,
                            target: Some(effected),
                            skill: Some(&ctx),
                        },
                    )
                }),
            })
            .map(|t| Func::from_template(t, owner))
            .collect()
    }

    /// `Creature.addStatFuncs(functions)`: the functions join the calculators; the stats that
    /// changed are broadcast unless `quiet`.
    pub(crate) fn add_stat_funcs(&mut self, id: ObjectId, funcs: Vec<Func>, quiet: bool) {
        if funcs.is_empty() {
            return;
        }
        let Some(core) = self.creature_core_mut(id) else {
            return;
        };
        let stats = core.calculators.add_all(funcs);
        if !quiet {
            self.effect_broadcast_modified_stats(id, &stats);
        }
    }

    /// `Creature.removeStatsOwner(owner)`, broadcasting the stats that changed unless `quiet`.
    pub(crate) fn remove_stats_owner(&mut self, id: ObjectId, owner: FuncOwner, quiet: bool) {
        let Some(core) = self.creature_core_mut(id) else {
            return;
        };
        let stats = core.calculators.remove_owner(owner);
        if !quiet {
            self.effect_broadcast_modified_stats(id, &stats);
        }
    }

    /// `Creature.broadcastModifiedStats(stats)` for a player or an NPC.
    pub(crate) fn effect_broadcast_modified_stats(&mut self, id: ObjectId, stats: &[Stat]) {
        if stats.is_empty() {
            return;
        }
        if self.npcs.contains_key(&id) {
            // An NPC whose speed changed is shown again (`NpcInfo`), anything else is a
            // `StatusUpdate` of the attack and cast speeds.
            if stats.contains(&Stat::MoveSpeed) {
                if self.objects.get(&id).is_some_and(|e| e.spawned) {
                    self.broadcast_info(id);
                }
                return;
            }
        }
        self.broadcast_modified_stats(id, stats);
    }

    pub(crate) fn creature_core_mut(&mut self, id: ObjectId) -> Option<&mut crate::player::CreatureCore> {
        if let Some(p) = self.players.get_mut(&id) {
            return Some(&mut p.core);
        }
        self.npcs.get_mut(&id).map(|n| &mut n.core)
    }

    /// Java `BuffInfo.addStats()`.
    pub(crate) fn add_info_stats(&mut self, info: &BuffInfo) {
        let Some(skills) = self.skill_table() else {
            return;
        };
        let Some(skill) = skills.get(info.skill.id, info.skill.level) else {
            return;
        };
        for effect in &info.effects {
            let funcs = self.effect_stat_funcs(info.effector, info.effected, skill, effect);
            self.add_stat_funcs(info.effected, funcs, info.facts.passive);
        }
    }

    /// Java `BuffInfo.removeStats()`: the functions of every effect, then those of the skill.
    pub(crate) fn remove_info_stats(&mut self, info: &BuffInfo) {
        let quiet = info.facts.passive;
        for effect in &info.effects {
            self.remove_stats_owner(info.effected, effect.func_owner(info.skill), quiet);
        }
        self.remove_stats_owner(
            info.effected,
            FuncOwner::Skill {
                id: info.skill.id,
                level: info.skill.level,
            },
            quiet,
        );
    }

    /// Java `BuffInfo.initializeEffects()`: the info joined its effect list; its effects start.
    // NOTE: a passive skill's functions and visual effects are not broadcast here (M4-STAT-01
    // applies passives without the `broadcastModifiedStats` / `updateAbnormalEffect` Java sends;
    // the callers send the final `UserInfo`). Java sends nothing either while a player or NPC is
    // not in view yet, which is when most passives start.
    pub(crate) fn initialize_effects(&mut self, owner: ObjectId, info_id: BuffInfoId) {
        let Some(info) = self.find_info(owner, info_id).cloned() else {
            return;
        };
        let Some(skills) = self.skill_table() else {
            return;
        };
        let Some(skill) = skills.get(info.skill.id, info.skill.level) else {
            return;
        };
        let passive = info.facts.passive;
        // "When effects are initialized, they have successfully landed."
        if let Some(player) = self.players.get(&owner)
            && (player.entered_world || self.settings.effects.show_effect_messages_on_login)
            && !passive
        {
            self.send_skill_message(owner, SystemMessageId::S1SEffectCanBeFelt, skill);
        }
        // Creates a task that will stop all the effects.
        if info.abnormal_time > 0 {
            self.add_buff_info_time(owner, info_id);
        }
        let mut update = false;
        for (at, effect) in info.effects.iter().enumerate() {
            if effect.handler.is_instant() || (self.is_dead(owner) && !passive) {
                continue;
            }
            let Some(template) = skill.effects.get(effect.scope).get(effect.index) else {
                continue;
            };
            let cx = EffectCx {
                effector: info.effector,
                effected: owner,
                skill,
                effect: template,
                info: Some(info_id),
            };
            effect.handler.on_start(self, &cx);
            // "Do not add continuous effect if target just died from the initial effect,
            // otherwise they'll be ticked forever."
            if self.is_dead(owner) && !passive {
                continue;
            }
            if template.ticks > 0 {
                self.start_effect_tick(owner, info_id, at, template.ticks);
            }
            // NOTE(java-quirk): Java adds the functions even when `onStart` removed the info
            // again; they then stay until the skill's functions are removed by owner.
            let funcs = self.effect_stat_funcs(info.effector, owner, skill, effect);
            self.add_stat_funcs(owner, funcs, passive);
            update = true;
        }
        if update {
            // Summons: `updateAndBroadcastStatus(1)` (M6).
            self.add_abnormal_visual_effects(owner, skill, !passive);
        }
    }

    /// Schedules the fixed-rate task of a ticking effect (`ThreadPool.scheduleAtFixedRate(task,
    /// ticks * EFFECT_TICK_RATIO, ticks * EFFECT_TICK_RATIO)`).
    fn start_effect_tick(&mut self, owner: ObjectId, info_id: BuffInfoId, at: usize, ticks: i32) {
        // A removed info has nobody to tick for (Java would tick the orphaned info forever).
        if self.find_info(owner, info_id).is_none() {
            return;
        }
        let period_ms =
            u64::try_from(i64::from(ticks).saturating_mul(self.settings.effects.effect_tick_ratio))
                .unwrap_or(0)
                .max(1);
        let start_tick = self.tick;
        let due = tick_due(start_tick, period_ms, 1);
        let timer = self.timers.schedule_at(
            due,
            TimerKind::EffectTick(EffectTickTask {
                effected: owner,
                info: info_id,
                effect: at,
            }),
        );
        let task = TickTask {
            timer,
            start_tick,
            period_ms,
            runs: 0,
        };
        match self
            .find_info_mut(owner, info_id)
            .and_then(|i| i.effects.get_mut(at))
        {
            Some(effect) => {
                if let Some(old) = effect.tick.replace(task) {
                    self.timers.cancel(old.timer);
                }
            }
            None => {
                self.timers.cancel(task.timer);
            }
        }
    }

    /// Java `EffectTickTask.run` -> `BuffInfo.onTick(effect)`.
    pub(crate) fn on_effect_tick(&mut self, task: EffectTickTask) {
        let EffectTickTask {
            effected: owner,
            info: info_id,
            effect: at,
        } = task;
        if !self.effect_owner_present(owner) {
            return;
        }
        let Some(info) = self.find_info(owner, info_id) else {
            return;
        };
        let Some(effect) = info.effects.get(at).cloned() else {
            return;
        };
        let (in_use, effector, skill_key, toggle) =
            (info.in_use, info.effector, info.skill, info.facts.toggle);
        let Some(skills) = self.skill_table() else {
            return;
        };
        let Some(skill) = skills.get(skill_key.id, skill_key.level) else {
            return;
        };
        let mut continue_forever = false;
        // If the effect is in use, allow it to affect the effected.
        if in_use && let Some(template) = skill.effects.get(effect.scope).get(effect.index) {
            let cx = EffectCx {
                effector,
                effected: owner,
                skill,
                effect: template,
                info: Some(info_id),
            };
            continue_forever = effect.handler.on_action_time(self, &cx);
        }
        if !continue_forever && toggle {
            // "Don't allow to finish current run": the task ends, then the toggle goes.
            if let Some(e) = self
                .find_info_mut(owner, info_id)
                .and_then(|i| i.effects.get_mut(at))
                && let Some(tick) = e.tick.take()
            {
                self.timers.cancel(tick.timer);
                self.stop_skill_effects(owner, SkillFinishType::Removed, skill_key.id);
            }
            return;
        }
        // The next run of the fixed-rate task, unless the info ended meanwhile.
        let Some(e) = self
            .find_info_mut(owner, info_id)
            .and_then(|i| i.effects.get_mut(at))
        else {
            return;
        };
        let Some(mut tick) = e.tick else {
            return;
        };
        tick.runs += 1;
        let due = tick.due(tick.runs + 1);
        tick.timer = self.timers.schedule_at(due, TimerKind::EffectTick(task));
        if let Some(e) = self
            .find_info_mut(owner, info_id)
            .and_then(|i| i.effects.get_mut(at))
        {
            e.tick = Some(tick);
        }
    }

    /// Java `BuffInfo.stopAllEffects(type, broadcast)` for an info that already left its list.
    pub(crate) fn stop_info_effects(
        &mut self,
        info: &mut BuffInfo,
        finish_type: SkillFinishType,
        broadcast: bool,
    ) {
        info.finish_type = finish_type;
        // Remove this buff info from BuffFinishTask.
        self.remove_buff_info_time(info.effected, info.id);
        self.finish_effects(info, broadcast);
    }

    /// Java `BuffInfo.finishEffects(broadcast)`.
    pub(crate) fn finish_effects(&mut self, info: &mut BuffInfo, broadcast: bool) {
        // Cancels the ticking tasks.
        for effect in &mut info.effects {
            if let Some(tick) = effect.tick.take() {
                self.timers.cancel(tick.timer);
            }
        }
        // Remove stats.
        self.remove_info_stats(info);
        let skills = self.skill_table();
        let skill = skills
            .as_ref()
            .and_then(|s| s.get(info.skill.id, info.skill.level));
        // Notify on exit (instant effects never join an info, so all of these are continuous).
        if let Some(skill) = skill {
            for effect in &info.effects {
                if effect.handler.is_instant() {
                    continue;
                }
                let Some(template) = skill.effects.get(effect.scope).get(effect.index) else {
                    continue;
                };
                let cx = EffectCx {
                    effector: info.effector,
                    effected: info.effected,
                    skill,
                    effect: template,
                    info: Some(info.id),
                };
                effect.handler.on_exit(self, &cx);
            }
            // Remove abnormal visual effects (a passive's quietly, see `initialize_effects`).
            self.remove_abnormal_visual_effects(info.effected, skill, broadcast && !info.facts.passive);
            // Set the proper system message (the summon check is M6).
            let message = match info.finish_type {
                SkillFinishType::Silent => None,
                _ if info.facts.toggle => Some(SystemMessageId::S1HasBeenAborted),
                SkillFinishType::Removed => Some(SystemMessageId::TheEffectOfS1HasBeenRemoved),
                SkillFinishType::Normal if !info.facts.passive => Some(SystemMessageId::S1HasWornOff),
                SkillFinishType::Normal => None,
            };
            if let Some(message) = message {
                self.send_skill_message(info.effected, message, skill);
            }
        }
        // Remove short buff.
        if self
            .effect_list(info.effected)
            .is_some_and(|l| l.short_buff == Some(info.id))
        {
            self.short_buff_status_update(info.effected, None);
        }
    }

    /// The skill's abnormal visual effects go on (`BuffInfo.addAbnormalVisualEffects`), then
    /// `updateAbnormalEffect()` when `update`.
    fn add_abnormal_visual_effects(&mut self, owner: ObjectId, skill: &SkillTemplate, update: bool) {
        let is_player = self.players.contains_key(&owner);
        if let Some(list) = self.effect_list_mut(owner) {
            list.start_abnormal_visual_effects(&skill.abnormal_visual_effects);
            if is_player {
                list.start_abnormal_visual_effects(&skill.abnormal_visual_effects_event);
            }
            list.start_abnormal_visual_effects(&skill.abnormal_visual_effects_special);
        }
        if update {
            self.update_abnormal_effect(owner);
        }
    }

    /// `BuffInfo.removeAbnormalVisualEffects(broadcast)`.
    fn remove_abnormal_visual_effects(&mut self, owner: ObjectId, skill: &SkillTemplate, broadcast: bool) {
        let is_player = self.players.contains_key(&owner);
        if let Some(list) = self.effects.lists.get_mut(&owner) {
            list.stop_abnormal_visual_effects(&skill.abnormal_visual_effects);
            if is_player {
                list.stop_abnormal_visual_effects(&skill.abnormal_visual_effects_event);
            }
            list.stop_abnormal_visual_effects(&skill.abnormal_visual_effects_special);
        }
        if broadcast {
            self.update_abnormal_effect(owner);
        }
    }

    /// Java `Creature.updateAbnormalEffect()`: a player's `broadcastUserInfo`, an NPC's `NpcInfo`
    /// to everybody who sees it.
    pub(crate) fn update_abnormal_effect(&mut self, id: ObjectId) {
        if self.players.contains_key(&id) {
            if self.sinks.contains_key(&id) {
                self.broadcast_user_info(id);
            }
        } else if self.npcs.contains_key(&id) && self.objects.get(&id).is_some_and(|e| e.spawned) {
            self.broadcast_info(id);
        }
    }

    /// The buff info `info_id` of `owner`, in any queue.
    pub fn find_info(&self, owner: ObjectId, info_id: BuffInfoId) -> Option<&BuffInfo> {
        self.effects.lists.get(&owner)?.get(info_id)
    }

    pub(crate) fn find_info_mut(&mut self, owner: ObjectId, info_id: BuffInfoId) -> Option<&mut BuffInfo> {
        self.effects.lists.get_mut(&owner)?.get_mut(info_id)
    }

    /// Ticks for `ms` milliseconds from now (the timer wheel's rounding).
    pub(crate) fn ticks_from_now(&self, ms: u64) -> u64 {
        self.tick + TimerWheel::ms_to_ticks(ms)
    }
}
