//! The air ship docks (`AirShipController` and its four subclasses) and the scheduled ship
//! between Gludio and Gracia (`AirShipGludioGracia`), built in.
// Java: dist/game/data/scripts/ai/areas/Gracia/vehicles/AirShipController.java
// Java: dist/game/data/scripts/ai/areas/Gracia/vehicles/{SoIController,SoDController,
//       KeucereusNorthController,KeucereusSouthController}/*.java
// Java: dist/game/data/scripts/ai/areas/Gracia/vehicles/AirShipGludioGracia/*.java
//
// The scripts' `ScriptZone` hooks (`onEnterZone`, `onExitZone`) are called from
// [`World::vehicle_revalidate_zone`], which looks at the four dock zones for a steered ship (the
// only creature the controllers care about). Their dialogs are the `Script <Name> <event>`
// bypass (`process_script_event`) and the first click (`npc_first_talk_on_click`).

use l2_data::Zones;
use l2_protocol_game::server::{ActionFailed, ChatType, NpcSay, SystemMessage, SystemMessageParam};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use tracing::warn;

use super::route::PathPoint;
use super::{Engine, VehicleTimer};
use crate::clan::model::ClanAccess;
use crate::target::distance_3d;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `DEPART_INTERVAL`: the ship leaves 5 minutes after it docked empty.
const DEPART_INTERVAL_MS: u64 = 300_000;
/// `LICENSE`, `STARSTONE` and `SUMMON_COST`.
const LICENSE: i32 = 13559;
const STARSTONE: i32 = 13277;
const SUMMON_COST: i64 = 5;
/// `ThreadPool.schedule(_decayTask, 1000)`.
const DECAY_DELAY_MS: u64 = 1000;
/// The folder of the pages of the controllers.
const HTML_ROOT: &str = "data/scripts/ai/areas/Gracia/vehicles";

/// What one `AirShipController` subclass sets in its constructor.
#[derive(Debug, Clone, Copy)]
pub struct ControllerDef {
    /// The script class (the name in `bypass -h Script <name> <event>`).
    pub name: &'static str,
    /// `DOCK_ZONE`: the `ScriptZone` id, also the dock id of the ship.
    pub dock_zone: i32,
    /// `LOCATION`: the id of the destination list window.
    pub location_id: i32,
    /// `CONTROLLER_ID`.
    pub npc_id: i32,
    /// `_shipSpawnX/Y/Z` (the heading stays 0).
    pub ship_spawn: (i32, i32, i32),
    /// `_oustLoc`.
    pub oust: (i32, i32, i32),
    /// `ARRIVAL` and `DEPART`.
    pub arrival: &'static [PathPoint],
    pub depart: &'static [PathPoint],
    /// `TELEPORTS` and `FUEL`.
    pub teleports: &'static [&'static [PathPoint]],
    pub fuel: &'static [i32],
    /// `_movie.getClientId()`.
    pub movie: i32,
    /// The page the first talk shows, inside [`HTML_ROOT`].
    pub html: &'static str,
}

const fn p(x: i32, y: i32, z: i32, move_speed: i32, rotation_speed: i32) -> PathPoint {
    PathPoint::new(x, y, z, move_speed, rotation_speed)
}

// ---- SoIController
static SOI_ARRIVAL: [PathPoint; 2] = [
    p(-214422, 211396, 5000, 280, 2000),
    p(-214422, 211396, 4422, 280, 2000),
];
static SOI_DEPART: [PathPoint; 2] = [
    p(-214422, 211396, 5000, 280, 2000),
    p(-215877, 209709, 5000, 280, 2000),
];
static SOI_TELEPORT_0: [PathPoint; 3] = [
    p(-214422, 211396, 5000, 280, 2000),
    p(-215877, 209709, 5000, 280, 2000),
    p(-206692, 220997, 3000, 0, 0),
];
static SOI_TELEPORT_1: [PathPoint; 3] = [
    p(-214422, 211396, 5000, 280, 2000),
    p(-215877, 209709, 5000, 280, 2000),
    p(-195357, 233430, 2500, 0, 0),
];
static SOI_TELEPORTS: [&[PathPoint]; 2] = [&SOI_TELEPORT_0, &SOI_TELEPORT_1];

// ---- SoDController
static SOD_ARRIVAL: [PathPoint; 1] = [p(-246445, 252331, 4359, 280, 2000)];
static SOD_DEPART: [PathPoint; 1] = [p(-245245, 251040, 4359, 280, 2000)];
static SOD_TELEPORT_0: [PathPoint; 2] = [
    p(-245245, 251040, 4359, 280, 2000),
    p(-235693, 248843, 5100, 0, 0),
];
static SOD_TELEPORT_1: [PathPoint; 2] = [
    p(-245245, 251040, 4359, 280, 2000),
    p(-195357, 233430, 2500, 0, 0),
];
static SOD_TELEPORTS: [&[PathPoint]; 2] = [&SOD_TELEPORT_0, &SOD_TELEPORT_1];

