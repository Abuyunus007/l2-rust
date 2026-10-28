//! Quest 124: Meeting the Elroki (Marquez, Mushika, Asamah, Karakawei and Mantarasa on Primeval Isle).
// Java: dist/game/data/scripts/quests/Q00124_MeetingTheElroki/Q00124_MeetingTheElroki.java
//
// Pattern: a chain of five NPCs, one cond step each. `on_event` moves the cond when the page's
// link is clicked and echoes every event (Java `return event`). The reward is paid by the
// start-side NPC's `on_talk` (Asamah at cond 6, when the player holds the egg).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MARQUEZ: i32 = 32113;
const MUSHIKA: i32 = 32114;
const ASAMAH: i32 = 32115;
const KARAKAWEI: i32 = 32117;
const MANTARASA: i32 = 32118;

// Item
const MANTARASA_EGG: i32 = 8778;

// Misc
const MIN_LEVEL: i32 = 75;

/// Java `Q00124_MeetingTheElroki`.
pub struct Q00124MeetingTheElroki;

impl Script for Q00124MeetingTheElroki {
    fn name(&self) -> &'static str {
        "Q00124_MeetingTheElroki"
    }

    fn path(&self) -> &'static str {
        "quests/Q00124_MeetingTheElroki"
    }

    fn id(&self) -> i32 {
        124
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MARQUEZ]);
        s.add_talk_id(&[MARQUEZ, MUSHIKA, ASAMAH, KARAKAWEI, MANTARASA]);
        s.register_quest_items(&[MANTARASA_EGG]);
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

        match event {
            "32113-03.html" => qs.start_quest(ctx),
            "32113-04.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                }
            }
            "32114-04.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                }
            }
            "32115-06.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                }
            }
            "32117-05.html" => {
                if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, true);
                }
            }
            "32118-04.html" => {
                if qs.is_cond(ctx, 5) {
                    ctx.give_items(player, MANTARASA_EGG, 1);
                    qs.set_cond(ctx, 6, true);
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            MARQUEZ => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "32113-01a.htm"
                    } else {
                        "32113-01.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "32113-05.html".to_owned(),
                    2 => htmltext = "32113-06.html".to_owned(),
                    3..=5 => htmltext = "32113-07.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            MUSHIKA => {
                if qs.is_started(ctx) {
                    htmltext = match qs.get_cond(ctx) {
                        1 => "32114-01.html",
                        2 => "32114-02.html",
                        _ => "32114-03.html",
                    }
                    .to_owned();
                }
            }
            ASAMAH => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 | 2 => htmltext = "32115-01.html".to_owned(),
                        3 => htmltext = "32115-02.html".to_owned(),
                        4 => htmltext = "32115-07.html".to_owned(),
                        5 => htmltext = "32115-08.html".to_owned(),
                        6 => {
                            if ctx.has_quest_items(player, MANTARASA_EGG) {
                                htmltext = "32115-09.html".to_owned();
                                ctx.give_adena(player, 100_013, true);
                                ctx.add_exp_and_sp(player, 301_922, 30294);
                                qs.exit_quest(ctx, false, true);
                            }
                        }
                        _ => {}
                    }
                }
            }
            KARAKAWEI => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=3 => htmltext = "32117-01.html".to_owned(),
                        4 => htmltext = "32117-02.html".to_owned(),
                        5 => htmltext = "32117-07.html".to_owned(),
                        6 => htmltext = "32117-06.html".to_owned(),
                        _ => {}
                    }
                }
            }
            MANTARASA => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=4 => htmltext = "32118-01.html".to_owned(),
                        5 => htmltext = "32118-03.html".to_owned(),
                        6 => htmltext = "32118-02.html".to_owned(),
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32113" type="Folk" level="75" name="Marquez"/>
        <npc id="32114" type="Folk" level="75" name="Mushika"/>
        <npc id="32115" type="Folk" level="75" name="Asamah"/>
        <npc id="32117" type="Folk" level="75" name="Karakawei"/>
        <npc id="32118" type="Folk" level="75" name="Mantarasa"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8778" type="EtcItem" name="Mantarasa Egg"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00124_MeetingTheElroki";

    struct Npcs {
        marquez: Oid,
        mushika: Oid,
        asamah: Oid,
        karakawei: Oid,
        mantarasa: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00124MeetingTheElroki);
        let pages = Pages::of(&Q00124MeetingTheElroki)
            .page("32113-01.htm", &["32113-02.htm"])
            .plain(&["32113-01a.htm", "32113-06.html", "32113-07.html", "32114-01.html", "32114-03.html"])
            .page("32113-02.htm", &["32113-03.html"])
            .page("32113-03.html", &["32113-04.html"])
            .plain(&["32113-04.html"])
            .page("32113-05.html", &["32113-04.html"])
            .page("32114-02.html", &["32114-04.html"])
            .plain(&["32114-04.html"])
            .page("32115-01.html", &[])
            .page("32115-02.html", &["32115-03.html"])
            .page("32115-03.html", &["32115-04.html"])
            .page("32115-04.html", &["32115-05.html"])
            .page("32115-05.html", &["32115-06.html"])
            .plain(&["32115-06.html", "32115-07.html", "32115-08.html", "32115-09.html"])
            .page("32117-02.html", &["32117-03.html"])
            .page("32117-03.html", &["32117-04.html", "32117-05.html"])
            .page("32117-04.html", &["32117-03.html"])
            .plain(&["32117-01.html", "32117-05.html", "32117-06.html", "32117-07.html"])
            .plain(&["32118-01.html", "32118-02.html"])
            .page("32118-03.html", &["32118-04.html"])
            .plain(&["32118-04.html"])
            .npcs(&[MARQUEZ, MUSHIKA, ASAMAH, KARAKAWEI, MANTARASA])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            marquez: t.spawn(MARQUEZ, 20),
            mushika: t.spawn(MUSHIKA, 30),
            asamah: t.spawn(ASAMAH, 40),
            karakawei: t.spawn(KARAKAWEI, 50),
            mantarasa: t.spawn(MANTARASA, 60),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, npcs.marquez).contains("32113-01a.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_chain_from_marquez_to_asamahs_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, npcs.marquez).contains("32113-01.htm"));
        assert!(t.event(1, NAME, "32113-02.htm").contains("32113-02.htm"));

        // Accept: `startQuest()` (the event `32113-03.html`).
        assert!(t.event(1, NAME, "32113-03.html").contains("32113-03.html"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.event(1, NAME, "32113-04.html").contains("32113-04.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.marquez).contains("32113-06.html"));

        // Mushika: cond 3.
        assert!(t.quest_page(1, npcs.mushika).contains("32114-02.html"));
        assert!(t.event(1, NAME, "32114-04.html").contains("32114-04.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);

        // Asamah's four pages: cond 4.
        assert!(t.quest_page(1, npcs.asamah).contains("32115-02.html"));
        for page in ["32115-03.html", "32115-04.html", "32115-05.html", "32115-06.html"] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert_eq!(t.take_writes(), [update(NAME, "cond", "4")]);

        // Karakawei: cond 5.
        assert!(t.quest_page(1, npcs.karakawei).contains("32117-02.html"));
        assert!(t.event(1, NAME, "32117-03.html").contains("32117-03.html"));
        assert!(t.event(1, NAME, "32117-05.html").contains("32117-05.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "5")]);

        // Mantarasa gives the egg: cond 6.
        assert!(t.quest_page(1, npcs.mantarasa).contains("32118-03.html"));
        assert!(t.event(1, NAME, "32118-04.html").contains("32118-04.html"));
        assert_eq!(t.count(1, MANTARASA_EGG), 1);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "6")]);

        // Asamah's reward: adena, exp and sp, `exitQuest(false, true)`; the egg goes with it.
        assert!(t.quest_page(1, npcs.asamah).contains("32115-09.html"));
        assert_eq!(t.count(1, 57), 100_013);
        assert_eq!(t.count(1, MANTARASA_EGG), 0);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, npcs.marquez).contains("This quest has already been completed."));
    }
}
