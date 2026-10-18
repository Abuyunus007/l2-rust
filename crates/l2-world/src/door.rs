//! Doors, fences and static objects as world objects: spawned from the datapack, shown to players
//! with Java's packets, open / close state owned by the world, and the `DynamicObstacles` the
//! geoengine asks (in the world thread over the world's tables, for path workers as an immutable
//! [`DoorSnapshot`]).
// Java: org/l2jmobius/gameserver/entity/actor/instance/Door.java (state, sendInfo,
//       broadcastStatusUpdate, openMe, closeMe, manageGroupOpen, notifyChildEvent)
// Java: org/l2jmobius/gameserver/entity/actor/instance/Fence.java, StaticObject.java
// Java: org/l2jmobius/gameserver/data/xml/DoorData.java (makeDoor, checkIfDoorsBetween),
//       FenceData.java (spawnFence, checkIfFenceBetween), StaticObjectData.java (addObject)
// Java: org/l2jmobius/gameserver/entity/WorldRegion.java (_doors, _fences)
//
// Object ids: Java takes a fresh id from `IdManager` for every door, fence (plus one per extra
// height piece) and static object, so they come from the Rust `IdFactory` here. The ids the
// client knows them by (`Door.getId()`, `StaticObject.getId()`) are the template ids.
//
// Not here (out of scope of M3-DOOR-01):
// - Door timers: `startTimerOpen` / `TimerOpen` (open by time) and `startAutoCloseTask` /
//   `AutoClose` (`close_time`). `open_door` and `close_door` do not arm them.
// - Castle, fort and clan hall doors: `isEnemy()` is always false, `useTargetable` is false, and
//   the clan hall door lists are not filled. Sieges come with M4+.
// - Damage: `reduceCurrentHp`, `doDie` and the "gate destroyed" message. [`Door::dead`] and
//   [`World::set_door_dead`] only carry the state the geodata checks read.
// - Instances (`Instance.getDoors()`): no instance exists yet, so a geodata check with an
//   instance id of 1 or more finds no door, like Java's `doors == null` for an unknown instance.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use l2_data::{DoorTemplate, DoorTemplates, FenceState, FenceTemplates, StaticObjectTemplates};
use l2_geo::{DynamicObstacles, Location};
use l2_protocol_game::server::{
    DeleteObject, DoorStatusUpdate, ExColosseumFenceInfo, OnEventTrigger, StaticObjectInfo,
};
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::ids::{IdError, IdFactory};
use crate::objects::{ObjectEntry, ObjectKind};
use crate::region::{RegionGrid, RegionId, region_of};
use crate::world::World;

/// `FenceData.MAX_Z_DIFF`: a fence only blocks at heights within this distance of its own z.
const FENCE_MAX_Z_DIFF: i32 = 100;

/// A door in the world (Java `Door`). The shared, immutable part is the [`DoorTemplate`].
#[derive(Debug, Clone, PartialEq)]
pub struct Door {
    pub object_id: ObjectId,
    /// `Door.getId()`: the door id from `Doors.xml`, the id the client knows the door by.
    pub door_id: i32,
    pub template: Arc<DoorTemplate>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Java `_open`, from the template's `default_status` at the start.
    pub open: bool,
    /// Java `getCurrentHp()`.
    pub current_hp: f64,
    /// Java `getMaxHp()`: `(int) baseHpMax` (doors have no hp modifiers).
    pub max_hp: i32,
    /// Java `_isTargetable`.
    pub targetable: bool,
    /// Java `_isAttackableDoor`.
    pub attackable_door: bool,
    /// Java `_meshindex`.
    pub mesh_index: i32,
    /// Java `isDead()`: a dead door does not block.
    pub dead: bool,
}

impl Door {
    // Java: Door constructor + DoorData.makeDoor (`setCurrentHp(getMaxHp())`)
    pub fn new(object_id: ObjectId, template: Arc<DoorTemplate>) -> Self {
        let max_hp = f64::from(template.base_hp_max) as i32;
        Self {
            object_id,
            door_id: template.id,
            x: template.pos_x,
            y: template.pos_y,
            z: template.pos_z,
            open: template.open_by_default,
            current_hp: f64::from(max_hp),
            max_hp,
            targetable: template.targetable,
            attackable_door: template.attackable,
            mesh_index: 1,
            dead: false,
            template,
        }
    }

