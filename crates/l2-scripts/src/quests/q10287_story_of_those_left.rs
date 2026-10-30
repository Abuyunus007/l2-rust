//! Quest 10287: Story of Those Left (Rafforty in the Gludio area, Jinia and Kegor in the Jinia guild hideout).
// Java: dist/game/data/scripts/quests/Q10287_StoryOfThoseLeft/Q10287_StoryOfThoseLeft.java
//
// Pattern: a memo-state quest (`docs/design/scripts.md`): the memo state tells the three steps
// apart (1 = the hideout, 2 = back from it), `ex1` and `ex2` are the two checks in the hideout
// (Jinia's and Kegor's). The reward is one item of the list the NPC shows (`exitQuest(false)`).
// The hideout's instance (`JiniaGuildHideout4`) is another script; this one only leaves it.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const RAFFORTY: i32 = 32020;
const JINIA: i32 = 32760;
const KEGOR: i32 = 32761;

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `Q10286_ReunionWithSirra.class.getSimpleName()`.
const REUNION_WITH_SIRRA: &str = "Q10286_ReunionWithSirra";

// Location (Java `EXIT_LOC`)
const EXIT_LOC: (i32, i32, i32) = (113_793, -109_342, -845);

/// Java `Q10287_StoryOfThoseLeft`.
pub struct Q10287StoryOfThoseLeft;

