//! The monsters of the Isle of Prayer drop the Seed of Evil shards.
// Java: dist/game/data/scripts/ai/areas/IsleOfPrayer/IsleOfPrayer.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Items
const YELLOW_SEED_OF_EVIL_SHARD: i32 = 9593;
const GREEN_SEED_OF_EVIL_SHARD: i32 = 9594;
const BLUE_SEED_OF_EVIL_SHARD: i32 = 9595;
const RED_SEED_OF_EVIL_SHARD: i32 = 9596;
// Monsters: (npc id, (item, chance of 10000))
const MONSTERS: [(i32, (i32, i32)); 15] = [
    (22257, (YELLOW_SEED_OF_EVIL_SHARD, 2087)), // Island Guardian
    (22258, (YELLOW_SEED_OF_EVIL_SHARD, 2147)), // White Sand Mirage
    (22259, (YELLOW_SEED_OF_EVIL_SHARD, 2642)), // Muddy Coral
    (22260, (YELLOW_SEED_OF_EVIL_SHARD, 2292)), // Kleopora
    (22261, (GREEN_SEED_OF_EVIL_SHARD, 1171)),  // Seychelles
    (22262, (GREEN_SEED_OF_EVIL_SHARD, 1173)),  // Naiad
    (22263, (GREEN_SEED_OF_EVIL_SHARD, 1403)),  // Sonneratia
    (22264, (GREEN_SEED_OF_EVIL_SHARD, 1207)),  // Castalia
    (22265, (RED_SEED_OF_EVIL_SHARD, 575)),     // Chrysocolla
    (22266, (RED_SEED_OF_EVIL_SHARD, 493)),     // Pythia
    (22267, (RED_SEED_OF_EVIL_SHARD, 770)),     // Dark Water Dragon
    (22268, (BLUE_SEED_OF_EVIL_SHARD, 987)),    // Shade
    (22269, (BLUE_SEED_OF_EVIL_SHARD, 995)),    // Shade
    (22270, (BLUE_SEED_OF_EVIL_SHARD, 1008)),   // Water Dragon Detractor
    (22271, (BLUE_SEED_OF_EVIL_SHARD, 1008)),   // Water Dragon Detractor
];

/// Java `ai.areas.IsleOfPrayer.IsleOfPrayer`.
pub struct IsleOfPrayer;

impl Script for IsleOfPrayer {
    fn name(&self) -> &'static str {
        "IsleOfPrayer"
    }

    fn path(&self) -> &'static str {
        "ai/areas/IsleOfPrayer"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let ids: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&ids);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some((_, (item, chance))) = MONSTERS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        // NOTE(java-quirk): `<=` against 10000 rolls 0..9999, so a chance of n is n+1 in 10000.
        if ctx.get_random(10000) <= *chance {
            ctx.drop_item(npc, Some(killer), *item, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const ITEMS: &str = r#"<item id="9593" type="EtcItem" name="Yellow Shard"/>"#;
    const NPCS: &str = r#"<npc id="22259" type="Monster" level="70" name="Muddy Coral"><stats><vitals hp="100" mp="100"/></stats></npc>"#;

    #[test]
    fn a_kill_drops_the_shard_of_its_color_now_and_then() {
        let mut registry = ScriptRegistry::new();
        registry.add(IsleOfPrayer);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.enter(1, 70, vec![]);
        // 26 percent of the kills drop the yellow shard; the seeded RNG gets one in 200 kills.
        for _ in 0..200 {
            let mob = t.spawn(22259, 20);
            t.world.run_as_script("IsleOfPrayer", |ctx| {
                let npc = ctx.script_npc(mob).unwrap();
                IsleOfPrayer.on_kill(ctx, npc, ObjectId(1), false);
            });
            if t.world
                .nearest_ground_item(ObjectId(1), YELLOW_SEED_OF_EVIL_SHARD, 2000)
                .is_some()
            {
                return;
            }
        }
        panic!("no shard dropped");
    }
}
