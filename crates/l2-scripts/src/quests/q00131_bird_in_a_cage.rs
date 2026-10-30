//! Quest 131: Bird in a Cage (Kanis and Parme, who sends the player out of the instance).
// Java: dist/game/data/scripts/quests/Q00131_BirdInACage/Q00131_BirdInACage.java
//
// Pattern: a talk chain; Parme (cond 3) teleports the player to the instance exit
// (`setInstanceId(0)` + `teleToLocation`). The reward is one of four fire stones.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KANIS: i32 = 32264;
const PARME: i32 = 32271;

// Items
const ECHO_CRYSTAL_OF_FREE_THOUGHT: i32 = 9783;
const PARMES_LETTER: i32 = 9784;
const FIRE_STONE: i32 = 9546;

// Locations
const INSTANCE_EXIT: (i32, i32, i32) = (143_281, 148_843, -12_004);

// Misc
const MIN_LEVEL: i32 = 78;

/// Java `Q00131_BirdInACage`.
pub struct Q00131BirdInACage;

impl Script for Q00131BirdInACage {
    fn name(&self) -> &'static str {
        "Q00131_BirdInACage"
    }

    fn path(&self) -> &'static str {
        "quests/Q00131_BirdInACage"
    }

    fn id(&self) -> i32 {
        131
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KANIS]);
        s.add_talk_id(&[KANIS, PARME]);
        s.register_quest_items(&[ECHO_CRYSTAL_OF_FREE_THOUGHT, PARMES_LETTER]);
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
            "32264-04.html" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-06.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-07.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-09.html" | "32264-10.html" | "32264-11.html" => {
                if qs.is_cond(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-12.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.give_items(player, ECHO_CRYSTAL_OF_FREE_THOUGHT, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-14.html" | "32264-15.html" => {
                if qs.is_cond(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-17.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, PARMES_LETTER) {
                    ctx.take_items(player, PARMES_LETTER, -1);
                    qs.set_cond(ctx, 5, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-19.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, ECHO_CRYSTAL_OF_FREE_THOUGHT) {
                    ctx.add_exp_and_sp(player, 250_677, 25_019);
                    let stone = FIRE_STONE + ctx.get_random(4);
                    ctx.give_items(player, stone, 4);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32271-03.html" => {
                if qs.is_cond(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32271-04.html" => {
                if qs.is_cond(ctx, 3) {
                    ctx.give_items(player, PARMES_LETTER, 1);
                    qs.set_cond(ctx, 4, true);
                    ctx.world_mut().set_instance_id(player, 0);
                    let instance_id = ctx.world().instance_id_of(player);
                    // `teleToLocation(loc, true)` keeps the instance the player has now.
                    let (x, y, z) = INSTANCE_EXIT;
                    ctx.teleport_player(player, x, y, z, instance_id, true);
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
                if npc.id == KANIS {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "32264-01.htm"
                    } else {
                        "32264-02.html"
                    }
                    .to_owned();
                }
            }
            STARTED => {
                if npc.id == KANIS {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "32264-05.html".to_owned(),
                        2 => htmltext = "32264-08.html".to_owned(),
                        3 => htmltext = "32264-13.html".to_owned(),
                        4 => htmltext = "32264-16.html".to_owned(),
                        5 => htmltext = "32264-18.html".to_owned(),
                        _ => {}
                    }
                } else if npc.id == PARME {
                    if qs.get_cond(ctx) < 3 {
                        htmltext = "32271-01.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        htmltext = "32271-02.html".to_owned();
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
        <npc id="32264" type="Folk" level="70" name="Kanis"/>
        <npc id="32271" type="Folk" level="70" name="Parme"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9783" type="EtcItem" name="Echo Crystal"><set name="is_questitem" val="true"/></item>
        <item id="9784" type="EtcItem" name="Parme's Letter"><set name="is_questitem" val="true"/></item>
        <item id="9546" type="EtcItem" name="Fire Stone"><set name="is_stackable" val="true"/></item>
        <item id="9547" type="EtcItem" name="Water Stone"><set name="is_stackable" val="true"/></item>
        <item id="9548" type="EtcItem" name="Earth Stone"><set name="is_stackable" val="true"/></item>
        <item id="9549" type="EtcItem" name="Wind Stone"><set name="is_stackable" val="true"/></item>"#;

    const NAME: &str = "Q00131_BirdInACage";

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00131BirdInACage);
        let pages = Pages::of(&Q00131BirdInACage)
            .page("32264-01.htm", &["32264-04.html"])
            .page("32264-04.html", &["32264-06.html"])
            .page("32264-05.html", &["32264-06.html"])
            .page("32264-06.html", &["32264-07.html"])
            .plain(&["32264-02.html", "32264-07.html", "32264-08.html"])
            .page("32264-08.html", &["32264-09.html"])
            .page("32264-09.html", &["32264-10.html"])
            .page("32264-10.html", &["32264-11.html"])
            .page("32264-11.html", &["32264-12.html"])
            .plain(&["32264-12.html", "32264-13.html"])
            .page("32264-16.html", &["32264-17.html"])
            .page("32264-17.html", &["32264-19.html"])
            .plain(&["32264-18.html", "32264-19.html", "32271-01.html"])
            .page("32271-02.html", &["32271-03.html"])
            .page("32271-03.html", &["32271-04.html"])
            .plain(&["32271-04.html"])
            .npcs(&[KANIS, PARME])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kanis = t.spawn(KANIS, 20);
        let parme = t.spawn(PARME, 40);
        (t, kanis, parme)
    }

    #[test]
    fn kanis_gates_level_78() {
        let (mut t, kanis, _) = setup();
        t.enter(1, 77, vec![]);
        assert!(t.quest_page(1, kanis).contains("32264-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_to_the_fire_stones() {
        let (mut t, kanis, parme) = setup();
        t.enter(1, 78, vec![]);
        assert!(t.quest_page(1, kanis).contains("32264-01.htm"));
        assert!(t.event(1, NAME, "32264-04.html").contains("32264-04.html"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.event(1, NAME, "32264-06.html").contains("32264-06.html"));
        assert!(t.event(1, NAME, "32264-07.html").contains("32264-07.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);

        // Kanis hands out the echo crystal: cond 3.
        assert!(t.quest_page(1, kanis).contains("32264-08.html"));
        for page in ["32264-09.html", "32264-10.html", "32264-11.html", "32264-12.html"] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert_eq!(t.count(1, ECHO_CRYSTAL_OF_FREE_THOUGHT), 1);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);

        // Parme gives the letter (cond 4) and sends the player out of the instance.
        assert!(t.quest_page(1, parme).contains("32271-02.html"));
        assert!(t.event(1, NAME, "32271-03.html").contains("32271-03.html"));
        assert!(t.event(1, NAME, "32271-04.html").contains("32271-04.html"));
        assert_eq!(t.count(1, PARMES_LETTER), 1);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "4")]);

        // The player stands at the instance exit now; back to Kanis.
        assert_eq!(
            t.world.position_of(Oid(1)).map(|p| (p.0 / 1000, p.1 / 1000)),
            Some((143, 148))
        );
        t.world.run_as_script(NAME, |ctx| {
            ctx.teleport_player(
                Oid(1),
                crate::testing::X,
                crate::testing::Y,
                crate::testing::Z,
                0,
                false,
            );
        });
        t.ticks(5);

        // Kanis takes the letter (cond 5) and pays: four of a random fire/water/earth/wind stone.
        assert!(t.quest_page(1, kanis).contains("32264-16.html"));
        assert!(t.event(1, NAME, "32264-17.html").contains("32264-17.html"));
        assert_eq!(t.count(1, PARMES_LETTER), 0);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "5")]);
        assert!(t.event(1, NAME, "32264-19.html").contains("32264-19.html"));
        let stones: i64 = (9546..=9549).map(|id| t.count(1, id)).sum();
        assert_eq!(stones, 4);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }
}
