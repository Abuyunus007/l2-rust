//! `SummonAI`: a summon follows its owner, attacks and casts on command, and steps aside when it
//! is hit next to its owner. Through the M4 intention machine (`ai/mod.rs`), which calls in here
//! for a summon where Java's `SummonAI` overrides `CreatureAI`.
// Java: ai/SummonAI.java, ai/PlayableAI.java (the summon's view: `asPlayer()` is the owner)
// Java: ai/CreatureAI.java (setIntentionFollow, checkTargetLost's summon branch)
// Java: entity/actor/Summon.java (setFollowStatus, followOwner, setImmobilized)
//
// NOTE: Java's `SummonAI` refuses the follow and attack intentions when the pathfinding is on
// and `PathFinding.findPath(...)` (a synchronous search) finds no path. Path searches run on the
// path workers here (D-14) and the world has no searcher of its own, so that refusal is left out:
// the summon takes the intention and its move fails the way any unreachable move does.

use l2_data::SkillTemplate;
use l2_util::ObjectId;

use super::SummonTimer;
use crate::movement::Intention;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `SummonAI.AVOID_RADIUS`.
const AVOID_RADIUS: i32 = 70;
/// `startAvoidTask`: `scheduleAtFixedRate(this, 100, 100)`.
const AVOID_INTERVAL_MS: u64 = 100;
/// `maybeMoveToPawn(getTarget(), 36)` of `thinkPickUp` / `thinkInteract`.
const PICK_UP_OFFSET: i32 = 36;

impl World {
    // ------------------------------------------------------------------ Summon

    /// Java `Summon.setFollowStatus(value)`.
    pub fn set_follow_status(&mut self, summon: ObjectId, value: bool) {
        let Some(state) = self.summon_mut(summon) else {
            return;
        };
        state.follow = value;
        let owner = state.owner;
        if value {
            self.summon_set_intention_follow(summon, Some(owner));
        } else {
            self.summon_set_intention_idle(summon);
        }
    }

    /// Java `Summon.getFollowStatus()`.
    pub fn follow_status(&self, summon: ObjectId) -> bool {
        self.summon(summon).is_some_and(|s| s.follow)
    }

    /// Java `Summon.setImmobilized(value)` after `Creature.setImmobilized`: an immobilized summon
    /// stops following and picks it up again afterwards.
    pub(crate) fn summon_on_immobilized(&mut self, summon: ObjectId, value: bool) {
        let Some(state) = self.summon_mut(summon) else {
            return;
        };
        if value {
            state.previous_follow = state.follow;
            // If immobilized temporarily disable follow mode.
            if state.previous_follow {
                self.set_follow_status(summon, false);
            }
        } else {
            // If not more immobilized restore previous follow mode.
            let previous = state.previous_follow;
            self.set_follow_status(summon, previous);
        }
    }

    // ------------------------------------------------------------------ intentions

    /// Java `SummonAI.setIntentionIdle()`.
    pub(crate) fn summon_set_intention_idle(&mut self, summon: ObjectId) {
        self.stop_follow(summon);
        if let Some(state) = self.summon_mut(summon) {
            state.ai.start_follow = false;
        }
        self.stop_summon_avoid_task(summon);
        self.summon_set_intention_active(summon);
    }

    /// Java `SummonAI.setIntentionActive()`: back to the owner when the summon follows, else
    /// `CreatureAI`'s.
    pub(crate) fn summon_set_intention_active(&mut self, summon: ObjectId) {
        let Some(state) = self.summon(summon) else {
            return;
        };
        let (start_follow, owner) = (state.ai.start_follow, state.owner);
        self.start_summon_avoid_task(summon);
        if start_follow {
            self.summon_set_intention_follow(summon, Some(owner));
        } else {
            self.creature_set_intention_active(summon);
        }
    }

