//! Quest 172: New Horizons (Zenya in the Kamael village, Ragara in Gludio's Kamael boat area).
// Java: dist/game/data/scripts/quests/Q00172_NewHorizons/Q00172_NewHorizons.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ZENYA: i32 = 32140;
const RAGARA: i32 = 32163;

// Items
const SCROLL_OF_ESCAPE_GIRAN: i32 = 7559;
const MARK_OF_TRAVELER: i32 = 7570;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00172_NewHorizons`.
pub struct Q00172NewHorizons;

impl Script for Q00172NewHorizons {
    fn name(&self) -> &'static str {
        "Q00172_NewHorizons"
    }

    fn path(&self) -> &'static str {
        "quests/Q00172_NewHorizons"
    }

    fn id(&self) -> i32 {
        172
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ZENYA]);
        s.add_talk_id(&[ZENYA, RAGARA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        match event {
            "32140-04.htm" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "32163-02.html" => {
                ctx.give_items(player, SCROLL_OF_ESCAPE_GIRAN, 1);
                ctx.give_items(player, MARK_OF_TRAVELER, 1);
                qs.exit_quest(ctx, false, true);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ZENYA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::KAMAEL {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "32140-01.htm"
                        } else {
                            "32140-02.htm"
                        }
                    } else {
                        "32140-03.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "32140-05.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            RAGARA => {
                if qs.is_started(ctx) {
                    htmltext = "32163-01.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00172_NewHorizons";

    const NPCS: &str = r#"
        <npc id="32140" type="Folk" level="70" name="Zenya"/>
        <npc id="32163" type="Folk" level="70" name="Ragara"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7559" type="EtcItem" name="Scroll of Escape: Giran"><set name="is_stackable" val="true"/></item>
        <item id="7570" type="EtcItem" name="Mark of Traveler"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00172NewHorizons);
        let pages = Pages::of(&Q00172NewHorizons)
            .page("32140-01.htm", &["32140-04.htm"])
            .plain(&["32140-02.htm", "32140-03.htm", "32140-04.htm", "32140-05.html"])
            .page("32163-01.html", &["32163-02.html"])
            .plain(&["32163-02.html"])
            .npcs(&[ZENYA, RAGARA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let zenya = t.spawn(ZENYA, 20);
        let ragara = t.spawn(RAGARA, 40);
        (t, zenya, ragara)
    }

    #[test]
    fn only_a_kamael_of_level_3_is_offered_the_quest() {
        let (mut t, zenya, _) = setup();
        t.enter_as(1, 3, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, zenya).contains("32140-03.htm"));
        t.enter_as(2, 2, race::KAMAEL, 123, vec![]);
        assert!(t.quest_page(2, zenya).contains("32140-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_then_the_boat_gives_the_scroll_and_the_mark() {
        let (mut t, zenya, ragara) = setup();
        t.enter_as(1, 3, race::KAMAEL, 123, vec![]);
        assert!(t.quest_page(1, zenya).contains("32140-01.htm"));
        assert!(t.event(1, NAME, "32140-04.htm").contains("32140-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, zenya).contains("32140-05.html"));

        assert!(t.quest_page(1, ragara).contains("32163-01.html"));
        assert!(t.event(1, NAME, "32163-02.html").contains("32163-02.html"));
        assert_eq!(
            (t.count(1, SCROLL_OF_ESCAPE_GIRAN), t.count(1, MARK_OF_TRAVELER)),
            (1, 1)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, zenya).contains("already been completed"));
    }
}
