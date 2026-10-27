//! How a vehicle moves: `executePath`, the AI's `moveTo`, `Creature.moveToLocation` and
//! `updatePosition` as they run for a flying creature without geodata, `moveToNextRoutePoint`,
//! the stuck monitor, `teleToLocation` and the engine timer.
// Java: entity/actor/Vehicle.java (executePath, moveToNextRoutePoint, updatePosition, stopMove,
//       teleToLocation, runEngine)
// Java: entity/actor/instance/{Boat,AirShip}.java (moveToNextRoutePoint, stopMove)
// Java: entity/actor/Creature.java (moveToLocation, updatePosition, broadcastMoveToLocation,
//       onTeleported)
// Java: ai/{BoatAI,AirShipAI}.java (moveTo, clientStopMoving), ai/CreatureAI.java
//       (setIntentionMoveTo, setIntentionActive, notifyActionArrived)
// Java: taskmanagers/MovementTaskManager.java
//
// A vehicle is flying (`setFlying(true)`), has no geodata checks, no collision radius and is never
// in water, so the shared movement code (`crate::movement`) is not needed: this is the same
// arithmetic on the vehicle's own state. The tick follows `update_movement`: the position only
// advances when the 100 ms game tick changed.

use l2_protocol_game::server::{
    ExMoveToLocationAirShip, ExStopMoveAirShip, MoveToLocation, VehicleInfo, VehicleStarted,
};
use l2_util::ObjectId;

use super::route::PathPoint;
use super::{Engine, VIntention, VehicleKind, VehicleMove, VehicleTimer};
use crate::movement::{heading_from_delta, heading_from_points};
use crate::teleport::TeleportTarget;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `GameTimeTaskManager.TICKS_PER_SECOND`.
const TICKS_PER_SECOND: i32 = 10;
/// The `scheduleAtFixedRate(monitor, 1000, 1000)` of `moveToNextRoutePoint`.
const MONITOR_PERIOD_MS: u64 = 1000;

impl World {
    /// Java `Creature.isMovementDisabled()` of a vehicle: nothing stuns it, but a vehicle that is
    /// being teleported does not move.
    fn vehicle_movement_disabled(&self, id: ObjectId) -> bool {
        self.veh(id).is_none_or(|v| v.teleporting)
    }

    fn vehicle_set_heading(&mut self, id: ObjectId, heading: i32) {
        if let Some(v) = self.veh_mut(id) {
            v.heading = heading;
        }
    }

    /// Java `Vehicle.executePath(path)`.
    pub(crate) fn vehicle_execute_path(&mut self, id: ObjectId, path: Option<&'static [PathPoint]>) {
        let Some(v) = self.veh_mut(id) else {
            return;
        };
        v.run_state = 0;
        v.path = path;
        if let Some(point) = path.and_then(<[PathPoint]>::first).copied() {
            if point.move_speed > 0 {
                v.move_speed = point.move_speed as f32;
            }
            if point.rotation_speed > 0 {
                v.rotation_speed = point.rotation_speed;
            }
            self.vehicle_set_intention_move_to(id, point.x, point.y, point.z);
            return;
        }
        self.vehicle_set_intention_active(id);
    }

    /// Java `CreatureAI.setIntentionMoveTo(loc)` of a vehicle.
    pub(crate) fn vehicle_set_intention_move_to(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        let Some(v) = self.veh_mut(id) else {
            return;
        };
        v.intention = VIntention::MoveTo;
        self.vehicle_ai_move_to(id, x, y, z);
    }

    /// `BoatAI.moveTo` and `AirShipAI.moveTo`.
    fn vehicle_ai_move_to(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        if self.vehicle_movement_disabled(id) {
            return;
        }
        let Some(kind) = self.veh(id).map(|v| v.kind) else {
            return;
        };
        match kind {
            VehicleKind::Boat => {
                if !self.veh(id).is_some_and(|v| v.is_moving()) {
                    self.broadcast_packet(id, &VehicleStarted { object_id: id.0, state: 1 });
                }
                self.vehicle_move_to_location(id, x, y, z);
                if let Some(packet) = self.vehicle_departure_packet(id) {
                    self.broadcast_packet(id, &packet);
                }
            }
            VehicleKind::AirShip => {
                self.vehicle_move_to_location(id, x, y, z);
                if let Some(packet) = self.vehicle_move_packet(id) {
                    self.broadcast_packet(id, &packet);
                }
            }
        }
    }

