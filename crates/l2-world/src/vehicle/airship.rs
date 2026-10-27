//! `AirShipManager` and `ControllableAirShip`: licenses, summoning a ship, the captain, the
//! destination list of a dock and the commands of the control panel.
// Java: managers/AirShipManager.java
// Java: entity/actor/instance/{AirShip,ControllableAirShip}.java
// Java: network/clientpackets/{MoveToLocationAirShip,RequestActionUse}.java (the air ship actions)
// Java: dist/game/data/scripts/handlers/skill/effects/RefuelAirship.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{AirShipTeleport, ExAirShipTeleportList, SystemMessage};
use l2_util::ObjectId;

use super::route::PathPoint;
use super::{VehicleKind, VehicleWrite};
use crate::outbox::PersistCommand;
use crate::world::World;

/// `ControllableAirShip.HELM`.
const HELM_ITEM_ID: i32 = 13556;
/// `AirShipManager`: a new ship has 600 fuel and flies at 280 with a turn of 2000.
const MAX_FUEL: i32 = 600;
const AIR_SHIP_MOVE_SPEED: f32 = 280.0;
const AIR_SHIP_ROTATION_SPEED: i32 = 2000;
/// `MoveToLocationAirShip.MIN_Z`, `MAX_Z` and `STEP`.
const STEP: i32 = 300;
/// `World.GRACIA_MAX_X`, `GRACIA_MAX_Z` and `GRACIA_MIN_Z`.
const GRACIA_MAX_X: i32 = -166_168;
const GRACIA_MAX_Z: i32 = 6105;
const GRACIA_MIN_Z: i32 = -895;
/// `setCaptain`: the helm is at (0x16e, 0, 0x6b) of the ship and the captain must be within 50.
const HELM_X: i32 = 0x16e;
const HELM_Z: i32 = 0x6b;
const HELM_REACH_SQ: i32 = 2500;

/// `AirShipManager.AirShipTeleportList`: where a dock sends ships and what it costs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TeleportList {
    pub(crate) location: i32,
    pub(crate) fuel: &'static [i32],
    pub(crate) routes: &'static [&'static [PathPoint]],
}

impl World {
    /// Java `AirShipManager.hasAirShipLicense(ownerId)`.
    pub fn has_air_ship_license(&self, owner_id: i32) -> bool {
        self.vehicle.licenses.contains_key(&owner_id)
    }

    /// Java `AirShipManager.registerLicense(ownerId)`: 600 fuel and a new row.
    pub fn register_air_ship_license(&mut self, owner_id: i32) {
        if self.vehicle.licenses.contains_key(&owner_id) {
            return;
        }
        self.vehicle.licenses.insert(owner_id, MAX_FUEL);
        self.outbox.persist(PersistCommand::Vehicle(VehicleWrite::AddLicense {
            owner_id,
            fuel: MAX_FUEL,
        }));
    }

    /// Java `AirShipManager.hasAirShip(ownerId)`: the clan's ship is in the world (or on its way
    /// through a teleport).
    pub fn has_air_ship(&self, owner_id: i32) -> bool {
        self.vehicle
            .owner_ships
            .get(&owner_id)
            .and_then(|id| self.veh(*id).map(|v| (id, v)))
            .is_some_and(|(id, v)| self.objects.get(id).is_some_and(|e| e.spawned) || v.teleporting)
    }

    /// Java `AirShipManager.getNewAirShip(x, y, z, heading)`: a ship nobody steers.
    pub(crate) fn new_air_ship(&mut self, at: (i32, i32, i32), heading: i32) -> Option<ObjectId> {
        let id = self.create_vehicle(VehicleKind::AirShip, at, heading, None, |_| {})?;
        if let Some(v) = self.veh_mut(id) {
            v.move_speed = AIR_SHIP_MOVE_SPEED;
            v.rotation_speed = AIR_SHIP_ROTATION_SPEED;
        }
        Some(id)
    }

