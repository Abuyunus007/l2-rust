//! Quest 7: A Trip Begins (Mirabel in Talking Island sends an elf to Ariel and Asterios).
// Java: dist/game/data/scripts/quests/Q00007_ATripBegins/Q00007_ATripBegins.java
//
// Pattern: a delivery chain (`docs/design/scripts.md`, "Quest patterns"): Mirabel starts it,
// Ariel hands out the recommendation (cond 2), Asterios takes it (cond 3), and Mirabel pays the
// reward items. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MIRABEL: i32 = 30146;
const ARIEL: i32 = 30148;
const ASTERIOS: i32 = 30154;

// Items
const ARIELS_RECOMMENDATION: i32 = 7572;
const SCROLL_OF_ESCAPE_GIRAN: i32 = 7559;
const MARK_OF_TRAVELER: i32 = 7570;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00007_ATripBegins`.
pub struct Q00007ATripBegins;

impl Script for Q00007ATripBegins {
    fn name(&self) -> &'static str {
        "Q00007_ATripBegins"
    }

    fn path(&self) -> &'static str {
        "quests/Q00007_ATripBegins"
    }

    fn id(&self) -> i32 {
        7
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MIRABEL]);
        s.add_talk_id(&[MIRABEL, ARIEL, ASTERIOS]);
        s.register_quest_items(&[ARIELS_RECOMMENDATION]);
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
            "30146-03.htm" => qs.start_quest(ctx),
            "30146-06.html" => {
                ctx.give_items(player, SCROLL_OF_ESCAPE_GIRAN, 1);
                ctx.give_items(player, MARK_OF_TRAVELER, 1);
                qs.exit_quest(ctx, false, true);
            }
            "30148-02.html" => {
                qs.set_cond(ctx, 2, true);
                ctx.give_items(player, ARIELS_RECOMMENDATION, 1);
            }
            "30154-02.html" => {
                if !ctx.has_quest_items(player, ARIELS_RECOMMENDATION) {
                    return Some("30154-03.html".to_owned());
                }

                ctx.take_items(player, ARIELS_RECOMMENDATION, -1);
                qs.set_cond(ctx, 3, true);
            }
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            MIRABEL => match qs.state(ctx) {
                CREATED => {
                    htmltext =
                        if ctx.player_race(player) == race::ELF && ctx.player_level(player) >= MIN_LEVEL {
                            "30146-01.htm"
                        } else {
                            "30146-02.html"
                        }
                        .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30146-04.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        htmltext = "30146-05.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ARIEL => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30148-01.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "30148-03.html".to_owned();
                    }
                }
            }
            ASTERIOS => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "30154-01.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        htmltext = "30154-04.html".to_owned();
                    }
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

    const NAME: &str = "Q00007_ATripBegins";

    const NPCS: &str = r#"
        <npc id="30146" type="Folk" level="70" name="Mirabel"/>
        <npc id="30148" type="Folk" level="70" name="Ariel"/>
        <npc id="30154" type="Folk" level="70" name="Asterios"/>"#;

    const ITEMS: &str = r#"
        <item id="7572" type="EtcItem" name="Ariel's Recommendation"><set name="is_questitem" val="true"/></item>
        <item id="7559" type="EtcItem" name="Scroll of Escape: Giran"/>
        <item id="7570" type="EtcItem" name="Mark of Traveler"/>"#;

    struct Npcs {
        mirabel: Oid,
        ariel: Oid,
        asterios: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00007ATripBegins);
        let pages = Pages::of(&Q00007ATripBegins)
            .page("30146-01.htm", &["30146-03.htm"])
            .page("30146-05.html", &["30146-06.html"])
            .page("30148-01.html", &["30148-02.html"])
            .page("30154-01.html", &["30154-02.html"])
            .plain(&[
                "30146-02.html",
                "30146-03.htm",
                "30146-04.html",
                "30146-06.html",
                "30148-02.html",
                "30148-03.html",
                "30154-02.html",
                "30154-03.html",
                "30154-04.html",
            ])
            .npcs(&[MIRABEL, ARIEL, ASTERIOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            mirabel: t.spawn(MIRABEL, 20),
            ariel: t.spawn(ARIEL, 40),
            asterios: t.spawn(ASTERIOS, 60),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_elf_of_level_three_may_start() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 5, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.mirabel).contains("30146-02.html"));
        t.enter_as(2, 2, race::ELF, 0, vec![]);
        assert!(t.quest_page(2, npcs.mirabel).contains("30146-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ariel_and_asterios_then_the_scroll_and_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.mirabel).contains("30146-01.htm"));
        assert!(t.event(1, NAME, "30146-03.htm").contains("30146-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Ariel gives the recommendation (cond 2).
        assert!(t.quest_page(1, npcs.ariel).contains("30148-01.html"));
        assert!(t.event(1, NAME, "30148-02.html").contains("30148-02.html"));
        assert_eq!(t.count(1, ARIELS_RECOMMENDATION), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.ariel).contains("30148-03.html"));

        // Asterios takes it (cond 3).
        assert!(t.quest_page(1, npcs.asterios).contains("30154-01.html"));
        assert!(t.event(1, NAME, "30154-02.html").contains("30154-02.html"));
        assert_eq!(t.count(1, ARIELS_RECOMMENDATION), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.asterios).contains("30154-04.html"));

        // Mirabel: the scroll and the mark, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.mirabel).contains("30146-05.html"));
        assert!(t.event(1, NAME, "30146-06.html").contains("30146-06.html"));
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
            t.quest_page(1, npcs.mirabel)
                .contains("This quest has already been completed.")
        );
    }
}
