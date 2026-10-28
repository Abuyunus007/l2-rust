//! Blackdagger Wing (raid boss 25721): a ranged magic attack and a 30 s lunge below half HP.
// Java: dist/game/data/scripts/ai/others/BlackdaggerWing.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const BLACKDAGGER_WING: i32 = 25721;

// Skills
const POWER_STRIKE: SkillKey = SkillKey { id: 6833, level: 1 };
const RANGE_MAGIC_ATTACK: SkillKey = SkillKey { id: 6834, level: 1 };

// Variables
const MID_HP_FLAG: &str = "MID_HP_FLAG";
const POWER_STRIKE_CAST_COUNT: &str = "POWER_STRIKE_CAST_COUNT";

// Timers
const DAMAGE_TIMER: &str = "DAMAGE_TIMER";

// Misc
const MAX_CHASE_DIST: f64 = 2500.0;
const MID_HP_PERCENTAGE: f64 = 0.50;

/// Java `ai.others.BlackdaggerWing`.
pub struct BlackdaggerWing;

impl Script for BlackdaggerWing {
    fn name(&self) -> &'static str {
        "BlackdaggerWing"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[BLACKDAGGER_WING]);
        s.add_spell_finished_id(&[BLACKDAGGER_WING]);
        s.add_creature_see_id(&[BLACKDAGGER_WING]);
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

        if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * MID_HP_PERCENTAGE
            && !ctx.npc_flag(npc, MID_HP_FLAG)
        {
            ctx.set_npc_flag(npc, MID_HP_FLAG, true);
            ctx.start_quest_timer(DAMAGE_TIMER, 10000, Some(npc), Some(attacker), false);
        }
    }

    // The engine does not fire `on_creature_see` yet (`CreatureSeeTaskManager`).
    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.npc_flag(npc, MID_HP_FLAG) {
            let most_hated = ctx.most_hated(npc);
            if let Some(most_hated) = most_hated {
                if ctx.world().player(most_hated).is_some() && most_hated != creature && ctx.get_random(5) < 1
                {
                    ctx.add_skill_cast_desire(npc, creature, RANGE_MAGIC_ATTACK, 99999);
                }
            }
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if skill.id == POWER_STRIKE.id {
            let count = ctx.npc_int(npc, POWER_STRIKE_CAST_COUNT) + 1;
            ctx.set_npc_int(npc, POWER_STRIKE_CAST_COUNT, count);
            if count > 3 {
                // NOTE(java-quirk): `player` is the target of the finished skill; Java passes it on
                // without a null check.
                if let Some(player) = player {
                    ctx.add_skill_cast_desire(npc, player, RANGE_MAGIC_ATTACK, 99999);
                }
                ctx.set_npc_int(npc, POWER_STRIKE_CAST_COUNT, 0);
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == DAMAGE_TIMER {
            if let (Some(npc), Some(player)) = (npc, player) {
                ctx.set_intention_attack(npc, player);
            }
            ctx.start_quest_timer(DAMAGE_TIMER, 30000, npc, player, false);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25721" type="RaidBoss" level="75" name="Blackdagger Wing"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(BlackdaggerWing);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let boss = t.spawn(BLACKDAGGER_WING, 100);
        t.enter(1, 75, vec![]);
        t.take();
        (t, boss)
    }

    fn hit(t: &mut ScriptTest, boss: ObjectId) {
        t.world.script_on_attackable_attack(boss, ObjectId(1), 10, None);
        t.world.script_drain();
    }

    #[test]
    fn below_half_hp_the_damage_timer_starts_once_and_repeats_every_thirty_seconds() {
        let (mut t, boss) = setup();
        hit(&mut t, boss);
        assert_eq!(t.world.quest_timer_count(), 0);

        t.world.npc_mut(boss).unwrap().core.cur_hp = 400.0;
        hit(&mut t, boss);
        assert_eq!(t.world.script_npc_var("BlackdaggerWing", boss, MID_HP_FLAG), 1);
        assert_eq!(t.world.quest_timer_count(), 1);
        hit(&mut t, boss);
        assert_eq!(t.world.quest_timer_count(), 1, "the flag keeps it to one timer");

        // 10 s later the timer fires and starts the 30 s one.
        t.ticks(210);
        assert_eq!(t.world.quest_timer_count(), 1);
        t.ticks(600);
        assert_eq!(t.world.quest_timer_count(), 1, "it repeats");
    }

    #[test]
    fn the_fourth_power_strike_adds_the_ranged_attack_and_resets_the_count() {
        let (mut t, boss) = setup();
        let skill = POWER_STRIKE;
        for _ in 0..3 {
            t.world.script_on_spell_finished(boss, Some(ObjectId(1)), skill);
            t.world.script_drain();
        }
        assert_eq!(
            t.world
                .script_npc_var("BlackdaggerWing", boss, POWER_STRIKE_CAST_COUNT),
            3
        );
        assert!(t.world.aggro_list(boss).is_empty());
        t.world.script_on_spell_finished(boss, Some(ObjectId(1)), skill);
        t.world.script_drain();
        assert_eq!(
            t.world
                .script_npc_var("BlackdaggerWing", boss, POWER_STRIKE_CAST_COUNT),
            0
        );
        assert_eq!(
            t.world.aggro_list(boss).len(),
            1,
            "the cast desire hates the player"
        );
        // Another skill does not count.
        t.world
            .script_on_spell_finished(boss, Some(ObjectId(1)), SkillKey { id: 1, level: 1 });
        t.world.script_drain();
        assert_eq!(
            t.world
                .script_npc_var("BlackdaggerWing", boss, POWER_STRIKE_CAST_COUNT),
            0
        );
    }

    #[test]
    fn a_creature_in_sight_draws_the_ranged_attack_only_below_half_hp() {
        let (mut t, boss) = setup();
        t.enter(2, 75, vec![]);
        // Player 1 is the most hated.
        t.world.add_damage_hate(boss, ObjectId(1), 10, 100);
        t.world.script_on_creature_see(boss, ObjectId(2));
        t.world.script_drain();
        assert!(
            !t.world.aggro_contains(boss, ObjectId(2)),
            "not yet below half HP"
        );

        t.world.npc_mut(boss).unwrap().core.cur_hp = 400.0;
        hit(&mut t, boss);
        // `getRandom(5) < 1` is a one in five chance; the seeded RNG gives it within a few tries.
        for _ in 0..40 {
            t.world.script_on_creature_see(boss, ObjectId(2));
            t.world.script_drain();
        }
        assert!(t.world.aggro_contains(boss, ObjectId(2)));
    }
}