impl Script for Q10287StoryOfThoseLeft {
    fn name(&self) -> &'static str {
        "Q10287_StoryOfThoseLeft"
    }

    fn path(&self) -> &'static str {
        "quests/Q10287_StoryOfThoseLeft"
    }

    fn id(&self) -> i32 {
        10287
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAFFORTY]);
        s.add_talk_id(&[RAFFORTY, JINIA, KEGOR]);
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
            "32020-02.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "32020-08.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_int(ctx, "ex1", 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-06.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_cond(ctx, 5, true);
                    ctx.teleport_player(player, EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2, 0, false);
                    // TODO: missing "jinia_npc_q10287_06.htm" (Java comment).
                    htmltext = Some(event.to_owned());
                }
            }
            "32761-02.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex1") == 1 && qs.get_int(ctx, "ex2") == 0 {
                    htmltext = Some(event.to_owned());
                }
            }
            "32761-03.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex1") == 1 && qs.get_int(ctx, "ex2") == 0 {
                    qs.set_int(ctx, "ex2", 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "10549" | "10550" | "10551" | "10552" | "10553" | "14219" => {
                if qs.is_memo_state(ctx, 2) {
                    // Java `rewardItems(player, Integer.parseInt(event), 1)`: the event is the item id.
                    ctx.reward_items(player, event.parse::<i32>().unwrap_or_default(), 1);
                    htmltext = Some("32020-09.html".to_owned());
                    qs.exit_quest(ctx, false, true);
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
                    htmltext = "32020-04.html".to_owned();
                }
            }
            CREATED => {
                if npc.id == RAFFORTY {
                    let reunion_done = ctx
                        .player_quest_state(player, REUNION_WITH_SIRRA)
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && reunion_done {
                        "32020-01.htm"
                    } else {
                        "32020-03.htm"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                RAFFORTY => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "32020-05.html"
                        } else {
                            "32020-06.html"
                        }
                        .to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "32020-07.html".to_owned();
                    }
                }
                JINIA => {
                    if qs.is_memo_state(ctx, 1) {
                        let state1 = qs.get_int(ctx, "ex1");
                        let state2 = qs.get_int(ctx, "ex2");
                        if state1 == 0 && state2 == 0 {
                            htmltext = "32760-01.html".to_owned();
                        } else if state1 == 1 && state2 == 0 {
                            htmltext = "32760-04.html".to_owned();
                        } else if state1 == 1 && state2 == 1 {
                            qs.set_cond(ctx, 5, true);
                            qs.set_memo_state(ctx, 2);
                            qs.unset(ctx, "ex1");
                            qs.unset(ctx, "ex2");
                            // Java: `InstanceManager.getPlayerWorld(player).removeAllowed(player)`
                            // then `setInstanceId(0)`. The world of the hideout is the instance
                            // script's; without one there is nothing to leave.
                            if let Some(id) = ctx.world().player_instance_world(player).map(|w| w.instance_id)
                                && let Some(world) = ctx.world_mut().instance_world_mut(id)
                            {
                                world.remove_allowed(player);
                            }
                            ctx.world_mut().set_instance_id(player, 0);
                            htmltext = "32760-05.html".to_owned();
                        }
                    }
                }
                KEGOR => {
                    if qs.is_memo_state(ctx, 1) {
                        let state1 = qs.get_int(ctx, "ex1");
                        let state2 = qs.get_int(ctx, "ex2");
                        if state1 == 1 && state2 == 0 {
                            htmltext = "32761-01.html".to_owned();
                        } else if state1 == 0 && state2 == 0 {
                            htmltext = "32761-04.html".to_owned();
                        } else if state1 == 1 && state2 == 1 {
                            htmltext = "32761-05.html".to_owned();
                        }
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
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q10287_StoryOfThoseLeft";

    const NPCS: &str = r#"
        <npc id="32020" type="Folk" level="80" name="Rafforty"/>
        <npc id="32760" type="Folk" level="80" name="Jinia"/>
        <npc id="32761" type="Folk" level="80" name="Kegor"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10549" type="EtcItem" name="Reward Item"><set name="is_stackable" val="true"/></item>"#;

    /// Stands in for Q10286 (its completion is read through `player_quest_state` only).
    struct ReunionStandIn;

    impl Script for ReunionStandIn {
        fn name(&self) -> &'static str {
            REUNION_WITH_SIRRA
        }

        fn path(&self) -> &'static str {
            "quests/Q10286_ReunionWithSirra"
        }

        fn id(&self) -> i32 {
            10286
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10287StoryOfThoseLeft);
        registry.add(ReunionStandIn);
        let pages = Pages::of(&Q10287StoryOfThoseLeft)
            .page("32020-01.htm", &["32020-02.htm"])
            .plain(&[
                "32020-02.htm",
                "32020-03.htm",
                "32020-04.html",
                "32020-05.html",
                "32020-06.html",
            ])
            .page("32020-07.html", &["32020-08.html"])
            .page(
                "32020-08.html",
                &["10549", "10550", "10551", "10552", "10553", "14219"],
            )
            .plain(&["32020-09.html"])
            .page("32760-01.html", &["32760-02.html"])
            .page("32760-02.html", &["32760-03.html"])
            .plain(&["32760-03.html", "32760-04.html"])
            .page("32760-05.html", &["32760-06.html"])
            .plain(&["32760-06.html"])
            .page("32761-01.html", &["32761-02.html"])
            .page("32761-02.html", &["32761-03.html"])
            .plain(&["32761-03.html", "32761-04.html", "32761-05.html"])
            .npcs(&[RAFFORTY, JINIA, KEGOR]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rafforty = t.spawn(RAFFORTY, 20);
        let jinia = t.spawn(JINIA, 30);
        let kegor = t.spawn(KEGOR, 40);
        (t, rafforty, jinia, kegor)
    }

    /// A character that has finished Reunion with Sirra (the start condition of this quest).
    fn reunion_done(t: &mut ScriptTest, id: i32, level: i32) {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(REUNION_WITH_SIRRA.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
    }

    #[test]
    fn the_start_needs_82_and_reunion_with_sirra() {
        let (mut t, rafforty, _, _) = setup();
        reunion_done(&mut t, 1, 81);
        assert!(t.quest_page(1, rafforty).contains("32020-03.htm"));
        t.enter(2, 82, vec![]);
        assert!(t.quest_page(2, rafforty).contains("32020-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hideout_checks_and_the_reward() {
        let (mut t, rafforty, jinia, kegor) = setup();
        reunion_done(&mut t, 1, 82);
        assert!(t.quest_page(1, rafforty).contains("32020-01.htm"));
        assert!(t.event(1, NAME, "32020-02.htm").contains("32020-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Jinia's first check, then Kegor's.
        assert!(t.quest_page(1, jinia).contains("32760-01.html"));
        assert!(t.event(1, NAME, "32760-02.html").contains("32760-02.html"));
        assert!(t.event(1, NAME, "32760-03.html").contains("32760-03.html"));
        // cond 1 -> 3 skips step 2: Java also records the skipped steps (`__compltdStateFlags`).
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "ex1", "1"),
                rows::update(NAME, "cond", "3"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483643")
            ]
        );
        assert!(t.quest_page(1, jinia).contains("32760-04.html"));
        assert!(t.quest_page(1, kegor).contains("32761-01.html"));
        assert!(t.event(1, NAME, "32761-02.html").contains("32761-02.html"));
        assert!(t.event(1, NAME, "32761-03.html").contains("32761-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "ex2", "1"),
                rows::update(NAME, "cond", "4"),
                rows::update(NAME, "__compltdStateFlags", "-2147483635")
            ]
        );
        assert!(t.quest_page(1, kegor).contains("32761-05.html"));

        // Back to Jinia: cond 5, the memo state 2, the checks go; she sends the player out.
        assert!(t.quest_page(1, jinia).contains("32760-05.html"));
        assert!(t.event(1, NAME, "32760-06.html").contains("32760-06.html"));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (113_793, -109_342));

        // The player is out at the exit point: Rafforty is talked to there. Rafforty pays one of
        // the items of the list; the quest ends.
        let rafforty_outside = t
            .world
            .spawn_npc_at(RAFFORTY, 113_793 + 20, -109_342, -845, 0)
            .unwrap();
        assert!(t.quest_page(1, rafforty_outside).contains("32020-07.html"));
        assert!(t.event(1, NAME, "32020-08.html").contains("32020-08.html"));
        assert!(t.event(1, NAME, "10549").contains("32020-09.html"));
        assert_eq!(t.count(1, 10549), 1);
        assert!(t.quest_page(1, rafforty_outside).contains("32020-04.html"));
    }
}
