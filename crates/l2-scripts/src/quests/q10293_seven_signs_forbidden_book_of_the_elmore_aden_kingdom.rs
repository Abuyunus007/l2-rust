//! Quest 10293: Seven Signs, Forbidden Book of the Elmore-Aden Kingdom (Elcadia, Sophia and the piles of books in the Library of Sages).
// Java: dist/game/data/scripts/quests/Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom/Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom.java
//
// Pattern: a delivery chain by condition (`docs/design/scripts.md`): the conditions 1 to 8 are
// the steps in Sophia's and Elcadia's rooms (the Library of Sages is another script), the
// Solinas' biography is the item of the piles of books (cond 6), and Elcadia pays the
// experience (one time, not for a sub-class). The first talk of Sophia (32863) answers from
// the condition, without a state of its own.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SOPHIA1: i32 = 32596;
const ELCADIA: i32 = 32784;
const ELCADIA_INSTANCE: i32 = 32785;
const PILE_OF_BOOKS1: i32 = 32809;
const PILE_OF_BOOKS2: i32 = 32810;
const PILE_OF_BOOKS3: i32 = 32811;
const PILE_OF_BOOKS4: i32 = 32812;
const PILE_OF_BOOKS5: i32 = 32813;
const SOPHIA2: i32 = 32861;
const SOPHIA3: i32 = 32863;

// Item
const SOLINAS_BIOGRAPHY: i32 = 17213;

// Misc
const MIN_LEVEL: i32 = 81;

/// Java `Q10292_SevenSignsGirlOfDoubt.class.getSimpleName()`.
const GIRL_OF_DOUBT: &str = "Q10292_SevenSignsGirlOfDoubt";

/// Java `Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom`.
pub struct Q10293SevenSignsForbiddenBookOfTheElmoreAdenKingdom;

