//! Targets and clicks: `Action`, `RequestTargetCanceld`, `Player.setTarget` and what a click on an
//! NPC, a player, a door or a sign does.
// Java: network/clientpackets/{Action,RequestTargetCanceld}.java
// Java: entity/actor/Player.java (setTarget), entity/actor/Creature.java (setTarget)
// Java: entity/WorldObject.java (onAction, onActionShift)
// Java: dist/game/data/scripts/handlers/actions/click/{NpcClick,PlayerClick,DoorClick,
//       StaticObjectClick}.java
// Java: entity/World.java (removeVisibleObject, switchRegion: the targets they clear)
//
// # What is left out, with the system that brings it
// - Flood protection (`canPerformPlayerAction`), observer mode, the bot-penalty effect, a pending
//   trade/party request (`getActiveRequester`), spawn protection (`onActionRequest`): none of them
//   exist yet.
// - Following (`setIntentionFollow`): M5. A second click on another player that cannot be
//   attacked only keeps the target. Attacking (`setIntentionAttack`) is `ai/` and `combat/`
//   (M4-COMBAT-01); doors are attacked in sieges (M5).
// - The GM shift click (`NpcShiftClick`, `npcinfo.htm`) and `AltGameViewNpc`: the GM gets the
//   target and `ActionFailed`.
// - Locked targets (aggression effects), `isOutOfControl`, vehicles, parties: M4/M5.

use l2_protocol_game::server::{
    ActionFailed, MoveToPawn, MyTargetSelected, NpcHtmlMessage, ShowTownMap, SocialAction, TargetSelected,
    TargetUnselected, ValidateLocation,
};
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::dialog::INTERACTION_DISTANCE;
use crate::objects::ObjectKind;
use crate::world::World;

/// Java `calculateDistance2D`: `sqrt(dx² + dy²)` with the differences taken in `int` first.
pub(crate) fn distance_2d(a: (i32, i32, i32), b: (i32, i32, i32)) -> f64 {
    let dx = f64::from(a.0.wrapping_sub(b.0));
    let dy = f64::from(a.1.wrapping_sub(b.1));
    (dx * dx + dy * dy).sqrt()
}

