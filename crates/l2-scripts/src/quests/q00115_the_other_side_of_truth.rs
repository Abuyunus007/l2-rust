//! Quest 115: The Other Side of Truth (Rafforty, Misa, Kier, the four ice sculptures).
// Java: dist/game/data/scripts/quests/Q00115_TheOtherSideOfTruth/Q00115_TheOtherSideOfTruth.java
//
// Pattern: a cond chain; the four ice sculptures share a bit field in the quest variable `ex`
// (each sculpture adds its own bit, the pages depend on the bits already set).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MISA: i32 = 32018;
const RAFFORTY: i32 = 32020;
const ICE_SCULPTURE1: i32 = 32021;
const KIER: i32 = 32022;
const ICE_SCULPTURE2: i32 = 32077;
const ICE_SCULPTURE3: i32 = 32078;
const ICE_SCULPTURE4: i32 = 32079;

// Items
const MISAS_LETTER: i32 = 8079;
const RAFFORTYS_LETTER: i32 = 8080;
const PIECE_OF_TABLET: i32 = 8081;
const REPORT_PIECE: i32 = 8082;

// Misc
const MIN_LEVEL: i32 = 53;

/// What differs between the four sculptures: whether the bits already set allow this one
/// (`ex % 2 <= 1`, `ex % 4 <= 1`, `ex % 8 <= 3`, `ex <= 7`), the bit it adds, the values of `ex`
/// at which it has been visited (`ex` is then in `visited`), and the `ex` that ends the puzzle.
struct Sculpture {
    allowed: fn(i32) -> bool,
    add: i32,
    visited: [i32; 3],
    last: i32,
}

fn sculpture(npc_id: i32) -> Option<Sculpture> {
    match npc_id {
        ICE_SCULPTURE1 => Some(Sculpture {
            allowed: |ex| ex % 2 <= 1,
            add: 1,
            visited: [6, 10, 12],
            last: 14,
        }),
        ICE_SCULPTURE2 => Some(Sculpture {
            allowed: |ex| ex % 4 <= 1,
            add: 2,
            visited: [5, 9, 12],
            last: 13,
        }),
        ICE_SCULPTURE3 => Some(Sculpture {
            allowed: |ex| ex % 8 <= 3,
            add: 4,
            visited: [3, 9, 10],
            last: 11,
        }),
        ICE_SCULPTURE4 => Some(Sculpture {
            allowed: |ex| ex <= 7,
            add: 8,
            visited: [3, 5, 6],
            last: 7,
        }),
        _ => None,
    }
}

/// Java `Q00115_TheOtherSideOfTruth`.
pub struct Q00115TheOtherSideOfTruth;

impl Q00115TheOtherSideOfTruth {
    /// The reward both `finish` links and Rafforty's cond 11 page share.
    fn reward(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        ctx.give_adena(player, 115_673, true);
        ctx.add_exp_and_sp(player, 493_595, 40_442);
    }
}

