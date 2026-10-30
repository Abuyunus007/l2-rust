//! Quest 10285: Meeting Sirra (Rafforty, Jinia, Kegor and Sirra in the Jinia guild hideout, Freya's steward).
// Java: dist/game/data/scripts/quests/Q10285_MeetingSirra/Q10285_MeetingSirra.java
//
// Pattern: a memo-state quest like Q10286: memo state 1 = the hideout (`ex` is the step inside it,
// 0..5), 2 = out of the hideout, 3 = the Freya castle script is done (set by that instance
// script, not by this quest). Sirra is spawned in the instance of the NPC that talks.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const RAFFORTY: i32 = 32020;
const FREYAS_STEWARD: i32 = 32029;
const JINIA: i32 = 32760;
const KEGOR: i32 = 32761;
const SIRRA: i32 = 32762;
const JINIA2: i32 = 32781;

// Misc
const MIN_LEVEL: i32 = 82;

// Locations (Java `EXIT_LOC`, `FREYA_LOC`)
const EXIT_LOC: (i32, i32, i32) = (113_793, -109_342, -845);
const FREYA_LOC: (i32, i32, i32) = (103_045, -124_361, -2_768);

/// Java `Q10284_AcquisitionOfDivineSword.class.getSimpleName()`.
const Q10284: &str = "Q10284_AcquisitionOfDivineSword";

/// Java `Q10285_MeetingSirra`.
pub struct Q10285MeetingSirra;

