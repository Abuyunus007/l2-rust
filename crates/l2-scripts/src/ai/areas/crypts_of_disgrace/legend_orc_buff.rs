//! The Legend Orc buffer blesses every player that comes into its sight.
// Java: dist/game/data/scripts/ai/areas/CryptsOfDisgrace/LegendOrcBuff.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const LEGEND_ORC_BUFF: i32 = 18837;
// Skill
const BLESSED_FOCUS: SkillKey = SkillKey { id: 6235, level: 1 };

/// Java `ai.areas.CryptsOfDisgrace.LegendOrcBuff`.
pub struct LegendOrcBuff;

impl Script for LegendOrcBuff {
    fn name(&self) -> &'static str {
        "LegendOrcBuff"
    }

    fn path(&self) -> &'static str {
        "ai/areas/CryptsOfDisgrace"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_creature_see_id(&[LEGEND_ORC_BUFF]);
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_some() {
            ctx.world_mut()
                .apply_effects(npc.object_id, creature, BLESSED_FOCUS);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="18837" type="Monster" level="70" name="Legend Orc"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn the_orc_sees_a_player_after_a_second_and_does_not_fail_for_an_npc() {
        let mut registry = ScriptRegistry::new();
        registry.add(LegendOrcBuff);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let orc = t.spawn(LEGEND_ORC_BUFF, 30);
        t.enter(1, 70, vec![]);
        t.wait_ms(2_500);
        // The world sees the player from the orc (the effect itself needs the skill table).
        assert!(t.world.creature_see_count() > 0);
        t.world.run_as_script("LegendOrcBuff", |ctx| {
            let npc = ctx.script_npc(orc).unwrap();
            LegendOrcBuff.on_creature_see(ctx, npc, ObjectId(1));
            LegendOrcBuff.on_creature_see(ctx, npc, orc);
        });
    }
}
