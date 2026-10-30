//! The Delu lizardmen of the Plains of Dion shout about their duel and call the others to help.
// Java: dist/game/data/scripts/ai/areas/PlainsOfDion/PlainsOfDion.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const DELU_LIZARDMEN: [i32; 3] = [
    21104, // Delu Lizardman Supplier
    21105, // Delu Lizardman Special Agent
    21107, // Delu Lizardman Commander
];

const MONSTERS_MSG: [NpcStringId; 5] = [
    NpcStringId::S1HowDareYouInterruptOurFightHeyGuysHelp,
    NpcStringId::S1HeyWeReHavingADuelHere,
    NpcStringId::TheDuelIsOverAttack,
    NpcStringId::FoulKillTheCoward,
    NpcStringId::HowDareYouInterruptASacredDuelYouMustBeTaughtALesson,
];

const MONSTERS_ASSIST_MSG: [NpcStringId; 3] = [
    NpcStringId::DieYouCoward,
    NpcStringId::KillTheCoward,
    NpcStringId::WhatAreYouLookingAt,
];

/// Java `ai.areas.PlainsOfDion.PlainsOfDion`.
pub struct PlainsOfDion;

impl Script for PlainsOfDion {
    fn name(&self) -> &'static str {
        "PlainsOfDion"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PlainsOfDion"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&DELU_LIZARDMEN);
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.is_script_value(npc, 0) {
            let i = usize::try_from(ctx.get_random(5)).unwrap_or(0);
            if i < 2 {
                let name = ctx.player_name(player);
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, MONSTERS_MSG[i], &[&name]);
            } else {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, MONSTERS_MSG[i], &[]);
            }
            let range = ctx.world().script_npc_clan_help_range(npc.object_id);
            for obj in ctx.world().visible_objects_in_range(npc.object_id, range) {
                let Some(other) = ctx.script_npc(obj) else {
                    continue;
                };
                if ctx.world().is_monster_npc(obj)
                    && DELU_LIZARDMEN.contains(&other.id)
                    && !ctx.world().is_attacking_now(obj)
                    && !ctx.is_dead(obj)
                    && ctx.world().can_see_target(npc.object_id, obj)
                {
                    ctx.add_attack_desire(other, player, 999);
                    let msg = usize::try_from(ctx.get_random(3)).unwrap_or(0);
                    ctx.broadcast_npc_say(other, ChatType::NpcGeneral, MONSTERS_ASSIST_MSG[msg], &[]);
                }
            }
            ctx.set_script_value(npc, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"
        <npc id="21104" type="Monster" level="40" name="Supplier"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="21105" type="Monster" level="40" name="Agent"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn the_first_hit_shouts_once_and_the_neighbours_help() {
        let mut registry = ScriptRegistry::new();
        registry.add(PlainsOfDion);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 40, vec![]);
        let hit = t.spawn(21104, 100);
        let other = t.spawn(21105, 110);
        let npc = ScriptNpc {
            object_id: hit,
            id: 21104,
        };
        t.take();
        for _ in 0..2 {
            t.world.run_as_script("PlainsOfDion", |ctx| {
                PlainsOfDion.on_attack(ctx, npc, ObjectId(1), 5, false, None);
            });
        }
        assert!(t.world.is_npc_script_value(hit, 1));
        // One shout from the hit one on the first hit only; the helper may shout too (clan help
        // range comes from the template, 0 in this world: nobody helps).
        let says = names_to(&t.take(), 1)
            .iter()
            .filter(|n| n.as_str() == "NPC_SAY")
            .count();
        assert!(says >= 1);
        t.world.delete_npc(hit);
        t.world.delete_npc(other);
    }
}
