//! The Abyss Gatekeeper at the Seed of Infinity: tells what the gaze lets pass.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/AbyssGaze/AbyssGaze.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::seeds::{soi_current_stage, soi_opened_time};

// NPC
const ABYSS_GATEKEEPER: i32 = 32539;

/// Java `ai.areas.Gracia.AI.NPC.AbyssGaze.AbyssGaze`.
pub struct AbyssGaze;

impl Script for AbyssGaze {
    fn name(&self) -> &'static str {
        "AbyssGaze"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/AbyssGaze"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ABYSS_GATEKEEPER]);
        s.add_first_talk_id(&[ABYSS_GATEKEEPER]);
        s.add_talk_id(&[ABYSS_GATEKEEPER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = Some(event.to_owned());
        // `newQuestState(player)` when the player has none.
        ctx.get_quest_state(player, true);

        if event == "request_permission" {
            let stage = soi_current_stage(ctx);
            if stage == 2 || stage == 5 {
                htmltext = Some("32539-2.htm".to_owned());
            } else if stage == 3 && soi_opened_time(ctx, ctx.now_ms()) > 0 {
                htmltext = Some("32539-3.htm".to_owned());
            } else {
                htmltext = Some("32539-1.htm".to_owned());
            }
        } else if event == "enter_seed" && soi_current_stage(ctx) == 3 {
            return None;
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        ctx.get_quest_state(player, true);
        Some("32539.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="32539" type="Folk" level="70" name="Abyss Gatekeeper"/>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/AbyssGaze";

    fn ask(vars: &[(&str, &str)]) -> String {
        let mut registry = ScriptRegistry::new();
        registry.add(AbyssGaze);
        let link = r#"<a action="bypass -h Script AbyssGaze request_permission">x</a>"#;
        let pages = [
            (format!("{DIR}/32539.htm"), format!("<html><body>32539 {link}</body></html>")),
            (format!("{DIR}/32539-1.htm"), "<html><body>32539-1</body></html>".to_owned()),
            (format!("{DIR}/32539-2.htm"), "<html><body>32539-2</body></html>".to_owned()),
            (format!("{DIR}/32539-3.htm"), "<html><body>32539-3</body></html>".to_owned()),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        t.world.set_global_variables(
            vars.iter()
                .map(|(k, v)| ((*k).to_owned(), Some((*v).to_owned())))
                .collect(),
        );
        let npc = t.spawn(ABYSS_GATEKEEPER, 20);
        t.enter(1, 80, vec![]);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32539 "));
        t.bypass(1, "Script AbyssGaze request_permission");
        pages_to(&t.take(), 1).pop().unwrap()
    }

    #[test]
    fn the_gaze_answers_by_the_stage_of_the_seed() {
        // Stage 1 is the default: the seed is closed.
        assert!(ask(&[]).contains("32539-1"));
        assert!(ask(&[("SoI_stage", "2")]).contains("32539-2"));
        assert!(ask(&[("SoI_stage", "5")]).contains("32539-2"));
        // Stage 3 with the seed still open (the close time is a second count, far ahead).
        assert!(ask(&[("SoI_stage", "3"), ("SoI_opened", "99999999999")]).contains("32539-3"));
        // Stage 3 but the open time is over.
        assert!(ask(&[("SoI_stage", "3"), ("SoI_opened", "1")]).contains("32539-1"));
    }
}