    /// `new ExMoveToLocationAirShip(ship)`: the destination of the running move, or the place
    /// itself.
    fn vehicle_move_packet(&self, id: ObjectId) -> Option<ExMoveToLocationAirShip> {
        let v = self.veh(id)?;
        let dest = match &v.mv {
            Some(mv) => (mv.x_destination, mv.y_destination, mv.z_destination),
            None => (v.x, v.y, v.z),
        };
        self.ex_move_to_location_air_ship(id, dest)
    }

    /// Java `CreatureAI.setIntentionActive()` of a vehicle.
    pub(crate) fn vehicle_set_intention_active(&mut self, id: ObjectId) {
        let Some(v) = self.veh_mut(id) else {
            return;
        };
        if v.intention == VIntention::Active {
            return;
        }
        v.intention = VIntention::Active;
        // "Check if region and its neighbors are active."
        if !self.are_region_neighbors_active(id) {
            return;
        }
        self.vehicle_client_stop_moving(id, None);
    }

    /// `BoatAI.clientStopMoving(loc)` and `AirShipAI.clientStopMoving(loc)`.
    pub(crate) fn vehicle_client_stop_moving(&mut self, id: ObjectId, loc: Option<(i32, i32, i32, i32)>) {
        let Some(v) = self.veh(id) else {
            return;
        };
        let (kind, moving) = (v.kind, v.is_moving());
        if moving {
            self.vehicle_stop_move(id, loc);
            // NOTE(java-quirk): `stopMove` already broadcast these, the AI sends them again.
            self.vehicle_stop_packets(id, kind);
            return;
        }
        if loc.is_some() {
            self.vehicle_stop_packets(id, kind);
        }
    }

    /// What the AI sends when a vehicle stopped: `VehicleStarted(0)` and `VehicleInfo` for a boat,
    /// `ExStopMoveAirShip` for an air ship.
    fn vehicle_stop_packets(&self, id: ObjectId, kind: VehicleKind) {
        let Some(v) = self.veh(id) else {
            return;
        };
        match kind {
            VehicleKind::Boat => {
                self.broadcast_packet(id, &VehicleStarted { object_id: id.0, state: 0 });
                self.broadcast_packet(
                    id,
                    &VehicleInfo {
                        object_id: id.0,
                        x: v.x,
                        y: v.y,
                        z: v.z,
                        heading: v.heading,
                    },
                );
            }
            VehicleKind::AirShip => {
                self.broadcast_packet(
                    id,
                    &ExStopMoveAirShip {
                        object_id: id.0,
                        x: v.x,
                        y: v.y,
                        z: v.z,
                        heading: v.heading,
                    },
                );
            }
        }
    }

    /// Java `Vehicle.stopMove(loc)` with the `Boat` and `AirShip` overrides: the move is gone, the
    /// vehicle may be put at a place, and the players around are told.
    pub(crate) fn vehicle_stop_move(&mut self, id: ObjectId, loc: Option<(i32, i32, i32, i32)>) {
        let Some(kind) = self.veh(id).map(|v| v.kind) else {
            return;
        };
        if let Some(v) = self.veh_mut(id) {
            v.mv = None;
        }
        if let Some((x, y, z, heading)) = loc {
            self.set_xyz(id, x, y, z);
            self.vehicle_set_heading(id, heading);
            self.vehicle_revalidate_zone(id, true);
        }
        self.vehicle_stop_packets(id, kind);
    }

    /// Java `Creature.moveToLocation(x, y, z, 0)` for a flying creature without geodata.
    fn vehicle_move_to_location(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        let Some(v) = self.veh(id) else {
            return;
        };
        // Get the Move Speed of the Creature.
        let speed = v.current_move_speed();
        if speed <= 0.0 || self.vehicle_movement_disabled(id) {
            // `sendPacket(ActionFailed)` goes to nobody: a vehicle has no client.
            return;
        }
        let (cur_x, cur_y, cur_z) = (v.x, v.y, v.z);
        let dx = f64::from(x.wrapping_sub(cur_x));
        let dy = f64::from(y.wrapping_sub(cur_y));
        let dz = f64::from(z.wrapping_sub(cur_z));
        let mut distance = (dx * dx + dy * dy).sqrt();

        let vertical_movement_only = distance == 0.0 && dz != 0.0;
        if vertical_movement_only {
            distance = dz.abs();
        }

        // "If no distance to go through, the movement is canceled." (`offset` is 0 here: the
        // branch is `distance < 1`, with `offset` raised to 5.)
        if distance < 1.0 {
            // "Notify the AI that the Creature is arrived at destination."
            self.vehicle_notify_action_arrived(id);
            return;
        }
        let sin = dy / distance;
        let cos = dx / distance;

        // "Apply Z distance for flying or swimming for correct timing calculations."
        // (Only the timing of the revalidate task uses it, which a vehicle does not have.)

        // Calculate and set the heading of the Creature. "Does not break heading on vertical
        // movements."
        if !vertical_movement_only {
            self.vehicle_set_heading(id, heading_from_delta(cos, sin));
        }
        let game_ticks = self.game_ticks();
        if let Some(v) = self.veh_mut(id) {
            v.mv = Some(VehicleMove {
                move_start_time: game_ticks,
                move_timestamp: 0,
                x_destination: x,
                y_destination: y,
                z_destination: z,
                x_accurate: 0.0,
                y_accurate: 0.0,
                last_broadcast_time: 0,
            });
        }
        self.vehicle_register_moving(id);
    }

