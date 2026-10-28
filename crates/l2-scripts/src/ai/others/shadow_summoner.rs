//! Shadow Summoner (raid boss 25722): summons Demon's Banquets below 25% HP, and goes home when
//! pulled too far.
// Java: dist/game/data/scripts/ai/others/ShadowSummoner.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const SHADOW_SUMMONER: i32 = 25722;
const DEMONS_BANQUET_1: i32 = 25730;
const DEMONS_BANQUET_2: i32 = 25731;

// Skills
const SUMMON_SKELETON: SkillKey = SkillKey { id: 6835, level: 1 };

// Variables
const LOW_HP_FLAG: &str = "LOW_HP_FLAG";
const LIMIT_FLAG: &str = "LIMIT_FLAG";

// Timers
const SUMMON_TIMER: &str = "SUMMON_TIMER";
const FEED_TIMER: &str = "FEED_TIMER";
const LIMIT_TIMER: &str = "LIMIT_TIMER";
const DELAY_TIMER: &str = "DELAY_TIMER";

// Misc
const MAX_CHASE_DIST: f64 = 2500.0;
const MIN_HP_PERCENTAGE: f64 = 0.25;

/// Java `ai.others.ShadowSummoner`.
pub struct ShadowSummoner;

impl Script for ShadowSummoner {
    fn name(&self) -> &'static str {
        "ShadowSummoner"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[SHADOW_SUMMONER]);
        s.add_creature_see_id(&[SHADOW_SUMMONER]);
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

        if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * MIN_HP_PERCENTAGE
            && !ctx.npc_flag(npc, LOW_HP_FLAG)
        {
            ctx.set_npc_flag(npc, LOW_HP_FLAG, true);
            ctx.start_quest_timer(SUMMON_TIMER, 1000, Some(npc), Some(attacker), false);
            ctx.start_quest_timer(FEED_TIMER, 30000, Some(npc), Some(attacker), false);
            ctx.start_quest_timer(LIMIT_TIMER, 600000, Some(npc), Some(attacker), false);
        }
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        // `!creature.isPlayer() && creature.getId() == DEMONS_BANQUET_2`: only NPCs have a
        // `ScriptNpc`.
        if ctx.script_npc(creature).is_some_and(|c| c.id == DEMONS_BANQUET_2) {
            ctx.world_mut().clear_aggro_list(npc.object_id);
            ctx.add_attack_desire(npc, creature, 99999);
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
        if ctx.is_dead(npc.object_id) {
            return None;
        }

        if event == SUMMON_TIMER {
            if !ctx.npc_flag(npc, LIMIT_FLAG) {
                ctx.start_quest_timer(DELAY_TIMER, 5000, Some(npc), player, false);
                ctx.start_quest_timer(SUMMON_TIMER, 30000, Some(npc), player, false);
            }
        } else if event == FEED_TIMER {
            if !ctx.npc_flag(npc, LIMIT_FLAG) {
                if let Some(player) = player {
                    ctx.set_intention_attack(npc, player);
                }
                ctx.start_quest_timer(FEED_TIMER, 30000, Some(npc), player, false);
            }
        } else if event == LIMIT_TIMER {
            ctx.set_npc_flag(npc, LIMIT_FLAG, true);
        } else if event == DELAY_TIMER {
            ctx.add_skill_cast_desire(npc, npc.object_id, SUMMON_SKELETON, 99999);
            let id = if ctx.get_random(2) < 1 {
                DEMONS_BANQUET_1
            } else {
                DEMONS_BANQUET_2
            };
            if let Some((x, y, z, heading)) = ctx.npc_position(npc) {
                let banquet = ctx.add_spawn(id, x + 150, y + 150, z, heading, false, 0, 0);
                if let (Some(banquet), Some(player)) = (banquet, player) {
                    ctx.add_attack_desire(banquet, player, 10000);
                }
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

    const NPC_DEFS: &str = r#"<npc id="25722" type="RaidBoss" level="80" name="Shadow Summoner"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25730" type="Monster" level="80" name="Demon's Banquet"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25731" type="Monster" level="80" name="Demon's Banquet"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(ShadowSummoner);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let boss = t.spawn(SHADOW_SUMMONER, 100);
        t.enter(1, 80, vec![]);
        t.take();
        (t, boss)
    }

    #[test]
    fn below_a_quarter_hp_the_banquets_come_every_thirty_seconds_until_the_limit() {
        let (mut t, boss) = setup();
        t.world.npc_mut(boss).unwrap().core.cur_hp = 200.0;
        t.world.script_on_attackable_attack(boss, ObjectId(1), 10, None);
        t.world.script_drain();
        // 1 s: the summon timer; 5 s later the banquet.
        t.wait_ms(1_500);
        assert!(t.npcs_of(DEMONS_BANQUET_1).len() + t.npcs_of(DEMONS_BANQUET_2).len() == 0);
        t.wait_ms(5_000);
        let count = |t: &ScriptTest| t.npcs_of(DEMONS_BANQUET_1).len() + t.npcs_of(DEMONS_BANQUET_2).len();
        assert_eq!(count(&t), 1);
        t.wait_ms(30_000);
        assert_eq!(count(&t), 2);
        // The flag keeps the whole thing to one start.
        t.world.script_on_attackable_attack(boss, ObjectId(1), 10, None);
        t.world.script_drain();
        t.wait_ms(1_500);
        assert_eq!(count(&t), 2);
        for id in [DEMONS_BANQUET_1, DEMONS_BANQUET_2] {
            for n in t.npcs_of(id) {
                t.world.delete_npc(n);
            }
        }
    }
}
