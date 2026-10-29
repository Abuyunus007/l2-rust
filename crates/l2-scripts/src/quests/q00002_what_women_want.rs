//! Quest 2: What Women Want (Arujien in Talking Island sends a letter chain to Mirabel, Herbiel and Greenis).
// Java: dist/game/data/scripts/quests/Q00002_WhatWomenWant/Q00002_WhatWomenWant.java
//
// Pattern: a delivery chain (`docs/design/scripts.md`, "Quest patterns"): the start NPC hands out
// the first letter, each NPC swaps one letter for the next, and the quest ends at the start NPC
// (a reward branch on the Newbie Guide, and one on the `30223-09.html` link).

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ARUJIEN: i32 = 30223;
const MIRABEL: i32 = 30146;
const HERBIEL: i32 = 30150;
const GREENIS: i32 = 30157;

// Items
const ARUJIENS_LETTER1: i32 = 1092;
const ARUJIENS_LETTER2: i32 = 1093;
const ARUJIENS_LETTER3: i32 = 1094;
const POETRY_BOOK: i32 = 689;
const GREENIS_LETTER: i32 = 693;
const EARRING: i32 = 113;

// Misc
const MIN_LEVEL: i32 = 2;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00002_WhatWomenWant`.
pub struct Q00002WhatWomenWant;

impl Script for Q00002WhatWomenWant {
    fn name(&self) -> &'static str {
        "Q00002_WhatWomenWant"
    }

    fn path(&self) -> &'static str {
        "quests/Q00002_WhatWomenWant"
    }

    fn id(&self) -> i32 {
        2
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ARUJIEN]);
        s.add_talk_id(&[ARUJIEN, MIRABEL, HERBIEL, GREENIS]);
        s.register_quest_items(&[
            ARUJIENS_LETTER1,
            ARUJIENS_LETTER2,
            ARUJIENS_LETTER3,
            POETRY_BOOK,
            GREENIS_LETTER,
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

        match event {
            "30223-04.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, ARUJIENS_LETTER1, 1);
            }
            "30223-08.html" => {
                ctx.take_items(player, ARUJIENS_LETTER3, -1);
                ctx.give_items(player, POETRY_BOOK, 1);
                qs.set_cond(ctx, 4, true);
            }
            "30223-09.html" => {
                ctx.give_adena(player, 450, true);
                ctx.add_exp_and_sp(player, 4254, 335);
                ctx.give_adena(player, 1850, true);
                qs.exit_quest(ctx, false, true);
            }
            "30223-03.html" => {}
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ARUJIEN => match qs.state(ctx) {
                CREATED => {
                    let race = ctx.player_race(player);
                    htmltext = if race != race::ELF && race != race::HUMAN {
                        "30223-00.htm"
                    } else if ctx.player_level(player) >= MIN_LEVEL {
                        "30223-02.htm"
                    } else {
                        "30223-01.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30223-05.html".to_owned(),
                    2 => htmltext = "30223-06.html".to_owned(),
                    3 => htmltext = "30223-07.html".to_owned(),
                    4 => htmltext = "30223-10.html".to_owned(),
                    5 => {
                        // Newbie Guide (only once `ai/others/NewbieGuide` is ported; Java skips
                        // the block the same way when the script is missing).
                        if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                            if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, 1);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::DeliveryDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            } else if ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 10 != 1 {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 1);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::DeliveryDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            }
                        }

                        ctx.give_items(player, EARRING, 1);
                        ctx.add_exp_and_sp(player, 4254, 335);
                        ctx.give_adena(player, 1850, true);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30223-11.html".to_owned();
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            MIRABEL => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        qs.set_cond(ctx, 2, true);
                        ctx.take_items(player, ARUJIENS_LETTER1, -1);
                        ctx.give_items(player, ARUJIENS_LETTER2, 1);
                        htmltext = "30146-01.html".to_owned();
                    } else {
                        htmltext = "30146-02.html".to_owned();
                    }
                }
            }
            HERBIEL => {
                if qs.is_started(ctx) && qs.get_cond(ctx) > 1 {
                    if qs.is_cond(ctx, 2) {
                        qs.set_cond(ctx, 3, true);
                        ctx.take_items(player, ARUJIENS_LETTER2, -1);
                        ctx.give_items(player, ARUJIENS_LETTER3, 1);
                        htmltext = "30150-01.html".to_owned();
                    } else {
                        htmltext = "30150-02.html".to_owned();
                    }
                }
            }
            GREENIS => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 4) {
                        qs.set_cond(ctx, 5, true);
                        ctx.take_items(player, POETRY_BOOK, -1);
                        ctx.give_items(player, GREENIS_LETTER, 1);
                        htmltext = "30157-02.html".to_owned();
                    } else if qs.is_cond(ctx, 5) {
                        htmltext = "30157-03.html".to_owned();
                    } else {
                        htmltext = "30157-01.html".to_owned();
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

    const NAME: &str = "Q00002_WhatWomenWant";

    const NPCS: &str = r#"
        <npc id="30223" type="Folk" level="70" name="Arujien"/>
        <npc id="30146" type="Folk" level="70" name="Mirabel"/>
        <npc id="30150" type="Folk" level="70" name="Herbiel"/>
        <npc id="30157" type="Folk" level="70" name="Greenis"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1092" type="EtcItem" name="Arujien's Letter 1"><set name="is_questitem" val="true"/></item>
        <item id="1093" type="EtcItem" name="Arujien's Letter 2"><set name="is_questitem" val="true"/></item>
        <item id="1094" type="EtcItem" name="Arujien's Letter 3"><set name="is_questitem" val="true"/></item>
        <item id="689" type="EtcItem" name="Poetry Book"><set name="is_questitem" val="true"/></item>
        <item id="693" type="EtcItem" name="Greenis's Letter"><set name="is_questitem" val="true"/></item>
        <item id="113" type="Armor" name="Earring of Knowledge"><set name="bodypart" val="rear;lear"/></item>"#;

    struct Npcs {
        arujien: Oid,
        mirabel: Oid,
        herbiel: Oid,
        greenis: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00002WhatWomenWant);
        let pages = Pages::of(&Q00002WhatWomenWant)
            .page("30223-02.htm", &["30223-03.html"])
            .page("30223-03.html", &["30223-04.htm"])
            .page("30223-07.html", &["30223-08.html", "30223-09.html"])
            .plain(&[
                "30223-00.htm",
                "30223-01.html",
                "30223-04.htm",
                "30223-05.html",
                "30223-06.html",
                "30223-08.html",
                "30223-09.html",
                "30223-10.html",
                "30223-11.html",
                "30146-01.html",
                "30146-02.html",
                "30150-01.html",
                "30150-02.html",
                "30157-01.html",
                "30157-02.html",
                "30157-03.html",
            ])
            .npcs(&[ARUJIEN, MIRABEL, HERBIEL, GREENIS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            arujien: t.spawn(ARUJIEN, 20),
            mirabel: t.spawn(MIRABEL, 40),
            herbiel: t.spawn(HERBIEL, 60),
            greenis: t.spawn(GREENIS, 80),
        };
        (t, npcs)
    }

    #[test]
    fn the_race_and_the_level_gate_the_start() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 10, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.arujien).contains("30223-00.htm"));
        t.enter_as(2, 1, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(2, npcs.arujien).contains("30223-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letters_from_arujien_to_greenis_and_the_earring() {
        let (mut t, npcs) = setup();
        t.enter(1, 2, vec![]);
        assert!(t.quest_page(1, npcs.arujien).contains("30223-02.htm"));
        assert!(t.event(1, NAME, "30223-03.html").contains("30223-03.html"));
        assert!(t.event(1, NAME, "30223-04.htm").contains("30223-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, ARUJIENS_LETTER1), 1);

        // Mirabel takes the first letter and gives the second (cond 2).
        assert!(t.quest_page(1, npcs.mirabel).contains("30146-01.html"));
        assert_eq!(
            (t.count(1, ARUJIENS_LETTER1), t.count(1, ARUJIENS_LETTER2)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.mirabel).contains("30146-02.html"));

        // Herbiel: the third letter (cond 3).
        assert!(t.quest_page(1, npcs.herbiel).contains("30150-01.html"));
        assert_eq!(t.count(1, ARUJIENS_LETTER3), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Arujien's page with the two ends: 08 gives the poetry book (cond 4).
        assert!(t.quest_page(1, npcs.arujien).contains("30223-07.html"));
        assert!(t.event(1, NAME, "30223-08.html").contains("30223-08.html"));
        assert_eq!((t.count(1, ARUJIENS_LETTER3), t.count(1, POETRY_BOOK)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Greenis takes the book and gives his letter (cond 5).
        assert!(t.quest_page(1, npcs.greenis).contains("30157-02.html"));
        assert_eq!((t.count(1, POETRY_BOOK), t.count(1, GREENIS_LETTER)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.greenis).contains("30157-03.html"));

        // Arujien: the earring and `exitQuest(false, true)` (no Newbie Guide here).
        assert!(t.quest_page(1, npcs.arujien).contains("30223-11.html"));
        assert_eq!(t.count(1, EARRING), 1);
        assert_eq!(t.count(1, 57), 1850);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(
            t.quest_page(1, npcs.arujien)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_short_way_out_at_cond_3_pays_the_adena_and_ends() {
        let (mut t, npcs) = setup();
        t.enter(1, 2, vec![]);
        t.quest_page(1, npcs.arujien);
        t.event(1, NAME, "30223-03.html");
        t.event(1, NAME, "30223-04.htm");
        t.quest_page(1, npcs.mirabel);
        t.quest_page(1, npcs.herbiel);
        t.take_writes();
        assert!(t.quest_page(1, npcs.arujien).contains("30223-07.html"));
        assert!(t.event(1, NAME, "30223-09.html").contains("30223-09.html"));
        assert_eq!(t.count(1, 57), 2300);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
