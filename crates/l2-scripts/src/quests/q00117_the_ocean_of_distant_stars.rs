//! Quest 117: The Ocean of Distant Stars (Abey, the ghosts of the railroad engineers, Obi, the box).
// Java: dist/game/data/scripts/quests/Q00117_TheOceanOfDistantStars/Q00117_TheOceanOfDistantStars.java
//
// Pattern: a long chain of memo states (`setMemoState`, Java's `isMemoState(n)`) that go with
// the conds, a kill with a drop chance for a random started party member (`on_kill`, the drop
// goes to the killer with a limit of one, `giveItemRandomly` seven-argument overload), and
// `onEvent` that answers only the events of the current step (`null` otherwise).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const OBI: i32 = 32052;
const ABEY: i32 = 32053;
const GHOST_OF_A_RAILROAD_ENGINEER: i32 = 32054;
const GHOST_OF_AN_ANCIENT_RAILROAD_ENGINEER: i32 = 32055;
const BOX: i32 = 32076;

// Items
const ENGRAVED_HAMMER: i32 = 8488;
const BOOK_OF_GREY_STAR: i32 = 8495;

// Misc
const MIN_LEVEL: i32 = 39;

// Monsters
const BANDIT_WARRIOR: i32 = 22023;
const BANDIT_INSPECTOR: i32 = 22024;

/// Java `MONSTER_DROP_CHANCES`.
fn monster_drop_chance(npc_id: i32) -> f64 {
    match npc_id {
        BANDIT_WARRIOR => 0.179,
        BANDIT_INSPECTOR => 0.1,
        _ => 0.0,
    }
}

/// Java `Q00117_TheOceanOfDistantStars`.
pub struct Q00117TheOceanOfDistantStars;