    /// Java `Door.getDamage()`: 0 (unhurt) to 6 (almost destroyed), the damage grade of the
    /// client's hp bar.
    pub fn damage(&self) -> i32 {
        // Java int arithmetic: `6 - (int) Math.ceil(hp / maxHp * 6)`, then clamped to 0..=6.
        let dmg = 6_i32.wrapping_sub(((self.current_hp / f64::from(self.max_hp)) * 6.0).ceil() as i32);
        dmg.clamp(0, 6)
    }

    /// Java `Door.getZMin()`.
    pub fn z_min(&self) -> i32 {
        self.template.node_z
    }

    /// Java `Door.getZMax()`.
    pub fn z_max(&self) -> i32 {
        self.template.node_z.wrapping_add(self.template.height)
    }

    /// Java `Door.isEnemy()`: true for a siege door of an active castle, fort or siegable clan
    /// hall with a visible hp bar. None of those exist yet.
    pub fn is_enemy(&self) -> bool {
        false
    }

    /// The filter of `DoorData.checkIfDoorsBetween`: closed, alive, collision on and a first
    /// node that is not at x = 0.
    // NOTE(java-quirk): `getX(0) == 0` skips a door whose first node has x = 0; Java uses it as
    // "no geometry". No shipped door has such a node.
    fn blocks(&self) -> bool {
        !(self.dead || self.open || !self.template.check_collision || self.template.node_x[0] == 0)
    }

    fn shape(&self) -> DoorShape {
        DoorShape {
            x: self.template.node_x,
            y: self.template.node_y,
            z_min: self.z_min(),
            z_max: self.z_max(),
        }
    }

    /// The packets `Door.sendInfo` sends: `StaticObjectInfo`, `DoorStatusUpdate` and, for a door
    /// with an emitter, `OnEventTrigger`. `targetable` is `StaticObjectInfo(door, targetable)`'s
    /// second argument: GMs get doors they can always target.
    pub fn info_packets(
        &self,
        targetable: bool,
    ) -> (StaticObjectInfo, DoorStatusUpdate, Option<OnEventTrigger>) {
        let info = StaticObjectInfo {
            static_object_id: self.door_id,
            object_id: self.object_id.0,
            kind: StaticObjectInfo::KIND_DOOR,
            targetable: self.targetable || targetable,
            mesh_index: self.mesh_index,
            closed: !self.open,
            is_enemy: self.is_enemy(),
            current_hp: self.current_hp as i32,
            max_hp: self.max_hp,
            show_hp: self.template.show_hp,
            damage_grade: self.damage(),
        };
        let status = DoorStatusUpdate {
            object_id: self.object_id.0,
            closed: !self.open,
            damage: self.damage(),
            is_enemy: self.is_enemy(),
            door_id: self.door_id,
            current_hp: self.current_hp as i32,
            max_hp: self.max_hp,
        };
        let trigger = (self.template.emitter > 0).then_some(OnEventTrigger {
            emitter_id: self.template.emitter,
            enabled: !self.open,
        });
        (info, status, trigger)
    }
}

/// A fence in the world (Java `Fence`): a rectangle on the ground, shown to the client as a stack
/// of fence pieces, blocking movement and sight while its state is `CLOSED` or `CLOSED_HIDDEN`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fence {
    pub object_id: ObjectId,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub width: i32,
    pub length: i32,
    /// `_xMin` and friends, from the x / y the fence was made with (before spawning clamps them).
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
    pub state: FenceState,
    /// Java `_heightFences`: ids of the extra pieces of a fence with `height > 1`. They are not
    /// world objects, only ids the client is told about.
    pub height_fences: Vec<ObjectId>,
}

impl Fence {
    fn shape(&self) -> FenceShape {
        FenceShape {
            x_min: self.x_min,
            x_max: self.x_max,
            y_min: self.y_min,
            y_max: self.y_max,
            z: self.z,
        }
    }
}

/// A static object in the world (Java `StaticObject`): a town map, a throne or a signboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticObject {
    pub object_id: ObjectId,
    /// `StaticObject.getId()`: the id from `StaticObjects.xml`.
    pub static_object_id: i32,
    pub name: String,
    /// Java `_type`: 0 map signs, 1 thrones, 2 arena signs.
    pub kind: i32,
    /// Java `_meshIndex`: 0 default textures, 1 alternate.
    pub mesh_index: i32,
    /// Java `setMap(texture, x, y)`: the town map the object opens when clicked.
    pub texture: String,
    pub map_x: i32,
    pub map_y: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// The lookups `DoorData` keeps next to the doors.
