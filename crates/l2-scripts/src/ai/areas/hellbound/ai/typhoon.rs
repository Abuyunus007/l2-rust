//! Typhoon's Gust.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Typhoon.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const TYPHOON: i32 = 25539;

// Skills
const STORM: (i32, i32) = (5434, 1); // Gust

/// Java `ai.areas.Hellbound.AI.Typhoon`.
pub struct Typhoon;

impl Script for Typhoon {
    fn name(&self) -> &'static str {
        "Typhoon"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_aggro_range_enter_id(&[TYPHOON]);
        s.add_spawn_id(&[TYPHOON]);
    }

    // The Java constructor calls `onSpawn` for the boss `RaidBossSpawnManager` already has.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let boss = ctx
            .world()
            .npc_ids()
            .into_iter()
            .find(|id| ctx.world().npc(*id).is_some_and(|n| n.npc_id == TYPHOON && !n.decayed))
            .and_then(|id| ctx.script_npc(id));
        if let Some(boss) = boss {
            self.on_spawn(ctx, boss);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("CAST")
            && let Some(npc) = npc
            && !ctx.is_dead(npc.object_id)
        {
            ctx.world_mut()
                .do_simultaneous_cast(npc.object_id, SkillKey::new(STORM.0, STORM.1));
            ctx.start_quest_timer("CAST", 5000, Some(npc), None, false);
        }

        None
    }

    fn on_aggro_range_enter(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId, _is_summon: bool) {
        ctx.world_mut()
            .do_simultaneous_cast(npc.object_id, SkillKey::new(STORM.0, STORM.1));
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("CAST", 5000, Some(npc), None, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25539" type="RaidBoss" level="80" name="Typhoon"/>"#;

    #[test]
    fn typhoon_casts_every_five_seconds_from_its_spawn() {
        let mut registry = ScriptRegistry::new();
        registry.add(Typhoon);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let boss = crate::ai::areas::hellbound::testing::spawn(&mut t, TYPHOON, 20);
        t.world.run_as_script("Typhoon", |ctx| {
            assert!(ctx.has_quest_timer("CAST", ctx.script_npc(boss), None));
        });
        t.wait_ms(5_200);
        t.world.run_as_script("Typhoon", |ctx| {
            assert!(ctx.has_quest_timer("CAST", ctx.script_npc(boss), None));
        });
    }
}