impl Script for Q00117TheOceanOfDistantStars {
    fn name(&self) -> &'static str {
        "Q00117_TheOceanOfDistantStars"
    }

    fn path(&self) -> &'static str {
        "quests/Q00117_TheOceanOfDistantStars"
    }

    fn id(&self) -> i32 {
        117
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ABEY]);
        s.add_talk_id(&[
            ABEY,
            GHOST_OF_A_RAILROAD_ENGINEER,
            GHOST_OF_AN_ANCIENT_RAILROAD_ENGINEER,
            BOX,
            OBI,
        ]);
        s.add_kill_id(&[BANDIT_WARRIOR, BANDIT_INSPECTOR]);
        s.register_quest_items(&[ENGRAVED_HAMMER, BOOK_OF_GREY_STAR]);
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
            "32053-02.htm" => {
                qs.set_memo_state(ctx, 1);
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32053-06.html" => {
                if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32053-09.html" => {
                if qs.is_memo_state(ctx, 5) && ctx.has_quest_items(player, ENGRAVED_HAMMER) {
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32054-02.html" => {
                if qs.is_memo_state(ctx, 9) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32054-03.html" => {
                if qs.is_memo_state(ctx, 9) {
                    ctx.give_adena(player, 17_647, true);
                    ctx.add_exp_and_sp(player, 107_387, 7369);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32055-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32055-05.html" => {
                if qs.is_memo_state(ctx, 8) {
                    if ctx.has_quest_items(player, ENGRAVED_HAMMER) {
                        qs.set_memo_state(ctx, 9);
                        qs.set_cond(ctx, 10, true);
                        ctx.take_items(player, ENGRAVED_HAMMER, -1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("32055-06.html".to_owned());
                    }
                }
            }
            "32076-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    ctx.give_items(player, ENGRAVED_HAMMER, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32052-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32052-05.html" => {
                if qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 7);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32052-07.html" => {
                if qs.is_memo_state(ctx, 7) && ctx.has_quest_items(player, BOOK_OF_GREY_STAR) {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 9, true);
                    ctx.take_items(player, BOOK_OF_GREY_STAR, -1);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, 7, 3, Some(npc)) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }
        // Java `giveItemRandomly(killer, npc, BOOK, 1, 1, chance, true)`: the limit is 1, and
        // the item goes to the killer while the cond moves on the member's state.
        if ctx.give_item_randomly(
            killer,
            Some(npc),
            BOOK_OF_GREY_STAR,
            1,
            1,
            1,
            monster_drop_chance(npc.id),
            true,
        ) {
            qs.set_cond(ctx, 8, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => {
                if npc.id == ABEY {
                    htmltext = ctx.get_already_completed_msg(player);
                }
            }
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32053-01.htm"
                } else {
                    "32053-03.htm"
                }
                .to_owned();
            }
            STARTED => match npc.id {
                ABEY => match qs.get_memo_state(ctx) {
                    1 => htmltext = "32053-04.html".to_owned(),
                    3 => htmltext = "32053-05.html".to_owned(),
                    4 => htmltext = "32053-07.html".to_owned(),
                    5 => {
                        if ctx.has_quest_items(player, ENGRAVED_HAMMER) {
                            htmltext = "32053-08.html".to_owned();
                        }
                    }
                    6 => htmltext = "32053-10.html".to_owned(),
                    _ => {}
                },
                GHOST_OF_A_RAILROAD_ENGINEER => {
                    if qs.is_memo_state(ctx, 9) {
                        htmltext = "32054-01.html".to_owned();
                    }
                }
                GHOST_OF_AN_ANCIENT_RAILROAD_ENGINEER => match qs.get_memo_state(ctx) {
                    1 => htmltext = "32055-01.html".to_owned(),
                    2 => htmltext = "32055-03.html".to_owned(),
                    8 => htmltext = "32055-04.html".to_owned(),
                    9 => htmltext = "32055-07.html".to_owned(),
                    _ => {}
                },
                BOX => {
                    if qs.is_memo_state(ctx, 4) {
                        htmltext = "32076-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 5) {
                        htmltext = "32076-03.html".to_owned();
                    }
                }
                OBI => match qs.get_memo_state(ctx) {
                    2 => htmltext = "32052-01.html".to_owned(),
                    3 => htmltext = "32052-03.html".to_owned(),
                    6 => htmltext = "32052-04.html".to_owned(),
                    7 => {
                        htmltext = if ctx.has_quest_items(player, BOOK_OF_GREY_STAR) {
                            "32052-06.html"
                        } else {
                            "32052-08.html"
                        }
                        .to_owned();
                    }
                    8 => htmltext = "32052-09.html".to_owned(),
                    _ => {}
                },
                _ => {}
            },
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32052" type="Folk" level="40" name="Obi"/>
        <npc id="32053" type="Folk" level="40" name="Abey"/>
        <npc id="32054" type="Folk" level="40" name="Ghost of a Railroad Engineer"/>
        <npc id="32055" type="Folk" level="40" name="Ghost of an Ancient Railroad Engineer"/>
        <npc id="32076" type="Folk" level="40" name="Box"/>
        <npc id="22023" type="Monster" level="40" name="Bandit Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8488" type="EtcItem" name="Engraved Hammer"><set name="is_questitem" val="true"/></item>
        <item id="8495" type="EtcItem" name="Book of Grey Star"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00117_TheOceanOfDistantStars";

    struct Npcs {
        obi: Oid,
        abey: Oid,
        engineer: Oid,
        ancient: Oid,
        boxed: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00117TheOceanOfDistantStars);
        let pages = Pages::of(&Q00117TheOceanOfDistantStars)
            .page("32053-01.htm", &["32053-02.htm"])
            .plain(&[
                "32053-02.htm",
                "32053-03.htm",
                "32053-04.html",
                "32053-07.html",
                "32053-10.html",
            ])
            .page("32053-05.html", &["32053-06.html"])
            .plain(&["32053-06.html"])
            .page("32053-08.html", &["32053-09.html"])
            .plain(&["32053-09.html"])
            .page("32055-01.html", &["32055-02.html"])
            .plain(&["32055-02.html", "32055-03.html", "32055-06.html", "32055-07.html"])
            .page("32055-04.html", &["32055-05.html"])
            .plain(&["32055-05.html"])
            .page("32052-01.html", &["32052-02.html"])
            .plain(&["32052-02.html", "32052-03.html", "32052-08.html", "32052-09.html"])
            .page("32052-04.html", &["32052-05.html"])
            .plain(&["32052-05.html"])
            .page("32052-06.html", &["32052-07.html"])
            .plain(&["32052-07.html"])
            .page("32076-01.html", &["32076-02.html"])
            .plain(&["32076-02.html", "32076-03.html"])
            .page("32054-01.html", &["32054-02.html"])
            .page("32054-02.html", &["32054-03.html"])
            .plain(&["32054-03.html"])
            .npcs(&[
                OBI,
                ABEY,
                GHOST_OF_A_RAILROAD_ENGINEER,
                GHOST_OF_AN_ANCIENT_RAILROAD_ENGINEER,
                BOX,
            ])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            obi: t.spawn(OBI, 20),
            abey: t.spawn(ABEY, 40),
            engineer: t.spawn(GHOST_OF_A_RAILROAD_ENGINEER, 60),
            ancient: t.spawn(GHOST_OF_AN_ANCIENT_RAILROAD_ENGINEER, 80),
            boxed: t.spawn(BOX, 100),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_39_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 38, vec![]);
        assert!(t.quest_page(1, npcs.abey).contains("32053-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_engineers_and_obi_have_nothing_to_say_before_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 39, vec![]);
        assert!(t.quest_page(1, npcs.obi).contains("noquest"));
        assert!(t.quest_page(1, npcs.engineer).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_abey_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 39, vec![]);
        assert!(t.quest_page(1, npcs.abey).contains("32053-01.htm"));

        // Accept: `setMemoState(1)`, `startQuest()`.
        assert!(t.event(1, NAME, "32053-02.htm").contains("32053-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                insert(NAME, "memoState", "1"),
                insert(NAME, "cond", "1"),
                insert(NAME, "<state>", "Started"),
            ]
        );

        // The ancient engineer: memo 2, cond 2.
        assert!(t.quest_page(1, npcs.ancient).contains("32055-01.html"));
        assert!(t.event(1, NAME, "32055-02.html").contains("32055-02.html"));
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "2"), update(NAME, "cond", "2")]
        );

        // Obi: memo 3, cond 3.
        assert!(t.quest_page(1, npcs.obi).contains("32052-01.html"));
        assert!(t.event(1, NAME, "32052-02.html").contains("32052-02.html"));
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "3"), update(NAME, "cond", "3")]
        );

        // Abey: memo 4, cond 4.
        assert!(t.quest_page(1, npcs.abey).contains("32053-05.html"));
        assert!(t.event(1, NAME, "32053-06.html").contains("32053-06.html"));
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "4"), update(NAME, "cond", "4")]
        );

        // The box gives the hammer: memo 5, cond 5.
        assert!(t.quest_page(1, npcs.boxed).contains("32076-01.html"));
        assert!(t.event(1, NAME, "32076-02.html").contains("32076-02.html"));
        assert_eq!(t.count(1, ENGRAVED_HAMMER), 1);
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "5"), update(NAME, "cond", "5")]
        );

        // Abey takes the hammer's word: memo 6, cond 6.
        assert!(t.quest_page(1, npcs.abey).contains("32053-08.html"));
        assert!(t.event(1, NAME, "32053-09.html").contains("32053-09.html"));
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "6"), update(NAME, "cond", "6")]
        );

        // Obi sends the player to the bandits: memo 7, cond 7.
        assert!(t.quest_page(1, npcs.obi).contains("32052-04.html"));
        assert!(t.event(1, NAME, "32052-05.html").contains("32052-05.html"));
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "7"), update(NAME, "cond", "7")]
        );

        // The book drops from a bandit (the first started member gets it, cond 8).
        assert_eq!(t.world.quest_list(Oid(1)).quests.len(), 1);
        let mut kills = 0;
        while t.count(1, BOOK_OF_GREY_STAR) == 0 && kills < 400 {
            let bandit = t.spawn(BANDIT_WARRIOR, 20);
            t.kill(1, bandit);
            kills += 1;
        }
        assert_eq!(t.count(1, BOOK_OF_GREY_STAR), 1, "no book after {kills} kills");
        assert_eq!(t.take_writes(), [update(NAME, "cond", "8")]);

        // Obi takes the book: memo 8, cond 9.
        assert!(t.quest_page(1, npcs.obi).contains("32052-06.html"));
        assert!(t.event(1, NAME, "32052-07.html").contains("32052-07.html"));
        assert_eq!(t.count(1, BOOK_OF_GREY_STAR), 0);
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "8"), update(NAME, "cond", "9")]
        );

        // The ancient engineer takes the hammer: memo 9, cond 10.
        assert!(t.quest_page(1, npcs.ancient).contains("32055-04.html"));
        assert!(t.event(1, NAME, "32055-05.html").contains("32055-05.html"));
        assert_eq!(t.count(1, ENGRAVED_HAMMER), 0);
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "9"), update(NAME, "cond", "10")]
        );

        // The railroad engineer's reward: adena, exp and sp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.engineer).contains("32054-01.html"));
        assert!(t.event(1, NAME, "32054-02.html").contains("32054-02.html"));
        assert!(t.event(1, NAME, "32054-03.html").contains("32054-03.html"));
        assert_eq!(t.count(1, 57), 17_647);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(
            t.quest_page(1, npcs.abey)
                .contains("This quest has already been completed.")
        );
    }
}
