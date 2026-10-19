//! Putting objects into the world, moving them between regions and taking them out, with the
//! packets Java sends for each, plus the "who can see this" queries every game feature uses.
// Java: org/l2jmobius/gameserver/entity/WorldObject.java (spawnMe, decayMe, setXYZ)
// Java: org/l2jmobius/gameserver/entity/World.java (addVisibleObject, removeVisibleObject,
//       switchRegion, forEachVisibleObject*, disposeOutOfBoundsObject)
// Java: org/l2jmobius/gameserver/entity/WorldRegion.java (addVisibleObject, removeVisibleObject,
//       setActive, startActivation, startDeactivation, areNeighborsEmpty)
//
// What Java does on top that is not here yet, with its card:
// - AI hooks (`describeStateToPlayer`, `setIntentionActive`, `notifyActionForgetObject`,
//   `switchAI`): M4.
// - Doors, fences and static objects are in `door.rs`; NPCs in `npc.rs`.

use l2_net::Encode;
use l2_protocol_game::server::DeleteObject;
use l2_util::ObjectId;
use tracing::warn;

use crate::objects::{ObjectEntry, ObjectKind};
use crate::region::{RegionId, WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN, region_of};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

impl World {
    /// The kind and grid state of an object.
    pub fn object(&self, id: ObjectId) -> Option<&ObjectEntry> {
        self.objects.get(&id)
    }

    /// Where an object is, from its own table.
    pub fn position_of(&self, id: ObjectId) -> Option<(i32, i32, i32)> {
        match self.objects.get(&id)?.kind {
            ObjectKind::Player => self.players.get(&id).map(|p| (p.core.x, p.core.y, p.core.z)),
            ObjectKind::Npc => self.npcs.get(&id).map(|n| (n.core.x, n.core.y, n.core.z)),
            ObjectKind::Door => self.doors.get(&id).map(|d| (d.x, d.y, d.z)),
            ObjectKind::Fence => self.fences.get(&id).map(|f| (f.x, f.y, f.z)),
            ObjectKind::StaticObject => self.static_objects.get(&id).map(|o| (o.x, o.y, o.z)),
        }
    }

    fn set_position_raw(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        let Some(kind) = self.objects.get(&id).map(|e| e.kind) else {
            return;
        };
        match kind {
            ObjectKind::Player => {
                if let Some(p) = self.players.get_mut(&id) {
                    (p.core.x, p.core.y, p.core.z) = (x, y, z);
                }
            }
            ObjectKind::Npc => {
                if let Some(n) = self.npcs.get_mut(&id) {
                    (n.core.x, n.core.y, n.core.z) = (x, y, z);
                }
            }
            ObjectKind::Door => {
                if let Some(d) = self.doors.get_mut(&id) {
                    (d.x, d.y, d.z) = (x, y, z);
                }
            }
            ObjectKind::Fence => {
                if let Some(f) = self.fences.get_mut(&id) {
                    (f.x, f.y, f.z) = (x, y, z);
                }
            }
            ObjectKind::StaticObject => {
                if let Some(o) = self.static_objects.get_mut(&id) {
                    (o.x, o.y, o.z) = (x, y, z);
                }
            }
        }
    }

    /// Registers an object that is in its kind's table (Java `World.addObject` comes with
    /// `spawnMe`; registering first lets a caller set position and flags before spawning).
    pub(crate) fn register_object(&mut self, id: ObjectId, kind: ObjectKind) {
        self.objects.entry(id).or_insert_with(|| ObjectEntry::new(kind));
    }

    /// Java `WorldObject.spawnMe(x, y, z)`: clamps into the world, then spawns.
    pub fn spawn_at(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        let x = if x > WORLD_X_MAX {
            WORLD_X_MAX - 5000
        } else if x < WORLD_X_MIN {
            WORLD_X_MIN + 5000
        } else {
            x
        };
        let y = if y > WORLD_Y_MAX {
            WORLD_Y_MAX - 5000
        } else if y < WORLD_Y_MIN {
            WORLD_Y_MIN + 5000
        } else {
            y
        };
        // Not spawned yet, so this does not switch regions (same as Java's setXYZ before spawnMe).
        // Creature.setXYZ may refuse the position (see `zones.rs`); `spawnMe()` then runs anyway,
        // at the old position.
        if self.zone_gate_set_xyz(id, x, y) {
            self.set_position_raw(id, x, y, z);
        }
        self.spawn(id);
    }

