//! The Hellbound engine: the level and the trust of the island, the spawns of each level, the
//! doors, and the trust points of the kills (`HellboundLoader` loads it first).
// Java: dist/game/data/scripts/ai/areas/Hellbound/HellboundEngine.java, HellboundLoader.java,
//       HellboundPointData.java, HellboundSpawns.java
//
// The engine is not a singleton here: its Java fields (`_cachedLevel`, `_maxTrust`, `_minTrust`
// and the `Spawn`s of `HellboundSpawns`) are the [`HbState`] of the script, a shared state the
// other Hellbound scripts reach through the free functions below (`level`, `update_trust`, ...).
// The level and the trust are the `HBLevel` / `HBTrust` global variables, as in Java (D-54: kept
// in memory, written only with `CycleDatabaseWrites`, and re-read every update because the Java
// GS may have changed them, D-50).
//
// `//hellbound_setlevel` and `//hellbound` (Java `AdminHellbound`, a handler outside this folder)
// live in `l2_world::script::hellbound_api`: the setter writes `HBLevel`, which this engine's
// update sees as "changed by administrator".

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};
use tracing::info;

use super::data::{SPAWNS, TRUST_POINTS};

/// Java `DOOR_LIST`: `(door id, the level it opens at)`.
const DOOR_LIST: [(i32, i32); 4] = [(19_250_001, 5), (19_250_002, 5), (20_250_001, 9), (20_250_002, 7)];
/// Java `MAX_TRUST`.
const MAX_TRUST: [i32; 12] = l2_world::script::hellbound_api::HELLBOUND_MAX_TRUST;

// Monsters
const DEREK: i32 = 18465;

// Engine
const ANNOUNCEMENT: &str = "Hellbound has reached level: %lvl%";
const UPDATE_INTERVAL: i64 = 60_000; // 1 minute.
const UPDATE_EVENT: &str = "UPDATE";
/// The update after the re-read of the two variables (D-50).
const UPDATE_RUN: &str = "UPDATE_RUN";

/// The fields of Java's `HellboundEngine` and the `Spawn`s of `HellboundSpawns`.
pub struct HbState {
    /// `_cachedLevel`.
    cached_level: i32,
    /// `_maxTrust`.
    max_trust: i32,
    /// `_minTrust`.
    min_trust: i32,
    /// `HellboundSpawns._spawns` with `_spawnLevels`: `(spawn index, min level, max level)`.
    spawns: Vec<(usize, i32, i32)>,
}

impl Default for HbState {
    fn default() -> Self {
        Self {
            cached_level: -1,
            max_trust: 0,
            min_trust: 0,
            spawns: Vec::new(),
        }
    }
}

/// Java `HellboundEngine.getLevel()`.
pub fn level(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("HBLevel", 0)
}

/// Java `HellboundEngine.setLevel(lvl)`.
pub fn set_level(ctx: &mut ScriptCtx<'_>, lvl: i32) {
    if lvl == level(ctx) {
        return;
    }
    info!("HellboundEngine: Changing level from {} to {}.", level(ctx), lvl);
    ctx.set_global_variable("HBLevel", &lvl.to_string());
}

/// Java `HellboundEngine.getTrust()`.
pub fn trust(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("HBTrust", 0)
}

/// Java `HellboundEngine.setTrust(trust)`.
fn set_trust(ctx: &mut ScriptCtx<'_>, trust: i32) {
    ctx.set_global_variable("HBTrust", &trust.to_string());
}

/// Java `HellboundEngine.isLocked()`.
pub fn is_locked(ctx: &ScriptCtx<'_>) -> bool {
    level(ctx) <= 0
}

/// Java `HellboundEngine.getMaxTrust()`.
pub fn max_trust(ctx: &mut ScriptCtx<'_>) -> i32 {
    ctx.shared_state::<HbState>().max_trust
}

/// Java `HellboundEngine.getMinTrust()`.
pub fn min_trust(ctx: &mut ScriptCtx<'_>) -> i32 {
    ctx.shared_state::<HbState>().min_trust
}

/// Java `HellboundEngine.setMaxTrust(trust)`.
fn set_max_trust(ctx: &mut ScriptCtx<'_>, value: i32) {
    ctx.shared_state::<HbState>().max_trust = value;
    if value > 0 && trust(ctx) > value {
        set_trust(ctx, value);
    }
}

