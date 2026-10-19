//! Walking up to something that was clicked: the `INTERACT` intention of a player, `moveToPawn`,
//! and the follow task that walks the last steps (also the chase of the `ATTACK` intention, see
//! `ai/`).
// Java: ai/CreatureAI.java (setIntentionInteract, maybeMoveToPawn, notifyActionForgetObject)
// Java: ai/PlayerAI.java (thinkInteract, notifyActionThink)
// Java: ai/AbstractAI.java (moveToPawn, startFollow, stopFollow, clientStoppedMoving)
// Java: taskmanagers/CreatureFollowTaskManager.java (addAttackFollow, follow)
// Java: entity/actor/Player.java (doInteract)
//
// # The flow of a talk click from afar
// `NpcClick` sees that the NPC is too far (`canInteract`), so `setIntentionInteract(npc)` walks
// towards it (`moveToPawn`, offset 60). When the player arrives, `thinkInteract` checks the
// distance again; if the player is still outside `36 + both collision radii` it starts the follow
// task (a move to the NPC every 500 ms until it is inside), and once close enough
// `doInteract` clicks the NPC again, which opens the dialog.
//
// # Left out
// - Following another player (`setIntentionFollow`) and the other intentions (cast, pick up):
//   M4/M5. The attack intention is in `ai/` (M4-COMBAT-01).

use l2_protocol_game::server::{ActionFailed, MoveToPawn, StopMove};
use l2_util::ObjectId;

use crate::movement::{AfterMove, Intention, MoveStep};
use crate::objects::ObjectKind;
use crate::target::{distance_2d, distance_3d};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `Interact` offsets of Java: the distance `setIntentionInteract` walks to, and the one
/// `thinkInteract` accepts.
const INTERACT_MOVE_OFFSET: i32 = 60;
const INTERACT_THINK_OFFSET: i32 = 36;
/// `CreatureFollowTaskManager`'s attack-follow task runs every 500 ms.
const FOLLOW_INTERVAL_MS: u64 = 500;
/// `1000 / GameTimeTaskManager.MILLIS_IN_TICK`: the pawn move timeout, in game ticks.
const MOVE_TO_PAWN_TIMEOUT_TICKS: i32 = 10;

/// One running follow task (Java `CreatureFollowTaskManager.ATTACK_FOLLOW_CREATURES` and
/// `AbstractAI._followTarget`).
#[derive(Debug, Clone, PartialEq)]
pub struct Follow {
    pub target: ObjectId,
    pub range: i32,
    timer: TimerId,
}

/// The AI fields of a player that the pawn moves use (Java `AbstractAI`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PawnState {
    /// `_target`: what the AI was told to go to.
    pub ai_target: Option<ObjectId>,
    /// `_clientMovingToPawnOffset`: non-zero while the client was sent a `MoveToPawn`.
    pub client_moving_to_pawn_offset: i32,
    /// `_moveToPawnTimeout`, in game ticks.
    pub move_to_pawn_timeout: i32,
    pub follow: Option<Follow>,
    /// `PlayerAI._thinking`: `notifyActionThink` does not run inside itself.
    thinking: bool,
}

impl World {
    /// Java `PlayerAI.notifyActionThink`: the attack and interact intentions think (cast and
    /// pick up arrive with their systems).
    pub(crate) fn player_notify_action_think(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if player.movement.pawn.thinking {
            return;
        }
        player.movement.pawn.thinking = true;
        match player.movement.intention {
            Intention::Attack => self.think_attack(id),
            Intention::Interact => self.think_interact(id),
            Intention::PickUp => self.think_pick_up(id),
            _ => {}
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.movement.pawn.thinking = false;
        }
    }

    /// Java `CreatureAI.setIntentionInteract(object)` for a player.
    pub(crate) fn set_intention_interact(&mut self, id: ObjectId, object: ObjectId) {
        self.stop_follow(id);
        let Some(player) = self.players.get(&id) else {
            return;
        };
        // FOLLOW-UP(M4): `isAllSkillsDisabled()` and `isCastingNow()` also cancel it.
        if player.movement.intention == Intention::Rest {
            // Cancel action client side by sending Server->Client packet ActionFailed.
            self.send(id, &ActionFailed);
            return;
        }
        // Stop the actor auto-attack client side (a player keeps its stance until it ends).
        self.client_stop_auto_attack(id);
        if self
            .players
            .get(&id)
            .is_some_and(|p| p.movement.intention == Intention::Interact)
        {
            return;
        }
        // Set the Intention of this AbstractAI to INTERACT.
        if let Some(player) = self.players.get_mut(&id) {
            player.movement.intention = Intention::Interact;
            // Set the AI interact target.
            player.movement.pawn.ai_target = Some(object);
        }
        // Move the actor to Pawn server side AND client side.
        self.move_to_pawn(id, object, INTERACT_MOVE_OFFSET);
    }

