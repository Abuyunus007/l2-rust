//! Every object in the world (players, NPCs, doors) by id, and the per-object state the region
//! grid needs.
// Java: org/l2jmobius/gameserver/entity/WorldObject.java (_isSpawned, _worldRegion, _isInvisible,
//       _instanceId), World._allObjects
//
// The kinds keep their own tables in `World` (`players`, `npcs`, `doors`); this table only says
// which kind an id is and where it is in the grid (D-11: ids, never references).

pub use crate::door::{Door, Fence, StaticObject};
use l2_util::ObjectId;

use crate::npc::NpcClass;
use crate::player::CreatureCore;
use crate::region::RegionId;

/// What an object is. Decides which table holds it and which packets describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Player,
    Npc,
    Door,
    /// Java `Fence` (a `WorldObject`, not a creature).
    Fence,
    /// Java `StaticObject` (town maps, thrones, signboards).
    StaticObject,
    /// Java `Item` lying on the ground (`ground_item.rs`). Items in an inventory are not world
    /// objects here: they are rows of the owner's item list.
    Item,
}

impl ObjectKind {
    /// Java `isPlayable()`: players (and later summons/pets) wake regions up.
    pub fn is_playable(self) -> bool {
        matches!(self, Self::Player)
    }

    /// Java `isCreature()`: forgets targets when an object leaves view. Doors and static objects
    /// extend `Creature` in Java; fences do not.
    pub fn is_creature(self) -> bool {
        matches!(self, Self::Player | Self::Npc | Self::Door | Self::StaticObject)
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

/// An NPC in the world (Java `Npc` and its subclasses, D-17: one struct plus [`NpcClass`]).
/// Spawning and `NpcInfo` are in `npc.rs`.
#[derive(Debug, Clone, PartialEq)]
pub struct Npc {
    pub core: CreatureCore,
    /// The template id (`npc_id`, Java `getId()`).
    pub npc_id: i32,
    /// Java: which `entity/actor/instance/*` class the template type names.
    pub class: NpcClass,
    /// Java `_spawn`: index into the world's spawn list; `None` for minions and NPCs made by
    /// admin commands.
    pub spawn: Option<usize>,
    /// Java `Monster._master`: the monster whose minion this is.
    pub leader: Option<ObjectId>,
    /// Java `_isDecayed`.
    pub decayed: bool,
    /// Java `isInsideZone(ZoneId.WATER)`, decided when it spawned (NPCs do not move before M4).
    pub in_water: bool,
    /// Java `_currentRHandId`, `_currentLHandId`, `_currentEnchant`.
    pub right_hand: i32,
    pub left_hand: i32,
    pub enchant_effect: i32,
    /// Java `_displayEffect`.
    pub display_effect: i32,
    /// Java `Npc._currentCollisionRadius`: the template's, or the grown one while `Grow` lasts.
    pub collision_radius: f64,
    /// Java `Attackable._mustGiveExpSp` (`mustRewardExpSp()`): `OpenChest` turns it off.
    pub must_reward_exp_sp: bool,
    /// Java `Chest._specialDrop`: the chest was unlocked (`OpenChest`).
    pub special_drop: bool,
    /// The Creature fields an NPC's movement uses: `_move`, `_isRunning`, `_isTeleporting`, the
    /// pending path search and `AbstractAI`'s pawn move fields (M4-AI-01, see `movement.rs`).
    pub movement: crate::movement::NpcMovement,
    /// Java `getSpawn().getLocation()` as of the time this NPC spawned: where `returnHome` walks.
    /// (0, 0, 0) for an NPC without a spawn.
    pub spawn_point: (i32, i32, i32),
    /// Java `Attackable._spawnTime`, in world milliseconds ([`crate::World::world_ms`]).
    pub spawn_time_ms: i64,
}
