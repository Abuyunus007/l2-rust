//! The Quarry slaves: a player leads one to the dolmen zone for a small gift and Hellbound's trust.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Quarry/Quarry.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const SLAVE: i32 = 32299;

// Items: `(item id, chance out of 10000)`, each one item.
const DROP_LIST: [(i32, i32); 6] = [
    (9628, 261),   // Leonard
    (9630, 175),   // Orichalcum
    (9629, 145),   // Adamantine
    (1876, 6667),  // Mithril ore
    (1877, 1333),  // Adamantine nugget
    (1874, 2222),  // Oriharukon ore
];

// Zone
const ZONE: i32 = 40107;
/// The quarry zone where a slave that ran out of time is sent home.
const QUARRY_ZONE: i32 = 40108;

// Misc
const TRUST: i32 = 50;

/// Java `ai.areas.Hellbound.AI.NPC.Quarry.Quarry`.
pub struct Quarry;

impl Script for Quarry {
    fn name(&self) -> &'static str {
        "Quarry"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Quarry"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[SLAVE]);
        s.add_first_talk_id(&[SLAVE]);
        s.add_start_npc(&[SLAVE]);
        s.add_talk_id(&[SLAVE]);
        s.add_kill_id(&[SLAVE]);
        s.add_enter_zone_id(&[ZONE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "FollowMe" => {
                let (npc, player) = (npc?, player?);
                ctx.world_mut().script_start_follow(npc.object_id, player);
                ctx.world_mut().set_npc_target(npc.object_id, Some(player));
                ctx.world_mut().set_quest_guard_auto_attackable(npc.object_id, true);
                // `npc.setRHandId(9136)`: the weapon it holds is not shown (cosmetic, not modelled).
                // `npc.setWalking()`.
                ctx.world_mut().set_running(npc.object_id, false);

                if !ctx.has_quest_timer("TIME_LIMIT", Some(npc), None) {
                    ctx.start_quest_timer("TIME_LIMIT", 900_000, Some(npc), None, false); // 15 min limit for save
                }

                htmltext = Some("32299-02.htm".to_owned());
            }
            "TIME_LIMIT" => {
                let npc = npc?;
                if ctx.world().is_inside_zone_id(npc.object_id, QUARRY_ZONE) {
                    ctx.world_mut().set_npc_target(npc.object_id, None);
                    ctx.world_mut().set_intention_active(npc.object_id);
                    ctx.world_mut().set_quest_guard_auto_attackable(npc.object_id, false);
                    // `npc.setRHandId(0)`.
                    if let Some((x, y, z)) = ctx.npc_spawn_point(npc).filter(|_| {
                        ctx.world().npc(npc.object_id).is_some_and(|n| n.spawn.is_some())
                    }) {
                        ctx.tele_npc_to(npc, x, y, z);
                    }
                    return None;
                }

                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::HunHungry, &[]);
                ctx.world_mut().do_die(npc.object_id, Some(npc.object_id));
            }
            "DECAY" => {
                if let Some(npc) = npc
                    && !ctx.is_dead(npc.object_id)
                {
                    let target = ctx.target_of(npc.object_id);
                    if let Some(target) = target
                        && ctx.world().player(target).is_some()
                    {
                        for (item_id, chance) in DROP_LIST {
                            if ctx.get_random(10000) < chance {
                                ctx.drop_item(npc, Some(target), item_id, 1);
                                break;
                            }
                        }
                    }

                    ctx.world_mut().set_quest_guard_auto_attackable(npc.object_id, false);
                    engine::update_trust(ctx, TRUST, true);
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().set_quest_guard_auto_attackable(npc.object_id, false);
        // `npc instanceof QuestGuard`
        if ctx
            .world()
            .npc(npc.object_id)
            .is_some_and(|n| n.class == l2_world::NpcClass::QuestGuard)
        {
            ctx.world_mut().set_quest_guard_passive(npc.object_id, true);
        }
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        if engine::level(ctx) != 5 {
            return Some("32299.htm".to_owned());
        }

        Some("32299-01.htm".to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.world_mut().set_quest_guard_auto_attackable(npc.object_id, false);
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        let Some(npc) = ctx.script_npc(creature) else {
            return;
        };
        if ctx.world().is_attackable_npc(creature)
            && npc.id == SLAVE
            && !ctx.is_dead(creature)
            && ctx.world().npc(creature).is_some_and(|n| !n.decayed)
            && ctx.world().is_intention_follow(creature)
            && engine::level(ctx) == 5
        {
            ctx.start_quest_timer("DECAY", 1000, Some(npc), None, false);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::ThankYouForTheRescueItSASmallGift,
                &[],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::{last_page, set_level, spawn};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="32299" type="QuestGuard" level="70" name="Quarry Slave"/>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(Quarry);
        let pages = Pages::of(&Quarry)
            .page("32299-01.htm", &["FollowMe"])
            .plain(&["32299.htm", "32299-02.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.enter(1, 80, vec![]);
        let slave = spawn(&mut t, SLAVE, 20);
        (t, slave)
    }

    #[test]
    fn the_slave_is_passive_and_not_attackable_until_it_follows() {
        let (mut t, slave) = setup();
        assert!(t.world.is_quest_guard_passive(slave));
        assert!(!t.world.is_quest_guard_auto_attackable(slave));
        set_level(&mut t, "Quarry", 5);
        t.talk(1, slave);
        assert!(last_page(&t.take(), 1).contains("32299-01.htm"));
        assert!(t.event(1, "Quarry", "FollowMe").contains("32299-02.htm"));
        assert!(t.world.is_quest_guard_auto_attackable(slave));
        assert!(t.world.is_intention_follow(slave));
        t.world.run_as_script("Quarry", |ctx| {
            assert!(ctx.has_quest_timer("TIME_LIMIT", ctx.script_npc(slave), None));
        });
    }

    #[test]
    fn the_slave_starves_after_fifteen_minutes() {
        let (mut t, slave) = setup();
        set_level(&mut t, "Quarry", 5);
        t.talk(1, slave);
        t.take();
        t.event(1, "Quarry", "FollowMe");
        t.wait_ms(900_100);
        assert!(t.world.is_dead(slave) || !t.in_world(slave));
    }

    #[test]
    fn a_slave_that_reaches_the_zone_decays_for_trust() {
        let (mut t, slave) = setup();
        set_level(&mut t, "Quarry", 5);
        t.talk(1, slave);
        t.take();
        t.event(1, "Quarry", "FollowMe");
        t.world.run_as_script("Quarry", |ctx| Quarry.on_enter_zone(ctx, slave, ZONE));
        t.wait_ms(1_100);
        assert!(!t.in_world(slave));
        assert_eq!(t.world.global_variables().get_int("HBTrust", 0), TRUST);
    }
}