#[derive(Debug, Default)]
pub(crate) struct DoorIndex {
    /// `DoorData._doors`: the door with this template id (the last one, for a repeated id).
    pub by_door_id: HashMap<i32, ObjectId>,
    /// `DoorData._groups`: group name to the template ids in it.
    pub groups: HashMap<String, BTreeSet<i32>>,
}

// ---------------------------------------------------------------------------------------------
// Geometry (shared by the world view and the snapshot)

/// The footprint of a door: `DoorData.checkIfDoorsBetween` reads these from the template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DoorShape {
    x: [i32; 4],
    y: [i32; 4],
    z_min: i32,
    z_max: i32,
}

/// The rectangle of a fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FenceShape {
    x_min: i32,
    x_max: i32,
    y_min: i32,
    y_max: i32,
    z: i32,
}

/// The segment / door-face intersection of `DoorData.checkIfDoorsBetween` for one (closed, alive)
/// door. All integer arithmetic wraps and the multipliers are `float`s, as in Java.
fn door_blocks(door: &DoorShape, from: Location, to: Location, double_face_check: bool) -> bool {
    let (x, y, z) = (from.x, from.y, from.z);
    let (tx, ty, tz) = (to.x, to.y, to.z);
    let mut intersect_face = false;
    for i in 0..4 {
        let j = if i + 1 < 4 { i + 1 } else { 0 };
        // Lower part of the multiplier fraction; 0 means the lines are parallel.
        let denominator = ty
            .wrapping_sub(y)
            .wrapping_mul(door.x[i].wrapping_sub(door.x[j]))
            .wrapping_sub(tx.wrapping_sub(x).wrapping_mul(door.y[i].wrapping_sub(door.y[j])));
        if denominator == 0 {
            continue;
        }
        // Multipliers of the two line equations: outside 0..=1 the segments do not intersect.
        let numerator1 = door.x[j]
            .wrapping_sub(door.x[i])
            .wrapping_mul(y.wrapping_sub(door.y[i]))
            .wrapping_sub(
                door.y[j]
                    .wrapping_sub(door.y[i])
                    .wrapping_mul(x.wrapping_sub(door.x[i])),
            );
        let multiplier1 = numerator1 as f32 / denominator as f32;
        let numerator2 = tx
            .wrapping_sub(x)
            .wrapping_mul(y.wrapping_sub(door.y[i]))
            .wrapping_sub(ty.wrapping_sub(y).wrapping_mul(x.wrapping_sub(door.x[i])));
        let multiplier2 = numerator2 as f32 / denominator as f32;
        if (0.0..=1.0).contains(&multiplier1) && (0.0..=1.0).contains(&multiplier2) {
            // Math.round(float): floor(v + 0.5), exact in f64.
            let z_at = z as f32 + multiplier1 * tz.wrapping_sub(z) as f32;
            let intersect_z = (f64::from(z_at) + 0.5).floor() as i32;
            // The point must be between the door's min and max z.
            if intersect_z > door.z_min && intersect_z < door.z_max {
                if !double_face_check || intersect_face {
                    return true;
                }
                intersect_face = true;
            }
        }
    }
    false
}