    fn vehicle_register_moving(&mut self, id: ObjectId) {
        if !self.vehicle.moving.contains(&id) {
            self.vehicle.moving.push(id);
        }
    }

    /// The `MovementTaskManager` run for the vehicles: once per world tick.
    pub(crate) fn update_vehicles(&mut self) {
        if self.vehicle.moving.is_empty() {
            return;
        }
        let ids = std::mem::take(&mut self.vehicle.moving);
        for id in ids {
            if self.vehicle_update_position(id) {
                // `iterator.remove()`, then `getAI().notifyActionArrived()`.
                self.vehicle_notify_action_arrived(id);
            } else {
                self.vehicle_register_moving(id);
            }
        }
    }

    /// Java `Vehicle.updatePosition()`: `Creature.updatePosition()` for a flying creature, then
    /// the passengers follow. `true` when the move is finished.
    fn vehicle_update_position(&mut self, id: ObjectId) -> bool {
        let finished = self.vehicle_update_position_base(id);
        // `for (Player player : _passengers) if (player.getVehicle() == this) setXYZ(...)`
        let Some(v) = self.veh(id) else {
            return finished;
        };
        let (x, y, z) = (v.x, v.y, v.z);
        for passenger in v.passengers.clone() {
            if self.vehicle_of(passenger) == Some(id) {
                // `setXYZ` revalidates the zones of a player (`revalidateZone(false)`).
                self.set_xyz(passenger, x, y, z);
            }
        }
        finished
    }

    /// Java `Creature.updatePosition()` of a flying creature.
    fn vehicle_update_position_base(&mut self, id: ObjectId) -> bool {
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        let game_ticks = self.game_ticks();
        let Some(v) = self.veh_mut(id) else {
            return true;
        };
        if !spawned {
            v.mv = None;
            return true;
        }
        let (x_prev, y_prev, z_prev) = (v.x, v.y, v.z);
        let Some(mut mv) = v.mv.take() else {
            return true;
        };

        // Check if this is the first update.
        if mv.move_timestamp == 0 {
            mv.move_timestamp = mv.move_start_time;
            mv.x_accurate = f64::from(x_prev);
            mv.y_accurate = f64::from(y_prev);
        }

        // Check if the position has already been calculated.
        if mv.move_timestamp == game_ticks {
            if let Some(v) = self.veh_mut(id) {
                v.mv = Some(mv);
            }
            return false;
        }

        let speed = self.veh(id).map_or(0.0, |v| v.current_move_speed());
        let dx = f64::from(mv.x_destination) - mv.x_accurate;
        let dy = f64::from(mv.y_destination) - mv.y_accurate;
        let dz = f64::from(mv.z_destination.wrapping_sub(z_prev));

        // Distance from destination. A vehicle is flying: the height counts. Its collision radius
        // is 0.
        let mut delta = (dx * dx + dy * dy + dz * dz).sqrt();
        delta = (delta - 0.0).max(0.000_01);

        let mut dist_fraction = f64::MAX;
        if delta > 1.0 {
            let dist_passed = (speed * f64::from(game_ticks.wrapping_sub(mv.move_timestamp)))
                / f64::from(TICKS_PER_SECOND);
            dist_fraction = dist_passed / delta;
        }

        let arrived = dist_fraction > 1.0;
        if arrived {
            // Set the position of the Creature to the destination.
            self.set_xyz(id, mv.x_destination, mv.y_destination, mv.z_destination);
        } else {
            let new_z = z_prev.wrapping_add(((dz * dist_fraction) + 0.5) as i32);
            mv.x_accurate += dx * dist_fraction;
            mv.y_accurate += dy * dist_fraction;
            // Set the position of the Creature to estimated after partial move.
            self.set_xyz(id, mv.x_accurate as i32, mv.y_accurate as i32, new_z);
        }

        self.vehicle_revalidate_zone(id, false);

        // Set the timer of last position update to now.
        mv.move_timestamp = game_ticks;
        let destination = (mv.x_destination, mv.y_destination, mv.z_destination);
        if let Some(v) = self.veh_mut(id) {
            v.mv = Some(mv);
        }

        // Broadcast MoveToLocation on arrived.
        if arrived {
            self.vehicle_broadcast_move_to_location(id, destination);
        }
        arrived
    }

