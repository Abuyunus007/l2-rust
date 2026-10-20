//! The intention machine: Java's `AbstractAI` / `CreatureAI` events and intentions, shared by
//! players and NPCs, as far as auto attack needs them.
// Java: ai/{AbstractAI,CreatureAI,PlayerAI,PlayableAI,Intention,NextAction}.java (the attack
//       parts: setIntentionAttack/Active/Idle, notifyActionReadyToAct, notifyActionAttacked,
//       notifyActionEvaded, notifyActionCancel, notifyActionDeath, clientStartAutoAttack,
//       clientStopAutoAttack, setAutoAttacking, checkTargetLostOrDead; the cast parts of
//       M5-CAST-01: setIntentionCast, changeIntentionToCast, notifyActionFinishCasting and
//       PlayerAI's saved intention)
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
// - The follow intention of a player (M6 summons and party follow).

pub mod attackable;
pub mod idle;
pub mod npc;
pub mod player;

use std::collections::HashMap;

use l2_protocol_game::server::{ActionFailed, AutoAttackStart, AutoAttackStop};
use l2_util::ObjectId;

use crate::effects::{SkillKey, effect_flag};
use crate::movement::Intention;
use crate::skills::cast::CastTimer;
use crate::timer::{TimerKind, TimerWheel};
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
    /// `AbstractAI._skill`: the skill of the CAST intention (M5-CAST-01).
    pub cast_skill: Option<SkillKey>,
    /// `AbstractAI._castTarget`.
    pub cast_target: Option<ObjectId>,
    /// `PlayerAI._savedIntention`: the intention replayed after a cast (or an attack). A saved
    /// `MOVE_TO` keeps its point in `saved_move_to`.
    pub saved_intention: Option<Intention>,
    /// `PlayerAI._savedAttackTarget`, `_savedCastSkill` / `_savedCastTarget`,
    /// `_savedFollowTarget`.
    pub saved_attack_target: Option<ObjectId>,
    pub saved_cast: Option<(SkillKey, Option<ObjectId>)>,
    pub saved_follow_target: Option<ObjectId>,
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
            cast_skill: None,
            cast_target: None,
            saved_intention: None,
            saved_attack_target: None,
            saved_cast: None,
            saved_follow_target: None,
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
        if self.intention_of(id) == Intention::Rest
            || self.is_all_skills_disabled(id)
            || self.is_casting_now(id)
            // `isControlBlocked()`: afraid.
            || self.is_affected(id, effect_flag::FEAR)
        {
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
        // Replay any saved intention from before a CAST.
        if self.is_player(id) && self.next_intention(id).is_some() {
            self.replay_saved_intention(id);
        }
        // `AttackableAI.notifyActionReadyToAct` renews the attack timeout first.
        self.attackable_ready_to_act(id);
        if !self.can_handle_action(id) {
            return;
        }
        if !self.is_casting_now(id) && !self.is_casting_simultaneously_now(id) {
            // Launch actions corresponding to the Action Think.
            self.notify_action_think(id);
        }
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
        if self.is_player(id) {
            self.clear_saved_intention(id);
        }
        if !self.can_handle_action(id) {
            return;
        }
        self.abort_cast(id);
        self.stop_follow(id);
        if !self.has_attack_stance_task(id) {
            self.broadcast_creature_packet(id, &AutoAttackStop { target_id: id.0 });
        }
        self.notify_action_think(id);
    }

    /// Java `notifyActionDeath()`: the `Die` broadcast, then the AI forgets everything and stops.
    pub fn notify_action_death(&mut self, id: ObjectId) {
        if !self.can_handle_action(id) {
            return;
        }
        // `stopAITask()` (an attackable leaves the think set).
        self.stop_ai_task(id);
        // `_actor.asNpc().setDisplayEffect(0)`: `ExChangeNpcState` is not ported, and no NPC has
        // a display effect before scripts (D-05).
        if let Some(npc) = self.npcs.get_mut(&id) {
            npc.display_effect = 0;
        }
        // `clientNotifyDead()`: PlayerAI first forgets the offset of a move to a pawn, then the
        // `Die` packet goes to the actor (a player gets it first) and everybody who sees it,
        // then the AI is reset.
        if let Some(p) = self.players.get_mut(&id) {
            // PlayerAI.clientNotifyDead
            p.movement.pawn.client_moving_to_pawn_offset = 0;
        }
        let die = self.die_packet(id);
        self.broadcast_creature_packet(id, &die);
        if let Some(p) = self.players.get_mut(&id) {
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

// ---------------------------------------------------------------------- the CAST intention

impl World {
    /// Java `getAI().getCastTarget()`.
    pub fn cast_target_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.ai.creatures.get(&id).and_then(|a| a.cast_target)
    }

    /// Java `getAI()._skill`: the skill of the CAST intention.
    pub fn cast_skill_of(&self, id: ObjectId) -> Option<SkillKey> {
        self.ai.creatures.get(&id).and_then(|a| a.cast_skill)
    }

    /// Java `AbstractAI.setCastTarget(target)`.
    pub(crate) fn set_cast_target(&mut self, id: ObjectId, target: Option<ObjectId>) {
        if target.is_none() && !self.ai.creatures.contains_key(&id) {
            return;
        }
        self.ai_state_mut(id).cast_target = target;
    }

    /// Java `getAI().getNextIntention()`: `PlayerAI`'s saved intention (none for other AIs).
    pub fn next_intention(&self, id: ObjectId) -> Option<Intention> {
        if !self.is_player(id) {
            return None;
        }
        self.ai.creatures.get(&id).and_then(|a| a.saved_intention)
    }

    /// Java `PlayerAI.clearSavedIntention()`.
    pub(crate) fn clear_saved_intention(&mut self, id: ObjectId) {
        if let Some(ai) = self.ai.creatures.get_mut(&id) {
            ai.saved_intention = None;
            ai.saved_attack_target = None;
            ai.saved_move_to = None;
            ai.saved_follow_target = None;
            ai.saved_cast = None;
        }
    }

    /// `PlayerAI.setIntentionMoveTo` while attacking or casting: "Save the move-to as the next
    /// intention to replay once ready."
    pub(crate) fn save_move_to_intention(&mut self, id: ObjectId, loc: (i32, i32, i32)) {
        let ai = self.ai_state_mut(id);
        ai.saved_intention = Some(Intention::MoveTo);
        ai.saved_move_to = Some(loc);
    }

    /// Java `PlayerAI.saveCurrentIntentionForCast()`.
    fn save_current_intention_for_cast(&mut self, id: ObjectId) {
        let intention = self.intention_of(id);
        let follow = self
            .players
            .get(&id)
            .and_then(|p| p.movement.pawn.follow.as_ref().map(|f| f.target));
        let ai = self.ai_state_mut(id);
        ai.saved_intention = Some(intention);
        ai.saved_attack_target = ai.attack_target;
        ai.saved_cast = ai.cast_skill.map(|skill| (skill, ai.cast_target));
        // Other typed targets are restored on replay via the same getters when possible.
        ai.saved_move_to = None;
        ai.saved_follow_target = follow;
    }

    /// Java `PlayerAI.replaySavedIntention()`.
    pub(crate) fn replay_saved_intention(&mut self, id: ObjectId) {
        let Some(ai) = self.ai.creatures.get(&id) else {
            return;
        };
        let Some(intention) = ai.saved_intention else {
            return;
        };
        let (attack_target, move_to, follow_target, cast) = (
            ai.saved_attack_target,
            ai.saved_move_to,
            ai.saved_follow_target,
            ai.saved_cast,
        );
        self.clear_saved_intention(id);
        match intention {
            Intention::Idle | Intention::Active => self.set_intention_idle(id),
            // Resting (sitting) is not modelled, so it is never saved.
            Intention::Rest => {}
            Intention::Attack => match attack_target {
                Some(target) => self.set_intention_attack(id, target),
                // `setIntentionAttack(null)`.
                None => self.client_action_failed(id),
            },
            Intention::Cast => {
                if let Some((skill, target)) = cast {
                    self.set_intention_cast(id, skill, target);
                }
            }
            Intention::MoveTo => {
                if let Some((x, y, z)) = move_to {
                    self.set_intention_move_to(id, x, y, z);
                }
            }
            Intention::Follow => {
                // M6: `setIntentionFollow(target)`; a player never follows yet, so only the
                // refusal of a null target is reachable.
                if follow_target.is_none() {
                    self.client_action_failed(id);
                }
            }
            // The pick up and interact targets are never saved: `setIntentionPickUp(null)` and
            // `setIntentionInteract(null)` stop the follow and refuse.
            Intention::PickUp | Intention::Interact => {
                self.stop_follow(id);
                self.client_action_failed(id);
            }
        }
    }

    /// Java `setIntentionCast(skill, target)`: `PlayerAI` remembers what the player did before
    /// (not for toggles), then `CreatureAI` waits for a running attack and changes the intention.
    pub fn set_intention_cast(&mut self, id: ObjectId, skill: SkillKey, target: Option<ObjectId>) {
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(template) = table.get(skill.id, skill.level) else {
            return;
        };
        // Forget next if it's not cast or it's cast and skill is toggle.
        if self.is_player(id) && !template.is_toggle() && self.intention_of(id) != Intention::Cast {
            // New non-toggle cast: remember the current intention.
            self.save_current_intention_for_cast(id);
        }
        // CreatureAI.setIntentionCast
        self.stop_follow(id);
        if self.intention_of(id) == Intention::Rest && template.is_magic() {
            self.client_action_failed(id);
            self.set_casting_now(id, false);
            return;
        }
        // Wait for the current attack to finish, so its pending hit does not land during the cast.
        let game_time = self.game_ticks();
        let now = self.world_ms();
        let (bow_end, attack_end) = self
            .combat_state(id)
            .map_or((0, 0), |c| (c.disable_bow_attack_end_time, c.attack_end_ms));
        let delay_ms = if bow_end > game_time {
            // `GameTimeTaskManager.MILLIS_IN_TICK`.
            i64::from(bow_end - game_time) * 100
        } else if attack_end > now {
            attack_end - now
        } else {
            0
        };
        if delay_ms > 0 {
            let ticks = TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
            self.timers.schedule_at(
                self.tick + ticks,
                TimerKind::Cast(CastTimer::ChangeIntentionToCast {
                    caster: id,
                    skill,
                    target,
                }),
            );
        } else {
            self.change_intention_to_cast(id, skill, target);
        }
    }

    /// Java `CreatureAI.changeIntentionToCast(skill, target)`.
    pub(crate) fn change_intention_to_cast(&mut self, id: ObjectId, skill: SkillKey, target: Option<ObjectId>) {
        // Set the AI cast target (`target.asCreature()`).
        let target = target.filter(|t| self.objects.get(t).is_some_and(|e| e.kind.is_creature()));
        let ai = self.ai_state_mut(id);
        ai.cast_target = target;
        // Set the AI skill used by INTENTION_CAST.
        ai.cast_skill = Some(skill);
        // Change the Intention of this AbstractAI to CAST.
        self.set_intention_value(id, Intention::Cast);
        // Launch the Think Action.
        self.notify_action_think(id);
    }

    /// Java `notifyActionFinishCasting()`: `PlayerAI` goes back to what the player did before
    /// the cast (or idle). `NextAction` callbacks are not ported.
    pub fn notify_action_finish_casting(&mut self, id: ObjectId) {
        if self.is_player(id) && self.intention_of(id) == Intention::Cast {
            // Run interrupted or next intention.
            match self.next_intention(id) {
                Some(Intention::Cast) => {
                    self.clear_saved_intention(id);
                    self.set_intention_idle(id);
                }
                Some(_) => self.replay_saved_intention(id),
                // Set intention to idle if skill doesn't change intention.
                None => self.set_intention_idle(id),
            }
        }
        // CreatureAI: `canHandleAction()` and the FINISH_CASTING `NextAction` (not ported).
    }
}
