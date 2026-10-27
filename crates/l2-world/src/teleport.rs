//! `teleToLocation`: taking a player out of the world, telling the client where it goes, and
//! putting it back once the client answers with `Appearing`.
// Java: entity/actor/Creature.java (teleToLocation, onTeleported, setTeleporting, stopMove)
// Java: entity/actor/Player.java (teleToLocation, onTeleported, setTeleporting, broadcastPacket,
//       TeleportWatchdogTask)
// Java: network/clientpackets/Appearing.java
// Java: entity/WorldObject.java (decayMe, spawnMe, broadcastInfo, setInstanceId)
// Java: data/xml/MapRegionData.java (getTeleToLocation)
//
// # The sequence
// `tele_to_location` is Java's `Creature.teleToLocation`: `StopMove` to the player and to those
// who see it, `setTeleporting(true)`, `decayMe` (everybody around gets `DeleteObject`, the
// player gets `DeleteObject` for everybody), the offset search, `TeleportToLocation` to the
// player and to those who saw it leave, the new instance, the new position (`setXYZ`). The player
// is now out of every region until its client has loaded the target and sends `Appearing`.
//
// `on_appearing` is `Appearing.runImpl`: `onTeleported` (`spawnMe`: the player sees what is
// around the target and the others see the player, `setTeleporting(false)`, the zones are
// revalidated for real, `broadcastInfo`), then `UserInfo`.
//
// # What is left out, with the system that brings it
// - `stopMove`, `abortAttack`, `abortCast`, `getAI().setIntentionActive()` and the
//   `doRevive` of a pending revive (a restart point, M4-DEATH-01) are in. `setTarget(null)` is
//   in (M3-TALK-01).
// - A pet follows its owner, trained beasts are lost, `checkItemRestriction`, the movie, auto play,
//   the teleport protection (`PlayerTeleportProtection`, default 0), and the
//   `Player.teleToLocation(loc)` override that first sends a flying form below -1005 to -1005:
//   M4+. A flying creature keeps the z of the target (`_isFlying`, wyverns and flying forms).
// - A detached client (`GameClient.isDetached`, offline mode) is never teleported "in place".

use l2_geo::Location;
use l2_protocol_game::server::TeleportToLocation;
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::timer::{TimerId, TimerKind};
use crate::world::World;

/// The `.ini` values a teleport reads (Java `PlayerConfig`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeleportSettings {
    /// `OffsetOnTeleportEnabled`.
    pub offset_on_teleport_enabled: bool,
    /// `MaxOffsetOnTeleport`: the random offset of `teleToLocation(..., true)`.
    pub max_offset_on_teleport: i32,
    /// `RandomRespawnInTownEnabled`: restart points are picked at random.
    pub random_respawn_in_town_enabled: bool,
    /// `TeleportWatchdogTimeout` in seconds; 0 = no watchdog.
    pub teleport_watchdog_timeout: i32,
    /// `PlayerTeleportProtection` in seconds; 0 = none.
    pub player_teleport_protection: i32,
}

impl Default for TeleportSettings {
    fn default() -> Self {
        Self {
            offset_on_teleport_enabled: true,
            max_offset_on_teleport: 50,
            random_respawn_in_town_enabled: true,
            teleport_watchdog_timeout: 0,
            player_teleport_protection: 0,
        }
    }
}

/// Java `Creature._isTeleporting` and the `TeleportWatchdogTask` of a player.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TeleportState {
    pub teleporting: bool,
    watchdog: Option<TimerId>,
    /// Java `_teleportProtectEndTime` in world milliseconds (0 = none).
    pub protect_end_ms: i64,
}

/// Where a teleport goes: the arguments of Java's `teleToLocation` overloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeleportTarget {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// 0 keeps the current heading (Java: `if (heading != 0) setHeading(heading)`).
    pub heading: i32,
    /// The instance to enter; negative keeps the current one (the overloads without a
    /// `Location` pass -1).
    pub instance_id: i32,
    /// `randomOffset`: 0 or `MaxOffsetOnTeleport`.
    pub random_offset: i32,
}

impl TeleportTarget {
    /// Java `teleToLocation(x, y, z)`: heading and instance unchanged, no random offset.
    pub fn at(x: i32, y: i32, z: i32) -> Self {
        Self {
            x,
            y,
            z,
            heading: 0,
            instance_id: -1,
            random_offset: 0,
        }
    }

