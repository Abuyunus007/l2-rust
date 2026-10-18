//! Player movement: `MoveToLocation` -> checks (geodata, pathfinding) -> per-tick position
//! updates -> `MoveToLocation` / `StopMove` broadcasts, `CannotMoveAnymore`, `ValidatePosition`.
// Java: org/l2jmobius/gameserver/entity/actor/Creature.java (moveToLocation, moveToNextRoutePoint,
//       updatePosition, stopMove, broadcastMoveToLocation, isOnGeodataPath, setSyncedXYZ)
// Java: org/l2jmobius/gameserver/taskmanagers/MovementTaskManager.java
// Java: org/l2jmobius/gameserver/network/clientpackets/{MoveToLocation,ValidatePosition,
//       CannotMoveAnymore}.java
// Java: org/l2jmobius/gameserver/ai/{AbstractAI,CreatureAI,PlayerAI}.java (the move intention path:
//       setIntentionMoveTo, moveTo, clientStopMoving, notifyActionArrived,
//       notifyActionArrivedBlocked)
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (isFalling, last server position,
//       client position)
//
// # Time
// Java's movement manager calls `updatePosition` for players every 50 ms, but the position only
// advances when `GameTimeTaskManager.getGameTicks()` (100 ms) changed. The world tick is 50 ms
// (D-12), so a game tick here is two world ticks and the same "already calculated" check makes
// every second call a no-op, exactly as in Java.
//
// # Pathfinding (D-14)
// Java pathfinds inline in `moveToLocation`. Here the search runs on the worker pool: the part of
// `moveToLocation` before the search is [`World::prepare_move`], the request goes out through
// [`PathRequests`], and when the result comes back as `WorldCommand::PathFound` the rest of the
// method ([`World::complete_move`]) runs. The old move (if any) keeps going meanwhile, as it does
// in Java until the new `MoveData` replaces it. A newer request, a stop or a new direct move
// drops the pending one, so a stale result is ignored.
//
// # What is not modelled yet (each has its card)
// - Zones (water, castle, `revalidateZone`): M3-TELE-01 / M3 zones. Nobody is "in water".
// - Flying, vehicles, transformations, falling damage (HP), casting/attacking intentions,
//   spawn protection (`onActionRequest`), the admin tele mode: M4/M5.
// - Doors and fences: M3-DOOR-01 (`doors_between`, the region door lists). The geoengine's own
//   obstacle hook is used by every check that goes through it.
// - NPC movement (`MoveToPawn`, attackable teleport back): M4.

use std::sync::Arc;

use l2_geo::coords::{geo_x, geo_y};
use l2_geo::{DynamicObstacles, GeoLocation, Location, NoObstacles, PathRequest, PathResult};
use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, MoveToLocation, StopMove, SystemMessage, ValidateLocation};
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::command::ClientInput;
use crate::enter::current_load;
use crate::player::Player;
use crate::region::{WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN};
use crate::world::{TICK, World};

/// `GameTimeTaskManager.TICKS_PER_SECOND`.
const TICKS_PER_SECOND: i32 = 10;
/// World ticks (50 ms) in one `GameTimeTaskManager` tick (100 ms).
const WORLD_TICKS_PER_GAME_TICK: u64 = 2;
/// Java counts game ticks since local midnight, so they are never near 0. `MoveData` uses 0 as
/// "not started", so the count here starts well above it.
const GAME_TICK_BASE: i32 = 1_000_000;
/// `Player.FALLING_VALIDATION_DELAY`.
const FALLING_VALIDATION_DELAY_MS: i64 = 1000;
/// `LocationUtil.calculateHeadingFrom`: client heading units per degree.
const HEADING_PER_DEGREE: f64 = 182.044_444_444;

/// Java `Intention`, as far as the move path needs it (the rest arrives with combat and NPC AI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Intention {
    #[default]
    Idle,
    Rest,
    MoveTo,
}

/// Java `Creature.MoveData`: one walk to a destination.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveData {
    /// Game tick at which the move was set up.
    pub move_start_time: i32,
    /// Game tick of the last position update (0 = no update yet).
    pub move_timestamp: i32,
    pub x_destination: i32,
    pub y_destination: i32,
    pub z_destination: i32,
    /// Java keeps x and y as doubles to avoid rounding errors (`xAccurate`, `yAccurate`).
    pub x_accurate: f64,
    pub y_accurate: f64,
    /// Java `heading`: the client's last heading while moving (coordinate sync); unused by the
    /// player path but part of the structure.
    pub heading: i32,
    pub disregarding_geodata: bool,
    /// -1 = not on a calculated path, else the index of the node being walked to.
    pub on_geodata_path_index: i32,
    pub geo_path: Option<Vec<GeoLocation>>,
    pub geo_path_accurate_tx: i32,
    pub geo_path_accurate_ty: i32,
    pub geo_path_gtx: i32,
    pub geo_path_gty: i32,
    pub last_broadcast_time: i32,
}

impl MoveData {
    fn new() -> Self {
        Self {
            move_start_time: 0,
            move_timestamp: 0,
            x_destination: 0,
            y_destination: 0,
            z_destination: 0,
            x_accurate: 0.0,
            y_accurate: 0.0,
            heading: 0,
            disregarding_geodata: false,
            on_geodata_path_index: -1,
            geo_path: None,
            geo_path_accurate_tx: 0,
            geo_path_accurate_ty: 0,
            geo_path_gtx: 0,
            geo_path_gty: 0,
            last_broadcast_time: 0,
        }
    }

    /// Java `Creature.isOnGeodataPath(move)`: walking a calculated path, and not its last node.
    pub fn is_on_geodata_path(&self) -> bool {
        if self.on_geodata_path_index == -1 {
            return false;
        }
        match &self.geo_path {
            Some(path) => usize::try_from(self.on_geodata_path_index).is_ok_and(|i| i + 1 != path.len()),
            None => false,
        }
    }
}

/// The locals of `Creature.moveToLocation` that live across the (asynchronous) path search.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveCalc {
    x: i32,
    y: i32,
    z: i32,
    cur_x: i32,
    cur_y: i32,
    cur_z: i32,
    dx: f64,
    dy: f64,
    dz: f64,
    distance: f64,
    cos: f64,
    sin: f64,
    mv: MoveData,
    original_x: i32,
    original_y: i32,
    original_z: i32,
    original_distance: f64,
    gtx: i32,
    gty: i32,
    /// Inside Java's `!disregardingGeodata && !flying && !water && !vehicle && !cursor` block.
    geo_checks: bool,
}

/// A path request in flight.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingPath {
    pub request_id: u64,
    calc: MoveCalc,
}

