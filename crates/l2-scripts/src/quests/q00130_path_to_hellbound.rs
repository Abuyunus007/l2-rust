//! Quest 130: Path to Hellbound (Casian and Galate; only while Hellbound is unlocked).
// Java: dist/game/data/scripts/quests/Q00130_PathToHellbound/Q00130_PathToHellbound.java
//
// Pattern: a short talk chain. Casian only offers the quest when the Hellbound engine is not
// locked (`ai::areas::hellbound::engine::is_locked`, level 0 or less).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine::is_locked;

// NPCs
const CASIAN: i32 = 30612;
const GALATE: i32 = 32292;

// Item
const CASIANS_BLUE_CRYSTAL: i32 = 12823;

// Misc
const MIN_LEVEL: i32 = 78;

/// Java `Q00130_PathToHellbound`.
pub struct Q00130PathToHellbound;

impl Script for Q00130PathToHellbound {
    fn name(&self) -> &'static str {
        "Q00130_PathToHellbound"
    }

    fn path(&self) -> &'static str {
        "quests/Q00130_PathToHellbound"
    }

    fn id(&self) -> i32 {
        130
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CASIAN]);
        s.add_talk_id(&[CASIAN, GALATE]);
        s.register_quest_items(&[CASIANS_BLUE_CRYSTAL]);
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
            "30612-04.htm" => htmltext = Some(event.to_owned()),
            "32292-02.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32292-06.html" => {
                if qs.is_cond(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30612-05.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30612-08.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.give_items(player, CASIANS_BLUE_CRYSTAL, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32292-03.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32292-07.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, CASIANS_BLUE_CRYSTAL) {
                    qs.exit_quest(ctx, false, true);
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
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc.id == CASIAN {
                    if !is_locked(ctx) {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "30612-01.htm"
                        } else {
                            "30612-02.html"
                        }
                        .to_owned();
                    } else {
                        htmltext = "30612-03.html".to_owned();
                    }
                }
            }
            STARTED => {
                if npc.id == CASIAN {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30612-06.html".to_owned(),
                        2 => htmltext = "30612-07.html".to_owned(),
                        3 => htmltext = "30612-09.html".to_owned(),
                        _ => {}
                    }
                } else if npc.id == GALATE {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "32292-01.html".to_owned(),
                        2 => htmltext = "32292-04.html".to_owned(),
                        3 => htmltext = "32292-05.html".to_owned(),
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
        <npc id="30612" type="Folk" level="70" name="Casian"/>
        <npc id="32292" type="Folk" level="70" name="Galate"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="12823" type="EtcItem" name="Casian's Blue Crystal"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00130_PathToHellbound";

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00130PathToHellbound);
        let pages = Pages::of(&Q00130PathToHellbound)
            .page("30612-01.htm", &["30612-04.htm"])
            .page("30612-04.htm", &["30612-05.html"])
            .plain(&["30612-02.html", "30612-03.html", "30612-05.html", "30612-06.html"])
            .page("30612-07.html", &["30612-08.html"])
            .plain(&["30612-08.html", "30612-09.html"])
            .page("32292-01.html", &["32292-02.html"])
            .page("32292-02.html", &["32292-03.html"])
            .plain(&["32292-03.html", "32292-04.html"])
            .page("32292-05.html", &["32292-06.html"])
            .page("32292-06.html", &["32292-07.html"])
            .plain(&["32292-07.html"])
            .npcs(&[CASIAN, GALATE])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let casian = t.spawn(CASIAN, 20);
        let galate = t.spawn(GALATE, 40);
        (t, casian, galate)
    }

    fn unlock(t: &mut ScriptTest) {
        t.world
            .run_as_script(NAME, |ctx| ctx.set_global_variable("HBLevel", "1"));
    }

    #[test]
    fn casian_refuses_while_hellbound_is_locked_and_below_level_78() {
        let (mut t, casian, _) = setup();
        t.enter(1, 78, vec![]);
        assert!(t.quest_page(1, casian).contains("30612-03.html"));
        unlock(&mut t);
        t.enter(2, 77, vec![]);
        assert!(t.quest_page(2, casian).contains("30612-02.html"));
        assert!(t.quest_page(1, casian).contains("30612-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_to_galates_exit() {
        let (mut t, casian, galate) = setup();
        unlock(&mut t);
        t.enter(1, 78, vec![]);
        assert!(t.quest_page(1, casian).contains("30612-01.htm"));
        assert!(t.event(1, NAME, "30612-04.htm").contains("30612-04.htm"));
        assert!(t.event(1, NAME, "30612-05.html").contains("30612-05.html"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );

        // Galate: cond 2.
        assert!(t.quest_page(1, galate).contains("32292-01.html"));
        assert!(t.event(1, NAME, "32292-02.html").contains("32292-02.html"));
        assert!(t.event(1, NAME, "32292-03.html").contains("32292-03.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);

        // Casian hands out the crystal: cond 3.
        assert!(t.quest_page(1, casian).contains("30612-07.html"));
        assert!(t.event(1, NAME, "30612-08.html").contains("30612-08.html"));
        assert_eq!(t.count(1, CASIANS_BLUE_CRYSTAL), 1);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);

        // Galate ends the quest (no reward in Java).
        assert!(t.quest_page(1, galate).contains("32292-05.html"));
        assert!(t.event(1, NAME, "32292-06.html").contains("32292-06.html"));
        assert!(t.event(1, NAME, "32292-07.html").contains("32292-07.html"));
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(
            t.quest_page(1, casian)
                .contains("This quest has already been completed.")
        );
    }
}
