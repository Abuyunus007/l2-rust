//! Budenka, the caravan guide of Hellbound's native village.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Budenka/Budenka.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BUDENKA: i32 = 32294;

// Items
const STANDART_CERT: i32 = 9851;
const PREMIUM_CERT: i32 = 9852;

/// Java `ai.areas.Hellbound.AI.NPC.Budenka.Budenka`.
pub struct Budenka;

impl Script for Budenka {
    fn name(&self) -> &'static str {
        "Budenka"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Budenka"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BUDENKA]);
        s.add_first_talk_id(&[BUDENKA]);
        s.add_talk_id(&[BUDENKA]);
    }

    fn on_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "Budenka-02.html" | "Budenka-03.html" | "Budenka-04.html" | "Budenka-05.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // `hasQuestItems(player, a, b)` wants both.
        let htmltext = if ctx.has_all_quest_items(player, &[STANDART_CERT, PREMIUM_CERT]) {
            "Budenka-07.html"
        } else if ctx.has_quest_items(player, STANDART_CERT) {
            "Budenka-06.html"
        } else {
            "Budenka-01.html"
        };

        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::last_page;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32294" type="Folk" level="70" name="Budenka"/>"#;
    const ITEMS: &str = r#"
        <item id="9851" type="EtcItem" name="Standard Certificate"/>
        <item id="9852" type="EtcItem" name="Premium Certificate"/>"#;

    fn setup(items: Vec<l2_world::ItemData>) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Budenka);
        let pages = Pages::of(&Budenka).plain(&[
            "Budenka-01.html",
            "Budenka-02.html",
            "Budenka-06.html",
            "Budenka-07.html",
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npc = t.spawn(BUDENKA, 20);
        t.enter(1, 80, items);
        (t, npc)
    }

    #[test]
    fn the_first_page_follows_the_certificates() {
        for (items, page) in [
            (vec![], "Budenka-01.html"),
            (vec![item(100, STANDART_CERT, 1)], "Budenka-06.html"),
            (
                vec![item(100, STANDART_CERT, 1), item(101, PREMIUM_CERT, 1)],
                "Budenka-07.html",
            ),
        ] {
            let (mut t, npc) = setup(items);
            t.talk(1, npc);
            assert!(last_page(&t.take(), 1).contains(page), "{page}");
        }
    }

    #[test]
    fn only_the_story_pages_are_answered() {
        let (mut t, _npc) = setup(vec![]);
        t.world.run_as_script("Budenka", |ctx| {
            let me = Budenka;
            assert_eq!(
                me.on_event(ctx, "Budenka-02.html", None, None).as_deref(),
                Some("Budenka-02.html")
            );
            assert_eq!(me.on_event(ctx, "Budenka-06.html", None, None), None);
        });
    }
}