/// Movement state of one player (what Java keeps in `Creature`/`Player` fields).
#[derive(Debug, Clone, PartialEq)]
pub struct MovementState {
    /// Java `_move`.
    pub mv: Option<MoveData>,
    /// The `PlayerAI` intention.
    pub intention: Intention,
    /// Java `_cursorKeyMovement` (keyboard movement).
    pub cursor_key_movement: bool,
    /// Java `isRunning()`.
    pub running: bool,
    /// Java `isTeleporting()`. M3-TELE-01 sets it for the time between `TeleportToLocation` and
    /// the client's `Appearing`.
    pub teleporting: bool,
    /// Java `Player.getLastServerPosition()`.
    pub last_server_position: (i32, i32, i32),
    /// Java `getClientX/Y/Z/Heading`: what the last `ValidatePosition` reported.
    pub client_x: i32,
    pub client_y: i32,
    pub client_z: i32,
    pub client_heading: i32,
    /// Java `_notMoveUntil`, in world milliseconds ([`World::world_ms`]). The NPC talk code sets it
    /// through [`World::update_not_move_until`].
    pub not_move_until_ms: i64,
    /// Java `_fallingTimestamp` in world milliseconds.
    falling_timestamp_ms: i64,
    pending: Option<PendingPath>,
}

impl MovementState {
    pub fn at(x: i32, y: i32, z: i32) -> Self {
        Self {
            mv: None,
            intention: Intention::Idle,
            cursor_key_movement: false,
            // `UserInfo` tells the client the player runs (`enter.rs`).
            running: true,
            teleporting: false,
            last_server_position: (x, y, z),
            client_x: 0,
            client_y: 0,
            client_z: 0,
            client_heading: 0,
            not_move_until_ms: 0,
            falling_timestamp_ms: 0,
            pending: None,
        }
    }

    /// Java `isMoving()`.
    pub fn is_moving(&self) -> bool {
        self.mv.is_some()
    }

    /// True while a path search for this player is out.
    pub fn is_waiting_for_path(&self) -> bool {
        self.pending.is_some()
    }
}

/// How the world asks for path searches (D-14). The game server implements it over the
/// `l2_geo::PathPool`; the result comes back as `WorldCommand::PathFound`. Tests use a recording
/// or an inline implementation.
pub trait PathRequests: Send {
    /// Queues a search. `false` when it was refused (queue full, pool shut down): the move then
    /// goes straight to the destination, as when Java finds no path.
    fn request(&mut self, request: PathRequest) -> bool;

    /// Drops the live request of `requester` (an object id), if any.
    fn cancel(&mut self, requester: u64);
}

/// What [`World::move_to_location`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveStep {
    /// Finished (a move was set up, or Java returned early).
    Finished,
    /// A path search is out; the rest happens when the result arrives.
    WaitingForPath,
}

/// Java `LocationUtil.calculateHeadingFrom(deltaX, deltaY)`.
pub fn heading_from_delta(delta_x: f64, delta_y: f64) -> i32 {
    let mut angle = delta_y.atan2(delta_x).to_degrees();
    if angle < 0.0 {
        angle += 360.0;
    }
    (angle * HEADING_PER_DEGREE) as i32
}

/// Java `LocationUtil.calculateHeadingFrom(originX, originY, targetX, targetY)`.
pub fn heading_from_points(origin_x: i32, origin_y: i32, target_x: i32, target_y: i32) -> i32 {
    heading_from_delta(
        f64::from(target_x.wrapping_sub(origin_x)),
        f64::from(target_y.wrapping_sub(origin_y)),
    )
}

/// Java `LocationUtil.convertHeadingToDegree`.
pub fn heading_to_degree(heading: i32) -> f64 {
    f64::from(heading) / HEADING_PER_DEGREE
}

/// The point `10 * (speed / 100)` units in front of a creature that faces `heading`, as the
/// `updatePosition` obstacle probes compute it.
fn front_probe(x_prev: i32, y_prev: i32, heading: i32, speed: f64) -> (i32, i32) {
    let angle = heading_to_degree(heading);
    let radian = angle.to_radians();
    let course = 180.0_f64.to_radians();
    let front_distance = 10.0 * (speed / 100.0);
    let x1 = ((std::f64::consts::PI + radian + course).cos() * front_distance) as i32;
    let y1 = ((std::f64::consts::PI + radian + course).sin() * front_distance) as i32;
    (x_prev.wrapping_add(x1), y_prev.wrapping_add(y1))
}

impl World {
    /// Installs the path search service. Without one, a move that needs a path goes straight to
    /// its destination (a warning is logged once). Without geodata (`set_geo`) every geodata
    /// check answers like an engine with no regions: the way is free and heights are unchanged.
    pub fn set_path_requests(&mut self, requests: Box<dyn PathRequests>) {
        self.path_requests = Some(requests);
    }

    /// Java `GameTimeTaskManager.getGameTicks()`: 100 ms ticks (two world ticks each).
    pub fn game_ticks(&self) -> i32 {
        let ticks = i32::try_from(self.tick / WORLD_TICKS_PER_GAME_TICK).unwrap_or(i32::MAX - GAME_TICK_BASE);
        GAME_TICK_BASE.saturating_add(ticks)
    }

    /// The world clock in milliseconds (`tick * 50`), the stand-in for `System.currentTimeMillis()`
    /// in the movement timers, so they follow the tick and tests stay deterministic.
    pub fn world_ms(&self) -> i64 {
        i64::try_from(self.tick).unwrap_or(i64::MAX) * i64::try_from(TICK.as_millis()).unwrap_or(50)
    }

