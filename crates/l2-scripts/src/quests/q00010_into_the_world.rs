//! Quest 10: Into the World (Balanki in Talking Island sends a dwarf to Reed and Gerald).
// Java: dist/game/data/scripts/quests/Q00010_IntoTheWorld/Q00010_IntoTheWorld.java
//
// Pattern: a chain with a necklace hand-over (`docs/design/scripts.md`, "Quest patterns"): Reed
// gives the necklace (cond 2), Gerald takes it (cond 3), Reed moves the quest on (cond 4), and
// Balanki pays the reward items. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const REED: i32 = 30520;
const BALANKI: i32 = 30533;
const GERALD: i32 = 30650;

// Items
const VERY_EXPENSIVE_NECKLACE: i32 = 7574;
const SCROLL_OF_ESCAPE_GIRAN: i32 = 7559;
const MARK_OF_TRAVELER: i32 = 7570;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00010_IntoTheWorld`.
pub struct Q00010IntoTheWorld;

impl Script for Q00010IntoTheWorld {
    fn name(&self) -> &'static str {
        "Q00010_IntoTheWorld"
    }

    fn path(&self) -> &'static str {
        "quests/Q00010_IntoTheWorld"
    }

    fn id(&self) -> i32 {
        10
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BALANKI]);
        s.add_talk_id(&[BALANKI, REED, GERALD]);
        s.register_quest_items(&[VERY_EXPENSIVE_NECKLACE]);
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
            "30533-03.htm" => qs.start_quest(ctx),
            "30533-06.html" => {
                ctx.give_items(player, MARK_OF_TRAVELER, 1);
                ctx.give_items(player, SCROLL_OF_ESCAPE_GIRAN, 1);
                qs.exit_quest(ctx, false, true);
            }
            "30520-02.html" => {
                ctx.give_items(player, VERY_EXPENSIVE_NECKLACE, 1);
                qs.set_cond(ctx, 2, true);
            }
            "30520-05.html" => qs.set_cond(ctx, 4, true),
            "30650-02.html" => {
                if !ctx.has_quest_items(player, VERY_EXPENSIVE_NECKLACE) {
                    return Some("30650-03.html".to_owned());
                }

                ctx.take_items(player, VERY_EXPENSIVE_NECKLACE, -1);
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
            BALANKI => match qs.state(ctx) {
                CREATED => {
                    htmltext =
                        if ctx.player_level(player) >= MIN_LEVEL && ctx.player_race(player) == race::DWARF {
                            "30533-01.htm"
                        } else {
                            "30533-02.html"
                        }
                        .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30533-04.html".to_owned();
                    } else if qs.is_cond(ctx, 4) {
                        htmltext = "30533-05.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            REED => {
                if qs.is_started(ctx) {
                    // The page of each cond; any other cond shows the default page.
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30520-01.html".to_owned(),
                        2 => htmltext = "30520-03.html".to_owned(),
                        3 => htmltext = "30520-04.html".to_owned(),
                        4 => htmltext = "30520-06.html".to_owned(),
                        _ => {}
                    }
                }
            }
            GERALD => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "30650-01.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        htmltext = "30650-04.html".to_owned();
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

    const NAME: &str = "Q00010_IntoTheWorld";

    const NPCS: &str = r#"
        <npc id="30520" type="Folk" level="70" name="Reed"/>
        <npc id="30533" type="Folk" level="70" name="Balanki"/>
        <npc id="30650" type="Folk" level="70" name="Gerald"/>"#;

    const ITEMS: &str = r#"
        <item id="7574" type="EtcItem" name="Very Expensive Necklace"><set name="is_questitem" val="true"/></item>
        <item id="7559" type="EtcItem" name="Scroll of Escape: Giran"/>
        <item id="7570" type="EtcItem" name="Mark of Traveler"/>"#;

    struct Npcs {
        reed: Oid,
        balanki: Oid,
        gerald: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00010IntoTheWorld);
        let pages = Pages::of(&Q00010IntoTheWorld)
            .page("30533-01.htm", &["30533-03.htm"])
            .page("30533-05.html", &["30533-06.html"])
            .page("30520-01.html", &["30520-02.html"])
            .page("30520-04.html", &["30520-05.html"])
            .page("30650-01.html", &["30650-02.html"])
            .plain(&[
                "30533-02.html",
                "30533-03.htm",
                "30533-04.html",
                "30533-06.html",
                "30520-02.html",
                "30520-03.html",
                "30520-05.html",
                "30520-06.html",
                "30650-02.html",
                "30650-03.html",
                "30650-04.html",
            ])
            .npcs(&[REED, BALANKI, GERALD]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            reed: t.spawn(REED, 20),
            balanki: t.spawn(BALANKI, 40),
            gerald: t.spawn(GERALD, 60),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_dwarf_of_level_three_may_start() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 5, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.balanki).contains("30533-02.html"));
        t.enter_as(2, 2, race::DWARF, 0, vec![]);
        assert!(t.quest_page(2, npcs.balanki).contains("30533-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_necklace_goes_from_reed_to_gerald_and_back_to_reed() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::DWARF, 0, vec![]);
        assert!(t.quest_page(1, npcs.balanki).contains("30533-01.htm"));
        assert!(t.event(1, NAME, "30533-03.htm").contains("30533-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Reed gives the necklace (cond 2).
        assert!(t.quest_page(1, npcs.reed).contains("30520-01.html"));
        assert!(t.event(1, NAME, "30520-02.html").contains("30520-02.html"));
        assert_eq!(t.count(1, VERY_EXPENSIVE_NECKLACE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.reed).contains("30520-03.html"));

        // Gerald takes it (cond 3).
        assert!(t.quest_page(1, npcs.gerald).contains("30650-01.html"));
        assert!(t.event(1, NAME, "30650-02.html").contains("30650-02.html"));
        assert_eq!(t.count(1, VERY_EXPENSIVE_NECKLACE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.gerald).contains("30650-04.html"));

        // Reed moves on (cond 4) and Balanki pays the reward.
        assert!(t.quest_page(1, npcs.reed).contains("30520-04.html"));
        assert!(t.event(1, NAME, "30520-05.html").contains("30520-05.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.reed).contains("30520-06.html"));
        assert!(t.quest_page(1, npcs.balanki).contains("30533-05.html"));
        assert!(t.event(1, NAME, "30533-06.html").contains("30533-06.html"));
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
            t.quest_page(1, npcs.balanki)
                .contains("This quest has already been completed.")
        );
    }
}
