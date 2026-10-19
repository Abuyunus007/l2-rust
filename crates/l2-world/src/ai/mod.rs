//! The intention machine: Java's `AbstractAI` / `CreatureAI` events and intentions, shared by
//! players and NPCs, as far as auto attack needs them.
// Java: ai/{AbstractAI,CreatureAI,PlayerAI,PlayableAI,Intention,NextAction}.java (the attack
//       parts: setIntentionAttack/Active/Idle, notifyActionReadyToAct, notifyActionAttacked,
//       notifyActionEvaded, notifyActionCancel, notifyActionDeath, clientStartAutoAttack,
//       clientStopAutoAttack, setAutoAttacking, checkTargetLostOrDead)
//
// # Where the state lives
// - A player's intention is `Player::movement.intention` (the move code of M3 already keeps it
//   there), its move and follow fields `movement.pawn`.
// - Everything else of an AI is an [`AiState`] in the world's AI table, by object id (D-11): the
//   attack target, Java's `_clientAutoAttacking`, PlayerAI's saved move, and an NPC's intention.
//   An NPC gets its entry when Java's `getAI()` would create its AI (a click on an attackable
//   NPC, the first damage); `has_ai` is Java's `hasAI()`.
//
// # How an NPC AI plugs in (M4-AI-01)
// - [`attackable`] is `AttackableAI` (think loop, aggro list, minions, faction call, region
//   switch); [`npc`] is the `CreatureAI` part the NPCs share (`ACTIVE` / `IDLE`, the hooks the
//   intention machine calls). The AI fields are in [`AiState`]; an attackable's think state and
//   the think set are [`attackable::AttackableWorld`].
// - NPC movement is `movement.rs` (the code players use, with the NPC branches of Java's
//   `Creature.moveToLocation` / `updatePosition`); the NPC's move fields are
//   `Npc::movement`.
//
// # Left out
// - Skills in the NPC AI (`AISkillScope`), scripts reacting to AI events: M5 / D-05, see
//   `attackable.rs`.
// - `NextAction` (callbacks run on AI events): only the client packets of skills and pick up set
//   one (M5); nothing in M4 does.
// - Cast, follow, pick up intentions (M5 / M4 ground items).

pub mod attackable;
pub mod npc;
pub mod player;

use std::collections::HashMap;

use l2_protocol_game::server::{ActionFailed, AutoAttackStart, AutoAttackStop};
use l2_util::ObjectId;

use crate::movement::Intention;
use crate::world::World;

/// The AI fields of one creature (Java `AbstractAI`, `PlayerAI` and `AttackableAI`).
#[derive(Debug, Clone, PartialEq)]
pub struct AiState {
    /// Java `_intention` of an NPC (a player's is in `MovementState::intention`).
    pub intention: Intention,
    /// Java `_attackTarget`.
    pub attack_target: Option<ObjectId>,
    /// Java `_clientAutoAttacking`: the client was told the creature fights (`AutoAttackStart`).
    pub client_auto_attacking: bool,
    /// `PlayerAI._savedIntention == MOVE_TO` and `_savedMoveTo`: a move asked during an attack,
    /// replayed when the attack is over.
    pub saved_move_to: Option<(i32, i32, i32)>,
    /// `AttackableAI._attackTimeout`, in game ticks (`i32::MAX` = none).
    pub attack_timeout: i32,
    /// `AttackableAI._globalAggro`.
    pub global_aggro: i32,
    /// `AttackableAI._thinking`: `notifyActionThink` does not run inside itself.
    pub(crate) thinking: bool,
    /// `AttackableAI._chaosTime`.
    pub chaos_time: i32,
    /// `AttackableAI._minionLeashTime`, in world ms (0 = not started).
    pub minion_leash_ms: i64,
}

