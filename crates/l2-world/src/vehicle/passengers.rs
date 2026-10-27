//! Who rides a vehicle: `Player.setVehicle` / `setInVehiclePosition`, `Vehicle.addPassenger`,
//! `oustPlayer`, `payForRide`, and the hooks the player code calls (teleport, logout, the info
//! packets).
// Java: entity/actor/Vehicle.java (addPassenger, removePassenger, oustPlayer, oustPlayers,
//       getOustLoc, payForRide)
// Java: entity/actor/instance/{Boat,AirShip,ControllableAirShip}.java (addPassenger, oustPlayer)
// Java: entity/actor/Player.java (setVehicle, setInVehiclePosition, teleToLocation, onTeleported,
//       sendInfo, deleteMe)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExGetOffAirShip, ExGetOnAirShip, GetOnVehicle, InventoryUpdate, SystemMessage,
};
use l2_util::ObjectId;

use super::route::Pay;
use super::{Rider, VehicleKind};
use crate::objects::ObjectKind;
use crate::teleport::TeleportTarget;
use crate::world::World;

/// `payForRide`'s `World.forEachVisibleObjectInRange(this, Player.class, 1000, ...)`.
const PAY_RANGE: i32 = 1000;

impl World {
    /// Java `Player.setVehicle(v)`: with `None` the old vehicle forgets the passenger.
    pub(crate) fn set_rider_vehicle(&mut self, player: ObjectId, vehicle: Option<ObjectId>) {
        if vehicle.is_none()
            && let Some(old) = self.vehicle_of(player)
        {
            self.vehicle_remove_passenger(old, player);
        }
        let rider = self.vehicle.riders.entry(player).or_default();
        rider.vehicle = vehicle;
        self.prune_rider(player);
    }

    /// Java `Player.setInVehiclePosition(pos)`.
    pub(crate) fn set_in_vehicle_position(&mut self, player: ObjectId, position: Option<(i32, i32, i32)>) {
        let rider = self.vehicle.riders.entry(player).or_default();
        rider.position = position;
        self.prune_rider(player);
    }

    fn prune_rider(&mut self, player: ObjectId) {
        if self.vehicle.riders.get(&player) == Some(&Rider::default()) {
            self.vehicle.riders.remove(&player);
        }
    }

    /// Java `Vehicle.removePassenger(player)`.
    pub(crate) fn vehicle_remove_passenger(&mut self, vehicle: ObjectId, player: ObjectId) {
        if let Some(v) = self.veh_mut(vehicle) {
            v.passengers.retain(|p| *p != player);
        }
    }

    /// Java `Vehicle.addPassenger(player)` with the `AirShip` override: an air ship also puts the
    /// player on board.
    pub(crate) fn vehicle_add_passenger(&mut self, vehicle: ObjectId, player: ObjectId) -> bool {
        let Some(v) = self.veh(vehicle) else {
            return false;
        };
        if !self.players.contains_key(&player)
            || v.passengers.contains(&player)
            || self.vehicle_of(player).is_some_and(|current| current != vehicle)
        {
            return false;
        }
        let (kind, x, y, z) = (v.kind, v.x, v.y, v.z);
        if let Some(v) = self.veh_mut(vehicle) {
            v.passengers.push(player);
        }
        if kind == VehicleKind::AirShip {
            self.set_rider_vehicle(player, Some(vehicle));
            self.set_in_vehicle_position(player, Some((0, 0, 0)));
            self.broadcast_with_self(
                player,
                &ExGetOnAirShip {
                    player_id: player.0,
                    air_ship_id: vehicle.0,
                    x: 0,
                    y: 0,
                    z: 0,
                },
            );
            self.set_xyz(player, x, y, z);
            self.revalidate_zone(player, true);
            self.stop_move(player, None);
        }
        true
    }

