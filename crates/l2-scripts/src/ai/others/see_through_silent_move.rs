//! Monsters that see players in silent move.
// Java: dist/game/data/scripts/ai/others/SeeThroughSilentMove.java

use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const MONSTERS: [i32; 44] = [
    18001, 18002, 22199, 22215, 22216, 22217, 22327, 22746, 22747, 22748, 22749, 22750, 22751, 22752, 22753,
    22754, 22755, 22756, 22757, 22758, 22759, 22760, 22761, 22762, 22763, 22764, 22765, 22794, 22795, 22796,
    22797, 22798, 22799, 22800, 22843, 22857, 25725, 25726, 25727, 29009, 29010, 29011, 29012, 29013,
];

/// Java `ai.others.SeeThroughSilentMove`.
pub struct SeeThroughSilentMove;

impl Script for SeeThroughSilentMove {
    fn name(&self) -> &'static str {
        "SeeThroughSilentMove"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&MONSTERS);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if ctx.world().is_attackable_npc(npc.object_id) {
            ctx.world_mut().set_see_through_silent_move(npc.object_id, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="22199" type="Monster" level="50" name="Mob"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22200" type="Monster" level="50" name="Other"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn a_listed_monster_sees_through_silent_move() {
        let mut registry = ScriptRegistry::new();
        registry.add(SeeThroughSilentMove);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let mob = t.spawn(22199, 20);
        let other = t.spawn(22200, 40);
        t.world.script_drain();
        assert!(t.world.can_see_through_silent_move(mob));
        assert!(!t.world.can_see_through_silent_move(other));
    }
}