    #[must_use]
    pub fn with_heading(mut self, heading: i32) -> Self {
        self.heading = heading;
        self
    }

    #[must_use]
    pub fn with_instance(mut self, instance_id: i32) -> Self {
        self.instance_id = instance_id;
        self
    }

    #[must_use]
    pub fn with_random_offset(mut self, random_offset: i32) -> Self {
        self.random_offset = random_offset;
        self
    }
}

/// Java `TeleportWhereType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeleportWhereType {
    Castle,
    ClanHall,
    SiegeFlag,
    Town,
    Fortress,
}

impl World {
    /// Java `Creature.isTeleporting()`.
    pub fn is_teleporting(&self, id: ObjectId) -> bool {
        if let Some(n) = self.npcs.get(&id) {
            return n.movement.teleporting;
        }
        self.players.get(&id).is_some_and(|p| p.tele.teleporting)
    }

    /// Java `Player.setTeleporting(value, true)`: the flag, and the watchdog that completes a
    /// teleport whose client never answers (`TeleportWatchdogTimeout`).
    /// Java `Player.broadcastPacket(packet)` for packets other than `CharInfo`: the player first,
    /// then everybody who can see it.
    pub(crate) fn broadcast_to_self_and_visible(&self, id: ObjectId, packet: &impl l2_net::Encode) {
        self.send(id, packet);
        self.broadcast_packet(id, packet);
    }

    fn set_teleporting(&mut self, id: ObjectId, value: bool) {
        // An NPC has no client to wait for: just the flag.
        if let Some(npc) = self.npcs.get_mut(&id) {
            npc.movement.teleporting = value;
            return;
        }
        let timeout = self.settings.teleport.teleport_watchdog_timeout;
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.tele.teleporting = value;
        // The same Java flag, as the movement code reads it (M3-MOVE-01).
        player.movement.teleporting = value;
        if value {
            if player.tele.watchdog.is_none() && timeout > 0 {
                let ticks = crate::timer::TimerWheel::duration_to_ticks(std::time::Duration::from_secs(
                    u64::try_from(timeout).unwrap_or(0),
                ));
                let due = self.tick + ticks;
                let timer = self.timers.schedule_at(due, TimerKind::TeleportWatchdog(id));
                if let Some(player) = self.players.get_mut(&id) {
                    player.tele.watchdog = Some(timer);
                }
            }
        } else if let Some(timer) = player.tele.watchdog.take() {
            self.timers.cancel(timer);
        }
    }

    /// Java `TeleportWatchdogTask.run`.
    pub(crate) fn on_teleport_watchdog(&mut self, id: ObjectId) {
        if self.is_teleporting(id) {
            debug!("{id} did not answer its teleport; completing it.");
            self.on_teleported(id);
        }
    }

    /// Java `WorldObject.broadcastInfo()`: `sendInfo` to every player who can see the object.
    pub(crate) fn broadcast_info(&mut self, id: ObjectId) {
        for viewer in self.visible_players(id) {
            if self.is_visible_for(id, viewer) {
                self.send_info(id, viewer);
            }
        }
    }

