//! Keltas, the Keymaster, Typhoon and the Mutated Elpy appear at one of several places.
// Java: dist/game/data/scripts/ai/others/RandomSpawn.java

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::teleport::TeleportTarget;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

/// Java `Location(x, y, z, heading, instanceId)`; the 3-argument constructor is heading 0,
/// instance 0.
type Loc = (i32, i32, i32, i32);

// Keltas
const KELTAS: [Loc; 4] = [
    (-27136, 250938, -3523, 0),
    (-29658, 252897, -3523, 0),
    (-27237, 251943, -3527, 0),
    (-28868, 250113, -3479, 0),
];
// Keymaster (instance 2)
const KEYMASTER: [Loc; 5] = [
    (14091, 250533, -1940, 2),
    (15762, 252440, -2015, 2),
    (19836, 256212, -2090, 2),
    (21940, 254107, -2010, 2),
    (17299, 252943, -2015, 2),
];
// Typhoon
const TYPHOON: [Loc; 10] = [
    (-20641, 255370, -3235, 0),
    (-16157, 250993, -3058, 0),
    (-18269, 250721, -3151, 0),
    (-16532, 254864, -3223, 0),
    (-19055, 253489, -3440, 0),
    (-9684, 254256, -3148, 0),
    (-6209, 251924, -3189, 0),
    (-10547, 251359, -2929, 0),
    (-7254, 254997, -3261, 0),
    (-4883, 253171, -3322, 0),
];
// Mutated Elpy
const MUTATED_ELPY: [Loc; 6] = [
    (-46080, 246368, -14183, 0),
    (-44816, 246368, -14183, 0),
    (-44224, 247440, -14184, 0),
    (-44896, 248464, -14183, 0),
    (-46064, 248544, -14183, 0),
    (-46720, 247424, -14183, 0),
];

const NPC_IDS: [i32; 4] = [22341, 22361, 25539, 25604];

/// Java `SPAWN_POINTS.get(npcId)`; the instance is the fifth constructor argument.
fn spawn_points(npc_id: i32) -> Option<&'static [Loc]> {
    match npc_id {
        22341 => Some(&KELTAS),
        22361 => Some(&KEYMASTER),
        25539 => Some(&TYPHOON),
        25604 => Some(&MUTATED_ELPY),
        _ => None,
    }
}

/// The places the pending `Teleport` runnables of Java carry (one per NPC).
#[derive(Default)]
struct Pending(HashMap<ObjectId, (i32, i32, i32, i32)>);

/// Java `ai.others.RandomSpawn`.
pub struct RandomSpawn;

impl Script for RandomSpawn {
    fn name(&self) -> &'static str {
        "RandomSpawn"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&NPC_IDS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        // Java's `Teleport.run()`: `_npc.teleToLocation(_loc, false)`.
        if event == "TELEPORT" {
            let npc = npc?;
            let (x, y, z, instance) = ctx.state::<Pending>().0.remove(&npc.object_id)?;
            ctx.world_mut().tele_to_location(
                npc.object_id,
                TeleportTarget::at(x, y, z)
                    .with_heading(0)
                    .with_instance(instance),
            );
        }
        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let Some(spawnlist) = spawn_points(npc.id) else {
            return;
        };
        let Some((x, y, z, instance)) = ctx.get_random_entry(spawnlist) else {
            return;
        };
        let inside = ctx.world().position_of(npc.object_id).is_some_and(|(nx, ny, _)| {
            let (dx, dy) = (i64::from(nx - x), i64::from(ny - y));
            dx * dx + dy * dy <= 200 * 200
        });
        if !inside {
            ctx.world_mut().set_npc_spawn_point(npc.object_id, x, y, z);
            ctx.state::<Pending>()
                .0
                .insert(npc.object_id, (x, y, z, instance));
            ctx.start_quest_timer("TELEPORT", 100, Some(npc), None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="25539" type="RaidBoss" level="80" name="Typhoon"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn typhoon_goes_to_one_of_its_places_after_100_ms() {
        let mut registry = ScriptRegistry::new();
        registry.add(RandomSpawn);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let typhoon = t.spawn(25539, 20);
        t.world.script_drain();
        t.wait_ms(200);
        let (x, y, _) = t.world.position_of(typhoon).unwrap();
        assert!(
            TYPHOON.iter().any(|&(px, py, _, _)| px == x && py == y),
            "{x} {y}"
        );
        assert_eq!(t.world.npc(typhoon).unwrap().spawn_point.0, x);
    }

    #[test]
    fn the_table_is_javas() {
        assert_eq!(spawn_points(22361).unwrap().len(), 5);
        assert_eq!(spawn_points(25604).unwrap()[5], (-46720, 247424, -14183, 0));
        assert!(spawn_points(1).is_none());
    }
}