    /// Java `Creature.broadcastMoveToLocation(true)` of a creature that is not playable (a
    /// vehicle has no target): `MoveToLocation` when the region is on.
    fn vehicle_broadcast_move_to_location(&mut self, id: ObjectId, dest: (i32, i32, i32)) {
        let game_ticks = self.game_ticks();
        let Some(v) = self.veh_mut(id) else {
            return;
        };
        let Some(mv) = v.mv.as_mut() else {
            return;
        };
        mv.last_broadcast_time = game_ticks;
        let packet = MoveToLocation {
            object_id: id.0,
            dest_x: dest.0,
            dest_y: dest.1,
            dest_z: dest.2,
            x: v.x,
            y: v.y,
            z: v.z,
        };
        if self.are_region_neighbors_active(id) {
            self.broadcast_packet(id, &packet);
        }
    }

    /// Java `CreatureAI.notifyActionArrived()` of a vehicle (`BoatAI` and `AirShipAI` do not
    /// change it): the zones are looked at, the path goes on, else the AI becomes active.
    pub(crate) fn vehicle_notify_action_arrived(&mut self, id: ObjectId) {
        // `canHandleAction()`: spawned or teleporting.
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        let teleporting = self.veh(id).is_some_and(|v| v.teleporting);
        if !(spawned || teleporting) {
            return;
        }
        self.vehicle_revalidate_zone(id, true);
        if self.vehicle_move_to_next_route_point(id) {
            return;
        }
        // `clientStoppedMoving()` is empty in both AIs.
        if self.veh(id).is_some_and(|v| v.intention == VIntention::MoveTo) {
            self.vehicle_set_intention_active(id);
        }
    }

    /// Java `Boat.moveToNextRoutePoint()` / `AirShip.moveToNextRoutePoint()`: the vehicle's, then
    /// the packet that tells everybody it set off for the next point.
    pub(crate) fn vehicle_move_to_next_route_point(&mut self, id: ObjectId) -> bool {
        let result = self.vehicle_move_to_next_route_point_base(id);
        if result {
            match self.veh(id).map(|v| v.kind) {
                Some(VehicleKind::Boat) => {
                    if let Some(packet) = self.vehicle_departure_packet(id) {
                        self.broadcast_packet(id, &packet);
                    }
                }
                Some(VehicleKind::AirShip) => {
                    if let Some(packet) = self.vehicle_move_packet(id) {
                        self.broadcast_packet(id, &packet);
                    }
                }
                None => {}
            }
        }
        result
    }

    /// Java `Vehicle.moveToNextRoutePoint()`.
    fn vehicle_move_to_next_route_point_base(&mut self, id: ObjectId) -> bool {
        let game_ticks = self.game_ticks();
        let disabled = self.vehicle_movement_disabled(id);
        let Some(v) = self.veh_mut(id) else {
            return false;
        };
        v.mv = None;
        // The point to go to, or the end of the path.
        let mut next = None;
        let mut end_of_path = false;
        if let Some(path) = v.path {
            v.run_state += 1;
            let index = usize::try_from(v.run_state).unwrap_or(usize::MAX);
            match path.get(index).copied() {
                Some(point) => next = Some(point),
                None => end_of_path = true,
            }
        }
        if end_of_path {
            let monitor = v.monitor.take();
            v.path = None;
            if let Some(timer) = monitor {
                self.timers.cancel(timer);
            }
        } else if let Some(point) = next.filter(|_| !disabled) {
            if point.move_speed == 0 {
                // The point is a teleport: its rotation speed is the heading.
                let monitor = v.monitor.take();
                v.path = None;
                if let Some(timer) = monitor {
                    self.timers.cancel(timer);
                }
                self.vehicle_tele_to_location(id, (point.x, point.y, point.z, point.rotation_speed));
            } else {
                if point.move_speed > 0 {
                    v.move_speed = point.move_speed as f32;
                }
                if point.rotation_speed > 0 {
                    v.rotation_speed = point.rotation_speed;
                }
                let (cur_x, cur_y) = (v.x, v.y);
                v.mv = Some(VehicleMove {
                    move_start_time: game_ticks,
                    move_timestamp: 0,
                    x_destination: point.x,
                    y_destination: point.y,
                    z_destination: point.z,
                    x_accurate: 0.0,
                    y_accurate: 0.0,
                    last_broadcast_time: 0,
                });
                let need_monitor = v.monitor.is_none();
                let dx = f64::from(point.x) - f64::from(cur_x);
                let dy = f64::from(point.y) - f64::from(cur_y);
                if (dx * dx + dy * dy).sqrt() > 1.0 {
                    self.vehicle_set_heading(id, heading_from_points(cur_x, cur_y, point.x, point.y));
                }
                self.vehicle_register_moving(id);
                // "Make sure vehicle is not stuck."
                if need_monitor {
                    let timer = self.timers.schedule_at(
                        self.tick + TimerWheel::ms_to_ticks(MONITOR_PERIOD_MS),
                        TimerKind::Vehicle(VehicleTimer::Monitor(id)),
                    );
                    if let Some(v) = self.veh_mut(id) {
                        v.monitor = Some(timer);
                    }
                }
                return true;
            }
        }
        self.vehicle_run_engine(id, 10);
        false
    }

