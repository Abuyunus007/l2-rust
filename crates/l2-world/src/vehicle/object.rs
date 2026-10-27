//! The vehicle as an object of the world: creating it (`BoatManager.getNewBoat`,
//! `AirShipManager.getNewAirShip`), `onSpawn`, `deleteMe`, and what the players are told about it.
// Java: managers/{BoatManager,AirShipManager}.java (getNewBoat, getNewAirShip, removeAirShip)
// Java: entity/actor/Vehicle.java (deleteMe), entity/actor/instance/{Boat,AirShip,
//       ControllableAirShip}.java (sendInfo, updateAbnormalEffect, onSpawn, deleteMe)
// Java: ai/{BoatAI,AirShipAI}.java (describeStateToPlayer)

use l2_protocol_game::server::{
    DeleteObject, ExAirShipInfo, ExMoveToLocationAirShip, VehicleDeparture, VehicleInfo,
};
use l2_util::ObjectId;
use tracing::warn;

use super::{Rider, ShipState, Vehicle, VehicleKind, VehicleTimer, VehicleWrite};
use crate::objects::ObjectKind;
use crate::outbox::PersistCommand;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `ControllableAirShip.LOW_FUEL`.
const LOW_FUEL: i32 = 40;
/// `ControllableAirShip.onSpawn`: `scheduleAtFixedRate(new CheckTask(), 60000, 10000)`.
const CHECK_FIRST_MS: u64 = 60_000;
const CHECK_PERIOD_MS: u64 = 10_000;
/// `scheduleAtFixedRate(new ConsumeFuelTask(), 60000, 60000)`.
const FUEL_PERIOD_MS: u64 = 60_000;

impl World {
    pub(crate) fn veh(&self, id: ObjectId) -> Option<&Vehicle> {
        self.vehicle.vehicles.get(&id)
    }

    pub(crate) fn veh_mut(&mut self, id: ObjectId) -> Option<&mut Vehicle> {
        self.vehicle.vehicles.get_mut(&id)
    }

    /// Where a vehicle is (`position_of` for [`ObjectKind::Vehicle`]).
    pub(crate) fn vehicle_position(&self, id: ObjectId) -> Option<(i32, i32, i32)> {
        self.veh(id).map(|v| (v.x, v.y, v.z))
    }

    /// The position part of `WorldObject.setXYZ` (`set_position_raw`).
    pub(crate) fn set_vehicle_position_raw(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        if let Some(v) = self.veh_mut(id) {
            (v.x, v.y, v.z) = (x, y, z);
        }
    }

    /// A new vehicle at a place: `getNewBoat` / `getNewAirShip` up to `spawnMe()`. `owner` makes
    /// it a `ControllableAirShip` of that owner; `init` sets what Java sets before `spawnMe()`.
    /// `None` when no object id is left.
    pub(crate) fn create_vehicle(
        &mut self,
        kind: VehicleKind,
        (x, y, z): (i32, i32, i32),
        heading: i32,
        owner: Option<i32>,
        init: impl FnOnce(&mut Vehicle),
    ) -> Option<ObjectId> {
        let Some(ids) = self.npc_world.ids.clone() else {
            warn!("Vehicle: no object id factory; the vehicle is not created.");
            return None;
        };
        let id = match ids.allocate() {
            Ok(id) => id,
            Err(e) => {
                warn!("Vehicle: no object id for a new vehicle: {e}");
                return None;
            }
        };
        let mut vehicle = Vehicle::new(id, kind, x, y, z, heading);
        if let Some(owner_id) = owner {
            // `_helmId = IdManager.getInstance().getNextId()`
            let helm_id = match ids.allocate() {
                Ok(helm) => helm,
                Err(e) => {
                    warn!("Vehicle: no object id for the helm of a new air ship: {e}");
                    ids.release(id);
                    return None;
                }
            };
            vehicle.ship = Some(ShipState {
                owner_id,
                fuel: 0,
                max_fuel: 0,
                captain: None,
                helm_id,
                task_pairs: 0,
            });
        }
        init(&mut vehicle);
        self.vehicle.vehicles.insert(id, vehicle);
        self.register_object(id, ObjectKind::Vehicle);
        // `setXYZInvisible(x, y, z)`, `spawnMe()`
        self.spawn_at(id, x, y, z);
        Some(id)
    }

