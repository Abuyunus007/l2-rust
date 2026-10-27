//! The client packets of the boats and air ships.
// Java: network/clientpackets/{RequestGetOnVehicle,RequestGetOffVehicle,
//       RequestMoveToLocationInVehicle,CannotMoveAnymoreInVehicle,MoveToLocationAirShip,
//       MoveToLocationInAirShip}.java
// Java: network/clientpackets/RequestActionUse.java (the air ship actions 67 to 70)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, ExMoveToLocationInAirShip, GetOffVehicle, GetOnVehicle,
    MoveToLocationInVehicle, StopMoveInVehicle, SystemMessage,
};
use l2_util::ObjectId;

use crate::target::distance_3d;
use crate::world::World;
use crate::zones::ZoneId;

/// `isInsideRadius3D(..., 1000)` of boarding and leaving.
const BOARDING_RADIUS: f64 = 1000.0;

/// The packets of this module as the session hands them to the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleInput {
    /// `RequestGetOnVehicle`.
    GetOn { boat_id: i32, x: i32, y: i32, z: i32 },
    /// `RequestGetOffVehicle`.
    GetOff { boat_id: i32, x: i32, y: i32, z: i32 },
    /// `RequestMoveToLocationInVehicle`.
    MoveInVehicle {
        boat_id: i32,
        target: (i32, i32, i32),
        origin: (i32, i32, i32),
    },
    /// `CannotMoveAnymoreInVehicle`.
    CannotMoveInVehicle {
        boat_id: i32,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
    },
    /// `MoveToLocationAirShip`: the control panel.
    AirShipControl { command: i32, param1: i32, param2: i32 },
    /// `MoveToLocationInAirShip`.
    MoveInAirShip {
        ship_id: i32,
        target: (i32, i32, i32),
        origin: (i32, i32, i32),
    },
}

impl World {
    /// Hands a packet of this module to its handler.
    pub(crate) fn on_vehicle_input(&mut self, player: ObjectId, input: VehicleInput) {
        if !self.players.contains_key(&player) {
            return;
        }
        match input {
            VehicleInput::GetOn { boat_id, x, y, z } => self.on_request_get_on_vehicle(player, boat_id, (x, y, z)),
            VehicleInput::GetOff { boat_id, x, y, z } => self.on_request_get_off_vehicle(player, boat_id, (x, y, z)),
            VehicleInput::MoveInVehicle {
                boat_id,
                target,
                origin,
            } => self.on_request_move_to_location_in_vehicle(player, boat_id, target, origin),
            VehicleInput::CannotMoveInVehicle {
                boat_id,
                x,
                y,
                z,
                heading,
            } => self.on_cannot_move_anymore_in_vehicle(player, boat_id, (x, y, z), heading),
            VehicleInput::AirShipControl {
                command,
                param1,
                param2,
            } => self.on_move_to_location_air_ship(player, command, param1, param2),
            VehicleInput::MoveInAirShip {
                ship_id,
                target,
                origin,
            } => self.on_move_to_location_in_air_ship(player, ship_id, target, origin),
        }
    }

    /// Java `RequestGetOnVehicle.runImpl`.
    fn on_request_get_on_vehicle(&mut self, player: ObjectId, boat_id: i32, pos: (i32, i32, i32)) {
        let boat = ObjectId(boat_id);
        let boat = if self.is_in_boat(player) {
            let Some(current) = self.vehicle_of(player) else {
                return;
            };
            if current != boat {
                self.send(player, &ActionFailed);
                return;
            }
            current
        } else {
            let player_pos = self.position_of(player);
            let usable = self.vehicle.boat(boat).is_some_and(|b| {
                !b.is_moving() && player_pos.is_some_and(|p| distance_3d(p, (b.x, b.y, b.z)) < BOARDING_RADIUS)
            });
            if !usable {
                self.send(player, &ActionFailed);
                return;
            }
            boat
        };
        let Some((bx, by, bz)) = self.vehicle_position(boat) else {
            return;
        };
        self.set_in_vehicle_position(player, Some(pos));
        self.set_rider_vehicle(player, Some(boat));
        self.broadcast_with_self(
            player,
            &GetOnVehicle {
                object_id: player.0,
                boat_object_id: boat.0,
                x: pos.0,
                y: pos.1,
                z: pos.2,
            },
        );
        self.set_xyz(player, bx, by, bz);
        self.set_inside(player, ZoneId::Peace, true);
        self.revalidate_zone(player, true);
    }