/// `FenceData.checkIfFenceBetween` for one fence whose state blocks.
fn fence_blocks(fence: &FenceShape, from: Location, to: Location) -> bool {
    let (x, y, z) = (from.x, from.y, from.z);
    let (tx, ty) = (to.x, to.y);
    if x < fence.x_min && tx < fence.x_min {
        return false;
    }
    if x > fence.x_max && tx > fence.x_max {
        return false;
    }
    if y < fence.y_min && ty < fence.y_min {
        return false;
    }
    if y > fence.y_max && ty > fence.y_max {
        return false;
    }
    if x > fence.x_min
        && tx > fence.x_min
        && x < fence.x_max
        && tx < fence.x_max
        && y > fence.y_min
        && ty > fence.y_min
        && y < fence.y_max
        && ty < fence.y_max
    {
        return false;
    }
    let (x_min, x_max, y_min, y_max) = (
        f64::from(fence.x_min),
        f64::from(fence.x_max),
        f64::from(fence.y_min),
        f64::from(fence.y_max),
    );
    let (sx, sy, stx, sty) = (f64::from(x), f64::from(y), f64::from(tx), f64::from(ty));
    let crosses = |x1: f64, y1: f64, x2: f64, y2: f64| {
        cross_line_part(x1, y1, x2, y2, sx, sy, stx, sty, x_min, y_min, x_max, y_max)
    };
    (crosses(x_min, y_min, x_max, y_min)
        || crosses(x_max, y_min, x_max, y_max)
        || crosses(x_max, y_max, x_min, y_max)
        || crosses(x_min, y_max, x_min, y_min))
        && z > fence.z.wrapping_sub(FENCE_MAX_Z_DIFF)
        && z < fence.z.wrapping_add(FENCE_MAX_Z_DIFF)
}

// Java: FenceData.crossLinePart
// NOTE(java-quirk): the crossing counts when its x is inside the fence's x range *or* its y is
// inside the y range, not both; together with the early-outs above this is what Java does.
#[allow(clippy::too_many_arguments)]
fn cross_line_part(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
    x4: f64,
    y4: f64,
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
) -> bool {
    let Some((x_cross, y_cross)) = intersection(x1, y1, x2, y2, x3, y3, x4, y4) else {
        return false;
    };
    (x_cross <= x_max && x_cross >= x_min) || (y_cross <= y_max && y_cross >= y_min)
}

// Java: FenceData.intersection (the intersection of two infinite lines)
#[allow(clippy::too_many_arguments)]
fn intersection(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
    x4: f64,
    y4: f64,
) -> Option<(f64, f64)> {
    let d = ((x1 - x2) * (y3 - y4)) - ((y1 - y2) * (x3 - x4));
    if d == 0.0 {
        return None;
    }
    let xi = (((x3 - x4) * ((x1 * y2) - (y1 * x2))) - ((x1 - x2) * ((x3 * y4) - (y3 * x4)))) / d;
    let yi = (((y3 - y4) * ((x1 * y2) - (y1 * x2))) - ((y1 - y2) * ((x3 * y4) - (y3 * x4)))) / d;
    Some((xi, yi))
}

// ---------------------------------------------------------------------------------------------
// DynamicObstacles over the world

/// The world's doors and fences as the geoengine's [`DynamicObstacles`], for checks made in the
/// world thread. It borrows the world's tables, so it sees the door state of this very moment;
/// build it with [`World::obstacles`] for one call, e.g.
/// `engine.can_move_to_target_with(&world.obstacles(), from, to, instance)`.
///
/// Like Java it looks only at the door and fence lists of the region of the segment's start.
pub struct WorldObstacles<'a> {
    doors: &'a HashMap<ObjectId, Door>,
    fences: &'a HashMap<ObjectId, Fence>,
    regions: &'a RegionGrid,
    objects: &'a HashMap<ObjectId, ObjectEntry>,
}

impl WorldObstacles<'_> {
    fn instance_of(&self, id: ObjectId) -> i32 {
        self.objects.get(&id).map_or(0, |e| e.instance_id)
    }
}

impl DynamicObstacles for WorldObstacles<'_> {
    // Java: DoorData.checkIfDoorsBetween
    fn doors_between(&self, from: Location, to: Location, instance_id: i32, double_face_check: bool) -> bool {
        // Java reads `Instance.getDoors()` here; there are no instances yet, so no doors.
        if instance_id >= 1 {
            return false;
        }
        let Some(region) = region_of(from.x, from.y, from.z) else {
            return false;
        };
        self.regions.doors(region).any(|id| {
            let Some(door) = self.doors.get(&id) else {
                return false;
            };
            instance_id == self.instance_of(id)
                && door.blocks()
                && door_blocks(&door.shape(), from, to, double_face_check)
        })
    }

    // Java: FenceData.checkIfFenceBetween
    fn fence_between(&self, from: Location, to: Location, instance_id: i32) -> bool {
        let Some(region) = region_of(from.x, from.y, from.z) else {
            return false;
        };
        self.regions.fences(region).any(|id| {
            let Some(fence) = self.fences.get(&id) else {
                return false;
            };
            fence.state.is_geodata_enabled()
                && self.instance_of(id) == instance_id
                && fence_blocks(&fence.shape(), from, to)
        })
    }
}

