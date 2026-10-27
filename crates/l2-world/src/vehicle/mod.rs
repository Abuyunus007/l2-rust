//! Boats and air ships (M8-VEHICLE-01): `BoatManager`, `AirShipManager`, `Vehicle`, `Boat`,
//! `AirShip`, `ControllableAirShip`, the five ferry routes and the air ship docks.
// Java: managers/{BoatManager,AirShipManager}.java
// Java: entity/actor/Vehicle.java, entity/actor/instance/{Boat,AirShip,ControllableAirShip}.java
// Java: entity/actor/stat/{VehicleStat,ControllableAirShipStat}.java, ai/{BoatAI,AirShipAI}.java
// Java: network/clientpackets/{RequestGetOnVehicle,RequestGetOffVehicle,
//       RequestMoveToLocationInVehicle,CannotMoveAnymoreInVehicle,MoveToLocationAirShip,
//       MoveToLocationInAirShip}.java
// Java: dist/game/data/scripts/vehicles/*.java (the ferries, `boat.rs`, `boat_routes.rs`)
// Java: dist/game/data/scripts/ai/areas/Gracia/vehicles/** (the docks and the Gludio-Gracia ship,
//       `controller.rs`)
//
// # How it fits in
// - A vehicle is an object of the world ([`ObjectKind::Vehicle`](crate::objects::ObjectKind)) with
//   its state in [`VehicleWorld`]: it is seen through the region grid like any creature, moves on
//   the 50 ms tick with the arithmetic of `Creature.updatePosition` (`movement.rs`) and talks
//   through the packets of `l2_protocol_game::server::vehicle`.
// - Who rides what is [`VehicleWorld::riders`], not a field of the player (D-11: ids only). The
//   passengers of a vehicle are a list of player ids ([`Vehicle::passengers`]); a boat only counts
//   the players who paid at the start of a trip (`payForRide`), an air ship the ones who boarded.
// - Java's scheduled tasks (`ThreadPool.schedule*`) are [`VehicleTimer`]s on the wheel (D-12).
//   The boat scripts' `Runnable` is [`Engine`]: the state machine of `run()` with the `case`s as
//   data (`boat_routes.rs`), run by the `Engine` timer and by every arrival at the end of a path.
// - The air ship docks (`AirShipController` and its subclasses) and the scheduled ship between
//   Gludio and Gracia are built in (`controller.rs`): their NPC dialogs go through the `Script`
//   bypass like the other built-in scripts.
//
// # Database (D-39)
// `airships` (owner_id, fuel) is read at start (`AirShipManager.load`), a license is a new row
// (`registerLicense`) and a ship that is deleted stores its fuel (`removeAirShip`), as Java does;
// the rows belong to a clan or a player and are not one of the global cycles of D-53.
//
// # What is left out
// - The cell based zone bookkeeping of non-player creatures (a vehicle leaving a zone cell is
//   dropped from the zones of the cell): the docks test their zone at every revalidation.
// - The movie holder (`Player.playMovie` keeps the movie until `EndScenePlayer`): the passengers
//   get `ExStartScenePlayer` and nothing else.

mod airship;
mod boat;
pub mod boat_routes;
pub mod controller;
mod input;
mod movement;
mod object;
mod passengers;
pub mod route;

use std::collections::HashMap;

use l2_util::ObjectId;

use crate::timer::TimerId;
use crate::world::World;

pub use input::VehicleInput;
pub use route::{BoatRoute, BoatStep, PathPoint};

/// Java `Intention`, as far as the vehicles' AIs use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VIntention {
    Idle,
    Active,
    MoveTo,
}

/// Which class the vehicle is: `Boat`, or an air ship (`AirShip` / `ControllableAirShip`, the
/// latter has [`Vehicle::ship`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleKind {
    Boat,
    AirShip,
}

/// Java `Creature.MoveData` of a vehicle: one straight leg.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VehicleMove {
    pub(crate) move_start_time: i32,
    pub(crate) move_timestamp: i32,
    pub(crate) x_destination: i32,
    pub(crate) y_destination: i32,
    pub(crate) z_destination: i32,
    pub(crate) x_accurate: f64,
    pub(crate) y_accurate: f64,
    pub(crate) last_broadcast_time: i32,
}

