//! Cubics: the small floating helpers a player summons with a skill. They pick a target on a
//! timer and cast attack, drain, debuff or heal skills on it.
// Java: entity/actor/instance/Cubic.java
// Java: entity/actor/tasks/cubics/{CubicAction,CubicHeal,CubicDisappear}.java
// Java: entity/actor/Player.java (addCubic, removeCubic, removeFirstCubic, stopCubics,
//       stopCubicsByOthers, getCubics, getCubicById)
// Java: entity/actor/stat/PlayerStat.java (get/setMaxCubicCount)
// Java: entity/cubic/{CubicSkill,ICubicConditionHolder}.java and cubic/conditions/*.java
// Java: taskmanagers/AttackStanceTaskManager.java (addAttackStanceTask: `cubic.doAction()`)
//
// # Where a cubic lives (D-11)
// A cubic is not an object of the world: it has no object id and nobody can target it. It is a
// [`Cubic`] in `CubicWorld::players`, under its owner's object id and its own cubic id (Java's
// `Player._cubics`, a `ConcurrentSkipListMap`, so the ids are in ascending order; `_cubicOrder`
// is the order the cubics were summoned in). Its two Java tasks are timers of the wheel
// ([`CubicTimer`], inside `SummonTimer`):
// - the disappear task (`CubicDisappear`), once, `duration` seconds after the summon;
// - the action task (`CubicAction`, or `CubicHeal` for a `HEAL` cubic), `scheduleAtFixedRate(task,
//   0, delay)`: the first run on the next tick, then every `delay`. It starts at the first attack
//   stance of the owner (`doAction`), a heal cubic starts at once.
// A timer carries the cubic's `serial`, so a timer that was already taken out of the wheel when
// its cubic was removed (or replaced by a new one of the same id) finds nothing to do.
//
// # Left out, with the system that brings it
// - Custom events (`isOnEvent()`), mounts and the observer mode (`SummonCubic` ignores a mounted
//   or observing player) are not modelled.
// - The access level's `canGiveDamage` of a GM (not loaded, see `effects/apply.rs`).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

use l2_data::skill::EffectScope;
use l2_data::{
    CubicCondition, CubicData, CubicSelectionMode, CubicSkill, CubicTargetType, CubicTemplate,
    HpConditionType, SkillTemplate, Stat,
};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{MagicSkillUse, SkillPosition, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use crate::effects::SkillKey;
use crate::effects::apply::has_negative_effect;
use crate::effects::list::ListKind;
use crate::effects::registry::EffectType;
use crate::formulas::{Rnd, SHIELD_DEFENSE_FAILED};
use crate::stats::skills::calc_skill;
use crate::target::distance_2d;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

use super::SummonTimer;

/// `Cubic.LIFE_CUBIC`: the cubic that does not start from an attack stance.
pub const LIFE_CUBIC: i32 = 3;
/// `Cubic.MAX_MAGIC_RANGE`.
pub const MAX_MAGIC_RANGE: i64 = 900;
/// `Cubic.SKILL_CUBIC_HEAL`: the skill the heal cubics cast.
pub const SKILL_CUBIC_HEAL: i32 = 4051;

/// The payload of `SummonTimer::Cubic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicTimer {
    /// `CubicAction.run()` / `CubicHeal.run()`.
    Action {
        owner: ObjectId,
        cubic: i32,
        serial: u64,
    },
    /// `CubicDisappear.run()`.
    Disappear {
        owner: ObjectId,
        cubic: i32,
        serial: u64,
    },
}

/// The running action task of a cubic: its timer and the task's `_currentCount`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActionTask {
    pub timer: TimerId,
    /// How many skills this task used (`CubicAction._currentCount`); a new task starts at 0.
    pub count: i32,
}

/// Java `Cubic`.
#[derive(Debug, Clone, PartialEq)]
pub struct Cubic {
    /// `_cubicId`.
    pub id: i32,
    /// `_owner`.
    pub owner: ObjectId,
    /// `_template`.
    pub template: Arc<CubicTemplate>,
    /// `_cubicPower`.
    pub power: i32,
    /// `_cubicDelay`, ms.
    pub delay_ms: i32,
    /// `_cubicSkillChance`.
    pub skill_chance: i32,
    /// `_cubicMaxCount`.
    pub max_count: i32,
    /// `_givenByOther`.
    pub given_by_other: bool,
    /// `_target`.
    pub target: Option<ObjectId>,
    pub(crate) serial: u64,
    /// `_disappearTask`.
    pub(crate) disappear_timer: Option<TimerId>,
    /// `_actionTask`.
    pub(crate) action: Option<ActionTask>,
}

impl Cubic {
    /// Java `isActionRunning`-like: whether the action task exists (`_actionTask != null`).
    pub fn is_acting(&self) -> bool {
        self.action.is_some()
    }

    /// How many skills the running action task used.
    pub fn action_count(&self) -> i32 {
        self.action.map_or(0, |a| a.count)
    }
}

/// The arguments of Java `Player.addCubic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddCubic {
    pub cubic_id: i32,
    pub level: i32,
    pub power: i32,
    pub delay: i32,
    pub skill_chance: i32,
    pub max_count: i32,
    pub duration: i32,
    pub given_by_other: bool,
}

/// One player's cubics.
#[derive(Debug, Default)]
pub(crate) struct PlayerCubics {
    /// `_cubics`, ascending by cubic id.
    by_id: BTreeMap<i32, Cubic>,
    /// `_cubicOrder`.
    order: VecDeque<i32>,
}

/// Every cubic of the world and `CubicData.xml`.
#[derive(Debug, Default)]
pub(crate) struct CubicWorld {
    data: Option<Arc<CubicData>>,
    players: HashMap<ObjectId, PlayerCubics>,
    /// `PlayerStat._maxCubicCount` (1 unless `CubicMastery` says otherwise).
    max_counts: HashMap<ObjectId, i32>,
    next_serial: u64,
}

// ------------------------------------------------------------------ the pure parts

/// Java `Cubic.getCubicSkill()`: the index of the next normal (not priority) skill by the
/// template's selection mode, `None` when there is none.
pub(crate) fn select_cubic_skill(template: &CubicTemplate, rng: &mut impl Rnd) -> Option<usize> {
    let normal = template.skills.iter().filter(|s| !s.priority).count();
    if normal == 0 {
        return None;
    }
    match template.selection_mode {
        CubicSelectionMode::Ordered => ordered_skill(&template.skills, rng),
        CubicSelectionMode::Weighted => weighted_skill(&template.skills, normal, rng),
        CubicSelectionMode::Random => random_skill(&template.skills, normal, rng),
    }
}

