//! The Mouth of Ekimus: which hall the seed's stage lets a party into.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/EkimusMouth/EkimusMouth.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::seeds::soi_current_stage;

// NPC
const EKIMUS_MOUTH: i32 = 32537;

/// Java `ai.areas.Gracia.AI.NPC.EkimusMouth.EkimusMouth`.
pub struct EkimusMouth;

impl Script for EkimusMouth {
    fn name(&self) -> &'static str {
        "EkimusMouth"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/EkimusMouth"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[EKIMUS_MOUTH]);
        s.add_first_talk_id(&[EKIMUS_MOUTH]);
        s.add_talk_id(&[EKIMUS_MOUTH]);
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
        ctx.get_quest_state(player, true);

        if event.eq_ignore_ascii_case("hos_enter") {
            if soi_current_stage(ctx) == 1 {
                htmltext = Some("32537-1.htm".to_owned());
            } else if soi_current_stage(ctx) == 4 {
                htmltext = Some("32537-2.htm".to_owned());
            }
        } else if event.eq_ignore_ascii_case("hoe_enter") {
            if soi_current_stage(ctx) == 1 {
                htmltext = Some("32537-3.htm".to_owned());
            } else if soi_current_stage(ctx) == 4 {
                htmltext = Some("32537-4.htm".to_owned());
            }
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        ctx.get_quest_state(player, true);
        Some("32537.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="32537" type="Folk" level="70" name="Mouth of Ekimus"/>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/EkimusMouth";

    fn ask(stage: &str, event: &str) -> String {
        let mut registry = ScriptRegistry::new();
        registry.add(EkimusMouth);
        let first = r#"<a action="bypass -h Script EkimusMouth hos_enter">a</a><a action="bypass -h Script EkimusMouth hoe_enter">b</a>"#;
        let pages = [
            (format!("{DIR}/32537.htm"), format!("<html><body>32537 {first}</body></html>")),
            (format!("{DIR}/32537-1.htm"), "<html><body>32537-1</body></html>".to_owned()),
            (format!("{DIR}/32537-2.htm"), "<html><body>32537-2</body></html>".to_owned()),
            (format!("{DIR}/32537-3.htm"), "<html><body>32537-3</body></html>".to_owned()),
            (format!("{DIR}/32537-4.htm"), "<html><body>32537-4</body></html>".to_owned()),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        t.world
            .set_global_variables(vec![("SoI_stage".to_owned(), Some(stage.to_owned()))]);
        let npc = t.spawn(32537, 20);
        t.enter(1, 80, vec![]);
        t.talk(1, npc);
        t.take();
        t.bypass(1, &format!("Script EkimusMouth {event}"));
        pages_to(&t.take(), 1).pop().unwrap()
    }

    #[test]
    fn the_halls_open_by_the_stage() {
        assert!(ask("1", "hos_enter").contains("32537-1"));
        assert!(ask("4", "hos_enter").contains("32537-2"));
        assert!(ask("1", "hoe_enter").contains("32537-3"));
        assert!(ask("4", "hoe_enter").contains("32537-4"));
    }
}