/// The `Runnable` a vehicle runs when it arrives or its timer fires (`Vehicle.registerEngine`).
#[derive(Debug, Clone, PartialEq)]
pub enum Engine {
    /// One of the boat scripts: the route (index into [`boat_routes::BOAT_ROUTES`]) and where it
    /// is in its `switch`.
    Boat {
        route: usize,
        cycle: usize,
        shout_count: i32,
    },
    /// `AirShipGludioGracia`.
    GludioGracia(controller::GludioGraciaState),
}

/// What `ControllableAirShip` adds to an air ship: the owner, the fuel, the helm and the captain.
#[derive(Debug, Clone, PartialEq)]
pub struct ShipState {
    /// Java `_ownerId`: the clan (or character) the ship belongs to.
    pub owner_id: i32,
    pub fuel: i32,
    pub max_fuel: i32,
    /// Java `_captain`.
    pub captain: Option<ObjectId>,
    /// Java `_helmId`: the object id of the helm the captain steers with.
    pub helm_id: ObjectId,
    /// How many pairs of `_checkTask` and `_consumeFuelTask` run: a ship that was teleported has
    /// more than one, because Java schedules both again in every `onSpawn`. The tasks end by
    /// themselves when the ship is gone.
    pub task_pairs: u32,
}

/// Java `Vehicle` with `Boat`, `AirShip` and `ControllableAirShip` folded in (D-17).
#[derive(Debug, Clone)]
pub struct Vehicle {
    pub object_id: ObjectId,
    pub kind: VehicleKind,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// `VehicleStat._moveSpeed` (a float in Java).
    pub move_speed: f32,
    /// `VehicleStat._rotationSpeed`.
    pub rotation_speed: i32,
    /// Java `_dockId`.
    pub dock_id: i32,
    /// Java `_passengers`.
    pub passengers: Vec<ObjectId>,
    /// Java `_oustLoc`.
    pub oust_loc: Option<(i32, i32, i32)>,
    /// Java `_engine`.
    pub engine: Option<Engine>,
    pub(crate) intention: VIntention,
    /// Java `_currentPath` and `_runState`.
    pub(crate) path: Option<&'static [PathPoint]>,
    pub(crate) run_state: i32,
    pub(crate) mv: Option<VehicleMove>,
    /// `_monitorTask` and `_monitorLocation`.
    pub(crate) monitor: Option<TimerId>,
    pub(crate) monitor_loc: (i32, i32, i32),
    /// Java `isTeleporting()`.
    pub teleporting: bool,
    /// `_lastZoneValidateLocation`.
    pub(crate) last_zone_validate: (i32, i32, i32),
    /// The dock zones (indices into [`VehicleWorld::controllers`]) the ship is inside.
    pub(crate) docks_inside: Vec<usize>,
    /// Set for a `ControllableAirShip`.
    pub ship: Option<ShipState>,
}

impl Vehicle {
    fn new(object_id: ObjectId, kind: VehicleKind, x: i32, y: i32, z: i32, heading: i32) -> Self {
        Self {
            object_id,
            kind,
            x,
            y,
            z,
            heading,
            move_speed: 0.0,
            rotation_speed: 0,
            dock_id: 0,
            passengers: Vec::new(),
            oust_loc: None,
            engine: None,
            intention: VIntention::Idle,
            path: None,
            run_state: 0,
            mv: None,
            monitor: None,
            // `new Location(this)` in the field initializer: the vehicle has no position yet.
            monitor_loc: (0, 0, 0),
            teleporting: false,
            last_zone_validate: (0, 0, 0),
            docks_inside: Vec::new(),
            ship: None,
        }
    }

    pub fn is_boat(&self) -> bool {
        self.kind == VehicleKind::Boat
    }

    pub fn is_air_ship(&self) -> bool {
        self.kind == VehicleKind::AirShip
    }

    /// Java `isInDock()`.
    pub fn is_in_dock(&self) -> bool {
        self.dock_id > 0
    }

