//! The Timak Orc Troop Leader calls its private minions when hit.
// Java: dist/game/data/scripts/ai/others/TimakOrcTroopLeader.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const TIMAK_ORC_TROOP_LEADER: i32 = 20767;
const ON_ATTACK_MSG: [NpcStringId; 4] = [
    NpcStringId::ComeOutYouChildrenOfDarkness,
    NpcStringId::ShowYourselves,
    NpcStringId::DestroyTheEnemyMyBrothers,
    NpcStringId::ForcesOfDarknessFollowMe,
];

/// Java `ai.others.TimakOrcTroopLeader`.
pub struct TimakOrcTroopLeader;

impl Script for TimakOrcTroopLeader {
    fn name(&self) -> &'static str {
        "TimakOrcTroopLeader"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[TIMAK_ORC_TROOP_LEADER]);
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.world().is_monster_npc(npc.object_id) {
            let rate = ctx.world().npc_template_param_int(npc.object_id, "SummonPrivateRate");
            if !ctx.world().is_teleporting(npc.object_id)
                && ctx.get_random_range(1, 100) <= rate.unwrap_or(0)
                && ctx.spawned_minion_count(npc) < 3
            {
                for minion_id in ctx.world().npc_template_minion_ids(npc.object_id, "Privates") {
                    let found = ctx
                        .world()
                        .minions_of(npc.object_id)
                        .iter()
                        .any(|&m| ctx.script_npc(m).is_some_and(|n| n.id == minion_id));

                    if !found {
                        if let Some(msg) = ctx.get_random_entry(&ON_ATTACK_MSG) {
                            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, msg, &[]);
                        }
                        ctx.spawn_minion(npc, minion_id);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="20767" type="Monster" level="30" name="Timak Orc Troop Leader">
          <parameters><param name="SummonPrivateRate" value="100"/><minions name="Privates"><npc id="20768" count="1" respawnTime="60"/></minions></parameters>
          <stats><vitals hp="100000" mp="100"/></stats></npc>
        <npc id="20768" type="Monster" level="30" name="Timak Orc"><stats><vitals hp="100" mp="100"/></stats></npc>"#;

    #[test]
    fn the_leader_calls_each_private_once() {
        let mut registry = ScriptRegistry::new();
        registry.add(TimakOrcTroopLeader);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let leader = t.spawn(TIMAK_ORC_TROOP_LEADER, 100);
        t.enter(1, 30, vec![]);
        let before = t.world.spawned_minion_count(leader);
        t.world.script_on_attackable_attack(leader, ObjectId(1), 10, None);
        t.world.script_drain();
        let after = t.world.spawned_minion_count(leader);
        assert!(after >= before);
        // A second hit: the private is there already.
        t.world.script_on_attackable_attack(leader, ObjectId(1), 10, None);
        t.world.script_drain();
        assert_eq!(t.world.spawned_minion_count(leader), after);
    }
}
