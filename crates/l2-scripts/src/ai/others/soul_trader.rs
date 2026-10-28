//! Asyatei, the Soul Trader of Seed of Infinity: his page depends on the Mark of Keucereus.
// Java: dist/game/data/scripts/ai/others/SoulTrader/SoulTrader.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ASYATEI: i32 = 32546;
// Item
const MARK_OF_KEUCEREUS_STAGE_2: i32 = 13692;

/// Java `ai.others.SoulTrader.SoulTrader`.
pub struct SoulTrader;

impl Script for SoulTrader {
    fn name(&self) -> &'static str {
        "SoulTrader"
    }

    fn path(&self) -> &'static str {
        "ai/others/SoulTrader"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ASYATEI]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        Some(
            if ctx.get_quest_items_count(player, MARK_OF_KEUCEREUS_STAGE_2) == 0 {
                "32546-01.html"
            } else {
                "32546-02.html"
            }
            .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to};

    const NPCS: &str = r#"<npc id="32546" type="Folk" level="70" name="Asyatei"/>"#;
    const ITEMS: &str = r#"<item id="13692" type="EtcItem" name="Mark of Keucereus - Stage 2"/>"#;
    const DIR: &str = "data/scripts/ai/others/SoulTrader";

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(SoulTrader);
        let pages = [
            (
                format!("{DIR}/32546-01.html"),
                "<html><body>no mark</body></html>",
            ),
            (
                format!("{DIR}/32546-02.html"),
                "<html><body>with mark</body></html>",
            ),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npc = t.spawn(ASYATEI, 20);
        (t, npc)
    }

    #[test]
    fn the_page_depends_on_the_mark() {
        let (mut t, npc) = setup();
        t.enter(1, 80, vec![]);
        t.take();
        t.talk(1, npc);
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"NPC_HTML_MESSAGE".to_owned()));
        assert!(pages_to(&out, 1).pop().unwrap().contains("no mark"));

        t.enter(2, 80, vec![item(5000, MARK_OF_KEUCEREUS_STAGE_2, 1)]);
        t.take();
        t.talk(2, npc);
        assert!(pages_to(&t.take(), 2).pop().unwrap().contains("with mark"));
    }
}
