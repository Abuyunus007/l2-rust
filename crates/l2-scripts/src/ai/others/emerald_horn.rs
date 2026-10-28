//! Emerald Horn (raid boss 25718): Reflect Attack, then Bleed or Piercing Storm.
// Java: dist/game/data/scripts/ai/others/EmeraldHorn.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const EMERALD_HORN: i32 = 25718;

// Skills
const REFLECT_ATTACK: SkillKey = SkillKey { id: 6823, level: 1 };
const PIERCING_STORM: SkillKey = SkillKey { id: 6824, level: 1 };
const BLEED_LEVEL_1: SkillKey = SkillKey { id: 6825, level: 1 };
const BLEED_LEVEL_2: SkillKey = SkillKey { id: 6825, level: 2 };

// Variables
const HIGH_DAMAGE_FLAG: &str = "HIGH_DAMAGE_FLAG";
const TOTAL_DAMAGE_COUNT: &str = "TOTAL_DAMAGE_COUNT";
const CAST_FLAG: &str = "CAST_FLAG";

// Timers
const DAMAGE_TIMER_15S: &str = "DAMAGE_TIMER_15S";

// Misc
const MAX_CHASE_DIST: f64 = 2500.0;

/// Java `ai.others.EmeraldHorn`.
pub struct EmeraldHorn;

impl Script for EmeraldHorn {
    fn name(&self) -> &'static str {
        "EmeraldHorn"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[EMERALD_HORN]);
        s.add_spell_finished_id(&[EMERALD_HORN]);
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.npc_distance_from_spawn(npc) > MAX_CHASE_DIST {
            if let Some((x, y, z)) = ctx.npc_spawn_point(npc) {
                ctx.tele_npc_to(npc, x, y, z);
            }
        }

        if ctx.is_affected_by_skill(npc.object_id, REFLECT_ATTACK.id) && ctx.npc_flag(npc, CAST_FLAG) {
            let total = ctx.npc_int(npc, TOTAL_DAMAGE_COUNT);
            ctx.set_npc_int(npc, TOTAL_DAMAGE_COUNT, total + damage);
        }

        if ctx.npc_int(npc, TOTAL_DAMAGE_COUNT) > 5000 {
            ctx.add_skill_cast_desire(npc, attacker, BLEED_LEVEL_2, 99999);
            ctx.set_npc_int(npc, TOTAL_DAMAGE_COUNT, 0);
            ctx.set_npc_flag(npc, CAST_FLAG, false);
            ctx.set_npc_flag(npc, HIGH_DAMAGE_FLAG, true);
        }

        // NOTE(java-quirk): the first block above resets the count to 0 whenever it passes 5000, so
        // this block (over 10000) can never run.
        if ctx.npc_int(npc, TOTAL_DAMAGE_COUNT) > 10000 {
            ctx.add_skill_cast_desire(npc, attacker, BLEED_LEVEL_1, 99999);
            ctx.set_npc_int(npc, TOTAL_DAMAGE_COUNT, 0);
            ctx.set_npc_flag(npc, CAST_FLAG, false);
            ctx.set_npc_flag(npc, HIGH_DAMAGE_FLAG, true);
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        _skill: SkillKey,
    ) {
        if ctx.get_random(5) < 1 {
            ctx.set_npc_int(npc, TOTAL_DAMAGE_COUNT, 0);
            ctx.set_npc_flag(npc, CAST_FLAG, true);
            ctx.add_skill_cast_desire(npc, npc.object_id, REFLECT_ATTACK, 99999);
            ctx.start_quest_timer(DAMAGE_TIMER_15S, 15 * 1000, Some(npc), player, false);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == DAMAGE_TIMER_15S {
            let npc = npc?;
            if !ctx.npc_flag(npc, HIGH_DAMAGE_FLAG) {
                if let Some(most_hated) = ctx.most_hated(npc) {
                    if ctx.is_dead(most_hated) {
                        ctx.stop_hating(npc, most_hated);
                    } else {
                        ctx.add_skill_cast_desire(npc, most_hated, PIERCING_STORM, 99999);
                    }
                }
            }

            ctx.set_npc_flag(npc, CAST_FLAG, false);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25718" type="RaidBoss" level="75" name="Emerald Horn"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(EmeraldHorn);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let boss = t.spawn(EMERALD_HORN, 100);
        t.enter(1, 75, vec![]);
        t.take();
        (t, boss)
    }

    fn hit(t: &mut ScriptTest, boss: ObjectId, damage: i32) {
        t.world
            .script_on_attackable_attack(boss, ObjectId(1), damage, None);
        t.world.script_drain();
    }

    fn finished(t: &mut ScriptTest, boss: ObjectId) {
        t.world
            .script_on_spell_finished(boss, Some(ObjectId(1)), SkillKey { id: 1, level: 1 });
        t.world.script_drain();
    }

    #[test]
    fn damage_is_only_counted_while_reflecting() {
        let (mut t, boss) = setup();
        hit(&mut t, boss, 3000);
        // No Reflect Attack effect on the boss: nothing counted.
        assert_eq!(t.world.script_npc_var("EmeraldHorn", boss, TOTAL_DAMAGE_COUNT), 0);
    }

    #[test]
    fn a_finished_spell_sometimes_starts_the_reflect_window() {
        let (mut t, boss) = setup();
        // `getRandom(5) < 1`: one in five; the seeded RNG gets there.
        for _ in 0..40 {
            finished(&mut t, boss);
        }
        assert_eq!(t.world.script_npc_var("EmeraldHorn", boss, CAST_FLAG), 1);
        assert!(t.world.quest_timer_count() >= 1);
        // 15 s later the window closes.
        t.ticks(310);
        assert_eq!(t.world.script_npc_var("EmeraldHorn", boss, CAST_FLAG), 0);
    }

    #[test]
    fn the_timer_makes_the_most_hated_the_target_of_the_piercing_storm() {
        let (mut t, boss) = setup();
        for _ in 0..40 {
            finished(&mut t, boss);
        }
        // The boss' own AI forgets a hate list that nothing keeps up, so the player hits just
        // before the 15 s timer, and the hate list is read the moment it fired.
        t.ticks(295);
        t.world.add_damage_hate(boss, ObjectId(1), 10, 100);
        for _ in 0..400 {
            t.ticks(1);
            if t.world.quest_timer_count() == 0 {
                break;
            }
        }
        assert_eq!(t.world.quest_timer_count(), 0);
        // `addSkillCastDesire(.., 99999)` added hate on the most hated player.
        assert!(t.world.aggro_list(boss)[0].hate >= 99999);
    }
}
