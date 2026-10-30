//! Mithril Mine instance zone, template 138: Kegor fights off five millipedes while the player
//! of `Q10284_AcquisitionOfDivineSword` watches.
// Java: dist/game/data/scripts/instances/MithrilMine/MithrilMine.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

const Q10284_ACQUISITION_OF_DIVINE_SWORD: &str = "Q10284_AcquisitionOfDivineSword";

// NPCs
const KEGOR: i32 = 18846;
const MITHRIL_MILLIPEDE: i32 = 22766;
const KRUN: i32 = 32653;
const TARUN: i32 = 32654;

// Item
const COLD_RESISTANCE_POTION: i32 = 15514;

// Skill
const BLESS_OF_SWORD: SkillKey = SkillKey { id: 6286, level: 1 };

// Location
const START_LOC: (i32, i32, i32) = (186852, -173492, -3763);
const EXIT_LOC: (i32, i32, i32) = (178823, -184303, -347);
/// `(x, y, z, heading)`.
const MOB_SPAWNS: [(i32, i32, i32, i32); 5] = [
    (185216, -184112, -3308, -15396),
    (185456, -184240, -3308, -19668),
    (185712, -184384, -3308, -26696),
    (185920, -184544, -3308, -32544),
    (185664, -184720, -3308, 27892),
];

// Misc
const TEMPLATE_ID: i32 = 138;

/// Java `instances.MithrilMine.MithrilMine`.
pub struct MithrilMine;