/// `getRandomCubicSkill`.
fn random_skill(skills: &[CubicSkill], normal: usize, rng: &mut impl Rnd) -> Option<usize> {
    let mut index = rng.get_int(i32::try_from(normal).unwrap_or(i32::MAX));
    for (i, skill) in skills.iter().enumerate() {
        if skill.priority {
            continue;
        }
        let here = index;
        index -= 1;
        if here == 0 {
            return Some(i);
        }
    }
    None
}

/// `getOrderedCubicSkill`: the first normal skill whose trigger rate passes.
fn ordered_skill(skills: &[CubicSkill], rng: &mut impl Rnd) -> Option<usize> {
    for (i, skill) in skills.iter().enumerate() {
        if skill.priority {
            continue;
        }
        if skill.trigger_rate >= 100 || rng.get_int(100) < skill.trigger_rate {
            return Some(i);
        }
    }
    None
}

/// `getWeightedCubicSkill`: the trigger rates are weights.
fn weighted_skill(skills: &[CubicSkill], normal: usize, rng: &mut impl Rnd) -> Option<usize> {
    let mut total_rate = 0_i32;
    for skill in skills {
        if !skill.priority && skill.trigger_rate > 0 {
            total_rate = total_rate.wrapping_add(skill.trigger_rate);
        }
    }
    if total_rate <= 0 {
        return random_skill(skills, normal, rng);
    }
    let mut chance = rng.get_int(total_rate);
    for (i, skill) in skills.iter().enumerate() {
        if skill.priority || skill.trigger_rate <= 0 {
            continue;
        }
        chance -= skill.trigger_rate;
        if chance < 0 {
            return Some(i);
        }
    }
    random_skill(skills, normal, rng)
}

/// What the `CubicCondition`s of a target read.
pub(crate) struct ConditionFacts {
    /// `getCurrentHpPercent()` of the target (a creature or a door); `None` for anything else.
    pub hp_percent: Option<i32>,
    /// `owner.calculateDistance2D(target)`.
    pub distance_2d: f64,
}

/// Java `HpCondition.test`, `RangeCondition.test` and `HealthCondition.test`.
pub(crate) fn test_condition(condition: &CubicCondition, facts: &ConditionFacts) -> bool {
    match *condition {
        CubicCondition::Hp { kind, percent } => {
            let Some(hp) = facts.hp_percent else {
                return false;
            };
            match kind {
                HpConditionType::Greater => hp > percent,
                HpConditionType::Lesser => hp < percent,
            }
        }
        CubicCondition::Range(range) => facts.distance_2d <= f64::from(range),
        CubicCondition::HealthPercent { min, max } => {
            let Some(hp) = facts.hp_percent else {
                return false;
            };
            hp > min && hp < max
        }
    }
}

// ------------------------------------------------------------------ the owner's table

impl World {
    /// Installs `data/stats/cubics`. Call before players enter.
    pub fn set_cubic_data(&mut self, data: Arc<CubicData>) {
        self.summons.cubics.data = Some(data);
    }

    /// Java `CubicData.getCubicTemplate(id, level)`.
    pub fn cubic_template(&self, id: i32, level: i32) -> Option<Arc<CubicTemplate>> {
        self.summons.cubics.data.as_ref()?.get(id, level).cloned()
    }

    /// Java `Player.getCubics()`: the owner's cubics, ascending by cubic id.
    pub fn cubics_of(&self, owner: ObjectId) -> Vec<&Cubic> {
        self.summons
            .cubics
            .players
            .get(&owner)
            .map(|p| p.by_id.values().collect())
            .unwrap_or_default()
    }

    /// Java `Player.getCubicById(cubicId)`.
    pub fn cubic(&self, owner: ObjectId, cubic_id: i32) -> Option<&Cubic> {
        self.summons.cubics.players.get(&owner)?.by_id.get(&cubic_id)
    }

    /// `getCubics().size()`.
    pub fn cubic_count(&self, owner: ObjectId) -> usize {
        self.summons
            .cubics
            .players
            .get(&owner)
            .map_or(0, |p| p.by_id.len())
    }

    /// The cubic ids `UserInfo` and `CharInfo` write (`getCubics().keySet()`, as shorts).
    pub fn cubic_ids_of(&self, owner: ObjectId) -> Vec<i16> {
        self.cubics_of(owner).iter().map(|c| c.id as i16).collect()
    }

    /// Java `PlayerStat.getMaxCubicCount()`: 1 unless `CubicMastery` raised it.
    pub fn max_cubic_count(&self, owner: ObjectId) -> i32 {
        self.summons.cubics.max_counts.get(&owner).copied().unwrap_or(1)
    }

    /// Java `PlayerStat.setMaxCubicCount(count)`.
    pub fn set_max_cubic_count(&mut self, owner: ObjectId, count: i32) {
        self.summons.cubics.max_counts.insert(owner, count);
    }

