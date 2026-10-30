//! Quest 10286: Reunion with Sirra (Rafforty in the Gludio area, Jinia, Sirra in the Jinia guild hideout).
// Java: dist/game/data/scripts/quests/Q10286_ReunionWithSirra/Q10286_ReunionWithSirra.java
//
// Pattern: a memo-state quest (`docs/design/scripts.md`): the memo state is the step (1 = the
// hideout, 2 = back from it, 10 = the last step, set by the Ice Queen's castle instance script),
// `ex` is the check inside the hideout (1 = Sirra was spawned, 2 = Sirra gave the cores). Sirra
// is spawned in the instance of Jinia's NPC; the quest only leaves that instance.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const RAFFORTY: i32 = 32020;
const JINIA: i32 = 32760;
const SIRRA: i32 = 32762;
const JINIA2: i32 = 32781;

// Item
const BLACK_FROZEN_CORE: i32 = 15470;

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `Q10285_MeetingSirra.class.getSimpleName()`.
const MEETING_SIRRA: &str = "Q10285_MeetingSirra";

// Location (Java `EXIT_LOC`)
const EXIT_LOC: (i32, i32, i32) = (113_793, -109_342, -845);

/// Java `Q10286_ReunionWithSirra`.
pub struct Q10286ReunionWithSirra;