// ---- KeucereusNorthController
static KEUCEREUS_NORTH_ARRIVAL: [PathPoint; 2] = [
    p(-183218, 239494, 2500, 280, 2000),
    p(-183218, 239494, 1336, 280, 2000),
];
static KEUCEREUS_NORTH_DEPART: [PathPoint; 2] = [
    p(-183218, 239494, 1700, 280, 2000),
    p(-181974, 235358, 1700, 280, 2000),
];
static KEUCEREUS_NORTH_TELEPORT_0: [PathPoint; 3] = [
    p(-183218, 239494, 1700, 280, 2000),
    p(-181974, 235358, 1700, 280, 2000),
    p(-186373, 234000, 2500, 0, 0),
];
static KEUCEREUS_NORTH_TELEPORT_1: [PathPoint; 3] = [
    p(-183218, 239494, 1700, 280, 2000),
    p(-181974, 235358, 1700, 280, 2000),
    p(-206692, 220997, 3000, 0, 0),
];
static KEUCEREUS_NORTH_TELEPORT_2: [PathPoint; 3] = [
    p(-183218, 239494, 1700, 280, 2000),
    p(-181974, 235358, 1700, 280, 2000),
    p(-235693, 248843, 5100, 0, 0),
];
static KEUCEREUS_NORTH_TELEPORTS: [&[PathPoint]; 3] = [
    &KEUCEREUS_NORTH_TELEPORT_0,
    &KEUCEREUS_NORTH_TELEPORT_1,
    &KEUCEREUS_NORTH_TELEPORT_2,
];

// ---- KeucereusSouthController
static KEUCEREUS_SOUTH_ARRIVAL: [PathPoint; 2] = [
    PathPoint::default_speed(-185312, 246544, 2500),
    PathPoint::default_speed(-185312, 246544, 1336),
];
static KEUCEREUS_SOUTH_DEPART: [PathPoint; 2] = [
    p(-185312, 246544, 1700, 280, 2000),
    p(-186900, 251699, 1700, 280, 2000),
];
static KEUCEREUS_SOUTH_TELEPORT_0: [PathPoint; 3] = [
    p(-185312, 246544, 1700, 280, 2000),
    p(-186900, 251699, 1700, 280, 2000),
    p(-186373, 234000, 2500, 0, 0),
];
static KEUCEREUS_SOUTH_TELEPORT_1: [PathPoint; 3] = [
    p(-185312, 246544, 1700, 280, 2000),
    p(-186900, 251699, 1700, 280, 2000),
    p(-206692, 220997, 3000, 0, 0),
];
static KEUCEREUS_SOUTH_TELEPORT_2: [PathPoint; 3] = [
    p(-185312, 246544, 1700, 280, 2000),
    p(-186900, 251699, 1700, 280, 2000),
    p(-235693, 248843, 5100, 0, 0),
];
static KEUCEREUS_SOUTH_TELEPORTS: [&[PathPoint]; 3] = [
    &KEUCEREUS_SOUTH_TELEPORT_0,
    &KEUCEREUS_SOUTH_TELEPORT_1,
    &KEUCEREUS_SOUTH_TELEPORT_2,
];

static FUEL_SOI: [i32; 2] = [0, 50];
static FUEL_SOD: [i32; 2] = [0, 100];
static FUEL_KEUCEREUS: [i32; 3] = [0, 50, 100];

/// The four docks, in the order of the list of `ai/areas/Gracia/vehicles`.
pub static CONTROLLERS: [ControllerDef; 4] = [
    ControllerDef {
        name: "SoIController",
        dock_zone: 50600,
        location_id: 101,
        npc_id: 32604,
        ship_spawn: (-212719, 213348, 5000),
        oust: (-213401, 210401, 4408),
        arrival: &SOI_ARRIVAL,
        depart: &SOI_DEPART,
        teleports: &SOI_TELEPORTS,
        fuel: &FUEL_SOI,
        // `Movie.LAND_UNDEAD_A`
        movie: 1002,
        html: "SoIController/32604.htm",
    },
    ControllerDef {
        name: "SoDController",
        dock_zone: 50601,
        location_id: 102,
        npc_id: 32605,
        ship_spawn: (-247702, 253631, 4359),
        oust: (-247746, 251079, 4328),
        arrival: &SOD_ARRIVAL,
        depart: &SOD_DEPART,
        teleports: &SOD_TELEPORTS,
        fuel: &FUEL_SOD,
        // `Movie.LAND_DISTRUCTION_A`
        movie: 1003,
        html: "SoDController/32605.htm",
    },
    ControllerDef {
        name: "KeucereusNorthController",
        dock_zone: 50602,
        location_id: 100,
        npc_id: 32606,
        ship_spawn: (-184145, 242373, 3000),
        oust: (-183900, 239384, 1320),
        arrival: &KEUCEREUS_NORTH_ARRIVAL,
        depart: &KEUCEREUS_NORTH_DEPART,
        teleports: &KEUCEREUS_NORTH_TELEPORTS,
        fuel: &FUEL_KEUCEREUS,
        // `Movie.LAND_KSERTH_B`
        movie: 1001,
        html: "KeucereusNorthController/32606.htm",
    },
    ControllerDef {
        name: "KeucereusSouthController",
        dock_zone: 50603,
        location_id: 100,
        npc_id: 32517,
        ship_spawn: (-184527, 243611, 3000),
        oust: (-186148, 246296, 1360),
        arrival: &KEUCEREUS_SOUTH_ARRIVAL,
        depart: &KEUCEREUS_SOUTH_DEPART,
        teleports: &KEUCEREUS_SOUTH_TELEPORTS,
        fuel: &FUEL_KEUCEREUS,
        // `Movie.LAND_KSERTH_A`
        movie: 1000,
        html: "KeucereusSouthController/32517.htm",
    },
];