    /// Java `AirShipManager.getNewAirShip(x, y, z, heading, ownerId)`: the clan's ship, with the
    /// fuel of its license. `None` without a license.
    pub(crate) fn new_controllable_air_ship(
        &mut self,
        at: (i32, i32, i32),
        heading: i32,
        owner_id: i32,
    ) -> Option<ObjectId> {
        let fuel = self.vehicle.licenses.get(&owner_id).copied()?;
        let id = self.create_vehicle(VehicleKind::AirShip, at, heading, Some(owner_id), |v| {
            if let Some(ship) = v.ship.as_mut() {
                ship.max_fuel = MAX_FUEL;
                ship.fuel = fuel.clamp(0, MAX_FUEL);
            }
            v.move_speed = AIR_SHIP_MOVE_SPEED;
            v.rotation_speed = AIR_SHIP_ROTATION_SPEED;
        })?;
        self.vehicle.owner_ships.insert(owner_id, id);
        Some(id)
    }

    /// Java `AirShipManager.registerAirShipTeleportList(...)`: ignored when the lists differ in
    /// length.
    pub(crate) fn register_air_ship_teleport_list(
        &mut self,
        dock_id: i32,
        location: i32,
        routes: &'static [&'static [PathPoint]],
        fuel: &'static [i32],
    ) {
        if routes.len() != fuel.len() {
            return;
        }
        self.vehicle.teleports.insert(
            dock_id,
            TeleportList {
                location,
                fuel,
                routes,
            },
        );
    }

    /// Java `AirShipManager.getTeleportDestination(dockId, index)`.
    pub(crate) fn air_ship_teleport_destination(&self, dock_id: i32, index: i32) -> Option<&'static [PathPoint]> {
        let all = self.vehicle.teleports.get(&dock_id)?;
        let len = i32::try_from(all.routes.len()).ok()?;
        if index < -1 || index >= len {
            return None;
        }
        // NOTE(java-quirk): `index == len - 1` passes the check and reads one past the end (an
        // exception in Java, no destination here).
        all.routes.get(usize::try_from(index + 1).ok()?).copied()
    }

    /// Java `AirShipManager.getFuelConsumption(dockId, index)`.
    pub(crate) fn air_ship_fuel_consumption(&self, dock_id: i32, index: i32) -> i32 {
        let Some(all) = self.vehicle.teleports.get(&dock_id) else {
            return 0;
        };
        let len = i32::try_from(all.fuel.len()).unwrap_or(i32::MAX);
        if index < -1 || index >= len {
            return 0;
        }
        usize::try_from(index + 1)
            .ok()
            .and_then(|i| all.fuel.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// Java `AirShipManager.sendAirShipTeleportList(player)`: the captain of a ship that is in a
    /// dock and not moving gets the destinations.
    pub(crate) fn send_air_ship_teleport_list(&mut self, player: ObjectId) {
        if !self.players.contains_key(&player) || !self.is_in_air_ship(player) {
            return;
        }
        let Some(ship) = self.vehicle_of(player) else {
            return;
        };
        if !self.ship_is_captain(ship, player) {
            return;
        }
        let Some(v) = self.veh(ship) else {
            return;
        };
        if !v.is_in_dock() || v.is_moving() {
            return;
        }
        let Some(all) = self.vehicle.teleports.get(&v.dock_id) else {
            return;
        };
        let teleports = all
            .routes
            .iter()
            .enumerate()
            .filter_map(|(i, route)| {
                let dst = route.last()?;
                Some(AirShipTeleport {
                    index: i32::try_from(i).ok()? - 1,
                    fuel: all.fuel.get(i).copied()?,
                    x: dst.x,
                    y: dst.y,
                    z: dst.z,
                })
            })
            .collect();
        let packet = ExAirShipTeleportList {
            dock_id: all.location,
            teleports,
        };
        self.send(player, &packet);
    }

    /// Java `ControllableAirShip.isCaptain(player)`.
    pub fn ship_is_captain(&self, ship: ObjectId, player: ObjectId) -> bool {
        self.veh(ship)
            .and_then(|v| v.ship.as_ref())
            .is_some_and(|s| s.captain == Some(player))
    }

    /// Java `ControllableAirShip.isOwner(player)`: the owner is the player's clan or the player.
    pub fn ship_is_owner(&self, ship: ObjectId, player: ObjectId) -> bool {
        let Some(owner) = self.veh(ship).and_then(|v| v.ship.as_ref()).map(|s| s.owner_id) else {
            return false;
        };
        if owner == 0 {
            return false;
        }
        self.clan_id_of(player) == owner || player.0 == owner
    }

    /// Java `ControllableAirShip.setCaptain(player)`: `None` releases the helm. A player who
    /// cannot steer is told why and `false` comes back.
    pub fn ship_set_captain(&mut self, ship: ObjectId, player: Option<ObjectId>) -> bool {
        use SystemMessageId as M;
        let Some(v) = self.veh(ship) else {
            return false;
        };
        if v.ship.is_none() {
            return false;
        }
        match player {
            None => {
                if let Some(s) = self.veh_mut(ship).and_then(|v| v.ship.as_mut()) {
                    s.captain = None;
                }
            }
            Some(player) => {
                let free = v.ship.as_ref().is_some_and(|s| s.captain.is_none());
                if !free || self.vehicle_of(player) != Some(ship) {
                    return false;
                }
                let (x, y, z) = self.in_vehicle_position(player).unwrap_or((0, 0, 0));
                let (dx, dy, dz) = (x - HELM_X, y, z - HELM_Z);
                if dx * dx + dy * dy + dz * dz > HELM_REACH_SQ {
                    self.send(player, &SystemMessage::new(M::YouCannotControlBecauseYouAreTooFar));
                    return false;
                }
                // "TODO: Missing message ID: 2739 Message: You cannot control the helm because you
                // do not meet the requirements."
                let refusal = if self.is_in_combat(player) {
                    Some(M::YouCannotControlTheHelmWhileInABattle)
                } else if self.is_sitting(player) {
                    Some(M::YouCannotControlTheHelmWhileInASittingPosition)
                } else if self.is_paralyzed(player) {
                    Some(M::YouCannotControlTheHelmWhileYouArePetrified)
                } else if self.is_cursed_weapon_equipped(player) {
                    Some(M::YouCannotControlTheHelmWhileACursedWeaponIsEquipped)
                } else if self.is_fishing(player) {
                    Some(M::YouCannotControlTheHelmWhileFishing)
                } else if self.is_dead(player) || self.is_fake_death(player) {
                    Some(M::YouCannotControlTheHelmWhenYouAreDead)
                } else if self.is_casting_now(player) {
                    Some(M::YouCannotControlTheHelmWhileUsingASkill)
                } else if self.is_transformed(player) {
                    Some(M::YouCannotControlTheHelmWhileTransformed)
                } else if self.is_combat_flag_equipped(player) {
                    Some(M::YouCannotControlTheHelmWhileHoldingAFlag)
                } else if self.is_in_duel(player) {
                    Some(M::YouCannotControlTheHelmWhileInADuel)
                } else {
                    None
                };
                if let Some(message) = refusal {
                    self.send(player, &SystemMessage::new(message));
                    return false;
                }
                if let Some(s) = self.veh_mut(ship).and_then(|v| v.ship.as_mut()) {
                    s.captain = Some(player);
                }
                self.broadcast_user_info(player);
            }
        }
        self.update_air_ship_info(ship);
        true
    }

    /// The air ship the player steers: `isInAirShip() && getAirShip().isCaptain(player)`.
    pub(crate) fn steered_air_ship(&self, player: ObjectId) -> Option<ObjectId> {
        let ship = self.vehicle_of(player).filter(|_| self.is_in_air_ship(player))?;
        self.ship_is_captain(ship, player).then_some(ship)
    }

    /// Java `AirShip.setHeading(degree)`.
    pub(crate) fn set_air_ship_heading(&mut self, ship: ObjectId, heading: i32) {
        if let Some(v) = self.veh_mut(ship) {
            v.heading = heading;
        }
    }

    /// `UserInfo`'s `airShipHelm`: the helm item of the ship the player steers, else 0.
    pub(crate) fn air_ship_helm_item(&self, player: ObjectId) -> i32 {
        match self.vehicle_of(player) {
            Some(ship) if self.ship_is_captain(ship, player) => HELM_ITEM_ID,
            _ => 0,
        }
    }

    /// Java `ConditionPlayerCanRefuelAirship`.
    pub fn can_refuel_air_ship(&self, player: ObjectId, value: i32) -> bool {
        self.players.contains_key(&player)
            && self
                .vehicle_of(player)
                .and_then(|s| self.veh(s))
                .and_then(|v| v.ship.as_ref())
                .is_some_and(|s| s.fuel.saturating_add(value) <= s.max_fuel)
    }

    /// Java `RefuelAirship.onStart`: `ship.setFuel(ship.getFuel() + value)`, then the ship is
    /// described again.
    pub fn refuel_air_ship(&mut self, player: ObjectId, value: i32) {
        let Some(ship) = self.vehicle_of(player).filter(|s| self.veh(*s).is_some_and(|v| v.is_air_ship())) else {
            return;
        };
        let fuel = self.veh(ship).map_or(0, |v| v.fuel());
        self.ship_set_fuel(ship, fuel.saturating_add(value));
        self.update_air_ship_info(ship);
    }

    /// Java `MoveToLocationAirShip.runImpl`: the commands of the control panel.
    pub(crate) fn on_move_to_location_air_ship(&mut self, player: ObjectId, command: i32, param1: i32, param2: i32) {
        if !self.players.contains_key(&player) || !self.is_in_air_ship(player) {
            return;
        }
        let Some(ship) = self.vehicle_of(player) else {
            return;
        };
        if !self.ship_is_captain(ship, player) {
            return;
        }
        let Some(v) = self.veh(ship) else {
            return;
        };
        let (sx, sy, mut z) = (v.x, v.y, v.z);
        // `canBeControlled()`: no engine, and for a steered ship also not in a dock.
        let can_be_controlled = v.engine.is_none() && !(v.ship.is_some() && v.is_in_dock());
        match command {
            0 => {
                if !can_be_controlled {
                    return;
                }
                if param1 < GRACIA_MAX_X {
                    self.vehicle_set_intention_move_to(ship, param1, param2, z);
                }
            }
            1 => {
                if !can_be_controlled {
                    return;
                }
                self.vehicle_set_intention_active(ship);
            }
            2 => {
                if !can_be_controlled {
                    return;
                }
                if z < GRACIA_MAX_Z {
                    z = (z + STEP).min(GRACIA_MAX_Z);
                    self.vehicle_set_intention_move_to(ship, sx, sy, z);
                }
            }
            3 => {
                if !can_be_controlled {
                    return;
                }
                if z > GRACIA_MIN_Z {
                    z = (z - STEP).max(GRACIA_MIN_Z);
                    self.vehicle_set_intention_move_to(ship, sx, sy, z);
                }
            }
            4 => {
                if !v.is_in_dock() || v.is_moving() {
                    return;
                }
                let dock_id = v.dock_id;
                let Some(destination) = self.air_ship_teleport_destination(dock_id, param1) else {
                    return;
                };
                // Consume fuel, if needed.
                let consumption = self.air_ship_fuel_consumption(dock_id, param1);
                if consumption > 0 {
                    let fuel = self.veh(ship).map_or(0, |v| v.fuel());
                    if consumption > fuel {
                        self.send(
                            player,
                            &SystemMessage::new(
                                SystemMessageId::YourShipCannotTeleportBecauseItDoesNotHaveEnoughFuelForTheTrip,
                            ),
                        );
                        return;
                    }
                    self.ship_set_fuel(ship, fuel - consumption);
                }
                self.vehicle_execute_path(ship, Some(destination));
            }
            _ => {}
        }
    }
}