    /// Java `AbstractAI.moveToPawn(pawn, offsetValue)`.
    pub(crate) fn move_to_pawn(&mut self, id: ObjectId, pawn: ObjectId, offset_value: i32) {
        // Check if actor can move. (`isCastingNow()`: M5.)
        if self.is_movement_disabled(id) || self.is_attacking_now(id) {
            self.send(id, &ActionFailed);
            return;
        }
        let offset = offset_value.max(10);

        // Prevent possible extra calls to this function, also don't send movetopawn packets too
        // often.
        let game_time = self.game_ticks();
        let moving = self.is_moving(id);
        let on_geodata_path = self.mv_of(id).is_some_and(|mv| mv.is_on_geodata_path());
        let Some(state) = self.pawn_of(id).cloned() else {
            return;
        };
        if moving && state.ai_target == Some(pawn) {
            if state.client_moving_to_pawn_offset == offset {
                if game_time < state.move_to_pawn_timeout {
                    return;
                }
            }
            // Minimum time to calculate new route is 2 seconds.
            else if on_geodata_path && game_time < state.move_to_pawn_timeout + 10 {
                return;
            }
        }

        // Set AI movement data.
        if let Some(state) = self.pawn_mut(id) {
            state.client_moving_to_pawn_offset = offset;
            state.ai_target = Some(pawn);
            state.move_to_pawn_timeout = game_time + MOVE_TO_PAWN_TIMEOUT_TICKS;
        }
        let Some((px, py, pz)) = self.position_of(pawn) else {
            return;
        };

        // Calculate movement data for a move to location action and add the actor to the moving
        // objects.
        if self.move_to_location(id, px, py, pz, offset) == MoveStep::WaitingForPath {
            // The packet goes out when the path is there (`on_path_found`). A player's follows
            // from its intention, an NPC's is told here.
            if !self.is_player(id) {
                self.set_after_move(id, AfterMove::ToPawn(pawn));
            }
            return;
        }
        self.broadcast_pawn_move(id, pawn);
    }

    /// The end of `AbstractAI.moveToPawn`: tell the client where the player goes.
    pub(crate) fn broadcast_pawn_move(&mut self, id: ObjectId, pawn: ObjectId) {
        let creature = self.objects.get(&pawn).is_some_and(|e| e.kind.is_creature());
        if creature {
            let on_geodata_path = self.mv_of(id).is_some_and(|mv| mv.is_on_geodata_path());
            if on_geodata_path {
                self.broadcast_move_to_location(id, false);
                if let Some(state) = self.pawn_mut(id) {
                    state.client_moving_to_pawn_offset = 0;
                }
            } else if self.object_region_active(id) {
                let offset = self.pawn_of(id).map_or(0, |p| p.client_moving_to_pawn_offset);
                if let (Some(me), Some(target)) = (self.position_of(id), self.position_of(pawn)) {
                    let packet = MoveToPawn {
                        object_id: id.0,
                        target_id: pawn.0,
                        distance: offset,
                        x: me.0,
                        y: me.1,
                        z: me.2,
                        target_x: target.0,
                        target_y: target.1,
                        target_z: target.2,
                    };
                    self.broadcast_creature_packet(id, &packet);
                }
            }
        } else {
            self.broadcast_move_to_location(id, false);
        }
    }

    fn object_region_active(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .and_then(|e| e.region)
            .is_some_and(|r| self.is_region_active(r))
    }