    /// Java `Creature.onSpawn()` and `ControllableAirShip.onSpawn()` of a vehicle: the zones are
    /// revalidated, an air ship that is steered starts its fuel and idle checks (again, after
    /// every teleport: Java schedules the pair in every `onSpawn` and cancels only the last).
    pub(crate) fn vehicle_on_spawn(&mut self, id: ObjectId) {
        if self.veh(id).is_none() {
            return;
        }
        self.vehicle_revalidate_zone(id, true);
        if self.veh(id).is_none_or(|v| v.ship.is_none()) {
            return;
        }
        let now = self.tick;
        self.timers.schedule_at(
            now + TimerWheel::ms_to_ticks(CHECK_FIRST_MS),
            TimerKind::Vehicle(VehicleTimer::ShipCheck(id)),
        );
        self.timers.schedule_at(
            now + TimerWheel::ms_to_ticks(FUEL_PERIOD_MS),
            TimerKind::Vehicle(VehicleTimer::ShipFuel(id)),
        );
        if let Some(ship) = self.veh_mut(id).and_then(|v| v.ship.as_mut()) {
            ship.task_pairs += 1;
        }
    }

    /// `ControllableAirShip.CheckTask`: an empty ship outside a dock decays.
    pub(crate) fn ship_check_tick(&mut self, id: ObjectId) {
        let Some(v) = self.veh(id) else {
            // The ship is gone: the task ends with it.
            return;
        };
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        let decay = spawned && v.is_empty() && !v.is_in_dock();
        // The task goes on every 10 s.
        self.timers.schedule_at(
            self.tick + TimerWheel::ms_to_ticks(CHECK_PERIOD_MS),
            TimerKind::Vehicle(VehicleTimer::ShipCheck(id)),
        );
        if decay {
            // "deleteMe() can't be called from CheckTask because task should not cancel itself":
            // `ThreadPool.execute(new DecayTask())`, i.e. right away.
            let due = self.tick + 1;
            self.timers
                .schedule_at(due, TimerKind::Vehicle(VehicleTimer::ShipDecay(id)));
        }
    }

    /// `ControllableAirShip.ConsumeFuelTask`: 10 fuel less every minute while there is some.
    pub(crate) fn ship_fuel_tick(&mut self, id: ObjectId) {
        if self.veh(id).is_none() {
            return;
        }
        self.timers.schedule_at(
            self.tick + TimerWheel::ms_to_ticks(FUEL_PERIOD_MS),
            TimerKind::Vehicle(VehicleTimer::ShipFuel(id)),
        );
        let mut fuel = self.veh(id).map_or(0, Vehicle::fuel);
        if fuel > 0 {
            fuel -= 10;
            if fuel < 0 {
                fuel = 0;
            }
            self.ship_set_fuel(id, fuel);
            self.update_air_ship_info(id);
        }
    }

    /// Java `ControllableAirShip.setFuel(f)`: clamped to the tank; the passengers are warned when
    /// it runs out and while it is low.
    pub(crate) fn ship_set_fuel(&mut self, id: ObjectId, fuel: i32) {
        use l2_protocol_game::SystemMessageId::{
            TheAirshipSFuelEpHasRunOutTheAirshipSSpeedWillBeGreatlyDecreasedInThisCondition as OutOfFuel,
            TheAirshipSFuelEpWillSoonRunOut as LowOnFuel,
        };
        let Some(ship) = self.veh_mut(id).and_then(|v| v.ship.as_mut()) else {
            return;
        };
        let old = ship.fuel;
        ship.fuel = fuel.clamp(0, ship.max_fuel.max(0));
        let now = ship.fuel;
        if now == 0 && old > 0 {
            self.broadcast_to_passengers(id, &l2_protocol_game::server::SystemMessage::new(OutOfFuel));
        } else if now < LOW_FUEL {
            self.broadcast_to_passengers(id, &l2_protocol_game::server::SystemMessage::new(LowOnFuel));
        }
    }