/// Java `calculateDistance3D`.
pub(crate) fn distance_3d(a: (i32, i32, i32), b: (i32, i32, i32)) -> f64 {
    let dx = f64::from(a.0.wrapping_sub(b.0));
    let dy = f64::from(a.1.wrapping_sub(b.1));
    let dz = f64::from(a.2.wrapping_sub(b.2));
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Java `Npc.MINIMUM_SOCIAL_INTERVAL`.
const MINIMUM_SOCIAL_INTERVAL_MS: i64 = 6000;

impl World {
    /// Java `Creature.getTarget()` of a player.
    pub fn target_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.players.get(&id).and_then(|p| p.target)
    }

    /// Java `World.findObject(id)`: an object that is in the world.
    pub fn find_object(&self, id: ObjectId) -> Option<ObjectId> {
        self.objects.get(&id).filter(|e| e.spawned).map(|_| id)
    }

    /// Java `isAlikeDead()`: dead (fake death is an effect, M5). See `World::is_dead`.
    pub fn is_alike_dead(&self, id: ObjectId) -> bool {
        self.is_dead(id)
    }

    /// Java `WorldObject.isTargetable()`.
    pub(crate) fn is_targetable(&self, id: ObjectId) -> bool {
        match self.objects.get(&id).map(|e| e.kind) {
            Some(ObjectKind::Npc) => self
                .npcs
                .get(&id)
                .and_then(|n| self.npc_world.template(n.npc_id))
                .is_none_or(|t| t.targetable),
            Some(ObjectKind::Door) => self.doors.get(&id).is_none_or(|d| d.targetable),
            _ => true,
        }
    }

    /// Java `target.isAutoAttackable(attacker)` for a player attacker.
    pub fn is_auto_attackable_by(&self, target: ObjectId, attacker: ObjectId) -> bool {
        let karma = self.players.get(&attacker).map_or(0, |p| p.data.karma);
        match self.objects.get(&target).map(|e| e.kind) {
            Some(ObjectKind::Npc) => self
                .npcs
                .get(&target)
                .is_some_and(|n| n.class.auto_attackable_by(karma)),
            // Doors can be attacked when the template says so; sieges (castle, fort, siegable
            // clan hall doors) arrive with M5.
            Some(ObjectKind::Door) => self.doors.get(&target).is_some_and(|d| d.attackable_door),
            Some(ObjectKind::Player) => self.is_auto_attackable(target, attacker),
            _ => false,
        }
    }

    /// `getLevel()` of a creature.
    fn level_of(&self, id: ObjectId) -> i32 {
        match self.objects.get(&id).map(|e| e.kind) {
            Some(ObjectKind::Player) => self.players.get(&id).map_or(0, |p| p.core.level),
            Some(ObjectKind::Npc) => self.npcs.get(&id).map_or(0, |n| n.core.level),
            Some(ObjectKind::Door) => self.doors.get(&id).map_or(0, |d| d.template.level),
            // StaticObject.getLevel()
            Some(ObjectKind::StaticObject) => 1,
            _ => 0,
        }
    }

    /// `(getMaxHp(), (int) getCurrentHp())` of a creature, for the target window.
    fn hp_of(&self, id: ObjectId) -> (i32, i32) {
        match self.objects.get(&id).map(|e| e.kind) {
            Some(ObjectKind::Player) => self
                .players
                .get(&id)
                .map_or((0, 0), |p| (p.core.max_hp as i32, p.core.cur_hp as i32)),
            Some(ObjectKind::Npc) => self
                .npcs
                .get(&id)
                .map_or((0, 0), |n| (n.core.max_hp as i32, n.core.cur_hp as i32)),
            Some(ObjectKind::Door) => self
                .doors
                .get(&id)
                .map_or((0, 0), |d| (d.max_hp, d.current_hp as i32)),
            // A static object has no hit points of its own: the stat system floors the maximum
            // at 1 and the status never filled the current value.
            Some(ObjectKind::StaticObject) => (1, 0),
            _ => (0, 0),
        }
    }

    fn broadcast_to_visible_players(&self, id: ObjectId, packet: &impl l2_net::Encode) {
        // Java `World.broadcastToVisiblePlayers`: every other player in the surrounding regions
        // (same instance); no invisibility check, unlike `Creature.broadcastPacket`.
        let bytes = packet.to_bytes();
        for viewer in self.visible_players(id) {
            self.send_bytes(viewer, bytes.clone());
        }
    }

    /// Java `Player.setTarget(worldObject)`.
    pub fn set_target(&mut self, id: ObjectId, new_target: Option<ObjectId>) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let (px, py, pz) = (player.core.x, player.core.y, player.core.z);
        let old_target = player.target;
        let mut new_target = new_target;

        if let Some(target) = new_target {
            // FOLLOW-UP(M5): a party member is exempt from both checks (`isInParty`).
            let in_party = false;
            // Prevents /target exploiting.
            if !in_party
                && self
                    .position_of(target)
                    .is_some_and(|(_, _, tz)| tz.wrapping_sub(pz).unsigned_abs() > 3000)
            {
                new_target = None;
            }
            // Check if the new target is visible.
            if new_target.is_some() && !in_party && !self.objects.get(&target).is_some_and(|e| e.spawned) {
                new_target = None;
            }
            // Vehicles cannot be targeted: none exist yet.
        }

        if let Some(old) = old_target {
            // No target change? Validate the location of the target.
            if Some(old) == new_target {
                if old != id {
                    self.send_validate_location_of(id, old);
                }
                return;
            }
            // Java `oldTarget.removeStatusListener(this)`.
            self.remove_status_listener(old, id);
        }

        if let Some(target) = new_target
            && self.objects.get(&target).is_some_and(|e| e.kind.is_creature())
        {
            // Validate location of the new target.
            if target != id {
                self.send_validate_location_of(id, target);
            }
            // Show the client his new target.
            let level = self.players.get(&id).map_or(0, |p| p.core.level);
            let auto_attackable = self.is_auto_attackable_by(target, id);
            self.send(
                id,
                &MyTargetSelected::new(target.0, level, self.level_of(target), auto_attackable),
            );
            // Register target to listen for hp changes (Java `target.addStatusListener(this)`).
            self.add_status_listener(target, id);
            // Send max/current hp.
            let (max_hp, cur_hp) = self.hp_of(target);
            self.send_target_hp(id, target, max_hp, cur_hp);
            // To others the new target, and not yourself!
            self.broadcast_to_visible_players(
                id,
                &TargetSelected {
                    object_id: id.0,
                    target_id: target.0,
                    x: px,
                    y: py,
                    z: pz,
                },
            );
        }

        // Target was removed?
        if new_target.is_none() && old_target.is_some() {
            self.broadcast_to_self_and_visible(
                id,
                &TargetUnselected {
                    target_id: id.0,
                    x: px,
                    y: py,
                    z: pz,
                },
            );
        }

        // `Creature.setTarget`: an object that is not spawned is never kept.
        let keep = new_target.filter(|t| self.objects.get(t).is_some_and(|e| e.spawned));
        if let Some(player) = self.players.get_mut(&id) {
            player.target = keep;
        }
    }

    /// `new ValidateLocation(object)` to a player.
    fn send_validate_location_of(&self, to: ObjectId, object: ObjectId) {
        let Some((x, y, z)) = self.position_of(object) else {
            return;
        };
        let heading = match self.objects.get(&object).map(|e| e.kind) {
            Some(ObjectKind::Player) => self.players.get(&object).map_or(0, |p| p.core.heading),
            Some(ObjectKind::Npc) => self.npcs.get(&object).map_or(0, |n| n.core.heading),
            // Doors and static objects have a heading of 0 (`Location.getHeading`).
            _ => 0,
        };
        self.send(
            to,
            &ValidateLocation {
                object_id: object.0,
                x,
                y,
                z,
                heading,
            },
        );
    }

    /// `if (creature.getTarget() == other) creature.setTarget(null)`, the part of Java's
    /// `removeVisibleObject` / `switchRegion` loops that runs when `other` leaves `creature`'s
    /// view, plus the AI's `notifyActionForgetObject`.
    pub(crate) fn clear_target_if(&mut self, creature: ObjectId, other: ObjectId) {
        if self.npcs.contains_key(&creature) {
            // `ai.notifyActionForgetObject(other)`; an NPC's own `_target` is only set by its
            // skills (M5).
            self.npc_notify_forget_object(creature, other);
            return;
        }
        if !self.players.contains_key(&creature) {
            return;
        }
        // `ai.notifyActionForgetObject(other)`.
        self.notify_action_forget_object(creature, other);
        if self.players.get(&creature).and_then(|p| p.target) == Some(other) {
            self.set_target(creature, None);
        }
    }

    /// Java `Action.runImpl`.
    pub(crate) fn on_action(&mut self, id: ObjectId, object_id: i32, action_id: i8) {
        if !self.players.contains_key(&id) {
            return;
        }
        // FOLLOW-UP: flood protection, observer mode, the bot-penalty effect.
        let wanted = ObjectId(object_id);
        let obj = if self.target_of(id) == Some(wanted) {
            Some(wanted)
        } else {
            self.find_object(wanted)
        };

        // If object requested does not exist, add warn msg into logs.
        let Some(obj) = obj else {
            // Pressing e.g. pickup many times quickly would get you here.
            self.send(id, &ActionFailed);
            return;
        };
        if !self.is_targetable(obj) && !self.is_gm(id) {
            self.send(id, &ActionFailed);
            return;
        }
        // Players can't interact with objects in the other instances, except from multiverse.
        let obj_instance = self.objects.get(&obj).map_or(0, |e| e.instance_id);
        let player_instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        if obj_instance != player_instance && player_instance != -1 {
            self.send(id, &ActionFailed);
            return;
        }
        // Only GMs can directly interact with invisible characters.
        if !self.is_visible_for(obj, id) {
            self.send(id, &ActionFailed);
            return;
        }
        // FOLLOW-UP: `getActiveRequester()` (trade, party and friend requests), `onActionRequest()`.

        match action_id {
            0 => self.on_action_object(id, obj, true),
            1 => {
                // `AltGameViewNpc` is off and fake players are not ported, so only a GM gets the
                // shift click.
                if !self.is_gm(id) {
                    self.on_action_object(id, obj, false);
                } else {
                    self.on_action_shift(id, obj);
                }
            }
            other => {
                // Invalid action detected (probably client cheating), log this.
                let name = self.players.get(&id).map_or("?", |p| p.data.name.as_str());
                warn!("[C] Action: Character: {name} requested invalid action: {other}");
                self.send(id, &ActionFailed);
            }
        }
    }

    /// Java `WorldObject.onAction(player, interact)`.
    pub(crate) fn on_action_object(&mut self, player: ObjectId, obj: ObjectId, interact: bool) {
        match self.objects.get(&obj).map(|e| e.kind) {
            Some(ObjectKind::Npc) => self.npc_click(player, obj, interact),
            Some(ObjectKind::Player) => self.player_click(player, obj, interact),
            Some(ObjectKind::Door) => self.door_click(player, obj, interact),
            Some(ObjectKind::StaticObject) => self.static_object_click(player, obj, interact),
            // A fence has no click handler.
            _ => {}
        }
        self.send(player, &ActionFailed);
    }

    /// Java `WorldObject.onActionShift(player)`.
    fn on_action_shift(&mut self, player: ObjectId, obj: ObjectId) {
        // FOLLOW-UP(admin): the GM windows (`npcinfo.htm`, door and player info) need the stat
        // system; the GM keeps the target they asked for.
        debug!("shift click of a GM on {obj}: only the target is set");
        self.set_target(player, Some(obj));
        self.send(player, &ActionFailed);
    }

    /// `NpcClick.onAction`.
    fn npc_click(&mut self, player: ObjectId, npc: ObjectId, interact: bool) {
        // `npc.canTarget(player)`: `isOutOfControl` and locked targets are not modelled.
        if let Some(p) = self.players.get_mut(&player) {
            p.talk.last_folk_npc = Some(npc);
        }

        // Check if the Player already target the Npc.
        if self.target_of(player) != Some(npc) {
            // Set the target of the Player player.
            self.set_target(player, Some(npc));
            // Check if the player is attackable (without a forced attack).
            if self.is_auto_attackable_by(npc, player) {
                // Wake up the AI (`npc.getAI()`).
                self.wake_ai(npc);
            }
        } else if interact {
            let attackable = self.is_auto_attackable_by(npc, player);
            // Check if the player is attackable (without a forced attack) and isn't dead.
            if attackable && !self.is_alike_dead(npc) {
                self.set_intention_attack(player, npc);
            } else if !attackable {
                // Calculate the distance between the Player and the Npc.
                if !self.can_interact(npc, player) {
                    self.set_intention_interact(player, npc);
                } else {
                    self.talk_to_npc(player, npc);
                }
            }
        }
    }

    /// The talk branch of `NpcClick.onAction`: the NPC is close enough and the player clicked it
    /// a second time.
    pub(crate) fn talk_to_npc(&mut self, player: ObjectId, npc: ObjectId) {
        // If player is moving and NPC is in interaction distance, change the intention to stop
        // moving.
        if self.is_moving(player) {
            self.set_intention_idle(player);
        }

        // Turn NPC to the player.
        if let (Some(p), Some(n)) = (self.players.get(&player), self.npcs.get(&npc)) {
            self.send(
                player,
                &MoveToPawn {
                    object_id: player.0,
                    target_id: npc.0,
                    distance: 100,
                    x: p.core.x,
                    y: p.core.y,
                    z: p.core.z,
                    target_x: n.core.x,
                    target_y: n.core.y,
                    target_z: n.core.z,
                },
            );
        }
        if self.has_random_animation(npc) {
            let animation = self.rng().below(8);
            self.on_random_animation(npc, animation);
        }

        // Stop movement when trying to talk to a moving NPC: NPCs do not move before M4.
        // Open a chat window on client with the text of the Npc. `ON_NPC_FIRST_TALK` and
        // `ON_NPC_QUEST_START` are script listeners (D-05).
        self.show_chat_window_default(npc, player);

        if self.settings.player_movement_block_time_ms > 0 {
            self.update_not_move_until(player);
        }
    }

    /// Java `Npc.hasRandomAnimation()`.
    fn has_random_animation(&self, npc: ObjectId) -> bool {
        let Some(template) = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
        else {
            return false;
        };
        self.settings.talk.max_npc_animation > 0
            && template.random_animation
            && !template.fake_player
            && template.ai_type != l2_data::npc::AiType::Corpse
    }

    /// Java `Npc.onRandomAnimation(animationId)`: the greeting is shown at most every 6 seconds.
    fn on_random_animation(&mut self, npc: ObjectId, animation: i32) {
        let now = self.world_ms();
        let last = self.talk.last_social_ms.get(&npc).copied();
        if last.is_none_or(|last| now - last > MINIMUM_SOCIAL_INTERVAL_MS) {
            self.talk.last_social_ms.insert(npc, now);
            // `broadcastSocialAction`: an NPC only does it where players are awake.
            if self.are_region_neighbors_active(npc) {
                self.broadcast_packet(
                    npc,
                    &SocialAction {
                        object_id: npc.0,
                        action_id: animation,
                    },
                );
            }
        }
    }

    /// `PlayerClick.onAction`.
    fn player_click(&mut self, player: ObjectId, target: ObjectId, interact: bool) {
        // `isOutOfControl` and locked targets are not modelled.
        if self.target_of(player) != Some(target) {
            self.set_target(player, Some(target));
        } else if interact {
            // FOLLOW-UP(M5): private stores (`setIntentionInteract`), the cursed weapon level
            // rule, `setIntentionFollow` for a player that cannot be attacked.
            if self.is_auto_attackable_by(target, player) {
                self.set_intention_attack(player, target);
            } else {
                // This Action Failed packet avoids player getting stuck when clicking three or
                // more times.
                self.send(player, &ActionFailed);
            }
        }
    }

    /// `DoorClick.onAction`.
    fn door_click(&mut self, player: ObjectId, door: ObjectId, interact: bool) {
        if self.target_of(player) != Some(door) {
            self.set_target(player, Some(door));
        } else if interact {
            // FOLLOW-UP(M4/M5): attacking a siege door, and the clan hall / fortress door
            // confirmation dialogs (clans).
            debug!("{player} clicked door {door} twice: attacking and clan doors arrive later");
        }
    }

    /// `StaticObjectClick.onAction`.
    fn static_object_click(&mut self, player: ObjectId, object: ObjectId, interact: bool) {
        if self.target_of(player) != Some(object) {
            // Set the target of the Player player.
            self.set_target(player, Some(object));
        } else if interact {
            let Some(static_object) = self.static_objects.get(&object) else {
                return;
            };
            let (kind, static_id, texture, map_x, map_y) = (
                static_object.kind,
                static_object.static_object_id,
                static_object.texture.clone(),
                static_object.map_x,
                static_object.map_y,
            );
            let (Some(pos), Some(me)) = (
                self.position_of(object),
                self.players.get(&player).map(|p| (p.core.x, p.core.y, p.core.z)),
            ) else {
                return;
            };
            // Calculate the distance between the Player and the object.
            if distance_2d(me, pos) >= f64::from(INTERACTION_DISTANCE) {
                // Notify the Player AI with INTERACT.
                self.set_intention_interact(player, object);
            } else if kind == 2 {
                let filename = if static_id == 24_230_101 {
                    "data/html/signboard/tomb_of_crystalgolem.htm"
                } else {
                    "data/html/signboard/pvp_signboard.htm"
                };
                let html = match self.get_htm(player, filename) {
                    Some(content) => NpcHtmlMessage::new(object.0, &content),
                    None => NpcHtmlMessage::new(
                        object.0,
                        &format!("<html><body>Signboard is missing:<br>{filename}</body></html>"),
                    ),
                };
                if let Some(html) = html {
                    self.send_html(player, &html);
                }
            } else if kind == 0 {
                self.send(
                    player,
                    &ShowTownMap {
                        texture: format!("town_map.{texture}"),
                        x: map_x,
                        y: map_y,
                    },
                );
            }
        }
    }

    /// Java `RequestTargetCanceld.runImpl`.
    pub(crate) fn on_target_canceled(&mut self, id: ObjectId, unselect: i16) {
        if !self.players.contains_key(&id) {
            return;
        }
        // FOLLOW-UP(M4): `isLockedTarget()` (FAILED_TO_DISABLE_ATTACK_TARGET) and, for Escape
        // (`unselect == 0`), aborting the cast the player is in before the target goes.
        let _ = unselect;
        if self.target_of(id).is_some() {
            self.set_target(id, None);
        }
        // Airships are not modelled (`TargetUnselected` for a player on a helm).
    }
}
