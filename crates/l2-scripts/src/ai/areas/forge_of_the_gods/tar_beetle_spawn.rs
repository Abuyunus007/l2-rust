//! The Tar Beetle zone spawn of the Forge of the Gods: keeps `maxNpcCount` beetles in each spawn
//! zone and takes the ones nobody used for 5 rounds away again.
// Java: dist/game/data/scripts/ai/areas/ForgeOfTheGods/TarBeetleSpawn.java
//
// The Java class owns two `ThreadPool` tasks (every 60 s from 1 s, and every 300 s from 300 s);
// here they are two quest timers of `TarBeetle` that start themselves again. The beetles of each
// zone are the script's state (`ctx.state`), the zone index of a beetle is its NPC variable
// `zoneIndex`, as in Java.

use l2_util::ObjectId;
use l2_world::{ScriptCtx, ScriptNpc};

use super::tar_beetle_data::{SPAWN_ZONES, Zone};
use crate::ai::areas::gracia::util::PolygonTerritory;

/// Java `new Spawn(18804)`.
pub const TAR_BEETLE: i32 = 18804;

/// Java `scheduleAtFixedRate(refreshSpawn, 1000, 60000)`.
pub const REFRESH_SPAWN: &str = "TAR_BEETLE_REFRESH_SPAWN";
pub const REFRESH_SPAWN_FIRST_MS: i64 = 1_000;
pub const REFRESH_SPAWN_MS: i64 = 60_000;
/// Java `scheduleAtFixedRate(refreshShots, 300000, 300000)`.
pub const REFRESH_SHOTS: &str = "TAR_BEETLE_REFRESH_SHOTS";
pub const REFRESH_SHOTS_MS: i64 = 300_000;

/// How many times `refresh_spawn` may fail to find a point for one beetle.
// NOTE(java-quirk): Java loops without a bound while `getRandomPoint()` gives `null`; the data
// always has points, the bound only keeps a broken zone from hanging the world loop.
const MAX_POINT_TRIES: usize = 1_000;

/// The beetles of each spawn zone (Java `SpawnZone._spawn`), by zone index.
#[derive(Default)]
pub struct TarBeetleZones {
    spawn: Vec<Vec<ObjectId>>,
}

impl TarBeetleZones {
    fn zone(&mut self, index: usize) -> &mut Vec<ObjectId> {
        if self.spawn.len() < SPAWN_ZONES.len() {
            self.spawn.resize_with(SPAWN_ZONES.len(), Vec::new);
        }
        &mut self.spawn[index]
    }
}

/// Java `TarBeetleSpawn.load()`: the two timers (the zones are in `tar_beetle_data.rs`).
pub fn load(ctx: &mut ScriptCtx<'_>) {
    if !SPAWN_ZONES.is_empty() {
        ctx.start_quest_timer(REFRESH_SPAWN, REFRESH_SPAWN_FIRST_MS, None, None, false);
        ctx.start_quest_timer(REFRESH_SHOTS, REFRESH_SHOTS_MS, None, None, false);
    }
}

/// Java `TarBeetleSpawn.unload()`: the timers and the beetles go.
pub fn unload(ctx: &mut ScriptCtx<'_>) {
    ctx.cancel_quest_timers(REFRESH_SPAWN);
    ctx.cancel_quest_timers(REFRESH_SHOTS);
    let zones = std::mem::take(&mut ctx.state::<TarBeetleZones>().spawn);
    for beetle in zones.into_iter().flatten() {
        if let Some(npc) = ctx.script_npc(beetle) {
            ctx.delete_npc(npc);
        }
    }
}

/// Java `TarBeetleSpawn.removeBeetle(npc)`.
pub fn remove_beetle(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let index = ctx
        .world()
        .npc_variables(npc.object_id)
        .map_or(0, |v| v.get_int("zoneIndex", 0));
    if let Ok(index) = usize::try_from(index)
        && index < SPAWN_ZONES.len()
    {
        ctx.state::<TarBeetleZones>().zone(index).retain(|n| *n != npc.object_id);
    }
    ctx.delete_npc(npc);
}

/// Java `Zone.getRandomPoint()`: a point of the polygon outside its banned zones.
fn random_point(ctx: &mut ScriptCtx<'_>, zone: &Zone) -> Option<(i32, i32, i32)> {
    let (min_z, max_z, points, banned) = zone;
    let mut polygon = PolygonTerritory::new(1);
    for (x, y) in *points {
        polygon.add(*x, *y, *min_z, *max_z, 0);
    }
    let banned: Vec<PolygonTerritory> = banned
        .iter()
        .map(|(b_min_z, b_max_z, points)| {
            let mut b = PolygonTerritory::new(1);
            for (x, y) in *points {
                b.add(*x, *y, *b_min_z, *b_max_z, 0);
            }
            b
        })
        .collect();

    let mut location = polygon.random_point(ctx);
    while let Some((x, y, _)) = location
        && banned.iter().any(|b| b.is_inside(x, y))
    {
        location = polygon.random_point(ctx);
    }
    location
}

/// Java `SpawnZone.refreshSpawn()` of every zone (`zones.forEach`).
pub fn refresh_spawn(ctx: &mut ScriptCtx<'_>) {
    for (index, (max_npc_count, zones)) in SPAWN_ZONES.iter().enumerate() {
        let mut tries = 0;
        while ctx.state::<TarBeetleZones>().zone(index).len() < *max_npc_count {
            tries += 1;
            if tries > MAX_POINT_TRIES {
                tracing::warn!("Problem with TarBeetleSpawn: no point found in zone {index}");
                break;
            }
            let pick = ctx.get_random(i32::try_from(zones.len()).unwrap_or(1));
            let Some(zone) = usize::try_from(pick).ok().and_then(|p| zones.get(p)) else {
                break;
            };
            let Some((x, y, z)) = random_point(ctx, zone) else {
                continue;
            };
            let heading = ctx.get_random(65_535);
            let z = ctx.geo_height(x, y, z);
            // `spawn.stopRespawn()`: `add_spawn` makes a spawn without respawn.
            let Some(npc) = ctx.add_spawn(TAR_BEETLE, x, y, z, heading, false, 0, 0) else {
                break;
            };
            ctx.set_random_walking(npc, false);
            ctx.set_immobilized(npc, true);
            ctx.world_mut().set_invul(npc.object_id, true);
            ctx.disable_core_ai(npc, true);
            ctx.set_script_value(npc, 5);
            ctx.world_mut()
                .npc_variables_mut(npc.object_id)
                .set_int("zoneIndex", i32::try_from(index).unwrap_or(0));
            ctx.state::<TarBeetleZones>().zone(index).push(npc.object_id);
        }
    }
}

/// Java `SpawnZone.refreshShots()` of every zone: a beetle that kept its 5 shots for a whole
/// round goes, the others get one back.
pub fn refresh_shots(ctx: &mut ScriptCtx<'_>) {
    for index in 0..SPAWN_ZONES.len() {
        let beetles = ctx.state::<TarBeetleZones>().zone(index).clone();
        for beetle in beetles {
            let Some(npc) = ctx.script_npc(beetle) else {
                continue;
            };
            let val = ctx.get_script_value(npc);
            if val == 5 {
                ctx.delete_npc(npc);
                ctx.state::<TarBeetleZones>().zone(index).retain(|n| *n != beetle);
            } else {
                ctx.set_script_value(npc, val + 1);
            }
        }
    }
}
