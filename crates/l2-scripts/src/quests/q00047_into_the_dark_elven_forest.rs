//! Quest 47: Into the Dark Elven Forest (Galladucci, Gentler, Sandra and Dustin along the order
//! chain; the scroll takes the player to the Dark Elven Village).
// Java: dist/game/data/scripts/quests/Q00047_IntoTheDarkElvenForest/Q00047_IntoTheDarkElvenForest.java
//
// Pattern: a delivery chain over several NPCs (`docs/design/scripts.md`, "Quest patterns"): the
// start NPC hands out the first order, every NPC swaps it for the next item, and the end is one-
// time. Q00045 to Q00049 are the same code with the scroll and the prerequisite quest changed.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Npcs
const GALLADUCCI: i32 = 30097;
const GENTLER: i32 = 30094;
const SANDRA: i32 = 30090;
const DUSTIN: i32 = 30116;

// Items
const MARK_OF_TRAVELER: i32 = 7570;
const GALLADUCCIS_ORDER_1: i32 = 7563;
const GALLADUCCIS_ORDER_2: i32 = 7564;
const GALLADUCCIS_ORDER_3: i32 = 7565;
const PURIFIED_MAGIC_NECKLACE: i32 = 7566;
const GEMSTONE_POWDER: i32 = 7567;
const MAGIC_SWORD_HILT: i32 = 7568;

// Misc
const MIN_LEVEL: i32 = 3;

// Reward
const SCROLL_OF_ESCAPE_DARK_ELF_VILLAGE: i32 = 7556;

/// Java `NPC_ITEMS`: each NPC with the cond it is talked to at and the order it checks.
const NPC_ITEMS: [(i32, i32, i32); 3] = [
    (GENTLER, 1, GALLADUCCIS_ORDER_1),
    (SANDRA, 3, GALLADUCCIS_ORDER_2),
    (DUSTIN, 5, GALLADUCCIS_ORDER_3),
];

/// Java `Q00047_IntoTheDarkElvenForest`.
pub struct Q00047IntoTheDarkElvenForest;