/// An immutable copy of the closed doors and the blocking fences, for the path worker threads
/// (D-14): the workers share the geoengine and get the doors as they were when the request was
/// made. Take it with [`World::door_snapshot`]; the world keeps one until a door or fence
/// changes, so asking for it on every move request costs an `Arc` clone.
///
/// Answers exactly what [`WorldObstacles`] answers at the moment of the snapshot.
#[derive(Debug, Default)]
pub struct DoorSnapshot {
    /// Closed, alive, colliding doors by the region they stand in. The region of a *query* is
    /// matched against the surrounding lists of these, like Java's per-region door lists.
    doors: HashMap<RegionId, Vec<SnapDoor>>,
    fences: HashMap<RegionId, Vec<SnapFence>>,
}

#[derive(Debug, Clone, Copy)]
struct SnapDoor {
    shape: DoorShape,
    instance_id: i32,
}

#[derive(Debug, Clone, Copy)]
struct SnapFence {
    shape: FenceShape,
    instance_id: i32,
}

impl DoorSnapshot {
    /// Number of doors in it (closed, alive, colliding).
    pub fn door_count(&self) -> usize {
        self.doors.values().map(Vec::len).sum()
    }

    /// Number of blocking fences in it.
    pub fn fence_count(&self) -> usize {
        self.fences.values().map(Vec::len).sum()
    }

    /// The entries whose home region has `query` in its surrounding list.
    fn near<T>(map: &HashMap<RegionId, Vec<T>>, query: RegionId) -> impl Iterator<Item = &T> {
        (-1..=1).flat_map(move |dx| {
            (-1..=1).flat_map(move |dy| {
                (-1..=1).flat_map(move |dz| {
                    let home = RegionId {
                        x: query.x + dx,
                        y: query.y + dy,
                        z: query.z + dz,
                    };
                    map.get(&home).filter(|_| home.lists(query)).into_iter().flatten()
                })
            })
        })
    }
}

impl DynamicObstacles for DoorSnapshot {
    fn doors_between(&self, from: Location, to: Location, instance_id: i32, double_face_check: bool) -> bool {
        if instance_id >= 1 {
            return false;
        }
        let Some(region) = region_of(from.x, from.y, from.z) else {
            return false;
        };
        Self::near(&self.doors, region)
            .any(|d| d.instance_id == instance_id && door_blocks(&d.shape, from, to, double_face_check))
    }

    fn fence_between(&self, from: Location, to: Location, instance_id: i32) -> bool {
        let Some(region) = region_of(from.x, from.y, from.z) else {
            return false;
        };
        Self::near(&self.fences, region)
            .any(|f| f.instance_id == instance_id && fence_blocks(&f.shape, from, to))
    }
}

// ---------------------------------------------------------------------------------------------
// The world side

impl World {
    /// Java `DoorData.makeDoor` for every door of `Doors.xml`, in file order: a `Door` with a
    /// fresh object id, its group registered, full hp, spawned at its position. Returns how many
    /// were spawned (Java prints the number of distinct door ids at load time).
    // Java: Door.startTimerOpen for `OPEN_BY_TIME` doors is not armed (see the module notes).
    pub fn spawn_doors(&mut self, templates: &DoorTemplates, ids: &IdFactory) -> Result<usize, IdError> {
        let mut spawned = 0;
        for template in templates.iter() {
            let id = ids.allocate()?;
            let template = Arc::new(template.clone());
            if let Some(group) = &template.group_name {
                self.door_index
                    .groups
                    .entry(group.clone())
                    .or_default()
                    .insert(template.id);
            }
            let (x, y, z) = (template.pos_x, template.pos_y, template.pos_z);
            self.door_index.by_door_id.insert(template.id, id);
            self.doors.insert(id, Door::new(id, template));
            self.register_object(id, ObjectKind::Door);
            self.spawn_at(id, x, y, z);
            spawned += 1;
        }
        self.door_snapshot = None;
        Ok(spawned)
    }

