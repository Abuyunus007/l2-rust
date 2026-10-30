//! Sirra's first page depends on the Freya instance the character stands in.
// Java: dist/game/data/scripts/ai/areas/FrozenLabyrinth/Sirra/Sirra.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SIRRA: i32 = 32762;
// Misc
const FREYA_INSTID: i32 = 139;
const FREYA_HARD_INSTID: i32 = 144;

/// Java `ai.areas.FrozenLabyrinth.Sirra.Sirra`.
pub struct Sirra;

impl Script for Sirra {
    fn name(&self) -> &'static str {
        "Sirra"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FrozenLabyrinth/Sirra"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[SIRRA]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        // `InstanceManager.getWorld(npc)`.
        let world = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| (w.template_id, w.status()));
        match world {
            Some((FREYA_INSTID, status)) => Some(
                if status == 0 {
                    "32762-easy.html"
                } else {
                    "32762-easyfight.html"
                }
                .to_owned(),
            ),
            Some((FREYA_HARD_INSTID, status)) => Some(
                if status == 0 {
                    "32762-hard.html"
                } else {
                    "32762-hardfight.html"
                }
                .to_owned(),
            ),
            _ => Some("32762.html".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{InstanceWorld, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="32762" type="Folk" level="70" name="Sirra"/>"#;

    #[test]
    fn the_page_follows_the_instance() {
        let mut registry = ScriptRegistry::new();
        registry.add(Sirra);
        let pages = Pages::of(&Sirra).plain(&[
            "32762.html",
            "32762-easy.html",
            "32762-easyfight.html",
            "32762-hard.html",
            "32762-hardfight.html",
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let sirra = t.spawn(SIRRA, 20);
        t.enter(1, 85, vec![]);
        t.talk(1, sirra);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32762.html"));

        // The NPC stands in instance 0 (the test world has no instance templates); a script
        // world is keyed by the instance id of the object.
        for (status, template, page) in [
            (0, FREYA_INSTID, "32762-easy.html"),
            (1, FREYA_INSTID, "32762-easyfight.html"),
            (0, FREYA_HARD_INSTID, "32762-hard.html"),
            (2, FREYA_HARD_INSTID, "32762-hardfight.html"),
        ] {
            let mut world = InstanceWorld::new(0, template);
            world.set_status(status);
            t.world.add_instance_world(world);
            t.world.run_as_script("Sirra", |ctx| {
                let npc = ctx.script_npc(sirra).unwrap();
                assert_eq!(Sirra.on_first_talk(ctx, npc, ObjectId(1)).as_deref(), Some(page));
            });
        }
    }
}