impl Script for Q10286ReunionWithSirra {
    fn name(&self) -> &'static str {
        "Q10286_ReunionWithSirra"
    }

    fn path(&self) -> &'static str {
        "quests/Q10286_ReunionWithSirra"
    }

    fn id(&self) -> i32 {
        10286
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAFFORTY]);
        s.add_talk_id(&[RAFFORTY, JINIA, SIRRA, JINIA2]);
        s.register_quest_items(&[BLACK_FROZEN_CORE]);
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
            "32020-02.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "32020-03.html" | "32760-02.html" | "32760-03.html" | "32760-04.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-05.html" => {
                if qs.is_memo_state(ctx, 1) {
                    // Java `addSpawn(SIRRA, .., npc.getInstanceId())`: Sirra appears in the
                    // instance of the NPC that talks, and says her line to it.
                    let instance = npc.map_or(0, |n| ctx.world().instance_id_of(n.object_id));
                    if let Some(sirra) =
                        ctx.add_spawn(SIRRA, -23_905, -8_790, -5_384, 56_238, false, 0, instance)
                    {
                        ctx.broadcast_npc_say(
                            sirra,
                            ChatType::NpcGeneral,
                            NpcStringId::YouAdvancedBravelyButGotSuchATinyResultHohoho,
                            &[],
                        );
                    }
                    qs.set_int(ctx, "ex", 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-07.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 2 {
                    qs.unset(ctx, "ex");
                    qs.set_memo_state(ctx, 2);
                    // Java: `InstanceManager.getPlayerWorld(player).removeAllowed(player)` then
                    // `setInstanceId(0)`. The hideout's world is the instance script's.
                    if let Some(id) = ctx.world().player_instance_world(player).map(|w| w.instance_id)
                        && let Some(world) = ctx.world_mut().instance_world_mut(id)
                    {
                        world.remove_allowed(player);
                    }
                    ctx.world_mut().set_instance_id(player, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-08.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_cond(ctx, 5, true);
                    ctx.teleport_player(player, EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2, 0, false);
                    // TODO: missing "jinia_npc_q10286_10.htm" (Java comment).
                    htmltext = Some(event.to_owned());
                }
            }
            "32762-02.html" | "32762-03.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 1 {
                    htmltext = Some(event.to_owned());
                }
            }
            "32762-04.html" => {
                if qs.is_memo_state(ctx, 1) && qs.get_int(ctx, "ex") == 1 {
                    if !ctx.has_quest_items(player, BLACK_FROZEN_CORE) {
                        ctx.give_items(player, BLACK_FROZEN_CORE, 5);
                    }
                    qs.set_int(ctx, "ex", 2);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32781-02.html" | "32781-03.html" => {
                if qs.is_memo_state(ctx, 2) {
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
                    htmltext = "32020-05.html".to_owned();
                }
            }
            CREATED => {
                if npc.id == RAFFORTY {
                    let meeting_done = ctx
                        .player_quest_state(player, MEETING_SIRRA)
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && meeting_done {
                        "32020-01.htm"
                    } else {
                        "32020-04.htm"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                RAFFORTY => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "32020-06.html"
                        } else {
                            "32020-08.html"
                        }
                        .to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "32020-07.html".to_owned();
                    }
                }
                JINIA => {
                    if qs.is_memo_state(ctx, 1) {
                        match qs.get_int(ctx, "ex") {
                            0 => htmltext = "32760-01.html".to_owned(),
                            1 => htmltext = "32760-05.html".to_owned(),
                            2 => htmltext = "32760-06.html".to_owned(),
                            _ => {}
                        }
                    }
                }
                SIRRA => {
                    if qs.is_memo_state(ctx, 1) {
                        let state = qs.get_int(ctx, "ex");
                        if state == 1 {
                            htmltext = "32762-01.html".to_owned();
                        } else if state == 2 {
                            htmltext = "32762-05.html".to_owned();
                        }
                    }
                }
                JINIA2 => {
                    // NOTE(java-quirk): memo state 10 is set by the Ice Queen's castle instance
                    // script (`IceQueensCastleBattle`), not by this quest.
                    if qs.is_memo_state(ctx, 10) {
                        ctx.add_exp_and_sp(player, 2_152_200, 181_070);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "32781-01.html".to_owned();
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

    const NAME: &str = "Q10286_ReunionWithSirra";

    const NPCS: &str = r#"
        <npc id="32020" type="Folk" level="80" name="Rafforty"/>
        <npc id="32760" type="Folk" level="80" name="Jinia"/>
        <npc id="32762" type="Folk" level="80" name="Sirra"/>
        <npc id="32781" type="Folk" level="80" name="Jinia"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15470" type="EtcItem" name="Black Frozen Core"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// Stands in for Q10285 (its completion is read through `player_quest_state` only).
    struct MeetingSirraStandIn;

    impl Script for MeetingSirraStandIn {
        fn name(&self) -> &'static str {
            MEETING_SIRRA
        }

        fn path(&self) -> &'static str {
            "quests/Q10285_MeetingSirra"
        }

        fn id(&self) -> i32 {
            10285
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10286ReunionWithSirra);
        registry.add(MeetingSirraStandIn);
        let pages = Pages::of(&Q10286ReunionWithSirra)
            .page("32020-01.htm", &["32020-02.htm"])
            .page("32020-02.htm", &["32020-03.html"])
            .plain(&[
                "32020-03.html",
                "32020-04.htm",
                "32020-05.html",
                "32020-06.html",
                "32020-07.html",
                "32020-08.html",
            ])
            .page("32760-01.html", &["32760-02.html"])
            .page("32760-02.html", &["32760-03.html"])
            .page("32760-03.html", &["32760-04.html"])
            .page("32760-04.html", &["32760-05.html"])
            .plain(&["32760-05.html"])
            .page("32760-06.html", &["32760-07.html"])
            .page("32760-07.html", &["32760-08.html"])
            .plain(&["32760-08.html"])
            .page("32762-01.html", &["32762-02.html"])
            .page("32762-02.html", &["32762-03.html"])
            .page("32762-03.html", &["32762-04.html"])
            .plain(&["32762-04.html", "32762-05.html"])
            .plain(&["32781-01.html", "32781-02.html", "32781-03.html"])
            .npcs(&[RAFFORTY, JINIA, SIRRA, JINIA2]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rafforty = t.spawn(RAFFORTY, 20);
        let jinia = t.spawn(JINIA, 30);
        (t, rafforty, jinia)
    }

    /// A character that has finished Meeting Sirra (the start condition of this quest).
    fn meeting_sirra_done(t: &mut ScriptTest, id: i32, level: i32) {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(MEETING_SIRRA.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
    }

    #[test]
    fn the_start_needs_82_and_meeting_sirra() {
        let (mut t, rafforty, _) = setup();
        meeting_sirra_done(&mut t, 1, 81);
        assert!(t.quest_page(1, rafforty).contains("32020-04.htm"));
        t.enter(2, 82, vec![]);
        assert!(t.quest_page(2, rafforty).contains("32020-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn jinia_sends_the_character_to_sirra() {
        let (mut t, rafforty, jinia) = setup();
        meeting_sirra_done(&mut t, 1, 82);
        assert!(t.quest_page(1, rafforty).contains("32020-01.htm"));
        assert!(t.event(1, NAME, "32020-02.htm").contains("32020-02.htm"));
        assert!(t.event(1, NAME, "32020-03.html").contains("32020-03.html"));
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
        assert!(t.event(1, NAME, "32760-03.html").contains("32760-03.html"));
        assert!(t.event(1, NAME, "32760-04.html").contains("32760-04.html"));
        assert!(t.event(1, NAME, "32760-05.html").contains("32760-05.html"));
        // Sirra is spawned in the hideout (her place), and cond 3 comes with the check.
        assert_eq!(t.npcs_of(SIRRA).len(), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "ex", "1"),
                rows::update(NAME, "cond", "3"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483643"),
            ]
        );
        assert!(t.quest_page(1, jinia).contains("32760-05.html"));
    }

    #[test]
    fn sirra_gives_the_cores_and_jinia_takes_the_character_back() {
        let (mut t, rafforty, jinia) = setup();
        meeting_sirra_done(&mut t, 1, 82);
        t.quest_page(1, rafforty);
        t.event(1, NAME, "32020-02.htm");
        t.quest_page(1, jinia);
        t.event(1, NAME, "32760-02.html");
        t.event(1, NAME, "32760-03.html");
        t.event(1, NAME, "32760-04.html");
        t.event(1, NAME, "32760-05.html");
        t.take_writes();

        // Sirra is at her place in the hideout; she is talked to near the player here.
        let sirra = t.spawn(SIRRA, 20);
        assert!(t.quest_page(1, sirra).contains("32762-01.html"));
        assert!(t.event(1, NAME, "32762-02.html").contains("32762-02.html"));
        assert!(t.event(1, NAME, "32762-03.html").contains("32762-03.html"));
        assert!(t.event(1, NAME, "32762-04.html").contains("32762-04.html"));
        assert_eq!(t.count(1, BLACK_FROZEN_CORE), 5);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "ex", "2"),
                rows::update(NAME, "cond", "4"),
                rows::update(NAME, "__compltdStateFlags", "-2147483635"),
            ]
        );

        // Jinia: the second check is done, the character goes back out.
        assert!(t.quest_page(1, jinia).contains("32760-06.html"));
        assert!(t.event(1, NAME, "32760-07.html").contains("32760-07.html"));
        assert!(t.event(1, NAME, "32760-08.html").contains("32760-08.html"));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (113_793, -109_342));
    }

    #[test]
    fn the_last_step_pays_the_experience_and_ends_the_quest() {
        let (mut t, _, _) = setup();
        // The memo state 10 comes from the Ice Queen's castle instance script (not ported here).
        let mut data = player_data(1, 82, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![
                (MEETING_SIRRA.to_owned(), Some("Completed".to_owned())),
                (NAME.to_owned(), Some("Started".to_owned())),
            ],
            variables: vec![
                (NAME.to_owned(), "cond".to_owned(), Some("5".to_owned())),
                (NAME.to_owned(), "memoState".to_owned(), Some("10".to_owned())),
            ],
        };
        t.enter_with(data);
        let jinia2 = t.spawn(JINIA2, 20);
        assert!(t.quest_page(1, jinia2).contains("32781-01.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