    /// Java `Creature.isMoving()`.
    pub fn is_moving(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.movement.is_moving())
    }

    /// Java `getXdestination()`, `getYdestination()`, `getZdestination()`: where the player is
    /// heading, or where it is when it stands still.
    pub fn destination_of(&self, id: ObjectId) -> Option<(i32, i32, i32)> {
        let p = self.players.get(&id)?;
        Some(match &p.movement.mv {
            Some(mv) => (mv.x_destination, mv.y_destination, mv.z_destination),
            None => (p.core.x, p.core.y, p.core.z),
        })
    }

    /// Java `Player.updateNotMoveUntil()`: the player may not move for `NpcTalkBlockingTime` after
    /// talking to an NPC.
    pub fn update_not_move_until(&mut self, id: ObjectId) {
        let until = self.world_ms() + i64::from(self.settings.player_movement_block_time_ms);
        if let Some(p) = self.players.get_mut(&id) {
            p.movement.not_move_until_ms = until;
        }
    }

    // ------------------------------------------------------------------ client packets

    pub(crate) fn on_movement_input(&mut self, id: ObjectId, input: ClientInput) {
        match input {
            ClientInput::MoveTo {
                target_x,
                target_y,
                target_z,
                origin_x,
                origin_y,
                origin_z,
                movement_mode,
            } => self.on_move_to_location(
                id,
                (target_x, target_y, target_z),
                (origin_x, origin_y, origin_z),
                movement_mode,
            ),
            ClientInput::ValidatePosition { x, y, z, heading } => {
                self.on_validate_position(id, x, y, z, heading);
            }
            ClientInput::CannotMoveAnymore { x, y, z, heading } => {
                self.notify_action_arrived_blocked(id, (x, y, z, heading));
            }
            _ => {}
        }
    }

    /// Java `MoveToLocation.runImpl`.
    fn on_move_to_location(
        &mut self,
        id: ObjectId,
        target: (i32, i32, i32),
        origin: (i32, i32, i32),
        mode: i32,
    ) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        if self.is_overloaded(player) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotMoveYouAreTooEncumbered),
            );
            self.send(id, &ActionFailed);
            return;
        }
        if self.settings.player_movement_block_time_ms > 0
            && !is_gm
            && player.movement.not_move_until_ms > self.world_ms()
        {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotMoveWhileSpeakingToAnNpcOneMomentPlease),
            );
            self.send(id, &ActionFailed);
            return;
        }

        let (target_x, target_y, target_z) = target;
        let (origin_x, origin_y, origin_z) = origin;
        let (px, py, pz, heading) = (player.core.x, player.core.y, player.core.z, player.core.heading);
        let last_server = player.movement.last_server_position;
        if target == origin {
            self.stop_move(id, Some((px, py, pz, heading)));
            self.send(id, &ActionFailed);
            return;
        }

        // Check if target location is obstructed.
        if self.is_completely_blocked(geo_x(target_x), geo_y(target_y), target_z) {
            self.send(id, &ActionFailed);
            return;
        }

        // Check for possible door logout and move over exploit. Also checked at ValidatePosition.
        let instance = self.instance_id_of(id);
        if self.doors_between(
            Location::new(last_server.0, last_server.1, last_server.2),
            Location::new(px, py, pz),
            instance,
            false,
        ) {
            // NOTE(java-quirk): `getLastServerPosition()` is a `Location` that never had a heading
            // set, so stopping there turns the character to heading 0.
            self.stop_move(id, Some((last_server.0, last_server.1, last_server.2, 0)));
            self.send(id, &ActionFailed);
            return;
        }

        if mode == 1 {
            if let Some(p) = self.players.get_mut(&id) {
                p.movement.cursor_key_movement = false;
            }
        } else {
            // 0: the cursor keys.
            if !self.settings.keyboard_movement {
                return;
            }
            let dx = origin_x.wrapping_sub(px);
            let dy = origin_y.wrapping_sub(py);
            if dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) < 1_000_000 {
                self.set_synced_xyz(id, origin_x, origin_y, origin_z);
            }
            if let Some(p) = self.players.get_mut(&id) {
                p.movement.cursor_key_movement = true;
                p.movement.last_server_position = (p.core.x, p.core.y, p.core.z);
            }
        }

        // NOTE: Java's `getTeleMode()` (the admin "//tele" click-to-teleport mode) is not ported;
        // it belongs to the admin commands.

        // Can't move if character is confused (`isOutOfControl`): confusion and fear are not
        // modelled yet.

        // Can't move if character is trying to move a huge distance.
        let (px, py) = self.position_of(id).map_or((px, py), |(x, y, _)| (x, y));
        let dx = f64::from(target_x.wrapping_sub(px));
        let dy = f64::from(target_y.wrapping_sub(py));
        if dx * dx + dy * dy > 98_010_000.0 {
            self.send(id, &ActionFailed);
            return;
        }

        // Finally move to the target location.
        self.set_intention_move_to(id, target_x, target_y, target_z);
        // Java `player.onActionRequest()`: spawn protection is not modelled.
    }

    /// Java `ValidatePosition.runImpl`.
    fn on_validate_position(&mut self, id: ObjectId, x: i32, y: i32, z: i32, heading: i32) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        // Java also returns for observer mode, casting and vehicles; none exists yet.
        if player.movement.teleporting {
            return;
        }
        let real_x = player.core.x;
        if x == 0 && y == 0 && real_x != 0 {
            return;
        }
        if self.is_falling(id, z) {
            return; // Disable validations during fall to avoid "jumping".
        }

        // Abnormal z read from client.
        if !(-20_000..=20_000).contains(&z) {
            // Java: `teleToLocation(lastServerPosition)`.
            // TODO(M3-TELE-01): call the teleport once it exists; until then the report is only
            // logged.
            warn!(
                "{id}: client reported an abnormal z ({z}); teleport to the last server position is not ported yet."
            );
            return;
        }

        // Java: `isFlyingMounted() && x > GRACIA_MAX_X` -> untransform: no transformations yet.

        let Some(player) = self.players.get(&id) else {
            return;
        };
        let (real_y, mut real_z) = (player.core.y, player.core.z);
        let client_z = player.movement.client_z;
        let dx = x.wrapping_sub(real_x);
        let dy = y.wrapping_sub(real_y);
        let dz = z.wrapping_sub(real_z);
        let diff_sq = f64::from(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)));
        if self.is_in_water(id) {
            // Java: `isFlying() || isInsideZone(WATER)`.
            self.set_xyz(id, real_x, real_y, z);
            if diff_sq > 90_000.0 {
                self.send_validate_location(id);
            }
        } else if diff_sq < 360_000.0 {
            // If too large, messes observation.
            if diff_sq > 250_000.0 || dz.unsigned_abs() > 200 {
                if dz.unsigned_abs() > 200
                    && dz.unsigned_abs() < 1500
                    && z.wrapping_sub(client_z).unsigned_abs() < 800
                {
                    self.set_xyz(id, real_x, real_y, z);
                    real_z = z;
                } else {
                    self.send_validate_location(id);
                }
            }
        }

        // Check out of sync.
        let move_speed = self.move_speed(id);
        if self.distance_3d_to(id, x, y, z) > move_speed {
            // Java: `isBlinkActive()` (a skill effect) delays the correction; not modelled.
            let new_z = if real_z > z {
                self.geo_height(x, y, real_z)
            } else {
                z
            };
            self.set_xyz(id, x, y, new_z);
        }

        let instance = self.instance_id_of(id);
        let cursor = {
            let Some(p) = self.players.get_mut(&id) else {
                return;
            };
            p.movement.client_x = x;
            p.movement.client_y = y;
            p.movement.client_z = z;
            p.movement.client_heading = heading; // No real need to validate heading.
            p.movement.cursor_key_movement
        };

        // Mobius: Check for possible door logout and move over exploit. Also checked at MoveToLocation.
        if !self.doors_between(
            Location::new(real_x, real_y, real_z),
            Location::new(x, y, z),
            instance,
            false,
        ) && let Some(p) = self.players.get_mut(&id)
        {
            p.movement.last_server_position = (real_x, real_y, real_z);
        }

        if cursor {
            // Use setSyncedXYZ so the active MoveData's xAccurate/yAccurate are updated too.
            self.set_synced_xyz(id, x, y, z);
            self.broadcast_validate_location(id);
        }
    }

    // ------------------------------------------------------------------ the move intention (AI)

    /// Java `PlayerAI.setIntentionMoveTo`.
    pub(crate) fn set_intention_move_to(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if player.movement.intention == Intention::Rest {
            self.send(id, &ActionFailed);
            return;
        }
        // Java also refuses while the duel state is "dead", while all skills are disabled, while
        // casting and while attacking (saving the move for later): none of it exists yet.

        // Java `stopFollow()`, `clearSavedIntention()`, `clientStopAutoAttack()`, `abortAttack()`
        // have nothing to do without followers and combat.
        player.movement.intention = Intention::MoveTo;
        self.ai_move_to(id, x, y, z);
    }

    /// Java `AbstractAI.moveTo(x, y, z)`.
    fn ai_move_to(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        if self.is_movement_disabled(id) {
            self.send(id, &ActionFailed);
            return;
        }
        match self.move_to_location(id, x, y, z, 0) {
            MoveStep::Finished => self.broadcast_move_to_location(id, false),
            MoveStep::WaitingForPath => {}
        }
    }

    /// Java `PlayerAI.setIntentionIdle` (`setIntentionActive` is the same for players).
    pub(crate) fn set_intention_idle(&mut self, id: ObjectId) {
        if let Some(p) = self.players.get_mut(&id) {
            p.movement.intention = Intention::Idle;
        }
        // A new intention supersedes a move that still waits for its path.
        self.clear_pending_path(id);
        self.client_stop_moving(id, None);
    }

    /// Java `AbstractAI.clientStopMoving`: only stops a creature that moves.
    pub(crate) fn client_stop_moving(&mut self, id: ObjectId, loc: Option<(i32, i32, i32, i32)>) {
        if self.is_moving(id) {
            self.stop_move(id, loc);
        }
    }

    /// Java `CreatureAI.notifyActionArrived`.
    pub(crate) fn notify_action_arrived(&mut self, id: ObjectId) {
        // Java `canHandleAction`: spawned (or teleporting) and has an AI.
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        let teleporting = self.players.get(&id).is_some_and(|p| p.movement.teleporting);
        if !self.players.contains_key(&id) || !(spawned || teleporting) {
            return;
        }
        // Casting is not modelled; `revalidateZone(true)` neither (zones).
        if self.move_to_next_route_point(id) {
            return;
        }
        // `clientStoppedMoving` only matters after `MoveToPawn`.
        // If the Intention was MOVE_TO, set the Intention to ACTIVE.
        if self
            .players
            .get(&id)
            .is_some_and(|p| p.movement.intention == Intention::MoveTo)
        {
            self.set_intention_idle(id);
        }
        // `notifyActionThink` does nothing for a player without attack/cast/pick-up/interact.
    }

    /// Java `CreatureAI.notifyActionArrivedBlocked` (the client's `CannotMoveAnymore`).
    pub(crate) fn notify_action_arrived_blocked(&mut self, id: ObjectId, loc: (i32, i32, i32, i32)) {
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        let teleporting = self.players.get(&id).is_some_and(|p| p.movement.teleporting);
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if !(spawned || teleporting) {
            return;
        }
        // If the Intention was MOVE_TO (or CAST), set the Intention to ACTIVE.
        if player.movement.intention == Intention::MoveTo {
            self.set_intention_idle(id);
        }
        // Stop the actor movement server side AND client side.
        self.client_stop_moving(id, Some(loc));
    }

    // ------------------------------------------------------------------ creature movement

    /// Java `Creature.stopMove(loc)`. Does not send anything but the `StopMove` broadcast.
    pub fn stop_move(&mut self, id: ObjectId, loc: Option<(i32, i32, i32, i32)>) {
        self.clear_pending_path(id);
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        // Delete movement data of the Creature.
        player.movement.mv = None;
        if let Some((x, y, z, heading)) = loc {
            self.set_xyz(id, x, y, z);
            if let Some(p) = self.players.get_mut(&id) {
                p.core.heading = heading;
            }
            // `revalidateZone(true)`: zones are not modelled.
        }
        let Some(p) = self.players.get(&id) else {
            return;
        };
        let packet = StopMove {
            object_id: id.0,
            x: p.core.x,
            y: p.core.y,
            z: p.core.z,
            heading: p.core.heading,
        };
        self.broadcast_with_self(id, &packet);
    }

    /// Java `Creature.setXYZ` (the override): `(0, 0)` and positions outside the world are
    /// ignored. The region switch is [`World::set_position`].
    pub(crate) fn set_xyz(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        // 0, 0 is not a valid location.
        if x == 0 && y == 0 {
            return;
        }
        // Mobius: Prevent moving to nonexistent regions.
        if !(WORLD_X_MIN..=WORLD_X_MAX).contains(&x) || !(WORLD_Y_MIN..=WORLD_Y_MAX).contains(&y) {
            return;
        }
        self.set_position(id, x, y, z);
    }

    /// Java `Creature.setSyncedXYZ`: adopt a client position and keep the running move's accurate
    /// coordinates in step.
    pub(crate) fn set_synced_xyz(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        self.set_xyz(id, x, y, z);
        if let Some(mv) = self.players.get_mut(&id).and_then(|p| p.movement.mv.as_mut()) {
            mv.x_accurate = f64::from(x);
            mv.y_accurate = f64::from(y);
        }
    }

    fn register_moving_object(&mut self, id: ObjectId) {
        if !self.moving.contains(&id) {
            self.moving.push(id);
        }
    }

    /// The `MovementTaskManager` run: every moving creature, once per world tick.
    pub(crate) fn update_movement(&mut self) {
        if self.moving.is_empty() {
            return;
        }
        let ids = self.moving.clone();
        for id in ids {
            if !self.moving.contains(&id) {
                continue;
            }
            if self.update_position(id) {
                self.moving.retain(|m| *m != id);
                self.notify_action_arrived(id);
            }
        }
    }

    /// Java `Creature.updatePosition()` for a player. `true` when the movement is finished.
    fn update_position(&mut self, id: ObjectId) -> bool {
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        let game_ticks = self.game_ticks();
        let speed = self.move_speed(id);
        let instance = self.instance_id_of(id);
        let Some(player) = self.players.get_mut(&id) else {
            return true;
        };
        if !spawned {
            player.movement.mv = None;
            return true;
        }
        let Some(mut mv) = player.movement.mv.take() else {
            return true;
        };
        let (x_prev, y_prev, z_prev) = (player.core.x, player.core.y, player.core.z);
        let heading = player.core.heading;
        let cursor = player.movement.cursor_key_movement;
        let collision = f64::from(player.stats.collision_radius);

        // Check if this is the first update.
        if mv.move_timestamp == 0 {
            mv.move_timestamp = mv.move_start_time;
            mv.x_accurate = f64::from(x_prev);
            mv.y_accurate = f64::from(y_prev);
        }

        // Check if the position has already been calculated.
        if mv.move_timestamp == game_ticks {
            player.movement.mv = Some(mv);
            return false;
        }

        let dx = f64::from(mv.x_destination) - mv.x_accurate;
        let dy = f64::from(mv.y_destination) - mv.y_accurate;
        let dz = f64::from(mv.z_destination.wrapping_sub(z_prev)); // Z coordinate will follow client values.

        // Java: `isPlayer() && !_isFlying`.
        if cursor {
            // In case of cursor movement, avoid moving through obstacles.
            let (x, y) = front_probe(x_prev, y_prev, heading, speed);
            if !self.geo_can_move_to_target(
                Location::new(x_prev, y_prev, z_prev),
                Location::new(x, y, z_prev),
                instance,
            ) {
                // Stop at the current position, not lastServerPosition (which is where the move
                // started).
                mv.on_geodata_path_index = -1;
                self.put_move(id, mv);
                self.stop_move(id, Some((x_prev, y_prev, z_prev, heading)));
                return true;
            }
        } else {
            // Mouse click movement.
            // Stop movement when player has clicked far away and intersected with an obstacle.
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 3000.0 {
                let (x, y) = front_probe(x_prev, y_prev, heading, speed);
                if !self.geo_can_move_to_target(
                    Location::new(x_prev, y_prev, z_prev),
                    Location::new(x, y, z_prev),
                    instance,
                ) {
                    mv.on_geodata_path_index = -1;
                    self.put_move(id, mv);
                    self.set_intention_idle(id);
                    return true;
                }
            } else if self.region_has_doors_or_fences(id)
                && self.doors_between(
                    Location::new(x_prev, y_prev, z_prev),
                    {
                        let (x, y) = front_probe(x_prev, y_prev, heading, speed);
                        Location::new(x, y, z_prev)
                    },
                    instance,
                    false,
                )
            {
                // Check for nearby doors or fences.
                mv.on_geodata_path_index = -1;
                self.put_move(id, mv);
                self.set_intention_idle(id);
                self.stop_move(id, None);
                return true;
            }
            // Java's `intention == ATTACK` direct-move probe: no attacking yet.
        }

        // Distance from destination.
        let mut delta = dx * dx + dy * dy;
        let is_floating = self.is_in_water(id);
        if !is_floating && delta < 10_000.0 && dz * dz > 2500.0 {
            // Close enough, allows error between client and server geodata if it cannot be avoided.
            delta = delta.sqrt();
        } else {
            delta = (delta + dz * dz).sqrt();
        }

        // Target collision should be subtracted from current distance (only when attacking a
        // target; this is the creature's own radius).
        delta = (delta - collision).max(0.000_01);

        let mut dist_fraction = f64::MAX;
        if delta > 1.0 {
            let dist_passed =
                (speed * f64::from(game_ticks.wrapping_sub(mv.move_timestamp))) / f64::from(TICKS_PER_SECOND);
            dist_fraction = dist_passed / delta;
        }

        let arrived = dist_fraction > 1.0;
        if arrived {
            // Set the position of the Creature to the destination.
            self.set_position(id, mv.x_destination, mv.y_destination, mv.z_destination);
        } else {
            let new_z = z_prev.wrapping_add(((dz * dist_fraction) + 0.5) as i32);
            mv.x_accurate += dx * dist_fraction;
            mv.y_accurate += dy * dist_fraction;
            // Set the position of the Creature to estimated after partial move.
            self.set_position(id, mv.x_accurate as i32, mv.y_accurate as i32, new_z);
        }

        // `revalidateZone(false)`: zones are not modelled.

        // Set the timer of last position update to now.
        mv.move_timestamp = game_ticks;
        let on_path = mv.is_on_geodata_path();
        self.put_move(id, mv);

        // Broadcast MoveToLocation on arrived.
        if arrived && !on_path {
            self.broadcast_move_to_location(id, true);
        }
        arrived
    }

    fn put_move(&mut self, id: ObjectId, mv: MoveData) {
        if let Some(p) = self.players.get_mut(&id) {
            p.movement.mv = Some(mv);
        }
    }

    /// Java `Creature.moveToNextRoutePoint()`: on to the next node of the calculated path.
    fn move_to_next_route_point(&mut self, id: ObjectId) -> bool {
        let speed = self.move_speed(id);
        let disabled = self.is_movement_disabled(id);
        let game_ticks = self.game_ticks();
        let Some(player) = self.players.get_mut(&id) else {
            return false;
        };
        let Some(mv) = player.movement.mv.as_ref() else {
            return false;
        };
        if !mv.is_on_geodata_path() {
            // Cancel the move action.
            player.movement.mv = None;
            return false;
        }
        if speed <= 0.0 || disabled {
            // Cancel the move action.
            player.movement.mv = None;
            return false;
        }
        let (cur_x, cur_y) = (player.core.x, player.core.y);
        let Some(path) = mv.geo_path.clone() else {
            return false;
        };
        let old_index = mv.on_geodata_path_index;

        // Update MoveData object.
        let mut new_move = MoveData::new();
        new_move.on_geodata_path_index = old_index + 1; // next segment
        new_move.geo_path_gtx = mv.geo_path_gtx;
        new_move.geo_path_gty = mv.geo_path_gty;
        new_move.geo_path_accurate_tx = mv.geo_path_accurate_tx;
        new_move.geo_path_accurate_ty = mv.geo_path_accurate_ty;
        let Some(next) = usize::try_from(new_move.on_geodata_path_index)
            .ok()
            .and_then(|i| path.get(i))
        else {
            player.movement.mv = None;
            return false;
        };
        if usize::try_from(old_index).is_ok_and(|i| i + 2 == path.len()) {
            new_move.x_destination = mv.geo_path_accurate_tx;
            new_move.y_destination = mv.geo_path_accurate_ty;
            new_move.z_destination = next.z();
        } else {
            new_move.x_destination = next.x();
            new_move.y_destination = next.y();
            new_move.z_destination = next.z();
        }
        new_move.geo_path = Some(path);

        // Calculate and set the heading of the Creature.
        let ndx = f64::from(new_move.x_destination.wrapping_sub(cur_x));
        let ndy = f64::from(new_move.y_destination.wrapping_sub(cur_y));
        let distance = (ndx * ndx + ndy * ndy).sqrt();
        if distance != 0.0 {
            player.core.heading =
                heading_from_points(cur_x, cur_y, new_move.x_destination, new_move.y_destination);
        }

        // (The `ticksToMove > 3000 ms` revalidate task only makes the AI think; players don't.)
        new_move.heading = 0; // Initial value for coordinate sync.
        new_move.move_start_time = game_ticks;
        player.movement.mv = Some(new_move);
        self.register_moving_object(id);

        // Send a Server->Client packet MoveToLocation to the actor and all Player in its known players.
        self.broadcast_move_to_location(id, true);
        true
    }

    /// Java `Creature.broadcastMoveToLocation(force)` for a player.
    pub(crate) fn broadcast_move_to_location(&mut self, id: ObjectId, force: bool) {
        let game_ticks = self.game_ticks();
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        let cursor = p.movement.cursor_key_movement;
        let (cx, cy, cz) = (p.core.x, p.core.y, p.core.z);
        let Some(mv) = p.movement.mv.as_mut() else {
            return;
        };
        // Broadcast MoveToLocation when forced or once per second.
        if !force && !cursor && mv.move_timestamp > 0 && game_ticks.wrapping_sub(mv.last_broadcast_time) < 10
        {
            return;
        }
        mv.last_broadcast_time = game_ticks;
        let packet = MoveToLocation {
            object_id: id.0,
            dest_x: mv.x_destination,
            dest_y: mv.y_destination,
            dest_z: mv.z_destination,
            x: cx,
            y: cy,
            z: cz,
        };
        self.broadcast_with_self(id, &packet);
    }

    // ------------------------------------------------------------------ moveToLocation

    /// Java `Creature.moveToLocation(x, y, z, offset)` for a player.
    pub(crate) fn move_to_location(&mut self, id: ObjectId, x: i32, y: i32, z: i32, offset: i32) -> MoveStep {
        let Some((calc, need_path)) = self.prepare_move(id, x, y, z, offset) else {
            return MoveStep::Finished;
        };
        if !need_path {
            self.complete_move(id, calc, None);
            return MoveStep::Finished;
        }
        match self.submit_path_request(id, &calc) {
            Some(request_id) => {
                if let Some(p) = self.players.get_mut(&id) {
                    p.movement.pending = Some(PendingPath { request_id, calc });
                }
                MoveStep::WaitingForPath
            }
            None => {
                // No pool, or it refused: the same as Java finding no path.
                self.complete_move(id, calc, Some(None));
                MoveStep::Finished
            }
        }
    }

    /// The first half of `moveToLocation`, up to the path search. `None`: Java returned early
    /// (everything it sends has been sent). Otherwise the locals and whether a path is needed.
    fn prepare_move(
        &mut self,
        id: ObjectId,
        x_value: i32,
        y_value: i32,
        z_value: i32,
        offset_value: i32,
    ) -> Option<(MoveCalc, bool)> {
        // Get the Move Speed of the Creature.
        let speed = self.move_speed(id);
        if speed <= 0.0 || self.is_movement_disabled(id) {
            self.send(id, &ActionFailed);
            return None;
        }

        let (mut x, mut y, mut z) = (x_value, y_value, z_value);
        let mut offset = offset_value;

        // Get current position of the Creature.
        let (cur_x, cur_y, cur_z) = self.position_of(id)?;
        let instance = self.instance_id_of(id);

        // Calculate distance (dx,dy) between current position and destination.
        let mut dx = f64::from(x.wrapping_sub(cur_x));
        let mut dy = f64::from(y.wrapping_sub(cur_y));
        let mut dz = f64::from(z.wrapping_sub(cur_z));
        let mut distance = (dx * dx + dy * dy).sqrt();

        // Make water move short and use no geodata checks for swimming chars.
        let in_water = self.is_in_water(id);
        if in_water && distance > 700.0 {
            let divider = 700.0 / distance;
            x = cur_x.wrapping_add((divider * dx) as i32);
            y = cur_y.wrapping_add((divider * dy) as i32);
            z = cur_z.wrapping_add((divider * dz) as i32);
            dx = f64::from(x.wrapping_sub(cur_x));
            dy = f64::from(y.wrapping_sub(cur_y));
            dz = f64::from(z.wrapping_sub(cur_z));
            distance = (dx * dx + dy * dy).sqrt();
        }

        let (cos, sin);

        // Check if a movement offset is defined or no distance to go through.
        if offset > 0 || distance < 1.0 {
            // approximation for moving closer when z coordinates are different
            offset = (f64::from(offset) - dz.abs()) as i32;
            if offset < 5 {
                offset = 5;
            }

            // If no distance to go through, the movement is canceled.
            if distance < 1.0 || (distance - f64::from(offset)) <= 0.0 {
                // Notify the AI that the Creature is arrived at destination.
                self.notify_action_arrived(id);
                self.send(id, &ActionFailed);
                return None;
            }

            // Calculate movement angles needed.
            sin = dy / distance;
            cos = dx / distance;
            distance -= f64::from(offset - 5); // Due to rounding error, we have to move a bit closer to be in range.

            // Calculate the new destination with offset included.
            x = cur_x.wrapping_add((distance * cos) as i32);
            y = cur_y.wrapping_add((distance * sin) as i32);
        } else {
            // Calculate movement angles needed.
            sin = dy / distance;
            cos = dx / distance;
        }

        // Create and Init a MoveData object.
        let mut mv = MoveData::new();

        // GEODATA MOVEMENT CHECKS AND PATHFINDING
        let region_active = self.are_region_neighbors_active(id);
        mv.disregarding_geodata = !region_active;
        mv.on_geodata_path_index = -1; // Initialize not on geodata path.
        let cursor = self
            .players
            .get(&id)
            .is_some_and(|p| p.movement.cursor_key_movement);
        let mut original = (x, y, z);
        let mut original_distance = distance;
        let (mut gtx, mut gty) = (0, 0);
        let mut need_path = false;
        let geo_checks = !mv.disregarding_geodata && !in_water && !cursor;
        if geo_checks {
            // Movement checks.
            if self.pathfinding_enabled() {
                original = (x, y, z);
                original_distance = distance;
                gtx = original.0.wrapping_sub(WORLD_X_MIN) >> 4;
                gty = original.1.wrapping_sub(WORLD_Y_MIN) >> 4;
                if let Some(current) = self.players.get_mut(&id).and_then(|p| p.movement.mv.as_mut())
                    && current.is_on_geodata_path()
                {
                    if gtx == current.geo_path_gtx && gty == current.geo_path_gty {
                        self.send(id, &ActionFailed);
                        return None;
                    }
                    current.on_geodata_path_index = -1; // Set not on geodata path.
                }

                // Player attack with direct movement (`Intention.ATTACK`): no attacking yet.
                let direct_move = false;
                let destination_in_water = self.is_water_at(x, y, z);
                if direct_move
                    || (!destination_in_water
                        && distance <= 3000.0 // Should be able to click far away and move.
                        && !(cur_z.wrapping_sub(z) > 300 && distance < 300.0))
                // Prohibit correcting destination if character wants to fall.
                {
                    // Location is different if destination wasn't reached (or just z coord is different).
                    let destination = self.geo_valid_location(
                        Location::new(cur_x, cur_y, cur_z),
                        Location::new(x, y, z),
                        instance,
                    );
                    x = destination.x;
                    y = destination.y;
                    // Players keep the z they asked for (`if (!isPlayer()) z = destination.getZ()`).
                    dx = f64::from(x.wrapping_sub(cur_x));
                    dy = f64::from(y.wrapping_sub(cur_y));
                    dz = f64::from(z.wrapping_sub(cur_z));
                    distance = (dx * dx + dy * dy).sqrt();
                }

                // Pathfinding checks. (`isControlBlocked` is fear: not modelled.)
                if !direct_move && (original_distance - distance) > 30.0 {
                    need_path = true;
                }
            }
        }

        let calc = MoveCalc {
            x,
            y,
            z,
            cur_x,
            cur_y,
            cur_z,
            dx,
            dy,
            dz,
            distance,
            cos,
            sin,
            mv,
            original_x: original.0,
            original_y: original.1,
            original_z: original.2,
            original_distance,
            gtx,
            gty,
            geo_checks,
        };
        Some((calc, need_path))
    }

    /// Sends the path request. `Some(request id)` when it is out.
    fn submit_path_request(&mut self, id: ObjectId, calc: &MoveCalc) -> Option<u64> {
        let instance = self.instance_id_of(id);
        let Some(sink) = self.path_requests.as_mut() else {
            if !self.warned_no_path_service {
                self.warned_no_path_service = true;
                warn!(
                    "Pathfinding is on but no path service is installed: moves go straight to their destination."
                );
            }
            return None;
        };
        self.next_path_request_id += 1;
        let request_id = self.next_path_request_id;
        let request = PathRequest::new(
            requester_of(id),
            request_id,
            Location::new(calc.cur_x, calc.cur_y, calc.cur_z),
            Location::new(calc.original_x, calc.original_y, calc.original_z),
            obstacle_snapshot(),
        )
        .with_instance(instance)
        .with_playable(true);
        if sink.request(request) {
            Some(request_id)
        } else {
            debug!("{id}: the path service refused the request");
            None
        }
    }

    /// Drops the pending path search of `id`, if any.
    pub(crate) fn clear_pending_path(&mut self, id: ObjectId) {
        let had = self
            .players
            .get_mut(&id)
            .is_some_and(|p| p.movement.pending.take().is_some());
        if had && let Some(sink) = self.path_requests.as_mut() {
            sink.cancel(requester_of(id));
        }
    }

    /// `WorldCommand::PathFound`: the rest of the move the search was for. A result nobody waits
    /// for any more (newer request, stop, left the world) is dropped.
    pub(crate) fn on_path_found(&mut self, requester: u64, request_id: u64, result: PathResult) {
        let requester = object_of_requester(requester);
        let Some(player) = self.players.get_mut(&requester) else {
            return;
        };
        match player.movement.pending.take() {
            Some(pending) if pending.request_id == request_id => {
                let path = result.path.ok();
                self.complete_move(requester, pending.calc, Some(path));
                if let Some(p) = self.players.get_mut(&requester) {
                    p.movement.intention = Intention::MoveTo;
                }
                self.broadcast_move_to_location(requester, false);
            }
            other => {
                // Not the request this player waits for (or none): put a newer one back.
                player.movement.pending = other;
                debug!("{requester}: stale path result {request_id} dropped");
            }
        }
    }

    /// The second half of `moveToLocation`. `path` is `None` when no search was made, `Some(None)`
    /// for Java's `null` path and `Some(Some(nodes))` for a found one.
    fn complete_move(&mut self, id: ObjectId, mut calc: MoveCalc, path: Option<Option<Vec<GeoLocation>>>) {
        let instance = self.instance_id_of(id);
        if let Some(found_path) = path {
            // Path calculation -- overrides previous movement check.
            calc.mv.geo_path = found_path;
            let found = calc.mv.geo_path.as_ref().is_some_and(|p| p.len() > 1);
            if let (true, Some(first)) = (found, calc.mv.geo_path.as_ref().and_then(|p| p.first())) {
                calc.mv.on_geodata_path_index = 0; // On first segment.
                calc.mv.geo_path_gtx = calc.gtx;
                calc.mv.geo_path_gty = calc.gty;
                calc.mv.geo_path_accurate_tx = calc.original_x;
                calc.mv.geo_path_accurate_ty = calc.original_y;
                calc.x = first.x();
                calc.y = first.y();
                calc.z = first.z();
                calc.dx = f64::from(calc.x.wrapping_sub(calc.cur_x));
                calc.dy = f64::from(calc.y.wrapping_sub(calc.cur_y));
                calc.dz = f64::from(calc.z.wrapping_sub(calc.cur_z));
                calc.distance = (calc.dx * calc.dx + calc.dy * calc.dy).sqrt();
                calc.sin = calc.dy / calc.distance;
                calc.cos = calc.dx / calc.distance;
            } else {
                // No path found.
                // Java (retail test, October 2024): use direct movement.
                calc.mv.disregarding_geodata = true;
                calc.x = calc.original_x;
                calc.y = calc.original_y;
                calc.z = calc.original_z;
                calc.distance = calc.original_distance;
            }
        }

        if calc.geo_checks {
            // Verify destination when using mouse movement and no path is found.
            // NOTE(java-quirk): a path with fewer than two nodes still counts as "a path" here
            // (`geoPath == null` fails), so the destination is not corrected in that case.
            let cursor = self
                .players
                .get(&id)
                .is_some_and(|p| p.movement.cursor_key_movement);
            if !cursor
                && calc.mv.geo_path.is_none()
                && calc.distance < 3000.0 // Should be able to click far away and move.
                && !(calc.cur_z.wrapping_sub(calc.z) > 300 && calc.distance < 300.0)
            // Prohibit correcting destination if character wants to fall.
            {
                let destination = self.geo_valid_location(
                    Location::new(calc.cur_x, calc.cur_y, calc.cur_z),
                    Location::new(calc.x, calc.y, calc.z),
                    instance,
                );
                calc.x = destination.x;
                calc.y = destination.y;
                calc.z = destination.z;
                calc.dx = f64::from(calc.x.wrapping_sub(calc.cur_x));
                calc.dy = f64::from(calc.y.wrapping_sub(calc.cur_y));
                calc.dz = f64::from(calc.z.wrapping_sub(calc.cur_z));
                calc.distance = (calc.dx * calc.dx + calc.dy * calc.dy).sqrt();
            }

            // If no distance to go through, the movement is canceled.
            if calc.distance < 1.0 {
                // Java: `(PATHFINDING > 0) || isPlayable()`; a player is always playable.
                // Not attacking or casting, so the intention goes idle.
                self.set_intention_idle(id);
                self.send(id, &ActionFailed);
                return;
            }
        }

        // (Flying and swimming add the z distance for the timing here; neither exists yet.)
        // Calculate the number of ticks between the current position and the destination: only
        // used for the AI revalidate task, which does nothing for a player.
        let MoveCalc {
            x,
            y,
            z,
            cos,
            sin,
            mut mv,
            ..
        } = calc;
        mv.x_destination = x;
        mv.y_destination = y;
        mv.z_destination = z; // This is what was requested from client.

        // Calculate and set the heading of the Creature.
        mv.heading = 0; // Initial value for coordinate sync.
        mv.move_start_time = self.game_ticks();

        self.clear_pending_path(id);
        if let Some(p) = self.players.get_mut(&id) {
            p.core.heading = heading_from_delta(cos, sin);
            // Set the Creature _move object to MoveData object.
            p.movement.mv = Some(mv);
        }
        // Add the Creature to moving objects of the MovementTaskManager.
        self.register_moving_object(id);
    }

    // ------------------------------------------------------------------ helpers

    /// Java `PlayerStat.getMoveSpeed()`: the run or walk speed of the stats. `DerivedStats` holds
    /// the template speed and the multiplier (`speed * multiplier` is what Java's `calcStat`
    /// returns).
    // NOTE: buffs and the GM speed multiplier arrive with the effect system (M5).
    pub fn move_speed(&self, id: ObjectId) -> f64 {
        self.players.get(&id).map_or(0.0, |p| {
            let base = if p.movement.running {
                p.stats.run_speed
            } else {
                p.stats.walk_speed
            };
            f64::from(base) * p.stats.move_multiplier
        })
    }

    /// Java `Player.isOverloaded()`: carrying more than the maximum load.
    fn is_overloaded(&self, player: &Player) -> bool {
        let max_load = player.stats.max_load;
        max_load > 0 && current_load(&player.data, &*self.data) > max_load
    }

    /// Java `Creature.isMovementDisabled()` (stun, root, sleep, paralysis and immobilization are
    /// effects: M5).
    fn is_movement_disabled(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_none_or(|p| self.is_overloaded(p) || p.core.cur_hp < 0.5 || p.movement.teleporting)
    }

    /// Java `isInsideZone(ZoneId.WATER) && !isInsideZone(ZoneId.CASTLE)`. Zones are not modelled
    /// yet, so nobody is in water.
    fn is_in_water(&self, _id: ObjectId) -> bool {
        false
    }

    /// Is there a water zone at this point (`ZoneManager.getZone(x, y, z, WaterZone.class)`)?
    fn is_water_at(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }

    /// Java `DoorData.checkIfDoorsBetween(...)`. M3-DOOR-01 answers it from the door table.
    fn doors_between(
        &self,
        _from: Location,
        _to: Location,
        _instance: i32,
        _double_face_check: bool,
    ) -> bool {
        false
    }

    /// `!region.getDoors().isEmpty() || !region.getFences().isEmpty()` (M3-DOOR-01).
    fn region_has_doors_or_fences(&self, _id: ObjectId) -> bool {
        false
    }

    fn instance_id_of(&self, id: ObjectId) -> i32 {
        self.objects.get(&id).map_or(0, |e| e.instance_id)
    }

    fn pathfinding_enabled(&self) -> bool {
        self.geo.as_ref().is_some_and(|g| g.options().pathfinding > 0)
    }

    fn geo_valid_location(&self, from: Location, to: Location, instance: i32) -> Location {
        match &self.geo {
            Some(geo) => geo.valid_location(from, to, instance),
            None => to,
        }
    }

    fn geo_can_move_to_target(&self, from: Location, to: Location, instance: i32) -> bool {
        match &self.geo {
            Some(geo) => geo.can_move_to_target(from, to, instance),
            None => true,
        }
    }

    fn geo_height(&self, x: i32, y: i32, z: i32) -> i32 {
        match &self.geo {
            Some(geo) => geo.height(x, y, z),
            None => z,
        }
    }

    fn is_completely_blocked(&self, geo_x: i32, geo_y: i32, z: i32) -> bool {
        self.geo
            .as_ref()
            .is_some_and(|geo| geo.is_completely_blocked(geo_x, geo_y, z))
    }

    /// Java `calculateDistance3D(x, y, z)` from the player.
    fn distance_3d_to(&self, id: ObjectId, x: i32, y: i32, z: i32) -> f64 {
        let Some((px, py, pz)) = self.position_of(id) else {
            return 0.0;
        };
        let dx = f64::from(x.wrapping_sub(px));
        let dy = f64::from(y.wrapping_sub(py));
        let dz = f64::from(z.wrapping_sub(pz));
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    fn send_validate_location(&self, id: ObjectId) {
        if let Some(p) = self.players.get(&id) {
            self.send(
                id,
                &ValidateLocation {
                    object_id: id.0,
                    x: p.core.x,
                    y: p.core.y,
                    z: p.core.z,
                    heading: p.core.heading,
                },
            );
        }
    }

    fn broadcast_validate_location(&self, id: ObjectId) {
        if let Some(p) = self.players.get(&id) {
            let packet = ValidateLocation {
                object_id: id.0,
                x: p.core.x,
                y: p.core.y,
                z: p.core.z,
                heading: p.core.heading,
            };
            self.broadcast_with_self(id, &packet);
        }
    }

    /// Java `Player.broadcastPacket`: the player itself first, then everybody who sees it.
    pub(crate) fn broadcast_with_self(&self, id: ObjectId, packet: &impl Encode) {
        self.send(id, packet);
        self.broadcast_packet(id, packet);
    }

    /// Java `Player.isFalling(z)`: `true` while the client is falling (its z reports are not
    /// trusted). Starts the fall window when the drop is more than the class can take.
    fn is_falling(&mut self, id: ObjectId, z: i32) -> bool {
        let now = self.world_ms();
        let Some(player) = self.players.get(&id) else {
            return false;
        };
        if player.core.cur_hp < 0.5 || self.is_in_water(id) {
            return false;
        }
        if player.movement.falling_timestamp_ms != 0 && now < player.movement.falling_timestamp_ms {
            return true;
        }
        let delta_z = player.core.z.wrapping_sub(z);
        let (px, py) = (player.core.x, player.core.y);
        let safe_fall = self.data.safe_fall_height(&player.data);
        let has_geo = self.geo.as_ref().is_some_and(|g| g.has_geo(px, py));
        if delta_z <= safe_fall || !has_geo {
            // If there is no geodata loaded for the place we are, client Z correction might cause
            // falling damage.
            if let Some(p) = self.players.get_mut(&id) {
                p.movement.falling_timestamp_ms = 0;
            }
            return false;
        }
        // TODO(M5): Java schedules the falling damage here (`Formulas.calcFallDam`,
        // `reduceCurrentHp` after 1.5 s, `YOU_RECEIVED_S1_FALLING_DAMAGE`) once HP is modelled.

        // Prevent falling under ground.
        self.send_validate_location(id);
        if let Some(p) = self.players.get_mut(&id) {
            p.movement.falling_timestamp_ms = now + FALLING_VALIDATION_DELAY_MS;
        }
        false
    }
}

/// The door and fence view sent with a path request. M3-DOOR-01 takes a snapshot of the world's
/// door table here.
fn obstacle_snapshot() -> Arc<dyn DynamicObstacles> {
    Arc::new(NoObstacles)
}

/// The requester number of a path request (the object id).
pub fn requester_of(id: ObjectId) -> u64 {
    u64::from(id.0.unsigned_abs())
}

/// The inverse of [`requester_of`].
pub fn object_of_requester(requester: u64) -> ObjectId {
    ObjectId(i32::try_from(requester).unwrap_or(0))
}
