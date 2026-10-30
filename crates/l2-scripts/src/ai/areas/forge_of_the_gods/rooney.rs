//! Rooney, the wandering merchant of the Forge of the Gods: he talks and jumps to another place
//! a few minutes after a character sees him.
// Java: dist/game/data/scripts/ai/areas/ForgeOfTheGods/Rooney.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ROONEY: i32 = 32049;

// Locations
const LOCATIONS: [(i32, i32, i32); 39] = [
    (175937, -112167, -5550),
    (178896, -112425, -5860),
    (180628, -115992, -6135),
    (183010, -114753, -6135),
    (184496, -116773, -6135),
    (181857, -109491, -5865),
    (178917, -107633, -5853),
    (178804, -110080, -5853),
    (182221, -106806, -6025),
    (186488, -109715, -5915),
    (183847, -119231, -3113),
    (185193, -120342, -3113),
    (188047, -120867, -3113),
    (189734, -120471, -3113),
    (188754, -118940, -3313),
    (190022, -116803, -3313),
    (188443, -115814, -3313),
    (186421, -114614, -3313),
    (185188, -113307, -3313),
    (187378, -112946, -3313),
    (189815, -113425, -3313),
    (189301, -111327, -3313),
    (190289, -109176, -3313),
    (187783, -110478, -3313),
    (185889, -109990, -3313),
    (181881, -109060, -3695),
    (183570, -111344, -3675),
    (182077, -112567, -3695),
    (180127, -112776, -3698),
    (179155, -108629, -3695),
    (176282, -109510, -3698),
    (176071, -113163, -3515),
    (179376, -117056, -3640),
    (179760, -115385, -3640),
    (177950, -119691, -4140),
    (177037, -120820, -4340),
    (181125, -120148, -3702),
    (182212, -117969, -3352),
    (186074, -118154, -3312),
];

/// Java `ai.areas.ForgeOfTheGods.Rooney`.
pub struct Rooney;

impl Script for Rooney {
    fn name(&self) -> &'static str {
        "Rooney"
    }

    fn path(&self) -> &'static str {
        "ai/areas/ForgeOfTheGods"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_creature_see_id(&[ROONEY]);
    }

    // The Java constructor spawns him at a random place.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        if let Some((x, y, z)) = ctx.get_random_entry(&LOCATIONS) {
            ctx.add_spawn(ROONEY, x, y, z, 0, false, 0, 0);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "TELEPORT" {
            let npc = npc?;
            let decayed = ctx.world().npc(npc.object_id).is_some_and(|n| n.decayed);
            if !decayed {
                let ai_val = ctx.get_script_value(npc);
                let text = match ai_val {
                    1 => NpcStringId::HurryHurry,
                    2 => NpcStringId::IAmNotThatTypeOfPersonWhoStaysInOnePlaceForALongTime,
                    3 => NpcStringId::ItSHardForMeToKeepStandingLikeThis,
                    4 => NpcStringId::WhyDonTIGoThatWayThisTime,
                    _ => {
                        if let Some((x, y, z)) = ctx.get_random_entry(&LOCATIONS) {
                            ctx.tele_npc_to(npc, x, y, z);
                        }
                        ctx.set_script_value(npc, 0);
                        return None;
                    }
                };
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);

                ctx.set_script_value(npc, ai_val + 1);
                ctx.start_quest_timer("TELEPORT", 60_000, Some(npc), None, false);
            }
        }

        None
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_some() && ctx.is_script_value(npc, 0) {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::Welcome, &[]);
            ctx.start_quest_timer("TELEPORT", 60_000, Some(npc), None, false);
            ctx.set_script_value(npc, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="32049" type="Folk" level="70" name="Rooney"/>"#;

    #[test]
    fn rooney_spawns_at_load_and_talks_then_jumps_when_a_character_comes() {
        let mut registry = ScriptRegistry::new();
        registry.add(Rooney);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let rooney = t.npcs_of(ROONEY);
        assert_eq!(rooney.len(), 1);
        let rooney = rooney[0];
        let first = t.world.position_of(rooney).unwrap();
        assert!(LOCATIONS.contains(&first));

        // A character at the test place is far away: bring Rooney to it.
        t.enter(1, 75, vec![]);
        t.world.run_as_script("Rooney", |ctx| {
            let npc = ctx.script_npc(rooney).unwrap();
            ctx.tele_npc_to(npc, crate::testing::X + 20, crate::testing::Y, crate::testing::Z);
        });
        t.ticks(60);
        t.wait_ms(1_100);
        t.world.run_as_script("Rooney", |ctx| {
            let npc = ctx.script_npc(rooney).unwrap();
            assert!(ctx.has_quest_timer("TELEPORT", Some(npc), None) || ctx.get_script_value(npc) >= 1);
        });
    }
}