    /// Java `FenceData.spawnFence` for every fence of `FenceData.xml`.
    pub fn spawn_fences(&mut self, templates: &FenceTemplates, ids: &IdFactory) -> Result<usize, IdError> {
        let mut spawned = 0;
        for t in templates.iter() {
            let id = ids.allocate()?;
            let mut height_fences = Vec::new();
            if t.height > 1 {
                for _ in 0..t.height - 1 {
                    height_fences.push(ids.allocate()?);
                }
            }
            let fence = Fence {
                object_id: id,
                name: t.name.clone(),
                x: t.x,
                y: t.y,
                z: t.z,
                width: t.width,
                length: t.length,
                x_min: t.x - (t.width / 2),
                x_max: t.x + (t.width / 2),
                y_min: t.y - (t.length / 2),
                y_max: t.y + (t.length / 2),
                state: t.state,
                height_fences,
            };
            self.fences.insert(id, fence);
            self.register_object(id, ObjectKind::Fence);
            self.spawn_at(id, t.x, t.y, t.z);
            spawned += 1;
        }
        self.door_snapshot = None;
        Ok(spawned)
    }

    /// Java `StaticObjectData.addObject` for every object of `StaticObjects.xml`.
    pub fn spawn_static_objects(
        &mut self,
        templates: &StaticObjectTemplates,
        ids: &IdFactory,
    ) -> Result<usize, IdError> {
        let mut spawned = 0;
        for t in templates.iter() {
            let id = ids.allocate()?;
            self.static_objects.insert(
                id,
                StaticObject {
                    object_id: id,
                    static_object_id: t.id,
                    name: t.name.clone(),
                    kind: t.kind,
                    mesh_index: 0,
                    texture: t.texture.clone(),
                    map_x: t.map_x,
                    map_y: t.map_y,
                    x: t.x,
                    y: t.y,
                    z: t.z,
                },
            );
            self.register_object(id, ObjectKind::StaticObject);
            self.spawn_at(id, t.x, t.y, t.z);
            spawned += 1;
        }
        Ok(spawned)
    }

    /// The doors, fences and static objects of the datapack, in the order `GameServer` loads
    /// them (`DoorData`, `FenceData`, `StaticObjectData`).
    pub fn spawn_door_data(&mut self, pack: &l2_data::Datapack, ids: &IdFactory) -> Result<(), IdError> {
        let doors = self.spawn_doors(&pack.doors, ids)?;
        let fences = self.spawn_fences(&pack.fences, ids)?;
        let objects = self.spawn_static_objects(&pack.static_objects, ids)?;
        debug!("spawned {doors} doors, {fences} fences, {objects} static objects");
        Ok(())
    }

    pub fn door(&self, id: ObjectId) -> Option<&Door> {
        self.doors.get(&id)
    }

    /// Java `DoorData.getDoor(doorId)`: the door with this `Doors.xml` id.
    pub fn door_by_door_id(&self, door_id: i32) -> Option<&Door> {
        self.door_index
            .by_door_id
            .get(&door_id)
            .and_then(|id| self.doors.get(id))
    }

    /// Java `DoorData.getDoorsByGroup(name)`: the door ids of a group.
    pub fn door_group(&self, name: &str) -> Option<&BTreeSet<i32>> {
        self.door_index.groups.get(name)
    }

    pub fn doors(&self) -> impl Iterator<Item = &Door> {
        self.doors.values()
    }

    pub fn fence(&self, id: ObjectId) -> Option<&Fence> {
        self.fences.get(&id)
    }

    pub fn fences(&self) -> impl Iterator<Item = &Fence> {
        self.fences.values()
    }

    pub fn static_object(&self, id: ObjectId) -> Option<&StaticObject> {
        self.static_objects.get(&id)
    }

    pub fn static_objects(&self) -> impl Iterator<Item = &StaticObject> {
        self.static_objects.values()
    }