    /// Java `AbstractAI.clientStoppedMoving`: a client that was sent a `MoveToPawn` is told to
    /// stop.
    pub(crate) fn client_stopped_moving(&mut self, id: ObjectId) {
        let Some(state) = self.pawn_mut(id) else {
            return;
        };
        if state.client_moving_to_pawn_offset > 0 {
            // movetoPawn needs to be stopped.
            state.client_moving_to_pawn_offset = 0;
            let Some(core) = self.core_of(id) else {
                return;
            };
            let packet = StopMove {
                object_id: id.0,
                x: core.x,
                y: core.y,
                z: core.z,
                heading: core.heading,
            };
            self.broadcast_creature_packet(id, &packet);
        }
    }

    /// Java `PlayerAI.thinkInteract`.
    pub(crate) fn think_interact(&mut self, id: ObjectId) {
        // FOLLOW-UP(M4): `isAllSkillsDisabled() || isCastingNow()` returns here.
        let target = self.players.get(&id).and_then(|p| p.movement.pawn.ai_target);
        // `checkTargetLost`: the target is gone.
        let Some(target) = target.filter(|t| self.objects.contains_key(t)) else {
            self.set_intention_idle(id);
            return;
        };
        if self.maybe_move_to_pawn(id, target, INTERACT_THINK_OFFSET) {
            return;
        }
        // Nothing happens when the target is a sign: the player has to click it again.
        if self.objects.get(&target).map(|e| e.kind) != Some(ObjectKind::StaticObject) {
            self.do_interact(id, target);
        }
        self.set_intention_idle(id);
    }

    /// Java `Player.doInteract(target)` for what is not a player: the same as a click.
    fn do_interact(&mut self, id: ObjectId, target: ObjectId) {
        match self.objects.get(&target).map(|e| e.kind) {
            Some(ObjectKind::Player) => {
                // FOLLOW-UP(M5): private store lists.
                self.send(id, &ActionFailed);
            }
            Some(_) => self.on_action_object(id, target, true),
            None => {}
        }
    }

    /// Java `CreatureAI.maybeMoveToPawn(target, offsetValue)` for a player.
    pub(crate) fn maybe_move_to_pawn(&mut self, id: ObjectId, target: ObjectId, offset_value: i32) -> bool {
        if offset_value < 0 {
            // skill radius -1
            return false;
        }
        let (Some(me), Some(pos)) = (self.position_of(id), self.position_of(target)) else {
            return false;
        };
        let my_radius = self.stats(id).map_or(0, |s| s.collision_radius() as i32);
        let mut offset_with_collision = offset_value + my_radius;
        let creature = self.objects.get(&target).is_some_and(|e| e.kind.is_creature());
        if creature {
            offset_with_collision += self.collision_radius_of(target);
        }

        if distance_2d(me, pos) >= f64::from(offset_with_collision) {
            // Caller should be Playable and thinkAttack/thinkCast/thinkInteract/thinkPickUp.
            if self.is_following(id) {
                // Allow larger hit range when the target is moving (check is run only once per
                // second).
                if distance_2d(me, pos) >= f64::from(offset_with_collision + 100) {
                    return true;
                }
                self.stop_follow(id);
                return false;
            }

            if self.is_movement_disabled(id) || self.move_speed(id) <= 0.0 {
                // If player is trying attack target but he cannot move to attack target.
                // Change his intention to idle.
                if self.intention_of(id) == Intention::Attack {
                    self.set_intention_idle(id);
                }
                return true;
            }

            self.stop_follow(id);
            let mut offset = offset_value;
            let is_door = self.objects.get(&target).map(|e| e.kind) == Some(ObjectKind::Door);
            if creature && !is_door {
                if self.is_moving(target) {
                    offset -= 100;
                }
                if offset < 5 {
                    offset = 5;
                }
                self.start_follow(id, target, offset);
            } else {
                // Move the actor to Pawn server side AND client side.
                self.move_to_pawn(id, target, offset);
            }
            return true;
        }

        if self.is_following(id) {
            self.stop_follow(id);
        }
        false
    }

    /// `getTemplate().getCollisionRadius()` of a creature that is not the player.
    pub(crate) fn collision_radius_of(&self, id: ObjectId) -> i32 {
        match self.objects.get(&id).map(|e| e.kind) {
            Some(ObjectKind::Player) => self.stats(id).map_or(0, |s| s.collision_radius() as i32),
            Some(ObjectKind::Npc) => self
                .npcs
                .get(&id)
                .and_then(|n| self.npc_world.template(n.npc_id))
                .map_or(0, |t| t.collision_radius_int()),
            _ => 0,
        }
    }

    // ------------------------------------------------------------------ follow