/// Java `HellboundEngine.setMinTrust(trust)`.
fn set_min_trust(ctx: &mut ScriptCtx<'_>, value: i32) {
    let max = {
        let state = ctx.shared_state::<HbState>();
        state.min_trust = value;
        state.max_trust
    };
    if trust(ctx) >= max {
        set_trust(ctx, value);
    }
}

/// Java `HellboundEngine.updateTrust(trust, useRates)`.
pub fn update_trust(ctx: &mut ScriptCtx<'_>, trust_points: i32, use_rates: bool) {
    if is_locked(ctx) {
        return;
    }
    let mut reward = trust_points;
    if use_rates {
        let settings = ctx.script_settings();
        let rate = if trust_points > 0 {
            settings.rate_hb_trust_increase
        } else {
            settings.rate_hb_trust_decrease
        };
        // `(int) (trust * rate)`: an int times a float is a float multiplication.
        reward = (trust_points as f32 * rate) as i32;
    }
    let (min, max) = {
        let state = ctx.shared_state::<HbState>();
        (state.min_trust, state.max_trust)
    };
    let final_trust = trust(ctx).wrapping_add(reward).max(min);
    if max > 0 {
        set_trust(ctx, final_trust.min(max));
    } else {
        set_trust(ctx, final_trust);
    }
}

/// Java `HellboundEngine.doSpawn()`: adds and removes the spawns for the level.
fn do_spawn(ctx: &mut ScriptCtx<'_>) {
    let mut added = 0;
    let mut deleted = 0;
    let lvl = level(ctx);
    let spawns = ctx.shared_state::<HbState>().spawns.clone();
    for (index, min_level, max_level) in spawns {
        let last = ctx.world().last_spawn(index);
        let alive = last.filter(|id| ctx.world().npc(*id).is_some_and(|n| !n.decayed));
        if lvl < min_level || lvl > max_level {
            ctx.world_mut().stop_spawn_respawn(index);
            if let Some(npc) = alive {
                ctx.world_mut().delete_npc(npc);
                deleted += 1;
            }
        } else {
            ctx.world_mut().start_spawn_respawn(index);
            let world = ctx.world();
            // Java reuses the `Npc` object of the spawn (`setDecayed(false)`, `doRevive()`,
            // `setSpawned(true)`); here an NPC that is gone or waits for its respawn is a new one.
            // NOTE(java-quirk): Java also revives a dead NPC whose corpse is still there; here
            // that NPC decays and respawns by itself.
            if world.spawn_current_count(index) == 0 && world.spawn_scheduled_count(index) == 0 {
                ctx.world_mut().script_do_spawn(index);
                added += 1;
            } else if let Some(npc) = alive {
                let stats = ctx
                    .world()
                    .stats(npc)
                    .map(|s| (f64::from(s.max_hp()), f64::from(s.max_mp())));
                if let Some((hp, mp)) = stats
                    && !ctx.is_dead(npc)
                {
                    ctx.world_mut().set_current_hp_mp(npc, hp, mp);
                }
            }
        }
    }
    if added > 0 {
        info!("HellboundEngine: Spawned {added} NPCs.");
    }
    if deleted > 0 {
        info!("HellboundEngine: Removed {deleted} NPCs.");
    }
}

/// Java `HellboundEngine.onLevelChange(newLevel)`: called on every level change.
fn on_level_change(ctx: &mut ScriptCtx<'_>, new_level: i32) {
    // `setMaxTrust(MAX_TRUST[newLevel]); setMinTrust(MAX_TRUST[newLevel - 1]);` inside a
    // try/catch: an index outside the table sets both to 0.
    let at = |l: i32| usize::try_from(l).ok().and_then(|i| MAX_TRUST.get(i)).copied();
    match at(new_level) {
        Some(max) => {
            set_max_trust(ctx, max);
            match at(new_level - 1) {
                Some(min) => set_min_trust(ctx, min),
                None => {
                    set_max_trust(ctx, 0);
                    set_min_trust(ctx, 0);
                }
            }
        }
        None => {
            set_max_trust(ctx, 0);
            set_min_trust(ctx, 0);
        }
    }
    update_trust(ctx, 0, false);
    do_spawn(ctx);
    for (door_id, open_level) in DOOR_LIST {
        let open = ctx.world().door_by_door_id(door_id).map(|d| d.open);
        match open {
            Some(true) => {
                if new_level < open_level {
                    ctx.close_door(door_id, 0);
                }
            }
            Some(false) => {
                if new_level >= open_level {
                    ctx.open_door(door_id, 0);
                }
            }
            None => info!("HellboundEngine : Doors problem! door {door_id} not found"),
        }
    }
    if ctx.shared_state::<HbState>().cached_level > 0 {
        let text = ANNOUNCEMENT.replace("%lvl%", &new_level.to_string());
        ctx.world().script_broadcast_announcement(&text);
        info!("HellboundEngine: New level: {new_level}");
    }
    ctx.shared_state::<HbState>().cached_level = new_level;
}

