//! Quest 42: Help the Uncle! (Pet Manager Waters at the Talking Island, Sophya the Uncle's
//! sister in the Gludio side; the eye destroyers give the map pieces).
// Java: dist/game/data/scripts/quests/Q00042_HelpTheUncle/Q00042_HelpTheUncle.java
//
// Pattern: a kill-count quest with a delivery chain (`docs/design/scripts.md`, "Quest patterns"):
// the trident starts the hunt, thirty map pieces make the map, and the map goes to Sophya for the
// ticket at the end.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WATERS: i32 = 30828;
const SOPHYA: i32 = 30735;

// Monsters
const MONSTER_EYE_DESTROYER: i32 = 20068;
const MONSTER_EYE_GAZER: i32 = 20266;

// Items
const TRIDENT: i32 = 291;
const MAP_PIECE: i32 = 7548;
const MAP: i32 = 7549;
const PET_TICKET: i32 = 7583;

// Misc
const MIN_LEVEL: i32 = 25;
const MAP_PIECE_COUNT: i64 = 30;

/// Java `Q00042_HelpTheUncle`.
pub struct Q00042HelpTheUncle;

impl Script for Q00042HelpTheUncle {
    fn name(&self) -> &'static str {
        "Q00042_HelpTheUncle"
    }

    fn path(&self) -> &'static str {
        "quests/Q00042_HelpTheUncle"
    }

    fn id(&self) -> i32 {
        42
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WATERS]);
        s.add_talk_id(&[WATERS, SOPHYA]);
        s.add_kill_id(&[MONSTER_EYE_DESTROYER, MONSTER_EYE_GAZER]);
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
            "30828-01.htm" => {
                qs.start_quest(ctx);
            }
            "30828-03.html" => {
                if ctx.has_quest_items(player, TRIDENT) {
                    ctx.take_items(player, TRIDENT, 1);
                    qs.set_cond(ctx, 2, true);
                } else {
                    htmltext = Some("30828-03a.html".to_owned());
                }
            }
            "30828-06.html" => {
                if ctx.get_quest_items_count(player, MAP_PIECE) == MAP_PIECE_COUNT {
                    ctx.take_items(player, MAP_PIECE, -1);
                    ctx.give_items(player, MAP, 1);
                    qs.set_cond(ctx, 4, true);
                } else {
                    htmltext = Some("30828-06a.html".to_owned());
                }
            }
            "30735-02.html" => {
                if ctx.has_quest_items(player, MAP) {
                    ctx.take_items(player, MAP, -1);
                    qs.set_cond(ctx, 5, true);
                } else {
                    htmltext = Some("30735-02a.html".to_owned());
                }
            }
            "30828-09.html" => {
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
            WATERS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30828-00.htm"
                    } else {
                        "30828-00a.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        htmltext = if ctx.has_quest_items(player, TRIDENT) {
                            "30828-02.html"
                        } else {
                            "30828-02a.html"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "30828-04.html".to_owned(),
                    3 => htmltext = "30828-05.html".to_owned(),
                    4 => htmltext = "30828-07.html".to_owned(),
                    5 => htmltext = "30828-08.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            SOPHYA => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        4 => htmltext = "30735-01.html".to_owned(),
                        5 => htmltext = "30735-03.html".to_owned(),
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

    const NAME: &str = "Q00042_HelpTheUncle";

    const NPCS: &str = r#"
        <npc id="30828" type="Folk" level="70" name="Pet Manager Waters"/>
        <npc id="30735" type="Folk" level="70" name="Sophya"/>
        <npc id="20068" type="Monster" level="30" name="Monster Eye Destroyer"/>
        <npc id="20266" type="Monster" level="30" name="Monster Eye Gazer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="291" type="Weapon" name="Trident"><set name="bodypart" val="rhand"/></item>
        <item id="7548" type="EtcItem" name="Map Piece"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7549" type="EtcItem" name="Map"><set name="is_questitem" val="true"/></item>
        <item id="7583" type="EtcItem" name="Pet Ticket"/>"#;

    struct Npcs {
        waters: Oid,
        sophya: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00042HelpTheUncle);
        let pages = Pages::of(&Q00042HelpTheUncle)
            .page("30828-00.htm", &["30828-01.htm"])
            .page("30828-02.html", &["30828-03.html"])
            .page("30828-05.html", &["30828-06.html"])
            .page("30828-08.html", &["30828-09.html"])
            .page("30735-01.html", &["30735-02.html"])
            .plain(&[
                "30828-00a.html",
                "30828-01.htm",
                "30828-02a.html",
                "30828-03.html",
                "30828-03a.html",
                "30828-04.html",
                "30828-06.html",
                "30828-06a.html",
                "30828-07.html",
                "30828-09.html",
                "30735-02.html",
                "30735-02a.html",
                "30735-03.html",
            ])
            .npcs(&[WATERS, SOPHYA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            waters: t.spawn(WATERS, 20),
            sophya: t.spawn(SOPHYA, 40),
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
            out = Q00042HelpTheUncle.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills eye destroyers until the player has 30 map pieces; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..200 {
            let eye = t.spawn(MONSTER_EYE_DESTROYER, 80);
            t.kill(1, eye);
            if t.count(1, MAP_PIECE) >= MAP_PIECE_COUNT {
                return kills;
            }
        }
        panic!("no 30 map pieces after 200 kills");
    }

    #[test]
    fn a_level_24_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 24, vec![]);
        assert!(t.quest_page(1, npcs.waters).contains("30828-00a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_trident_the_map_pieces_the_map_and_the_ticket() {
        let (mut t, npcs) = setup();
        t.enter(1, 25, vec![item(1001, TRIDENT, 1)]);
        assert!(t.quest_page(1, npcs.waters).contains("30828-00.htm"));
        assert!(t.event(1, NAME, "30828-01.htm").contains("30828-01.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // With the trident: the trident goes, cond 2.
        assert!(t.quest_page(1, npcs.waters).contains("30828-02.html"));
        assert!(t.event(1, NAME, "30828-03.html").contains("30828-03.html"));
        assert_eq!(t.count(1, TRIDENT), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.waters).contains("30828-04.html"));

        // The eye destroyers give the map pieces; the thirtieth sets cond 3.
        hunt(&mut t);
        assert_eq!(t.count(1, MAP_PIECE), 30);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Waters takes the 30 pieces: the map, cond 4.
        assert!(t.quest_page(1, npcs.waters).contains("30828-05.html"));
        assert!(t.event(1, NAME, "30828-06.html").contains("30828-06.html"));
        assert_eq!((t.count(1, MAP_PIECE), t.count(1, MAP)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Sophya takes the map: cond 5.
        assert!(t.quest_page(1, npcs.sophya).contains("30735-01.html"));
        assert!(t.event(1, NAME, "30735-02.html").contains("30735-02.html"));
        assert_eq!(t.count(1, MAP), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.sophya).contains("30735-03.html"));

        // Waters gives the pet ticket: the end.
        assert!(t.quest_page(1, npcs.waters).contains("30828-08.html"));
        assert!(t.event(1, NAME, "30828-09.html").contains("30828-09.html"));
        assert_eq!(t.count(1, PET_TICKET), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.waters)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_refusals_without_the_trident_or_the_pieces() {
        let (mut t, npcs) = setup();
        t.enter(1, 25, vec![]);
        t.quest_page(1, npcs.waters);
        t.event(1, NAME, "30828-01.htm");
        t.take_writes();

        // No trident: the page for it, cond stays 1.
        assert!(t.quest_page(1, npcs.waters).contains("30828-02a.html"));
        assert_eq!(direct(&mut t, "30828-03.html").as_deref(), Some("30828-03a.html"));
        assert!(t.take_writes().is_empty());

        // 29 pieces at the map: the page for it, nothing taken.
        set_cond(&mut t, 3);
        assert_eq!(direct(&mut t, "30828-06.html").as_deref(), Some("30828-06a.html"));
        assert_eq!(t.count(1, MAP), 0);
        assert!(t.take_writes().is_empty());

        // No map for Sophya: the page for it.
        set_cond(&mut t, 4);
        assert_eq!(direct(&mut t, "30735-02.html").as_deref(), Some("30735-02a.html"));
        assert!(t.take_writes().is_empty());

        // A kill at cond 1 gives nothing (the hunt starts at cond 2).
        let eye = t.spawn(MONSTER_EYE_GAZER, 80);
        set_cond(&mut t, 1);
        t.kill(1, eye);
        assert_eq!(t.count(1, MAP_PIECE), 0);
        assert!(names_to(&t.take(), 1).iter().all(|n| n != "PLAY_SOUND"));
    }
}
