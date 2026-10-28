//! Sin Wardens: five dead minions and the master leaves.
// Java: dist/game/data/scripts/ai/others/SinWardens.java

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const SIN_WARDEN_MINIONS: [i32; 14] = [
    22424, 22425, 22426, 22427, 22428, 22429, 22430, 22432, 22433, 22434, 22435, 22436, 22437, 22438,
];

/// Java `killedMinionsCount`: master object id -> minions killed.
#[derive(Default)]
struct Killed(HashMap<ObjectId, i32>);

/// Java `ai.others.SinWardens`.
pub struct SinWardens;

impl Script for SinWardens {
    fn name(&self) -> &'static str {
        "SinWardens"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&SIN_WARDEN_MINIONS);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // `npc.isMinion()` and `getLeader()`.
        let Some(master) = ctx.world().npc_leader(npc.object_id) else {
            return;
        };
        if ctx.is_dead(master) {
            return;
        }
        let killed_count = ctx.state::<Killed>().0.get(&master).copied().unwrap_or(0) + 1;

        if killed_count == 5 {
            if let Some(master_npc) = ctx.script_npc(master) {
                ctx.broadcast_npc_say(
                    master_npc,
                    ChatType::NpcGeneral,
                    NpcStringId::WeMightNeedNewSlavesILlBeBackSoonSoWait,
                    &[],
                );
                ctx.do_die(master_npc, killer);
            }
            ctx.state::<Killed>().0.remove(&master);
        } else {
            ctx.state::<Killed>().0.insert(master, killed_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="22424" type="Monster" level="80" name="Sin Warden Minion"><stats><vitals hp="10" mp="100"/></stats></npc>
        <npc id="22420" type="Monster" level="80" name="Sin Warden"><stats><vitals hp="100000" mp="100"/></stats></npc>"#;

    #[test]
    fn the_fifth_dead_minion_kills_the_master() {
        let mut registry = ScriptRegistry::new();
        registry.add(SinWardens);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let master = t.spawn(22420, 100);
        t.enter(1, 80, vec![]);
        for i in 0..5 {
            let minion = t.world.spawn_minion_by_id(master, 22424).unwrap();
            assert!(!t.world.is_dead(master), "after {i} dead minions");
            t.kill(1, minion);
        }
        assert!(t.world.is_dead(master));
    }
}