// ---- AirShipGludioGracia
const GLUDIO_GRACIA_NAME: &str = "AirShipGludioGracia";
/// `CONTROLLERS`: Gracia's and Gludio's.
const GLUDIO_GRACIA_NPCS: [i32; 2] = [32607, 32609];
const GLUDIO_DOCK_ID: i32 = 10;
const GRACIA_DOCK_ID: i32 = 11;
const OUST_GLUDIO: (i32, i32, i32) = (-149379, 255246, -80);
const OUST_GRACIA: (i32, i32, i32) = (-186563, 243590, 2608);
const GLUDIO_GRACIA_SPAWN: (i32, i32, i32) = (-149378, 252552, 198);
const GLUDIO_GRACIA_HEADING: i32 = 33837;
/// `getFirstVisibleObjectInRange(_ship, Npc.class, 600, ...)`.
const FIND_CONTROLLER_RANGE: i32 = 600;
const GLUDIO_GRACIA_START_MS: u64 = 60_000;

pub static GLUDIO_TO_WARPGATE: [PathPoint; 3] = [
    PathPoint::default_speed(-151202, 252556, 231),
    PathPoint::default_speed(-160403, 256144, 222),
    // teleport: x,y,z,speed=0,heading
    p(-167874, 256731, -509, 0, 41035),
];
pub static WARPGATE_TO_GRACIA: [PathPoint; 8] = [
    PathPoint::default_speed(-169763, 254815, 282),
    PathPoint::default_speed(-171822, 250061, 425),
    PathPoint::default_speed(-172595, 247737, 398),
    PathPoint::default_speed(-174538, 246185, 39),
    PathPoint::default_speed(-179440, 243651, 1337),
    PathPoint::default_speed(-182601, 243957, 2739),
    PathPoint::default_speed(-184952, 245122, 2694),
    PathPoint::default_speed(-186936, 244563, 2617),
];
pub static GRACIA_TO_WARPGATE: [PathPoint; 16] = [
    PathPoint::default_speed(-187801, 244997, 2672),
    PathPoint::default_speed(-188520, 245932, 2465),
    PathPoint::default_speed(-189932, 245243, 1682),
    PathPoint::default_speed(-191192, 242969, 1523),
    PathPoint::default_speed(-190408, 239088, 1706),
    PathPoint::default_speed(-187475, 237113, 2768),
    PathPoint::default_speed(-184673, 238433, 2802),
    PathPoint::default_speed(-184524, 241119, 2816),
    PathPoint::default_speed(-182129, 243385, 2733),
    PathPoint::default_speed(-179440, 243651, 1337),
    PathPoint::default_speed(-174538, 246185, 39),
    PathPoint::default_speed(-172595, 247737, 398),
    PathPoint::default_speed(-171822, 250061, 425),
    PathPoint::default_speed(-169763, 254815, 282),
    PathPoint::default_speed(-168067, 256626, 343),
    // teleport: x,y,z,speed=0,heading
    p(-157261, 255664, 221, 0, 64781),
];
pub static WARPGATE_TO_GLUDIO: [PathPoint; 6] = [
    PathPoint::default_speed(-153414, 255385, 221),
    PathPoint::default_speed(-149548, 258172, 221),
    PathPoint::default_speed(-146884, 257097, 221),
    PathPoint::default_speed(-146672, 254239, 221),
    PathPoint::default_speed(-147855, 252712, 206),
    PathPoint::default_speed(-149378, 252552, 198),
];

/// The fields of `AirShipGludioGracia`: where the script is in its `switch`, and the controller
/// NPCs it found for its shouts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GludioGraciaState {
    pub cycle: usize,
    found_atc_gludio: bool,
    atc_gludio: Option<ObjectId>,
    found_atc_gracia: bool,
    atc_gracia: Option<ObjectId>,
}