    /// Java `WorldObject.spawnMe()`: into its region, then everybody around learns about it and
    /// it learns about everybody around.
    pub fn spawn(&mut self, id: ObjectId) {
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        let Some(region) = region_of(x, y, z) else {
            self.dispose_out_of_bounds(id);
            return;
        };
        if let Some(entry) = self.objects.get_mut(&id) {
            entry.spawned = true;
            entry.region = Some(region);
        }
        self.region_add(region, id);
        self.world_add_visible(id, region);
        // Java `onSpawn()`.
        self.on_spawn(id);
    }

    /// Java `WorldObject.decayMe()`: out of view for everybody, out of the world table.
    pub fn decay(&mut self, id: ObjectId) {
        self.decay_keep_entry(id);
        // Java `World.removeObject`.
        self.objects.remove(&id);
    }

    /// `decayMe()` without `World.removeObject`: out of view for everybody, but the object keeps
    /// its table entry (kind, instance, invisibility, last region). A teleporting player needs
    /// that, because it is still online and gets its position set while it is away from the
    /// grid (Java keeps `_allPlayers` and the object's own fields the same way).
    pub(crate) fn decay_keep_entry(&mut self, id: ObjectId) {
        // Fences send DeleteObject for their height pieces first; the door snapshot is dropped.
        self.on_object_decay(id);
        let region = match self.objects.get_mut(&id) {
            Some(entry) => {
                entry.spawned = false;
                entry.region
            }
            None => return,
        };
        if let Some(region) = region {
            self.world_remove_visible(id, region);
        }
    }

    /// Java `WorldObject.setXYZ`: moves the object; a spawned object that crosses into another
    /// region switches regions (with the packets that brings).
    ///
    /// For creatures this also runs `Creature.setXYZ`'s zone part (a position outside the zone
    /// grid is refused, a new zone cell drops the old cell's zones) and, for a spawned player,
    /// Java's movement-tick `revalidateZone(false)` right after the move, so every caller (the
    /// movement tick, `ValidatePosition`, teleports) keeps the zone flags and the compass icon
    /// current without a call of its own. See `zones.rs`.
    pub fn set_position(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        if !self.zone_gate_set_xyz(id, x, y) {
            return;
        }
        self.set_position_raw(id, x, y, z);
        self.set_position_region(id, x, y, z);
        self.revalidate_zone(id, false);
    }

    /// The region part of `WorldObject.setXYZ`.
    fn set_position_region(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        let Some(entry) = self.objects.get(&id) else {
            return;
        };
        if !entry.spawned {
            return;
        }
        let old = entry.region;
        let Some(new) = region_of(x, y, z) else {
            return;
        };
        if Some(new) == old {
            return;
        }
        if let Some(old) = old {
            self.region_remove(old, id);
        }
        self.region_add(new, id);
        self.switch_region(id, new);
        if let Some(entry) = self.objects.get_mut(&id) {
            entry.region = Some(new);
        }
    }

    /// Java `WorldRegion.isActive()`.
    pub fn is_region_active(&self, region: RegionId) -> bool {
        self.regions.is_active(region)
    }

    /// Java `WorldRegion.areNeighborsActive()` of the region `id` is in (NPC AI asks it, M4).
    pub fn are_region_neighbors_active(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .and_then(|e| e.region)
            .is_some_and(|r| self.regions.are_neighbors_active(r))
    }

    /// Java `isVisibleFor(player)`: invisible objects are seen by GMs only.
    pub fn is_visible_for(&self, id: ObjectId, viewer: ObjectId) -> bool {
        let invisible = self.objects.get(&id).is_some_and(|e| e.invisible);
        !invisible || self.is_gm(viewer)
    }

