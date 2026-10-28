//! Quest 44: Help the Son! (Lundy in the Gludin Village, Drikus in the Gludio area; the Maille
//! guards, scouts and lizardmen give the gemstone fragments).
// Java: dist/game/data/scripts/quests/Q00044_HelpTheSon/Q00044_HelpTheSon.java
//
// Pattern: the same chain as Q00042 Help the Uncle (`docs/design/scripts.md`, "Quest patterns"):
// a work hammer starts the hunt, thirty fragments make the gemstone, and the gemstone goes to
// Drikus for the ticket.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LUNDY: i32 = 30827;
const DRIKUS: i32 = 30505;

// Monsters
const MAILLE_GUARD: i32 = 20921;
const MAILLE_SCOUT: i32 = 20920;
const MAILLE_LIZARDMAN: i32 = 20919;

// Items
const WORK_HAMMER: i32 = 168;
const GEMSTONE_FRAGMENT: i32 = 7552;
const GEMSTONE: i32 = 7553;
const PET_TICKET: i32 = 7585;

// Misc
const MIN_LEVEL: i32 = 24;
const FRAGMENT_COUNT: i64 = 30;

/// Java `Q00044_HelpTheSon`.
pub struct Q00044HelpTheSon;

impl Script for Q00044HelpTheSon {
    fn name(&self) -> &'static str {
        "Q00044_HelpTheSon"
    }

    fn path(&self) -> &'static str {
        "quests/Q00044_HelpTheSon"
    }

    fn id(&self) -> i32 {
        44
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LUNDY]);
        s.add_talk_id(&[LUNDY, DRIKUS]);
        s.add_kill_id(&[MAILLE_GUARD, MAILLE_LIZARDMAN, MAILLE_SCOUT]);
        s.register_quest_items(&[GEMSTONE, GEMSTONE_FRAGMENT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        let mut htmltext = Some(event.to_owned());
        match event {
            "30827-01.htm" => {
                qs.start_quest(ctx);
            }
            "30827-03.html" => {
                if ctx.has_quest_items(player, WORK_HAMMER) {
                    ctx.take_items(player, WORK_HAMMER, 1);
                    qs.set_cond(ctx, 2, true);
                } else {
                    htmltext = Some("30827-03a.html".to_owned());
                }
            }
            "30827-06.html" => {
                if ctx.get_quest_items_count(player, GEMSTONE_FRAGMENT) == FRAGMENT_COUNT {
                    ctx.take_items(player, GEMSTONE_FRAGMENT, -1);
                    ctx.give_items(player, GEMSTONE, 1);
                    qs.set_cond(ctx, 4, true);
                } else {
                    htmltext = Some("30827-06a.html".to_owned());
                }
            }
            "30505-02.html" => {
                if ctx.has_quest_items(player, GEMSTONE) {
                    ctx.take_items(player, GEMSTONE, -1);
                    qs.set_cond(ctx, 5, true);
                } else {
                    htmltext = Some("30505-02a.html".to_owned());
                }
            }
            "30827-09.html" => {
                ctx.give_items(player, PET_TICKET, 1);
                qs.exit_quest(ctx, false, true);
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.get_quest_state(player, false)
            && qs.is_cond(ctx, 2)
        {
            ctx.give_items(player, GEMSTONE_FRAGMENT, 1);
            if ctx.get_quest_items_count(player, GEMSTONE_FRAGMENT) == FRAGMENT_COUNT {
                qs.set_cond(ctx, 3, true);
            } else {
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            LUNDY => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30827-00.htm"
                    } else {
                        "30827-00a.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        htmltext = if ctx.has_quest_items(player, WORK_HAMMER) {
                            "30827-02.html"
                        } else {
                            "30827-02a.html"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "30827-04.html".to_owned(),
                    3 => htmltext = "30827-05.html".to_owned(),
                    4 => htmltext = "30827-07.html".to_owned(),
                    5 => htmltext = "30827-08.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            DRIKUS => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        4 => htmltext = "30505-01.html".to_owned(),
                        5 => htmltext = "30505-03.html".to_owned(),
                        _ => {}
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
    use crate::testing::{Pages, ScriptTest, item, names_to, rows};

    const NAME: &str = "Q00044_HelpTheSon";

    const NPCS: &str = r#"
        <npc id="30827" type="Folk" level="70" name="Lundy"/>
        <npc id="30505" type="Folk" level="70" name="Drikus"/>
        <npc id="20919" type="Monster" level="18" name="Maille Lizardman"/>
        <npc id="20920" type="Monster" level="20" name="Maille Lizardman Scout"/>
        <npc id="20921" type="Monster" level="22" name="Maille Lizardman Guard"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="168" type="Weapon" name="Work Hammer"><set name="bodypart" val="rhand"/></item>
        <item id="7552" type="EtcItem" name="Gemstone Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7553" type="EtcItem" name="Gemstone"><set name="is_questitem" val="true"/></item>
        <item id="7585" type="EtcItem" name="Pet Ticket"/>"#;

    struct Npcs {
        lundy: Oid,
        drikus: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00044HelpTheSon);
        let pages = Pages::of(&Q00044HelpTheSon)
            .page("30827-00.htm", &["30827-01.htm"])
            .page("30827-02.html", &["30827-03.html"])
            .page("30827-05.html", &["30827-06.html"])
            .page("30827-08.html", &["30827-09.html"])
            .page("30505-01.html", &["30505-02.html"])
            .plain(&[
                "30827-00a.html",
                "30827-01.htm",
                "30827-02a.html",
                "30827-03.html",
                "30827-03a.html",
                "30827-04.html",
                "30827-06.html",
                "30827-06a.html",
                "30827-07.html",
                "30827-09.html",
                "30505-02.html",
                "30505-02a.html",
                "30505-03.html",
            ])
            .npcs(&[LUNDY, DRIKUS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            lundy: t.spawn(LUNDY, 20),
            drikus: t.spawn(DRIKUS, 40),
        };
        (t, npcs)
    }

    /// Sets the player's cond directly (for the branches reached through a long hunt).
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
            out = Q00044HelpTheSon.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills Maille lizardmen until the player has 30 fragments; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..200 {
            let lizardman = t.spawn(MAILLE_LIZARDMAN, 80);
            t.kill(1, lizardman);
            if t.count(1, GEMSTONE_FRAGMENT) >= FRAGMENT_COUNT {
                return kills;
            }
        }
        panic!("no 30 gemstone fragments after 200 kills");
    }

    #[test]
    fn a_level_23_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 23, vec![]);
        assert!(t.quest_page(1, npcs.lundy).contains("30827-00a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hammer_the_fragments_the_gemstone_and_the_ticket() {
        let (mut t, npcs) = setup();
        t.enter(1, 24, vec![item(1001, WORK_HAMMER, 1)]);
        assert!(t.quest_page(1, npcs.lundy).contains("30827-00.htm"));
        assert!(t.event(1, NAME, "30827-01.htm").contains("30827-01.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // With the hammer: the hammer goes, cond 2.
        assert!(t.quest_page(1, npcs.lundy).contains("30827-02.html"));
        assert!(t.event(1, NAME, "30827-03.html").contains("30827-03.html"));
        assert_eq!(t.count(1, WORK_HAMMER), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.lundy).contains("30827-04.html"));

        // The Maille lizardmen give the fragments; the thirtieth sets cond 3.
        hunt(&mut t);
        assert_eq!(t.count(1, GEMSTONE_FRAGMENT), 30);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Lundy takes the 30 fragments: the gemstone, cond 4.
        assert!(t.quest_page(1, npcs.lundy).contains("30827-05.html"));
        assert!(t.event(1, NAME, "30827-06.html").contains("30827-06.html"));
        assert_eq!((t.count(1, GEMSTONE_FRAGMENT), t.count(1, GEMSTONE)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Drikus takes the gemstone: cond 5.
        assert!(t.quest_page(1, npcs.drikus).contains("30505-01.html"));
        assert!(t.event(1, NAME, "30505-02.html").contains("30505-02.html"));
        assert_eq!(t.count(1, GEMSTONE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.drikus).contains("30505-03.html"));

        // Lundy gives the pet ticket: the end.
        assert!(t.quest_page(1, npcs.lundy).contains("30827-08.html"));
        assert!(t.event(1, NAME, "30827-09.html").contains("30827-09.html"));
        assert_eq!(t.count(1, PET_TICKET), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.lundy)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_refusals_without_the_hammer_or_the_fragments() {
        let (mut t, npcs) = setup();
        t.enter(1, 24, vec![]);
        t.quest_page(1, npcs.lundy);
        t.event(1, NAME, "30827-01.htm");
        t.take_writes();

        // No hammer: the page for it, cond stays 1.
        assert!(t.quest_page(1, npcs.lundy).contains("30827-02a.html"));
        assert_eq!(direct(&mut t, "30827-03.html").as_deref(), Some("30827-03a.html"));
        assert!(t.take_writes().is_empty());

        // 29 fragments at the gemstone: the page for it, nothing taken.
        set_cond(&mut t, 3);
        assert_eq!(direct(&mut t, "30827-06.html").as_deref(), Some("30827-06a.html"));
        assert_eq!(t.count(1, GEMSTONE), 0);
        assert!(t.take_writes().is_empty());

        // No gemstone for Drikus: the page for it.
        set_cond(&mut t, 4);
        assert_eq!(direct(&mut t, "30505-02.html").as_deref(), Some("30505-02a.html"));
        assert!(t.take_writes().is_empty());

        // A kill at cond 1 gives nothing (the hunt starts at cond 2).
        let scout = t.spawn(MAILLE_SCOUT, 80);
        set_cond(&mut t, 1);
        t.kill(1, scout);
        assert_eq!(t.count(1, GEMSTONE_FRAGMENT), 0);
        assert!(names_to(&t.take(), 1).iter().all(|n| n != "PLAY_SOUND"));
    }
}
