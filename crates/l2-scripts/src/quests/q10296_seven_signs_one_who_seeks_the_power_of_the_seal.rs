//! Quest 10296: Seven Signs, One Who Seeks the Power of the Seal (Eris Evil Thoughts in the Monastery of Silence).
// Java: dist/game/data/scripts/quests/Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal/Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal.java
//
// Pattern: a memo-state quest (`docs/design/scripts.md`): the memo state is the step of the Seven
// Signs story, and the other scripts of the story (the monastery, the hideout, the tomb) move it
// in their own places. This quest only reads the memo state and moves it where Java does: from
// 1 to 2 at Eris, 4 to 5 at Hardin, and the Franz reward (a sub-class cannot take it).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HARDIN: i32 = 30832;
const WOOD: i32 = 32593;
const FRANZ: i32 = 32597;
const ELCADIA: i32 = 32784;
const ELCADIA_2: i32 = 32787;
const ERISS_EVIL_THOUGHTS: i32 = 32792;
const ODD_GLOBE: i32 = 32815;

// Reward
const CERTIFICATE_OF_DAWN: i32 = 17265;

// Misc
const MIN_LEVEL: i32 = 81;

/// Java `Q10295_SevenSignsSolinasTomb.class.getSimpleName()`.
const SOLINAS_TOMB: &str = "Q10295_SevenSignsSolinasTomb";

/// Java `Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal`.
pub struct Q10296SevenSignsOneWhoSeeksThePowerOfTheSeal;

