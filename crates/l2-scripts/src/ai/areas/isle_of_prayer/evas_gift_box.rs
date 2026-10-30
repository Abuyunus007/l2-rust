//! Eva's Gift Box drops coral and crystal for a character with the Kiss of Eva.
// Java: dist/game/data/scripts/ai/areas/IsleOfPrayer/EvasGiftBox.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const BOX: i32 = 32342; // Eva's Gift Box
// Skill
const BUFF: i32 = 1073; // Kiss of Eva
// Items
const CORAL: (i32, i64) = (9692, 1); // Red Coral
const CRYSTAL: (i32, i64) = (9693, 1); // Crystal Fragment

/// Java `ai.areas.IsleOfPrayer.EvasGiftBox`.
pub struct EvasGiftBox;

impl Script for EvasGiftBox {
    fn name(&self) -> &'static str {
        "EvasGiftBox"
    }

    fn path(&self) -> &'static str {
        "ai/areas/IsleOfPrayer"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[BOX]);
        s.add_spawn_id(&[BOX]);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.is_affected_by_skill(killer, BUFF) {
            if ctx.get_random_boolean() {
                ctx.drop_item(npc, Some(killer), CRYSTAL.0, CRYSTAL.1);
            }
            if ctx.get_random(100) < 33 {
                ctx.drop_item(npc, Some(killer), CORAL.0, CORAL.1);
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_random_walking(npc, false);
        ctx.world_mut().set_on_kill_delay(npc.object_id, 0);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="32342" type="Monster" level="70" name="Eva's Gift Box"><stats><vitals hp="100" mp="100"/></stats></npc>"#;

    #[test]
    fn the_box_drops_nothing_without_the_kiss_of_eva() {
        let mut registry = ScriptRegistry::new();
        registry.add(EvasGiftBox);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 70, vec![]);
        let mut boxes = Vec::new();
        for _ in 0..30 {
            let b = t.spawn(BOX, 20);
            t.ticks(2);
            boxes.push(b);
        }
        for b in &boxes {
            t.world.run_as_script("EvasGiftBox", |ctx| {
                let npc = ctx.script_npc(*b).unwrap();
                EvasGiftBox.on_kill(ctx, npc, ObjectId(1), false);
            });
        }
        // No buff, no drop: nothing of the two items lies on the ground.
        assert!(
            t.world
                .nearest_ground_item(ObjectId(1), CRYSTAL.0, 2000)
                .is_none()
        );
        assert!(t.world.nearest_ground_item(ObjectId(1), CORAL.0, 2000).is_none());
    }
}