impl Script for MithrilMine {
    fn name(&self) -> &'static str {
        "MithrilMine"
    }

    fn path(&self) -> &'static str {
        "instances/MithrilMine"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[KEGOR]);
        s.add_kill_id(&[KEGOR, MITHRIL_MILLIPEDE]);
        s.add_start_npc(&[TARUN, KRUN]);
        s.add_talk_id(&[TARUN, KRUN, KEGOR]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): every event needs the NPC (`InstanceManager.getWorld(npc)` throws
        // without one).
        let npc = npc?;
        let world = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id);

        match event {
            "BUFF" => {
                if let Some(player) = player
                    && let Some((x, y, z)) = ctx.world().position_of(player)
                    && ctx.is_inside_radius_3d(npc, x, y, z, 1000)
                    && ctx.is_script_value(npc, 1)
                    && !ctx.is_dead(player)
                {
                    ctx.cast_skill_at(npc, Some(player), BLESS_OF_SWORD);
                }

                // NOTE(java-quirk): the timer starts again whatever the condition said; only the
                // last millipede's kill cancels it.
                ctx.start_quest_timer("BUFF", 30000, Some(npc), player, false);
            }
            "TIMER" => {
                if let Some(instance_id) = world {
                    for (x, y, z, heading) in MOB_SPAWNS {
                        let Some(spawned_mob) =
                            ctx.add_spawn(MITHRIL_MILLIPEDE, x, y, z, heading, false, 0, instance_id)
                        else {
                            continue;
                        };
                        ctx.set_script_value(spawned_mob, 1);
                        ctx.world_mut().set_npc_running(spawned_mob.object_id);
                        ctx.world_mut().set_intention_attack(spawned_mob.object_id, npc.object_id);
                        ctx.world_mut()
                            .add_damage_hate(spawned_mob.object_id, npc.object_id, 0, 999_999);
                    }
                }
            }
            "FINISH" => {
                // NOTE(java-quirk): `world` is used below without a null check.
                let instance_id = world?;
                let known = ctx.world().visible_objects(npc.object_id);
                for known_char in known {
                    if ctx.world().npc(known_char).is_none_or(|n| n.npc_id != KEGOR) {
                        continue;
                    }
                    let kegor = ScriptNpc {
                        object_id: known_char,
                        id: KEGOR,
                    };
                    ctx.set_script_value(kegor, 2);
                    ctx.world_mut().set_running(known_char, false); // setWalking
                    if let Some(player) = player {
                        ctx.world_mut().set_npc_target(known_char, Some(player));
                        ctx.world_mut().script_start_follow(known_char, player);
                    }
                    ctx.broadcast_npc_say(
                        kegor,
                        ChatType::NpcGeneral,
                        NpcStringId::ICanFinallyTakeABreatherByTheWayWhoAreYouHmmIThinkIKnowWhoSentYou,
                        &[],
                    );
                }
                ctx.world_mut().set_instance_duration(instance_id, 3000);
            }
            _ => {}
        }

        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.quest_state_of(Q10284_ACQUISITION_OF_DIVINE_SWORD, player, false);
        if let Some(qs) = qs {
            if qs.is_memo_state(ctx, 2) {
                return Some(
                    if ctx.is_script_value(npc, 0) {
                        "18846.html"
                    } else {
                        "18846-01.html"
                    }
                    .to_owned(),
                );
            } else if qs.is_memo_state(ctx, 3) {
                // NOTE(java-quirk): `getPlayerWorld(player)` is dereferenced unchecked; a player
                // no world allows ends the talk here, without the reward.
                let instance_id = ctx.world().player_instance_world(player).map(|w| w.instance_id)?;
                if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                    world.remove_allowed(player);
                }
                ctx.world_mut().set_instance_id(player, 0);
                // `player.teleToLocation(EXIT_LOC, 0)`: instance 0, no random offset.
                ctx.teleport_player(player, EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2, 0, false);
                ctx.give_adena(player, 296425, true);
                ctx.add_exp_and_sp(player, 921805, 82230);
                qs.exit_quest(ctx, false, true);
                return Some("18846-03.html".to_owned());
            }
        }

        // `super.onFirstTalk(npc, player)`: `Quest.onFirstTalk` returns null.
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // NOTE(java-quirk): both branches use `world` without a null check.
        let Some(instance_id) = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id)
        else {
            return;
        };
        if npc.id == KEGOR {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::HowCouldIFallInAPlaceLikeThis, &[]);
            ctx.world_mut().set_instance_duration(instance_id, 1000);
        } else {
            if ctx.is_script_value(npc, 1) {
                let count = count_of(ctx, instance_id);
                if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                    world.parameters.insert("count".to_owned(), (count + 1).to_string());
                }
            }

            if count_of(ctx, instance_id) >= 5 {
                let qs = ctx.quest_state_of(Q10284_ACQUISITION_OF_DIVINE_SWORD, killer, false);
                if let Some(qs) = qs
                    && qs.is_memo_state(ctx, 2)
                {
                    // NOTE(java-quirk): `npc` is the millipede, but the BUFF timer belongs to
                    // Kegor, so this cancels nothing and Kegor keeps buffing every 30 seconds.
                    ctx.cancel_quest_timer("BUFF", Some(npc), Some(killer));
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 6, true);
                    ctx.start_quest_timer("FINISH", 3000, Some(npc), Some(killer), false);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        match npc.id {
            TARUN | KRUN => {
                let qs = ctx.quest_state_of(Q10284_ACQUISITION_OF_DIVINE_SWORD, talker, false);
                if let Some(qs) = qs
                    && qs.is_memo_state(ctx, 2)
                {
                    if !ctx.has_quest_items(talker, COLD_RESISTANCE_POTION) {
                        ctx.give_items(talker, COLD_RESISTANCE_POTION, 1);
                    }

                    qs.set_cond(ctx, 4, true);
                    enter_instance(ctx, talker);
                }
            }
            KEGOR => {
                let qs = ctx.quest_state_of(Q10284_ACQUISITION_OF_DIVINE_SWORD, talker, false);
                if let Some(qs) = qs
                    && qs.is_memo_state(ctx, 2)
                    && ctx.has_quest_items(talker, COLD_RESISTANCE_POTION)
                    && ctx.is_script_value(npc, 0)
                {
                    ctx.take_items(talker, COLD_RESISTANCE_POTION, -1);
                    qs.set_cond(ctx, 5, true);
                    ctx.set_script_value(npc, 1);
                    ctx.start_quest_timer("TIMER", 3000, Some(npc), Some(talker), false);
                    ctx.start_quest_timer("BUFF", 3500, Some(npc), Some(talker), false);
                    return Some("18846-02.html".to_owned());
                }
            }
            _ => {}
        }

        // `super.onTalk(npc, talker)`.
        None
    }
}

