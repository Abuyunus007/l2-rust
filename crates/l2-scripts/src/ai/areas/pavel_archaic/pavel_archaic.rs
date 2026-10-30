//! The golems of Pavel Archaic leave two more golems behind when they die.
// Java: dist/game/data/scripts/ai/areas/PavelArchaic/PavelArchaic.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const SAFETY_DEVICE: i32 = 18917; // Pavel Safety Device
const PINCER_GOLEM: i32 = 22801; // Cruel Pincer Golem
const PINCER_GOLEM2: i32 = 22802; // Cruel Pincer Golem
const PINCER_GOLEM3: i32 = 22803; // Cruel Pincer Golem
const JACKHAMMER_GOLEM: i32 = 22804; // Horrifying Jackhammer Golem

/// Java `ai.areas.PavelArchaic.PavelArchaic`.
pub struct PavelArchaic;

impl Script for PavelArchaic {
    fn name(&self) -> &'static str {
        "PavelArchaic"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PavelArchaic"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[SAFETY_DEVICE, PINCER_GOLEM, JACKHAMMER_GOLEM]);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_random(100) < 70
            && let Some((x, y, z, heading)) = ctx.npc_position(npc)
        {
            for golem_id in [PINCER_GOLEM2, PINCER_GOLEM3] {
                if let Some(golem) = ctx.add_spawn(golem_id, x, y, z + 20, heading, false, 0, 0) {
                    ctx.add_attack_desire(golem, killer, 999);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22801" type="Monster" level="80" name="Pincer 1"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22802" type="Monster" level="80" name="Pincer 2"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22803" type="Monster" level="80" name="Pincer 3"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn a_kill_leaves_two_golems_most_of_the_time() {
        let mut registry = ScriptRegistry::new();
        registry.add(PavelArchaic);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let golem = t.spawn(PINCER_GOLEM, 300);
        let npc = ScriptNpc {
            object_id: golem,
            id: PINCER_GOLEM,
        };
        let mut pairs = 0;
        for _ in 0..10 {
            t.world.run_as_script("PavelArchaic", |ctx| {
                PavelArchaic.on_kill(ctx, npc, ObjectId(1), false);
            });
            let (a, b) = (t.npcs_of(PINCER_GOLEM2), t.npcs_of(PINCER_GOLEM3));
            assert_eq!(a.len(), b.len());
            if a.len() > pairs {
                pairs = a.len();
            }
            for id in a.into_iter().chain(b) {
                t.world.delete_npc(id);
            }
        }
        assert!(pairs >= 1);
    }
}