impl Script for Q10285MeetingSirra {
    fn name(&self) -> &'static str {
        "Q10285_MeetingSirra"
    }

    fn path(&self) -> &'static str {
        "quests/Q10285_MeetingSirra"
    }

    fn id(&self) -> i32 {
        10285
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAFFORTY]);
        s.add_talk_id(&[RAFFORTY, JINIA, KEGOR, SIRRA, JINIA2, FREYAS_STEWARD]);
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
            "32020-02.htm" => htmltext = Some(event.to_owned()),
            "32020-03.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "32760-02.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 0 {
                    qs.set_int(ctx, "ex", 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-05.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 2 {
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-06.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 2 {
                    // Java `addSpawn(SIRRA, .., npc.getInstanceId())`.
                    let instance = npc.map_or(0, |n| ctx.world().instance_id_of(n.object_id));
                    if let Some(sirra) =
                        ctx.add_spawn(SIRRA, -23_905, -8_790, -5_384, 56_238, false, 0, instance)
                    {
                        ctx.broadcast_npc_say(
                            sirra,
                            ChatType::NpcGeneral,
                            NpcStringId::ThereSNothingYouCanTSayICanTListenToYouAnymore,
                            &[],
                        );
                    }
                    qs.set_int(ctx, "ex", 3);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-09.html" | "32760-10.html" | "32760-11.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 4 {
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-12.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 4 {
                    qs.set_int(ctx, "ex", 5);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-13.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 5 {
                    qs.unset(ctx, "ex");
                    qs.set_memo_state(ctx, 2);
                    // Java: `InstanceManager.getPlayerWorld(player).removeAllowed(player)` then
                    // `setInstanceId(0)`. The hideout's world belongs to the instance script.
                    if let Some(id) = ctx.world().player_instance_world(player).map(|w| w.instance_id)
                        && let Some(world) = ctx.world_mut().instance_world_mut(id)
                    {
                        world.remove_allowed(player);
                    }
                    ctx.world_mut().set_instance_id(player, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-14.html" => {
                if qs.is_memo_state(ctx, 2) {
                    ctx.teleport_player(player, EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2, 0, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "32761-02.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 1 {
                    qs.set_int(ctx, "ex", 2);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32762-02.html" | "32762-03.html" | "32762-04.html" | "32762-05.html" | "32762-06.html"
            | "32762-07.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 3 {
                    htmltext = Some(event.to_owned());
                }
            }
            "32762-08.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 3 {
                    qs.set_int(ctx, "ex", 4);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                    if let Some(npc) = npc {
                        ctx.delete_npc(npc);
                    }
                }
            }
            "32781-02.html" | "32781-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "TELEPORT" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    ctx.teleport_player(player, FREYA_LOC.0, FREYA_LOC.1, FREYA_LOC.2, 0, false);
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
                    htmltext = "32020-05.htm".to_owned();
                }
            }
            CREATED => {
                if npc.id == RAFFORTY {
                    let done = ctx
                        .player_quest_state(player, Q10284)
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && done {
                        "32020-01.htm"
                    } else {
                        "32020-04.htm"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                RAFFORTY => match qs.get_memo_state(ctx) {
                    1 => {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "32020-06.html"
                        } else {
                            "32020-09.html"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "32020-07.html".to_owned(),
                    3 => {
                        ctx.give_adena(player, 283_425, true);
                        ctx.add_exp_and_sp(player, 939_075, 83_855);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "32020-08.html".to_owned();
                    }
                    _ => {}
                },
                JINIA => {
                    if qs.is_memo_state(ctx, 1) {
                        match qs.get_int(ctx, "ex") {
                            0 => htmltext = "32760-01.html".to_owned(),
                            1 => htmltext = "32760-03.html".to_owned(),
                            2 => htmltext = "32760-04.html".to_owned(),
                            3 => htmltext = "32760-07.html".to_owned(),
                            4 => htmltext = "32760-08.html".to_owned(),
                            5 => htmltext = "32760-15.html".to_owned(),
                            _ => {}
                        }
                    }
                }
                KEGOR => {
                    if qs.is_memo_state(ctx, 1) {
                        match qs.get_int(ctx, "ex") {
                            1 => htmltext = "32761-01.html".to_owned(),
                            2 => htmltext = "32761-03.html".to_owned(),
                            3 => htmltext = "32761-04.html".to_owned(),
                            _ => {}
                        }
                    }
                }
                SIRRA => {
                    if qs.is_memo_state(ctx, 1) {
                        let state = qs.get_int(ctx, "ex");
                        if state == 3 {
                            htmltext = "32762-01.html".to_owned();
                        } else if state == 4 {
                            htmltext = "32762-09.html".to_owned();
                        }
                    }
                }
                JINIA2 => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = "32781-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 3) {
                        htmltext = "32781-04.html".to_owned();
                    }
                }
                FREYAS_STEWARD => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = "32029-01.html".to_owned();
                        qs.set_cond(ctx, 8, true);
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
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q10285_MeetingSirra";

    const NPCS: &str = r#"
        <npc id="32020" type="Folk" level="80" name="Rafforty"/>
        <npc id="32029" type="Folk" level="80" name="Freya's Steward"/>
        <npc id="32760" type="Folk" level="80" name="Jinia"/>
        <npc id="32761" type="Folk" level="80" name="Kegor"/>
        <npc id="32762" type="Folk" level="80" name="Sirra"/>
        <npc id="32781" type="Folk" level="80" name="Jinia"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// Stands in for Q10284 (its completion is read through `player_quest_state` only).
    struct DivineSwordStandIn;

    impl Script for DivineSwordStandIn {
        fn name(&self) -> &'static str {
            Q10284
        }

        fn path(&self) -> &'static str {
            "quests/Q10284_AcquisitionOfDivineSword"
        }

        fn id(&self) -> i32 {
            10284
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, ObjectId, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10285MeetingSirra);
        registry.add(DivineSwordStandIn);
        let pages = Pages::of(&Q10285MeetingSirra)
            .page("32020-01.htm", &["32020-02.htm"])
            .page("32020-02.htm", &["32020-03.htm"])
            .plain(&[
                "32020-03.htm",
                "32020-04.htm",
                "32020-05.htm",
                "32020-06.html",
                "32020-08.html",
            ])
            .page("32760-01.html", &["32760-02.html"])
            .plain(&["32760-02.html", "32760-03.html", "32760-04.html", "32760-07.html"])
            .page("32761-01.html", &["32761-02.html"])
            .plain(&["32761-02.html", "32781-01.html", "32029-01.html"])
            .page("32760-04.html", &["32760-05.html"])
            .page("32760-05.html", &["32760-06.html"])
            .plain(&["32760-06.html"])
            .page("32762-01.html", &["32762-02.html"])
            .page("32762-07.html", &["32762-08.html"])
            .plain(&["32762-08.html", "32762-09.html"])
            .npcs(&[RAFFORTY, JINIA, KEGOR, SIRRA, JINIA2, FREYAS_STEWARD])
            .file(
                "data/scripts/quests/Q10285_MeetingSirra/32762-02.html",
                "<html><body>32762-02</body></html>",
            );
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rafforty = t.spawn(RAFFORTY, 20);
        let jinia = t.spawn(JINIA, 30);
        let kegor = t.spawn(KEGOR, 40);
        (t, rafforty, jinia, kegor)
    }

    fn loaded(t: &mut ScriptTest, level: i32, memo: Option<&str>, vars: &[(&str, &str)]) {
        let mut data = player_data(1, level, vec![]);
        let mut states = vec![(Q10284.to_owned(), Some("Completed".to_owned()))];
        let mut variables = vec![];
        if let Some(memo) = memo {
            states.push((NAME.to_owned(), Some("Started".to_owned())));
            variables.push((NAME.to_owned(), "cond".to_owned(), Some("1".to_owned())));
            variables.push((NAME.to_owned(), "memoState".to_owned(), Some(memo.to_owned())));
        }
        for (k, v) in vars {
            variables.push((NAME.to_owned(), (*k).to_owned(), Some((*v).to_owned())));
        }
        data.profile.quests = QuestLoad { states, variables };
        t.enter_with(data);
    }

    #[test]
    fn the_start_needs_82_and_the_divine_sword() {
        let (mut t, rafforty, _, _) = setup();
        loaded(&mut t, 81, None, &[]);
        assert!(t.quest_page(1, rafforty).contains("32020-04.htm"));
        t.enter(2, 82, vec![]);
        assert!(t.quest_page(2, rafforty).contains("32020-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn rafforty_starts_and_jinia_and_kegor_move_it_on() {
        let (mut t, rafforty, jinia, kegor) = setup();
        loaded(&mut t, 82, None, &[]);
        assert!(t.quest_page(1, rafforty).contains("32020-01.htm"));
        t.event(1, NAME, "32020-02.htm");
        assert!(t.event(1, NAME, "32020-03.htm").contains("32020-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.quest_page(1, jinia).contains("32760-01.html"));
        assert!(t.event(1, NAME, "32760-02.html").contains("32760-02.html"));
        assert!(t.quest_page(1, kegor).contains("32761-01.html"));
        assert!(t.event(1, NAME, "32761-02.html").contains("32761-02.html"));
        assert!(t.quest_page(1, jinia).contains("32760-04.html"));
    }

    #[test]
    fn jinia_spawns_sirra() {
        let (mut t, _, jinia, _) = setup();
        loaded(&mut t, 82, Some("1"), &[("ex", "2")]);
        t.quest_page(1, jinia);
        t.event(1, NAME, "32760-05.html");
        assert!(t.event(1, NAME, "32760-06.html").contains("32760-06.html"));
        assert_eq!(t.npcs_of(SIRRA).len(), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "ex", "3")));
    }

    #[test]
    fn rafforty_pays_at_memo_state_3() {
        let (mut t, rafforty, _, _) = setup();
        loaded(&mut t, 82, Some("3"), &[]);
        assert!(t.quest_page(1, rafforty).contains("32020-08.html"));
        assert!(t.count(1, 57) > 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, rafforty).contains("32020-05.htm"));
    }
}