/// `world.getParameters().getInt("count", 0)`.
fn count_of(ctx: &ScriptCtx<'_>, instance_id: i32) -> i32 {
    ctx.world()
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get("count"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Java `enterInstance(talker, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                if let Some(instance_world) = world.instance_world_mut(instance_id) {
                    instance_world.add_allowed(player);
                }
            }
            world.tele_to_location(
                player,
                TeleportTarget::at(START_LOC.0, START_LOC.1, START_LOC.2).with_instance(instance_id),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest, rows};

    const NPCS: &str = r#"
        <npc id="18846" type="Folk" level="70" name="Kegor"/>
        <npc id="22766" type="Monster" level="70" name="Mithril Millipede"/>
        <npc id="32653" type="Folk" level="70" name="Krun"/>
        <npc id="32654" type="Folk" level="70" name="Tarun"/>"#;
    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15514" type="EtcItem" name="Cold Resistance Potion"><set name="is_questitem" val="true"/></item>"#;

    /// The quest of the chain: a script of that name.
    struct StandIn;
    impl Script for StandIn {
        fn name(&self) -> &'static str {
            Q10284_ACQUISITION_OF_DIVINE_SWORD
        }
        fn path(&self) -> &'static str {
            "quests/Q10284_AcquisitionOfDivineSword"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn);
        registry.add(MithrilMine);
        let pages = Pages::of(&MithrilMine)
            .file(
                "data/html/default/32654.htm",
                r#"<html><body><a action="bypass -h Script MithrilMine">go</a></body></html>"#,
            )
            .file(
                "data/scripts/instances/MithrilMine/18846.html",
                r#"<html><body>18846.html<a action="bypass -h Script MithrilMine">Feed him the antidote.</a></body></html>"#,
            )
            .plain(&["18846-01.html", "18846-02.html", "18846-03.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        t.world.set_instance_data(instance_data(
            &[(138, "Mithril Mine")],
            &[(138, r#"<exitPoint x="178823" y="-184303" z="-347" />"#)],
        ));
        let tarun = t.spawn(TARUN, 20);
        let kegor = t.spawn(KEGOR, 40);
        (t, tarun, kegor)
    }

    fn memo_state(t: &mut ScriptTest, state: i32) {
        t.world.run_as_script(Q10284_ACQUISITION_OF_DIVINE_SWORD, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_memo_state(ctx, state);
        });
        t.take_writes();
    }

    fn cond(t: &mut ScriptTest) -> i32 {
        let mut cond = 0;
        t.world.run_as_script(Q10284_ACQUISITION_OF_DIVINE_SWORD, |ctx| {
            cond = ctx.get_quest_state(Oid(1), false).unwrap().get_cond(ctx);
        });
        cond
    }

    fn page(t: &mut ScriptTest) -> String {
        crate::testing::pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    #[test]
    fn tarun_gives_the_potion_and_sends_the_player_in() {
        let (mut t, tarun, _) = setup();
        t.enter(1, 82, vec![]);
        // Without the quest nothing happens.
        t.talk(1, tarun);
        t.bypass(1, "Script MithrilMine");
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);

        memo_state(&mut t, 2);
        t.talk(1, tarun);
        t.bypass(1, "Script MithrilMine");
        t.ticks(60);
        assert_eq!(t.count(1, COLD_RESISTANCE_POTION), 1);
        assert_eq!(cond(&mut t), 4);
        let instance_id = t.world.instance_id_of(Oid(1));
        assert!(instance_id > 0);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (138, vec![Oid(1)]));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (START_LOC.0, START_LOC.1));
    }

    #[test]
    fn kegor_wakes_up_the_millipedes_come_and_the_reward_follows() {
        let (mut t, tarun, kegor) = setup();
        t.enter(1, 82, vec![item(57)]);
        memo_state(&mut t, 2);
        t.talk(1, tarun);
        t.bypass(1, "Script MithrilMine");
        t.ticks(60);
        let instance_id = t.world.instance_id_of(Oid(1));
        put_kegor_beside_the_player(&mut t, kegor, instance_id);

        // First talk: the unwoken Kegor, page 18846.html.
        t.talk(1, kegor);
        t.ticks(5);
        let shown = page(&mut t);
        assert!(shown.contains("18846.html"), "{shown}");
        // Talk: takes the potion, cond 5, the timers.
        t.bypass(1, "Script MithrilMine");
        assert!(page(&mut t).contains("18846-02.html"));
        assert_eq!(t.count(1, COLD_RESISTANCE_POTION), 0);
        assert_eq!(cond(&mut t), 5);
        assert_eq!(t.world.quest_timer_count(), 2);

        // After 3 seconds: five millipedes with script value 1.
        t.wait_ms(3_100);
        let mobs = t.npcs_of(MITHRIL_MILLIPEDE);
        assert_eq!(mobs.len(), 5);
        for mob in &mobs {
            assert_eq!(t.world.npc_script_value(*mob), 1);
            assert_eq!(t.world.instance_id_of(*mob), instance_id);
        }

        // The fight happens where the millipedes are: Kegor is next to them.
        t.world.tele_to_location(
            kegor,
            TeleportTarget::at(185500, -184300, -3308).with_instance(instance_id),
        );
        t.ticks(5);
        // The fifth death: memo 3, cond 6, FINISH in 3 seconds (the BUFF timer is not cancelled,
        // see the quirk in `on_kill`).
        for mob in &mobs {
            t.kill(1, *mob);
        }
        assert_eq!(cond(&mut t), 6);
        assert_eq!(count_of_world(&t, instance_id), 5);
        assert_eq!(t.world.quest_timer_count(), 2);
        let end_before = t.world.instance(instance_id).unwrap().end_time_ms;
        t.wait_ms(3_100);
        assert_eq!(t.world.npc_script_value(kegor), 2);
        // `setDuration(3000)`: the instance ends within seconds of FINISH.
        let end = t.world.instance(instance_id).map(|i| i.end_time_ms);
        assert_ne!(end, Some(end_before));
    }

    /// Kegor stands in the instance, next to the player who just arrived.
    fn put_kegor_beside_the_player(t: &mut ScriptTest, kegor: Oid, instance_id: i32) {
        let (x, y, z) = t.world.position_of(Oid(1)).unwrap();
        t.world.tele_to_location(
            kegor,
            TeleportTarget::at(x + 20, y, z).with_instance(instance_id),
        );
        t.ticks(5);
    }

    fn count_of_world(t: &ScriptTest, instance_id: i32) -> i32 {
        t.world
            .instance_world(instance_id)
            .and_then(|w| w.parameters.get("count"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    fn item(id: i32) -> l2_world::ItemData {
        crate::testing::item(900_000 + id, id, 0)
    }

    #[test]
    fn the_second_talk_with_kegor_pays_and_ends_the_quest() {
        let (mut t, tarun, kegor) = setup();
        t.enter(1, 82, vec![]);
        memo_state(&mut t, 2);
        t.talk(1, tarun);
        t.bypass(1, "Script MithrilMine");
        t.ticks(60);
        let instance_id = t.world.instance_id_of(Oid(1));
        put_kegor_beside_the_player(&mut t, kegor, instance_id);
        t.world.run_as_script(Q10284_ACQUISITION_OF_DIVINE_SWORD, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, 3);
        });
        t.take_writes();
        t.take();

        t.talk(1, kegor);
        t.ticks(5);
        let shown = page(&mut t);
        assert!(shown.contains("18846-03.html"), "{shown}");
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert!(t.world.instance_world(instance_id).unwrap().allowed.is_empty());
        let writes = t.take_writes();
        assert!(writes.contains(&rows::of(1, rows::delete(Q10284_ACQUISITION_OF_DIVINE_SWORD, false))));
    }
}
