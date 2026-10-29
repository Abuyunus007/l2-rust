//! Quest 8: An Adventure Begins (Jasmine in Talking Island sends a dark elf to Roselyn and Harne).
// Java: dist/game/data/scripts/quests/Q00008_AnAdventureBegins/Q00008_AnAdventureBegins.java
//
// Pattern: a delivery chain (`docs/design/scripts.md`, "Quest patterns"): Jasmine starts it,
// Roselyn hands out the note (cond 2), Harne takes it (cond 3), and Jasmine pays the reward
// items. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const JASMINE: i32 = 30134;
const ROSELYN: i32 = 30355;
const HARNE: i32 = 30144;

// Items
const ROSELYNS_NOTE: i32 = 7573;
const SCROLL_OF_ESCAPE_GIRAN: i32 = 7559;
const MARK_OF_TRAVELER: i32 = 7570;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00008_AnAdventureBegins`.
pub struct Q00008AnAdventureBegins;

impl Script for Q00008AnAdventureBegins {
    fn name(&self) -> &'static str {
        "Q00008_AnAdventureBegins"
    }

    fn path(&self) -> &'static str {
        "quests/Q00008_AnAdventureBegins"
    }

    fn id(&self) -> i32 {
        8
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JASMINE]);
        s.add_talk_id(&[JASMINE, ROSELYN, HARNE]);
        s.register_quest_items(&[ROSELYNS_NOTE]);
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
            "30134-03.htm" => qs.start_quest(ctx),
            "30134-06.html" => {
                ctx.give_items(player, SCROLL_OF_ESCAPE_GIRAN, 1);
                ctx.give_items(player, MARK_OF_TRAVELER, 1);
                qs.exit_quest(ctx, false, true);
            }
            "30355-02.html" => {
                qs.set_cond(ctx, 2, true);
                ctx.give_items(player, ROSELYNS_NOTE, 1);
            }
            "30144-02.html" => {
                if !ctx.has_quest_items(player, ROSELYNS_NOTE) {
                    return Some("30144-03.html".to_owned());
                }

                ctx.take_items(player, ROSELYNS_NOTE, -1);
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
            JASMINE => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::DARK_ELF
                        && ctx.player_level(player) >= MIN_LEVEL
                    {
                        "30134-02.htm"
                    } else {
                        "30134-01.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30134-04.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        htmltext = "30134-05.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ROSELYN => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30355-01.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "30355-03.html".to_owned();
                    }
                }
            }
            HARNE => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "30144-01.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        htmltext = "30144-04.html".to_owned();
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

    const NAME: &str = "Q00008_AnAdventureBegins";

    const NPCS: &str = r#"
        <npc id="30134" type="Folk" level="70" name="Jasmine"/>
        <npc id="30355" type="Folk" level="70" name="Roselyn"/>
        <npc id="30144" type="Folk" level="70" name="Harne"/>"#;

    const ITEMS: &str = r#"
        <item id="7573" type="EtcItem" name="Roselyn's Note"><set name="is_questitem" val="true"/></item>
        <item id="7559" type="EtcItem" name="Scroll of Escape: Giran"/>
        <item id="7570" type="EtcItem" name="Mark of Traveler"/>"#;

    struct Npcs {
        jasmine: Oid,
        roselyn: Oid,
        harne: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00008AnAdventureBegins);
        let pages = Pages::of(&Q00008AnAdventureBegins)
            .page("30134-02.htm", &["30134-03.htm"])
            .page("30134-05.html", &["30134-06.html"])
            .page("30144-01.html", &["30144-02.html"])
            .page("30355-01.html", &["30355-02.html"])
            .plain(&[
                "30134-01.html",
                "30134-03.htm",
                "30134-04.html",
                "30134-06.html",
                "30144-02.html",
                "30144-03.html",
                "30144-04.html",
                "30355-02.html",
                "30355-03.html",
            ])
            .npcs(&[JASMINE, ROSELYN, HARNE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            jasmine: t.spawn(JASMINE, 20),
            roselyn: t.spawn(ROSELYN, 40),
            harne: t.spawn(HARNE, 60),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_dark_elf_of_level_three_may_start() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 5, race::ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.jasmine).contains("30134-01.html"));
        t.enter_as(2, 2, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(2, npcs.jasmine).contains("30134-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn roselyn_and_harne_then_the_scroll_and_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.jasmine).contains("30134-02.htm"));
        assert!(t.event(1, NAME, "30134-03.htm").contains("30134-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Roselyn gives the note (cond 2).
        assert!(t.quest_page(1, npcs.roselyn).contains("30355-01.html"));
        assert!(t.event(1, NAME, "30355-02.html").contains("30355-02.html"));
        assert_eq!(t.count(1, ROSELYNS_NOTE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.roselyn).contains("30355-03.html"));

        // Harne takes the note (cond 3).
        assert!(t.quest_page(1, npcs.harne).contains("30144-01.html"));
        assert!(t.event(1, NAME, "30144-02.html").contains("30144-02.html"));
        assert_eq!(t.count(1, ROSELYNS_NOTE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.harne).contains("30144-04.html"));

        // Jasmine: the scroll and the mark, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.jasmine).contains("30134-05.html"));
        assert!(t.event(1, NAME, "30134-06.html").contains("30134-06.html"));
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
            t.quest_page(1, npcs.jasmine)
                .contains("This quest has already been completed.")
        );
    }
}