    /// Java `Vehicle.broadcastToPassengers(packet)`.
    pub(crate) fn broadcast_to_passengers(&self, id: ObjectId, packet: &impl l2_net::Encode) {
        let Some(v) = self.veh(id) else {
            return;
        };
        let bytes = packet.to_bytes();
        for passenger in &v.passengers {
            self.send_bytes(*passenger, bytes.clone());
        }
    }

    /// Java `Vehicle.deleteMe()` with `AirShip.deleteMe()` and `ControllableAirShip.deleteMe()`:
    /// the move stops, the passengers are put ashore, the vehicle leaves the world. An air ship
    /// of an owner stores its fuel. `false` when there is no such vehicle.
    pub(crate) fn vehicle_delete_me(&mut self, id: ObjectId) -> bool {
        if self.veh(id).is_none() {
            return false;
        }
        if let Some(v) = self.veh_mut(id) {
            v.engine = None;
        }
        if self.veh(id).is_some_and(Vehicle::is_moving) {
            self.vehicle_stop_move(id, None);
        }
        self.vehicle_oust_players(id);
        // The zones lose the ship (`removeFromZones`): a dock that held it is free again.
        self.vehicle_leave_all_zones(id);
        // Who sees the vehicle (and the helm) when it goes.
        let viewers = self.visible_players(id);
        // `decayMe()`
        self.decay(id);
        let Some(vehicle) = self.vehicle.vehicles.remove(&id) else {
            return true;
        };
        self.vehicle.moving.retain(|m| *m != id);
        self.vehicle.boats.retain(|b| *b != id);
        if let Some(timer) = vehicle.monitor {
            self.timers.cancel(timer);
        }
        if let Some(ship) = &vehicle.ship {
            // `AirShipManager.removeAirShip`: `storeInDb(ownerId)` writes the fuel the manager knew
            // before this flight, then the manager learns the ship's.
            if ship.owner_id != 0 {
                // NOTE(java-quirk): `storeInDb` runs before `info.set("fuel", ship.getFuel())`, so
                // the row is one flight behind.
                if let Some(old) = self.vehicle.licenses.get(&ship.owner_id).copied() {
                    self.outbox.persist(PersistCommand::Vehicle(VehicleWrite::StoreFuel {
                        owner_id: ship.owner_id,
                        fuel: old,
                    }));
                    self.vehicle.licenses.insert(ship.owner_id, ship.fuel);
                }
                if self.vehicle.owner_ships.get(&ship.owner_id) == Some(&id) {
                    self.vehicle.owner_ships.remove(&ship.owner_id);
                }
            }
            // `broadcastPacket(new DeleteObject(_helmId))`
            let packet = DeleteObject {
                object_id: ship.helm_id.0,
            };
            for viewer in viewers {
                self.send(viewer, &packet);
            }
            if let Some(ids) = self.npc_world.ids.clone() {
                ids.release(ship.helm_id);
            }
        }
        if self.vehicle.gludio_gracia == Some(id) {
            self.vehicle.gludio_gracia = None;
        }
        // The object id goes back (Java frees it with `refreshId()` at the next summon).
        if let Some(ids) = self.npc_world.ids.clone() {
            ids.release(id);
        }
        self.vehicle.riders.retain(|_, rider: &mut Rider| rider.vehicle != Some(id));
        true
    }

    /// Java `sendInfo(player)` of a vehicle: `VehicleInfo` for a boat; `ExAirShipInfo` for an air
    /// ship the player can see, and for a ship that is steered also the captain's info.
    pub(crate) fn send_vehicle_info(&mut self, subject: ObjectId, viewer: ObjectId) {
        let Some(v) = self.veh(subject) else {
            return;
        };
        match v.kind {
            VehicleKind::Boat => {
                let packet = VehicleInfo {
                    object_id: subject.0,
                    x: v.x,
                    y: v.y,
                    z: v.z,
                    heading: v.heading,
                };
                self.send(viewer, &packet);
            }
            VehicleKind::AirShip => {
                if !self.is_visible_for(subject, viewer) {
                    return;
                }
                let packet = self.air_ship_info_packet(subject);
                let captain = v.ship.as_ref().and_then(|s| s.captain);
                if let Some(packet) = packet {
                    self.send(viewer, &packet);
                }
                if let Some(captain) = captain {
                    self.send_info(captain, viewer);
                }
            }
        }
    }

