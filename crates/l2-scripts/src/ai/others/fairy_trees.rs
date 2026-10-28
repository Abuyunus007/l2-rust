//! The Fairy Trees: killed from nearby they call twenty Soul Guardians.
// Java: dist/game/data/scripts/ai/others/FairyTrees.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const SOUL_GUARDIAN: i32 = 27189; // Soul of Tree Guardian

const MOBS: [i32; 4] = [
    27185, // Fairy Tree of Wind
    27186, // Fairy Tree of Star
    27187, // Fairy Tree of Twilight
    27188, // Fairy Tree of Abyss
];

// Skill
const VENOMOUS_POISON: SkillKey = SkillKey { id: 4243, level: 1 }; // Venomous Poison

// Misc
const MIN_DISTANCE: f64 = 1500.0;

/// Java `ai.others.FairyTrees`.
pub struct FairyTrees;

impl Script for FairyTrees {
    fn name(&self) -> &'static str {
        "FairyTrees"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&MOBS);
        s.add_spawn_id(&MOBS);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        if ctx.distance_3d(npc.object_id, killer).is_some_and(|d| d <= MIN_DISTANCE) {
            for _ in 0..20 {
                let guardian = ctx.add_spawn_at(SOUL_GUARDIAN, npc.object_id, false, 30000);
                // NOTE(java-quirk): a summon's kill with the summon gone gives Java a null attacker
                // and a NullPointerException that ends the loop.
                let attacker = if is_summon { ctx.summon_of(killer) } else { Some(killer) };
                let (Some(guardian), Some(attacker)) = (guardian, attacker) else {
                    return;
                };
                ctx.add_attack_desire(guardian, attacker, 999);
                if ctx.get_random_boolean() {
                    ctx.cast_skill(guardian, attacker, VENOMOUS_POISON);
                }
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_random_walking(npc, false);
        ctx.set_immobilized(npc, true);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="27185" type="Monster" level="30" name="Fairy Tree of Wind"><stats><vitals hp="100" mp="10"/></stats></npc>
        <npc id="27189" type="Monster" level="30" name="Soul of Tree Guardian"><stats><vitals hp="100" mp="10"/></stats></npc>"#;

    fn setup(distance: i32) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(FairyTrees);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let tree = t.spawn(27185, distance);
        t.enter(1, 30, vec![]);
        t.take();
        (t, tree)
    }

    fn guardians(t: &ScriptTest) -> usize {
        t.world
            .npc_ids()
            .into_iter()
            .filter(|id| t.world.npc(*id).unwrap().npc_id == SOUL_GUARDIAN)
            .count()
    }

    #[test]
    fn a_tree_stands_still() {
        let (mut t, tree) = setup(100);
        // The spawn hook ran when the tree entered the world.
        t.world.script_drain();
        assert!(t.world.npc_random_walking_forbidden(tree));
    }

    #[test]
    fn a_kill_from_nearby_calls_twenty_guardians_for_thirty_seconds() {
        let (mut t, tree) = setup(100);
        t.world.script_on_attackable_kill(tree, Some(ObjectId(1)));
        t.ticks(60); // onKillDelay 2.5 s
        assert_eq!(guardians(&t), 20);
        t.ticks(700); // 30 s
        assert_eq!(guardians(&t), 0);
    }

    #[test]
    fn a_kill_from_afar_calls_nobody() {
        let (mut t, tree) = setup(100);
        // The player walks away: 1501 units from the tree.
        let (x, y, z) = t.world.position_of(tree).unwrap();
        t.world.player_mut(ObjectId(1)).unwrap().core.x = x - 1501;
        let _ = (y, z);
        t.world.script_on_attackable_kill(tree, Some(ObjectId(1)));
        t.ticks(60);
        assert_eq!(guardians(&t), 0);
    }
}
