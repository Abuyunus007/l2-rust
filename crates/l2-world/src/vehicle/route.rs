//! The data types of the boat routes and the air ship paths.
// Java: entity/actor/holders/creature/VehiclePathPoint.java
// Java: dist/game/data/scripts/vehicles/*.java (what one `case` of a boat script does)

use l2_protocol_game::SystemMessageId;

/// Java `VehiclePathPoint`: a point of a path with the speeds the vehicle takes on when it starts
/// to move there.
///
/// A point with a move speed of 0 is a teleport: the vehicle jumps there and the rotation speed
/// is its new heading (`Vehicle.moveToNextRoutePoint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathPoint {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub move_speed: i32,
    pub rotation_speed: i32,
}

impl PathPoint {
    /// `new VehiclePathPoint(x, y, z, moveSpeed, rotationSpeed)`.
    pub const fn new(x: i32, y: i32, z: i32, move_speed: i32, rotation_speed: i32) -> Self {
        Self {
            x,
            y,
            z,
            move_speed,
            rotation_speed,
        }
    }

    /// `new VehiclePathPoint(x, y, z)`: Java's defaults, 350 and 4000.
    pub const fn default_speed(x: i32, y: i32, z: i32) -> Self {
        Self::new(x, y, z, 350, 4000)
    }
}

/// A place: `BoatManager.broadcastPacket` only looks at x and y, the sounds carry z too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pt(pub i32, pub i32, pub i32);

/// One packet of `BoatManager.broadcastPackets(point1, point2, packets...)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cast {
    /// `new CreatureSay(ChatType.BOAT, 801, id)`.
    Msg(SystemMessageId),
    /// `new PlaySound(0, "itemsound.ship_arrival_departure", 1, boatId, x, y, z)`.
    Sound(Pt),
}

/// `BoatManager.broadcastPackets(near.0, near.1, items...)`: the packets go to every player within
/// `BoatBroadcastRadius` of one of the two places.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Broadcast {
    pub near: (Pt, Pt),
    pub items: &'static [Cast],
}

/// `Vehicle.payForRide(itemId, count, oustX, oustY, oustZ)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pay {
    /// 0: no ticket is taken (the Innadril tour is free).
    pub item_id: i32,
    pub count: i64,
    pub oust: (i32, i32, i32),
}

/// The `if (dockBusy(dock)) { ... BUSY ...; schedule(this, 5000); return; }` head of a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitDock {
    pub dock: i32,
    pub near: (Pt, Pt),
    pub busy: SystemMessageId,
}

/// One `case` of a boat script's `run()`, in the order Java runs its statements.
#[derive(Debug, Clone, Copy)]
pub struct BoatStep {
    /// The busy dock check that repeats the case every 5 s.
    pub wait_dock: Option<WaitDock>,
    /// `BoatManager.dockShip(dock, busy)`.
    pub dock_ops: &'static [(i32, bool)],
    /// `BoatManager.broadcastPacket(s)`.
    pub broadcast: Option<Broadcast>,
    /// `_boat.broadcastPacket(sound)`: the sound goes to the players who see the boat.
    pub boat_sound: Option<Pt>,
    /// `_boat.payForRide(...)`.
    pub pay: Option<Pay>,
    /// `_boat.executePath(path)`.
    pub exec: Option<&'static [PathPoint]>,
    /// `ThreadPool.schedule(this, ms)`; none when the arrival of the boat runs the next case.
    pub next_ms: Option<u64>,
}

/// One boat script: the boat `main` creates and the cases of `run()`.
#[derive(Debug, Clone, Copy)]
pub struct BoatRoute {
    /// The script class.
    pub name: &'static str,
    /// `getNewBoat(boatId, ...)`: the template id of the boat.
    pub boat_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub steps: &'static [BoatStep],
    /// `boat.runEngine(delay)` of `main`.
    pub start_delay_ms: u64,
    /// The `dockShip` calls of `main`.
    pub start_docks: &'static [(i32, bool)],
}