    /// Java `Creature.teleToLocation(x, y, z, heading, instanceId, randomOffset)` for a player or
    /// an NPC. Does nothing for a player who is not online.
    pub fn tele_to_location(&mut self, id: ObjectId, target: TeleportTarget) {
        let is_npc = self.npcs.contains_key(&id);
        // "Prevent teleporting for players that disconnected unexpectedly."
        // (`isOnline()`: an offline trader without a connection is online.)
        if !is_npc
            && (!self.players.contains_key(&id)
                || !(self.sinks.contains_key(&id) || self.is_offline_trader(id)))
        {
            return;
        }
        // `Player.teleToLocation`: "if the vehicle is not teleporting, the player gets off".
        self.vehicle_before_player_teleport(id);
        let TeleportTarget {
            x: x_value,
            y: y_value,
            z: z_value,
            heading,
            instance_id,
            random_offset,
        } = target;
        let geo = self.zoning.geo.clone();
        let mut x = x_value;
        let mut y = y_value;
        // Not flying: the ground under the target.
        let flying = self.is_flying(id);
        let mut z = if flying {
            z_value
        } else {
            geo.as_ref()
                .map_or(z_value, |geo| geo.height(x_value, y_value, z_value))
        };

        // "Prepare creature for teleport." A player who picked a restart point is revived now,
        // before he leaves (`RequestRestartPoint` set the flag).
        if self.death.pending_revive.contains(&id) {
            self.do_revive(id);
        }

        // "Abort any client actions, casting and remove target."
        // `stopMove(null)`: the movement state goes (with a pending path search) and `StopMove` is
        // broadcast.
        self.stop_move(id, None);
        self.abort_attack(id);
        self.abort_cast(id);
        // `setTarget(null)`: TargetUnselected to everybody if it had a target.
        self.set_target(id, None);

        self.set_teleporting(id, true);
        self.set_intention_active(id);

        // Java sends `TeleportToLocation` after `decayMe()`, but `forEachVisibleObject` goes by
        // the object's position, which has not changed yet: the same players who just got
        // `DeleteObject` get the teleport packet too.
        let watchers: Vec<ObjectId> = self
            .visible_players(id)
            .into_iter()
            .filter(|viewer| self.is_visible_for(id, *viewer))
            .collect();
        // "Remove the object from its old location."
        self.decay_keep_entry(id);

        // "Adjust position a bit."
        if self.settings.teleport.offset_on_teleport_enabled || random_offset > 0 {
            let radius = self.stats(id).map_or(0.0, |s| s.collision_radius());
            let ro = random_offset;
            x = x_value + self.rng().get(-ro, ro);
            y = y_value + self.rng().get(-ro, ro);

            let collision = f64::from((radius * 2.0).min(ro as f32));
            let from = Location::new(x_value, y_value, z_value);
            let mut count = 0;
            loop {
                let attempt = count;
                count += 1;
                if attempt >= 100 {
                    break;
                }
                let (dx, dy, dz) = (
                    f64::from(x_value - x),
                    f64::from(y_value - y),
                    f64::from(z_value - z),
                );
                let too_close = (dx * dx + dy * dy + dz * dz).sqrt() < collision;
                let to = Location::new(x, y, z);
                let blocked = too_close
                    || geo.as_ref().is_some_and(|geo| {
                        !geo.can_see_target_in_instance(from, to, instance_id)
                            || !geo.can_move_to_target(from, to, instance_id)
                    });
                if !blocked {
                    break;
                }
                x = x_value + self.rng().get(-ro, ro);
                y = y_value + self.rng().get(-ro, ro);
            }

            // Not flying.
            if !flying && let Some(geo) = &geo {
                let valid = geo.valid_location(
                    Location::new(x_value, y_value, z),
                    Location::new(x, y, z),
                    instance_id,
                );
                (x, y, z) = (valid.x, valid.y, valid.z);
            }
        }

        z += 5;

        // "Send teleport packet where needed."
        let packet = TeleportToLocation {
            object_id: id.0,
            x,
            y,
            z,
            heading,
        };
        self.send(id, &packet);
        let bytes = l2_net::Encode::to_bytes(&packet);
        for viewer in watchers {
            self.send_bytes(viewer, bytes.clone());
        }

        // "Change instance id."
        self.set_instance_id(id, instance_id);

        // "Set the x,y,z position of the WorldObject and if necessary modify its _worldRegion."
        self.set_position(id, x, y, z);

        // "Also adjust heading."
        // NOTE(java-quirk): a spawn's heading of -1 (random) is set as it is by an NPC that
        // teleports to its spawn.
        if heading != 0 {
            self.set_heading(id, heading);
        }

        // Java completes the teleport right here only for a detached client; everybody else
        // waits for `Appearing`. An NPC has no client: `onTeleported()` at once.
        if is_npc || self.is_offline_trader(id) {
            self.on_teleported(id);
        }

        self.revalidate_zone(id, true);
    }

    // Java `WorldObject.setInstanceId` is `World::set_instance_id` (`instance/membership.rs`).

    /// Java `Appearing.runImpl`: the client has loaded the target of a teleport.
    pub(crate) fn on_appearing(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        if self.is_teleporting(id) {
            self.on_teleported(id);
        }
        self.send_user_info(id);
    }