impl Default for AiState {
    fn default() -> Self {
        Self {
            intention: Intention::Idle,
            attack_target: None,
            client_auto_attacking: false,
            saved_move_to: None,
            // `AttackableAI`'s constructor: `_attackTimeout = Integer.MAX_VALUE`.
            attack_timeout: i32::MAX,
            global_aggro: 0,
            thinking: false,
            chaos_time: 0,
            minion_leash_ms: 0,
        }
    }
}

/// The AI table of the world.
#[derive(Debug, Default)]
pub struct AiWorld {
    pub(crate) creatures: HashMap<ObjectId, AiState>,
    /// The think set, the minion lists and the settings of the attackable AI.
    pub(crate) attackable: attackable::AttackableWorld,
}

impl World {
    /// The AI state of a creature, if it has one.
    pub fn ai_state(&self, id: ObjectId) -> Option<&AiState> {
        self.ai.creatures.get(&id)
    }

    pub(crate) fn ai_state_mut(&mut self, id: ObjectId) -> &mut AiState {
        // `getAI()` makes an NPC's AI (an attackable's rolls its aggro timeout).
        self.wake_ai(id);
        self.ai.creatures.entry(id).or_default()
    }

    /// Java `getAI()` for an NPC: the AI exists from now on.
    pub(crate) fn wake_ai(&mut self, id: ObjectId) {
        if self.is_npc(id) && !self.ai.creatures.contains_key(&id) {
            let mut state = AiState::default();
            if self.is_attackable_npc(id) {
                // The `AttackableAI` constructor: `rollGlobalAggro()`.
                state.global_aggro = self.roll_global_aggro(id);
            }
            self.ai.creatures.insert(id, state);
        }
    }

    /// Java `hasAI()`: players always have one; an NPC once something woke it.
    pub fn has_ai(&self, id: ObjectId) -> bool {
        self.is_player(id) || (self.is_npc(id) && self.ai.creatures.contains_key(&id))
    }

    /// Java `getAI().getIntention()`.
    pub fn intention_of(&self, id: ObjectId) -> Intention {
        if let Some(p) = self.players.get(&id) {
            return p.movement.intention;
        }
        self.ai
            .creatures
            .get(&id)
            .map_or(Intention::Idle, |a| a.intention)
    }

    /// Sets `_intention` (no side effects).
    pub(crate) fn set_intention_value(&mut self, id: ObjectId, intention: Intention) {
        if let Some(p) = self.players.get_mut(&id) {
            p.movement.intention = intention;
        } else if self.is_npc(id) {
            self.ai_state_mut(id).intention = intention;
        }
    }

