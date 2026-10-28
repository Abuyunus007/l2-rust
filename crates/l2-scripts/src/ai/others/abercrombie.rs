//! Abercrombie, the Golden Ram mercenary officer: a first-talk NPC script.
// Java: dist/game/data/scripts/ai/others/Abercrombie/Abercrombie.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ABERCROMBIE: i32 = 31555;

// Items
const GOLDEN_RAM_BADGE_RECRUIT: i32 = 7246;
const GOLDEN_RAM_BADGE_SOLDIER: i32 = 7247;

/// Java `ai.others.Abercrombie.Abercrombie`.
pub struct Abercrombie;

impl Script for Abercrombie {
    fn name(&self) -> &'static str {
        "Abercrombie"
    }

    fn path(&self) -> &'static str {
        "ai/others/Abercrombie"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ABERCROMBIE]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let htmltext = if ctx.has_quest_items(player, GOLDEN_RAM_BADGE_SOLDIER) {
            "31555-07.html"
        } else if ctx.has_quest_items(player, GOLDEN_RAM_BADGE_RECRUIT) {
            "31555-01.html"
        } else {
            "31555-09.html"
        };
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="31555" type="Folk" level="70" name="Abercrombie"/>"#;
    const ITEMS: &str = r#"<item id="7246" type="EtcItem" name="Recruit"/><item id="7247" type="EtcItem" name="Soldier"/>"#;
    const DIR: &str = "data/scripts/ai/others/Abercrombie";

    #[test]
    fn the_page_follows_the_badge() {
        let mut registry = ScriptRegistry::new();
        registry.add(Abercrombie);
        let pages: Vec<(String, String)> = ["31555-01", "31555-07", "31555-09"]
            .iter()
            .map(|n| (format!("{DIR}/{n}.html"), format!("<html><body>{n}</body></html>")))
            .collect();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npc = t.spawn(ABERCROMBIE, 20);
        t.enter(1, 60, vec![]);
        t.enter(2, 60, vec![item(900, GOLDEN_RAM_BADGE_RECRUIT, 1)]);
        t.enter(
            3,
            60,
            vec![item(901, GOLDEN_RAM_BADGE_RECRUIT, 1), item(902, GOLDEN_RAM_BADGE_SOLDIER, 1)],
        );
        t.take();
        for (player, page) in [(1, "31555-09"), (2, "31555-01"), (3, "31555-07")] {
            t.talk(player, npc);
            assert!(pages_to(&t.take(), player).pop().unwrap().contains(page), "{page}");
        }
    }
}
