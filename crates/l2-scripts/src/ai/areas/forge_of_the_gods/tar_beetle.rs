//! Tar Beetle AI: the beetles of the Forge of the Gods spit tar at characters who come near.
// Java: dist/game/data/scripts/ai/areas/ForgeOfTheGods/TarBeetle.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use super::tar_beetle_spawn::{
    REFRESH_SHOTS, REFRESH_SHOTS_MS, REFRESH_SPAWN, REFRESH_SPAWN_MS, TAR_BEETLE, load, refresh_shots,
    refresh_spawn, remove_beetle,
};

// Skills
const TAR_SPITE: i32 = 6142;
const SKILLS: [SkillKey; 3] = [
    SkillKey {
        id: TAR_SPITE,
        level: 1,
    },
    SkillKey {
        id: TAR_SPITE,
        level: 2,
    },
    SkillKey {
        id: TAR_SPITE,
        level: 3,
    },
];

/// Java `ai.areas.ForgeOfTheGods.TarBeetle`.
pub struct TarBeetle;

impl Script for TarBeetle {
    fn name(&self) -> &'static str {
        "TarBeetle"
    }

    fn path(&self) -> &'static str {
        "ai/areas/ForgeOfTheGods"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_aggro_range_enter_id(&[TAR_BEETLE]);
        s.add_spell_finished_id(&[TAR_BEETLE]);
    }

    // The Java constructor creates the `TarBeetleSpawn` (its two scheduled tasks).
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        load(ctx);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            REFRESH_SPAWN => {
                refresh_spawn(ctx);
                ctx.start_quest_timer(REFRESH_SPAWN, REFRESH_SPAWN_MS, None, None, false);
            }
            REFRESH_SHOTS => {
                refresh_shots(ctx);
                ctx.start_quest_timer(REFRESH_SHOTS, REFRESH_SHOTS_MS, None, None, false);
            }
            _ => {}
        }
        None
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) {
        if ctx.get_script_value(npc) > 0 {
            let level = ctx
                .world()
                .script_buff_abnormal_level_of_skill(player, TAR_SPITE)
                .unwrap_or(0);
            if level < 3 {
                let skill = SKILLS[usize::try_from(level).unwrap_or(0)];
                if !ctx.world_mut().is_skill_disabled_of(npc.object_id, skill) {
                    ctx.cast_skill_at(npc, Some(player), skill);
                }
            }
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if skill.id == TAR_SPITE {
            let val = ctx.get_script_value(npc) - 1;
            let mp_consume = f64::from(ctx.world().script_skill_mp_consume(SKILLS[0]));
            let mp = ctx.world().npc(npc.object_id).map_or(0.0, |n| n.core.cur_mp);
            if val <= 0 || mp_consume > mp {
                remove_beetle(ctx, npc);
            } else {
                ctx.set_script_value(npc, val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::forge_of_the_gods::tar_beetle_data::SPAWN_ZONES;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="18804" type="Monster" level="50" name="Tar Beetle"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(TarBeetle);
        ScriptTest::new(registry, NPCS, "", &[])
    }

    fn total() -> usize {
        SPAWN_ZONES.iter().map(|z| z.0).sum()
    }

    #[test]
    fn the_zones_fill_up_and_every_beetle_is_set_up() {
        let mut t = setup();
        assert!(t.npcs_of(TAR_BEETLE).is_empty());
        t.wait_ms(1_100);
        let beetles = t.npcs_of(TAR_BEETLE);
        assert_eq!(beetles.len(), total());
        let first = beetles[0];
        assert_eq!(t.world.npc_script_value(first), 5);
        assert!(t.world.is_invul(first));
        assert!(t.world.npc_core_ai_disabled(first));
        assert!(t.world.npc_variables(first).unwrap().has_variable("zoneIndex"));
    }

    #[test]
    fn a_spell_uses_a_shot_and_the_last_one_removes_the_beetle() {
        let mut t = setup();
        t.wait_ms(1_100);
        let beetle = t.npcs_of(TAR_BEETLE)[0];
        let finish = |t: &mut ScriptTest| {
            t.world.run_as_script("TarBeetle", |ctx| {
                let npc = ctx.script_npc(beetle).unwrap();
                TarBeetle.on_spell_finished(ctx, npc, None, SKILLS[0]);
            });
        };
        for left in [4, 3, 2, 1] {
            finish(&mut t);
            assert_eq!(t.world.npc_script_value(beetle), left);
        }
        finish(&mut t);
        assert!(!t.in_world(beetle));
        // The next round fills the zone again.
        t.wait_ms(60_100);
        assert_eq!(t.npcs_of(TAR_BEETLE).len(), total());
    }

    #[test]
    fn unused_beetles_go_after_the_shot_round() {
        let mut t = setup();
        t.wait_ms(1_100);
        let old = t.npcs_of(TAR_BEETLE);
        t.wait_ms(299_000);
        assert!(old.iter().all(|b| !t.in_world(*b)));
    }
}