    /// Java `RequestGetOffVehicle.runImpl`.
    fn on_request_get_off_vehicle(&mut self, player: ObjectId, boat_id: i32, (x, y, z): (i32, i32, i32)) {
        let boat = ObjectId(boat_id);
        let near = self
            .position_of(player)
            .is_some_and(|p| distance_3d(p, (x, y, z)) < BOARDING_RADIUS);
        let valid = self.is_in_boat(player)
            && self.vehicle_of(player) == Some(boat)
            && !self.veh(boat).is_some_and(|b| b.is_moving())
            && near;
        if !valid {
            self.send(player, &ActionFailed);
            return;
        }
        self.broadcast_stop_move_in_vehicle(player, boat);
        self.set_rider_vehicle(player, None);
        self.set_in_vehicle_position(player, None);
        self.send(player, &ActionFailed);
        self.broadcast_with_self(
            player,
            &GetOffVehicle {
                object_id: player.0,
                boat_object_id: boat.0,
                x,
                y,
                z,
            },
        );
        self.set_xyz(player, x, y, z);
        self.set_inside(player, ZoneId::Peace, false);
        self.revalidate_zone(player, true);
    }

    /// `new StopMoveInVehicle(player, boatId)` to the player and everybody around.
    fn broadcast_stop_move_in_vehicle(&self, player: ObjectId, boat: ObjectId) {
        // `player.getInVehiclePosition()` is never null where this is called from.
        let Some((x, y, z)) = self.in_vehicle_position(player) else {
            return;
        };
        let heading = self.heading_of(player);
        self.broadcast_with_self(
            player,
            &StopMoveInVehicle {
                object_id: player.0,
                boat_id: boat.0,
                x,
                y,
                z,
                heading,
            },
        );
    }

