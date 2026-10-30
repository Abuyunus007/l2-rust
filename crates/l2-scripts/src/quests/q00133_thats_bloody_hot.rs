//! Quest 133: That's Bloody Hot! (Kanis and Galate; unlocks Hellbound level 1 when it is locked).
// Java: dist/game/data/scripts/quests/Q00133_ThatsBloodyHot/Q00133_ThatsBloodyHot.java
//
// Pattern: a short talk chain behind Q00131. The reward at the end depends on the Hellbound
// engine (`ai::areas::hellbound::engine`): a locked island is set to level 1 and pays less exp.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine::{is_locked, set_level};

// NPCs
const KANIS: i32 = 32264;
const GALATE: i32 = 32292;

// Item
const REFINED_CRYSTAL_SAMPLE: i32 = 9785;

// Misc
const MIN_LEVEL: i32 = 78;

const BIRD_IN_A_CAGE: &str = "Q00131_BirdInACage";

/// Java `Q00133_ThatsBloodyHot`.
pub struct Q00133ThatsBloodyHot;

impl Script for Q00133ThatsBloodyHot {
    fn name(&self) -> &'static str {
        "Q00133_ThatsBloodyHot"
    }

    fn path(&self) -> &'static str {
        "quests/Q00133_ThatsBloodyHot"
    }

    fn id(&self) -> i32 {
        133
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KANIS]);
        s.add_talk_id(&[KANIS, GALATE]);
        s.register_quest_items(&[REFINED_CRYSTAL_SAMPLE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let st = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32264-04.html" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    st.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-06.html" | "32264-07.html" => {
                if st.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-08.html" => {
                st.set_cond(ctx, 2, false);
                htmltext = Some(event.to_owned());
            }
            "32264-10.html" | "32264-11.html" => {
                if st.is_cond(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32264-12.html" => {
                if st.is_cond(ctx, 2) {
                    ctx.give_items(player, REFINED_CRYSTAL_SAMPLE, 1);
                    st.set_cond(ctx, 3, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "32292-03.html" => {
                if st.is_cond(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32292-05.html" => {
                if st.is_cond(ctx, 3) && ctx.has_quest_items(player, REFINED_CRYSTAL_SAMPLE) {
                    ctx.take_items(player, REFINED_CRYSTAL_SAMPLE, -1);
                    htmltext = Some(event.to_owned());
                    st.set_cond(ctx, 4, false);
                }
            }
            "32292-06.html" => {
                if st.is_cond(ctx, 4) {
                    if !is_locked(ctx) {
                        ctx.give_adena(player, 254_247, true);
                        ctx.add_exp_and_sp(player, 331_457, 32_524);
                        st.exit_quest(ctx, false, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        set_level(ctx, 1);
                        ctx.give_adena(player, 254_247, true);
                        ctx.add_exp_and_sp(player, 325_881, 32_524);
                        st.exit_quest(ctx, false, true);
                        htmltext = Some("32292-07.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let st = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match st.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc.id == KANIS {
                    let done = ctx
                        .player_quest_state(player, BIRD_IN_A_CAGE)
                        .is_some_and(|q| q.is_completed());
                    if done {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "32264-01.htm"
                        } else {
                            "32264-02.html"
                        }
                        .to_owned();
                    } else {
                        htmltext = "32264-03.html".to_owned();
                    }
                }
            }
            STARTED => {
                if npc.id == KANIS {
                    if st.is_cond(ctx, 1) {
                        htmltext = "32264-05.html".to_owned();
                    } else if st.is_cond(ctx, 2) {
                        htmltext = "32264-09.html".to_owned();
                    } else if st.get_cond(ctx) >= 3 {
                        htmltext = "32264-13.html".to_owned();
                    }
                } else if npc.id == GALATE {
                    if st.get_cond(ctx) < 3 {
                        htmltext = "32292-01.html".to_owned();
                    } else if st.is_cond(ctx, 3) {
                        htmltext = "32292-02.html".to_owned();
                    } else if st.is_cond(ctx, 4) {
                        htmltext = "32292-04.html".to_owned();
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
        <npc id="32292" type="Folk" level="70" name="Galate"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9785" type="EtcItem" name="Refined Crystal Sample"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00133_ThatsBloodyHot";

    /// The script of the quest this one reads by name (`player.getQuestState(name)`).
    struct StandIn(&'static str);

    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn(BIRD_IN_A_CAGE));
        registry.add(Q00133ThatsBloodyHot);
        let pages = Pages::of(&Q00133ThatsBloodyHot)
            .page("32264-01.htm", &["32264-04.html"])
            .page("32264-04.html", &["32264-06.html"])
            .page("32264-06.html", &["32264-07.html"])
            .page("32264-07.html", &["32264-08.html"])
            .plain(&["32264-02.html", "32264-03.html", "32264-05.html", "32264-08.html"])
            .page("32264-09.html", &["32264-10.html"])
            .page("32264-10.html", &["32264-11.html"])
            .page("32264-11.html", &["32264-12.html"])
            .plain(&["32264-12.html", "32264-13.html"])
            .plain(&["32292-01.html"])
            .page("32292-02.html", &["32292-03.html"])
            .page("32292-03.html", &["32292-05.html"])
            .page("32292-04.html", &["32292-06.html"])
            .plain(&["32292-05.html", "32292-06.html", "32292-07.html"])
            .npcs(&[KANIS, GALATE])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kanis = t.spawn(KANIS, 20);
        let galate = t.spawn(GALATE, 40);
        (t, kanis, galate)
    }

    fn finish_bird_in_a_cage(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(BIRD_IN_A_CAGE, |ctx| {
            let q = ctx.get_quest_state(Oid(player), true).unwrap();
            q.start_quest(ctx);
            q.exit_quest(ctx, false, false);
        });
        t.take_writes();
    }

    #[test]
    fn quest_131_and_level_78_gate_the_start() {
        let (mut t, kanis, _) = setup();
        t.enter(1, 78, vec![]);
        assert!(t.quest_page(1, kanis).contains("32264-03.html"));
        finish_bird_in_a_cage(&mut t, 1);
        assert!(t.quest_page(1, kanis).contains("32264-01.htm"));
        t.enter(2, 77, vec![]);
        finish_bird_in_a_cage(&mut t, 2);
        assert!(t.quest_page(2, kanis).contains("32264-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_on_a_locked_island_sets_hellbound_level_1() {
        let (mut t, kanis, galate) = setup();
        t.enter(1, 78, vec![]);
        finish_bird_in_a_cage(&mut t, 1);
        assert!(t.quest_page(1, kanis).contains("32264-01.htm"));
        assert!(t.event(1, NAME, "32264-04.html").contains("32264-04.html"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.event(1, NAME, "32264-06.html").contains("32264-06.html"));
        assert!(t.event(1, NAME, "32264-07.html").contains("32264-07.html"));
        assert!(t.event(1, NAME, "32264-08.html").contains("32264-08.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, kanis).contains("32264-09.html"));
        for page in ["32264-10.html", "32264-11.html", "32264-12.html"] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert_eq!(t.count(1, REFINED_CRYSTAL_SAMPLE), 1);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);

        assert!(t.quest_page(1, galate).contains("32292-02.html"));
        assert!(t.event(1, NAME, "32292-03.html").contains("32292-03.html"));
        assert!(t.event(1, NAME, "32292-05.html").contains("32292-05.html"));
        assert_eq!(t.count(1, REFINED_CRYSTAL_SAMPLE), 0);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "4")]);

        // Hellbound is locked (level 0): the quest sets level 1 and answers with page 07.
        assert!(t.quest_page(1, galate).contains("32292-04.html"));
        assert!(t.event(1, NAME, "32292-06.html").contains("32292-07.html"));
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(crate::ai::areas::hellbound::engine::level(ctx), 1);
        });
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }
}