    /// Java `SummonAI.setIntentionFollow(target)` then `CreatureAI.setIntentionFollow(target)`.
    pub(crate) fn summon_set_intention_follow(&mut self, summon: ObjectId, target: Option<ObjectId>) {
        // `target.asCreature()`.
        let Some(target) = target.filter(|t| self.objects.get(t).is_some_and(|e| e.kind.is_creature())) else {
            self.client_action_failed(summon);
            return;
        };
        // The pathfinding refusal: see the module comment.
        self.start_summon_avoid_task(summon);
        self.creature_set_intention_follow(summon, target);
    }

    /// Java `CreatureAI.setIntentionFollow(target)` (players use it too, see `ai/mod.rs`).
    pub(crate) fn creature_set_intention_follow(&mut self, id: ObjectId, target: ObjectId) {
        if self.intention_of(id) == Intention::Rest
            || self.is_all_skills_disabled(id)
            || self.is_casting_now(id)
            || self.is_movement_disabled(id)
            || self.is_dead(id)
            || id == target
        {
            self.client_action_failed(id);
            return;
        }
        // Stop the actor auto-attack client side by sending Server->Client packet AutoAttackStop
        // (broadcast).
        self.client_stop_auto_attack(id);
        // Set the Intention of this AbstractAI to FOLLOW.
        self.set_intention_value(id, Intention::Follow);
        // Create and Launch an AI Follow Task to execute every 1s.
        self.start_follow(id, target, crate::interact::NORMAL_FOLLOW_RANGE);
    }

    /// Java `SummonAI.setIntentionCast(skill, target)`: remembers the attack the cast interrupts,
    /// stops the avoid task. (The caller goes on with `PlayableAI` / `CreatureAI`.)
    pub(crate) fn summon_before_intention_cast(&mut self, summon: ObjectId) {
        let last_attack = if self.intention_of(summon) == Intention::Attack {
            self.attack_target_of(summon)
        } else {
            None
        };
        if let Some(state) = self.summon_mut(summon) {
            state.ai.last_attack = last_attack;
        }
        self.stop_summon_avoid_task(summon);
    }

    /// Java `SummonAI.notifyFollowStatusChange()` (the movement mode button).
    pub fn notify_follow_status_change(&mut self, summon: ObjectId) {
        let Some(state) = self.summon_mut(summon) else {
            return;
        };
        state.ai.start_follow = !state.ai.start_follow;
        let start_follow = state.ai.start_follow;
        if matches!(
            self.intention_of(summon),
            Intention::Active | Intention::Follow | Intention::Idle | Intention::MoveTo | Intention::PickUp
        ) {
            self.set_follow_status(summon, start_follow);
        }
    }

    /// Java `SummonAI.setStartFollowController(value)`.
    pub(crate) fn set_start_follow_controller(&mut self, summon: ObjectId, value: bool) {
        if let Some(state) = self.summon_mut(summon) {
            state.ai.start_follow = value;
        }
    }

    // ------------------------------------------------------------------ think

    /// Java `SummonAI.notifyActionThink()`.
    pub(crate) fn summon_notify_think(&mut self, summon: ObjectId) {
        let Some(state) = self.summon(summon) else {
            return;
        };
        if state.ai.thinking || self.is_casting_now(summon) || self.is_all_skills_disabled(summon) {
            return;
        }
        if let Some(state) = self.summon_mut(summon) {
            state.ai.thinking = true;
        }
        match self.intention_of(summon) {
            Intention::Attack => self.summon_think_attack(summon),
            Intention::Cast => self.summon_think_cast(summon),
            Intention::PickUp => self.summon_think_pick_up(summon),
            Intention::Interact => self.summon_think_interact(summon),
            _ => {}
        }
        if let Some(state) = self.summon_mut(summon) {
            state.ai.thinking = false;
        }
    }

    /// Java `SummonAI.thinkAttack()`.
    fn summon_think_attack(&mut self, summon: ObjectId) {
        let target = self.attack_target_of(summon);
        if self.check_target_lost_or_dead(summon, target) {
            self.set_attack_target(summon, None);
            return;
        }
        let Some(target) = target else {
            return;
        };
        let range = self.stats(summon).map_or(0, |s| s.physical_attack_range());
        if self.maybe_move_to_pawn(summon, target, range) {
            return;
        }
        self.client_stop_moving(summon, None);
        self.do_attack(summon, target);
    }

