//! The Sandstorm casts Gust at whoever comes near.
// Java: dist/game/data/scripts/ai/others/Sandstorms.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const SANDSTORM: i32 = 32350;

// Skills
const GUST: SkillKey = SkillKey { id: 5435, level: 1 }; // Gust

/// Java `ai.others.Sandstorms`.
pub struct Sandstorms;

impl Script for Sandstorms {
    fn name(&self) -> &'static str {
        "Sandstorms"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_aggro_range_enter_id(&[SANDSTORM]); // Sandstorm
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) {
        ctx.cast_skill(npc, player, GUST);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="32350" type="Monster" level="50" name="Sandstorm"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn the_sandstorm_targets_the_player() {
        let mut registry = ScriptRegistry::new();
        registry.add(Sandstorms);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let storm = t.spawn(SANDSTORM, 20);
        t.enter(1, 60, vec![]);
        t.world.run_as_script("Sandstorms", |ctx| {
            let npc = ctx.script_npc(storm).unwrap();
            Sandstorms.on_aggro_range_enter(ctx, npc, ObjectId(1), false);
        });
        assert_eq!(t.world.creature_target(storm), Some(ObjectId(1)));
    }
}