    /// Java `Vehicle.getOustLoc()`: the place set for the vehicle, else the nearest town.
    pub(crate) fn vehicle_oust_loc(&mut self, vehicle: ObjectId) -> Option<(i32, i32, i32)> {
        let v = self.veh(vehicle)?;
        if let Some(loc) = v.oust_loc {
            return Some(loc);
        }
        let (x, y, z) = (v.x, v.y, v.z);
        self.nearest_town_respawn_at(x, y, z)
    }

    /// Java `Vehicle.oustPlayers()`.
    pub(crate) fn vehicle_oust_players(&mut self, vehicle: ObjectId) {
        let passengers = self
            .veh(vehicle)
            .map(|v| v.passengers.clone())
            .unwrap_or_default();
        for passenger in passengers {
            self.vehicle_oust_player(vehicle, passenger);
        }
        if let Some(v) = self.veh_mut(vehicle) {
            v.passengers.clear();
        }
    }

    /// Java `Vehicle.oustPlayer(player)` with the overrides of `Boat`, `AirShip` and
    /// `ControllableAirShip`: the player gets off and is put at the place of the vehicle's
    /// landing.
    pub(crate) fn vehicle_oust_player(&mut self, vehicle: ObjectId, player: ObjectId) {
        let Some(v) = self.veh(vehicle) else {
            return;
        };
        let kind = v.kind;
        // `ControllableAirShip.oustPlayer`: "no need to broadcast userinfo here".
        if v.ship.as_ref().is_some_and(|s| s.captain == Some(player)) {
            self.ship_set_captain(vehicle, None);
        }
        // `Vehicle.oustPlayer`
        self.set_rider_vehicle(player, None);
        self.set_in_vehicle_position(player, None);
        self.vehicle_remove_passenger(vehicle, player);

        let Some((x, y, z)) = self.vehicle_oust_loc(vehicle) else {
            return;
        };
        // `isOnline()`: a player of a closed connection is put at the place without packets.
        if self.sinks.contains_key(&player) {
            if kind == VehicleKind::AirShip {
                self.broadcast_with_self(
                    player,
                    &ExGetOffAirShip {
                        player_id: player.0,
                        air_ship_id: vehicle.0,
                        x,
                        y,
                        z,
                    },
                );
            }
            self.tele_to_location(player, TeleportTarget::at(x, y, z));
        } else if let Some(p) = self.players.get_mut(&player) {
            // `setXYZInvisible(x, y, z)`: "disconnects handling".
            (p.core.x, p.core.y, p.core.z) = (x, y, z);
        }
    }