    /// Java `SummonAI.thinkCast()`.
    fn summon_think_cast(&mut self, summon: ObjectId) {
        let Some(key) = self.cast_skill_of(summon) else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        let target = self.cast_target_of(summon);
        if self.summon_check_target_lost(summon, target, Some(skill)) {
            self.set_cast_target(summon, None);
            return;
        }
        let Some(target) = target else {
            return;
        };
        let start_follow = self.summon(summon).is_some_and(|s| s.ai.start_follow);
        let range = self.magical_attack_range_for(summon, skill);
        if self.maybe_move_to_pawn(summon, target, range) {
            return;
        }
        self.client_stop_moving(summon, None);
        self.set_follow_status(summon, false);
        self.summon_set_intention_idle(summon);
        self.set_start_follow_controller(summon, start_follow);
        self.do_cast(summon, key);
    }

    /// Java `SummonAI.thinkPickUp()`: a servitor picks nothing up (`Servitor.doPickupItem`).
    fn summon_think_pick_up(&mut self, summon: ObjectId) {
        let target = self.pawn_of(summon).and_then(|p| p.ai_target);
        if self.summon_check_target_lost(summon, target, None) {
            return;
        }
        if let Some(target) = target
            && self.maybe_move_to_pawn(summon, target, PICK_UP_OFFSET)
        {
            return;
        }
        self.summon_set_intention_idle(summon);
        // `doPickupItem(getTarget())`: a pet's is PET-01's.
    }

    /// Java `SummonAI.thinkInteract()`.
    fn summon_think_interact(&mut self, summon: ObjectId) {
        let target = self.pawn_of(summon).and_then(|p| p.ai_target);
        if self.summon_check_target_lost(summon, target, None) {
            return;
        }
        if let Some(target) = target
            && self.maybe_move_to_pawn(summon, target, PICK_UP_OFFSET)
        {
            return;
        }
        self.summon_set_intention_idle(summon);
    }

    /// Java `CreatureAI.checkTargetLost(target)` for a summon, with the AI's `_skill` (`None` =
    /// `null`): `true` (and the intention goes active) when the target is gone, an offensive
    /// area skill cannot see it, or the summon cannot walk to it.
    pub(crate) fn summon_check_target_lost(
        &mut self,
        summon: ObjectId,
        target: Option<ObjectId>,
        skill: Option<&SkillTemplate>,
    ) -> bool {
        let Some(target) = target else {
            self.set_intention_active(summon);
            return true;
        };
        if let Some(skill) = skill
            && crate::effects::apply::has_negative_effect(skill)
            && skill.affect_range > 0
            && !self.can_see_target(summon, target)
        {
            self.set_intention_active(summon);
            return true;
        }
        if self.summon_can_move_to(summon, target) {
            return false;
        }
        self.set_intention_active(summon);
        true
    }

    /// Java `GeoEngine.canMoveToTarget(summon, target)`.
    pub(crate) fn summon_can_move_to(&self, summon: ObjectId, target: ObjectId) -> bool {
        let (Some(from), Some(to)) = (self.position_of(summon), self.position_of(target)) else {
            return false;
        };
        let instance = self.objects.get(&summon).map_or(0, |e| e.instance_id);
        self.geo_can_move_to_target(
            l2_geo::Location::new(from.0, from.1, from.2),
            l2_geo::Location::new(to.0, to.1, to.2),
            instance,
        )
    }

    // ------------------------------------------------------------------ events

    /// Java `SummonAI.notifyActionFinishCasting()`: back to following (or idle), or the attack
    /// the cast interrupted.
    pub(crate) fn summon_notify_finish_casting(&mut self, summon: ObjectId) {
        let Some(state) = self.summon_mut(summon) else {
            return;
        };
        match state.ai.last_attack.take() {
            None => {
                let start_follow = state.ai.start_follow;
                self.set_follow_status(summon, start_follow);
            }
            Some(replay) => self.set_intention_attack(summon, replay),
        }
    }

