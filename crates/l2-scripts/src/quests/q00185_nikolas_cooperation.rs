//! Quest 185: Nikolas' Cooperation (Maestro Nikola, Researcher Lorain, the Destroyed Device and the Alarm of Giant).
// Java: dist/game/data/scripts/quests/Q00185_NikolasCooperation/Q00185_NikolasCooperation.java
//
// The same steps as Q00184 (Art of Persuasion) with its own items; the two quests are the two
// choices Nikola offers (`docs/design/scripts.md`: Q00183 sends the event to one of them).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MAESTRO_NIKOLA: i32 = 30621;
const RESEARCHER_LORAIN: i32 = 30673;
const DESTROYED_DEVICE: i32 = 32366;
const ALARM_OF_GIANT: i32 = 32367;

// Items
const METALLOGRAPH: i32 = 10363;
const BROKEN_METAL_PIECES: i32 = 10364;
const NIKOLAS_MAP: i32 = 10365;

// Reward
const LORAINES_CERTIFICATE: i32 = 10362;

// Misc
const MIN_LEVEL: i32 = 40;
const MAX_LEVEL_FOR_EXP_SP: i32 = 46;

/// Java `Q00183_RelicExploration.class.getSimpleName()`: the quest this one follows.
const RELIC_EXPLORATION: &str = "Q00183_RelicExploration";
/// Java `Q00184_ArtOfPersuasion.class.getSimpleName()`: the other choice.
const ART_OF_PERSUASION: &str = "Q00184_ArtOfPersuasion";

/// Java `Q00185_NikolasCooperation`.
pub struct Q00185NikolasCooperation;

