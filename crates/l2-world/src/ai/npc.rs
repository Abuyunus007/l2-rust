//! The NPC side of the intention machine: `CreatureAI` as the NPCs use it, and the hooks that lead
//! to `AttackableAI` ([`super::attackable`]).
// Java: ai/CreatureAI.java (setIntentionIdle, setIntentionActive, notifyActionThink)

use l2_util::ObjectId;

use crate::movement::Intention;
use crate::world::World;

impl World {
    /// Java `getAI().notifyActionThink()` of an NPC: `AttackableAI`'s for an attackable,
    /// `CreatureAI`'s (which does nothing) for any other NPC.
    pub(crate) fn npc_notify_think(&mut self, id: ObjectId) {
        if self.is_attackable_npc(id) {
            self.attackable_notify_think(id);
        }
    }

    /// Java `CreatureAI.setIntentionFollow(target)` of an NPC (`AttackableAI` starts its task
    /// after), for the scripts (S-BOSS-01).
    pub fn npc_set_intention_follow(&mut self, id: ObjectId, target: ObjectId) {
        if !self.is_npc(id) {
            return;
        }
        self.wake_ai(id);
        let blocked = !self.objects.get(&target).is_some_and(|e| e.kind.is_creature())
            || self.intention_of(id) == Intention::Rest
            || self.is_all_skills_disabled(id)
            || self.is_casting_now(id)
            || self.is_movement_disabled(id)
            || self.is_dead(id)
            || id == target;
        if blocked {
            self.client_action_failed(id);
        } else {
            // Stop the actor auto-attack client side by sending Server->Client packet
            // AutoAttackStop (broadcast).
            self.client_stop_auto_attack(id);
            // Set the Intention of this AbstractAI to FOLLOW.
            self.set_intention_value(id, Intention::Follow);
            // Create and Launch an AI Follow Task to execute every 1s.
            self.start_follow(id, target, crate::interact::NORMAL_FOLLOW_RANGE);
        }
        if self.is_attackable_npc(id) {
            self.start_ai_task(id);
        }
    }

    /// Java `getAI().setIntentionIdle()` of an NPC (players: `World::set_intention_idle`).
    pub fn npc_set_intention_idle(&mut self, id: ObjectId) {
        // `SummonAI.setIntentionIdle()`.
        if self.is_summon(id) {
            self.summon_set_intention_idle(id);
            return;
        }
        if !self.is_npc(id) {
            return;
        }
        // `getAI()` makes the AI.
        self.wake_ai(id);
        if self.is_attackable_npc(id) {
            self.attackable_set_intention_idle(id);
        } else {
            self.creature_set_intention_idle(id);
        }
    }

    /// Java `CreatureAI.setIntentionIdle()`: stop everything, forget the targets.
    pub(crate) fn creature_set_intention_idle(&mut self, id: ObjectId) {
        self.stop_follow(id);
        // Set the AI Intention to IDLE.
        self.set_intention_value(id, Intention::Idle);
        // Init cast and attack target.
        self.set_attack_target(id, None);
        // A new intention supersedes a move that still waits for its path.
        self.clear_pending_path(id);
        // Stop the actor movement server side AND client side by sending Server->Client packet
        // StopMove/StopRotation (broadcast).
        self.client_stop_moving(id, None);
        // Stop the actor auto-attack client side by sending Server->Client packet AutoAttackStop
        // (broadcast).
        self.client_stop_auto_attack(id);
    }

    /// Java `CreatureAI.setIntentionActive()`.
    pub(crate) fn creature_set_intention_active(&mut self, id: ObjectId) {
        self.stop_follow(id);
        // Check if the Intention is not already Active.
        if self.intention_of(id) == Intention::Active {
            return;
        }
        // Set the AI Intention to ACTIVE.
        self.set_intention_value(id, Intention::Active);
        // Check if region and its neighbors are active.
        if !self.are_region_neighbors_active(id) {
            return;
        }
        // Init cast and attack target.
        self.set_attack_target(id, None);
        // Stop the actor movement server side AND client side by sending Server->Client packet
        // StopMove/StopRotation (broadcast).
        self.clear_pending_path(id);
        self.client_stop_moving(id, None);
        // Stop the actor auto-attack client side by sending Server->Client packet AutoAttackStop
        // (broadcast).
        self.client_stop_auto_attack(id);
        // Launch the Think Action.
        self.notify_action_think(id);
    }
}