    /// Java `AbstractAI.describeStateToPlayer(player)` of `BoatAI` and `AirShipAI`: a vehicle on
    /// its way tells the new player where to.
    pub(crate) fn vehicle_describe_state_to_player(&mut self, subject: ObjectId, viewer: ObjectId) {
        let Some(v) = self.veh(subject) else {
            return;
        };
        let Some(mv) = &v.mv else {
            return;
        };
        let (dest, kind) = ((mv.x_destination, mv.y_destination, mv.z_destination), v.kind);
        match kind {
            VehicleKind::Boat => {
                let packet = self.vehicle_departure_packet(subject);
                if let Some(packet) = packet {
                    self.send(viewer, &packet);
                }
            }
            VehicleKind::AirShip => {
                if let Some(packet) = self.ex_move_to_location_air_ship(subject, dest) {
                    self.send(viewer, &packet);
                }
            }
        }
    }

    /// Java `WorldObject.broadcastInfo()`: `sendInfo` to everybody who can see the vehicle.
    pub(crate) fn vehicle_broadcast_info(&mut self, id: ObjectId) {
        for viewer in self.visible_players(id) {
            self.send_vehicle_info(id, viewer);
        }
    }

    /// Java `AirShip.updateAbnormalEffect()`: everybody around gets `ExAirShipInfo`.
    pub(crate) fn update_air_ship_info(&mut self, id: ObjectId) {
        if self.veh(id).is_none_or(|v| !v.is_air_ship()) {
            return;
        }
        if let Some(packet) = self.air_ship_info_packet(id) {
            self.broadcast_packet(id, &packet);
        }
    }

    /// `new ExAirShipInfo(ship)`.
    pub(crate) fn air_ship_info_packet(&self, id: ObjectId) -> Option<ExAirShipInfo> {
        let v = self.veh(id)?;
        let helm = v.helm_object_id();
        Some(ExAirShipInfo {
            ship_object_id: id.0,
            x: v.x,
            y: v.y,
            z: v.z,
            heading: v.heading,
            captain_id: v.captain_id(),
            // `(int) ship.getStat().getMoveSpeed()`
            move_speed: v.current_move_speed() as i32,
            rotation_speed: v.rotation_speed,
            helm_object_id: helm,
            // "TODO: unhardcode these!": the positions are written only with a helm.
            helm_points: if helm != 0 {
                ExAirShipInfo::HELM_POINTS
            } else {
                [0; 6]
            },
            fuel: v.fuel(),
            max_fuel: v.max_fuel(),
        })
    }

    /// `new VehicleDeparture(boat)`.
    pub(crate) fn vehicle_departure_packet(&self, id: ObjectId) -> Option<VehicleDeparture> {
        let v = self.veh(id)?;
        let (x, y, z) = match &v.mv {
            Some(mv) => (mv.x_destination, mv.y_destination, mv.z_destination),
            // `getXdestination()` is the position of a creature that does not move.
            None => (v.x, v.y, v.z),
        };
        Some(VehicleDeparture {
            object_id: id.0,
            // `(int) boat.getMoveSpeed()`
            move_speed: v.current_move_speed() as i32,
            rotation_speed: v.rotation_speed,
            x,
            y,
            z,
        })
    }

    /// `new ExMoveToLocationAirShip(ship)`.
    pub(crate) fn ex_move_to_location_air_ship(
        &self,
        id: ObjectId,
        destination: (i32, i32, i32),
    ) -> Option<ExMoveToLocationAirShip> {
        let v = self.veh(id)?;
        Some(ExMoveToLocationAirShip {
            object_id: id.0,
            destination_x: destination.0,
            destination_y: destination.1,
            destination_z: destination.2,
            x: v.x,
            y: v.y,
            z: v.z,
        })
    }
}