impl Script for Q00185NikolasCooperation {
    fn name(&self) -> &'static str {
        "Q00185_NikolasCooperation"
    }

    fn path(&self) -> &'static str {
        "quests/Q00185_NikolasCooperation"
    }

    fn id(&self) -> i32 {
        185
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MAESTRO_NIKOLA]);
        s.add_talk_id(&[
            MAESTRO_NIKOLA,
            RESEARCHER_LORAIN,
            DESTROYED_DEVICE,
            ALARM_OF_GIANT,
        ]);
        s.register_quest_items(&[METALLOGRAPH, BROKEN_METAL_PIECES, NIKOLAS_MAP]);
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
            "30621-06.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.give_items(player, NIKOLAS_MAP, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30621-03.htm" => {
                htmltext = Some(
                    if ctx.player_level(player) >= MIN_LEVEL {
                        event
                    } else {
                        "30621-03a.htm"
                    }
                    .to_owned(),
                );
            }
            "30621-04.htm" | "30621-05.htm" => htmltext = Some(event.to_owned()),
            "30673-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30673-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    ctx.take_items(player, NIKOLAS_MAP, -1);
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30673-05.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30673-08.html" => {
                if qs.is_memo_state(ctx, 6) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30673-09.html" => {
                if qs.is_memo_state(ctx, 6) {
                    if ctx.has_quest_items(player, METALLOGRAPH) {
                        ctx.give_items(player, LORAINES_CERTIFICATE, 1);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30673-10.html".to_owned());
                        qs.exit_quest(ctx, false, true);
                    }
                    ctx.give_adena(player, 72_527, true);
                    if ctx.player_level(player) < MAX_LEVEL_FOR_EXP_SP {
                        ctx.add_exp_and_sp(player, 203_717, 14032);
                    }
                }
            }
            "32366-03.html" => {
                // Java `npc.getVariables()` is the NPC's own variables (shared with other scripts).
                if let Some(npc) = npc
                    && qs.is_memo_state(ctx, 3)
                    && !ctx
                        .world_mut()
                        .npc_variables_mut(npc.object_id)
                        .get_bool("SPAWNED", false)
                {
                    let variables = ctx.world_mut().npc_variables_mut(npc.object_id);
                    variables.set_bool("SPAWNED", true);
                    variables.set_int("PLAYER_ID", player.0);
                    // `addSpawn(ALARM, x + 80, y + 60, z, 16384, false, 0)` at the player's place.
                    if let Some((x, y, z)) = ctx.world().position_of(player)
                        && let Some(alarm) =
                            ctx.add_spawn(ALARM_OF_GIANT, x + 80, y + 60, z, 16384, false, 0, 0)
                    {
                        // The alarm's AI reads the player and the device (`player0`, `npc0`).
                        let alarm_vars = ctx.world_mut().npc_variables_mut(alarm.object_id);
                        alarm_vars.set_long("player0", i64::from(player.0));
                        alarm_vars.set_long("npc0", i64::from(npc.object_id.0));
                    }
                }
            }
            "32366-06.html" => {
                if qs.is_memo_state(ctx, 4) {
                    ctx.give_items(player, METALLOGRAPH, 1);
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32366-08.html" => {
                if qs.is_memo_state(ctx, 5) {
                    ctx.give_items(player, BROKEN_METAL_PIECES, 1);
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == MAESTRO_NIKOLA {
                    // Java reads the three states; this quest's own state exists from the line above.
                    let relic_done = ctx
                        .player_quest_state(player, RELIC_EXPLORATION)
                        .is_some_and(|q183| q183.is_completed());
                    let has_184 = ctx.player_quest_state(player, ART_OF_PERSUASION).is_some();
                    let has_185 = ctx
                        .player_quest_state(player, "Q00185_NikolasCooperation")
                        .is_some();
                    if relic_done && has_184 && has_185 {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "30621-01.htm"
                        } else {
                            "30621-02.html"
                        }
                        .to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                MAESTRO_NIKOLA => {
                    if memo_state == 1 {
                        htmltext = "30621-07.html".to_owned();
                    }
                }
                RESEARCHER_LORAIN => {
                    if memo_state == 1 {
                        htmltext = "30673-01.html".to_owned();
                    } else if memo_state == 2 {
                        htmltext = "30673-04.html".to_owned();
                    } else if (3..=5).contains(&memo_state) {
                        htmltext = "30673-06.html".to_owned();
                    } else if memo_state == 6 {
                        htmltext = "30673-07.html".to_owned();
                    }
                }
                DESTROYED_DEVICE => {
                    if memo_state == 3 {
                        let vars = ctx.world_mut().npc_variables_mut(npc.object_id);
                        if !vars.get_bool("SPAWNED", false) {
                            htmltext = "32366-01.html".to_owned();
                        } else if vars.get_int("PLAYER_ID", 0) == player.0 {
                            htmltext = "32366-03.html".to_owned();
                        } else {
                            htmltext = "32366-04.html".to_owned();
                        }
                    } else if memo_state == 4 {
                        htmltext = "32366-05.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "32366-07.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == MAESTRO_NIKOLA {
                    htmltext = ctx.get_already_completed_msg(player);
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
    use l2_world::script::{COMPLETED, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, pages_to, rows};

    const NAME: &str = "Q00185_NikolasCooperation";

    const NPCS: &str = r#"
        <npc id="30621" type="Folk" level="70" name="Maestro Nikola"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>
        <npc id="32366" type="Folk" level="70" name="Destroyed Device"/>
        <npc id="32367" type="Monster" level="40" name="Alarm of Giant"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10362" type="EtcItem" name="Loraine's Certificate"><set name="is_questitem" val="true"/></item>
        <item id="10363" type="EtcItem" name="Metallograph"><set name="is_questitem" val="true"/></item>
        <item id="10364" type="EtcItem" name="Broken Metal Pieces"><set name="is_questitem" val="true"/></item>
        <item id="10365" type="EtcItem" name="Nikola's Map"><set name="is_questitem" val="true"/></item>"#;

    /// A quest of the chain this one reads by name: a script of that name.
    struct StandIn(&'static str, &'static str);

    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            self.1
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        nikola: Oid,
        lorain: Oid,
        device: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn(RELIC_EXPLORATION, "quests/Q00183_RelicExploration"));
        registry.add(StandIn(ART_OF_PERSUASION, "quests/Q00184_ArtOfPersuasion"));
        registry.add(Q00185NikolasCooperation);
        let pages = Pages::of(&Q00185NikolasCooperation)
            .page("30621-01.htm", &["30621-03.htm"])
            .page("30621-03.htm", &["30621-04.htm"])
            .page("30621-04.htm", &["30621-05.htm"])
            .page("30621-05.htm", &["30621-06.htm"])
            .plain(&["30621-02.html", "30621-03a.htm", "30621-06.htm", "30621-07.html"])
            .page("30673-01.html", &["30673-02.html"])
            .page("30673-02.html", &["30673-03.html"])
            .plain(&["30673-03.html"])
            .page("30673-04.html", &["30673-05.html"])
            .plain(&["30673-05.html", "30673-06.html", "30673-07.html"])
            .page("30673-07.html", &["30673-08.html"])
            .page("30673-08.html", &["30673-09.html"])
            .plain(&["30673-09.html", "30673-10.html"])
            .page("32366-01.html", &["32366-03.html"])
            .plain(&["32366-03.html", "32366-04.html"])
            .page("32366-05.html", &["32366-06.html"])
            .plain(&["32366-06.html"])
            .page("32366-07.html", &["32366-08.html"])
            .plain(&["32366-08.html"])
            .npcs(&[MAESTRO_NIKOLA, RESEARCHER_LORAIN, DESTROYED_DEVICE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            nikola: t.spawn(MAESTRO_NIKOLA, 20),
            lorain: t.spawn(RESEARCHER_LORAIN, 40),
            device: t.spawn(DESTROYED_DEVICE, 60),
        };
        (t, npcs)
    }

    fn complete(t: &mut ScriptTest, name: &'static str, player: i32) {
        t.world.run_as_script(name, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_offer_needs_relic_exploration_and_art_of_persuasion_too() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, 0, vec![]);
        complete(&mut t, RELIC_EXPLORATION, 1);
        // No state of Art of Persuasion yet: no offer.
        assert!(!t.quest_page(1, npcs.nikola).contains("30621-"));
        t.world.run_as_script(ART_OF_PERSUASION, |ctx| {
            ctx.get_quest_state(Oid(1), true);
        });
        assert!(t.quest_page(1, npcs.nikola).contains("30621-01.htm"));

        t.enter_as(2, 39, race::HUMAN, 0, vec![]);
        complete(&mut t, RELIC_EXPLORATION, 2);
        t.world.run_as_script(ART_OF_PERSUASION, |ctx| {
            ctx.get_quest_state(Oid(2), true);
        });
        assert!(t.quest_page(2, npcs.nikola).contains("30621-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_map_then_the_device_alarm_then_the_reward_without_the_metallograph() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, 0, vec![]);
        complete(&mut t, RELIC_EXPLORATION, 1);
        t.world.run_as_script(ART_OF_PERSUASION, |ctx| {
            ctx.get_quest_state(Oid(1), true);
        });
        t.take_writes();

        assert!(t.quest_page(1, npcs.nikola).contains("30621-01.htm"));
        assert!(t.event(1, NAME, "30621-03.htm").contains("30621-03.htm"));
        assert!(t.event(1, NAME, "30621-04.htm").contains("30621-04.htm"));
        assert!(t.event(1, NAME, "30621-05.htm").contains("30621-05.htm"));
        assert!(t.event(1, NAME, "30621-06.htm").contains("30621-06.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert_eq!(t.count(1, NIKOLAS_MAP), 1);

        assert!(t.quest_page(1, npcs.lorain).contains("30673-01.html"));
        assert!(t.event(1, NAME, "30673-02.html").contains("30673-02.html"));
        assert!(t.event(1, NAME, "30673-03.html").contains("30673-03.html"));
        assert_eq!(t.count(1, NIKOLAS_MAP), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, npcs.lorain).contains("30673-04.html"));
        assert!(t.event(1, NAME, "30673-05.html").contains("30673-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3")
            ]
        );

        // The device: the alarm at the player's side, once.
        assert!(t.quest_page(1, npcs.device).contains("32366-01.html"));
        t.bypass(1, &format!("Script {NAME} 32366-03.html"));
        t.take();
        assert_eq!(t.npcs_of(ALARM_OF_GIANT).len(), 1);
        assert!(t.quest_page(1, npcs.device).contains("32366-03.html"));

        // Memo state 4 (set by the test): the device gives the metallograph.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 4);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.device).contains("32366-05.html"));
        assert!(t.event(1, NAME, "32366-06.html").contains("32366-06.html"));
        assert_eq!(t.count(1, METALLOGRAPH), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "4")
            ]
        );

        // Lorain: the metallograph is in the inventory, so the certificate is the reward.
        assert!(t.quest_page(1, npcs.lorain).contains("30673-07.html"));
        assert!(t.event(1, NAME, "30673-08.html").contains("30673-08.html"));
        t.bypass(1, &format!("Script {NAME} 30673-09.html"));
        assert!(pages_to(&t.take(), 1).last().unwrap().contains("30673-09.html"));
        assert_eq!(t.count(1, LORAINES_CERTIFICATE), 1);
        assert_eq!(t.count(1, 57), 72_527);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