    /// The `_monitorTask`: a vehicle on a path that has not moved since the last look is pushed
    /// on to the previous point again.
    pub(crate) fn vehicle_monitor_tick(&mut self, id: ObjectId) {
        let Some(v) = self.veh(id) else {
            return;
        };
        if v.monitor.is_none() {
            // Cancelled while the timer was due.
            return;
        }
        let here = (v.x, v.y, v.z);
        let stuck = !v.is_in_dock() && here == v.monitor_loc;
        let path_len = v.path.map(<[PathPoint]>::len);
        let run_state = v.run_state;
        // Fixed rate: the next look.
        let timer = self.timers.schedule_at(
            self.tick + TimerWheel::ms_to_ticks(MONITOR_PERIOD_MS),
            TimerKind::Vehicle(VehicleTimer::Monitor(id)),
        );
        if let Some(v) = self.veh_mut(id) {
            v.monitor = Some(timer);
        }
        if stuck {
            if let Some(len) = path_len {
                if usize::try_from(run_state).is_ok_and(|s| s < len) {
                    if let Some(v) = self.veh_mut(id) {
                        v.run_state = (run_state - 1).max(0);
                    }
                    self.vehicle_move_to_next_route_point(id);
                } else {
                    self.vehicle_broadcast_info(id);
                }
            }
        } else if let Some(v) = self.veh_mut(id) {
            v.monitor_loc = here;
        }
    }

    /// Java `Vehicle.teleToLocation(loc, false)`: the passengers go first, then the vehicle
    /// leaves and comes back at the place.
    pub(crate) fn vehicle_tele_to_location(&mut self, id: ObjectId, (x, y, z, heading): (i32, i32, i32, i32)) {
        if self.veh(id).is_none() {
            return;
        }
        if self.veh(id).is_some_and(|v| v.is_moving()) {
            self.vehicle_stop_move(id, None);
        }
        if let Some(v) = self.veh_mut(id) {
            v.teleporting = true;
        }
        self.vehicle_set_intention_active(id);
        let passengers = self.veh(id).map(|v| v.passengers.clone()).unwrap_or_default();
        for passenger in passengers {
            self.tele_to_location(
                passenger,
                TeleportTarget::at(x, y, z).with_heading(heading).with_instance(0),
            );
        }
        // `decayMe()`, `setXYZ(loc)`
        self.decay_keep_entry(id);
        self.set_vehicle_position_raw(id, x, y, z);
        // "Temporary fix for heading on teleports."
        if heading != 0 {
            self.vehicle_set_heading(id, heading);
        }
        // `onTeleported()`: back into the world at the place.
        if self.veh(id).is_some_and(|v| v.teleporting) {
            self.spawn_at(id, x, y, z);
            if let Some(v) = self.veh_mut(id) {
                v.teleporting = false;
            }
        }
        self.vehicle_revalidate_zone(id, true);
    }

    /// Java `Vehicle.runEngine(delay)`.
    pub(crate) fn vehicle_run_engine(&mut self, id: ObjectId, delay_ms: u64) {
        if self.veh(id).is_none_or(|v| v.engine.is_none()) {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        self.timers
            .schedule_at(due, TimerKind::Vehicle(VehicleTimer::Engine(id)));
    }

    /// The `Engine` timer: the `run()` of the script.
    pub(crate) fn vehicle_run_engine_now(&mut self, id: ObjectId) {
        match self.veh(id).and_then(|v| v.engine.as_ref()) {
            Some(Engine::Boat { .. }) => self.boat_engine_run(id),
            Some(Engine::GludioGracia(_)) => self.gludio_gracia_run(id),
            None => {}
        }
    }
}
