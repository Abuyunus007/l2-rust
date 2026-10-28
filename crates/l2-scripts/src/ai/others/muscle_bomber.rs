//! Muscle Bomber (raid boss 25724): Enhance at 80% and 50% HP, and Drakos Assassins every minute.
// Java: dist/game/data/scripts/ai/others/MuscleBomber.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const MUSCLE_BOMBER: i32 = 25724;
const DRAKOS_ASSASSIN: i32 = 22823;

// Skills
const ENHANCE_LEVEL_1: SkillKey = SkillKey { id: 6842, level: 1 };
const ENHANCE_LEVEL_2: SkillKey = SkillKey { id: 6842, level: 2 };

// Variables
const HIGH_HP_FLAG: &str = "HIGH_HP_FLAG";
const MED_HP_FLAG: &str = "MED_HP_FLAG";
const LIMIT_FLAG: &str = "LIMIT_FLAG";

// Timers
const TIMER_SUMMON: &str = "TIMER_SUMMON";
const TIMER_LIMIT: &str = "TIMER_LIMIT";

// Misc
const MAX_CHASE_DIST: f64 = 2500.0;
const HIGH_HP_PERCENTAGE: f64 = 0.80;
const MED_HP_PERCENTAGE: f64 = 0.50;

/// Java `ai.others.MuscleBomber`.
pub struct MuscleBomber;

impl Script for MuscleBomber {
    fn name(&self) -> &'static str {
        "MuscleBomber"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[MUSCLE_BOMBER]);
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.npc_distance_from_spawn(npc) > MAX_CHASE_DIST {
            if let Some((x, y, z)) = ctx.npc_spawn_point(npc) {
                ctx.tele_npc_to(npc, x, y, z);
            }
        }

        if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * HIGH_HP_PERCENTAGE
            && !ctx.npc_flag(npc, HIGH_HP_FLAG)
        {
            ctx.set_npc_flag(npc, HIGH_HP_FLAG, true);
            ctx.add_skill_cast_desire(npc, npc.object_id, ENHANCE_LEVEL_1, 99999);
        }

        if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * MED_HP_PERCENTAGE
            && !ctx.npc_flag(npc, MED_HP_FLAG)
        {
            ctx.set_npc_flag(npc, MED_HP_FLAG, true);
            ctx.add_skill_cast_desire(npc, npc.object_id, ENHANCE_LEVEL_2, 99999);
            ctx.start_quest_timer(TIMER_SUMMON, 60000, Some(npc), Some(attacker), false);
            ctx.start_quest_timer(TIMER_LIMIT, 300000, Some(npc), Some(attacker), false);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        match event {
            TIMER_LIMIT => {
                ctx.set_npc_flag(npc, LIMIT_FLAG, true);
            }
            TIMER_SUMMON => {
                if !ctx.is_dead(npc.object_id) && !ctx.npc_flag(npc, LIMIT_FLAG) {
                    if let Some(player) = player {
                        if let Some((x, y, z, heading)) = ctx.npc_position(npc) {
                            for _ in 0..2 {
                                let dx = ctx.get_random(100);
                                let dy = ctx.get_random(10);
                                let assassin =
                                    ctx.add_spawn(DRAKOS_ASSASSIN, x + dx, y + dy, z, heading, false, 0, 0);
                                if let Some(assassin) = assassin {
                                    ctx.add_attack_desire(assassin, player, 999);
                                }
                            }
                        }
                    }

                    ctx.start_quest_timer(TIMER_SUMMON, 60000, Some(npc), player, false);
                }
            }
            _ => {}
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25724" type="RaidBoss" level="75" name="Muscle Bomber"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22823" type="Monster" level="70" name="Drakos Assassin"><stats><vitals hp="100" mp="10"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(MuscleBomber);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let boss = t.spawn(MUSCLE_BOMBER, 100);
        t.enter(1, 75, vec![]);
        t.take();
        (t, boss)
    }

    fn hit(t: &mut ScriptTest, boss: ObjectId) {
        t.world.script_on_attackable_attack(boss, ObjectId(1), 10, None);
        t.world.script_drain();
    }

    fn assassins(t: &ScriptTest) -> usize {
        t.world
            .npc_ids()
            .into_iter()
            .filter(|id| t.world.npc(*id).unwrap().npc_id == DRAKOS_ASSASSIN)
            .count()
    }

    #[test]
    fn enhance_comes_at_eighty_percent_and_the_summons_at_fifty() {
        let (mut t, boss) = setup();
        t.world.npc_mut(boss).unwrap().core.cur_hp = 790.0;
        hit(&mut t, boss);
        assert_eq!(t.world.script_npc_var("MuscleBomber", boss, HIGH_HP_FLAG), 1);
        assert_eq!(t.world.script_npc_var("MuscleBomber", boss, MED_HP_FLAG), 0);
        assert_eq!(t.world.quest_timer_count(), 0);

        t.world.npc_mut(boss).unwrap().core.cur_hp = 490.0;
        hit(&mut t, boss);
        assert_eq!(t.world.script_npc_var("MuscleBomber", boss, MED_HP_FLAG), 1);
        assert_eq!(t.world.quest_timer_count(), 2);
    }

    #[test]
    fn two_assassins_a_minute_until_the_five_minute_limit() {
        let (mut t, boss) = setup();
        t.world.npc_mut(boss).unwrap().core.cur_hp = 490.0;
        hit(&mut t, boss);
        assert_eq!(assassins(&t), 0);
        t.ticks(1210); // 60 s
        assert_eq!(assassins(&t), 2);
        t.ticks(1200);
        assert_eq!(assassins(&t), 4);
        // After 300 s the limit flag stops the summoning (the 5th minute's timer is the same
        // second as the limit; the limit timer was started first and runs first).
        t.ticks(1200 * 3);
        assert_eq!(t.world.script_npc_var("MuscleBomber", boss, LIMIT_FLAG), 1);
        let total = assassins(&t);
        t.ticks(1250);
        assert_eq!(assassins(&t), total);
    }

    #[test]
    fn a_dead_boss_summons_nothing() {
        let (mut t, boss) = setup();
        t.world.npc_mut(boss).unwrap().core.cur_hp = 490.0;
        hit(&mut t, boss);
        t.world.set_dead(boss, true);
        t.ticks(1250);
        assert_eq!(assassins(&t), 0);
    }
}
