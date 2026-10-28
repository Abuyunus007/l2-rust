//! Small `Npc` helpers the scripts of Gracia share, built on the `pub` world API.
// Java: entity/actor/Npc.java (setDisplayEffect, broadcastSocialAction)

use l2_protocol_game::server::{ExChangeNpcState, SocialAction};
use l2_util::ObjectId;
use l2_world::ScriptCtx;

/// Java `Npc.setDisplayEffect(value)`: announced to the players around only when it changes.
pub fn set_display_effect(ctx: &mut ScriptCtx<'_>, npc: ObjectId, value: i32) {
    let changed = ctx.world_mut().npc_mut(npc).is_some_and(|n| {
        let changed = n.display_effect != value;
        n.display_effect = value;
        changed
    });
    if changed {
        ctx.world().broadcast_packet(
            npc,
            &ExChangeNpcState {
                object_id: npc.0,
                state: value,
            },
        );
    }
}

/// Java `WorldObject.calculateDistance3D(other)`: `f64::MAX` when either is not in the world.
pub fn distance_3d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    let (Some(a), Some(b)) = (ctx.world().position_of(a), ctx.world().position_of(b)) else {
        return f64::MAX;
    };
    let (dx, dy, dz) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1), f64::from(a.2 - b.2));
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// The iteration order of a Java `HashMap<Integer, Integer>` of at most 12 entries (the default
/// capacity 16 never grows): by bucket `(h ^ (h >>> 16)) & 15`, in insertion order inside a bucket.
/// The order decides which random draw goes to which spawn.
pub fn java_hash_map_order(entries: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let bucket = |key: i32| {
        let h = key as u32;
        ((h ^ (h >> 16)) & 15) as usize
    };
    let mut ordered: Vec<(usize, usize, (i32, i32))> = entries
        .iter()
        .enumerate()
        .map(|(at, entry)| (bucket(entry.0), at, *entry))
        .collect();
    ordered.sort_by_key(|(bucket, at, _)| (*bucket, *at));
    ordered.into_iter().map(|(_, _, entry)| entry).collect()
}

/// Java `util.PolygonTerritory`: a polygon with a height range, for a random point inside.
// Java: dist/game/java .../gameserver/util/PolygonTerritory.java
pub struct PolygonTerritory {
    /// Java `_points`: `(x, y, zmin, zmax, proc)`.
    points: Vec<(i32, i32, i32, i32, i32)>,
    terr: i32,
    x_min: i32,
    x_max: i32,
    y_min: i32,
    y_max: i32,
    z_min: i32,
    z_max: i32,
    proc_max: i32,
}

impl PolygonTerritory {
    pub fn new(terr: i32) -> Self {
        Self {
            points: Vec::new(),
            terr,
            x_min: 999_999,
            x_max: -999_999,
            y_min: 999_999,
            y_max: -999_999,
            z_min: 999_999,
            z_max: -999_999,
            proc_max: 0,
        }
    }

    /// Java `add(x, y, zmin, zmax, proc)`.
    pub fn add(&mut self, x: i32, y: i32, zmin: i32, zmax: i32, proc: i32) {
        self.points.push((x, y, zmin, zmax, proc));
        self.x_min = self.x_min.min(x);
        self.y_min = self.y_min.min(y);
        self.x_max = self.x_max.max(x);
        self.y_max = self.y_max.max(y);
        self.z_min = self.z_min.min(zmin);
        self.z_max = self.z_max.max(zmax);
        self.proc_max += proc;
    }

    fn is_intersect(x: i32, y: i32, p1: (i32, i32, i32, i32, i32), p2: (i32, i32, i32, i32, i32)) -> bool {
        let dy1 = f64::from(p1.1 - y);
        let dy2 = f64::from(p2.1 - y);
        if (dy1.signum_java() - dy2.signum_java()).abs() <= 1e-6 {
            return false;
        }

        let dx1 = f64::from(p1.0 - x);
        let dx2 = f64::from(p2.0 - x);
        if dx1 >= 0.0 && dx2 >= 0.0 {
            return true;
        }

        if dx1 < 0.0 && dx2 < 0.0 {
            return false;
        }

        let dx0 = (dy1 * f64::from(p1.0 - p2.0)) / f64::from(p1.1 - p2.1);
        dx0 <= dx1
    }

    /// Java `isInside(x, y)`.
    pub fn is_inside(&self, x: i32, y: i32) -> bool {
        let mut intersect_count = 0;
        for i in 0..self.points.len() {
            let p1 = self.points[if i > 0 { i - 1 } else { self.points.len() - 1 }];
            let p2 = self.points[i];
            if Self::is_intersect(x, y, p1, p2) {
                intersect_count += 1;
            }
        }

        intersect_count % 2 == 1
    }

    /// Java `getRandomPoint()`: `None` (Java `null`) when 100 tries found no point inside.
    pub fn random_point(&self, ctx: &mut ScriptCtx<'_>) -> Option<(i32, i32, i32)> {
        if self.proc_max > 0 {
            let mut pos = 0;
            let rnd = ctx.get_random(self.proc_max);
            for p1 in &self.points {
                pos += p1.4;
                if rnd <= pos {
                    return Some((p1.0, p1.1, ctx.get_random_range(p1.2, p1.3)));
                }
            }
        }

        for _ in 0..100 {
            let x = ctx.get_random_range(self.x_min, self.x_max);
            let y = ctx.get_random_range(self.y_min, self.y_max);
            if self.is_inside(x, y) {
                let mut curdistance = 0.0;
                let mut zmin = self.z_min;
                for p1 in &self.points {
                    let dx = f64::from(p1.0 - x);
                    let dy = f64::from(p1.1 - y);
                    let distance = (dx * dx + dy * dy).sqrt();
                    if curdistance == 0.0 || distance < curdistance {
                        curdistance = distance;
                        zmin = p1.2;
                    }
                }

                return Some((x, y, ctx.get_random_range(zmin, self.z_max)));
            }
        }

        // Java logs "Can't make point for territory <terr>" here (`l2-scripts` has no logger).
        let _ = self.terr;
        None
    }
}

/// Java `Math.signum` (0 stays 0, which `f64::signum` does not do).
trait SignumJava {
    fn signum_java(self) -> f64;
}

impl SignumJava for f64 {
    fn signum_java(self) -> f64 {
        if self == 0.0 { 0.0 } else { self.signum() }
    }
}

/// Java `ZoneManager.getInstance().getZoneById(zoneId).getPlayersInside()`.
pub fn players_in_zone(ctx: &ScriptCtx<'_>, zone_id: i32) -> Vec<ObjectId> {
    ctx.world()
        .players()
        .filter(|p| p.zones.members().iter().any(|(id, _)| *id == zone_id))
        .map(|p| p.data.object_id)
        .collect()
}

/// Java `Creature.broadcastSocialAction(id)`.
pub fn broadcast_social_action(ctx: &ScriptCtx<'_>, creature: ObjectId, action_id: i32) {
    ctx.world().broadcast_packet(
        creature,
        &SocialAction {
            object_id: creature.0,
            action_id,
        },
    );
}
