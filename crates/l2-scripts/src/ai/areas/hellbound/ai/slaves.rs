//! The slaves of Hellbound: when their master falls they thank the players and walk away.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Slaves.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const MASTERS: [i32; 2] = [
    22320, // Junior Watchman
    22321, // Junior Summoner
];

// Locations
const MOVE_TO: (i32, i32, i32) = (-25451, 252291, -3252);

// Misc
const TRUST_REWARD: i32 = 10;

/// Java `ai.areas.Hellbound.AI.Slaves`.
pub struct Slaves;

impl Script for Slaves {
    fn name(&self) -> &'static str {
        "Slaves"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&MASTERS);
        s.add_kill_id(&MASTERS);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        // NOTE(java-quirk): `enableMinions(level < 5)` sets a field nothing reads in this
        // version, so the minions spawn at every level.
        ctx.world_mut().set_on_kill_delay(npc.object_id, 1000);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let slaves: Vec<ObjectId> = ctx.world().minions_of(npc.object_id).to_vec();
        for slave in slaves {
            if ctx.is_dead(slave) {
                continue;
            }
            let Some(slave_npc) = ctx.script_npc(slave) else {
                continue;
            };

            ctx.world_mut().clear_aggro_list(slave);
            ctx.world_mut().abort_attack(slave);
            ctx.world_mut().abort_cast(slave);
            ctx.broadcast_npc_say(
                slave_npc,
                ChatType::NpcGeneral,
                NpcStringId::ThankYouForSavingMeFromTheClutchesOfEvil,
                &[],
            );
            if (1..=2).contains(&engine::level(ctx)) {
                engine::update_trust(ctx, TRUST_REWARD, false);
            }

            ctx.npc_move_to(slave_npc, MOVE_TO.0, MOVE_TO.1, MOVE_TO.2);
            ctx.world_mut().script_add_decay_task(slave);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::set_level;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="22320" type="Monster" level="60" name="Junior Watchman"/>
        <npc id="22321" type="Monster" level="60" name="Junior Summoner"/>"#;

    #[test]
    fn the_freed_slaves_give_ten_trust_at_levels_one_and_two() {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(Slaves);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        set_level(&mut t, "Slaves", 1);
        let master = crate::ai::areas::hellbound::testing::spawn(&mut t, 22320, 20);
        t.world.run_as_script("Slaves", |ctx| {
            let m = ctx.script_npc(master).unwrap();
            assert!(ctx.spawn_minion(m, 22321).is_some());
        });
        // The master dies; `onKillDelay` 1000 instead of 2500.
        t.world.do_die(master, Some(ObjectId(1)));
        t.wait_ms(1_100);
        // The Junior Watchman's own kill is worth 1 more point at level 1.
        assert_eq!(t.world.global_variables().get_int("HBTrust", 0), 11);

        // At level 3 the slaves still walk away, but give no trust.
        set_level(&mut t, "Slaves", 3);
        let master = crate::ai::areas::hellbound::testing::spawn(&mut t, 22320, 40);
        t.world.run_as_script("Slaves", |ctx| {
            let m = ctx.script_npc(master).unwrap();
            ctx.spawn_minion(m, 22321);
        });
        t.world.do_die(master, Some(ObjectId(1)));
        t.wait_ms(1_100);
        assert_eq!(t.world.global_variables().get_int("HBTrust", 0), 11);
    }
}
