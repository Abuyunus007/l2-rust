//! Bleeding Fly (raid boss 25720): Parasitic Leeches at half and a quarter of its HP.
// Java: dist/game/data/scripts/ai/others/BleedingFly.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const BLEEDING_FLY: i32 = 25720;
const PARASITIC_LEECH: i32 = 25734;

// Skills
const SUMMON_PARASITE_LEECH: SkillKey = SkillKey { id: 6832, level: 1 };
const NPC_ACUMEN_LEVEL_3: SkillKey = SkillKey { id: 6915, level: 3 };

// Variables
const MID_HP_FLAG: &str = "MID_HP_FLAG";
const LOW_HP_FLAG: &str = "LOW_HP_FLAG";
const MID_HP_MINION_COUNT: &str = "MID_HP_MINION_COUNT";
const LOW_HP_MINION_COUNT: &str = "LOW_HP_MINION_COUNT";

// Timers
const TIMER_MID_HP: &str = "TIMER_MID_HP";
const TIMER_LOW_HP: &str = "TIMER_LOW_HP";

// Misc
const MAX_CHASE_DIST: f64 = 2500.0;
const MID_HP_PERCENTAGE: f64 = 0.50;
const MIN_HP_PERCENTAGE: f64 = 0.25;

/// Java `ai.others.BleedingFly`.
pub struct BleedingFly;

/// The two leeches of one timer step: `addSpawn(PARASITIC_LEECH, x + getRandom(150), y +
/// getRandom(150), z, heading, false, 0)` twice, the random numbers in Java's order.
fn spawn_leeches(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let Some((x, y, z, heading)) = ctx.npc_position(npc) else {
        return;
    };
    for _ in 0..2 {
        let dx = ctx.get_random(150);
        let dy = ctx.get_random(150);
        ctx.add_spawn(PARASITIC_LEECH, x + dx, y + dy, z, heading, false, 0, 0);
    }
}