    /// Java `Player.addCubic(..)`: makes the cubic (`new Cubic(..)`), puts it in the table and at
    /// the end of the summon order. Returns the cubic it replaced, if any.
    // The replaced cubic's tasks are cancelled here; Java leaves them running (`SummonCubic`
    // always removes a cubic of the same id first, so the case does not happen there).
    pub fn add_cubic(&mut self, owner: ObjectId, params: AddCubic) -> Option<Cubic> {
        let template = self.cubic_template(params.cubic_id, params.level);
        let Some(template) = template else {
            // Java still makes a cubic without template (no skills) that only disappears; its
            // only caller checks the template first.
            warn!(
                "Missing cubic template. cubicId: {} level: {} ownerId: {}",
                params.cubic_id, params.level, owner.0
            );
            return None;
        };
        self.summons.cubics.next_serial += 1;
        let serial = self.summons.cubics.next_serial;
        let cubic = Cubic {
            id: params.cubic_id,
            owner,
            power: if params.power > 0 {
                params.power
            } else {
                template.base_m_atk()
            },
            delay_ms: (if params.delay > 0 {
                params.delay
            } else {
                template.delay
            })
            .wrapping_mul(1000),
            skill_chance: params.skill_chance,
            max_count: if params.max_count > -1 {
                params.max_count
            } else {
                template.max_count
            },
            given_by_other: params.given_by_other,
            target: None,
            serial,
            disappear_timer: None,
            action: None,
            template,
        };
        let duration = if params.duration > 0 {
            params.duration
        } else {
            cubic.template.duration
        };
        let heal = cubic.template.target_type == CubicTargetType::Heal;
        let id = cubic.id;

        // Player.addCubic: `_cubics.put(cubicId, cubic)`, `_cubicOrder.remove(cubicId)`, `add`.
        let entry = self.summons.cubics.players.entry(owner).or_default();
        let old = entry.by_id.insert(id, cubic);
        entry.order.retain(|c| *c != id);
        entry.order.push_back(id);
        if let Some(old) = &old {
            self.cancel_cubic_timers(old);
        }

        // The constructor: schedule the disappear task, start a heal cubic.
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(i64::from(duration) * 1000).unwrap_or(0));
        let timer = self.timers.schedule_at(
            due,
            TimerKind::Summon(SummonTimer::Cubic(CubicTimer::Disappear {
                owner,
                cubic: id,
                serial,
            })),
        );
        if let Some(c) = self.cubic_mut(owner, id) {
            c.disappear_timer = Some(timer);
        }
        if heal {
            self.cubic_do_action(owner, id);
        }
        old
    }

    fn cubic_mut(&mut self, owner: ObjectId, cubic_id: i32) -> Option<&mut Cubic> {
        self.summons
            .cubics
            .players
            .get_mut(&owner)?
            .by_id
            .get_mut(&cubic_id)
    }

    /// The cubic if it is still the one a timer was made for.
    fn cubic_of_timer(&self, owner: ObjectId, cubic_id: i32, serial: u64) -> Option<&Cubic> {
        self.cubic(owner, cubic_id).filter(|c| c.serial == serial)
    }

    fn cancel_cubic_timers(&mut self, cubic: &Cubic) {
        if let Some(timer) = cubic.disappear_timer {
            self.timers.cancel(timer);
        }
        if let Some(action) = cubic.action {
            self.timers.cancel(action.timer);
        }
    }

    /// Java `Player.removeCubic(cubicId)`: the cubic and its tasks are gone.
    pub fn remove_cubic(&mut self, owner: ObjectId, cubic_id: i32) -> Option<Cubic> {
        let player = self.summons.cubics.players.get_mut(&owner)?;
        let cubic = player.by_id.remove(&cubic_id);
        player.order.retain(|c| *c != cubic_id);
        let mut cubic = cubic?;
        // `cubic.stopAction(); cubic.cancelDisappear();`
        cubic.target = None;
        self.cancel_cubic_timers(&cubic);
        cubic.action = None;
        cubic.disappear_timer = None;
        Some(cubic)
    }

    /// Java `Player.removeFirstCubic()`: the cubic summoned first.
    pub fn remove_first_cubic(&mut self, owner: ObjectId) -> Option<Cubic> {
        loop {
            let first = self.summons.cubics.players.get_mut(&owner)?.order.pop_front()?;
            if self.cubic(owner, first).is_some() {
                return self.remove_cubic(owner, first);
            }
        }
    }

    /// Java `Player.stopCubics()`: every cubic goes and the player's `UserInfo` is sent again.
    pub fn stop_cubics(&mut self, owner: ObjectId) {
        if self.cubic_count(owner) == 0 {
            return;
        }
        self.clear_cubics(owner);
        self.broadcast_user_info(owner);
    }

    /// The loop of `stopCubics()` without the broadcast (`doDie`, `enterOlympiadObserverMode`).
    pub(crate) fn clear_cubics(&mut self, owner: ObjectId) {
        let Some(player) = self.summons.cubics.players.remove(&owner) else {
            return;
        };
        for cubic in player.by_id.values() {
            self.cancel_cubic_timers(cubic);
        }
    }

    /// Java `Player.stopCubicsByOthers()`: the cubics another player summoned for this one go
    /// (when an Olympiad match starts, M8).
    pub fn stop_cubics_by_others(&mut self, owner: ObjectId) {
        let given: Vec<i32> = self
            .cubics_of(owner)
            .iter()
            .filter(|c| c.given_by_other)
            .map(|c| c.id)
            .collect();
        if given.is_empty() {
            return;
        }
        for id in given {
            self.remove_cubic(owner, id);
        }
        self.broadcast_user_info(owner);
    }

    /// The owner left the world: the cubics are gone with the `Player` object (`stopCubics()` ran
    /// before), and so is its `PlayerStat`.
    pub(crate) fn forget_cubics(&mut self, owner: ObjectId) {
        self.clear_cubics(owner);
        self.summons.cubics.max_counts.remove(&owner);
    }

    // ------------------------------------------------------------------ the action task

    /// Java `Cubic.doAction()`: starts the action task unless it runs, the template has no
    /// skills or is missing.
    pub fn cubic_do_action(&mut self, owner: ObjectId, cubic_id: i32) {
        let Some(cubic) = self.cubic(owner, cubic_id) else {
            return;
        };
        if cubic.action.is_some() || cubic.template.skills.is_empty() {
            return;
        }
        // `scheduleAtFixedRate` throws for a period of 0 or less: the task is never made.
        if cubic.delay_ms <= 0 {
            return;
        }
        let serial = cubic.serial;
        let timer = self.timers.schedule_at(
            self.tick + 1,
            TimerKind::Summon(SummonTimer::Cubic(CubicTimer::Action {
                owner,
                cubic: cubic_id,
                serial,
            })),
        );
        if let Some(c) = self.cubic_mut(owner, cubic_id) {
            c.action = Some(ActionTask { timer, count: 0 });
        }
    }

    /// Java `Cubic.stopAction()`: no target, no action task.
    pub fn cubic_stop_action(&mut self, owner: ObjectId, cubic_id: i32) {
        let Some(cubic) = self.cubic_mut(owner, cubic_id) else {
            return;
        };
        cubic.target = None;
        let action = cubic.action.take();
        if let Some(action) = action {
            self.timers.cancel(action.timer);
        }
    }

    /// `AttackStanceTaskManager.addAttackStanceTask`: a playable's stance starts the action tasks
    /// of its player's cubics (all but the life cubic).
    pub(crate) fn cubics_on_attack_stance(&mut self, creature: ObjectId) {
        let Some(owner) = self.acting_player(creature) else {
            return;
        };
        let ids: Vec<i32> = self
            .cubics_of(owner)
            .iter()
            .map(|c| c.id)
            .filter(|id| *id != LIFE_CUBIC)
            .collect();
        for id in ids {
            self.cubic_do_action(owner, id);
        }
    }

    /// The timers of the cubics.
    pub(crate) fn on_cubic_timer(&mut self, timer: CubicTimer) {
        match timer {
            CubicTimer::Disappear { owner, cubic, serial } => {
                if self.cubic_of_timer(owner, cubic, serial).is_none() {
                    return;
                }
                // `CubicDisappear.run()`.
                self.remove_cubic(owner, cubic);
                self.broadcast_user_info(owner);
            }
            CubicTimer::Action { owner, cubic, serial } => {
                let Some(found) = self.cubic_of_timer(owner, cubic, serial) else {
                    return;
                };
                if found.action.is_none() {
                    return;
                }
                let heal = found.template.target_type == CubicTargetType::Heal;
                let delay = u64::try_from(found.delay_ms).unwrap_or(0);
                // The fixed rate: the next run is due before this one starts.
                let next = self.timers.schedule_at(
                    self.tick + TimerWheel::ms_to_ticks(delay),
                    TimerKind::Summon(SummonTimer::Cubic(CubicTimer::Action { owner, cubic, serial })),
                );
                if let Some(action) = self.cubic_mut(owner, cubic).and_then(|c| c.action.as_mut()) {
                    action.timer = next;
                }
                if heal {
                    self.run_cubic_heal(owner, cubic);
                } else {
                    self.run_cubic_action(owner, cubic);
                }
            }
        }
    }

    /// `_currentCount.incrementAndGet()`.
    fn cubic_count_use(&mut self, owner: ObjectId, cubic_id: i32) {
        if let Some(action) = self.cubic_mut(owner, cubic_id).and_then(|c| c.action.as_mut()) {
            action.count += 1;
        }
    }

    /// The part `CubicAction` and `CubicHeal` share before they pick a skill: the owner is gone or
    /// dead (the cubic goes), or the task used up its `maxCount` (it stops, and a `useUp` cubic
    /// goes). `true` when the run is over.
    fn cubic_run_ends(&mut self, owner: ObjectId, cubic_id: i32, check_stance: bool) -> bool {
        if !self.players.contains_key(&owner) || self.is_dead(owner) {
            self.remove_cubic(owner, cubic_id);
            self.broadcast_user_info(owner);
            return true;
        }
        if check_stance && !self.has_attack_stance_task(owner) {
            let summon_fights = self
                .summon_of(owner)
                .is_some_and(|summon| self.has_attack_stance_task(summon));
            if !summon_fights {
                self.cubic_stop_action(owner, cubic_id);
                return true;
            }
        }
        let Some(cubic) = self.cubic(owner, cubic_id) else {
            return true;
        };
        if cubic.max_count > -1 && cubic.action_count() >= cubic.max_count {
            let use_up = cubic.template.use_up > 0;
            self.cubic_stop_action(owner, cubic_id);
            if use_up {
                self.remove_cubic(owner, cubic_id);
                self.broadcast_user_info(owner);
            }
            return true;
        }
        false
    }

    /// Java `CubicAction.run()`.
    fn run_cubic_action(&mut self, owner: ObjectId, cubic_id: i32) {
        if self.cubic_run_ends(owner, cubic_id, true) {
            return;
        }
        let Some(cubic) = self.cubic(owner, cubic_id) else {
            return;
        };
        let (template, chance) = (Arc::clone(&cubic.template), cubic.skill_chance);
        for index in 0..template.skills.len() {
            if template.skills[index].priority && self.cubic_try_use_skill(owner, cubic_id, index) {
                self.cubic_count_use(owner, cubic_id);
                return;
            }
        }
        if self.rng().get_int(100) >= chance {
            return;
        }
        let picked = select_cubic_skill(&template, self.rng());
        if let Some(index) = picked
            && self.cubic_try_use_skill(owner, cubic_id, index)
        {
            self.cubic_count_use(owner, cubic_id);
        }
    }

    /// Java `CubicAction.tryUseSkill(cubicSkill)`.
    fn cubic_try_use_skill(&mut self, owner: ObjectId, cubic_id: i32, index: usize) -> bool {
        let Some(cubic) = self.cubic(owner, cubic_id) else {
            return false;
        };
        let template = Arc::clone(&cubic.template);
        let power = cubic.power;
        let cubic_skill = &template.skills[index];
        let Some(skills) = self.skill_table() else {
            return false;
        };
        let Some(skill) = skills.get(cubic_skill.skill_id, cubic_skill.skill_level) else {
            return false;
        };
        let Some(target) = self.cubic_resolve_target(owner, cubic_id, &template, cubic_skill) else {
            return false;
        };
        if self.is_dead(target) {
            return false;
        }
        if !self.cubic_conditions_hold(owner, &template.conditions, target) {
            return false;
        }
        if !self.cubic_skill_conditions_hold(owner, cubic_skill, target) {
            return false;
        }
        self.cubic_activate_skill(owner, power, skill, target);
        true
    }

    /// Java `CubicAction.resolveTarget(cubicSkill)`.
    fn cubic_resolve_target(
        &mut self,
        owner: ObjectId,
        cubic_id: i32,
        template: &CubicTemplate,
        cubic_skill: &CubicSkill,
    ) -> Option<ObjectId> {
        let mut target_type = template.target_type;
        if target_type == CubicTargetType::BySkill {
            target_type = cubic_skill.target_type;
        }
        match target_type {
            CubicTargetType::Heal => {
                self.cubic_target_for_heal(owner, cubic_id);
            }
            CubicTargetType::Master => {
                if let Some(c) = self.cubic_mut(owner, cubic_id) {
                    c.target = Some(owner);
                }
            }
            // `TARGET`, and `BY_SKILL` with a skill that names no other.
            CubicTargetType::Target | CubicTargetType::BySkill => {
                let target = self.cubic_target_for_attack(owner);
                // `isInCubicRange(owner, null)` is false.
                let in_range = target.is_some_and(|t| self.is_in_cubic_range(owner, t));
                if let Some(c) = self.cubic_mut(owner, cubic_id) {
                    c.target = if in_range { target } else { None };
                }
            }
        }
        self.cubic(owner, cubic_id)?.target
    }

    // ------------------------------------------------------------------ conditions

    fn current_hp_percent(&self, id: ObjectId) -> Option<i32> {
        if let Some(door) = self.doors.get(&id) {
            return Some(((door.current_hp() * 100.0) / f64::from(door.max_hp())) as i32);
        }
        let hp = self.core_of(id)?.cur_hp;
        let max = f64::from(self.stats(id)?.max_hp());
        Some(((hp * 100.0) / max) as i32)
    }

    fn cubic_conditions_hold(
        &self,
        owner: ObjectId,
        conditions: &[CubicCondition],
        target: ObjectId,
    ) -> bool {
        if conditions.is_empty() {
            return true;
        }
        let facts = ConditionFacts {
            hp_percent: self.current_hp_percent(target),
            distance_2d: match (self.position_of(owner), self.position_of(target)) {
                (Some(a), Some(b)) => distance_2d(a, b),
                _ => f64::INFINITY,
            },
        };
        conditions.iter().all(|c| test_condition(c, &facts))
    }

    /// Java `CubicSkill.validateConditions(cubic, owner, target)`.
    fn cubic_skill_conditions_hold(
        &self,
        owner: ObjectId,
        cubic_skill: &CubicSkill,
        target: ObjectId,
    ) -> bool {
        if cubic_skill.target_debuff && !self.has_dispellable_debuff(target) {
            return false;
        }
        self.cubic_conditions_hold(owner, &cubic_skill.conditions, target)
    }

    /// Java `CubicSkill.hasDispellableDebuff(target)`.
    fn has_dispellable_debuff(&self, target: ObjectId) -> bool {
        let Some(list) = self.effect_list(target) else {
            return false;
        };
        let Some(skills) = self.skill_table() else {
            return false;
        };
        list.queue(ListKind::Debuffs).iter().any(|info| {
            skills
                .get(info.skill.id, info.skill.level)
                .is_some_and(|s| s.can_be_dispeled)
        })
    }

    /// Java `Cubic.isInCubicRange(owner, target)`: within `MAX_MAGIC_RANGE` in 3D.
    pub(crate) fn is_in_cubic_range(&self, owner: ObjectId, target: ObjectId) -> bool {
        let (Some(o), Some(t)) = (self.position_of(owner), self.position_of(target)) else {
            return false;
        };
        let x = i64::from(o.0) - i64::from(t.0);
        let y = i64::from(o.1) - i64::from(t.1);
        let z = i64::from(o.2) - i64::from(t.2);
        (x * x) + (y * y) + (z * z) <= MAX_MAGIC_RANGE * MAX_MAGIC_RANGE
    }

    // ------------------------------------------------------------------ targets

    /// Java `Cubic.getCubicTarget()`: the valid offensive target for the owner's cubics, from the
    /// owner's own target.
    pub(crate) fn cubic_target_for_attack(&self, owner: ObjectId) -> Option<ObjectId> {
        let owner_target = self.target_of(owner)?;
        // `isOnEvent()` (custom events): not modelled.
        if self.is_in_duel(owner) {
            return self.cubic_duel_target(owner, owner_target);
        }
        // "Olympiad targeting." (M8-OLY-01): an enemy fighter of the same game, else nothing.
        if self.is_in_olympiad_mode(owner) {
            if self.is_olympiad_start(owner) && self.is_playable(owner_target) {
                let target_player = self.acting_player(owner_target)?;
                if self.olympiad_game_id(target_player) == self.olympiad_game_id(owner)
                    && self.olympiad_side(target_player) != self.olympiad_side(owner)
                {
                    return Some(owner_target);
                }
            }
            return None;
        }
        // Test the owner's target and use it when valid.
        let is_creature = self
            .objects
            .get(&owner_target)
            .is_some_and(|e| e.kind.is_creature());
        let summon = self.summon_of(owner);
        if !is_creature || Some(owner_target) == summon || owner_target == owner {
            return None;
        }
        // Target mobs that have aggro on the owner or the owner's summon.
        if self.is_attackable_npc(owner_target) {
            if self.aggro_contains(owner_target, owner) && !self.is_dead(owner_target) {
                return Some(owner_target);
            }
            if let Some(summon) = summon
                && self.aggro_contains(owner_target, summon)
                && !self.is_dead(owner_target)
            {
                return Some(owner_target);
            }
        }

        // Get target in PvP or siege.
        let in_pvp_zone = self.is_inside_zone(owner, ZoneId::Pvp);
        if !((self.pvp_flag_of(owner) > 0 && !self.is_inside_zone(owner, ZoneId::Peace)) || in_pvp_zone) {
            return None;
        }
        if self.is_dead(owner_target) {
            return None;
        }
        // NOTE(java-quirk): `ownerTarget.asPlayer()` is the owner of a summon, so a cubic aimed at
        // an enemy's summon attacks the enemy player.
        let enemy = self.acting_player(owner_target)?;
        let mut target_it = true;
        if let Some(party) = self.party_of(owner)
            && self.party_members(party).contains(&enemy)
        {
            target_it = false;
        } else if let Some(channel) = self.command_channel_of(owner)
            && self.command_channel_contains(channel, enemy)
        {
            target_it = false;
        }
        if !in_pvp_zone && let Some(clan) = self.clan_of(owner) {
            if clan.is_member(enemy.0) {
                target_it = false;
            }
            let ally = self.ally_id_of(owner);
            if ally > 0 && self.ally_id_of(enemy) > 0 && ally == self.ally_id_of(enemy) {
                target_it = false;
            }
        }
        if self.pvp_flag_of(enemy) == 0 && !self.is_inside_zone(enemy, ZoneId::Pvp) {
            target_it = false;
        }
        if self.is_inside_zone(enemy, ZoneId::Peace) {
            target_it = false;
        }
        // The siege states (`getSiegeState() > 0 && equal`, M7-SIEGE-01).
        let state = self.siege_state_of(owner);
        if state > 0 && state == self.siege_state_of(enemy) {
            target_it = false;
        }
        if !self.objects.get(&enemy).is_some_and(|e| e.spawned) {
            target_it = false;
        }
        target_it.then_some(enemy)
    }

    /// The duel branch of `Cubic.getCubicTarget()`.
    fn cubic_duel_target(&self, owner: ObjectId, owner_target: ObjectId) -> Option<ObjectId> {
        // A duel whose challenger or challenged left the party: Java's null pointer is caught and
        // the target stays empty.
        let (player_a, player_b, party_duel) = self.duel_parts(self.duel_id_of(owner))?;
        let (player_a, player_b) = (player_a?, player_b?);
        let mut target: Option<ObjectId> = None;
        if party_duel {
            let party_a = self.party_of(player_a);
            let party_b = self.party_of(player_b);
            let mut party_enemy: Option<crate::party::PartyId> = None;
            if let Some(party_a_id) = party_a {
                if self.party_members(party_a_id).contains(&owner) {
                    if party_b.is_some() {
                        party_enemy = party_b;
                    } else {
                        target = Some(player_b);
                    }
                } else {
                    party_enemy = party_a;
                }
            } else if player_a == owner {
                if party_b.is_some() {
                    party_enemy = party_b;
                } else {
                    target = Some(player_b);
                }
            } else {
                target = Some(player_a);
            }
            if (target == Some(player_a) || target == Some(player_b)) && target == Some(owner_target) {
                return target;
            }
            if let Some(enemy) = party_enemy {
                return self
                    .party_members(enemy)
                    .contains(&owner_target)
                    .then_some(owner_target);
            }
        }
        if player_a != owner && owner_target == player_a {
            return Some(player_a);
        }
        if player_b != owner && owner_target == player_b {
            return Some(player_b);
        }
        None
    }

    /// Java `Cubic.cubicTargetForHeal()`: the friend with the lowest HP fraction in range.
    fn cubic_target_for_heal(&mut self, owner: ObjectId, cubic_id: i32) {
        let hp_of = |world: &World, id: ObjectId| -> Option<(f64, f64)> {
            Some((world.core_of(id)?.cur_hp, f64::from(world.stats(id)?.max_hp())))
        };
        let mut target: Option<ObjectId> = None;
        let mut percent_left = 100.0_f64;
        let mut party = self.party_of(owner);
        // Treat non-party duels as solo context for heal target selection.
        if self.is_in_duel(owner)
            && !self
                .duel_parts(self.duel_id_of(owner))
                .is_some_and(|(_, _, party_duel)| party_duel)
        {
            party = None;
        }
        // The Olympiad check (`!isInOlympiadMode()`, M8-OLY-01).
        if let Some(party) = party.filter(|_| !self.is_in_olympiad_mode(owner)) {
            // Check party members and their summons inside cubic range.
            for member in self.party_members(party) {
                // Select the party member with the lowest HP percentage inside cubic range.
                if let Some((hp, max)) = hp_of(self, member)
                    && !self.is_dead(member)
                    && self.is_in_cubic_range(owner, member)
                    && hp < max
                    && percent_left > hp / max
                {
                    percent_left = hp / max;
                    target = Some(member);
                }
                let Some(summon) = self.summon_of(member) else {
                    continue;
                };
                if self.is_dead(summon) {
                    continue;
                }
                // Check if the party member's summon is inside cubic range.
                if !self.is_in_cubic_range(owner, summon) {
                    continue;
                }
                // Select the summon when it has the lowest HP percentage.
                if let Some((hp, max)) = hp_of(self, summon)
                    && hp < max
                    && percent_left > hp / max
                {
                    percent_left = hp / max;
                    target = Some(summon);
                }
            }
        } else {
            if let Some((hp, max)) = hp_of(self, owner)
                && hp < max
            {
                percent_left = hp / max;
                target = Some(owner);
            }
            if let Some(summon) = self.summon_of(owner)
                && !self.is_dead(summon)
                && let Some((hp, max)) = hp_of(self, summon)
                && hp < max
                && percent_left > hp / max
                && self.is_in_cubic_range(owner, summon)
            {
                target = Some(summon);
            }
        }
        if let Some(c) = self.cubic_mut(owner, cubic_id) {
            c.target = target;
        }
    }

    // ------------------------------------------------------------------ the heal task

    /// Java `CubicHeal.run()`.
    fn run_cubic_heal(&mut self, owner: ObjectId, cubic_id: i32) {
        if self.cubic_run_ends(owner, cubic_id, false) {
            return;
        }
        self.cubic_target_for_heal(owner, cubic_id);
        let Some(cubic) = self.cubic(owner, cubic_id) else {
            return;
        };
        let template = Arc::clone(&cubic.template);
        let Some(target) = cubic.target else {
            return;
        };
        if self.is_dead(target) {
            return;
        }
        // `getHealSkill(target)`.
        if !self.cubic_conditions_hold(owner, &template.conditions, target) {
            return;
        }
        let Some(skills) = self.skill_table() else {
            return;
        };
        let Some(cubic_skill) = template.skills.iter().find(|cs| {
            cs.skill_id == SKILL_CUBIC_HEAL
                && skills.get(cs.skill_id, cs.skill_level).is_some()
                && self.cubic_skill_conditions_hold(owner, cs, target)
        }) else {
            return;
        };
        let Some(skill) = skills.get(cubic_skill.skill_id, cubic_skill.skill_level) else {
            return;
        };
        let missing = match (self.core_of(target), self.stats(target)) {
            (Some(core), Some(view)) => f64::from(view.max_hp()) - core.cur_hp,
            _ => return,
        };
        if missing <= f64::from(skill.power) {
            return;
        }
        self.activate_skill_for_cubic(owner, &[target], SkillKey::of(skill));
        self.broadcast_cubic_skill_use(owner, target, skill);
        self.cubic_count_use(owner, cubic_id);
    }

    // ------------------------------------------------------------------ casting

    /// `broadcastPacket(new MagicSkillUse(owner, target, skillId, skillLevel, 0, 0))`.
    fn broadcast_cubic_skill_use(&self, owner: ObjectId, target: ObjectId, skill: &SkillTemplate) {
        let (Some(me), Some(at)) = (self.position_of(owner), self.position_of(target)) else {
            return;
        };
        let ground = self
            .cast
            .creatures
            .get(&owner)
            .and_then(|s| s.current_skill_world_position)
            .map(|(x, y, z)| SkillPosition { x, y, z });
        self.broadcast_creature_packet(
            owner,
            &MagicSkillUse {
                creature_id: owner.0,
                target_id: target.0,
                skill_id: skill.id,
                skill_level: skill.level,
                hit_time: 0,
                reuse_delay: 0,
                creature: SkillPosition {
                    x: me.0,
                    y: me.1,
                    z: me.2,
                },
                ground_location: ground,
                target: SkillPosition {
                    x: at.0,
                    y: at.1,
                    z: at.2,
                },
            },
        );
    }

    /// Java `CubicAction.activateSkill(skill, target)`.
    fn cubic_activate_skill(&mut self, owner: ObjectId, power: i32, skill: &SkillTemplate, target: ObjectId) {
        self.broadcast_cubic_skill_use(owner, target, skill);
        if skill.operate_type.is_continuous() || skill.operate_type.is_self_continuous() {
            self.cubic_use_continuous(owner, skill, target);
        } else if self.skill_has_effect_type(skill, &[EffectType::MagicalAttack]) {
            self.cubic_use_mdam(owner, power, skill, target);
        } else if self.skill_has_effect_type(skill, &[EffectType::HpDrain]) {
            self.cubic_use_drain(owner, power, skill, target);
        } else if self
            .skill_has_effect_type(skill, &[EffectType::Stun, EffectType::Root, EffectType::Paralyze])
        {
            self.cubic_use_disabler(owner, skill, target);
        } else if self
            .skill_has_effect_type(skill, &[EffectType::DmgOverTime, EffectType::DmgOverTimePercent])
        {
            self.cubic_use_continuous(owner, skill, target);
        } else if self.skill_has_effect_type(skill, &[EffectType::Aggression]) {
            self.cubic_use_disabler(owner, skill, target);
        } else {
            self.activate_skill_for_cubic(owner, &[target], SkillKey::of(skill));
        }
    }

    /// Java `Cubic.applyCubicEffects(skill, target)`: the effects land without the landing roll
    /// of `Skill.applyEffects` (the cubic rolled `calcCubicSkillSuccess` before).
    fn cubic_apply_effects(&mut self, owner: ObjectId, skill: &SkillTemplate, target: ObjectId) {
        // The GM test (`!getAccessLevel().canGiveDamage()`) is not modelled, see `effects/apply.rs`.
        if target != owner && has_negative_effect(skill) && self.is_invul(target) {
            return;
        }
        if self.is_invul_against(target, skill.id, skill.level) {
            return;
        }
        let add_continuous = skill.is_toggle()
            || skill.operate_type.is_continuous()
            || skill.operate_type.is_self_continuous();
        let mut info = self.new_buff_info(owner, target, skill);
        self.apply_effect_scope(EffectScope::General, &mut info, true, add_continuous);
        if let Some(scope) = self.pvp_or_pve_scope(owner, target) {
            self.apply_effect_scope(scope, &mut info, true, add_continuous);
        }
        self.apply_effect_scope(EffectScope::Channeling, &mut info, true, add_continuous);
        if add_continuous {
            self.add_buff_info(info);
        }
    }

    /// "If this is a negative effect skill notify the duel manager, so it can be removed after the
    /// duel (player & target must be in the same duel)."
    fn cubic_notify_duel(&mut self, owner: ObjectId, skill: &SkillTemplate, target: ObjectId) {
        if self.is_player(target)
            && self.is_in_duel(target)
            && has_negative_effect(skill)
            && self.duel_id_of(owner) == self.duel_id_of(target)
        {
            self.duel_on_buff(target, skill.id);
        }
    }

    /// Java `Cubic.useCubicContinuous(skill, targets)` for one target.
    fn cubic_use_continuous(&mut self, owner: ObjectId, skill: &SkillTemplate, target: ObjectId) {
        if self.is_dead(target) {
            return;
        }
        if has_negative_effect(skill) {
            let acted = self.calc_cubic_skill_success(owner, target, skill, SHIELD_DEFENSE_FAILED);
            if !acted {
                self.send(owner, &SystemMessage::new(SystemMessageId::YourAttackHasFailed));
                return;
            }
        }
        self.cubic_apply_effects(owner, skill, target);
        self.cubic_notify_duel(owner, skill, target);
    }

    /// The common start of the two damage skills: the magic critical hit, the shield and the
    /// damage as an `int`.
    fn cubic_magic_hit(
        &mut self,
        owner: ObjectId,
        power: i32,
        skill: &SkillTemplate,
        target: ObjectId,
    ) -> (i32, bool) {
        let mcrit = self.calc_mcrit_for(owner, target, skill);
        let shld = self.calc_shld_use_skill(owner, target, skill, true);
        let damage = self.calc_cubic_magic_dam(owner, target, skill, power, mcrit, shld) as i32;
        (damage, mcrit)
    }

    /// Java `Cubic.useCubicMdam(skill, targets)` for one target.
    fn cubic_use_mdam(&mut self, owner: ObjectId, power: i32, skill: &SkillTemplate, target: ObjectId) {
        if self.is_alike_dead(target) {
            // NOTE(java-quirk): a dead player target is hit as well when the fake death setting
            // is on (`stopFakeDeath` is all that is checked).
            if self.is_player(target) && self.settings.effects.fake_death_damage_stand {
                self.stop_fake_death(target, true);
            } else {
                return;
            }
        }
        let (damage, mcrit) = self.cubic_magic_hit(owner, power, skill, target);
        if damage <= 0 {
            return;
        }
        // Manage attack or cast break of the target.
        self.skill_atk_break(target, f64::from(damage));
        // Shield Deflect Magic: If target is reflecting the skill then no damage is done.
        let ctx = calc_skill(skill);
        let deflect = self.stats(target).map_or(0.0, |t| {
            t.calc_stat(Stat::VengeanceSkillMagicDamage, 0.0, Some(target), Some(&ctx))
        });
        if deflect > f64::from(self.rng().get_int(100)) {
            return;
        }
        self.send_damage_message(owner, target, damage, mcrit, false, false);
        self.reduce_current_hp_by_skill(target, f64::from(damage), Some(owner), skill);
    }

    /// Java `Cubic.useCubicDrain(skill, targets)` for one target.
    fn cubic_use_drain(&mut self, owner: ObjectId, power: i32, skill: &SkillTemplate, target: ObjectId) {
        if self.is_alike_dead(target) {
            return;
        }
        let (damage, mcrit) = self.cubic_magic_hit(owner, power, skill, target);
        // TODO(java): Unhardcode fixed value.
        let hp_add = 0.4 * f64::from(damage);
        let current = self.core_of(owner).map_or(0.0, |c| c.cur_hp);
        let max_hp = self.stats(owner).map_or(0.0, |v| f64::from(v.max_hp()));
        let hp = if current + hp_add > max_hp {
            max_hp
        } else {
            current + hp_add
        };
        self.set_current_hp(owner, hp, true);

        // Check if the target should be damaged.
        if damage > 0 && !self.is_dead(target) {
            self.reduce_current_hp_by_skill(target, f64::from(damage), Some(owner), skill);
            // Manage attack or cast break of the target.
            self.skill_atk_break(target, f64::from(damage));
            self.send_damage_message(owner, target, damage, mcrit, false, false);
        }
    }

    /// Java `Cubic.useCubicDisabler(skill, targets)` for one target.
    fn cubic_use_disabler(&mut self, owner: ObjectId, skill: &SkillTemplate, target: ObjectId) {
        if self.is_dead(target) {
            return;
        }
        if self.skill_has_effect_type(skill, &[EffectType::Stun, EffectType::Paralyze, EffectType::Root])
            && self.calc_cubic_skill_success(owner, target, skill, SHIELD_DEFENSE_FAILED)
        {
            self.cubic_apply_effects(owner, skill, target);
            self.cubic_notify_duel(owner, skill, target);
        }
        if self.skill_has_effect_type(skill, &[EffectType::Aggression])
            && self.calc_cubic_skill_success(owner, target, skill, SHIELD_DEFENSE_FAILED)
        {
            if self.is_attackable_npc(target) {
                let level = self.stats(target).map_or(0, |v| v.level());
                // `(int) ((150 * skill.getPower()) / (target.getLevel() + 7))`: float arithmetic.
                let aggro = ((150.0_f32 * skill.power) / (level + 7) as f32) as i32;
                self.notify_action_aggression(target, owner, i64::from(aggro));
            }
            self.cubic_apply_effects(owner, skill, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formulas::rng::testing::Fixed;

    fn skill(id: i32, rate: i32, priority: bool) -> CubicSkill {
        CubicSkill {
            skill_id: id,
            skill_level: 1,
            trigger_rate: rate,
            target_type: CubicTargetType::Target,
            target_debuff: false,
            priority,
            conditions: vec![],
        }
    }

    fn template(mode: CubicSelectionMode, skills: Vec<CubicSkill>) -> CubicTemplate {
        CubicTemplate {
            id: 1,
            level: 1,
            slot: 1,
            duration: 900,
            delay: 10,
            max_count: 30,
            use_up: 1,
            power: 100.0,
            target_type: CubicTargetType::Target,
            selection_mode: mode,
            conditions: vec![],
            skills,
        }
    }

    #[test]
    fn random_mode_picks_the_nth_normal_skill_and_skips_priority_ones() {
        let t = template(
            CubicSelectionMode::Random,
            vec![skill(1, 100, false), skill(2, 100, true), skill(3, 100, false)],
        );
        // Two normal skills: `Rnd.get(2)`; index 1 is the third entry (the priority one is
        // skipped and not counted).
        let mut rng = Fixed::new(&[1]);
        assert_eq!(select_cubic_skill(&t, &mut rng), Some(2));
        let mut rng = Fixed::new(&[0]);
        assert_eq!(select_cubic_skill(&t, &mut rng), Some(0));
        // Only priority skills: nothing to pick, no draw.
        let only = template(CubicSelectionMode::Random, vec![skill(2, 100, true)]);
        let mut rng = Fixed::new(&[]);
        assert_eq!(select_cubic_skill(&only, &mut rng), None);
        assert!(rng.exhausted());
    }

    #[test]
    fn ordered_mode_takes_the_first_skill_whose_rate_passes() {
        let t = template(
            CubicSelectionMode::Ordered,
            vec![skill(1, 30, false), skill(2, 100, false), skill(3, 100, false)],
        );
        // The first has rate 30: a draw of 29 passes, 30 does not (and the 100 rate needs no
        // draw).
        let mut rng = Fixed::new(&[29]);
        assert_eq!(select_cubic_skill(&t, &mut rng), Some(0));
        let mut rng = Fixed::new(&[30]);
        assert_eq!(select_cubic_skill(&t, &mut rng), Some(1));
        assert!(rng.exhausted());
    }

    #[test]
    fn weighted_mode_uses_the_rates_as_weights() {
        let t = template(
            CubicSelectionMode::Weighted,
            vec![
                skill(1, 20, false),
                skill(2, 0, false),
                skill(3, 70, true),
                skill(4, 30, false),
            ],
        );
        // Total 50 (the rate 0 and the priority skill do not count): 0..19 skill 1, 20..49 skill 4.
        for (draw, expected) in [(0, 0), (19, 0), (20, 3), (49, 3)] {
            let mut rng = Fixed::new(&[draw]);
            assert_eq!(select_cubic_skill(&t, &mut rng), Some(expected), "draw {draw}");
        }
        // No weight at all falls back to the random pick (`Rnd.get(2)` normal skills).
        let t = template(
            CubicSelectionMode::Weighted,
            vec![skill(1, 0, false), skill(2, 0, false)],
        );
        let mut rng = Fixed::new(&[1]);
        assert_eq!(select_cubic_skill(&t, &mut rng), Some(1));
    }

    #[test]
    fn conditions_follow_java() {
        let facts = ConditionFacts {
            hp_percent: Some(50),
            distance_2d: 300.0,
        };
        let hp = |kind, percent| CubicCondition::Hp { kind, percent };
        assert!(test_condition(&hp(HpConditionType::Greater, 49), &facts));
        assert!(!test_condition(&hp(HpConditionType::Greater, 50), &facts));
        assert!(test_condition(&hp(HpConditionType::Lesser, 51), &facts));
        assert!(!test_condition(&hp(HpConditionType::Lesser, 50), &facts));
        assert!(test_condition(&CubicCondition::Range(300), &facts));
        assert!(!test_condition(&CubicCondition::Range(299), &facts));
        // Strictly between the bounds.
        assert!(test_condition(
            &CubicCondition::HealthPercent { min: 49, max: 51 },
            &facts
        ));
        assert!(!test_condition(
            &CubicCondition::HealthPercent { min: 50, max: 51 },
            &facts
        ));
        assert!(!test_condition(
            &CubicCondition::HealthPercent { min: 49, max: 50 },
            &facts
        ));
        // Something that is no creature or door has no HP percent.
        let none = ConditionFacts {
            hp_percent: None,
            distance_2d: 0.0,
        };
        assert!(!test_condition(&hp(HpConditionType::Lesser, 100), &none));
        assert!(!test_condition(
            &CubicCondition::HealthPercent { min: 0, max: 100 },
            &none
        ));
        assert!(test_condition(&CubicCondition::Range(0), &none));
    }
}
