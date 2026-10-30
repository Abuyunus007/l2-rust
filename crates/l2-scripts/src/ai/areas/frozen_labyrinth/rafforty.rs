//! Rafforty swaps a Pretty Necklace and a Bottle for a Blessed Necklace.
// Java: dist/game/data/scripts/ai/areas/FrozenLabyrinth/Rafforty/Rafforty.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RAFFORTY: i32 = 32020;
// Items
const NECKLACE: i32 = 16025;
const BLESSED_NECKLACE: i32 = 16026;
const BOTTLE: i32 = 16027;

/// Java `ai.areas.FrozenLabyrinth.Rafforty.Rafforty`.
pub struct Rafforty;

impl Script for Rafforty {
    fn name(&self) -> &'static str {
        "Rafforty"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FrozenLabyrinth/Rafforty"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAFFORTY]);
        s.add_first_talk_id(&[RAFFORTY]);
        s.add_talk_id(&[RAFFORTY]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): every other event is echoed as the page to show.
        let mut htmltext = event.to_owned();
        let player = player?;
        match event {
            "32020-01.html" => {
                if !ctx.has_quest_items(player, NECKLACE) {
                    htmltext = "32020-02.html".to_owned();
                }
            }
            "32020-04.html" => {
                if !ctx.has_quest_items(player, BOTTLE) {
                    htmltext = "32020-05.html".to_owned();
                }
            }
            "32020-07.html" => {
                if !ctx.has_all_quest_items(player, &[BOTTLE, NECKLACE]) {
                    return Some("32020-08.html".to_owned());
                }
                ctx.take_items(player, NECKLACE, 1);
                ctx.take_items(player, BOTTLE, 1);
                ctx.give_items(player, BLESSED_NECKLACE, 1);
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="32020" type="Folk" level="70" name="Rafforty"/>"#;
    const ITEMS: &str = r#"<item id="16025" type="EtcItem" name="Necklace"/>
        <item id="16026" type="EtcItem" name="Blessed Necklace"/>
        <item id="16027" type="EtcItem" name="Bottle"/>"#;

    #[test]
    fn the_swap_needs_both_items() {
        let mut registry = ScriptRegistry::new();
        registry.add(Rafforty);
        let pages = Pages::of(&Rafforty)
            .page("32020.html", &["32020-01.html", "32020-04.html", "32020-07.html"])
            .page("32020-01.html", &["32020-04.html", "32020-07.html"])
            .page("32020-04.html", &["32020-07.html"])
            .page("32020-02.html", &["32020-04.html", "32020-07.html"])
            .page("32020-05.html", &["32020-07.html"])
            .plain(&["32020-07.html", "32020-08.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rafforty = t.spawn(RAFFORTY, 20);
        t.enter(1, 80, vec![item(500, NECKLACE, 1)]);
        t.talk(1, rafforty);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32020.html"));
        assert!(t.event(1, "Rafforty", "32020-01.html").contains("32020-01.html"));
        assert!(t.event(1, "Rafforty", "32020-04.html").contains("32020-05.html"));
        // Without the bottle the swap is refused.
        assert!(t.event(1, "Rafforty", "32020-07.html").contains("32020-08.html"));
        assert_eq!(t.count(1, NECKLACE), 1);

        t.enter(2, 80, vec![item(501, NECKLACE, 1), item(502, BOTTLE, 1)]);
        t.talk(2, rafforty);
        t.take();
        assert!(t.event(2, "Rafforty", "32020-01.html").contains("32020-01.html"));
        assert!(t.event(2, "Rafforty", "32020-04.html").contains("32020-04.html"));
        assert!(t.event(2, "Rafforty", "32020-07.html").contains("32020-07.html"));
        assert_eq!(t.count(2, BLESSED_NECKLACE), 1);
        assert_eq!(t.count(2, NECKLACE) + t.count(2, BOTTLE), 0);
    }
}
