//! Pronghorns and Frost Buffalos split into six spirits when hit with a physical skill.
// Java: dist/game/data/scripts/ai/areas/FrozenLabyrinth/FrozenLabyrinth.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// Monsters
const PRONGHORN_SPIRIT: i32 = 22087;
const PRONGHORN: i32 = 22088;
const LOST_BUFFALO: i32 = 22093;
const FROST_BUFFALO: i32 = 22094;

/// Java `ai.areas.FrozenLabyrinth.FrozenLabyrinth`.
pub struct FrozenLabyrinth;

impl Script for FrozenLabyrinth {
    fn name(&self) -> &'static str {
        "FrozenLabyrinth"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FrozenLabyrinth"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[PRONGHORN, FROST_BUFFALO]);
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        skill: Option<SkillKey>,
    ) {
        if ctx.is_script_value(npc, 0)
            && let Some(skill) = skill
            && !ctx.world().script_skill_is_magic(skill)
        {
            let spawn_id = if npc.id == PRONGHORN {
                PRONGHORN_SPIRIT
            } else {
                LOST_BUFFALO
            };
            let Some((nx, ny, nz, heading)) = ctx.npc_position(npc) else {
                return;
            };
            let mut diff = 0;
            for _ in 0..6 {
                let x = if diff < 60 { nx + diff } else { nx };
                let y = if diff >= 60 { ny + (diff - 40) } else { ny };
                if let Some(monster) = ctx.add_spawn(spawn_id, x, y, nz, heading, false, 0, 0) {
                    ctx.add_attack_desire(monster, attacker, 999);
                }
                diff += 20;
            }
            ctx.set_script_value(npc, 1);
            ctx.delete_npc(npc);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22087" type="Monster" level="80" name="Pronghorn Spirit"><stats><vitals hp="10000" mp="100"/></stats></npc>
        <npc id="22088" type="Monster" level="80" name="Pronghorn"><stats><vitals hp="10000" mp="100"/></stats></npc>"#;

    #[test]
    fn a_physical_skill_splits_the_pronghorn_once() {
        let mut registry = ScriptRegistry::new();
        registry.add(FrozenLabyrinth);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let horn = t.spawn(PRONGHORN, 300);
        let npc = ScriptNpc {
            object_id: horn,
            id: PRONGHORN,
        };
        // A plain hit does nothing.
        t.world.run_as_script("FrozenLabyrinth", |ctx| {
            FrozenLabyrinth.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        assert!(t.in_world(horn));
        // The test world has no skill table: the skill counts as physical.
        t.world.run_as_script("FrozenLabyrinth", |ctx| {
            FrozenLabyrinth.on_attack(ctx, npc, ObjectId(1), 5, false, Some(SkillKey::new(1, 1)));
        });
        assert!(!t.in_world(horn));
        let spirits = t.npcs_of(PRONGHORN_SPIRIT);
        assert_eq!(spirits.len(), 6);
        for id in spirits {
            t.world.delete_npc(id);
        }
    }
}
