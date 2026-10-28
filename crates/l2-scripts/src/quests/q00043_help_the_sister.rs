//! Quest 43: Help the Sister! (Cooper in the Gludin Village, Galladucci in the Gludio area; the
//! specters and sorrow maidens give the map pieces).
// Java: dist/game/data/scripts/quests/Q00043_HelpTheSister/Q00043_HelpTheSister.java
//
// Pattern: the same chain as Q00042 Help the Uncle (`docs/design/scripts.md`, "Quest patterns"):
// a crafted dagger starts the hunt, thirty map pieces make the map, and the map goes to
// Galladucci for the ticket.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const COOPER: i32 = 30829;
const GALLADUCCI: i32 = 30097;

// Monsters
const SPECTER: i32 = 20171;
const SORROW_MAIDEN: i32 = 20197;

// Items
const CRAFTED_DAGGER: i32 = 220;
const MAP_PIECE: i32 = 7550;
const MAP: i32 = 7551;
const PET_TICKET: i32 = 7584;

// Misc
const MIN_LEVEL: i32 = 26;
const MAP_PIECE_COUNT: i64 = 30;

/// Java `Q00043_HelpTheSister`.
pub struct Q00043HelpTheSister;

impl Script for Q00043HelpTheSister {
    fn name(&self) -> &'static str {
        "Q00043_HelpTheSister"
    }

    fn path(&self) -> &'static str {
        "quests/Q00043_HelpTheSister"
    }

    fn id(&self) -> i32 {
        43
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[COOPER]);
        s.add_talk_id(&[COOPER, GALLADUCCI]);
        s.add_kill_id(&[SORROW_MAIDEN, SPECTER]);
        s.register_quest_items(&[MAP, MAP_PIECE]);
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
            "30829-01.htm" => {
                qs.start_quest(ctx);
            }
            "30829-03.html" => {
                if ctx.has_quest_items(player, CRAFTED_DAGGER) {
                    ctx.take_items(player, CRAFTED_DAGGER, 1);
                    qs.set_cond(ctx, 2, true);
                } else {
                    htmltext = Some(ctx.get_no_quest_msg(player));
                }
            }
            "30829-06.html" => {
                if ctx.get_quest_items_count(player, MAP_PIECE) == MAP_PIECE_COUNT {
                    ctx.take_items(player, MAP_PIECE, -1);
                    ctx.give_items(player, MAP, 1);
                    qs.set_cond(ctx, 4, true);
                } else {
                    htmltext = Some("30829-06a.html".to_owned());
                }
            }
            "30097-02.html" => {
                if ctx.has_quest_items(player, MAP) {
                    ctx.take_items(player, MAP, -1);
                    qs.set_cond(ctx, 5, true);
                } else {
                    htmltext = Some("30097-02a.html".to_owned());
                }
            }
            "30829-09.html" => {
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
            ctx.give_items(player, MAP_PIECE, 1);
            if ctx.get_quest_items_count(player, MAP_PIECE) == MAP_PIECE_COUNT {
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
            COOPER => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30829-00.htm"
                    } else {
                        "30829-00a.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        htmltext = if ctx.has_quest_items(player, CRAFTED_DAGGER) {
                            "30829-02.html"
                        } else {
                            "30829-02a.html"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "30829-04.html".to_owned(),
                    3 => htmltext = "30829-05.html".to_owned(),
                    4 => htmltext = "30829-07.html".to_owned(),
                    5 => htmltext = "30829-08.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            GALLADUCCI => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        4 => htmltext = "30097-01.html".to_owned(),
                        5 => htmltext = "30097-03.html".to_owned(),
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

    const NAME: &str = "Q00043_HelpTheSister";

    const NPCS: &str = r#"
        <npc id="30829" type="Folk" level="70" name="Cooper"/>
        <npc id="30097" type="Folk" level="70" name="Galladucci"/>
        <npc id="20171" type="Monster" level="30" name="Specter"/>
        <npc id="20197" type="Monster" level="30" name="Sorrow Maiden"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="220" type="Weapon" name="Crafted Dagger"><set name="bodypart" val="rhand"/></item>
        <item id="7550" type="EtcItem" name="Map Piece"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7551" type="EtcItem" name="Map"><set name="is_questitem" val="true"/></item>
        <item id="7584" type="EtcItem" name="Pet Ticket"/>"#;

    struct Npcs {
        cooper: Oid,
        galladucci: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00043HelpTheSister);
        let pages = Pages::of(&Q00043HelpTheSister)
            .page("30829-00.htm", &["30829-01.htm"])
            .page("30829-02.html", &["30829-03.html"])
            .page("30829-05.html", &["30829-06.html"])
            .page("30829-08.html", &["30829-09.html"])
            .page("30097-01.html", &["30097-02.html"])
            .plain(&[
                "30829-00a.html",
                "30829-01.htm",
                "30829-02a.html",
                "30829-03.html",
                "30829-04.html",
                "30829-06.html",
                "30829-06a.html",
                "30829-07.html",
                "30829-09.html",
                "30097-02.html",
                "30097-02a.html",
                "30097-03.html",
            ])
            .npcs(&[COOPER, GALLADUCCI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            cooper: t.spawn(COOPER, 20),
            galladucci: t.spawn(GALLADUCCI, 40),
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
            out = Q00043HelpTheSister.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills sorrow maidens until the player has 30 map pieces; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..200 {
            let maiden = t.spawn(SORROW_MAIDEN, 80);
            t.kill(1, maiden);
            if t.count(1, MAP_PIECE) >= MAP_PIECE_COUNT {
                return kills;
            }
        }
        panic!("no 30 map pieces after 200 kills");
    }

    #[test]
    fn a_level_25_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 25, vec![]);
        assert!(t.quest_page(1, npcs.cooper).contains("30829-00a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_dagger_the_map_pieces_the_map_and_the_ticket() {
        let (mut t, npcs) = setup();
        t.enter(1, 26, vec![item(1001, CRAFTED_DAGGER, 1)]);
        assert!(t.quest_page(1, npcs.cooper).contains("30829-00.htm"));
        assert!(t.event(1, NAME, "30829-01.htm").contains("30829-01.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // With the dagger: the dagger goes, cond 2.
        assert!(t.quest_page(1, npcs.cooper).contains("30829-02.html"));
        assert!(t.event(1, NAME, "30829-03.html").contains("30829-03.html"));
        assert_eq!(t.count(1, CRAFTED_DAGGER), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.cooper).contains("30829-04.html"));

        // The sorrow maidens give the map pieces; the thirtieth sets cond 3.
        hunt(&mut t);
        assert_eq!(t.count(1, MAP_PIECE), 30);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Cooper takes the 30 pieces: the map, cond 4.
        assert!(t.quest_page(1, npcs.cooper).contains("30829-05.html"));
        assert!(t.event(1, NAME, "30829-06.html").contains("30829-06.html"));
        assert_eq!((t.count(1, MAP_PIECE), t.count(1, MAP)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Galladucci takes the map: cond 5.
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-01.html"));
        assert!(t.event(1, NAME, "30097-02.html").contains("30097-02.html"));
        assert_eq!(t.count(1, MAP), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.galladucci).contains("30097-03.html"));

        // Cooper gives the pet ticket: the end.
        assert!(t.quest_page(1, npcs.cooper).contains("30829-08.html"));
        assert!(t.event(1, NAME, "30829-09.html").contains("30829-09.html"));
        assert_eq!(t.count(1, PET_TICKET), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.cooper)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_refusals_without_the_dagger_or_the_pieces() {
        let (mut t, npcs) = setup();
        t.enter(1, 26, vec![]);
        t.quest_page(1, npcs.cooper);
        t.event(1, NAME, "30829-01.htm");
        t.take_writes();

        // No dagger: the no-quest page (not the dagger page), cond stays 1.
        assert!(t.quest_page(1, npcs.cooper).contains("30829-02a.html"));
        assert!(
            direct(&mut t, "30829-03.html")
                .unwrap()
                .contains("not on a quest")
        );
        assert!(t.take_writes().is_empty());

        // 29 pieces at the map: the page for it, nothing taken.
        set_cond(&mut t, 3);
        assert_eq!(direct(&mut t, "30829-06.html").as_deref(), Some("30829-06a.html"));
        assert_eq!(t.count(1, MAP), 0);
        assert!(t.take_writes().is_empty());

        // No map for Galladucci: the page for it.
        set_cond(&mut t, 4);
        assert_eq!(direct(&mut t, "30097-02.html").as_deref(), Some("30097-02a.html"));
        assert!(t.take_writes().is_empty());

        // A kill at cond 1 gives nothing (the hunt starts at cond 2).
        let specter = t.spawn(SPECTER, 80);
        set_cond(&mut t, 1);
        t.kill(1, specter);
        assert_eq!(t.count(1, MAP_PIECE), 0);
        assert!(names_to(&t.take(), 1).iter().all(|n| n != "PLAY_SOUND"));
    }
}
