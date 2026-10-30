//! Fantasy Isle Parade: every hour the actors march along five routes and are removed at the end.
// Java: dist/game/data/scripts/ai/areas/FantasyIsle/Parade.java
//
// Java's three `ThreadPool` tasks (spawn every 5 s, delete every 1 s, clean after 7 min) are
// world timers here: "spawn", "delete" and "clean".

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};
use tracing::info;

// @formatter:off
const ACTORS: [i32; 180] = [
    32379, 0, 32379, 32379, 0, 32379, 32379, 0, 32379, 0, 0, 0, 32380, 0, 32380, 32380, 32381, 32380, 32380,
    0, 32380, 32380, 32381, 32380, 0, 0, 0, 32382, 32382, 32382, 32382, 32383, 32382, 32383, 32384, 32383,
    32383, 32384, 32383, 0, 0, 0, 0, 32385, 0, 32385, 0, 32385, 0, 32385, 0, 0, 0, 0, 32412, 0, 32411, 0, 0,
    0, 32421, 0, 32409, 32423, 0, 32422, 0, 0, 0, 32420, 32419, 32417, 32418, 0, 32416, 0, 0, 0, 32414, 0,
    32414, 0, 32413, 0, 32414, 0, 32414, 0, 0, 0, 32393, 0, 32394, 0, 32430, 0, 32392, 0, 32391, 0, 0, 0, 0,
    32404, 0, 32403, 0, 32401, 0, 0, 0, 0, 32408, 0, 32406, 0, 32407, 0, 32405, 0, 0, 0, 0, 32390, 32389,
    32387, 32388, 0, 32386, 0, 0, 0, 0, 32400, 0, 32397, 32398, 32396, 0, 0, 0, 0, 32450, 0, 32448, 32449,
    32447, 0, 0, 0, 32380, 0, 32380, 32380, 32381, 32380, 32380, 0, 32380, 32380, 32381, 32380, 0, 0, 0,
    32379, 0, 32379, 32379, 0, 32379, 32379, 0, 32379, 0, 0, 0, 0, 32415, 0,
];

/// `(x, y, z, heading)` of the three actors in a row.
type Row = [(i32, i32, i32, i32); 3];

// (Northbound 270 degrees) Route 1
const START1: Row = [
    (-54780, -56810, -2015, 49152),
    (-54860, -56810, -2015, 49152),
    (-54940, -56810, -2015, 49152),
];
const GOAL1: Row = [
    (-54780, -57965, -2015, 49152),
    (-54860, -57965, -2015, 49152),
    (-54940, -57965, -2015, 49152),
];

// (Westbound 180 degrees) Route 2
const START2: Row = [
    (-55715, -58900, -2015, 32768),
    (-55715, -58820, -2015, 32768),
    (-55715, -58740, -2015, 32768),
];
const GOAL2: Row = [
    (-60850, -58900, -2015, 32768),
    (-60850, -58820, -2015, 32768),
    (-60850, -58740, -2015, 32768),
];

// (Southbound 90 degrees) Route 3
const START3: Row = [
    (-61790, -57965, -2015, 16384),
    (-61710, -57965, -2015, 16384),
    (-61630, -57965, -2015, 16384),
];
const GOAL3: Row = [
    (-61790, -53890, -2116, 16384),
    (-61710, -53890, -2116, 16384),
    (-61630, -53890, -2116, 16384),
];

// (Eastbound 0 degrees) Route 4
const START4: Row = [
    (-60840, -52990, -2108, 0),
    (-60840, -53070, -2108, 0),
    (-60840, -53150, -2108, 0),
];
const GOAL4: Row = [
    (-58620, -52990, -2015, 0),
    (-58620, -53070, -2015, 0),
    (-58620, -53150, -2015, 0),
];

// (To 315 degrees northeast) Route 5
const START5: Row = [
    (-57233, -53554, -2015, 57344),
    (-57290, -53610, -2015, 57344),
    (-57346, -53667, -2015, 57344),
];
const GOAL5: Row = [
    (-55338, -55435, -2015, 57344),
    (-55395, -55491, -2015, 57344),
    (-55451, -55547, -2015, 57344),
];

const START: [Row; 5] = [START1, START2, START3, START4, START5];
const GOAL: [Row; 5] = [GOAL1, GOAL2, GOAL3, GOAL4, GOAL5];
// @formatter:on

/// Every hour (Java: "repeats every 6 hours" in the comment, `cycle = 3600000` in the code).
const CYCLE: i64 = 3_600_000;

/// Java's `npcIndex` and `spawns` (with the destination each actor was sent to, which Java
/// reads back from the actor as `getXdestination()`).
#[derive(Default)]
struct ParadeState {
    npc_index: usize,
    spawns: Vec<(ObjectId, (i32, i32, i32))>,
}

/// Java `timeLeftMilli(hh, mm, ss)`.
fn time_left_milli(ctx: &ScriptCtx<'_>, hh: i32, mm: i32, ss: i32) -> i64 {
    let now = (i64::from(ctx.world().game_ticks()) * 60) / 100;
    let mut dd = i64::from((hh * 3600) + (mm * 60) + ss) - (now % 86_400);
    if dd < 0 {
        dd += 86_400;
    }

    (dd * 1000) / 6
}