    /// Java `RequestMoveToLocationInVehicle.runImpl`.
    fn on_request_move_to_location_in_vehicle(
        &mut self,
        player: ObjectId,
        boat_id: i32,
        target: (i32, i32, i32),
        origin: (i32, i32, i32),
    ) {
        let is_gm = self.is_gm(player);
        let not_move_until = self.players.get(&player).map_or(0, |p| p.movement.not_move_until_ms);
        if self.settings.player_movement_block_time_ms > 0 && !is_gm && not_move_until > self.world_ms() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotMoveWhileSpeakingToAnNpcOneMomentPlease),
            );
            self.send(player, &ActionFailed);
            return;
        }
        if target == origin {
            if let Some((x, y, z)) = self.in_vehicle_position(player) {
                let heading = self.heading_of(player);
                self.send(
                    player,
                    &StopMoveInVehicle {
                        object_id: player.0,
                        boat_id,
                        x,
                        y,
                        z,
                        heading,
                    },
                );
            }
            return;
        }
        if self.is_attacking_now(player) && self.active_weapon_is_bow(player) {
            self.send(player, &ActionFailed);
            return;
        }
        if self.is_sitting(player) || self.is_movement_disabled(player) {
            self.send(player, &ActionFailed);
            return;
        }
        if self.summon_of(player).is_some() {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouShouldReleaseYourPetOrServitorSoThatItDoesNotFallOffOfTheBoatAndDrown,
                ),
            );
            self.send(player, &ActionFailed);
            return;
        }
        if self.is_transformed(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotPolymorphWhileRidingABoat),
            );
            self.send(player, &ActionFailed);
            return;
        }

        let boat = ObjectId(boat_id);
        // The boat the player is on, or the one the packet names (the player is put on it).
        let boat = match self.vehicle_of(player).filter(|_| self.is_in_boat(player)) {
            Some(current) if current == boat => current,
            _ => {
                if self.vehicle.boat(boat).is_some() {
                    self.set_rider_vehicle(player, Some(boat));
                }
                boat
            }
        };
        self.set_in_vehicle_position(player, Some(target));
        self.broadcast_with_self(
            player,
            &MoveToLocationInVehicle {
                object_id: player.0,
                boat_id: boat.0,
                destination_x: target.0,
                destination_y: target.1,
                destination_z: target.2,
                origin_x: origin.0,
                origin_y: origin.1,
                origin_z: origin.2,
            },
        );
    }

    /// Java `CannotMoveAnymoreInVehicle.runImpl`.
    fn on_cannot_move_anymore_in_vehicle(
        &mut self,
        player: ObjectId,
        boat_id: i32,
        pos: (i32, i32, i32),
        heading: i32,
    ) {
        let boat = ObjectId(boat_id);
        if self.is_in_boat(player) && self.vehicle_of(player) == Some(boat) {
            self.set_in_vehicle_position(player, Some(pos));
            self.set_heading(player, heading);
            self.broadcast_stop_move_in_vehicle(player, boat);
        }
    }

    /// Java `MoveToLocationInAirShip.runImpl`.
    fn on_move_to_location_in_air_ship(
        &mut self,
        player: ObjectId,
        ship_id: i32,
        target: (i32, i32, i32),
        origin: (i32, i32, i32),
    ) {
        let ship = ObjectId(ship_id);
        if target == origin {
            if let Some((x, y, z)) = self.in_vehicle_position(player) {
                let heading = self.heading_of(player);
                self.send(
                    player,
                    &StopMoveInVehicle {
                        object_id: player.0,
                        boat_id: ship_id,
                        x,
                        y,
                        z,
                        heading,
                    },
                );
            }
            return;
        }
        if self.is_attacking_now(player) && self.active_weapon_is_bow(player) {
            self.send(player, &ActionFailed);
            return;
        }
        if self.is_sitting(player) || self.is_movement_disabled(player) {
            self.send(player, &ActionFailed);
            return;
        }
        if !self.is_in_air_ship(player) || self.vehicle_of(player) != Some(ship) {
            self.send(player, &ActionFailed);
            return;
        }
        self.set_in_vehicle_position(player, Some(target));
        let heading = self.heading_of(player);
        self.broadcast_with_self(
            player,
            &ExMoveToLocationInAirShip {
                object_id: player.0,
                air_ship_id: ship_id,
                destination_x: target.0,
                destination_y: target.1,
                destination_z: target.2,
                heading,
            },
        );
    }

    /// `RequestActionUse` 67 to 70: steer, cancel control, the destination map, exit the ship.
    pub(crate) fn air_ship_action_use(&mut self, player: ObjectId, action_id: i32) {
        let ship = self.vehicle_of(player).filter(|_| self.is_in_air_ship(player));
        match action_id {
            // Steer
            67 => {
                if let Some(ship) = ship
                    && self.ship_set_captain(ship, Some(player))
                {
                    self.broadcast_user_info(player);
                }
            }
            // Cancel Control
            68 => {
                if let Some(ship) = ship
                    && self.ship_is_captain(ship, player)
                    && self.ship_set_captain(ship, None)
                {
                    self.broadcast_user_info(player);
                }
            }
            // Destination Map
            69 => self.send_air_ship_teleport_list(player),
            // Exit Airship
            70 => {
                let Some(ship) = ship else {
                    return;
                };
                if self.ship_is_captain(ship, player) {
                    if self.ship_set_captain(ship, None) {
                        self.broadcast_user_info(player);
                    }
                } else if self.veh(ship).is_some_and(|v| v.is_in_dock()) {
                    self.vehicle_oust_player(ship, player);
                }
            }
            _ => {}
        }
    }

    /// `getActiveWeaponItem().getItemType() == WeaponType.BOW`.
    fn active_weapon_is_bow(&self, player: ObjectId) -> bool {
        self.stats(player)
            .and_then(|s| s.active_weapon_item())
            .and_then(|t| t.weapon())
            .is_some_and(|w| w.weapon_type == l2_data::item::WeaponType::Bow)
    }
}