impl Script for Q00047IntoTheDarkElvenForest {
    fn name(&self) -> &'static str {
        "Q00047_IntoTheDarkElvenForest"
    }

    fn path(&self) -> &'static str {
        "quests/Q00047_IntoTheDarkElvenForest"
    }

    fn id(&self) -> i32 {
        47
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GALLADUCCI]);
        s.add_talk_id(&[GALLADUCCI]);
        s.add_talk_id(&[GENTLER, SANDRA, DUSTIN]);
        s.register_quest_items(&[
            GALLADUCCIS_ORDER_1,
            GALLADUCCIS_ORDER_2,
            GALLADUCCIS_ORDER_3,
            PURIFIED_MAGIC_NECKLACE,
            GEMSTONE_POWDER,
            MAGIC_SWORD_HILT,
        ]);
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

        let mut htmltext = None;
        match event {
            "30097-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, GALLADUCCIS_ORDER_1, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30094-02.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, GALLADUCCIS_ORDER_1) {
                    ctx.take_items(player, GALLADUCCIS_ORDER_1, 1);
                    ctx.give_items(player, MAGIC_SWORD_HILT, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30094-03.html".to_owned());
                }
            }
            "30097-07.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, MAGIC_SWORD_HILT) {
                    ctx.take_items(player, MAGIC_SWORD_HILT, 1);
                    ctx.give_items(player, GALLADUCCIS_ORDER_2, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30097-08.html".to_owned());
                }
            }
            "30090-02.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, GALLADUCCIS_ORDER_2) {
                    ctx.take_items(player, GALLADUCCIS_ORDER_2, 1);
                    ctx.give_items(player, GEMSTONE_POWDER, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30090-03.html".to_owned());
                }
            }
            "30097-11.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, GEMSTONE_POWDER) {
                    ctx.take_items(player, GEMSTONE_POWDER, 1);
                    ctx.give_items(player, GALLADUCCIS_ORDER_3, 1);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30097-12.html".to_owned());
                }
            }
            "30116-02.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, GALLADUCCIS_ORDER_3) {
                    ctx.take_items(player, GALLADUCCIS_ORDER_3, 1);
                    ctx.give_items(player, PURIFIED_MAGIC_NECKLACE, 1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30116-03.html".to_owned());
                }
            }
            "30097-15.html" => {
                if qs.is_cond(ctx, 6) && ctx.has_quest_items(player, PURIFIED_MAGIC_NECKLACE) {
                    ctx.give_items(player, SCROLL_OF_ESCAPE_DARK_ELF_VILLAGE, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30097-16.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);

        match npc.id {
            GALLADUCCI => match qs.state(ctx) {
                CREATED => {
                    if ctx.player_level(talker) < MIN_LEVEL {
                        htmltext = "30097-03.html".to_owned();
                    } else {
                        // `talker.getQuestState("Q00008_AnAdventureBegins")`: another quest's
                        // state, read only.
                        let previous_done = ctx
                            .player_quest_state(talker, "Q00008_AnAdventureBegins")
                            .is_some_and(|previous| previous.is_completed());
                        if previous_done && ctx.has_quest_items(talker, MARK_OF_TRAVELER) {
                            htmltext = "30097-01.htm".to_owned();
                        } else {
                            htmltext = "30097-02.html".to_owned();
                        }
                    }
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30097-05.html".to_owned(),
                    2 => {
                        if ctx.has_quest_items(talker, MAGIC_SWORD_HILT) {
                            htmltext = "30097-06.html".to_owned();
                        }
                    }
                    3 => htmltext = "30097-09.html".to_owned(),
                    4 => {
                        if ctx.has_quest_items(talker, GEMSTONE_POWDER) {
                            htmltext = "30097-10.html".to_owned();
                        }
                    }
                    5 => htmltext = "30097-13.html".to_owned(),
                    6 => {
                        if ctx.has_quest_items(talker, PURIFIED_MAGIC_NECKLACE) {
                            htmltext = "30097-14.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
                _ => {}
            },
            GENTLER | SANDRA | DUSTIN => {
                if qs.is_started(ctx)
                    && let Some(&(_, cond, item_id)) = NPC_ITEMS.iter().find(|(id, _, _)| *id == npc.id)
                {
                    if qs.is_cond(ctx, cond) {
                        if ctx.has_quest_items(talker, item_id) {
                            htmltext = format!("{}-01.html", npc.id);
                        }
                    } else if qs.is_cond(ctx, cond + 1) {
                        htmltext = format!("{}-04.html", npc.id);
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
    use l2_world::{Script, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, rows};

    const NAME: &str = "Q00047_IntoTheDarkElvenForest";
    const PREVIOUS: &str = "Q00008_AnAdventureBegins";

    const NPCS: &str = r#"
        <npc id="30097" type="Folk" level="70" name="Galladucci"/>
        <npc id="30094" type="Folk" level="70" name="Gentler"/>
        <npc id="30090" type="Folk" level="70" name="Sandra"/>
        <npc id="30116" type="Folk" level="70" name="Dustin"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7556" type="EtcItem" name="Scroll of Escape: Dark Elven Village"/>
        <item id="7563" type="EtcItem" name="Galladucci's Order 1"><set name="is_questitem" val="true"/></item>
        <item id="7564" type="EtcItem" name="Galladucci's Order 2"><set name="is_questitem" val="true"/></item>
        <item id="7565" type="EtcItem" name="Galladucci's Order 3"><set name="is_questitem" val="true"/></item>
        <item id="7566" type="EtcItem" name="Purified Magic Necklace"><set name="is_questitem" val="true"/></item>
        <item id="7567" type="EtcItem" name="Gemstone Powder"><set name="is_questitem" val="true"/></item>
        <item id="7568" type="EtcItem" name="Magic Sword Hilt"><set name="is_questitem" val="true"/></item>
        <item id="7570" type="EtcItem" name="Mark of Traveler"/>"#;

    /// The quest this one checks at the start (`Q00008_AnAdventureBegins`), as a stand-in
    /// script: its state is set by the test.
    struct PreviousQuest;

    impl Script for PreviousQuest {
        fn name(&self) -> &'static str {
            PREVIOUS
        }
        fn path(&self) -> &'static str {
            "quests/Q00008_AnAdventureBegins"
        }
        fn id(&self) -> i32 {
            8
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        galladucci: Oid,
        gentler: Oid,
        sandra: Oid,
        dustin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(PreviousQuest);
        registry.add(Q00047IntoTheDarkElvenForest);
        let pages = Pages::of(&Q00047IntoTheDarkElvenForest)
            .page("30097-01.htm", &["30097-04.htm"])
            .page("30094-01.html", &["30094-02.html"])
            .page("30097-06.html", &["30097-07.html"])
            .page("30090-01.html", &["30090-02.html"])
            .page("30097-10.html", &["30097-11.html"])
            .page("30116-01.html", &["30116-02.html"])
            .page("30097-14.html", &["30097-15.html"])
            .plain(&[
                "30097-02.html",
                "30097-03.html",
                "30097-04.htm",
                "30097-05.html",
                "30097-07.html",
                "30097-08.html",
                "30097-09.html",
                "30097-11.html",
                "30097-12.html",
                "30097-13.html",
                "30097-15.html",
                "30097-16.html",
                "30094-02.html",
                "30094-03.html",
                "30094-04.html",
                "30090-02.html",
                "30090-03.html",
                "30090-04.html",
                "30116-02.html",
                "30116-03.html",
                "30116-04.html",
            ])
            .npcs(&[GALLADUCCI, GENTLER, SANDRA, DUSTIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            galladucci: t.spawn(GALLADUCCI, 20),
            gentler: t.spawn(GENTLER, 40),
            sandra: t.spawn(SANDRA, 60),
            dustin: t.spawn(DUSTIN, 80),
        };
        (t, npcs)
    }

    /// The player finished `Q00008_AnAdventureBegins` (the stand-in's state is set to completed).
    fn finish_previous_quest(t: &mut ScriptTest) {
        t.world.run_as_script(PREVIOUS, |ctx| {
            if let Some(qs) = ctx.quest_state_of(PREVIOUS, Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    /// Sets the player's cond directly (for the branches reached without the whole chain).
    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, cond, false);
            }
        });
        t.take_writes();
    }

    /// Calls a script event directly (for the refusals whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00047IntoTheDarkElvenForest.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    #[test]
    fn a_character_under_level_three_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 2, vec![]);
        finish_previous_quest(&mut t);
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_previous_quest_and_its_mark_are_needed() {
        let (mut t, npcs) = setup();
        t.enter(1, 3, vec![]);
        // Without the previous quest: the page for it.
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-02.html"));
        // Finished, but without the mark of the traveler: the same page.
        finish_previous_quest(&mut t);
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_order_chain_then_the_scroll() {
        let (mut t, npcs) = setup();
        t.enter(1, 3, vec![item(1001, MARK_OF_TRAVELER, 1)]);
        finish_previous_quest(&mut t);
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-01.htm"));
        assert!(t.event(1, NAME, "30097-04.htm").contains("30097-04.htm"));
        assert_eq!(t.count(1, GALLADUCCIS_ORDER_1), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-05.html"));

        // Gentler takes the first order and gives the sword hilt: cond 2.
        assert!(t.quest_page(1, npcs.gentler).contains("30094-01.html"));
        assert!(t.event(1, NAME, "30094-02.html").contains("30094-02.html"));
        assert_eq!(
            (t.count(1, GALLADUCCIS_ORDER_1), t.count(1, MAGIC_SWORD_HILT)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.gentler).contains("30094-04.html"));

        // Galladucci takes the hilt and gives the second order: cond 3.
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-06.html"));
        assert!(t.event(1, NAME, "30097-07.html").contains("30097-07.html"));
        assert_eq!(
            (t.count(1, MAGIC_SWORD_HILT), t.count(1, GALLADUCCIS_ORDER_2)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Sandra takes the second order and gives the gemstone powder: cond 4.
        assert!(t.quest_page(1, npcs.sandra).contains("30090-01.html"));
        assert!(t.event(1, NAME, "30090-02.html").contains("30090-02.html"));
        assert_eq!(
            (t.count(1, GALLADUCCIS_ORDER_2), t.count(1, GEMSTONE_POWDER)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Galladucci takes the powder and gives the third order: cond 5.
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-10.html"));
        assert!(t.event(1, NAME, "30097-11.html").contains("30097-11.html"));
        assert_eq!(
            (t.count(1, GEMSTONE_POWDER), t.count(1, GALLADUCCIS_ORDER_3)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Dustin takes the third order and gives the necklace: cond 6.
        assert!(t.quest_page(1, npcs.dustin).contains("30116-01.html"));
        assert!(t.event(1, NAME, "30116-02.html").contains("30116-02.html"));
        assert_eq!(
            (
                t.count(1, GALLADUCCIS_ORDER_3),
                t.count(1, PURIFIED_MAGIC_NECKLACE)
            ),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Galladucci takes the necklace and gives the scroll: the end.
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-14.html"));
        assert!(t.event(1, NAME, "30097-15.html").contains("30097-15.html"));
        assert_eq!(t.count(1, SCROLL_OF_ESCAPE_DARK_ELF_VILLAGE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.galladucci)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_order_givers_send_the_player_back_without_the_item() {
        let (mut t, npcs) = setup();
        t.enter(1, 3, vec![item(1001, MARK_OF_TRAVELER, 1)]);
        finish_previous_quest(&mut t);
        t.quest_page(1, npcs.galladucci);
        t.event(1, NAME, "30097-04.htm");
        t.take_writes();

        // Gentler at cond 2 (the order taken already): the next page; Sandra at cond 1 has
        // nothing for the player.
        set_cond(&mut t, 2);
        assert!(t.quest_page(1, npcs.gentler).contains("30094-04.html"));
        assert!(t.quest_page(1, npcs.sandra).contains("not on a quest"));

        // Without the first order at Gentler's: the refusal page.
        set_cond(&mut t, 1);
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), GALLADUCCIS_ORDER_1, -1);
        });
        assert_eq!(direct(&mut t, "30094-02.html").as_deref(), Some("30094-03.html"));
        assert!(t.take_writes().is_empty());

        // The scroll without the necklace: the refusal page, nothing given.
        set_cond(&mut t, 6);
        assert_eq!(direct(&mut t, "30097-15.html").as_deref(), Some("30097-16.html"));
        assert_eq!(t.count(1, SCROLL_OF_ESCAPE_DARK_ELF_VILLAGE), 0);
        assert!(names_to(&t.take(), 1).iter().all(|n| n != "PLAY_SOUND"));
    }
}