impl Script for BleedingFly {
    fn name(&self) -> &'static str {
        "BleedingFly"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[BLEEDING_FLY]);
        s.add_spawn_id(&[BLEEDING_FLY]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_npc_int(npc, MID_HP_MINION_COUNT, 5);
        ctx.set_npc_int(npc, LOW_HP_MINION_COUNT, 10);
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.npc_distance_from_spawn(npc) > MAX_CHASE_DIST {
            if let Some((x, y, z)) = ctx.npc_spawn_point(npc) {
                ctx.tele_npc_to(npc, x, y, z);
            }
        }

        if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * MID_HP_PERCENTAGE
            && !ctx.npc_flag(npc, MID_HP_FLAG)
        {
            ctx.set_npc_flag(npc, MID_HP_FLAG, true);
            ctx.start_quest_timer(TIMER_MID_HP, 1000, Some(npc), None, false);
        }

        if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * MIN_HP_PERCENTAGE
            && !ctx.npc_flag(npc, LOW_HP_FLAG)
        {
            ctx.set_npc_flag(npc, MID_HP_FLAG, false);
            ctx.set_npc_flag(npc, LOW_HP_FLAG, true);
            ctx.start_quest_timer(TIMER_LOW_HP, 1000, Some(npc), None, false);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        if ctx.is_dead(npc.object_id) {
            return None;
        }

        if event == TIMER_MID_HP {
            if ctx.npc_int(npc, MID_HP_MINION_COUNT) > 0 {
                let count = ctx.npc_int(npc, MID_HP_MINION_COUNT);
                ctx.set_npc_int(npc, MID_HP_MINION_COUNT, count - 1);
                ctx.add_skill_cast_desire(npc, npc.object_id, SUMMON_PARASITE_LEECH, 99999);
                spawn_leeches(ctx, npc);
                if ctx.npc_flag(npc, MID_HP_FLAG) {
                    ctx.start_quest_timer(TIMER_MID_HP, 140000, Some(npc), None, false);
                }
            }
        } else if event == TIMER_LOW_HP && ctx.npc_int(npc, LOW_HP_MINION_COUNT) > 0 {
            let count = ctx.npc_int(npc, LOW_HP_MINION_COUNT);
            ctx.set_npc_int(npc, LOW_HP_MINION_COUNT, count - 1);
            ctx.add_skill_cast_desire(npc, npc.object_id, SUMMON_PARASITE_LEECH, 99999);
            ctx.add_skill_cast_desire(npc, npc.object_id, NPC_ACUMEN_LEVEL_3, 99999);
            spawn_leeches(ctx, npc);
            if ctx.npc_flag(npc, LOW_HP_FLAG) {
                ctx.start_quest_timer(TIMER_LOW_HP, 80000, Some(npc), None, false);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25720" type="RaidBoss" level="75" name="Bleeding Fly"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25734" type="Monster" level="70" name="Parasitic Leech"><stats><vitals hp="100" mp="10"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(BleedingFly);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let boss = t.spawn(BLEEDING_FLY, 100);
        t.world.script_on_npc_spawn(boss);
        t.world.script_drain();
        t.enter(1, 75, vec![]);
        t.take();
        (t, boss)
    }

    fn hit(t: &mut ScriptTest, boss: ObjectId) {
        t.world.script_on_attackable_attack(boss, ObjectId(1), 10, None);
        t.world.script_drain();
    }

    fn leeches(t: &ScriptTest) -> usize {
        t.world
            .npc_ids()
            .into_iter()
            .filter(|id| t.world.npc(*id).unwrap().npc_id == PARASITIC_LEECH)
            .count()
    }

    #[test]
    fn spawn_sets_the_minion_counts() {
        let (t, boss) = setup();
        assert_eq!(
            t.world.script_npc_var("BleedingFly", boss, MID_HP_MINION_COUNT),
            5
        );
        assert_eq!(
            t.world.script_npc_var("BleedingFly", boss, LOW_HP_MINION_COUNT),
            10
        );
    }

    #[test]
    fn half_hp_calls_two_leeches_every_140_seconds_up_to_five_times() {
        let (mut t, boss) = setup();
        t.world.npc_mut(boss).unwrap().core.cur_hp = 400.0;
        hit(&mut t, boss);
        assert_eq!(t.world.script_npc_var("BleedingFly", boss, MID_HP_FLAG), 1);
        assert_eq!(leeches(&t), 0);
        t.ticks(25); // 1 s
        assert_eq!(leeches(&t), 2);
        assert_eq!(
            t.world.script_npc_var("BleedingFly", boss, MID_HP_MINION_COUNT),
            4
        );
        // 140 s later the next two.
        t.ticks(2810);
        assert_eq!(leeches(&t), 4);
    }

    #[test]
    fn a_quarter_hp_switches_to_the_low_hp_timer() {
        let (mut t, boss) = setup();
        t.world.npc_mut(boss).unwrap().core.cur_hp = 200.0;
        hit(&mut t, boss);
        // Both flags were handled in one hit: the mid timer started, then the low branch
        // cleared the mid flag.
        assert_eq!(t.world.script_npc_var("BleedingFly", boss, MID_HP_FLAG), 0);
        assert_eq!(t.world.script_npc_var("BleedingFly", boss, LOW_HP_FLAG), 1);
        t.ticks(25);
        assert_eq!(
            t.world.script_npc_var("BleedingFly", boss, LOW_HP_MINION_COUNT),
            9
        );
        assert_eq!(
            t.world.script_npc_var("BleedingFly", boss, MID_HP_MINION_COUNT),
            4
        );
        // NOTE(java-quirk): the mid timer still fired once (1 s), spawning its two leeches too.
        assert_eq!(leeches(&t), 4);
    }

    #[test]
    fn a_dead_boss_calls_nothing() {
        let (mut t, boss) = setup();
        t.world.npc_mut(boss).unwrap().core.cur_hp = 400.0;
        hit(&mut t, boss);
        t.world.set_dead(boss, true);
        t.ticks(30);
        assert_eq!(leeches(&t), 0);
    }
}