/// One `AirShipController` at work: its dock, its flags and the ship that sits in it.
#[derive(Debug, Clone)]
pub struct Controller {
    pub(crate) def: &'static ControllerDef,
    /// Java `_isBusy`.
    pub(crate) busy: bool,
    /// Java `_dockedShip`.
    pub(crate) docked: Option<ObjectId>,
    /// `_arrivalPath` and `_departPath` after `validityCheck()`.
    pub(crate) arrival: Option<&'static [PathPoint]>,
    pub(crate) depart: Option<&'static [PathPoint]>,
    /// `_departSchedule`.
    pub(crate) depart_timer: Option<TimerId>,
}

impl Controller {
    /// Java `_dockedShip`.
    pub fn docked(&self) -> Option<ObjectId> {
        self.docked
    }

    /// Java `_isBusy`.
    pub fn is_busy(&self) -> bool {
        self.busy
    }
}

/// `AirShipController.validityCheck()` against the zone table.
fn validate(def: &'static ControllerDef, zones: Option<&Zones>) -> (Controller, bool) {
    let mut c = Controller {
        def,
        busy: false,
        docked: None,
        arrival: Some(def.arrival),
        depart: Some(def.depart),
        depart_timer: None,
    };
    let Some(zone) = zones.and_then(|z| z.get_of_type(def.dock_zone, "ScriptZone")) else {
        warn!(
            "{}: Invalid zone {}, controller disabled",
            def.name, def.dock_zone
        );
        c.busy = true;
        return (c, false);
    };
    if let Some(path) = c.arrival {
        match path.last() {
            None => {
                warn!("{}: Zero arrival path length.", def.name);
                c.arrival = None;
            }
            Some(last) if !zone.is_inside_zone(last.x, last.y, last.z) => {
                warn!(
                    "{}: Arrival path finish point ({},{},{}) not in zone {}",
                    def.name, last.x, last.y, last.z, def.dock_zone
                );
                c.arrival = None;
            }
            Some(_) => {}
        }
    }
    let (sx, sy, sz) = def.ship_spawn;
    if c.arrival.is_none() && !zone.is_inside_zone(sx, sy, sz) {
        warn!(
            "{}: Arrival path is null and spawn point not in zone {}, controller disabled",
            def.name, def.dock_zone
        );
        c.busy = true;
        return (c, false);
    }
    if let Some(path) = c.depart {
        match path.last() {
            None => {
                warn!("{}: Zero depart path length.", def.name);
                c.depart = None;
            }
            Some(last) if zone.is_inside_zone(last.x, last.y, last.z) => {
                warn!(
                    "{}: Departure path finish point ({},{},{}) in zone {}",
                    def.name, last.x, last.y, last.z, def.dock_zone
                );
                c.depart = None;
            }
            Some(_) => {}
        }
    }
    (c, true)
}

impl World {
    /// `AirShipController` constructors (`validityCheck`) and the `AirShipGludioGracia` script:
    /// call after the zones, the NPCs and the licenses are in.
    pub fn start_air_ships(&mut self) {
        let zones = self.zoning.zones.clone();
        self.vehicle.controllers.clear();
        for def in &CONTROLLERS {
            let (controller, valid) = validate(def, zones.as_deref());
            // `validityCheck()` registers the destinations at its end, which a controller that
            // was disabled does not reach.
            if valid {
                self.register_air_ship_teleport_list(def.dock_zone, def.location_id, def.teleports, def.fuel);
            }
            self.vehicle.controllers.push(controller);
        }
        self.start_gludio_gracia();
    }

    /// The constructor of `AirShipGludioGracia`.
    fn start_gludio_gracia(&mut self) {
        let Some(ship) = self.new_air_ship(GLUDIO_GRACIA_SPAWN, GLUDIO_GRACIA_HEADING) else {
            warn!("{GLUDIO_GRACIA_NAME}: the air ship could not be created.");
            return;
        };
        if let Some(v) = self.veh_mut(ship) {
            v.oust_loc = Some(OUST_GLUDIO);
            v.dock_id = GLUDIO_DOCK_ID;
            v.engine = Some(Engine::GludioGracia(GludioGraciaState::default()));
        }
        self.vehicle.gludio_gracia = Some(ship);
        self.vehicle_run_engine(ship, GLUDIO_GRACIA_START_MS);
    }

    /// Java `Creature.revalidateZone(force)` of a steered air ship, for the dock zones.
    pub(crate) fn vehicle_revalidate_zone(&mut self, id: ObjectId, force: bool) {
        let Some(v) = self.veh(id) else {
            return;
        };
        // The controllers only act for a `ControllableAirShip`.
        if v.ship.is_none() || v.teleporting || self.vehicle.controllers.is_empty() {
            return;
        }
        let pos = (v.x, v.y, v.z);
        if !force && distance_3d(pos, v.last_zone_validate) < 100.0 {
            return;
        }
        let Some(zones) = self.zoning.zones.clone() else {
            return;
        };
        if let Some(v) = self.veh_mut(id) {
            v.last_zone_validate = pos;
        }
        for index in 0..self.vehicle.controllers.len() {
            let Some(zone) = zones.get_of_type(self.vehicle.controllers[index].def.dock_zone, "ScriptZone")
            else {
                continue;
            };
            let inside = zone.is_inside_zone(pos.0, pos.1, pos.2);
            let member = self.veh(id).is_some_and(|v| v.docks_inside.contains(&index));
            if inside && !member {
                if let Some(v) = self.veh_mut(id) {
                    v.docks_inside.push(index);
                }
                self.controller_on_enter_zone(index, id);
            } else if !inside && member {
                if let Some(v) = self.veh_mut(id) {
                    v.docks_inside.retain(|i| *i != index);
                }
                self.controller_on_exit_zone(index, id);
            }
        }
    }