    /// The door and fence hook for geoengine calls made in the world thread. Borrows the world;
    /// make a new one per call.
    pub fn obstacles(&self) -> WorldObstacles<'_> {
        WorldObstacles {
            doors: &self.doors,
            fences: &self.fences,
            regions: &self.regions,
            objects: &self.objects,
        }
    }

    /// The immutable door / fence view for path workers. Rebuilt only after a door or fence
    /// changed since the last call.
    pub fn door_snapshot(&mut self) -> Arc<DoorSnapshot> {
        if let Some(snapshot) = &self.door_snapshot {
            return Arc::clone(snapshot);
        }
        let mut snapshot = DoorSnapshot::default();
        for (id, door) in &self.doors {
            let Some(entry) = self.objects.get(id) else {
                continue;
            };
            let (Some(home), true) = (entry.region, door.blocks()) else {
                continue;
            };
            snapshot.doors.entry(home).or_default().push(SnapDoor {
                shape: door.shape(),
                instance_id: entry.instance_id,
            });
        }
        for (id, fence) in &self.fences {
            let Some(entry) = self.objects.get(id) else {
                continue;
            };
            let (Some(home), true) = (entry.region, fence.state.is_geodata_enabled()) else {
                continue;
            };
            snapshot.fences.entry(home).or_default().push(SnapFence {
                shape: fence.shape(),
                instance_id: entry.instance_id,
            });
        }
        let snapshot = Arc::new(snapshot);
        self.door_snapshot = Some(Arc::clone(&snapshot));
        snapshot
    }

    // ---- state changes

    /// Java `Door.openMe()`: opens the door, or its whole group, and tells everybody who sees it.
    pub fn open_door(&mut self, id: ObjectId) {
        let Some(door) = self.doors.get(&id) else {
            return;
        };
        if let Some(group) = door.template.group_name.clone() {
            self.manage_group_open(id, true, &group);
            return;
        }
        if !door.open {
            self.set_door_open(id, true);
            self.broadcast_door_status(id);
            // Java: startAutoCloseTask() (see the module notes).
        }
    }

    /// Java `Door.closeMe()`.
    pub fn close_door(&mut self, id: ObjectId) {
        // Java first cancels the auto close task (not modelled).
        let Some(door) = self.doors.get(&id) else {
            return;
        };
        if let Some(group) = door.template.group_name.clone() {
            self.manage_group_open(id, false, &group);
            return;
        }
        if door.open {
            self.set_door_open(id, false);
            self.broadcast_door_status(id);
        }
    }

    /// Java `Door.setTargetable`: sets the flag and sends the door's state again.
    pub fn set_door_targetable(&mut self, id: ObjectId, value: bool) {
        let Some(door) = self.doors.get_mut(&id) else {
            return;
        };
        door.targetable = value;
        self.broadcast_door_status(id);
    }

    /// Marks a door dead or alive. Java's `doDie` / revival belong to combat (M4); this carries
    /// the one thing the geodata checks read, since a dead door does not block.
    pub fn set_door_dead(&mut self, id: ObjectId, dead: bool) {
        if let Some(door) = self.doors.get_mut(&id) {
            door.dead = dead;
            self.door_snapshot = None;
        }
    }

    /// Java `Fence.setState`: changes the state and sends the fence to everybody who sees it.
    pub fn set_fence_state(&mut self, id: ObjectId, state: FenceState) {
        let Some(fence) = self.fences.get_mut(&id) else {
            return;
        };
        fence.state = state;
        self.door_snapshot = None;
        // Java `broadcastInfo()`
        for viewer in self.visible_players(id) {
            if self.is_visible_for(id, viewer) {
                self.send_fence_info(id, viewer);
            }
        }
    }

    /// Java `Fence.deleteMe()`: out of view for everybody and out of `FenceData`.
    pub fn remove_fence(&mut self, id: ObjectId) {
        if !self.fences.contains_key(&id) {
            return;
        }
        self.decay(id);
        self.fences.remove(&id);
        self.door_snapshot = None;
    }

    /// Java `StaticObject.setMeshIndex`: switches the textures and sends the object again.
    pub fn set_static_object_mesh_index(&mut self, id: ObjectId, mesh_index: i32) {
        let Some(object) = self.static_objects.get_mut(&id) else {
            return;
        };
        object.mesh_index = mesh_index;
        let packet = StaticObjectInfo::static_object(object.static_object_id, id.0, mesh_index);
        // Creature.broadcastPacket
        self.broadcast_packet(id, &packet);
    }

    // Java: Door.setOpen
    fn set_door_open(&mut self, id: ObjectId, open: bool) {
        let Some(door) = self.doors.get_mut(&id) else {
            return;
        };
        door.open = open;
        let child_id = door.template.child_door_id;
        self.door_snapshot = None;
        if child_id > 0 {
            match self.sibling_door(id, child_id) {
                Some(sibling) => self.notify_child_event(sibling, open),
                None => warn!("Door: cannot find child id: {child_id}"),
            }
        }
    }

    // Java: Door.notifyChildEvent
    fn notify_child_event(&mut self, id: ObjectId, open: bool) {
        let Some(door) = self.doors.get(&id) else {
            return;
        };
        let open_this = if open {
            door.template.master_door_open
        } else {
            door.template.master_door_close
        };
        match open_this {
            1 => self.open_door(id),
            -1 => self.close_door(id),
            _ => {}
        }
    }

    // Java: Door.manageGroupOpen
    fn manage_group_open(&mut self, id: ObjectId, open: bool, group: &str) {
        let members: Vec<i32> = self
            .door_index
            .groups
            .get(group)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        for door_id in members {
            let Some(door) = self.sibling_door(id, door_id) else {
                continue;
            };
            if self.doors.get(&door).is_some_and(|d| d.open != open) {
                self.set_door_open(door, open);
                self.broadcast_door_status(door);
            }
        }
        // Java: the first door of the group starts the auto close task when opening.
    }

    /// Java `Door.getSiblingDoor`: a door of the same world. Doors in an instance look in the
    /// instance, which does not exist yet.
    fn sibling_door(&self, id: ObjectId, door_id: i32) -> Option<ObjectId> {
        if self.objects.get(&id).is_some_and(|e| e.instance_id > 0) {
            return None;
        }
        self.door_index.by_door_id.get(&door_id).copied()
    }

    // ---- packets

    /// Java `Door.broadcastStatusUpdate`: every player who can see the door gets its state.
    fn broadcast_door_status(&self, id: ObjectId) {
        for viewer in self.visible_players(id) {
            if self.is_visible_for(id, viewer) {
                self.send_door_info(id, viewer);
            }
        }
    }

    /// Java `Door.sendInfo(player)`.
    pub(crate) fn send_door_info(&self, door: ObjectId, viewer: ObjectId) {
        let Some(door) = self.doors.get(&door) else {
            return;
        };
        if !self.is_visible_for(door.object_id, viewer) {
            return;
        }
        let (info, status, trigger) = door.info_packets(self.is_gm(viewer));
        self.send(viewer, &info);
        self.send(viewer, &status);
        if let Some(trigger) = trigger {
            self.send(viewer, &trigger);
        }
    }

    /// Java `Fence.sendInfo(player)`.
    pub(crate) fn send_fence_info(&self, fence: ObjectId, viewer: ObjectId) {
        let Some(fence) = self.fences.get(&fence) else {
            return;
        };
        let piece = |object_id: ObjectId| ExColosseumFenceInfo {
            object_id: object_id.0,
            state: fence.state.client_id(),
            x: fence.x,
            y: fence.y,
            z: fence.z,
            width: fence.width,
            length: fence.length,
        };
        self.send(viewer, &piece(fence.object_id));
        // NOTE(java-quirk): every height piece is sent at the fence's own z, so the pieces do not
        // stack; the client decides what to draw from the ids.
        for height_id in &fence.height_fences {
            self.send(viewer, &piece(*height_id));
        }
    }

    /// Java `StaticObject.sendInfo(player)`.
    pub(crate) fn send_static_object_info(&self, object: ObjectId, viewer: ObjectId) {
        let Some(object) = self.static_objects.get(&object) else {
            return;
        };
        self.send(
            viewer,
            &StaticObjectInfo::static_object(object.static_object_id, object.object_id.0, object.mesh_index),
        );
    }

    // ---- hooks called by the generic spawn / decay code

    /// Before an object leaves the world: Java `Fence.decayMe` deletes the height pieces for
    /// everybody who sees the fence (no visibility check, as in Java).
    pub(crate) fn on_object_decay(&mut self, id: ObjectId) {
        let Some(kind) = self.objects.get(&id).map(|e| e.kind) else {
            return;
        };
        match kind {
            ObjectKind::Fence => {
                let pieces = self
                    .fences
                    .get(&id)
                    .map(|f| f.height_fences.clone())
                    .unwrap_or_default();
                if !pieces.is_empty() {
                    for viewer in self.visible_players(id) {
                        for piece in &pieces {
                            self.send(viewer, &DeleteObject { object_id: piece.0 });
                        }
                    }
                }
                self.door_snapshot = None;
            }
            ObjectKind::Door => self.door_snapshot = None,
            _ => {}
        }
    }
}