    /// Java `Player.onTeleported()` (with `Creature.onTeleported()`): back into the world at the
    /// new position.
    pub fn on_teleported(&mut self, id: ObjectId) {
        // Creature.onTeleported
        if !self.is_teleporting(id) {
            return;
        }
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        self.spawn_at(id, x, y, z);
        self.set_teleporting(id, false);
        // `Summon.onTeleported()`: the owner is told where it is.
        if self.is_summon(id) {
            let heading = self.heading_of(id);
            self.send(
                id,
                &TeleportToLocation {
                    object_id: id.0,
                    x,
                    y,
                    z,
                    heading,
                },
            );
            return;
        }
        if self.npcs.contains_key(&id) {
            // Monster.onTeleported: the minions follow their master.
            self.on_master_teleported(id);
            return;
        }

        // Player.onTeleported
        // `isInAirShip()`: the ship describes itself; else `setLastServerPosition(x, y, z)`, the
        // position `ValidatePosition` compares against.
        if !self.vehicle_player_on_teleported(id)
            && let Some(p) = self.players.get_mut(&id)
        {
            p.movement.last_server_position = (x, y, z);
        }
        // "Force a revalidation"
        self.revalidate_zone(id, true);
        // `checkItemRestriction()`
        let mut changed = Vec::new();
        self.check_item_restriction(id, &mut changed);
        // `PLAYER_TELEPORT_PROTECTION > 0 && !_inOlympiadMode` (the olympiad is not modelled)
        if self.settings.teleport.player_teleport_protection > 0 {
            self.set_teleport_protection(id, true);
        }
        // Not modelled: the tamed beasts (`TamedBeast`), the movie holder and the auto play
        // tasks (`AutoPlayConfig`, Custom).
        // "Modify the position of the pet if necessary."
        self.summon_follow_owner_teleport(id);
        // A dimensional rift member who arrives outside the waiting room is alive again
        // (`memberRessurected`, M8-FEST-01).
        self.rift_member_resurrected(id);
        // "Send info to nearby players."
        self.broadcast_info(id);
    }

    /// Java `MapRegionData.getTeleToLocation(player, where)`: the clan's hall, castle, fortress or
    /// siege headquarters (`siege/restart.rs`, M7-SIEGE-01), else outside the town for a karma
    /// player, far from a castle under siege for its participants while Dawn owns the Seal of
    /// Strife, the exit point of the player's instance (M8-INST-01), else the nearest town. The
    /// faction base is not modelled.
    pub fn get_tele_to_location(
        &mut self,
        id: ObjectId,
        teleport_where: TeleportWhereType,
    ) -> Option<(i32, i32, i32)> {
        let karma = self.players.get(&id)?.data.karma;
        // `player.getUCState() != UC_STATE_NONE`: the coliseum has its own way back (M8-GAMES-01).
        if self.uc_state_of(id) != crate::minigame::UC_STATE_NONE {
            return None;
        }
        if let Some(location) = self.residence_tele_to_location(id, teleport_where) {
            return Some(location);
        }
        // Karma player respawn outside city.
        if karma > 0 {
            return self.nearest_karma_respawn(id);
        }
        // Respawn far from castle during siege if clan is participating.
        if let Some(location) = self.siege_strife_restart_point(id) {
            return Some(location);
        }
        // Instance exit location.
        if self.is_in_instance(id)
            && let Some(location) = self
                .instance(self.instance_id_of(id))
                .and_then(|i| i.exit_location)
        {
            return Some(location);
        }
        self.nearest_town_respawn(id)
    }

    /// Java `Player.setTeleportProtection(protect)`.
    pub fn set_teleport_protection(&mut self, id: ObjectId, protect: bool) {
        let seconds = i64::from(self.settings.teleport.player_teleport_protection);
        let end = self.world_ms() + seconds * 1000;
        if let Some(p) = self.players.get_mut(&id) {
            p.tele.protect_end_ms = if protect { end } else { 0 };
        }
    }

    /// Java `Player.isTeleportProtected()`.
    pub fn is_teleport_protected(&self, id: ObjectId) -> bool {
        let now = self.world_ms();
        self.players
            .get(&id)
            .is_some_and(|p| p.tele.protect_end_ms != 0 && p.tele.protect_end_ms > now)
    }

    /// Java `teleToLocation(TeleportWhereType)`: `teleToLocation(getTeleToLocation(this, where),
    /// true)`, i.e. the restart point with the random offset, in instance 0.
    pub fn tele_to_location_where(&mut self, id: ObjectId, teleport_where: TeleportWhereType) {
        let Some((x, y, z)) = self.get_tele_to_location(id, teleport_where) else {
            // Java throws a NullPointerException here.
            warn!("{id}: no restart point for {teleport_where:?}; not teleporting.");
            return;
        };
        let offset = self.settings.teleport.max_offset_on_teleport;
        self.tele_to_location(
            id,
            TeleportTarget::at(x, y, z)
                .with_instance(0)
                .with_random_offset(offset),
        );
    }
}
