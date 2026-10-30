//! The fortress Ballistas: a Ballista Bomb cast at one destroys it and pays the clan reputation.
// Java: dist/game/data/scripts/ai/others/Ballista.java

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const BALLISTA: [i32; 21] = [
    35685, // Shanty Fortress
    35723, // Southern Fortress
    35754, // Hive Fortress
    35792, // Valley Fortress
    35823, // Ivory Fortress
    35854, // Narsell Fortress
    35892, // Bayou Fortress
    35923, // White Sands Fortress
    35961, // Borderland Fortress
    35999, // Swamp Fortress
    36030, // Archaic Fortress
    36068, // Floran Fortress
    36106, // Cloud Mountain)
    36137, // Tanor Fortress
    36168, // Dragonspine Fortress
    36206, // Antharas's Fortress
    36244, // Western Fortress
    36282, // Hunter's Fortress
    36313, // Aaru Fortress
    36351, // Demon Fortress
    36389, // Monastic Fortress
];

// Skill
const BOMB: SkillKey = SkillKey { id: 2342, level: 1 }; // Ballista Bomb
// Misc
const MIN_CLAN_LV: i32 = 5;

/// Java `ai.others.Ballista`.
pub struct Ballista;

impl Script for Ballista {
    fn name(&self) -> &'static str {
        "Ballista"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_skill_see_id(&BALLISTA);
        s.add_spawn_id(&BALLISTA);
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if ctx.target_of(caster) == Some(npc.object_id) && ctx.get_random(100) < 40 && skill == BOMB {
            if ctx.npc_fort_siege_in_progress(npc)
                && ctx.clan_level_of(caster).is_some_and(|l| l >= MIN_CLAN_LV)
            {
                let points = ctx.script_settings().ballista_points;
                ctx.add_clan_reputation_score(caster, points);
                ctx.send_system_message(
                    caster,
                    SystemMessageId::TheBallistaHasBeenSuccessfullyDestroyedTheClanSReputationWillBeIncreased,
                );
            }

            ctx.do_die(npc, caster);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.disable_core_ai(npc, true);
        ctx.world_mut().set_mortal(npc.object_id, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="35685" type="Monster" level="50" name="Ballista"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup(target_it: bool) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Ballista);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let ballista = t.spawn(35685, 100);
        t.enter(1, 60, vec![]);
        if target_it {
            t.world.set_target(ObjectId(1), Some(ballista));
        }
        t.take();
        (t, ballista)
    }

    fn see(t: &mut ScriptTest, ballista: ObjectId, skill: SkillKey, times: usize) {
        for _ in 0..times {
            t.world
                .script_on_skill_see(ballista, ObjectId(1), skill, vec![ballista]);
            t.world.script_drain();
        }
    }

    #[test]
    fn the_core_ai_is_off() {
        let (t, ballista) = setup(false);
        assert!(t.world.npc_core_ai_disabled(ballista));
    }

    #[test]
    fn the_ballista_is_not_mortal() {
        let (t, ballista) = setup(false);
        assert!(!t.world.is_mortal(ballista));
    }

    #[test]
    fn the_bomb_destroys_a_targeted_ballista_sometimes() {
        let (mut t, ballista) = setup(true);
        see(&mut t, ballista, BOMB, 60);
        assert!(t.world.is_dead(ballista), "40 % a cast: dead within 60 casts");
    }

    #[test]
    fn another_skill_or_another_target_does_nothing() {
        let (mut t, ballista) = setup(true);
        see(&mut t, ballista, SkillKey { id: 2342, level: 2 }, 60);
        see(&mut t, ballista, SkillKey { id: 1, level: 1 }, 60);
        assert!(!t.world.is_dead(ballista));

        let (mut t, ballista) = setup(false);
        see(&mut t, ballista, BOMB, 60);
        assert!(!t.world.is_dead(ballista));
    }
}