/// Java `ai.areas.Hellbound.HellboundEngine`.
pub struct HellboundEngine;

impl Script for HellboundEngine {
    fn name(&self) -> &'static str {
        "HellboundEngine"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let ids: Vec<i32> = TRUST_POINTS.iter().map(|row| row.0).collect();
        s.add_kill_id(&ids);
    }

    // The Java constructor loads `HellboundSpawns` (`SpawnTable.addSpawn`, nothing spawned yet)
    // and starts the update timer.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let mut spawns = Vec::with_capacity(SPAWNS.len());
        for &(npc_id, x, y, z, heading, delay, random, min_level, max_level) in SPAWNS {
            let index = ctx
                .world_mut()
                .script_add_hellbound_spawn(npc_id, (x, y, z), heading, delay, random);
            spawns.push((index, min_level, max_level));
        }
        info!("HellboundSpawns: Loaded {} Hellbound spawns.", spawns.len());
        ctx.shared_state::<HbState>().spawns = spawns;
        ctx.start_quest_timer(UPDATE_EVENT, 1000, None, None, false);
        info!("HellboundEngine: Level: {}", level(ctx));
        info!("HellboundEngine: Trust: {}", trust(ctx));
        info!(
            "HellboundEngine: Status: {}",
            if is_locked(ctx) { "locked." } else { "unlocked." }
        );
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        // D-50: the Java GS may have changed the two rows; read them, then run the update.
        if event == UPDATE_EVENT {
            ctx.reread_global_variables(&["HBLevel", "HBTrust"], UPDATE_RUN, None, None);
        } else if event == UPDATE_RUN {
            let mut lvl = level(ctx);
            let cached = ctx.shared_state::<HbState>().cached_level;
            if lvl > 0 && lvl == cached {
                let max = ctx.shared_state::<HbState>().max_trust;
                // Only exclusion is kill of Derek.
                if trust(ctx) == max && lvl != 4 {
                    lvl += 1;
                    set_level(ctx, lvl);
                    on_level_change(ctx, lvl);
                }
            } else {
                // First run or changed by administrator.
                on_level_change(ctx, lvl);
            }
            ctx.start_quest_timer(UPDATE_EVENT, UPDATE_INTERVAL, None, None, false);
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let npc_id = npc.id;
        if let Some(&(_, points, min_hb_lvl, max_hb_lvl, lowest_trust_limit)) =
            TRUST_POINTS.iter().find(|row| row.0 == npc_id)
        {
            let lvl = level(ctx);
            if lvl >= min_hb_lvl
                && lvl <= max_hb_lvl
                && (lowest_trust_limit == 0 || trust(ctx) > lowest_trust_limit)
            {
                update_trust(ctx, points, true);
            }
            if npc_id == DEREK && lvl == 4 {
                set_level(ctx, 5);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    // One NPC of each level range of the spawn table, and the trust kills.
    const NPCS: &str = r#"<npc id="32352" type="Folk" level="70" name="Level 1 to 100"/>
        <npc id="22320" type="Monster" level="60" name="Junior Watchman"/>
        <npc id="18465" type="Monster" level="60" name="Derek"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        // `CycleDatabaseWrites` on: the update's re-read of the two rows (D-50) runs at once.
        ScriptTest::new_with(registry, NPCS, "", &[], |world, _| {
            world.set_script_settings(l2_world::ScriptSettings {
                cycle_database_writes: true,
                ..l2_world::ScriptSettings::default()
            });
        })
    }

    fn var(t: &ScriptTest, name: &str) -> i32 {
        t.world.global_variables().get_int(name, 0)
    }

    fn set(t: &mut ScriptTest, name: &str, value: i32) {
        t.world.run_as_script("HellboundEngine", |ctx| {
            ctx.set_global_variable(name, &value.to_string());
        });
    }

    #[test]
    fn the_spawn_and_point_tables_are_the_xml() {
        assert_eq!(SPAWNS.len(), 598);
        assert_eq!(TRUST_POINTS.len(), 47);
        // Derek: 10000 points at level 4.
        assert!(TRUST_POINTS.contains(&(18465, 10000, 4, 4, 0)));
        // A native: -10 points above a trust of 100020.
        assert!(TRUST_POINTS.contains(&(22322, -10, 1, 4, 100_020)));
    }

    #[test]
    fn a_locked_island_takes_no_trust() {
        let mut t = setup();
        t.world.run_as_script("HellboundEngine", |ctx| {
            assert!(is_locked(ctx));
            update_trust(ctx, 500, false);
        });
        assert_eq!(var(&t, "HBTrust"), 0);
    }

    #[test]
    fn the_update_applies_the_level_the_administrator_set() {
        let mut t = setup();
        set(&mut t, "HBLevel", 2);
        set(&mut t, "HBTrust", 450_000);
        // The first update runs a second after the load.
        t.wait_ms(1_100);
        t.world.run_as_script("HellboundEngine", |ctx| {
            assert_eq!(min_trust(ctx), 300_000);
            assert_eq!(max_trust(ctx), 600_000);
        });
        assert_eq!(var(&t, "HBTrust"), 450_000);
        // The trust is held between the limits of the level.
        t.world.run_as_script("HellboundEngine", |ctx| {
            update_trust(ctx, 10_000_000, false);
        });
        assert_eq!(var(&t, "HBTrust"), 600_000);
        t.world.run_as_script("HellboundEngine", |ctx| {
            update_trust(ctx, -10_000_000, false);
        });
        assert_eq!(var(&t, "HBTrust"), 300_000);
    }

    #[test]
    fn the_level_goes_up_when_the_trust_reaches_the_maximum() {
        let mut t = setup();
        set(&mut t, "HBLevel", 1);
        t.wait_ms(1_100);
        set(&mut t, "HBTrust", 300_000);
        // The next update is a minute later.
        t.wait_ms(60_100);
        assert_eq!(var(&t, "HBLevel"), 2);
        // Setting the trust to the maximum of the new level: the cached level moved on.
        t.world.run_as_script("HellboundEngine", |ctx| {
            assert_eq!(max_trust(ctx), 600_000);
        });
    }

    #[test]
    fn a_kill_in_the_level_range_gives_its_points() {
        let mut t = setup();
        t.enter(1, 80, vec![]);
        set(&mut t, "HBLevel", 1);
        t.wait_ms(1_100);
        let mob = t.spawn(22320, 20);
        t.kill(1, mob);
        // Junior Watchman: 1 point at level 1, on top of the minimum of the level (0).
        assert_eq!(var(&t, "HBTrust"), 1);
    }

    #[test]
    fn derek_opens_level_five() {
        let mut t = setup();
        t.enter(1, 80, vec![]);
        set(&mut t, "HBLevel", 4);
        t.wait_ms(1_100);
        let derek = t.spawn(18465, 20);
        t.kill(1, derek);
        assert_eq!(var(&t, "HBLevel"), 5);
    }

    #[test]
    fn the_spawns_follow_the_level() {
        let mut t = setup();
        // Level 0: the spawns of level 1 and up are not there.
        t.wait_ms(1_100);
        assert!(t.npcs_of(32352).is_empty());
        set(&mut t, "HBLevel", 1);
        t.wait_ms(60_100);
        assert_eq!(t.npcs_of(32352).len(), 1);
        // Back to 0: gone again (the spawn is `min=1`).
        set(&mut t, "HBLevel", 0);
        t.wait_ms(60_100);
        // NOTE: 32352 has no level range in the XML (1 to 100), so level 0 removes it.
        assert!(t.npcs_of(32352).is_empty());
    }
}
