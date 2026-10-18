//! Every object in the world (players, NPCs, doors) by id, and the per-object state the region
//! grid needs.
// Java: org/l2jmobius/gameserver/entity/WorldObject.java (_isSpawned, _worldRegion, _isInvisible,
//       _instanceId), World._allObjects
//
// The kinds keep their own tables in `World` (`players`, `npcs`, `doors`); this table only says
// which kind an id is and where it is in the grid (D-11: ids, never references).

use l2_util::ObjectId;

use crate::player::CreatureCore;
use crate::region::RegionId;

/// What an object is. Decides which table holds it and which packets describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Player,
    Npc,
    Door,
}

impl ObjectKind {
    /// Java `isPlayable()`: players (and later summons/pets) wake regions up.
    pub fn is_playable(self) -> bool {
        matches!(self, Self::Player)
    }

    /// Java `isCreature()`: forgets targets when an object leaves view.
    pub fn is_creature(self) -> bool {
        matches!(self, Self::Player | Self::Npc)
    }
}

/// The grid-related state of one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectEntry {
    pub kind: ObjectKind,
    /// Java `_worldRegion`.
    pub region: Option<RegionId>,
    /// Java `_isSpawned`.
    pub spawned: bool,
    /// Java `_isInvisible` (GM invisibility).
    pub invisible: bool,
    /// Java `getInstanceId()`; 0 = the normal world.
    pub instance_id: i32,
}

impl ObjectEntry {
    pub fn new(kind: ObjectKind) -> Self {
        Self {
            kind,
            region: None,
            spawned: false,
            invisible: false,
            instance_id: 0,
        }
    }
}

/// An NPC in the world. M3-NPC-01 fills in what spawning and `NpcInfo` need.
#[derive(Debug, Clone, PartialEq)]
pub struct Npc {
    pub core: CreatureCore,
    /// The template id (`npc_id`, Java `getId()`).
    pub npc_id: i32,
}

/// A door in the world. M3-DOOR-01 fills in templates, state and packets.
#[derive(Debug, Clone, PartialEq)]
pub struct Door {
    pub object_id: ObjectId,
    /// The door id from `Doors.xml`.
    pub door_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub open: bool,
}