impl Script for Q10296SevenSignsOneWhoSeeksThePowerOfTheSeal {
    fn name(&self) -> &'static str {
        "Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal"
    }

    fn path(&self) -> &'static str {
        "quests/Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal"
    }

    fn id(&self) -> i32 {
        10296
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ERISS_EVIL_THOUGHTS, ODD_GLOBE]);
        s.add_talk_id(&[
            ERISS_EVIL_THOUGHTS,
            ODD_GLOBE,
            HARDIN,
            WOOD,
            FRANZ,
            ELCADIA,
            ELCADIA_2,
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
            "32792-02.htm" => htmltext = Some(event.to_owned()),
            "32792-03.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "30832-03.html" => {
                if qs.is_memo_state(ctx, 4) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30832-04.html" => {
                if qs.is_memo_state(ctx, 4) {
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32593-03.html" => {
                if qs.is_memo_state(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32597-02.html" => {
                if qs.is_memo_state(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32597-03.html" => {
                if qs.is_memo_state(ctx, 5) {
                    if ctx.is_sub_class_active(player) {
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.add_exp_and_sp(player, 125_000_000, 12_500_000);
                        ctx.give_items(player, CERTIFICATE_OF_DAWN, 1);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some("32597-04.html".to_owned());
                    }
                }
            }
            "32784-02.html" => {
                if qs.is_memo_state(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32784-03.html" => {
                if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => {
                if npc.id == ERISS_EVIL_THOUGHTS {
                    htmltext = "32792-04.html".to_owned();
                }
            }
            CREATED => {
                let tomb_done = ctx
                    .player_quest_state(player, SOLINAS_TOMB)
                    .is_some_and(|q| q.is_completed());
                if tomb_done {
                    htmltext = if npc.id == ERISS_EVIL_THOUGHTS && ctx.player_level(player) >= MIN_LEVEL {
                        "32792-01.htm"
                    } else {
                        "32815-01.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                ERISS_EVIL_THOUGHTS => {
                    if qs.is_memo_state(ctx, 1) {
                        qs.set_memo_state(ctx, 2);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "32792-05.html".to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "32792-06.html".to_owned();
                    }
                }
                ODD_GLOBE => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state > 0 && memo_state <= 2 {
                        htmltext = "32815-01.html".to_owned();
                    } else if memo_state > 2 {
                        htmltext = "32815-02.html".to_owned();
                    }
                }
                HARDIN => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state < 4 {
                        htmltext = "30832-01.html".to_owned();
                    } else if memo_state == 4 {
                        htmltext = "30832-02.html".to_owned();
                    } else if memo_state > 4 {
                        htmltext = "30832-04.html".to_owned();
                    }
                }
                WOOD => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state < 5 {
                        htmltext = "32593-01.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "32593-02.html".to_owned();
                    } else if memo_state > 5 {
                        htmltext = "32593-04.html".to_owned();
                    }
                }
                FRANZ => {
                    if qs.is_memo_state(ctx, 5) {
                        htmltext = "32597-01.html".to_owned();
                    }
                }
                ELCADIA => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state == 3 {
                        htmltext = "32784-01.html".to_owned();
                    } else if memo_state > 3 {
                        htmltext = "32784-04.html".to_owned();
                    }
                }
                ELCADIA_2 => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state < 1 {
                        htmltext = "32787-01.html".to_owned();
                    } else if memo_state < 2 {
                        htmltext = "32787-02.html".to_owned();
                    } else if memo_state == 2 {
                        htmltext = "32787-03.html".to_owned();
                    } else {
                        qs.set_cond(ctx, 3, true);
                        htmltext = "32787-04.html".to_owned();
                    }
                }
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
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal";

    const NPCS: &str = r#"
        <npc id="30832" type="Folk" level="80" name="Hardin"/>
        <npc id="32593" type="Folk" level="80" name="Wood"/>
        <npc id="32597" type="Folk" level="80" name="Franz"/>
        <npc id="32784" type="Folk" level="80" name="Elcadia"/>
        <npc id="32787" type="Folk" level="80" name="Elcadia"/>
        <npc id="32792" type="Folk" level="80" name="Eris Evil Thoughts"/>
        <npc id="32815" type="Folk" level="80" name="Odd Globe"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17265" type="EtcItem" name="Certificate of Dawn"><set name="is_stackable" val="true"/></item>"#;

    /// Stands in for Q10295 (its completion is read through `player_quest_state` only).
    struct SolinasTombStandIn;

    impl Script for SolinasTombStandIn {
        fn name(&self) -> &'static str {
            SOLINAS_TOMB
        }

        fn path(&self) -> &'static str {
            "quests/Q10295_SevenSignsSolinasTomb"
        }

        fn id(&self) -> i32 {
            10295
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    struct Npcs {
        hardin: Oid,
        wood: Oid,
        franz: Oid,
        elcadia: Oid,
        elcadia_2: Oid,
        eris: Oid,
        globe: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10296SevenSignsOneWhoSeeksThePowerOfTheSeal);
        registry.add(SolinasTombStandIn);
        let pages = Pages::of(&Q10296SevenSignsOneWhoSeeksThePowerOfTheSeal)
            .page("32792-01.htm", &["32792-02.htm"])
            .page("32792-02.htm", &["32792-03.htm"])
            .plain(&["32792-03.htm", "32792-04.html", "32792-05.html", "32792-06.html"])
            .plain(&["32815-01.html", "32815-02.html"])
            .page("30832-02.html", &["30832-03.html"])
            .page("30832-03.html", &["30832-04.html"])
            .plain(&["30832-01.html", "30832-04.html"])
            .page("32593-02.html", &["32593-03.html"])
            .plain(&["32593-01.html", "32593-03.html", "32593-04.html"])
            .page("32597-01.html", &["32597-02.html"])
            .page("32597-02.html", &["32597-03.html"])
            .plain(&["32597-03.html", "32597-04.html"])
            .page("32784-01.html", &["32784-02.html"])
            .page("32784-02.html", &["32784-03.html"])
            .plain(&["32784-03.html", "32784-04.html"])
            .plain(&["32787-01.html", "32787-02.html", "32787-03.html", "32787-04.html"])
            .npcs(&[
                HARDIN,
                WOOD,
                FRANZ,
                ELCADIA,
                ELCADIA_2,
                ERISS_EVIL_THOUGHTS,
                ODD_GLOBE,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            hardin: t.spawn(HARDIN, 20),
            wood: t.spawn(WOOD, 30),
            franz: t.spawn(FRANZ, 40),
            elcadia: t.spawn(ELCADIA, 50),
            elcadia_2: t.spawn(ELCADIA_2, 60),
            eris: t.spawn(ERISS_EVIL_THOUGHTS, 70),
            globe: t.spawn(ODD_GLOBE, 80),
        };
        (t, npcs)
    }

    /// A character that has finished Solinas' Tomb, with `level`.
    fn tomb_done(t: &mut ScriptTest, id: i32, level: i32) {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(SOLINAS_TOMB.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
    }

    /// A started character at `cond` and `memo`, with Solinas' Tomb done.
    fn started_at(t: &mut ScriptTest, id: i32, level: i32, cond: &str, memo: &str) {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![
                (SOLINAS_TOMB.to_owned(), Some("Completed".to_owned())),
                (NAME.to_owned(), Some("Started".to_owned())),
            ],
            variables: vec![
                (NAME.to_owned(), "cond".to_owned(), Some(cond.to_owned())),
                (NAME.to_owned(), "memoState".to_owned(), Some(memo.to_owned())),
            ],
        };
        t.enter_with(data);
    }

    #[test]
    fn the_start_needs_the_tomb_and_81_for_eris() {
        let (mut t, npcs) = setup();
        tomb_done(&mut t, 1, 80);
        assert!(t.quest_page(1, npcs.eris).contains("32815-01.html"));
        tomb_done(&mut t, 2, 81);
        assert!(t.quest_page(2, npcs.eris).contains("32792-01.htm"));
        // Without Solinas' Tomb there is no page of this quest.
        t.enter(3, 81, vec![]);
        assert!(!t.quest_page(3, npcs.eris).contains("32792-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn eris_starts_the_quest_and_sends_the_character_to_the_monastery() {
        let (mut t, npcs) = setup();
        tomb_done(&mut t, 1, 81);
        assert!(t.quest_page(1, npcs.eris).contains("32792-01.htm"));
        assert!(t.event(1, NAME, "32792-02.htm").contains("32792-02.htm"));
        assert!(t.event(1, NAME, "32792-03.htm").contains("32792-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.quest_page(1, npcs.eris).contains("32792-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2"),
            ]
        );
        assert!(t.quest_page(1, npcs.eris).contains("32792-06.html"));
    }

    #[test]
    fn elcadia_then_hardin_then_wood_then_franz_pays_the_certificate() {
        let (mut t, npcs) = setup();
        // Memo state 3 comes from the Monastery of Silence (another script).
        started_at(&mut t, 1, 81, "2", "3");
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-01.html"));
        assert!(t.event(1, NAME, "32784-02.html").contains("32784-02.html"));
        assert!(t.event(1, NAME, "32784-03.html").contains("32784-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483637"),
            ]
        );

        assert!(t.quest_page(1, npcs.hardin).contains("30832-02.html"));
        assert!(t.event(1, NAME, "30832-03.html").contains("30832-03.html"));
        assert!(t.event(1, NAME, "30832-04.html").contains("30832-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5"),
                rows::update(NAME, "__compltdStateFlags", "-2147483621"),
            ]
        );

        assert!(t.quest_page(1, npcs.wood).contains("32593-02.html"));
        assert!(t.event(1, NAME, "32593-03.html").contains("32593-03.html"));
        assert!(t.quest_page(1, npcs.franz).contains("32597-01.html"));
        assert!(t.event(1, NAME, "32597-02.html").contains("32597-02.html"));
        assert!(t.event(1, NAME, "32597-03.html").contains("32597-04.html"));
        assert_eq!(t.count(1, CERTIFICATE_OF_DAWN), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.eris).contains("32792-04.html"));
    }

    #[test]
    fn elcadia_two_moves_the_cond_once_the_step_is_reached() {
        let (mut t, npcs) = setup();
        started_at(&mut t, 1, 81, "2", "3");
        assert!(t.quest_page(1, npcs.elcadia_2).contains("32787-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
    }

    #[test]
    fn the_odd_globe_pages_follow_the_memo_state() {
        let (mut t, npcs) = setup();
        started_at(&mut t, 1, 81, "2", "2");
        assert!(t.quest_page(1, npcs.globe).contains("32815-01.html"));
        started_at(&mut t, 2, 81, "4", "4");
        assert!(t.quest_page(2, npcs.globe).contains("32815-02.html"));
        assert!(t.quest_page(2, npcs.hardin).contains("30832-02.html"));
    }
}
