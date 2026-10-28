//! The monsters that run away when hit (Victims, Rabbit, Elpy).
// Java: dist/game/data/scripts/ai/others/FleeMonsters.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const MOBS: [i32; 12] = [
    18150, // Victim
    18151, // Victim
    18152, // Victim
    18153, // Victim
    18154, // Victim
    18155, // Victim
    18156, // Victim
    18157, // Victim
    20002, // Rabbit
    20432, // Elpy
    22228, // Grey Elpy
    25604, // Mutated Elpy
];

// Misc
const FLEE_DISTANCE: i32 = 500;

/// Java `ai.others.FleeMonsters`.
pub struct FleeMonsters;

impl Script for FleeMonsters {
    fn name(&self) -> &'static str {
        "FleeMonsters"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&MOBS);
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        ctx.disable_core_ai(npc, true);
        ctx.set_running(npc);

        // The flight is from the summon when the summon hit (the owner when it is gone).
        let summon = if is_summon { ctx.summon_of(attacker) } else { None };
        let attacker_loc = summon.unwrap_or(attacker);
        ctx.flee_from(npc, attacker_loc, FLEE_DISTANCE);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="20002" type="Monster" level="1" name="Rabbit"><stats><vitals hp="100" mp="10"/><speed><walk ground="40"/><run ground="120"/></speed></stats></npc>"#;

    #[test]
    fn a_hit_rabbit_runs_away_from_the_attacker() {
        let mut registry = ScriptRegistry::new();
        registry.add(FleeMonsters);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let rabbit = t.spawn(20002, 50);
        t.enter(1, 5, vec![]);
        t.take();
        let (start_x, start_y, _) = t.world.position_of(rabbit).unwrap();

        assert!(!t.world.npc_core_ai_disabled(rabbit));
        t.world.script_on_attackable_attack(rabbit, ObjectId(1), 5, None);
        t.world.script_drain();
        assert!(t.world.npc_core_ai_disabled(rabbit));

        // The player stands west of the rabbit: it runs east.
        t.ticks(100);
        let (x, y, _) = t.world.position_of(rabbit).unwrap();
        assert!(x > start_x + 20, "{x} from {start_x}");
        assert_eq!(y, start_y);
    }
}
