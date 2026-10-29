//! Quest 9: Into the City of Humans (Petukai in the Orc Village sends an orc to Tanapi and Tamil).
// Java: dist/game/data/scripts/quests/Q00009_IntoTheCityOfHumans/Q00009_IntoTheCityOfHumans.java
//
// Pattern: a short delivery chain with no quest items (`docs/design/scripts.md`, "Quest
// patterns"): Petukai starts it, Tanapi moves it to cond 2 with a page, and Tamil pays the
// reward items. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PETUKAI: i32 = 30583;
const TANAPI: i32 = 30571;
const TAMIL: i32 = 30576;

// Items
const SCROLL_OF_ESCAPE_GIRAN: i32 = 7559;
const MARK_OF_TRAVELER: i32 = 7570;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00009_IntoTheCityOfHumans`.
pub struct Q00009IntoTheCityOfHumans;

impl Script for Q00009IntoTheCityOfHumans {
    fn name(&self) -> &'static str {
        "Q00009_IntoTheCityOfHumans"
    }

    fn path(&self) -> &'static str {
        "quests/Q00009_IntoTheCityOfHumans"
    }

    fn id(&self) -> i32 {
        9
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PETUKAI]);
        s.add_talk_id(&[PETUKAI, TANAPI, TAMIL]);
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
            "30583-04.htm" => qs.start_quest(ctx),
            "30576-02.html" => {
                ctx.give_items(player, MARK_OF_TRAVELER, 1);
                ctx.give_items(player, SCROLL_OF_ESCAPE_GIRAN, 1);
                qs.exit_quest(ctx, false, true);
            }
            "30571-02.html" => qs.set_cond(ctx, 2, true),
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            PETUKAI => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.player_race(player) == race::ORC {
                            "30583-01.htm"
                        } else {
                            "30583-02.html"
                        }
                    } else {
                        "30583-03.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30583-05.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            TANAPI => {
                if qs.is_started(ctx) {
                    htmltext = if qs.is_cond(ctx, 1) {
                        "30571-01.html"
                    } else {
                        "30571-03.html"
                    }
                    .to_owned();
                }
            }
            TAMIL => {
                if qs.is_started(ctx) && qs.is_cond(ctx, 2) {
                    htmltext = "30576-01.html".to_owned();
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

    const NAME: &str = "Q00009_IntoTheCityOfHumans";

    const NPCS: &str = r#"
        <npc id="30583" type="Folk" level="70" name="Petukai"/>
        <npc id="30571" type="Folk" level="70" name="Tanapi"/>
        <npc id="30576" type="Folk" level="70" name="Tamil"/>"#;

    const ITEMS: &str = r#"
        <item id="7559" type="EtcItem" name="Scroll of Escape: Giran"/>
        <item id="7570" type="EtcItem" name="Mark of Traveler"/>"#;

    struct Npcs {
        petukai: Oid,
        tanapi: Oid,
        tamil: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00009IntoTheCityOfHumans);
        let pages = Pages::of(&Q00009IntoTheCityOfHumans)
            .page("30583-01.htm", &["30583-04.htm"])
            .page("30571-01.html", &["30571-02.html"])
            .page("30576-01.html", &["30576-02.html"])
            .plain(&[
                "30583-02.html",
                "30583-03.html",
                "30583-04.htm",
                "30583-05.html",
                "30571-02.html",
                "30571-03.html",
                "30576-02.html",
            ])
            .npcs(&[PETUKAI, TANAPI, TAMIL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            petukai: t.spawn(PETUKAI, 20),
            tanapi: t.spawn(TANAPI, 40),
            tamil: t.spawn(TAMIL, 60),
        };
        (t, npcs)
    }

    #[test]
    fn the_start_page_depends_on_level_and_race() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 2, race::ORC, 0, vec![]);
        assert!(t.quest_page(1, npcs.petukai).contains("30583-03.html"));
        t.enter_as(2, 3, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(2, npcs.petukai).contains("30583-02.html"));
        t.enter_as(3, 3, race::ORC, 0, vec![]);
        assert!(t.quest_page(3, npcs.petukai).contains("30583-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn tanapi_and_tamil_then_the_scroll_and_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::ORC, 0, vec![]);
        assert!(t.quest_page(1, npcs.petukai).contains("30583-01.htm"));
        assert!(t.event(1, NAME, "30583-04.htm").contains("30583-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.petukai).contains("30583-05.html"));

        // Tanapi moves the quest to cond 2 (no item).
        assert!(t.quest_page(1, npcs.tanapi).contains("30571-01.html"));
        assert!(t.event(1, NAME, "30571-02.html").contains("30571-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.tanapi).contains("30571-03.html"));

        // Tamil: the scroll and the mark, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.tamil).contains("30576-01.html"));
        assert!(t.event(1, NAME, "30576-02.html").contains("30576-02.html"));
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
        assert!(
            t.quest_page(1, npcs.petukai)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn tamil_says_nothing_before_cond_two() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::ORC, 0, vec![]);
        t.quest_page(1, npcs.petukai);
        t.event(1, NAME, "30583-04.htm");
        t.take_writes();
        assert!(!t.quest_page(1, npcs.tamil).contains("30576-01.html"));
        assert!(t.take_writes().is_empty());
    }
}