impl Script for Q00115TheOtherSideOfTruth {
    fn name(&self) -> &'static str {
        "Q00115_TheOtherSideOfTruth"
    }

    fn path(&self) -> &'static str {
        "quests/Q00115_TheOtherSideOfTruth"
    }

    fn id(&self) -> i32 {
        115
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAFFORTY]);
        s.add_talk_id(&[
            RAFFORTY,
            MISA,
            KIER,
            ICE_SCULPTURE1,
            ICE_SCULPTURE2,
            ICE_SCULPTURE3,
            ICE_SCULPTURE4,
        ]);
        s.register_quest_items(&[MISAS_LETTER, RAFFORTYS_LETTER, PIECE_OF_TABLET, REPORT_PIECE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32020-02.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32020-07.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.take_items(player, MISAS_LETTER, -1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32020-05.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.take_items(player, MISAS_LETTER, -1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32020-10.html" | "32020-11.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32020-12.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32020-08.html" | "32020-09.html" | "32020-13.html" | "32020-14.html" => {
                htmltext = Some(event.to_owned());
            }
            "32020-15.html" => {
                if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, true);
                    ctx.play_sound(player, QuestSound::AmbsoundWingflap);
                    htmltext = Some(event.to_owned());
                }
            }
            "32020-23.html" => {
                if qs.is_cond(ctx, 9) {
                    qs.set_cond(ctx, 10, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "finish" | "finish2" => {
                if qs.is_cond(ctx, 10) {
                    if ctx.has_quest_items(player, PIECE_OF_TABLET) {
                        Self::reward(ctx, player);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some(if event == "finish" { "32020-25.html" } else { "32020-27.html" }.to_owned());
                    } else {
                        qs.set_cond(ctx, 11, true);
                        htmltext = Some(if event == "finish" { "32020-26.html" } else { "32020-28.html" }.to_owned());
                        ctx.play_sound(player, QuestSound::AmbsoundThunder);
                    }
                }
            }
            "32018-05.html" => {
                if qs.is_cond(ctx, 6) && ctx.has_quest_items(player, RAFFORTYS_LETTER) {
                    ctx.take_items(player, RAFFORTYS_LETTER, -1);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32022-02.html" => {
                if qs.is_cond(ctx, 8) {
                    ctx.give_items(player, REPORT_PIECE, 1);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32021-02.html" | "32021-03.html" => {
                // NOTE(java-quirk): `npc.getId()` throws without an NPC.
                let npc = npc?;
                if let Some(mut sc) = sculpture(npc.id) {
                    // NOTE(java-quirk): in "32021-03" the third sculpture lists 12 where "32021-02"
                    // lists 10 as the second visited value.
                    if npc.id == ICE_SCULPTURE3 && event == "32021-03.html" {
                        sc.visited = [3, 9, 12];
                    }
                    let ex = qs.get_int(ctx, "ex");
                    if qs.is_cond(ctx, 7) && (sc.allowed)(ex) && sc.visited.contains(&ex) {
                        qs.set_int(ctx, "ex", ex + sc.add);
                        if event == "32021-02.html" {
                            ctx.give_items(player, PIECE_OF_TABLET, 1);
                        }
                        htmltext = Some(event.to_owned());
                    }
                }
            }
            "32021-06.html" => {
                let npc = npc?;
                if let Some(sc) = sculpture(npc.id)
                    && qs.is_cond(ctx, 7)
                    && qs.get_int(ctx, "ex") == sc.last
                {
                    qs.set_cond(ctx, 8, false);
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
                if npc.id == RAFFORTY {
                    htmltext = ctx.get_already_completed_msg(player);
                }
            }
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32020-01.htm"
                } else {
                    "32020-03.html"
                }
                .to_owned();
            }
            STARTED => match npc.id {
                RAFFORTY => match qs.get_cond(ctx) {
                    1 => htmltext = "32020-04.html".to_owned(),
                    2 => {
                        htmltext = if !ctx.has_quest_items(player, MISAS_LETTER) {
                            "32020-05.html"
                        } else {
                            "32020-06.html"
                        }
                        .to_owned();
                    }
                    3 => htmltext = "32020-16.html".to_owned(),
                    4 => htmltext = "32020-17.html".to_owned(),
                    5 => {
                        ctx.give_items(player, RAFFORTYS_LETTER, 1);
                        qs.set_cond(ctx, 6, true);
                        htmltext = "32020-18.html".to_owned();
                    }
                    6 => {
                        if ctx.has_quest_items(player, RAFFORTYS_LETTER) {
                            htmltext = "32020-19.html".to_owned();
                        } else {
                            ctx.give_items(player, RAFFORTYS_LETTER, 1);
                            htmltext = "32020-20.html".to_owned();
                        }
                    }
                    7 | 8 => htmltext = "32020-21.html".to_owned(),
                    9 => {
                        if ctx.has_quest_items(player, REPORT_PIECE) {
                            htmltext = "32020-22.html".to_owned();
                        }
                    }
                    10 => htmltext = "32020-24.html".to_owned(),
                    11 => {
                        if !ctx.has_quest_items(player, PIECE_OF_TABLET) {
                            htmltext = "32020-29.html".to_owned();
                        } else {
                            Self::reward(ctx, player);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "32020-30.html".to_owned();
                        }
                    }
                    _ => {}
                },
                MISA => match qs.get_cond(ctx) {
                    1 => {
                        ctx.give_items(player, MISAS_LETTER, 1);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "32018-01.html".to_owned();
                    }
                    2 => htmltext = "32018-02.html".to_owned(),
                    3 | 4 => htmltext = "32018-03.html".to_owned(),
                    6 => {
                        if ctx.has_quest_items(player, RAFFORTYS_LETTER) {
                            htmltext = "32018-04.html".to_owned();
                        }
                    }
                    7 => htmltext = "32018-06.html".to_owned(),
                    _ => {}
                },
                KIER => match qs.get_cond(ctx) {
                    8 => htmltext = "32022-01.html".to_owned(),
                    9 => {
                        if ctx.has_quest_items(player, REPORT_PIECE) {
                            htmltext = "32022-03.html".to_owned();
                        } else {
                            ctx.give_items(player, REPORT_PIECE, 1);
                            htmltext = "32022-04.html".to_owned();
                        }
                    }
                    11 => {
                        if !ctx.has_quest_items(player, REPORT_PIECE) {
                            htmltext = "32022-05.html".to_owned();
                        }
                    }
                    _ => {}
                },
                ICE_SCULPTURE1 | ICE_SCULPTURE2 | ICE_SCULPTURE3 | ICE_SCULPTURE4 => {
                    let Some(sc) = sculpture(npc.id) else {
                        return Some(htmltext);
                    };
                    match qs.get_cond(ctx) {
                        7 => {
                            let ex = qs.get_int(ctx, "ex");
                            if (sc.allowed)(ex) {
                                if sc.visited.contains(&ex) {
                                    htmltext = "32021-01.html".to_owned();
                                } else if ex == sc.last {
                                    htmltext = "32021-05.html".to_owned();
                                } else {
                                    qs.set_int(ctx, "ex", ex + sc.add);
                                    htmltext = "32021-07.html".to_owned();
                                }
                            } else {
                                htmltext = "32021-04.html".to_owned();
                            }
                        }
                        8 => htmltext = "32021-08.html".to_owned(),
                        11 => {
                            if !ctx.has_quest_items(player, PIECE_OF_TABLET) {
                                ctx.give_items(player, PIECE_OF_TABLET, 1);
                                htmltext = "32021-09.html".to_owned();
                            } else {
                                htmltext = "32021-10.html".to_owned();
                            }
                        }
                        _ => {}
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00115_TheOtherSideOfTruth";

    const NPCS: &str = r#"
        <npc id="32018" type="Folk" level="70" name="Misa"/>
        <npc id="32020" type="Folk" level="70" name="Rafforty"/>
        <npc id="32021" type="Folk" level="70" name="Ice Sculpture"/>
        <npc id="32077" type="Folk" level="70" name="Ice Sculpture 2"/>
        <npc id="32022" type="Folk" level="70" name="Kier"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8079" type="EtcItem" name="Misa's Letter"><set name="is_questitem" val="true"/></item>
        <item id="8081" type="EtcItem" name="Piece of Tablet"><set name="is_questitem" val="true"/></item>
        <item id="8082" type="EtcItem" name="Report Piece"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00115TheOtherSideOfTruth);
        let pages = Pages::of(&Q00115TheOtherSideOfTruth)
            .page("32020-01.htm", &["32020-02.html"])
            .plain(&["32020-02.html", "32020-03.html", "32020-25.html", "32020-26.html"])
            .page("32018-01.html", &["32020-07.html"])
            .page("32021-01.html", &["32021-02.html", "32021-06.html"])
            .plain(&["32021-02.html", "32021-06.html", "32021-07.html"])
            .npcs(&[RAFFORTY, MISA, ICE_SCULPTURE1, ICE_SCULPTURE2, KIER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rafforty = t.spawn(RAFFORTY, 20);
        let misa = t.spawn(MISA, 40);
        let s1 = t.spawn(ICE_SCULPTURE1, 60);
        let s2 = t.spawn(ICE_SCULPTURE2, 80);
        (t, rafforty, misa, s1, s2)
    }

    fn ev(t: &mut ScriptTest, event: &str, npc: Option<Oid>) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            out = Q00115TheOtherSideOfTruth.on_event(ctx, event, npc, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00115TheOtherSideOfTruth.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn set_cond(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, value, false);
            }
        });
        t.take_writes();
    }

    #[test]
    fn it_needs_level_53() {
        let (mut t, rafforty, ..) = setup();
        t.enter(1, 52, vec![]);
        assert!(t.quest_page(1, rafforty).contains("32020-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_and_misas_letter_write_the_rows() {
        let (mut t, rafforty, misa, ..) = setup();
        t.enter(1, 53, vec![]);
        assert!(t.quest_page(1, rafforty).contains("32020-01.htm"));
        assert!(t.event(1, NAME, "32020-02.html").contains("32020-02.html"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]
        );
        assert_eq!(talk(&mut t, misa).as_deref(), Some("32018-01.html"));
        assert_eq!(t.count(1, MISAS_LETTER), 1);
        assert_eq!(t.take_writes().len(), 1);
        assert_eq!(ev(&mut t, "32020-07.html", None).as_deref(), Some("32020-07.html"));
        assert_eq!(t.count(1, MISAS_LETTER), 0);
    }

    #[test]
    fn the_sculptures_set_their_bits_in_ex() {
        let (mut t, rafforty, _, s1, s2) = setup();
        t.enter(1, 53, vec![]);
        t.quest_page(1, rafforty);
        t.event(1, NAME, "32020-02.html");
        set_cond(&mut t, 7);

        // ex 0: the first sculpture adds its bit (page 07), the second one too.
        assert_eq!(talk(&mut t, s1).as_deref(), Some("32021-07.html"));
        assert_eq!(talk(&mut t, s2).as_deref(), Some("32021-07.html"));
        let w = t.take_writes();
        assert_eq!(w, [rows::insert(NAME, "ex", "1"), rows::update(NAME, "ex", "3")]);
        // ex 3: the first one has been visited (bit 1 set, 3 is not listed for it: page 07 again
        // would be wrong); sculpture 1 lists 6, 10, 12, so ex 3 is not "visited" and adds again.
        assert_eq!(talk(&mut t, s1).as_deref(), Some("32021-07.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "ex", "4")]);
        // A link needs the visited values: 4 is not one of them.
        assert_eq!(ev(&mut t, "32021-02.html", Some(s1)), None);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_int(ctx, "ex", 6);
            }
        });
        t.take_writes();
        assert_eq!(talk(&mut t, s1).as_deref(), Some("32021-01.html"));
        assert_eq!(ev(&mut t, "32021-02.html", Some(s1)).as_deref(), Some("32021-02.html"));
        assert_eq!(t.count(1, PIECE_OF_TABLET), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "ex", "7")]);
        // ex 14 ends the puzzle for the first sculpture.
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_int(ctx, "ex", 14);
            }
        });
        t.take_writes();
        assert_eq!(talk(&mut t, s1).as_deref(), Some("32021-05.html"));
        assert_eq!(ev(&mut t, "32021-06.html", Some(s1)).as_deref(), Some("32021-06.html"));
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "8")));
    }

    #[test]
    fn finish_pays_with_the_tablet_and_sends_back_without() {
        let (mut t, rafforty, ..) = setup();
        t.enter(1, 53, vec![]);
        t.quest_page(1, rafforty);
        t.event(1, NAME, "32020-02.html");
        set_cond(&mut t, 10);
        assert_eq!(ev(&mut t, "finish", None).as_deref(), Some("32020-26.html"));
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "11")));

        t.world.run_as_script(NAME, |ctx| ctx.give_items(Oid(1), PIECE_OF_TABLET, 1));
        assert_eq!(talk(&mut t, rafforty).as_deref(), Some("32020-30.html"));
        assert_eq!(t.count(1, 57), 115_673);
        let w: Vec<_> = t
            .take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect();
        assert!(w.ends_with(&[
            rows::delete(NAME, false),
            rows::update(NAME, "<state>", "Completed")
        ]));
    }
}
