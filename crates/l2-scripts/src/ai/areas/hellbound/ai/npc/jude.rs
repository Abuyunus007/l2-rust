//! Jude, who trades the treasure sacks of the natives for the Ring of Wind Mastery.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Jude/Jude.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const JUDE: i32 = 32356;
const NATIVE_TREASURE: i32 = 9684;
const RING_OF_WIND_MASTERY: i32 = 9677;

/// Java `ai.areas.Hellbound.AI.NPC.Jude.Jude`.
pub struct Jude;

impl Script for Jude {
    fn name(&self) -> &'static str {
        "Jude"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Jude"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[JUDE]);
        s.add_start_npc(&[JUDE]);
        s.add_talk_id(&[JUDE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if event.eq_ignore_ascii_case("TreasureSacks") {
            if engine::level(ctx) == 3 && ctx.get_quest_items_count(player, NATIVE_TREASURE) >= 40 {
                ctx.take_items(player, NATIVE_TREASURE, 40);
                ctx.give_items(player, RING_OF_WIND_MASTERY, 1);
                return Some("32356-02.htm".to_owned());
            }

            return Some("32356-02a.htm".to_owned());
        }

        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(
            match engine::level(ctx) {
                0..=2 => "32356-01.htm",
                3 | 4 => "32356-01c.htm",
                5 => "32356-01a.htm",
                _ => "32356-01b.htm",
            }
            .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::{last_page, set_level};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32356" type="Folk" level="70" name="Jude"/>"#;
    const ITEMS: &str = r#"
        <item id="9677" type="EtcItem" name="Ring of Wind Mastery"/>
        <item id="9684" type="EtcItem" name="Treasure"><set name="is_stackable" val="true"/></item>"#;

    #[test]
    fn forty_sacks_buy_the_ring_at_level_three() {
        let mut registry = ScriptRegistry::new();
        registry.add(Jude);
        let pages = Pages::of(&Jude)
            .page("32356-01c.htm", &["TreasureSacks"])
            .plain(&["32356-01.htm", "32356-01a.htm", "32356-02.htm", "32356-02a.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npc = t.spawn(JUDE, 20);
        t.enter(1, 80, vec![item(100, NATIVE_TREASURE, 40)]);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32356-01.htm"));
        set_level(&mut t, "Jude", 3);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32356-01c.htm"));
        assert!(t.event(1, "Jude", "TreasureSacks").contains("32356-02.htm"));
        assert_eq!(t.count(1, NATIVE_TREASURE), 0);
        assert_eq!(t.count(1, RING_OF_WIND_MASTERY), 1);
        // No sacks left.
        t.talk(1, npc);
        t.take();
        assert!(t.event(1, "Jude", "TreasureSacks").contains("32356-02a.htm"));
        set_level(&mut t, "Jude", 5);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32356-01a.htm"));
    }
}