    /// Java `Vehicle.payForRide(itemId, count, oustX, oustY, oustZ)`: everybody on the boat in
    /// reach pays the ticket and becomes a passenger, or is put ashore.
    pub(crate) fn vehicle_pay_for_ride(&mut self, vehicle: ObjectId, pay: Pay) {
        let riders: Vec<ObjectId> = self
            .visible_objects_in_range(vehicle, PAY_RANGE)
            .into_iter()
            .filter(|p| {
                self.objects.get(p).is_some_and(|e| e.kind == ObjectKind::Player)
                    && self.is_in_boat(*p)
                    && self.vehicle_of(*p) == Some(vehicle)
            })
            .collect();
        for player in riders {
            if pay.item_id > 0 {
                let ticket = self
                    .inventory_item_by_item_id(player, pay.item_id)
                    .map(|i| i.object_id);
                let removed =
                    ticket.and_then(|object_id| self.destroy_inventory_item(player, object_id, pay.count));
                let Some(item) = removed else {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YouDoNotPossessTheCorrectTicketToBoardTheBoat),
                    );
                    let offset = self.settings.teleport.max_offset_on_teleport;
                    self.tele_to_location(
                        player,
                        TeleportTarget::at(pay.oust.0, pay.oust.1, pay.oust.2)
                            .with_instance(0)
                            .with_random_offset(offset),
                    );
                    continue;
                };
                // `iu.addModifiedItem(ticket)`
                let entry = self.update_entry(&item, InventoryUpdate::MODIFIED);
                self.send_inventory_update(player, entry);
            }
            self.vehicle_add_passenger(vehicle, player);
        }
    }

    /// The first lines of `Player.teleToLocation`: "if the vehicle is not teleporting, the player
    /// gets off".
    pub(crate) fn vehicle_before_player_teleport(&mut self, player: ObjectId) {
        let Some(vehicle) = self.vehicle_of(player) else {
            return;
        };
        if !self.veh(vehicle).is_some_and(|v| v.teleporting) {
            self.set_rider_vehicle(player, None);
        }
    }

    /// `Player.deleteMe()`: `_vehicle.oustPlayer(this)` with the player already offline, which
    /// only puts it at the landing place. The state of the player was stored before (Java stores
    /// first, so the stored position is the vehicle's).
    pub(crate) fn vehicle_player_left(&mut self, player: ObjectId) {
        let Some(vehicle) = self.vehicle_of(player) else {
            self.vehicle.riders.remove(&player);
            return;
        };
        if self.veh(vehicle).is_none() {
            self.vehicle.riders.remove(&player);
            return;
        }
        let kind_ship = self
            .veh(vehicle)
            .is_some_and(|v| v.ship.as_ref().is_some_and(|s| s.captain == Some(player)));
        if kind_ship {
            self.ship_set_captain(vehicle, None);
        }
        self.set_rider_vehicle(player, None);
        self.set_in_vehicle_position(player, None);
        self.vehicle_remove_passenger(vehicle, player);
        if let Some((x, y, z)) = self.vehicle_oust_loc(vehicle)
            && let Some(p) = self.players.get_mut(&player)
        {
            (p.core.x, p.core.y, p.core.z) = (x, y, z);
        }
    }

    /// `CharInfo` and `UserInfo`: the vehicle id of a player on one and, for `CharInfo`, the place
    /// inside it (`getVehicle() != null && getInVehiclePosition() != null`).
    pub(crate) fn rider_char_view(&self, player: ObjectId) -> Option<(i32, (i32, i32, i32))> {
        let rider = self.vehicle.riders.get(&player)?;
        Some((rider.vehicle?.0, rider.position?))
    }

    /// `UserInfo`: `getVehicle() != null ? getVehicle().getObjectId() : 0`.
    pub(crate) fn rider_vehicle_id(&self, player: ObjectId) -> i32 {
        self.vehicle_of(player).map_or(0, |v| v.0)
    }

    /// The end of `Player.sendInfo(player)` for a player on a boat or an air ship: where in the
    /// vehicle the subject is.
    pub(crate) fn send_rider_get_on(&self, subject: ObjectId, viewer: ObjectId) {
        let (Some(vehicle), Some((x, y, z))) = (self.vehicle_of(subject), self.in_vehicle_position(subject))
        else {
            return;
        };
        match self.veh(vehicle).map(|v| v.kind) {
            Some(VehicleKind::Boat) => self.send(
                viewer,
                &GetOnVehicle {
                    object_id: subject.0,
                    boat_object_id: vehicle.0,
                    x,
                    y,
                    z,
                },
            ),
            Some(VehicleKind::AirShip) => self.send(
                viewer,
                &ExGetOnAirShip {
                    player_id: subject.0,
                    air_ship_id: vehicle.0,
                    x,
                    y,
                    z,
                },
            ),
            None => false,
        };
    }

    /// `Player.onTeleported()`: "if (isInAirShip()) getAirShip().sendInfo(this)" instead of the
    /// last server position. `true` when the player is on an air ship.
    pub(crate) fn vehicle_player_on_teleported(&mut self, player: ObjectId) -> bool {
        if !self.is_in_air_ship(player) {
            return false;
        }
        if let Some(vehicle) = self.vehicle_of(player) {
            self.send_vehicle_info(vehicle, player);
        }
        true
    }
}