    /// Java `AbstractAI.isFollowing()`.
    pub(crate) fn is_following(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.movement.pawn.follow.is_some())
    }

    /// Java `AbstractAI.startFollow(target, range)` with a range (the attack-follow task). The
    /// task runs once at once, then every 500 ms.
    fn start_follow(&mut self, id: ObjectId, target: ObjectId, range: i32) {
        self.stop_follow(id);
        let timer = self.schedule_follow(id);
        if let Some(p) = self.players.get_mut(&id) {
            p.movement.pawn.follow = Some(Follow { target, range, timer });
        }
        self.follow(id);
    }

    fn schedule_follow(&mut self, id: ObjectId) -> TimerId {
        let ticks = TimerWheel::ms_to_ticks(FOLLOW_INTERVAL_MS);
        let due = self.tick + ticks;
        self.timers.schedule_at(due, TimerKind::Follow(id))
    }

    /// Java `AbstractAI.stopFollow()`.
    pub(crate) fn stop_follow(&mut self, id: ObjectId) {
        let follow = self
            .players
            .get_mut(&id)
            .and_then(|p| p.movement.pawn.follow.take());
        if let Some(follow) = follow {
            self.timers.cancel(follow.timer);
        }
    }

    /// The timer of the follow task.
    pub(crate) fn on_follow_timer(&mut self, id: ObjectId) {
        if !self.is_following(id) {
            return;
        }
        let timer = self.schedule_follow(id);
        if let Some(follow) = self
            .players
            .get_mut(&id)
            .and_then(|p| p.movement.pawn.follow.as_mut())
        {
            follow.timer = timer;
        }
        self.follow(id);
    }

    /// Java `CreatureFollowTaskManager.follow(creature, range)`.
    fn follow(&mut self, id: ObjectId) {
        let Some(follow) = self.players.get(&id).and_then(|p| p.movement.pawn.follow.clone()) else {
            // The follow target is gone: `ai.setIntentionIdle()`.
            self.set_intention_idle(id);
            return;
        };
        let (Some(me), Some(pos)) = (self.position_of(id), self.position_of(follow.target)) else {
            self.set_intention_idle(id);
            return;
        };
        let distance = distance_3d(me, pos);
        if distance >= f64::from(follow.range) {
            if distance >= 3000.0 {
                // If the target is too far (maybe also teleported).
                self.set_intention_idle(id);
                return;
            }
            self.move_to_pawn(id, follow.target, follow.range);
            // GeoEngine can reject or truncate a route without ever raising ARRIVED. Revalidate
            // rather than leave the action waiting on an event that never comes.
            if !self.is_moving(id) && self.intention_of(id) == Intention::Attack {
                self.notify_action_arrived(id);
            }
        } else if !self.is_moving(id) && self.intention_of(id) == Intention::Attack {
            // This task can observe arrival before the movement manager does.
            self.notify_action_arrived(id);
        }
    }

    /// Java `CreatureAI.notifyActionForgetObject(object)` for a player, as far as clicks go.
    pub(crate) fn notify_action_forget_object(&mut self, id: ObjectId, object: ObjectId) {
        let teleporting = self.is_teleporting(id);
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        if !(spawned || teleporting) {
            return;
        }
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        // If the object was targeted and the Intention was INTERACT, set the Intention to ACTIVE.
        let was_target = p.movement.pawn.ai_target == Some(object);
        let interacting = matches!(p.movement.intention, Intention::Interact | Intention::PickUp);
        if was_target {
            p.movement.pawn.ai_target = None;
            if interacting {
                self.set_intention_idle(id);
            }
        }
        // Check if the object was targeted to attack.
        if self.attack_target_of(id) == Some(object) {
            // Cancel attack target.
            self.set_attack_target(id, None);
            // Set the Intention of this AbstractAI to ACTIVE (not for a corpse: fixes stop move
            // from cast target decay).
            if !self.is_alike_dead(object) {
                self.set_intention_active(id);
            }
        }
        // Check if the object was targeted to follow.
        let following = self
            .players
            .get(&id)
            .and_then(|p| p.movement.pawn.follow.as_ref())
            .is_some_and(|f| f.target == object);
        if following {
            self.client_stop_moving(id, None);
            self.stop_follow(id);
            self.set_intention_idle(id);
        }
    }
}
