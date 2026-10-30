//! Quest 695: Defend the Hall of Suffering (Tepios at the Seed of Infinity entrance, and
//! the Tepios inside the Hall of Suffering who pays the party at the end of the attack).
// Java: dist/game/data/scripts/quests/Q00695_DefendTheHallOfSuffering/Q00695_DefendTheHallOfSuffering.java
//
// Pattern: the reward tag (13777..13786, by the time the run took) is stored by the instance
// (`ai/areas/gracia/instances/hall_of_suffering_defence.rs`) in the world parameter `tag`.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::instances::common::{allowed, expire_in_five_minutes, is_online};
use crate::ai::areas::gracia::seeds::soi_current_stage;

// NPCs
const TEPIOS: i32 = 32603;
const TEPIOS2: i32 = 32530;
// Items
const SOE: i32 = 736;
// Misc
const TEMPLATE_ID: i32 = 116;

/// Java `Q00695_DefendTheHallOfSuffering`.
pub struct Q00695DefendTheHallOfSuffering;

impl Script for Q00695DefendTheHallOfSuffering {
    fn name(&self) -> &'static str {
        "Q00695_DefendTheHallOfSuffering"
    }

    fn path(&self) -> &'static str {
        "quests/Q00695_DefendTheHallOfSuffering"
    }

    fn id(&self) -> i32 {
        695
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TEPIOS]);
        s.add_talk_id(&[TEPIOS]);
        s.add_talk_id(&[TEPIOS2]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // NOTE(java-quirk): every event is echoed back as a page, also for a player without a
        // quest state.
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };
        if event == "32603-02.html" {
            qs.start_quest(ctx);
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let level = ctx.player_level(player);
                if (75..=82).contains(&level) {
                    htmltext = if soi_current_stage(ctx) == 4 {
                        "32603-01.html".to_owned()
                    } else {
                        "32603-04.html".to_owned()
                    };
                } else {
                    htmltext = "32603-00.html".to_owned();
                    qs.exit_quest(ctx, true, false);
                }
            }
            STARTED => match npc.id {
                TEPIOS => htmltext = "32603-01a.html".to_owned(),
                TEPIOS2 => {
                    let world = ctx
                        .world()
                        .player_instance_world(player)
                        .map(|w| (w.template_id, w.instance_id, w.parameters.get("tag").cloned()));
                    match world {
                        Some((TEMPLATE_ID, _, tag)) => {
                            // `getInt("tag", -1)`.
                            let tag = tag.and_then(|v| v.parse::<i32>().ok()).unwrap_or(-1);
                            let party = ctx.world().party_of(player);
                            if tag == -1 {
                                htmltext = "32530-11.html".to_owned();
                            } else if let Some(party) = party
                                && ctx.world().party_leader(party) == Some(player)
                            {
                                for member in ctx.world().party_members(party) {
                                    let Some(qs1) = ctx.get_quest_state(member, false) else {
                                        continue;
                                    };
                                    if (13777..=13786).contains(&tag) {
                                        ctx.give_items(member, tag, 1);
                                        ctx.give_items(member, SOE, 1);
                                        qs1.unset(ctx, "cond");
                                        qs1.exit_quest(ctx, true, false);
                                        ctx.play_sound_named(member, "ItemSound.quest_finish");
                                        htmltext = format!("32530-{:02}.html", tag - 13777);
                                        finish_instance(ctx, player, qs1);
                                    } else {
                                        htmltext = "32530-11.html".to_owned();
                                    }
                                }
                            } else {
                                return Some("32530-10.html".to_owned());
                            }
                        }
                        _ => htmltext = "32530-11.html".to_owned(),
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `finishInstance(player)`: nobody may enter again before the next 6:30, five minutes
/// left for the instance.
fn finish_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: l2_world::script::Qs) {
    let Some(instance_id) = ctx.world().player_instance_world(player).map(|w| w.instance_id) else {
        return;
    };
    let reenter = qs.next_restart_time_ms(ctx);
    let sm = SystemMessage::new(
        SystemMessageId::InstantZoneS1SEntryHasBeenRestrictedYouCanCheckTheNextPossibleEntryTimeByUsingTheCommandInstancezone,
    )
    .with(SystemMessageParam::InstanceName(TEMPLATE_ID));
    for plr in allowed(ctx, instance_id) {
        ctx.world_mut().set_instance_time(plr, TEMPLATE_ID, reenter);
        if is_online(ctx, plr) {
            ctx.world().send(plr, &sm);
        }
    }
    expire_in_five_minutes(ctx, instance_id);
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::gracia::instances::common::{add_allowed, create_world, set_parameter};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00695_DefendTheHallOfSuffering";

    const NPCS: &str = r#"
        <npc id="32603" type="Folk" level="1" name="Tepios"/>
        <npc id="32530" type="Folk" level="1" name="Tepios"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="736" type="EtcItem" name="Scroll of Escape"><set name="is_stackable" val="true"/></item>
        <item id="13777" type="EtcItem" name="Reward 1"/>
        <item id="13778" type="EtcItem" name="Reward 2"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00695DefendTheHallOfSuffering);
        let pages = Pages::of(&Q00695DefendTheHallOfSuffering)
            .page("32603-01.html", &["32603-02.html"])
            .plain(&[
                "32603-00.html",
                "32603-04.html",
                "32603-01a.html",
                "32603-02.html",
                "32530-00.html",
                "32530-01.html",
                "32530-10.html",
                "32530-11.html",
            ])
            .npcs(&[TEPIOS, TEPIOS2]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tepios = t.spawn(TEPIOS, 20);
        (t, tepios)
    }

    fn start(t: &mut ScriptTest, tepios: Oid) {
        t.enter(1, 80, vec![]);
        t.world
            .run_as_script(NAME, |ctx| ctx.set_global_variable("SoI_stage", "4"));
        assert!(t.quest_page(1, tepios).contains("32603-01.html"));
        assert!(t.event(1, NAME, "32603-02.html").contains("32603-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    fn talk(t: &mut ScriptTest, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ScriptNpc {
                id: TEPIOS2,
                object_id: Oid(0),
            };
            out = Q00695DefendTheHallOfSuffering.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    /// An instance of the Hall of Suffering with these players allowed and the reward tag.
    fn hall(t: &mut ScriptTest, players: &[i32], tag: Option<i32>) -> i32 {
        let mut instance = 0;
        t.world.run_as_script(NAME, |ctx| {
            instance = create_world(ctx, TEMPLATE_ID);
            for p in players {
                add_allowed(ctx, instance, Oid(*p));
            }
            if let Some(tag) = tag {
                set_parameter(ctx, instance, "tag", &tag.to_string());
            }
        });
        instance
    }

    #[test]
    fn a_level_83_character_is_refused() {
        let (mut t, tepios) = setup();
        t.enter(1, 83, vec![]);
        assert!(t.quest_page(1, tepios).contains("32603-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hall_is_closed_outside_the_defence_stage() {
        let (mut t, tepios) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, tepios).contains("32603-04.html"));
    }

    #[test]
    fn a_level_80_character_starts_the_quest() {
        let (mut t, tepios) = setup();
        start(&mut t, tepios);
        assert!(t.quest_page(1, tepios).contains("32603-01a.html"));
    }

    #[test]
    fn the_inner_tepios_refuses_outside_the_hall() {
        let (mut t, tepios) = setup();
        start(&mut t, tepios);
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32530-11.html"));
        hall(&mut t, &[1], None);
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32530-11.html"));
    }

    #[test]
    fn only_the_party_leader_is_paid() {
        let (mut t, tepios) = setup();
        start(&mut t, tepios);
        hall(&mut t, &[1], Some(13777));
        // Not in a party: the leader's page.
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32530-10.html"));
    }

    #[test]
    fn the_party_is_paid_the_tag_of_the_run() {
        let (mut t, tepios) = setup();
        start(&mut t, tepios);
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        let instance = hall(&mut t, &[1, 2], Some(13778));
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32530-01.html"));
        // The leader has the quest, the member none: only the leader is paid.
        assert_eq!((t.count(1, 13778), t.count(1, SOE)), (1, 1));
        assert_eq!(t.count(2, 13778), 0);
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
        // Both may not enter before the next 6:30.
        let now = t.world.instance_time(Oid(1), TEMPLATE_ID);
        assert!(now > 0);
        assert_eq!(t.world.instance_time(Oid(2), TEMPLATE_ID), now);
        assert!(t.world.instance(instance).is_some());
    }
}
