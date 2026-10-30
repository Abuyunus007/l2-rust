//! Quest 178: Iconic Trinity (Kamael, Hierarch Kekropus and three icons with a password puzzle).
// Java: dist/game/data/scripts/quests/Q00178_IconicTrinity/Q00178_IconicTrinity.java
//
// Pattern: the memo state picks the step, `memoStateEx(1)` adds up the answers of a three (or
// four) step puzzle (1, 10, 100, 1000 per answer; the right ones add up to 111 / 1111), pages
// with the player's name (`getHtm(..).replace("%name1%", ..)`).

use l2_util::ObjectId;
use l2_world::script::{player_class, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIERARCH_KEKROPUS: i32 = 32138;
const ICON_OF_THE_PAST: i32 = 32255;
const ICON_OF_THE_PRESENT: i32 = 32256;
const ICON_OF_THE_FUTURE: i32 = 32257;

// Reward
const SCROLL_ENCHANT_ARMOR_D_GRADE: i32 = 956;

// Misc
const MIN_LEVEL: i32 = 17;
const TWENTY_LEVEL: i32 = 20;

/// Java `Q00178_IconicTrinity`.
pub struct Q00178IconicTrinity;

/// Java `getHtm(player, event).replace("%name1%", player.getName())`.
fn named_page(ctx: &mut ScriptCtx<'_>, player: ObjectId, file: &str) -> Option<String> {
    let name = ctx.player_name(player);
    ctx.get_htm(player, file)
        .map(|page| page.replace("%name1%", &name))
}

impl Script for Q00178IconicTrinity {
    fn name(&self) -> &'static str {
        "Q00178_IconicTrinity"
    }

    fn path(&self) -> &'static str {
        "quests/Q00178_IconicTrinity"
    }

    fn id(&self) -> i32 {
        178
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HIERARCH_KEKROPUS]);
        s.add_talk_id(&[
            HIERARCH_KEKROPUS,
            ICON_OF_THE_PAST,
            ICON_OF_THE_PRESENT,
            ICON_OF_THE_FUTURE,
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
            "32138-05.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32255-11.html" | "32256-11.html" | "32256-12.html" | "32256-13.html" => {
                htmltext = named_page(ctx, player, event);
            }
            "32138-14.html" => {
                // NOTE(java-quirk): `||` binds weaker than the memo/level/class group, so a
                // female soldier passes whatever the memo state and level are.
                let class = ctx.player_class_id(player);
                if (qs.is_memo_state(ctx, 10)
                    && ctx.player_level(player) <= TWENTY_LEVEL
                    && class == player_class::MALE_SOLDIER)
                    || class == player_class::FEMALE_SOLDIER
                {
                    ctx.give_items(player, SCROLL_ENCHANT_ARMOR_D_GRADE, 1);
                    ctx.add_exp_and_sp(player, 20_123, 976);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-17.html" => {
                // NOTE(java-quirk): the same precedence slip: any class but female soldier passes.
                let class = ctx.player_class_id(player);
                if (qs.is_memo_state(ctx, 10)
                    && ctx.player_level(player) > TWENTY_LEVEL
                    && class != player_class::MALE_SOLDIER)
                    || class != player_class::FEMALE_SOLDIER
                {
                    ctx.give_items(player, SCROLL_ENCHANT_ARMOR_D_GRADE, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32255-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    htmltext = Some(event.to_owned());
                }
            }
            "32255-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state_ex(ctx, 1, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "PASS1_1" => {
                if qs.is_memo_state(ctx, 2) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 1);
                    htmltext = Some("32255-04.html".to_owned());
                }
            }
            "PASS1_2" => {
                if qs.is_memo_state(ctx, 2) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 10);
                    htmltext = Some("32255-05.html".to_owned());
                }
            }
            "PASS1_3" => {
                if qs.is_memo_state(ctx, 2) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 100);
                    htmltext = Some("32255-06.html".to_owned());
                }
            }
            "PASS1_4" => {
                if qs.is_memo_state(ctx, 2) {
                    if qs.get_memo_state_ex(ctx, 1) == 111 {
                        qs.set_memo_state(ctx, 3);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        htmltext = Some("32255-07.html".to_owned());
                    } else if qs.get_memo_state_ex(ctx, 1) != 111 {
                        htmltext = Some("32255-08.html".to_owned());
                    }
                }
            }
            "32255-13.html" => {
                if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 2, true);
                    htmltext = named_page(ctx, player, event);
                }
            }
            "32256-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    qs.set_memo_state(ctx, 5);
                    htmltext = Some(event.to_owned());
                }
            }
            "32256-03.html" => {
                if qs.is_memo_state(ctx, 5) {
                    qs.set_memo_state_ex(ctx, 1, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "PASS2_1" => {
                if qs.is_memo_state(ctx, 5) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 1);
                    htmltext = Some("32256-04.html".to_owned());
                }
            }
            "PASS2_2" => {
                if qs.is_memo_state(ctx, 5) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 10);
                    htmltext = Some("32256-05.html".to_owned());
                }
            }
            "PASS2_3" => {
                if qs.is_memo_state(ctx, 5) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 100);
                    htmltext = Some("32256-06.html".to_owned());
                }
            }
            "PASS2_4" => {
                if qs.is_memo_state(ctx, 5) {
                    if qs.get_memo_state_ex(ctx, 1) == 111 {
                        qs.set_memo_state(ctx, 6);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        htmltext = Some("32256-07.html".to_owned());
                    } else if qs.get_memo_state_ex(ctx, 1) != 111 {
                        htmltext = Some("32256-08.html".to_owned());
                    }
                }
            }
            "32256-14.html" => {
                if qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 7);
                    qs.set_cond(ctx, 3, true);
                    htmltext = named_page(ctx, player, event);
                }
            }
            "32257-02.html" => {
                if qs.is_memo_state(ctx, 7) {
                    qs.set_memo_state(ctx, 8);
                    htmltext = Some(event.to_owned());
                }
            }
            "32257-03.html" => {
                if qs.is_memo_state(ctx, 8) {
                    qs.set_memo_state_ex(ctx, 1, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "PASS3_1" => {
                if qs.is_memo_state(ctx, 8) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 1);
                    htmltext = Some("32257-04.html".to_owned());
                }
            }
            "PASS3_2" => {
                if qs.is_memo_state(ctx, 8) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 10);
                    htmltext = Some("32257-05.html".to_owned());
                }
            }
            "PASS3_3" => {
                if qs.is_memo_state(ctx, 8) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 100);
                    htmltext = Some("32257-06.html".to_owned());
                }
            }
            "PASS3_4" => {
                if qs.is_memo_state(ctx, 8) {
                    let i0 = qs.get_memo_state_ex(ctx, 1);
                    qs.set_memo_state_ex(ctx, 1, i0 + 1000);
                    htmltext = Some("32257-07.html".to_owned());
                }
            }
            "PASS3_5" => {
                if qs.is_memo_state(ctx, 8) {
                    if qs.get_memo_state_ex(ctx, 1) == 1111 {
                        qs.set_memo_state(ctx, 9);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        htmltext = Some("32257-08.html".to_owned());
                    } else if qs.get_memo_state_ex(ctx, 1) != 1111 {
                        htmltext = Some("32257-09.html".to_owned());
                    }
                }
            }
            "32257-12.html" => {
                if qs.is_memo_state(ctx, 9) {
                    qs.set_memo_state(ctx, 10);
                    qs.set_cond(ctx, 4, true);
                    htmltext = named_page(ctx, player, event);
                }
            }
            "32138-13.html" | "32138-16.html" | "32255-04.html" | "32255-05.html" | "32255-06.html"
            | "32255-07.html" | "32255-08.html" | "32255-09.html" | "32255-10.html" | "32255-12.html"
            | "32256-04.html" | "32256-05.html" | "32256-06.html" | "32256-07.html" | "32256-08.html"
            | "32256-09.html" | "32256-10.html" | "32257-04.html" | "32257-05.html" | "32257-06.html"
            | "32257-07.html" | "32257-08.html" | "32257-09.html" | "32257-10.html" | "32257-11.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == HIERARCH_KEKROPUS {
                if ctx.player_race(player) != race::KAMAEL {
                    htmltext = "32138-03.htm".to_owned();
                } else if ctx.player_level(player) >= MIN_LEVEL {
                    htmltext = "32138-01.htm".to_owned();
                } else {
                    htmltext = "32138-02.htm".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                HIERARCH_KEKROPUS => match qs.get_memo_state(ctx) {
                    1 | 2 => htmltext = "32138-06.html".to_owned(),
                    3 => htmltext = "32138-07.html".to_owned(),
                    4 | 5 => htmltext = "32138-08.html".to_owned(),
                    6 => htmltext = "32138-09.html".to_owned(),
                    7 | 8 => htmltext = "32138-10.html".to_owned(),
                    9 => htmltext = "32138-11.html".to_owned(),
                    10 => {
                        let class = ctx.player_class_id(player);
                        if (ctx.player_level(player) <= TWENTY_LEVEL && class == player_class::MALE_SOLDIER)
                            || class == player_class::FEMALE_SOLDIER
                        {
                            htmltext = "32138-12.html".to_owned();
                        } else {
                            htmltext = "32138-15.html".to_owned();
                        }
                    }
                    _ => {}
                },
                ICON_OF_THE_PAST => match qs.get_memo_state(ctx) {
                    1 => htmltext = "32255-01.html".to_owned(),
                    2 => {
                        qs.set_memo_state_ex(ctx, 1, 0);
                        htmltext = "32255-03.html".to_owned();
                    }
                    3 => htmltext = "32255-09.html".to_owned(),
                    4 | 5 => htmltext = "32255-14.html".to_owned(),
                    _ => {}
                },
                ICON_OF_THE_PRESENT => match qs.get_memo_state(ctx) {
                    4 => htmltext = "32256-01.html".to_owned(),
                    5 => {
                        qs.set_memo_state_ex(ctx, 1, 0);
                        htmltext = "32256-03.html".to_owned();
                    }
                    6 => htmltext = "32256-09.html".to_owned(),
                    7 | 8 => htmltext = "32256-15.html".to_owned(),
                    _ => {}
                },
                ICON_OF_THE_FUTURE => match qs.get_memo_state(ctx) {
                    7 => htmltext = "32257-01.html".to_owned(),
                    8 => {
                        qs.set_memo_state_ex(ctx, 1, 0);
                        htmltext = "32257-03.html".to_owned();
                    }
                    9 => htmltext = "32257-10.html".to_owned(),
                    10 => htmltext = "32257-13.html".to_owned(),
                    _ => {}
                },
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == HIERARCH_KEKROPUS {
            htmltext = ctx.get_already_completed_msg(player);
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

    const NAME: &str = "Q00178_IconicTrinity";

    const NPCS: &str = r#"
        <npc id="32138" type="Folk" level="1" name="Kekropus"/>
        <npc id="32255" type="Folk" level="1" name="Icon of the Past"/>
        <npc id="32256" type="Folk" level="1" name="Icon of the Present"/>
        <npc id="32257" type="Folk" level="1" name="Icon of the Future"/>"#;

    const ITEMS: &str = r#"
        <item id="956" type="EtcItem" name="Scroll: Enchant Armor (D)"/>"#;

    struct Npcs {
        kekropus: Oid,
        past: Oid,
        present: Oid,
        future: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00178IconicTrinity);
        let mut pages = Pages::of(&Q00178IconicTrinity)
            .page("32138-01.htm", &["32138-05.htm"])
            .page("32138-12.html", &["32138-13.html"])
            .page("32138-13.html", &["32138-14.html"])
            .page("32138-15.html", &["32138-16.html"])
            .page("32138-16.html", &["32138-17.html"])
            // Icon of the past: after the answers, 07/08 and the page chain to the cond 2 page.
            .page("32255-01.html", &["32255-02.html"])
            .page("32255-02.html", &["32255-03.html"])
            .page("32255-03.html", &["PASS1_1"])
            .page("32255-04.html", &["PASS1_2"])
            .page("32255-05.html", &["PASS1_3"])
            .page("32255-06.html", &["PASS1_4"])
            .page("32255-08.html", &["32255-03.html"])
            .page("32255-09.html", &["32255-10.html"])
            .page("32255-10.html", &["32255-11.html"])
            .page("32255-12.html", &["32255-13.html"])
            // Icon of the present.
            .page("32256-01.html", &["32256-02.html"])
            .page("32256-02.html", &["32256-03.html"])
            .page("32256-03.html", &["PASS2_1"])
            .page("32256-04.html", &["PASS2_2"])
            .page("32256-05.html", &["PASS2_3"])
            .page("32256-06.html", &["PASS2_4"])
            .page("32256-08.html", &["32256-03.html"])
            .page("32256-09.html", &["32256-10.html"])
            .page("32256-10.html", &["32256-11.html"])
            // Icon of the future.
            .page("32257-01.html", &["32257-02.html"])
            .page("32257-02.html", &["32257-03.html"])
            .page("32257-03.html", &["PASS3_1"])
            .page("32257-04.html", &["PASS3_2"])
            .page("32257-05.html", &["PASS3_3"])
            .page("32257-06.html", &["PASS3_4"])
            .page("32257-07.html", &["PASS3_5"])
            .page("32257-09.html", &["32257-03.html"])
            .page("32257-10.html", &["32257-11.html"])
            .page("32257-11.html", &["32257-12.html"]);
        // The pages with the player's name carry the marker the script replaces (and the link
        // to the next page, as the real ones do).
        for (file, next) in [
            ("32255-11.html", Some("32255-12.html")),
            ("32255-13.html", None),
            ("32256-11.html", Some("32256-12.html")),
            ("32256-12.html", Some("32256-13.html")),
            ("32256-13.html", Some("32256-14.html")),
            ("32256-14.html", None),
            ("32257-12.html", None),
        ] {
            let link = next
                .map(|n| format!(r#"<a action="bypass Script {NAME} {n}">next</a>"#))
                .unwrap_or_default();
            pages = pages.file(
                &format!("data/scripts/quests/{NAME}/{file}"),
                &format!("<html><body>Well done, %name1%.{link}</body></html>"),
            );
        }
        let pages = pages
            .plain(&[
                "32138-02.htm",
                "32138-03.htm",
                "32138-05.htm",
                "32138-06.html",
                "32138-07.html",
                "32138-08.html",
                "32138-09.html",
                "32138-10.html",
                "32138-11.html",
                "32138-14.html",
                "32138-17.html",
                "32255-07.html",
                "32255-14.html",
                "32256-07.html",
                "32256-15.html",
                "32257-08.html",
                "32257-13.html",
            ])
            .npcs(&[
                HIERARCH_KEKROPUS,
                ICON_OF_THE_PAST,
                ICON_OF_THE_PRESENT,
                ICON_OF_THE_FUTURE,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            kekropus: t.spawn(HIERARCH_KEKROPUS, 20),
            past: t.spawn(ICON_OF_THE_PAST, 30),
            present: t.spawn(ICON_OF_THE_PRESENT, 40),
            future: t.spawn(ICON_OF_THE_FUTURE, 50),
        };
        (t, npcs)
    }

    fn kamael(t: &mut ScriptTest, level: i32, class: i32) {
        t.enter_as(1, level, race::KAMAEL, class, vec![]);
    }

    #[test]
    fn only_a_kamael_of_level_17_starts_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-03.htm"));
        t.enter_as(2, 16, race::KAMAEL, player_class::MALE_SOLDIER, vec![]);
        assert!(t.quest_page(2, npcs.kekropus).contains("32138-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_three_icons_with_their_puzzles_and_the_reward() {
        let (mut t, npcs) = setup();
        kamael(&mut t, 17, player_class::MALE_SOLDIER);
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-01.htm"));
        assert!(t.event(1, NAME, "32138-05.htm").contains("32138-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1")
            ]
        );
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-06.html"));

        // The past: memo 2, then the answers 1 + 10 + 100 = 111 pass the puzzle.
        assert!(t.quest_page(1, npcs.past).contains("32255-01.html"));
        assert!(t.event(1, NAME, "32255-02.html").contains("32255-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "2")]);
        assert!(t.event(1, NAME, "32255-03.html").contains("32255-03.html"));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "memoStateEx1", "0")]);
        // The answers add up in the memo (1, then 10).
        assert!(t.event(1, NAME, "PASS1_1").contains("32255-04.html"));
        assert!(t.event(1, NAME, "PASS1_2").contains("32255-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoStateEx1", "1"),
                rows::update(NAME, "memoStateEx1", "11")
            ]
        );
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            assert_eq!(qs.get_memo_state_ex(ctx, 1), 11);
        });
        assert!(t.event(1, NAME, "PASS1_3").contains("32255-06.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoStateEx1", "111")]);
        assert!(t.event(1, NAME, "PASS1_4").contains("32255-07.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "memoStateEx1", "0")
            ]
        );
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-07.html"));
        assert!(t.quest_page(1, npcs.past).contains("32255-09.html"));
        assert!(t.event(1, NAME, "32255-10.html").contains("32255-10.html"));
        assert!(t.event(1, NAME, "32255-11.html").contains("Well done"));
        assert!(t.event(1, NAME, "32255-12.html").contains("32255-12.html"));
        let page = t.event(1, NAME, "32255-13.html");
        assert!(page.contains("Well done"), "{page}");
        assert!(!page.contains("%name1%"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, npcs.past).contains("32255-14.html"));

        // The present: the same, memo 5, 6, then cond 3.
        assert!(t.quest_page(1, npcs.present).contains("32256-01.html"));
        assert!(t.event(1, NAME, "32256-02.html").contains("32256-02.html"));
        assert!(t.event(1, NAME, "32256-03.html").contains("32256-03.html"));
        assert!(t.event(1, NAME, "PASS2_1").contains("32256-04.html"));
        assert!(t.event(1, NAME, "PASS2_2").contains("32256-05.html"));
        assert!(t.event(1, NAME, "PASS2_3").contains("32256-06.html"));
        assert!(t.event(1, NAME, "PASS2_4").contains("32256-07.html"));
        t.take_writes();
        assert!(t.quest_page(1, npcs.present).contains("32256-09.html"));
        for e in ["32256-10.html", "32256-11.html", "32256-12.html", "32256-13.html"] {
            t.event(1, NAME, e);
        }
        let page = t.event(1, NAME, "32256-14.html");
        assert!(page.contains("Well done"), "{page}");
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "7"),
                rows::update(NAME, "cond", "3")
            ]
        );
        assert!(t.quest_page(1, npcs.present).contains("32256-15.html"));

        // The future: four answers (1111), then cond 4.
        assert!(t.quest_page(1, npcs.future).contains("32257-01.html"));
        assert!(t.event(1, NAME, "32257-02.html").contains("32257-02.html"));
        assert!(t.event(1, NAME, "32257-03.html").contains("32257-03.html"));
        for (e, next) in [
            ("PASS3_1", "32257-04.html"),
            ("PASS3_2", "32257-05.html"),
            ("PASS3_3", "32257-06.html"),
            ("PASS3_4", "32257-07.html"),
            ("PASS3_5", "32257-08.html"),
        ] {
            assert!(t.event(1, NAME, e).contains(next), "{e}");
        }
        t.take_writes();
        assert!(t.quest_page(1, npcs.future).contains("32257-10.html"));
        t.event(1, NAME, "32257-11.html");
        let page = t.event(1, NAME, "32257-12.html");
        assert!(page.contains("Well done"), "{page}");
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "10"),
                rows::update(NAME, "cond", "4")
            ]
        );
        assert!(t.quest_page(1, npcs.future).contains("32257-13.html"));

        // Kekropus: a male soldier of level <= 20 gets the exp page, the scroll and the exit.
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-12.html"));
        assert!(t.event(1, NAME, "32138-13.html").contains("32138-13.html"));
        assert!(t.event(1, NAME, "32138-14.html").contains("32138-14.html"));
        assert_eq!(t.count(1, SCROLL_ENCHANT_ARMOR_D_GRADE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn a_higher_level_gets_only_the_scroll() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 25, race::KAMAEL, player_class::MALE_SOLDIER, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_memo_state(ctx, 10);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-15.html"));
        assert!(t.event(1, NAME, "32138-16.html").contains("32138-16.html"));
        assert!(t.event(1, NAME, "32138-17.html").contains("32138-17.html"));
        assert_eq!(t.count(1, SCROLL_ENCHANT_ARMOR_D_GRADE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