    /// Java `isMoving()`.
    pub fn is_moving(&self) -> bool {
        self.mv.is_some()
    }

    /// Java `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.passengers.is_empty()
    }

    /// Java `ControllableAirShipStat.getMoveSpeed()` / `VehicleStat.getMoveSpeed()`: a ship without
    /// fuel flies at 5 % of its speed unless it is in a dock.
    pub fn current_move_speed(&self) -> f64 {
        let speed = f64::from(self.move_speed);
        match &self.ship {
            Some(ship) if !(self.is_in_dock() || ship.fuel > 0) => speed * f64::from(0.05_f32),
            _ => speed,
        }
    }

    /// Java `getCaptainId()`.
    pub fn captain_id(&self) -> i32 {
        self.ship
            .as_ref()
            .and_then(|s| s.captain)
            .map_or(0, |c| c.0)
    }

    /// Java `getHelmObjectId()`.
    pub fn helm_object_id(&self) -> i32 {
        self.ship.as_ref().map_or(0, |s| s.helm_id.0)
    }

    pub fn fuel(&self) -> i32 {
        self.ship.as_ref().map_or(0, |s| s.fuel)
    }

    pub fn max_fuel(&self) -> i32 {
        self.ship.as_ref().map_or(0, |s| s.max_fuel)
    }
}

/// Who rides which vehicle (Java `Player._vehicle` and `_inVehiclePosition`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rider {
    pub vehicle: Option<ObjectId>,
    pub position: Option<(i32, i32, i32)>,
}

/// The values the vehicles read. `Default` is Java's default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleSettings {
    /// `GeneralConfig.ALLOW_BOAT` (`AllowBoat`).
    pub allow_boat: bool,
    /// `GeneralConfig.BOAT_BROADCAST_RADIUS` (`BoatBroadcastRadius`).
    pub boat_broadcast_radius: i32,
}

impl Default for VehicleSettings {
    fn default() -> Self {
        Self {
            allow_boat: true,
            boat_broadcast_radius: 20_000,
        }
    }
}

/// A row of `airships`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirshipRow {
    pub owner_id: i32,
    pub fuel: i32,
}

/// What the game server read from `airships` before the world started (`AirShipManager.load`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VehicleLoad {
    pub airships: Vec<AirshipRow>,
}

/// A write of the air ship rows with Java's statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VehicleWrite {
    /// `INSERT INTO airships (owner_id,fuel) VALUES (?,?)` (`registerLicense`).
    AddLicense { owner_id: i32, fuel: i32 },
    /// `UPDATE airships SET fuel=? WHERE owner_id=?` (`storeInDb`).
    StoreFuel { owner_id: i32, fuel: i32 },
}

/// The tasks of the vehicles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleTimer {
    /// `ThreadPool.schedule(_engine, delay)` (`runEngine`).
    Engine(ObjectId),
    /// `_monitorTask`: every second, is the vehicle stuck?
    Monitor(ObjectId),
    /// `ControllableAirShip.CheckTask`: every 10 s after 60 s.
    ShipCheck(ObjectId),
    /// `ControllableAirShip.ConsumeFuelTask`: every 60 s after 60 s.
    ShipFuel(ObjectId),
    /// `ControllableAirShip.DecayTask` (`ThreadPool.execute`).
    ShipDecay(ObjectId),
    /// `AirShipController.DepartTask` of the dock with this index.
    ControllerDepart(usize),
    /// `AirShipController.DecayTask` of the dock with this index.
    ControllerDecay(usize),
}