    pub(crate) fn is_gm(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| self.data.access_level(p.data.access_level).is_gm)
    }

    fn instance_of(&self, id: ObjectId) -> i32 {
        self.objects.get(&id).map_or(0, |e| e.instance_id)
    }

    /// Java `World.forEachVisibleObject(object, WorldObject.class, ...)`: every other object in the
    /// surrounding regions of `id`'s current position, same instance, in region then id order.
    pub fn visible_objects(&self, id: ObjectId) -> Vec<ObjectId> {
        let Some((x, y, z)) = self.position_of(id) else {
            return Vec::new();
        };
        let Some(region) = region_of(x, y, z) else {
            return Vec::new();
        };
        let instance = self.instance_of(id);
        let mut out = Vec::new();
        for r in region.surrounding() {
            for wo in self.regions.objects(r) {
                if wo != id && self.instance_of(wo) == instance {
                    out.push(wo);
                }
            }
        }
        out
    }

    /// Java `forEachVisibleObjectInRange`: like [`Self::visible_objects`], within `range`
    /// (`calculateDistance3D`).
    pub fn visible_objects_in_range(&self, id: ObjectId, range: i32) -> Vec<ObjectId> {
        let Some((x, y, z)) = self.position_of(id) else {
            return Vec::new();
        };
        let range = f64::from(range);
        self.visible_objects(id)
            .into_iter()
            .filter(|wo| {
                self.position_of(*wo).is_some_and(|(ox, oy, oz)| {
                    let (dx, dy, dz) = (f64::from(ox - x), f64::from(oy - y), f64::from(oz - z));
                    (dx * dx + dy * dy + dz * dz).sqrt() <= range
                })
            })
            .collect()
    }

    /// The players among [`Self::visible_objects`].
    pub fn visible_players(&self, id: ObjectId) -> Vec<ObjectId> {
        self.visible_objects(id)
            .into_iter()
            .filter(|wo| self.objects.get(wo).is_some_and(|e| e.kind == ObjectKind::Player))
            .collect()
    }

    /// Java `Creature.broadcastPacket`: every player who can see `id` gets the packet (not `id`
    /// itself; a player sends its own copy with `send`).
    pub fn broadcast_packet(&self, id: ObjectId, packet: &impl Encode) {
        let bytes = packet.to_bytes();
        for viewer in self.visible_players(id) {
            if self.is_visible_for(id, viewer) {
                self.send_bytes(viewer, bytes.clone());
            }
        }
    }

    /// Java `broadcastPacket(packet, radius)`: the same within `range`.
    pub fn broadcast_packet_in_range(&self, id: ObjectId, packet: &impl Encode, range: i32) {
        let bytes = packet.to_bytes();
        for viewer in self.visible_objects_in_range(id, range) {
            if self
                .objects
                .get(&viewer)
                .is_some_and(|e| e.kind == ObjectKind::Player)
                && self.is_visible_for(id, viewer)
            {
                self.send_bytes(viewer, bytes.clone());
            }
        }
    }

    /// Java `World.addVisibleObject`.
    fn world_add_visible(&mut self, id: ObjectId, region: RegionId) {
        if !self.regions.is_active(region) {
            return;
        }
        let Some(kind) = self.objects.get(&id).map(|e| e.kind) else {
            return;
        };
        for wo in self.visible_objects(id) {
            let Some(wo_kind) = self.objects.get(&wo).map(|e| e.kind) else {
                continue;
            };
            if kind == ObjectKind::Player && self.is_visible_for(wo, id) {
                self.send_info(wo, id);
            }
            if wo_kind == ObjectKind::Player && self.is_visible_for(id, wo) {
                self.send_info(id, wo);
            }
        }
    }

    /// Java `World.removeVisibleObject`.
    fn world_remove_visible(&mut self, id: ObjectId, old_region: RegionId) {
        self.region_remove(old_region, id);
        for r in old_region.surrounding() {
            let objects: Vec<ObjectId> = self.regions.objects(r).collect();
            for wo in objects {
                if wo != id {
                    self.forget_each_other(id, wo);
                }
            }
        }
    }

    /// Java `World.switchRegion` (called with `id` already in `new`'s object set).
    fn switch_region(&mut self, id: ObjectId, new: RegionId) {
        let Some(old) = self.objects.get(&id).and_then(|e| e.region) else {
            return;
        };
        if old == new {
            return;
        }
        for r in old.surrounding() {
            if new.is_surrounding(r) {
                continue;
            }
            let objects: Vec<ObjectId> = self.regions.objects(r).collect();
            for wo in objects {
                if wo != id {
                    self.forget_each_other(id, wo);
                }
            }
        }
        let Some(kind) = self.objects.get(&id).map(|e| e.kind) else {
            return;
        };
        let instance = self.instance_of(id);
        for r in new.surrounding() {
            if old.is_surrounding(r) {
                continue;
            }
            let objects: Vec<ObjectId> = self.regions.objects(r).collect();
            for wo in objects {
                if wo == id || self.instance_of(wo) != instance {
                    continue;
                }
                let Some(wo_kind) = self.objects.get(&wo).map(|e| e.kind) else {
                    continue;
                };
                if kind == ObjectKind::Player && self.is_visible_for(wo, id) {
                    self.send_info(wo, id);
                }
                if wo_kind == ObjectKind::Player && self.is_visible_for(id, wo) {
                    self.send_info(id, wo);
                }
            }
        }
    }

    /// The two halves of Java's loops in `removeVisibleObject` / `switchRegion`: each forgets
    /// the other (targets cleared, `DeleteObject` to players). No visibility check, as in Java.
    fn forget_each_other(&mut self, object: ObjectId, wo: ObjectId) {
        let (Some(kind), Some(wo_kind)) = (
            self.objects.get(&object).map(|e| e.kind),
            self.objects.get(&wo).map(|e| e.kind),
        ) else {
            return;
        };
        if kind.is_creature() {
            self.clear_target_if(object, wo);
            if kind == ObjectKind::Player {
                self.send(object, &DeleteObject { object_id: wo.0 });
            }
        }
        if wo_kind.is_creature() {
            self.clear_target_if(wo, object);
            if wo_kind == ObjectKind::Player {
                self.send(wo, &DeleteObject { object_id: object.0 });
            }
        }
    }

    /// Java `sendInfo(player)`: what `viewer` gets when `subject` comes into view.
    pub(crate) fn send_info(&mut self, subject: ObjectId, viewer: ObjectId) {
        let Some(kind) = self.objects.get(&subject).map(|e| e.kind) else {
            return;
        };
        match kind {
            ObjectKind::Player => self.send_player_info(subject, viewer),
            ObjectKind::Npc => self.send_npc_info(subject, viewer),
            ObjectKind::Door => self.send_door_info(subject, viewer),
            ObjectKind::Fence => self.send_fence_info(subject, viewer),
            ObjectKind::StaticObject => self.send_static_object_info(subject, viewer),
        }
    }

    /// Java `WorldRegion.addVisibleObject` (the region part of spawning / switching).
    fn region_add(&mut self, region: RegionId, id: ObjectId) {
        let kind = self.objects.get(&id).map(|e| e.kind);
        let playable = kind.is_some_and(ObjectKind::is_playable);
        let always_on = self.regions.always_on;
        let active = {
            let r = self.regions.get_mut(region);
            r.objects.insert(id);
            r.active
        };
        // Java: a door or fence also goes into the lists of every surrounding region, which the
        // geodata checks read.
        match kind {
            Some(ObjectKind::Door) => {
                for r in region.surrounding() {
                    self.regions.get_mut(r).doors.insert(id);
                }
            }
            Some(ObjectKind::Fence) => {
                for r in region.surrounding() {
                    self.regions.get_mut(r).fences.insert(id);
                }
            }
            _ => {}
        }
        // "If this is the first player to enter the region, activate self and neighbors."
        if playable && !active && !always_on {
            self.start_activation(region);
        }
    }

    /// Java `WorldRegion.removeVisibleObject`.
    fn region_remove(&mut self, region: RegionId, id: ObjectId) {
        let removed = match self.regions.get(region) {
            Some(r) if !r.objects.is_empty() => self.regions.get_mut(region).objects.remove(&id),
            _ => false,
        };
        let kind = self.objects.get(&id).map(|e| e.kind);
        // Java removes a door or fence from the surrounding regions' lists after the
        // `_visibleObjects.isEmpty()` early return, whether or not it was in the set.
        if self.regions.get(region).is_some() && matches!(kind, Some(ObjectKind::Door | ObjectKind::Fence)) {
            for r in region.surrounding() {
                if let Some(list) = self.regions.get(r) {
                    if !list.doors.is_empty() || !list.fences.is_empty() {
                        let list = self.regions.get_mut(r);
                        list.doors.remove(&id);
                        list.fences.remove(&id);
                    }
                    self.regions.compact(r);
                }
            }
        }
        let playable = kind.is_some_and(ObjectKind::is_playable);
        if removed && playable && self.are_neighbors_empty(region) && !self.regions.always_on {
            self.start_deactivation(region);
        }
        self.regions.compact(region);
    }

    /// Java `WorldRegion.areNeighborsEmpty`: no playable in any active surrounding region.
    fn are_neighbors_empty(&self, region: RegionId) -> bool {
        for r in region.surrounding() {
            if self.regions.is_active(r)
                && self
                    .regions
                    .objects(r)
                    .any(|o| self.objects.get(&o).is_some_and(|e| e.kind.is_playable()))
            {
                return false;
            }
        }
        true
    }

    /// Java `WorldRegion.setActive`.
    fn set_region_active(&mut self, region: RegionId, value: bool) {
        if self.regions.is_active(region) == value {
            return;
        }
        self.regions.get_mut(region).active = value;
        for r in region.surrounding() {
            let n = self.regions.get_mut(r);
            n.active_neighbors += if value { 1 } else { -1 };
            self.regions.compact(r);
        }
        // Java `switchAI(value)`: NPC AI, regeneration and random animation tasks. M4.
        self.regions.compact(region);
    }

    /// Java `WorldRegion.startActivation`: self now, the neighbours after
    /// `GridNeighborTurnOnTime` seconds (unless the player leaves again first).
    fn start_activation(&mut self, region: RegionId) {
        self.set_region_active(region, true);
        let delay = secs_to_ticks(self.settings.grid_neighbor_turnon_secs);
        self.replace_neighbors_task(region, delay, TimerKind::RegionNeighborsOn(region));
    }

    /// Java `WorldRegion.startDeactivation`: after `GridNeighborTurnOffTime` seconds, switch off
    /// every surrounding region that holds no player.
    fn start_deactivation(&mut self, region: RegionId) {
        let delay = secs_to_ticks(self.settings.grid_neighbor_turnoff_secs);
        self.replace_neighbors_task(region, delay, TimerKind::RegionNeighborsOff(region));
    }

    fn replace_neighbors_task(&mut self, region: RegionId, delay_ticks: u64, kind: TimerKind) {
        let due = self.tick + delay_ticks;
        let timer = self.timers.schedule_at(due, kind);
        if let Some(old) = self.regions.get_mut(region).neighbors_task.replace(timer) {
            self.timers.cancel(old);
        }
    }

    /// The region timers of [`TimerKind`].
    pub(crate) fn on_region_timer(&mut self, kind: TimerKind) {
        match kind {
            TimerKind::RegionNeighborsOn(region) => {
                self.regions.get_mut(region).neighbors_task = None;
                for r in region.surrounding() {
                    self.set_region_active(r, true);
                }
                self.regions.compact(region);
            }
            TimerKind::RegionNeighborsOff(region) => {
                self.regions.get_mut(region).neighbors_task = None;
                for r in region.surrounding() {
                    if self.are_neighbors_empty(r) {
                        self.set_region_active(r, false);
                    }
                }
                self.regions.compact(region);
            }
            _ => {}
        }
    }

    /// Java `World.disposeOutOfBoundsObject`, as far as M3 models it: a player stays where the
    /// server last saw it (movement keeps the last valid position, M3-MOVE-01); other objects are
    /// removed.
    fn dispose_out_of_bounds(&mut self, id: ObjectId) {
        let Some(entry) = self.objects.get(&id) else {
            return;
        };
        if entry.kind == ObjectKind::Player {
            warn!("Player {id} is at an invalid location; not spawned.");
            return;
        }
        warn!("Deleting object {id} from an invalid location.");
        if let Some(region) = entry.region {
            self.world_remove_visible(id, region);
        }
        self.objects.remove(&id);
        self.npcs.remove(&id);
        self.doors.remove(&id);
        self.fences.remove(&id);
        self.static_objects.remove(&id);
        self.door_snapshot = None;
    }
}

/// `1000 * seconds` milliseconds of `ThreadPool.schedule`, in world ticks.
fn secs_to_ticks(secs: i32) -> u64 {
    TimerWheel::duration_to_ticks(std::time::Duration::from_secs(
        u64::try_from(secs.max(0)).unwrap_or(0),
    ))
}
