//! Quest 237: Winds of Change (Flauen in the Hot Springs Village sends the characters of level 82
//! and up with a letter through Iason, Roman, Morelyn and Doskozer to Helvetica or Athenia, who
//! pay with the Vicinity of Fos or the Support Certificate).
// Java: dist/game/data/scripts/quests/Q00237_WindsOfChange/Q00237_WindsOfChange.java
//
// Pattern: a delivery chain over six NPCs with the cond as the step (`docs/design/scripts.md`,
// "Quest patterns"), two end NPCs that each pay and end the quest. The reward events of the two
// end NPCs do not check the cond (Java): kept as they are.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const FLAUEN: i32 = 30899;
const IASON: i32 = 30969;
const ROMAN: i32 = 30897;
const MORELYN: i32 = 30925;
const HELVETICA: i32 = 32641;
const ATHENIA: i32 = 32643;

// Items
const FLAUENS_LETTER: i32 = 14862;
const DOSKOZER_LETTER: i32 = 14863;
const ATHENIA_LETTER: i32 = 14864;
const VICINITY_OF_FOS: i32 = 14865;
const SUPPORT_CERTIFICATE: i32 = 14866;

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `Q00237_WindsOfChange`.
pub struct Q00237WindsOfChange;

impl Script for Q00237WindsOfChange {
    fn name(&self) -> &'static str {
        "Q00237_WindsOfChange"
    }

    fn path(&self) -> &'static str {
        "quests/Q00237_WindsOfChange"
    }

    fn id(&self) -> i32 {
        237
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FLAUEN]);
        s.add_talk_id(&[FLAUEN, IASON, ROMAN, MORELYN, HELVETICA, ATHENIA]);
        s.register_quest_items(&[FLAUENS_LETTER, DOSKOZER_LETTER, ATHENIA_LETTER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (no page) when there is no quest state.
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "30899-02.htm" | "30899-03.htm" | "30899-04.htm" | "30899-05.htm" | "30969-03.html"
            | "30969-03a.html" | "30969-03b.html" | "30969-04.html" | "30969-08.html" | "30969-08a.html"
            | "30969-08b.html" | "30969-08c.html" | "30897-02.html" | "30925-02.html" => {
                Some(event.to_owned())
            }
            "30899-06.html" => {
                qs.start_quest(ctx);
                ctx.give_items(player, FLAUENS_LETTER, 1);
                Some(event.to_owned())
            }
            "30969-02.html" => {
                ctx.take_items(player, FLAUENS_LETTER, -1);
                Some(event.to_owned())
            }
            "30969-05.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30897-03.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30925-03.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30969-09.html" => {
                if qs.is_cond(ctx, 4) {
                    ctx.give_items(player, DOSKOZER_LETTER, 1);
                    qs.set_cond(ctx, 5, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30969-10.html" => {
                if qs.is_cond(ctx, 4) {
                    ctx.give_items(player, ATHENIA_LETTER, 1);
                    qs.set_cond(ctx, 6, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            // NOTE(java-quirk): the reward of Helvetica does not check the cond.
            "32641-02.html" => {
                ctx.give_adena(player, 213876, true);
                ctx.give_items(player, VICINITY_OF_FOS, 1);
                ctx.add_exp_and_sp(player, 892773, 60012);
                qs.exit_quest(ctx, false, true);
                Some(event.to_owned())
            }
            // NOTE(java-quirk): the reward of Athenia does not check the cond either.
            "32643-02.html" => {
                ctx.give_adena(player, 213876, true);
                ctx.give_items(player, SUPPORT_CERTIFICATE, 1);
                ctx.add_exp_and_sp(player, 892773, 60012);
                qs.exit_quest(ctx, false, true);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            FLAUEN => match qs.state(ctx) {
                COMPLETED => htmltext = "30899-09.html".to_owned(),
                CREATED => {
                    htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                        "30899-01.htm"
                    } else {
                        "30899-00.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 | 4 => htmltext = "30899-07.html".to_owned(),
                    2 => htmltext = "30899-10.html".to_owned(),
                    3 => htmltext = "30899-11.html".to_owned(),
                    5 | 6 => htmltext = "30899-08.html".to_owned(),
                    _ => {}
                },
                _ => {}
            },
            IASON => {
                if qs.is_completed(ctx) {
                    htmltext = ctx.get_no_quest_msg(talker);
                } else {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30969-01.html".to_owned(),
                        2 => htmltext = "30969-06.html".to_owned(),
                        4 => htmltext = "30969-07.html".to_owned(),
                        5 | 6 => htmltext = "30969-11.html".to_owned(),
                        _ => {}
                    }
                }
            }
            ROMAN => match qs.get_cond(ctx) {
                2 => htmltext = "30897-01.html".to_owned(),
                3 | 4 => htmltext = "30897-04.html".to_owned(),
                _ => {}
            },
            MORELYN => match qs.get_cond(ctx) {
                3 => htmltext = "30925-01.html".to_owned(),
                4 => htmltext = "30925-04.html".to_owned(),
                _ => {}
            },
            HELVETICA => {
                if qs.is_completed(ctx) {
                    let q238_done = ctx
                        .player_quest_state(talker, "Q00238_SuccessFailureOfBusiness")
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.has_quest_items(talker, VICINITY_OF_FOS) || q238_done {
                        "32641-03.html"
                    } else {
                        "32641-05.html"
                    }
                    .to_owned();
                } else if qs.is_cond(ctx, 5) {
                    htmltext = "32641-01.html".to_owned();
                } else if qs.is_cond(ctx, 6) {
                    htmltext = "32641-04.html".to_owned();
                }
            }
            ATHENIA => {
                if qs.is_completed(ctx) {
                    let q239_done = ctx
                        .player_quest_state(talker, "Q00239_WontYouJoinUs")
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.has_quest_items(talker, SUPPORT_CERTIFICATE) || q239_done {
                        "32643-03.html"
                    } else {
                        "32643-05.html"
                    }
                    .to_owned();
                } else if qs.is_cond(ctx, 5) {
                    htmltext = "32643-04.html".to_owned();
                } else if qs.is_cond(ctx, 6) {
                    htmltext = "32643-01.html".to_owned();
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

    const NAME: &str = "Q00237_WindsOfChange";

    const NPCS: &str = r#"
        <npc id="30899" type="Folk" level="70" name="Flauen"/>
        <npc id="30969" type="Folk" level="70" name="Iason"/>
        <npc id="30897" type="Folk" level="70" name="Roman"/>
        <npc id="30925" type="Folk" level="70" name="Morelyn"/>
        <npc id="32641" type="Folk" level="70" name="Helvetica"/>
        <npc id="32643" type="Folk" level="70" name="Athenia"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14862" type="EtcItem" name="Flauen's Letter"><set name="is_questitem" val="true"/></item>
        <item id="14863" type="EtcItem" name="Doskozer's Letter"><set name="is_questitem" val="true"/></item>
        <item id="14864" type="EtcItem" name="Athenia's Letter"><set name="is_questitem" val="true"/></item>
        <item id="14865" type="EtcItem" name="Vicinity of Fos"><set name="is_questitem" val="true"/></item>
        <item id="14866" type="EtcItem" name="Support Certificate"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        flauen: Oid,
        iason: Oid,
        roman: Oid,
        morelyn: Oid,
        helvetica: Oid,
        athenia: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00237WindsOfChange);
        let pages = Pages::of(&Q00237WindsOfChange)
            .page("30899-01.htm", &["30899-02.htm"])
            .page("30899-02.htm", &["30899-03.htm"])
            .page("30899-03.htm", &["30899-04.htm"])
            .page("30899-04.htm", &["30899-05.htm"])
            .page("30899-05.htm", &["30899-06.html"])
            .plain(&[
                "30899-00.html",
                "30899-06.html",
                "30899-07.html",
                "30899-08.html",
                "30899-09.html",
                "30899-10.html",
                "30899-11.html",
            ])
            .page("30969-01.html", &["30969-02.html"])
            .page("30969-02.html", &["30969-03.html"])
            .page(
                "30969-03.html",
                &["30969-03a.html", "30969-03b.html", "30969-04.html"],
            )
            .page("30969-03a.html", &["30969-03.html"])
            .page("30969-03b.html", &["30969-03.html"])
            .page("30969-04.html", &["30969-05.html"])
            .plain(&["30969-05.html", "30969-06.html", "30969-11.html"])
            .page("30969-07.html", &["30969-08.html"])
            .page("30969-08.html", &["30969-08a.html", "30969-08b.html"])
            .page("30969-08a.html", &["30969-09.html"])
            .page("30969-08b.html", &["30969-10.html"])
            .plain(&["30969-09.html", "30969-10.html"])
            .page("30897-01.html", &["30897-02.html"])
            .page("30897-02.html", &["30897-03.html"])
            .plain(&["30897-03.html", "30897-04.html"])
            .page("30925-01.html", &["30925-02.html"])
            .page("30925-02.html", &["30925-03.html"])
            .plain(&["30925-03.html", "30925-04.html"])
            .page("32641-01.html", &["32641-02.html"])
            .plain(&["32641-02.html", "32641-03.html", "32641-04.html", "32641-05.html"])
            .page("32643-01.html", &["32643-02.html"])
            .plain(&["32643-02.html", "32643-03.html", "32643-04.html", "32643-05.html"])
            .npcs(&[FLAUEN, IASON, ROMAN, MORELYN, HELVETICA, ATHENIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            flauen: t.spawn(FLAUEN, 20),
            iason: t.spawn(IASON, 40),
            roman: t.spawn(ROMAN, 60),
            morelyn: t.spawn(MORELYN, 80),
            helvetica: t.spawn(HELVETICA, 100),
            athenia: t.spawn(ATHENIA, 120),
        };
        (t, npcs)
    }

    /// Flauen gives the letter (cond 1): the quest starts and the letter is in the bag.
    fn start(t: &mut ScriptTest, n: &Npcs) {
        assert!(t.quest_page(1, n.flauen).contains("30899-01.htm"));
        for event in ["30899-02.htm", "30899-03.htm", "30899-04.htm", "30899-05.htm"] {
            t.event(1, NAME, event);
        }
        assert!(t.event(1, NAME, "30899-06.html").contains("30899-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, FLAUENS_LETTER), 1);
    }

    /// The letter goes to Iason, Roman and Morelyn (conds 2 to 4); Iason's page for the two
    /// last roads is shown at cond 4.
    fn to_cond_4(t: &mut ScriptTest, n: &Npcs) {
        assert!(t.quest_page(1, n.iason).contains("30969-01.html"));
        for event in ["30969-02.html", "30969-03.html", "30969-04.html"] {
            t.event(1, NAME, event);
        }
        assert_eq!(t.count(1, FLAUENS_LETTER), 0);
        t.event(1, NAME, "30969-05.html");
        assert!(t.quest_page(1, n.roman).contains("30897-01.html"));
        t.event(1, NAME, "30897-02.html");
        t.event(1, NAME, "30897-03.html");
        assert!(t.quest_page(1, n.morelyn).contains("30925-01.html"));
        t.event(1, NAME, "30925-02.html");
        t.event(1, NAME, "30925-03.html");
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "2"),
                rows::update(NAME, "cond", "3"),
                rows::update(NAME, "cond", "4")
            ]
        );
        assert!(t.quest_page(1, n.iason).contains("30969-07.html"));
        t.event(1, NAME, "30969-08.html");
    }

    #[test]
    fn a_character_below_level_82_is_sent_away() {
        let (mut t, n) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, n.flauen).contains("30899-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letter_goes_round_to_helvetica_and_the_vicinity_is_paid() {
        let (mut t, n) = setup();
        t.enter(1, 82, vec![]);
        start(&mut t, &n);
        to_cond_4(&mut t, &n);

        // Iason's road for Helvetica: Doskozer's letter and cond 5.
        assert!(t.event(1, NAME, "30969-08a.html").contains("30969-08a.html"));
        assert!(t.event(1, NAME, "30969-09.html").contains("30969-09.html"));
        assert_eq!(t.count(1, DOSKOZER_LETTER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, n.flauen).contains("30899-08.html"));

        // Helvetica pays with the Vicinity of Fos and the quest ends.
        assert!(t.quest_page(1, n.helvetica).contains("32641-01.html"));
        assert!(t.event(1, NAME, "32641-02.html").contains("32641-02.html"));
        assert_eq!(t.count(1, VICINITY_OF_FOS), 1);
        assert_eq!(t.count(1, 57), 213876);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, n.helvetica).contains("32641-03.html"));
    }

    #[test]
    fn athenia_pays_the_support_certificate_on_the_other_road() {
        let (mut t, n) = setup();
        t.enter(1, 82, vec![]);
        start(&mut t, &n);
        to_cond_4(&mut t, &n);

        // Iason's other road: Athenia's letter and cond 6.
        assert!(t.event(1, NAME, "30969-08b.html").contains("30969-08b.html"));
        assert!(t.event(1, NAME, "30969-10.html").contains("30969-10.html"));
        assert_eq!(t.count(1, ATHENIA_LETTER), 1);
        // cond 4 to 6 skips step 5: `setCond` records the skipped step in `__compltdStateFlags`.
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "6"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483601")
            ]
        );

        assert!(t.quest_page(1, n.athenia).contains("32643-01.html"));
        assert!(t.event(1, NAME, "32643-02.html").contains("32643-02.html"));
        assert_eq!(t.count(1, SUPPORT_CERTIFICATE), 1);
        assert_eq!(t.count(1, 57), 213876);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, n.athenia).contains("32643-03.html"));
    }
}