/// Java `Start.run()`.
fn start(ctx: &mut ScriptCtx<'_>) {
    // `load()`
    ctx.state::<ParadeState>().npc_index = 0;
    // `spawnTask`: at once, then every 5 s.
    spawn_step(ctx);
    ctx.start_quest_timer("spawn", 5000, None, None, true);
    // `deleteTask`: after 10 s, then every 1 s.
    ctx.start_quest_timer("delete_first", 10_000, None, None, false);
    // `cleanTask`
    ctx.start_quest_timer("clean", 420_000, None, None, false);
}

/// Java `Spawn.run()`.
fn spawn_step(ctx: &mut ScriptCtx<'_>) {
    for i in 0..3 {
        let index = ctx.state::<ParadeState>().npc_index;
        if index >= ACTORS.len() {
            ctx.cancel_quest_timer("spawn", None, None);
            break;
        }

        let npc_id = ACTORS[index];
        ctx.state::<ParadeState>().npc_index += 1;
        if npc_id == 0 {
            continue;
        }

        for route in 0..5 {
            let (sx, sy, sz, sh) = START[route][i];
            let goal = GOAL[route][i];
            if let Some(actor) = ctx.add_spawn(npc_id, sx, sy, sz, sh, false, 0, 0) {
                ctx.set_running(actor);
                ctx.npc_move_to(actor, goal.0, goal.1, goal.2);
                ctx.state::<ParadeState>()
                    .spawns
                    .push((actor.object_id, (goal.0, goal.1, goal.2)));
            }
        }
    }
}

/// Java `Delete.run()`.
fn delete_step(ctx: &mut ScriptCtx<'_>) {
    let spawns = std::mem::take(&mut ctx.state::<ParadeState>().spawns);
    if spawns.is_empty() {
        ctx.state::<ParadeState>().spawns = spawns;
        return;
    }

    let mut kept = Vec::with_capacity(spawns.len());
    for (actor, goal) in spawns {
        let Some(npc) = ctx.script_npc(actor) else {
            continue;
        };
        let at = ctx.world().position_of(actor);
        let near = at.is_some_and(|(x, y, _)| {
            let (dx, dy) = (f64::from(x - goal.0), f64::from(y - goal.1));
            (dx * dx + dy * dy).sqrt() < 100.0
        });
        if near {
            ctx.delete_npc(npc);
        } else {
            if !ctx.world().is_moving(actor) {
                ctx.npc_move_to(npc, goal.0, goal.1, goal.2);
            }
            kept.push((actor, goal));
        }
    }

    let empty = kept.is_empty();
    ctx.state::<ParadeState>().spawns = kept;
    if empty {
        ctx.cancel_quest_timer("delete", None, None);
    }
}

/// Java `clean()`.
fn clean(ctx: &mut ScriptCtx<'_>) {
    let spawns = std::mem::take(&mut ctx.state::<ParadeState>().spawns);
    for (actor, _) in spawns {
        if let Some(npc) = ctx.script_npc(actor) {
            ctx.delete_npc(npc);
        }
    }
}

/// Java `ai.areas.FantasyIsle.Parade`.
pub struct Parade;

impl Script for Parade {
    fn name(&self) -> &'static str {
        "Parade"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FantasyIsle"
    }

    fn setup(&self, _s: &mut ScriptSetup) {}

    // The Java constructor: starts at 8:00 game time and repeats every `CYCLE`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let diff = time_left_milli(ctx, 8, 0, 0);
        ctx.start_quest_timer("first_start", diff, None, None, false);

        info!(
            "Fantasy Isle: Parade starting in {} minutes and is scheduled each next {} hours.",
            diff / 60_000,
            CYCLE / 3_600_000
        );
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "first_start" => {
                start(ctx);
                ctx.start_quest_timer("start", CYCLE, None, None, true);
            }
            "start" => start(ctx),
            "spawn" => spawn_step(ctx),
            "delete_first" => {
                delete_step(ctx);
                ctx.start_quest_timer("delete", 1000, None, None, true);
            }
            "delete" => delete_step(ctx),
            "clean" => {
                ctx.cancel_quest_timer("spawn", None, None);
                ctx.cancel_quest_timer("delete", None, None);
                clean(ctx);
            }
            _ => {}
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="32379" type="Folk" level="70" name="Parade actor"/>"#;

    #[test]
    fn the_tables_have_java_s_sizes() {
        assert_eq!(ACTORS.len(), 180);
        assert_eq!(ACTORS.iter().filter(|a| **a != 0).count(), 90);
        assert_eq!(START[4][2], (-57346, -53667, -2015, 57344));
        assert_eq!(GOAL[2][0], (-61790, -53890, -2116, 16384));
    }

    #[test]
    fn a_start_spawns_five_actors_per_slot_and_the_clean_removes_them() {
        let mut registry = ScriptRegistry::new();
        registry.add(Parade);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.world.run_as_script("Parade", |ctx| {
            assert!(ctx.has_quest_timer("first_start", None, None));
            // The first row is 32379, 0, 32379: two slots of five actors.
            let _ = Parade.on_event(ctx, "first_start", None, None);
            assert!(ctx.has_quest_timer("spawn", None, None));
            assert!(ctx.has_quest_timer("start", None, None));
            assert_eq!(ctx.state::<ParadeState>().spawns.len(), 10);
        });
        assert_eq!(t.npcs_of(32379).len(), 10);
        t.world.run_as_script("Parade", |ctx| {
            let _ = Parade.on_event(ctx, "clean", None, None);
            assert!(ctx.state::<ParadeState>().spawns.is_empty());
        });
        assert!(t.npcs_of(32379).is_empty());
    }
}