impl Script for Q10293SevenSignsForbiddenBookOfTheElmoreAdenKingdom {
    fn name(&self) -> &'static str {
        "Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom"
    }

    fn path(&self) -> &'static str {
        "quests/Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom"
    }

    fn id(&self) -> i32 {
        10293
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[SOPHIA3]);
        s.add_start_npc(&[ELCADIA]);
        s.add_talk_id(&[
            ELCADIA,
            ELCADIA_INSTANCE,
            SOPHIA1,
            SOPHIA2,
            SOPHIA3,
            PILE_OF_BOOKS1,
            PILE_OF_BOOKS2,
            PILE_OF_BOOKS3,
            PILE_OF_BOOKS4,
            PILE_OF_BOOKS5,
        ]);
        s.register_quest_items(&[SOLINAS_BIOGRAPHY]);
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
            "32784-03.htm" | "32784-05.html" | "32861-13.html" | "32863-02.html" | "32863-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "32784-04.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32784-07.html" | "32784-08.html" => {
                if qs.is_cond(ctx, 8) {
                    htmltext = Some(event.to_owned());
                }
            }
            "REWARD" => {
                if ctx.is_sub_class_active(player) {
                    htmltext = Some("32784-10.html".to_owned());
                } else {
                    ctx.add_exp_and_sp(player, 15_000_000, 1_500_000);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some("32784-09.html".to_owned());
                }
            }
            "32785-02.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32785-07.html" => {
                if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32596-03.html" | "32596-04.html" => {
                let cond = qs.get_cond(ctx);
                if (1..8).contains(&cond) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32861-02.html" | "32861-03.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32861-04.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32861-07.html" => {
                if qs.is_cond(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32861-08.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32861-11.html" => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32809-02.html" => {
                if qs.is_cond(ctx, 6) {
                    qs.set_cond(ctx, 7, true);
                    ctx.give_items(player, SOLINAS_BIOGRAPHY, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32810-02.html" | "32811-02.html" | "32812-02.html" | "32813-02.html" => {
                if qs.is_cond(ctx, 6) {
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let cond = ctx.get_quest_state(player, true).map_or(0, |qs| qs.get_cond(ctx));
        let htmltext = if (1..8).contains(&cond) {
            "32863-01.html"
        } else {
            "32863-04.html"
        };
        Some(htmltext.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ELCADIA => {
                if qs.is_completed(ctx) {
                    htmltext = "32784-02.html".to_owned();
                } else if qs.is_created(ctx) {
                    let girl_done = ctx
                        .player_quest_state(player, GIRL_OF_DOUBT)
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && girl_done {
                        "32784-01.htm"
                    } else {
                        "32784-11.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "32784-06.html".to_owned();
                    } else if qs.is_cond(ctx, 8) {
                        htmltext = "32784-07.html".to_owned();
                    }
                }
            }
            ELCADIA_INSTANCE => match qs.get_cond(ctx) {
                1 => htmltext = "32785-01.html".to_owned(),
                2 => {
                    htmltext = "32785-04.html".to_owned();
                    qs.set_cond(ctx, 3, true);
                }
                3 => htmltext = "32785-05.html".to_owned(),
                4 => htmltext = "32785-06.html".to_owned(),
                5 => htmltext = "32785-08.html".to_owned(),
                6 => htmltext = "32785-09.html".to_owned(),
                7 => {
                    qs.set_cond(ctx, 8, true);
                    htmltext = "32785-11.html".to_owned();
                }
                8 => htmltext = "32785-12.html".to_owned(),
                _ => {}
            },
            SOPHIA1 => {
                if qs.is_started(ctx) {
                    let cond = qs.get_cond(ctx);
                    htmltext = if (1..8).contains(&cond) {
                        "32596-01.html"
                    } else {
                        "32596-05.html"
                    }
                    .to_owned();
                }
            }
            SOPHIA2 => match qs.get_cond(ctx) {
                1 => htmltext = "32861-01.html".to_owned(),
                2 => htmltext = "32861-05.html".to_owned(),
                3 => htmltext = "32861-06.html".to_owned(),
                4 => htmltext = "32861-09.html".to_owned(),
                5 => htmltext = "32861-10.html".to_owned(),
                6 | 7 => htmltext = "32861-12.html".to_owned(),
                8 => htmltext = "32861-14.html".to_owned(),
                _ => {}
            },
            PILE_OF_BOOKS1 => {
                if qs.is_cond(ctx, 6) {
                    htmltext = "32809-01.html".to_owned();
                }
            }
            PILE_OF_BOOKS2 => {
                if qs.is_cond(ctx, 6) {
                    htmltext = "32810-01.html".to_owned();
                }
            }
            PILE_OF_BOOKS3 => {
                if qs.is_cond(ctx, 6) {
                    htmltext = "32811-01.html".to_owned();
                }
            }
            PILE_OF_BOOKS4 => {
                if qs.is_cond(ctx, 6) {
                    htmltext = "32812-01.html".to_owned();
                }
            }
            PILE_OF_BOOKS5 => {
                if qs.is_cond(ctx, 6) {
                    htmltext = "32813-01.html".to_owned();
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
    use l2_world::{QuestLoad, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, pages_to, player_data, rows};

    const NAME: &str = "Q10293_SevenSignsForbiddenBookOfTheElmoreAdenKingdom";

    const NPCS: &str = r#"
        <npc id="32784" type="Folk" level="80" name="Elcadia"/>
        <npc id="32785" type="Folk" level="80" name="Elcadia"/>
        <npc id="32809" type="Folk" level="80" name="Pile of Books"/>
        <npc id="32861" type="Folk" level="80" name="Sophia"/>
        <npc id="32863" type="Folk" level="80" name="Sophia"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17213" type="EtcItem" name="Solinas' Biography"><set name="is_questitem" val="true"/></item>"#;

    /// Stands in for Q10292 (its completion is read through `player_quest_state` only).
    struct GirlOfDoubtStandIn;

    impl Script for GirlOfDoubtStandIn {
        fn name(&self) -> &'static str {
            GIRL_OF_DOUBT
        }

        fn path(&self) -> &'static str {
            "quests/Q10292_SevenSignsGirlOfDoubt"
        }

        fn id(&self) -> i32 {
            10292
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    struct Npcs {
        elcadia: Oid,
        instance: Oid,
        sophia: Oid,
        sophia3: Oid,
        book: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10293SevenSignsForbiddenBookOfTheElmoreAdenKingdom);
        registry.add(GirlOfDoubtStandIn);
        let pages = Pages::of(&Q10293SevenSignsForbiddenBookOfTheElmoreAdenKingdom)
            .page("32784-01.htm", &["32784-03.htm"])
            .page("32784-03.htm", &["32784-04.html", "32784-05.html"])
            .plain(&[
                "32784-02.html",
                "32784-04.html",
                "32784-05.html",
                "32784-06.html",
                "32784-09.html",
                "32784-10.html",
                "32784-11.htm",
            ])
            .page("32784-07.html", &["32784-08.html"])
            .page("32784-08.html", &["REWARD"])
            .page("32785-01.html", &["32785-02.html"])
            .plain(&["32785-02.html", "32785-04.html", "32785-05.html"])
            .page("32785-06.html", &["32785-07.html"])
            .plain(&[
                "32785-07.html",
                "32785-08.html",
                "32785-09.html",
                "32785-11.html",
                "32785-12.html",
            ])
            .page("32809-01.html", &["32809-02.html"])
            .plain(&["32809-02.html"])
            .page("32861-01.html", &["32861-02.html"])
            .page("32861-02.html", &["32861-03.html"])
            .page("32861-03.html", &["32861-04.html"])
            .plain(&["32861-04.html", "32861-05.html"])
            .page("32861-06.html", &["32861-07.html"])
            .page("32861-07.html", &["32861-08.html"])
            .plain(&["32861-08.html", "32861-09.html"])
            .page("32861-10.html", &["32861-11.html"])
            .plain(&["32861-11.html", "32861-12.html", "32861-13.html", "32861-14.html"])
            .page("32863-01.html", &["32863-02.html"])
            .page("32863-02.html", &["32863-03.html"])
            .plain(&["32863-03.html"])
            .page("32863-04.html", &["32863-03.html"])
            .npcs(&[ELCADIA, ELCADIA_INSTANCE, SOPHIA2, SOPHIA3, PILE_OF_BOOKS1]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            elcadia: t.spawn(ELCADIA, 20),
            instance: t.spawn(ELCADIA_INSTANCE, 30),
            sophia: t.spawn(SOPHIA2, 40),
            sophia3: t.spawn(SOPHIA3, 50),
            book: t.spawn(PILE_OF_BOOKS1, 60),
        };
        (t, npcs)
    }

    /// A character that has finished Girl of Doubt, with `level`.
    fn girl_done(t: &mut ScriptTest, id: i32, level: i32) {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(GIRL_OF_DOUBT.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
    }

    #[test]
    fn elcadia_starts_at_81_with_girl_of_doubt() {
        let (mut t, npcs) = setup();
        girl_done(&mut t, 1, 80);
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-11.htm"));
        girl_done(&mut t, 2, 81);
        assert!(t.quest_page(2, npcs.elcadia).contains("32784-01.htm"));
        assert!(t.event(2, NAME, "32784-03.htm").contains("32784-03.htm"));
        assert!(t.event(2, NAME, "32784-04.html").contains("32784-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::of(2, rows::insert(NAME, "cond", "1")),
                rows::of(2, rows::insert(NAME, "<state>", "Started")),
            ]
        );
    }

    #[test]
    fn the_first_talk_of_sophia_follows_the_condition() {
        let (mut t, npcs) = setup();
        // No quest: the first talk answers with the closed page.
        t.enter(1, 81, vec![]);
        t.talk(1, npcs.sophia3);
        assert!(pages_to(&t.take(), 1).iter().any(|p| p.contains("32863-04.html")));

        // Cond 1 (the book is open): the first talk answers with the open page.
        girl_done(&mut t, 2, 81);
        assert!(t.quest_page(2, npcs.elcadia).contains("32784-01.htm"));
        t.event(2, NAME, "32784-03.htm");
        t.event(2, NAME, "32784-04.html");
        t.take_writes();
        t.talk(2, npcs.sophia3);
        assert!(pages_to(&t.take(), 2).iter().any(|p| p.contains("32863-01.html")));
    }

    #[test]
    fn the_five_steps_of_the_book_and_the_reward() {
        let (mut t, npcs) = setup();
        girl_done(&mut t, 1, 81);
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-01.htm"));
        t.event(1, NAME, "32784-03.htm");
        t.event(1, NAME, "32784-04.html");
        t.take_writes();

        // Cond 1: the instance Elcadia, then Sophia (1 -> 2).
        assert!(t.quest_page(1, npcs.instance).contains("32785-01.html"));
        assert!(t.event(1, NAME, "32785-02.html").contains("32785-02.html"));
        assert!(t.quest_page(1, npcs.sophia).contains("32861-01.html"));
        t.event(1, NAME, "32861-02.html");
        t.event(1, NAME, "32861-03.html");
        assert!(t.event(1, NAME, "32861-04.html").contains("32861-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Cond 2 -> 3 is the instance Elcadia's talk; cond 3 -> 4 is Sophia's.
        assert!(t.quest_page(1, npcs.instance).contains("32785-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.sophia).contains("32861-06.html"));
        assert!(t.event(1, NAME, "32861-07.html").contains("32861-07.html"));
        assert!(t.event(1, NAME, "32861-08.html").contains("32861-08.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Cond 4 -> 5 is the instance Elcadia's event; Sophia's page takes it to 6.
        assert!(t.quest_page(1, npcs.instance).contains("32785-06.html"));
        assert!(t.event(1, NAME, "32785-07.html").contains("32785-07.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.sophia).contains("32861-10.html"));
        assert!(t.event(1, NAME, "32861-11.html").contains("32861-11.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Cond 6: the pile of books gives Solinas' Biography and cond 7.
        assert!(t.quest_page(1, npcs.book).contains("32809-01.html"));
        assert!(t.event(1, NAME, "32809-02.html").contains("32809-02.html"));
        assert_eq!(t.count(1, SOLINAS_BIOGRAPHY), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Cond 7 -> 8: the instance Elcadia; then Elcadia pays.
        assert!(t.quest_page(1, npcs.instance).contains("32785-11.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-07.html"));
        assert!(t.event(1, NAME, "32784-08.html").contains("32784-08.html"));
        assert!(t.event(1, NAME, "REWARD").contains("32784-09.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