    /// `removeFromZones` for a ship that is deleted.
    pub(crate) fn vehicle_leave_all_zones(&mut self, id: ObjectId) {
        let inside = self.veh(id).map(|v| v.docks_inside.clone()).unwrap_or_default();
        if let Some(v) = self.veh_mut(id) {
            v.docks_inside.clear();
        }
        for index in inside {
            self.controller_on_exit_zone(index, id);
        }
    }

    /// `AirShipController.onEnterZone(creature, zone)`.
    fn controller_on_enter_zone(&mut self, index: usize, ship: ObjectId) {
        let Some(c) = self.vehicle.controllers.get(index) else {
            return;
        };
        if c.docked.is_some() {
            return;
        }
        let (dock, oust, movie) = (c.def.dock_zone, c.def.oust, c.def.movie);
        self.vehicle.controllers[index].docked = Some(ship);
        let passengers = {
            let Some(v) = self.veh_mut(ship) else {
                return;
            };
            v.dock_id = dock;
            v.oust_loc = Some(oust);
            v.passengers.clone()
        };
        // "Ship is not empty - display movie to passengers and dock"
        if !passengers.is_empty() {
            self.play_movie(&passengers, movie);
            self.timers.schedule_at(
                self.tick + TimerWheel::ms_to_ticks(DECAY_DELAY_MS),
                TimerKind::Vehicle(VehicleTimer::ControllerDecay(index)),
            );
        } else {
            let timer = self.timers.schedule_at(
                self.tick + TimerWheel::ms_to_ticks(DEPART_INTERVAL_MS),
                TimerKind::Vehicle(VehicleTimer::ControllerDepart(index)),
            );
            self.vehicle.controllers[index].depart_timer = Some(timer);
        }
    }

    /// `AirShipController.onExitZone(creature, zone)`.
    fn controller_on_exit_zone(&mut self, index: usize, ship: ObjectId) {
        let Some(c) = self.vehicle.controllers.get_mut(index) else {
            return;
        };
        if c.docked != Some(ship) {
            return;
        }
        if let Some(timer) = c.depart_timer.take() {
            self.timers.cancel(timer);
        }
        if let Some(c) = self.vehicle.controllers.get_mut(index) {
            c.docked = None;
            c.busy = false;
        }
        if let Some(v) = self.veh_mut(ship) {
            v.dock_id = 0;
        }
    }

    /// `AirShipController.DepartTask`.
    pub(crate) fn controller_depart_task(&mut self, index: usize) {
        let Some(c) = self.vehicle.controllers.get_mut(index) else {
            return;
        };
        c.depart_timer = None;
        let (docked, depart) = (c.docked, c.depart);
        let Some(ship) = docked else {
            return;
        };
        if !self.veh(ship).is_some_and(|v| v.is_in_dock() && !v.is_moving()) {
            return;
        }
        match depart {
            Some(path) => self.vehicle_execute_path(ship, Some(path)),
            None => {
                self.vehicle_delete_me(ship);
            }
        }
    }

    /// `AirShipController.DecayTask`.
    pub(crate) fn controller_decay_task(&mut self, index: usize) {
        if let Some(ship) = self.vehicle.controllers.get(index).and_then(|c| c.docked) {
            self.vehicle_delete_me(ship);
        }
    }

    /// `Quest.playMovie(players, movie)` (the movie holder: `script/movie.rs`).
    fn play_movie(&mut self, players: &[ObjectId], movie_id: i32) {
        self.script_play_movie(players, movie_id);
    }

    // ------------------------------------------------------------------ the dialogs