    /// Java `SummonAI.avoidAttack(attacker)`: hit next to its owner, the summon steps aside on
    /// the avoid task's next run.
    pub(crate) fn summon_avoid_attack(&mut self, summon: ObjectId, attacker: ObjectId) {
        if !self.objects.get(&attacker).is_some_and(|e| e.kind.is_creature()) {
            return;
        }
        let Some(owner) = self.summon_owner(summon) else {
            return;
        };
        if owner == attacker {
            return;
        }
        let (Some(a), Some(b)) = (self.position_of(owner), self.position_of(summon)) else {
            return;
        };
        // `isInsideRadius3D(actor, 2 * AVOID_RADIUS)`: strictly inside.
        if crate::target::distance_3d(a, b) < f64::from(2 * AVOID_RADIUS)
            && let Some(state) = self.summon_mut(summon)
        {
            state.ai.start_avoid = true;
        }
    }

    /// Java `SummonAI.startAvoidTask()`.
    fn start_summon_avoid_task(&mut self, summon: ObjectId) {
        if self.summon(summon).is_none_or(|s| s.ai.avoid_timer.is_some()) {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(AVOID_INTERVAL_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Summon(SummonTimer::Avoid(summon)));
        if let Some(state) = self.summon_mut(summon) {
            state.ai.avoid_timer = Some(timer);
        }
    }

    /// Java `SummonAI.stopAvoidTask()`.
    pub(crate) fn stop_summon_avoid_task(&mut self, summon: ObjectId) {
        let timer = self.summon_mut(summon).and_then(|s| s.ai.avoid_timer.take());
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// Java `SummonAI.run()`: one run of the avoid task.
    pub(crate) fn on_summon_avoid_timer(&mut self, summon: ObjectId) {
        let Some(state) = self.summon_mut(summon) else {
            return;
        };
        // The fixed rate: the next run first.
        state.ai.avoid_timer = None;
        let start_avoid = std::mem::take(&mut state.ai.start_avoid);
        let owner = state.owner;
        self.start_summon_avoid_task(summon);
        if !start_avoid {
            return;
        }
        if self.is_moving(summon) || self.is_dead(summon) || self.is_movement_disabled(summon) {
            return;
        }
        let (Some((owner_x, owner_y, _)), Some((x, y, z))) = (self.position_of(owner), self.position_of(summon))
        else {
            return;
        };
        let degrees = self.rng().get(-90, 90);
        let angle = f64::from(degrees).to_radians() + f64::from(owner_y - y).atan2(f64::from(owner_x - x));
        let target_x = owner_x + (f64::from(AVOID_RADIUS) * angle.cos()) as i32;
        let target_y = owner_y + (f64::from(AVOID_RADIUS) * angle.sin()) as i32;
        let instance = self.objects.get(&summon).map_or(0, |e| e.instance_id);
        if self.geo_can_move_to_target(
            l2_geo::Location::new(x, y, z),
            l2_geo::Location::new(target_x, target_y, z),
            instance,
        ) {
            self.ai_move_to(summon, target_x, target_y, z);
        }
    }

    /// Java `SummonAI.stopAITask()`: the avoid task, then `AbstractAI.stopAITask()` (the follow).
    pub(crate) fn stop_summon_ai_task(&mut self, summon: ObjectId) {
        self.stop_summon_avoid_task(summon);
        self.stop_follow(summon);
    }

    /// Java `Summon.cancelAction()` (the stop button): active again unless it cannot move.
    pub fn summon_cancel_action(&mut self, summon: ObjectId) {
        if !self.is_movement_disabled(summon) {
            self.set_intention_active(summon);
        }
    }

    /// Java `Summon.doSummonAttack(target)`.
    pub fn do_summon_attack(&mut self, summon: ObjectId, target: Option<ObjectId>) {
        let Some(target) = target else {
            return;
        };
        if self.summon_owner(summon).is_none() {
            return;
        }
        self.set_creature_target(summon, Some(target));
        self.set_intention_attack(summon, target);
        // Fake players (`FAKE_PLAYER_AUTO_ATTACKABLE`) are not ported.
    }

}
