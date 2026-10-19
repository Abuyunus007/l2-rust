//! The NPC side of the intention machine: the hooks M4-AI-01 fills with `AttackableAI`.
// Java: ai/CreatureAI.java (notifyActionThink, notifyActionAttacked: what an NPC without its own
//       AI does), ai/AttackableAI.java (notifyActionAttacked: the part kept here; the rest is
//       M4-AI-01)

use l2_util::ObjectId;

use crate::movement::Intention;
use crate::world::World;

impl World {
    /// Java `getAI().notifyActionThink()` of an NPC.
    // M4-AI-01: `AttackableAI.notifyActionThink` (`onEvtThink`: `thinkActive` / `thinkAttack`, the
    // chase and `doAttack`). `CreatureAI.notifyActionThink` does nothing, which is all an NPC does
    // until then. An NPC that attacked (`World::do_attack`) is told
    // `notify_action_ready_to_act` when its attack ends, which lands here.
    pub(crate) fn npc_notify_think(&mut self, id: ObjectId) {
        let _ = id;
    }

    /// Java `getAI().notifyActionAttacked(attacker)` of an NPC.
    ///
    /// For an attackable NPC this keeps the two lines of `AttackableAI.notifyActionAttacked` that
    /// make its state consistent with Java's (the attacker in the aggro list, the attack
    /// intention), so `addDamageHate` sees the same intention as in Java. M4-AI-01 replaces the
    /// attackable branch with the whole method: the attack timeout, `_globalAggro`, running,
    /// `getMostHated() != getAttackTarget()`, `isCoreAIDisabled()`, minions and the faction call.
    pub(crate) fn npc_notify_attacked(&mut self, id: ObjectId, attacker: ObjectId) {
        if self.is_attackable_npc(id) && self.objects.contains_key(&attacker) {
            // Add the attacker to the _aggroList of the actor if not present.
            if !self.aggro_list(id).iter().any(|a| a.attacker == attacker) {
                self.add_damage_hate(id, attacker, 0, 1);
            }
            // Set the Intention to ATTACK.
            if self.intention_of(id) != Intention::Attack {
                self.set_intention_attack(id, attacker);
            }
        }
        // CreatureAI.notifyActionAttacked
        if !self.can_handle_action(id) {
            return;
        }
        self.client_start_auto_attack(id);
    }
}