    /// The controller NPC with this id, if it is one of the five.
    fn controller_def_of_npc(npc_id: i32) -> Option<&'static ControllerDef> {
        CONTROLLERS.iter().find(|c| c.npc_id == npc_id)
    }

    /// `onFirstTalk` of the controllers and of `AirShipGludioGracia`: the page of the NPC. `true`
    /// when the NPC is one of them.
    pub(crate) fn air_ship_first_talk(&mut self, npc: ObjectId, player: ObjectId, from_click: bool) -> bool {
        let Some(npc_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return false;
        };
        let page = match Self::controller_def_of_npc(npc_id) {
            Some(def) => format!("{HTML_ROOT}/{}", def.html),
            None if GLUDIO_GRACIA_NPCS.contains(&npc_id) => {
                format!("{HTML_ROOT}/{GLUDIO_GRACIA_NAME}/{npc_id}.htm")
            }
            None => return false,
        };
        // `addStartNpc`: `ON_NPC_QUEST_START` makes the NPC the quest NPC of the player.
        if from_click && let Some(p) = self.players.get_mut(&player) {
            p.talk.last_quest_npc_object = npc.0;
        }
        self.air_ship_show_page(player, npc, &page);
        true
    }

    /// `Quest.showResult` of a `.htm` result: the page with the NPC's object id, then
    /// `ActionFailed`.
    fn air_ship_show_page(&mut self, player: ObjectId, npc: ObjectId, path: &str) {
        match self.get_htm(player, path) {
            Some(content) if !content.is_empty() => {
                self.insert_object_id_and_show_chat_window(npc, player, &content);
                self.send(player, &ActionFailed);
            }
            _ => warn!("{player}: page {path} of the air ship controller not found."),
        }
    }

    /// `Player.processScriptEvent(scriptName, event)` for the controllers and the Gludio-Gracia
    /// ship. `false` when the script is another one.
    pub(crate) fn air_ship_script_event(&mut self, player: ObjectId, script_name: &str, event: &str) -> bool {
        let controller = CONTROLLERS.iter().position(|c| c.name == script_name);
        if controller.is_none() && script_name != GLUDIO_GRACIA_NAME {
            return false;
        }
        if event.is_empty() {
            return true;
        }
        let Some(npc) = self.script_event_npc(player) else {
            return true;
        };
        match controller {
            Some(index) => {
                let result = self.controller_on_event(index, event, npc, player);
                // `Quest.showResult`: the event itself comes back for an event the script does
                // not know.
                if let Some(result) = result {
                    self.air_ship_show_result(player, npc, &result, index);
                }
            }
            None => self.gludio_gracia_on_event(player),
        }
        true
    }

    fn air_ship_show_result(&mut self, player: ObjectId, npc: ObjectId, result: &str, index: usize) {
        if result.ends_with(".htm") || result.ends_with(".html") {
            // The page is looked up in the folder of the script.
            let dir = CONTROLLERS[index]
                .html
                .rsplit_once('/')
                .map_or("", |(dir, _)| dir);
            self.air_ship_show_page(player, npc, &format!("{HTML_ROOT}/{dir}/{result}"));
        } else if result.starts_with("<html") {
            self.insert_object_id_and_show_chat_window(npc, player, result);
            self.send(player, &ActionFailed);
        } else {
            self.send_message(player, result);
        }
    }

    /// The conditions both scripts check before a player boards: the message of the first one
    /// that fails.
    fn air_ship_board_refusal(&self, player: ObjectId, controller: bool) -> Option<SystemMessageId> {
        use SystemMessageId as M;
        if self.is_transformed(player) {
            Some(M::YouCannotBoardAnAirshipWhileTransformed)
        } else if self.is_paralyzed(player) {
            Some(M::YouCannotBoardAnAirshipWhilePetrified)
        } else if self.is_dead(player) || self.is_fake_death(player) {
            Some(M::YouCannotBoardAnAirshipWhileDead)
        } else if self.is_fishing(player) {
            Some(M::YouCannotBoardAnAirshipWhileFishing)
        } else if self.is_in_combat(player) {
            Some(M::YouCannotBoardAnAirshipWhileInBattle)
        } else if self.is_in_duel(player) {
            Some(M::YouCannotBoardAnAirshipWhileInADuel)
        } else if self.is_sitting(player) {
            Some(M::YouCannotBoardAnAirshipWhileSitting)
        } else if self.is_casting_now(player) {
            Some(M::YouCannotBoardAnAirshipWhileCasting)
        } else if self.is_cursed_weapon_equipped(player) {
            Some(M::YouCannotBoardAnAirshipWhenACursedWeaponIsEquipped)
        } else if self.is_combat_flag_equipped(player) {
            Some(M::YouCannotBoardAnAirshipWhileHoldingAFlag)
        } else if self.summon_of(player).is_some() || self.is_mounted(player) {
            Some(M::YouCannotBoardAnAirshipWhileAPetOrAServitorIsSummoned)
        } else if controller && self.stats(player).is_some_and(|s| s.is_flying_mounted()) {
            Some(M::YouCannotBoardBecauseYouDoNotMeetTheRequirements)
        } else {
            None
        }
    }

    /// `AirShipController.onEvent`. The result is what `Quest.showResult` gets.
    fn controller_on_event(
        &mut self,
        index: usize,
        event: &str,
        npc: ObjectId,
        player: ObjectId,
    ) -> Option<String> {
        use SystemMessageId as M;
        if event.eq_ignore_ascii_case("summon") {
            self.controller_summon(index, npc, player);
            None
        } else if event.eq_ignore_ascii_case("board") {
            if let Some(message) = self.air_ship_board_refusal(player, true) {
                self.send(player, &SystemMessage::new(message));
                return None;
            }
            if let Some(ship) = self.vehicle.controllers[index].docked {
                self.vehicle_add_passenger(ship, player);
            }
            None
        } else if event.eq_ignore_ascii_case("register") {
            let clan_level = self.clan_of(player).map(|c| c.level);
            if clan_level.is_none_or(|level| level < 5) {
                self.send(
                    player,
                    &SystemMessage::new(M::InOrderToAcquireAnAirshipTheClanSLevelMustBeLevel5OrHigher),
                );
                return None;
            }
            if !self.is_clan_leader(player) {
                self.send(
                    player,
                    &SystemMessage::new(M::AirshipSummonLicenseRegistrationCanOnlyBeDoneByTheClanLeader),
                );
                return None;
            }
            let owner_id = self.clan_id_of(player);
            if self.has_air_ship_license(owner_id) {
                self.send(
                    player,
                    &SystemMessage::new(M::TheAirshipSummonLicenseHasAlreadyBeenAcquired),
                );
                return None;
            }
            if !self.destroy_item_by_item_id(player, LICENSE, 1, true) {
                // NOTE(java-quirk): the license shortage is told with the Starstone message.
                self.send(player, &Self::not_enough_starstones());
                return None;
            }
            self.register_air_ship_license(owner_id);
            self.send(
                player,
                &SystemMessage::new(M::TheAirshipSummonLicenseHasBeenEnteredYourClanCanNowSummonTheAirship),
            );
            None
        } else {
            Some(event.to_owned())
        }
    }

    /// `SM_NEED_MORE`.
    fn not_enough_starstones() -> SystemMessage {
        SystemMessage::new(SystemMessageId::AnAirshipCannotBeSummonedBecauseYouDonTHaveEnoughS1)
            .with(SystemMessageParam::ItemName(STARSTONE))
    }

    /// The `summon` event.
    fn controller_summon(&mut self, index: usize, npc: ObjectId, player: ObjectId) {
        use SystemMessageId as M;
        if let Some(ship) = self.vehicle.controllers[index].docked {
            if self.ship_is_owner(ship, player) {
                self.send(player, &SystemMessage::new(M::TheClanOwnedAirshipAlreadyExists));
            }
            return;
        }
        if self.vehicle.controllers[index].busy {
            self.send(
                player,
                &SystemMessage::new(M::AnotherAirshipHasAlreadyBeenSummonedPleaseTryAgainLater),
            );
            return;
        }
        if !self.has_clan_access(player, ClanAccess::AccessAirship) {
            self.send(
                player,
                &SystemMessage::new(M::AirshipSummonLicenseRegistrationCanOnlyBeDoneByTheClanLeader),
            );
            return;
        }
        let owner_id = self.clan_id_of(player);
        if !self.has_air_ship_license(owner_id) {
            self.send(
                player,
                &SystemMessage::new(
                    M::AnAirshipCannotBeSummonedBecauseEitherYouHaveNotRegisteredYourAirshipLicenseOrTheAirshipHasNotYetBeenSummoned,
                ),
            );
            return;
        }
        if self.has_air_ship(owner_id) {
            self.send(
                player,
                &SystemMessage::new(M::YourClanSAirshipIsAlreadyBeingUsedByAnotherClanMember),
            );
            return;
        }
        if !self.destroy_item_by_item_id(player, STARSTONE, SUMMON_COST, true) {
            self.send(player, &Self::not_enough_starstones());
            return;
        }
        self.vehicle.controllers[index].busy = true;
        let def = self.vehicle.controllers[index].def;
        let arrival = self.vehicle.controllers[index].arrival;
        match self.new_controllable_air_ship(def.ship_spawn, 0, owner_id) {
            Some(ship) => {
                if let Some(path) = arrival {
                    self.vehicle_execute_path(ship, Some(path));
                }
                let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
                self.broadcast_packet(
                    npc,
                    &NpcSay {
                        object_id: npc.0,
                        text_type: ChatType::NpcShout.client_id(),
                        npc_id,
                        npc_string: NpcStringId::TheAirshipHasBeenSummonedItWillAutomaticallyDepartIn5Minutes
                            .id(),
                        text: String::new(),
                        parameters: Vec::new(),
                    },
                );
            }
            None => self.vehicle.controllers[index].busy = false,
        }
    }

    /// `AirShipGludioGracia.onEvent`: whatever the event, the player boards.
    fn gludio_gracia_on_event(&mut self, player: ObjectId) {
        if let Some(message) = self.air_ship_board_refusal(player, false) {
            self.send(player, &SystemMessage::new(message));
            return;
        }
        let Some(ship) = self.vehicle.gludio_gracia else {
            return;
        };
        // `_ship.isInDock() && World.getFirstVisibleObject(player, AirShip.class, it -> it == _ship)`
        let in_dock = self.veh(ship).is_some_and(|v| v.is_in_dock());
        if in_dock && self.visible_objects(player).contains(&ship) {
            self.vehicle_add_passenger(ship, player);
        }
    }

    /// `AirShipGludioGracia.findController()`: the first controller NPC within 600 of the ship.
    fn gludio_gracia_find_controller(&self, ship: ObjectId) -> Option<ObjectId> {
        self.visible_objects_in_range(ship, FIND_CONTROLLER_RANGE)
            .into_iter()
            .find(|o| {
                self.npcs
                    .get(o)
                    .is_some_and(|n| GLUDIO_GRACIA_NPCS.contains(&n.npc_id))
            })
    }

    /// `broadcastInGludio` and `broadcastInGracia`.
    fn gludio_gracia_shout(&mut self, ship: ObjectId, gludio: bool, string: NpcStringId) {
        let Some(Engine::GludioGracia(state)) = self.veh(ship).and_then(|v| v.engine.clone()) else {
            return;
        };
        let (found, atc) = if gludio {
            (state.found_atc_gludio, state.atc_gludio)
        } else {
            (state.found_atc_gracia, state.atc_gracia)
        };
        let atc = if found {
            atc
        } else {
            let atc = self.gludio_gracia_find_controller(ship);
            if let Some(Engine::GludioGracia(state)) = self.veh_mut(ship).and_then(|v| v.engine.as_mut()) {
                if gludio {
                    state.found_atc_gludio = true;
                    state.atc_gludio = atc;
                } else {
                    state.found_atc_gracia = true;
                    state.atc_gracia = atc;
                }
            }
            atc
        };
        if let Some(atc) = atc {
            let npc_id = self.npcs.get(&atc).map_or(0, |n| n.npc_id);
            self.broadcast_packet(
                atc,
                &NpcSay {
                    object_id: atc.0,
                    text_type: ChatType::NpcShout.client_id(),
                    npc_id,
                    npc_string: string.id(),
                    text: String::new(),
                    parameters: Vec::new(),
                },
            );
        }
    }

    /// `AirShipGludioGracia.run()`.
    pub(crate) fn gludio_gracia_run(&mut self, ship: ObjectId) {
        let Some(Engine::GludioGracia(state)) = self.veh(ship).and_then(|v| v.engine.clone()) else {
            return;
        };
        let cycle = state.cycle;
        match cycle {
            0 => {
                self.gludio_gracia_shout(
                    ship,
                    true,
                    NpcStringId::TheRegularlyScheduledAirshipThatFliesToTheGraciaContinentHasDeparted,
                );
                self.set_ship_dock(ship, 0);
                self.vehicle_execute_path(ship, Some(&GLUDIO_TO_WARPGATE));
            }
            1 => {
                // _ship.teleToLocation(-167874, 256731, -509, 41035, false);
                if let Some(v) = self.veh_mut(ship) {
                    v.oust_loc = Some(OUST_GRACIA);
                }
                self.vehicle_run_engine(ship, 5000);
            }
            2 => self.vehicle_execute_path(ship, Some(&WARPGATE_TO_GRACIA)),
            3 => {
                self.gludio_gracia_shout(
                    ship,
                    false,
                    NpcStringId::TheRegularlyScheduledAirshipHasArrivedItWillDepartForTheAdenContinentIn1Minute,
                );
                self.set_ship_dock(ship, GRACIA_DOCK_ID);
                self.vehicle_oust_players(ship);
                self.vehicle_run_engine(ship, 60_000);
            }
            4 => {
                self.gludio_gracia_shout(
                    ship,
                    false,
                    NpcStringId::TheRegularlyScheduledAirshipThatFliesToTheAdenContinentHasDeparted,
                );
                self.set_ship_dock(ship, 0);
                self.vehicle_execute_path(ship, Some(&GRACIA_TO_WARPGATE));
            }
            5 => {
                // _ship.teleToLocation(-157261, 255664, 221, 64781, false);
                if let Some(v) = self.veh_mut(ship) {
                    v.oust_loc = Some(OUST_GLUDIO);
                }
                self.vehicle_run_engine(ship, 5000);
            }
            6 => self.vehicle_execute_path(ship, Some(&WARPGATE_TO_GLUDIO)),
            7 => {
                self.gludio_gracia_shout(
                    ship,
                    true,
                    NpcStringId::TheRegularlyScheduledAirshipHasArrivedItWillDepartForTheGraciaContinentIn1Minute,
                );
                self.set_ship_dock(ship, GLUDIO_DOCK_ID);
                self.vehicle_oust_players(ship);
                self.vehicle_run_engine(ship, 60_000);
            }
            _ => {}
        }
        if let Some(Engine::GludioGracia(state)) = self.veh_mut(ship).and_then(|v| v.engine.as_mut()) {
            state.cycle = if cycle + 1 > 7 { 0 } else { cycle + 1 };
        }
    }

    fn set_ship_dock(&mut self, ship: ObjectId, dock: i32) {
        if let Some(v) = self.veh_mut(ship) {
            v.dock_id = dock;
        }
    }

    /// The controllers, for the tests and the admin tools.
    pub fn air_ship_controllers(&self) -> &[Controller] {
        &self.vehicle.controllers
    }
}
