//! The Hellbound Remnants cannot die, but the holy water frees their souls.
// Java: dist/game/data/scripts/ai/others/Remnants.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const NPCS: [i32; 3] = [18463, 18464, 18465];
const SKILL_HOLY_WATER: i32 = 2358;

/// Java `ai.others.Remnants`.
pub struct Remnants;

impl Script for Remnants {
    fn name(&self) -> &'static str {
        "Remnants"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&NPCS);
        s.add_skill_see_id(&NPCS);
        // Do not override onKill for Derek here. Let's make global Hellbound manipulations in Engine where it is possible.
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().set_mortal(npc.object_id, false);
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if skill.id == SKILL_HOLY_WATER
            && !ctx.is_dead(npc.object_id)
            && !targets.is_empty()
            && targets[0] == npc.object_id
            && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.02
        {
            ctx.do_die(npc, caster);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="18463" type="Monster" level="50" name="Remnants Ghost"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Remnants);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let ghost = t.spawn(18463, 20);
        t.enter(1, 60, vec![]);
        t.world.script_drain();
        (t, ghost)
    }

    fn see(t: &mut ScriptTest, ghost: ObjectId, skill_id: i32) {
        t.world
            .script_on_skill_see(ghost, ObjectId(1), SkillKey { id: skill_id, level: 1 }, vec![ghost]);
        t.world.script_drain();
    }

    #[test]
    fn the_ghost_is_immortal() {
        let (mut t, ghost) = setup();
        assert!(!t.world.is_mortal(ghost));
        t.world.reduce_current_hp(ghost, 5000.0, Some(ObjectId(1)));
        assert!(!t.world.is_dead(ghost));
    }

    #[test]
    fn holy_water_frees_a_weak_ghost_only() {
        let (mut t, ghost) = setup();
        t.world.npc_mut(ghost).unwrap().core.cur_hp = 500.0;
        see(&mut t, ghost, SKILL_HOLY_WATER);
        assert!(!t.world.is_dead(ghost), "not below two percent");
        see(&mut t, ghost, 1);
        t.world.npc_mut(ghost).unwrap().core.cur_hp = 10.0;
        see(&mut t, ghost, 1);
        assert!(!t.world.is_dead(ghost), "another skill");
        see(&mut t, ghost, SKILL_HOLY_WATER);
        assert!(t.world.is_dead(ghost));
    }
}