    /// Java `getAI().getAttackTarget()`.
    pub fn attack_target_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.ai.creatures.get(&id).and_then(|a| a.attack_target)
    }

    pub(crate) fn set_attack_target(&mut self, id: ObjectId, target: Option<ObjectId>) {
        if target.is_none() && !self.ai.creatures.contains_key(&id) {
            return;
        }
        self.ai_state_mut(id).attack_target = target;
    }

    /// Java `AbstractAI.isAutoAttacking()`.
    pub fn is_auto_attacking(&self, id: ObjectId) -> bool {
        self.ai
            .creatures
            .get(&id)
            .is_some_and(|a| a.client_auto_attacking)
    }

    /// Java `AbstractAI.setAutoAttacking(value)` (summons pass it to their owner: M5).
    pub(crate) fn set_auto_attacking(&mut self, id: ObjectId, value: bool) {
        if !value && !self.ai.creatures.contains_key(&id) {
            return;
        }
        self.ai_state_mut(id).client_auto_attacking = value;
    }

    /// Java `CreatureAI.canHandleAction()`: spawned (or teleporting) and has an AI.
    pub(crate) fn can_handle_action(&self, id: ObjectId) -> bool {
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        (spawned || self.is_teleporting(id)) && self.has_ai(id)
    }

    /// Java `AbstractAI.clientActionFailed()`: only a player is told.
    pub(crate) fn client_action_failed(&self, id: ObjectId) {
        if self.is_player(id) {
            self.send(id, &ActionFailed);
        }
    }

    // ------------------------------------------------------------------ intentions

    /// Java `setIntentionAttack(target)`: `AttackableAI`'s (the attack timeout, the AI task) around
    /// `CreatureAI`'s.
    pub fn set_intention_attack(&mut self, id: ObjectId, target: ObjectId) {
        let attackable = self.is_attackable_npc(id);
        if attackable {
            // Calculate the attack timeout.
            self.wake_ai(id);
            self.renew_attack_timeout(id);
        }
        self.creature_set_intention_attack(id, target);
        if attackable {
            self.start_ai_task(id);
        }
    }

    /// Java `CreatureAI.setIntentionAttack(target)` (with `PlayableAI`'s checks).
    fn creature_set_intention_attack(&mut self, id: ObjectId, target: ObjectId) {
        // `target.asCreature()`: fences and unknown ids are no creatures.
        if !self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
            self.client_action_failed(id);
            return;
        }
        // PlayableAI: newbie protection and cursed weapon checks between players (M5).
        // M5: `isAllSkillsDisabled()`, `isCastingNow()`, `isControlBlocked()`.
        if self.intention_of(id) == Intention::Rest {
            self.client_action_failed(id);
            return;
        }
        if self.intention_of(id) == Intention::Attack {
            // Check if the AI already targets the Creature.
            if self.attack_target_of(id) != Some(target) {
                // Set the AI attack target (change target).
                self.set_attack_target(id, Some(target));
            }
            // A repeated attack on the same target revalidates a move that ended without
            // arriving (Java's comment); the follow is restarted either way.
            self.stop_follow(id);
            self.notify_action_think(id);
        } else {
            self.set_intention_value(id, Intention::Attack);
            self.set_attack_target(id, Some(target));
            self.stop_follow(id);
            self.notify_action_think(id);
        }
    }

    /// Java `setIntentionActive()`: `PlayerAI` makes it idle, `CreatureAI` stops the attack and
    /// thinks.
    pub fn set_intention_active(&mut self, id: ObjectId) {
        if self.is_player(id) {
            self.set_intention_idle(id);
            return;
        }
        if !self.is_npc(id) {
            return;
        }
        // `getAI()` makes the AI.
        self.wake_ai(id);
        if self.is_attackable_npc(id) {
            self.attackable_set_intention_active(id);
        } else {
            self.creature_set_intention_active(id);
        }
    }

    // ------------------------------------------------------------------ events

    /// Java `notifyActionReadyToAct()`: the attack is over; `PlayerAI` first replays a move asked
    /// during it, then the AI thinks (attacks again).
    pub fn notify_action_ready_to_act(&mut self, id: ObjectId) {
        if self.is_player(id) {
            let saved = self
                .ai
                .creatures
                .get_mut(&id)
                .and_then(|a| a.saved_move_to.take());
            if let Some((x, y, z)) = saved {
                self.set_intention_move_to(id, x, y, z);
            }
        }
        // `AttackableAI.notifyActionReadyToAct` renews the attack timeout first.
        self.attackable_ready_to_act(id);
        if !self.can_handle_action(id) {
            return;
        }
        // M5: not while casting.
        self.notify_action_think(id);
    }

    /// Java `notifyActionAttacked(attacker)`.
    pub fn notify_action_attacked(&mut self, id: ObjectId, attacker: ObjectId) {
        // Java's callers use `getAI()`, which creates an NPC's AI.
        self.wake_ai(id);
        if self.is_npc(id) {
            self.npc_notify_attacked(id, attacker);
            return;
        }
        if !self.can_handle_action(id) {
            return;
        }
        self.client_start_auto_attack(id);
    }

    /// Java `notifyActionEvaded(attacker)`: nothing in `CreatureAI` (M4-AI-01 may react).
    pub fn notify_action_evaded(&mut self, id: ObjectId, attacker: ObjectId) {
        let _ = (id, attacker);
    }

    /// Java `notifyActionCancel()` (`PlayerAI` forgets the saved move first).
    pub fn notify_action_cancel(&mut self, id: ObjectId) {
        if let Some(ai) = self.ai.creatures.get_mut(&id) {
            ai.saved_move_to = None;
        }
        if !self.can_handle_action(id) {
            return;
        }
        // M5: `abortCast()`.
        self.stop_follow(id);
        if !self.has_attack_stance_task(id) {
            self.broadcast_creature_packet(id, &AutoAttackStop { target_id: id.0 });
        }
        self.notify_action_think(id);
    }

    /// Java `notifyActionDeath()` without the `Die` broadcast (M4-DEATH-01 sends it from
    /// `doDie`): the AI forgets everything and stops.
    pub fn notify_action_death(&mut self, id: ObjectId) {
        if !self.can_handle_action(id) {
            return;
        }
        // `stopAITask()` (an attackable leaves the think set).
        self.stop_ai_task(id);
        // `clientNotifyDead()`: (`Die` broadcast: M4-DEATH-01), then the AI is reset.
        if let Some(p) = self.players.get_mut(&id) {
            // PlayerAI.clientNotifyDead
            p.movement.pawn.client_moving_to_pawn_offset = 0;
            p.movement.pawn.ai_target = None;
        } else if let Some(pawn) = self.pawn_mut(id) {
            // AbstractAI.clientNotifyDead: `_target = null`.
            pawn.ai_target = None;
        }
        self.set_intention_value(id, Intention::Idle);
        self.set_attack_target(id, None);
        self.stop_follow(id);
    }

    /// Java `notifyActionThink()`: players think in `interact.rs` (`PlayerAI`), NPCs in [`npc`].
    pub fn notify_action_think(&mut self, id: ObjectId) {
        if self.is_player(id) {
            self.player_notify_action_think(id);
        } else if self.is_npc(id) {
            self.npc_notify_think(id);
        }
    }

    // ------------------------------------------------------------------ the client side

    /// Java `AbstractAI.clientStartAutoAttack()`: `AutoAttackStart` once per fight, and the
    /// combat stance refreshed every time.
    pub fn client_start_auto_attack(&mut self, id: ObjectId) {
        // Non attackable NPCs should not get in combat.
        // M4-AI-01: `isCoreAIDisabled()` (scripts) also keeps them out.
        if self.is_npc(id) && !self.is_attackable_npc(id) {
            return;
        }
        // Summons pass it to their owner (M5).
        if !self.is_auto_attacking(id) {
            // The player's summon gets its own `AutoAttackStart` here (M5).
            self.broadcast_creature_packet(id, &AutoAttackStart { target_id: id.0 });
            self.set_auto_attacking(id, true);
        }
        self.add_attack_stance_task(id);
    }

    /// Java `AbstractAI.clientStopAutoAttack()`: a player keeps its stance (the stance manager
    /// ends it), anybody else stops at once.
    pub fn client_stop_auto_attack(&mut self, id: ObjectId) {
        if self.is_player(id) {
            if !self.has_attack_stance_task(id) && self.is_auto_attacking(id) {
                self.add_attack_stance_task(id);
            }
        } else if self.is_auto_attacking(id) {
            self.broadcast_creature_packet(id, &AutoAttackStop { target_id: id.0 });
            self.set_auto_attacking(id, false);
        }
    }

    /// Java `CreatureAI.checkTargetLostOrDead(target)`: `true` (and the intention goes active)
    /// when the target is gone or dead.
    pub(crate) fn check_target_lost_or_dead(&mut self, id: ObjectId, target: Option<ObjectId>) -> bool {
        let lost = match target {
            None => true,
            Some(t) => !self.objects.contains_key(&t) || self.is_dead(t),
        };
        if lost {
            self.set_intention_active(id);
        }
        lost
    }
}