/// The state of `BoatManager` and `AirShipManager` (one field of the [`World`]).
#[derive(Debug, Default)]
pub struct VehicleWorld {
    pub(crate) settings: VehicleSettings,
    pub(crate) vehicles: HashMap<ObjectId, Vehicle>,
    /// Vehicles with a running move, in the order they started.
    pub(crate) moving: Vec<ObjectId>,
    /// `BoatManager._boats`, in creation order.
    pub(crate) boats: Vec<ObjectId>,
    /// `BoatManager._docksBusy`: three flags, the dock numbers go 1 to 3.
    pub(crate) docks_busy: [bool; 3],
    pub(crate) riders: HashMap<ObjectId, Rider>,
    /// `AirShipManager._airShipsInfo`: owner to fuel.
    pub(crate) licenses: HashMap<i32, i32>,
    /// `AirShipManager._airShips`, as far as it matters: the ship of each owner that is in the
    /// world.
    pub(crate) owner_ships: HashMap<i32, ObjectId>,
    /// `AirShipManager._teleports`: dock to destinations.
    pub(crate) teleports: HashMap<i32, airship::TeleportList>,
    /// The `AirShipController` scripts.
    pub(crate) controllers: Vec<controller::Controller>,
    /// The `AirShipGludioGracia` ship.
    pub(crate) gludio_gracia: Option<ObjectId>,
}

impl VehicleWorld {
    /// The vehicle with this id.
    pub fn vehicle(&self, id: ObjectId) -> Option<&Vehicle> {
        self.vehicles.get(&id)
    }

    /// Java `BoatManager.getBoat(id)`.
    pub fn boat(&self, id: ObjectId) -> Option<&Vehicle> {
        self.vehicles.get(&id).filter(|v| v.is_boat())
    }

    /// The `AirShipGludioGracia` ship.
    pub fn gludio_gracia_ship(&self) -> Option<ObjectId> {
        self.gludio_gracia
    }

    /// The boats, in the order they were created.
    pub fn boats(&self) -> &[ObjectId] {
        &self.boats
    }
}

impl World {
    /// Installs the configuration values the vehicles read. Call before [`Self::spawn_boats`].
    pub fn set_vehicle_settings(&mut self, settings: VehicleSettings) {
        self.vehicle.settings = settings;
    }

    /// `AirShipManager.load()`: the licenses of the clans.
    pub fn load_airships(&mut self, load: VehicleLoad) {
        for row in &load.airships {
            self.vehicle.licenses.insert(row.owner_id, row.fuel);
        }
        tracing::info!("AirShipManager: Loaded {} private airships", self.vehicle.licenses.len());
    }

    /// The vehicles and the riders.
    pub fn vehicles(&self) -> &VehicleWorld {
        &self.vehicle
    }

    /// Java `Player.getVehicle()`.
    pub fn vehicle_of(&self, player: ObjectId) -> Option<ObjectId> {
        self.vehicle.riders.get(&player).and_then(|r| r.vehicle)
    }

    /// Java `Player.getInVehiclePosition()`.
    pub fn in_vehicle_position(&self, player: ObjectId) -> Option<(i32, i32, i32)> {
        self.vehicle.riders.get(&player).and_then(|r| r.position)
    }

    /// Java `Player.isInVehicle()`.
    pub fn is_in_vehicle(&self, player: ObjectId) -> bool {
        self.vehicle_of(player).is_some()
    }

    /// Java `Player.isInBoat()`.
    pub fn is_in_boat(&self, player: ObjectId) -> bool {
        self.vehicle_of(player)
            .and_then(|v| self.vehicle.vehicles.get(&v))
            .is_some_and(Vehicle::is_boat)
    }

    /// Java `Player.isInAirShip()`.
    pub fn is_in_air_ship(&self, player: ObjectId) -> bool {
        self.vehicle_of(player)
            .and_then(|v| self.vehicle.vehicles.get(&v))
            .is_some_and(Vehicle::is_air_ship)
    }

    /// The vehicle timers of [`crate::TimerKind`].
    pub(crate) fn on_vehicle_timer(&mut self, timer: VehicleTimer) {
        match timer {
            VehicleTimer::Engine(id) => self.vehicle_run_engine_now(id),
            VehicleTimer::Monitor(id) => self.vehicle_monitor_tick(id),
            VehicleTimer::ShipCheck(id) => self.ship_check_tick(id),
            VehicleTimer::ShipFuel(id) => self.ship_fuel_tick(id),
            VehicleTimer::ShipDecay(id) => {
                self.vehicle_delete_me(id);
            }
            VehicleTimer::ControllerDepart(index) => self.controller_depart_task(index),
            VehicleTimer::ControllerDecay(index) => self.controller_decay_task(index),
        }
    }
}
