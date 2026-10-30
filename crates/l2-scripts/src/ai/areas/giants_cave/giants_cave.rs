//! The scouts of the Giants' Cave shout and call their neighbours on an intruder.
// Java: dist/game/data/scripts/ai/areas/GiantsCave/GiantsCave.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const SCOUTS: [i32; 2] = [
    22668, // Gamlin (Scout)
    22669, // Leogul (Scout)
];

/// Java `ai.areas.GiantsCave.GiantsCave`.
pub struct GiantsCave;

impl Script for GiantsCave {
    fn name(&self) -> &'static str {
        "GiantsCave"
    }

    fn path(&self) -> &'static str {
        "ai/areas/GiantsCave"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&SCOUTS);
        s.add_aggro_range_enter_id(&SCOUTS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "ATTACK"
            && let (Some(player), Some(npc)) = (player, npc)
            && !ctx.is_dead(npc.object_id)
        {
            if npc.id == SCOUTS[0] {
                // Gamlin
                ctx.broadcast_npc_say(npc, ChatType::NpcShout, NpcStringId::IntruderDetected, &[]);
            } else {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcShout,
                    NpcStringId::OhGiantsAnIntruderHasBeenDiscovered,
                    &[],
                );
            }
            let neighbours = ctx.world().visible_objects_in_range(npc.object_id, 450);
            for obj in neighbours {
                if ctx.world().is_attackable_npc(obj)
                    && ctx.get_random_boolean()
                    && let Some(other) = ctx.script_npc(obj)
                {
                    ctx.add_attack_desire(other, player, 999);
                }
            }
        } else if event == "CLEAR"
            && let Some(npc) = npc
            && !ctx.is_dead(npc.object_id)
        {
            ctx.set_script_value(npc, 0);
        }
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.is_script_value(npc, 0) {
            ctx.set_script_value(npc, 1);
            ctx.start_quest_timer("ATTACK", 6000, Some(npc), Some(attacker), false);
            ctx.start_quest_timer("CLEAR", 120000, Some(npc), None, false);
        }
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) {
        if ctx.is_script_value(npc, 0) {
            ctx.set_script_value(npc, 1);
            if ctx.get_random_boolean() {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::YouGuysAreDetected, &[]);
            } else {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::WhatKindOfCreaturesAreYou,
                    &[],
                );
            }
            ctx.start_quest_timer("ATTACK", 6000, Some(npc), Some(player), false);
            ctx.start_quest_timer("CLEAR", 120000, Some(npc), None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"
        <npc id="22668" type="Monster" level="70" name="Gamlin"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22669" type="Monster" level="70" name="Leogul"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn the_first_hit_shouts_after_six_seconds_and_the_scout_calms_down_after_two_minutes() {
        let mut registry = ScriptRegistry::new();
        registry.add(GiantsCave);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 70, vec![]);
        let scout = t.spawn(SCOUTS[0], 100);
        let neighbour = t.spawn(SCOUTS[1], 120);
        let npc = ScriptNpc {
            object_id: scout,
            id: SCOUTS[0],
        };
        t.world.run_as_script("GiantsCave", |ctx| {
            GiantsCave.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        assert!(t.world.is_npc_script_value(scout, 1));
        t.take();
        t.wait_ms(6_200);
        let says = names_to(&t.take(), 1)
            .iter()
            .filter(|n| n.as_str() == "NPC_SAY")
            .count();
        assert_eq!(says, 1);
        t.wait_ms(114_000);
        assert!(t.world.is_npc_script_value(scout, 0));
        t.world.delete_npc(scout);
        t.world.delete_npc(neighbour);
    }

    #[test]
    fn a_player_in_aggro_range_starts_the_same_timers() {
        let mut registry = ScriptRegistry::new();
        registry.add(GiantsCave);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 70, vec![]);
        let scout = t.spawn(SCOUTS[1], 100);
        let npc = ScriptNpc {
            object_id: scout,
            id: SCOUTS[1],
        };
        t.world.run_as_script("GiantsCave", |ctx| {
            GiantsCave.on_aggro_range_enter(ctx, npc, ObjectId(1), false);
            assert!(ctx.has_quest_timer("ATTACK", Some(npc), Some(ObjectId(1))));
            assert!(ctx.has_quest_timer("CLEAR", Some(npc), None));
        });
        assert!(t.world.is_npc_script_value(scout, 1));
    }
}
