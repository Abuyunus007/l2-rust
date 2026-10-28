//! Crimson Hatu Otis (18558): Spinning Slash every minute, Haste below 30% HP.
// Java: dist/game/data/scripts/ai/others/CrimsonHatuOtis.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// Npc
const CRIMSON_HATU_OTIS: i32 = 18558;

// Skills
const BOSS_SPINING_SLASH: SkillKey = SkillKey { id: 4737, level: 1 };
const BOSS_HASTE: SkillKey = SkillKey { id: 4175, level: 1 };

/// Java `ai.others.CrimsonHatuOtis`.
pub struct CrimsonHatuOtis;

impl Script for CrimsonHatuOtis {
    fn name(&self) -> &'static str {
        "CrimsonHatuOtis"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[CRIMSON_HATU_OTIS]);
        s.add_kill_id(&[CRIMSON_HATU_OTIS]);
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
            "SKILL" => {
                if ctx.is_dead(npc.object_id) {
                    ctx.cancel_quest_timer("SKILL", Some(npc), None);
                    return None;
                }

                // NOTE(java-quirk): the timer is started without a player, so Java targets null
                // here (`npc.setTarget(null)`) and casts at nobody.
                ctx.cast_skill_at(npc, player, BOSS_SPINING_SLASH);
                ctx.start_quest_timer("SKILL", 60000, Some(npc), None, false);
            }
            "BUFF" => {
                if ctx.is_script_value(npc, 2) {
                    ctx.cast_skill(npc, npc.object_id, BOSS_HASTE);
                }
            }
            _ => {}
        }

        None
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
        if ctx.is_script_value(npc, 0) {
            ctx.set_script_value(npc, 1);
            ctx.start_quest_timer("SKILL", 5000, Some(npc), None, false);
        } else if ctx.is_script_value(npc, 1)
            && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.3
        {
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::IVeHadItUpToHereWithYouILlTakeCareOfYou,
                &[],
            );
            ctx.set_script_value(npc, 2);
            ctx.start_quest_timer("BUFF", 1000, Some(npc), None, false);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.cancel_quest_timer("SKILL", Some(npc), None);
        ctx.cancel_quest_timer("BUFF", Some(npc), None);
        // NOTE(java-quirk): the script value stays 1 or 2 after the kill, and a respawn brings
        // the same NPC back with it, so the slash timer never starts again.
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="18558" type="Monster" level="60" name="Crimson Hatu Otis"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(CrimsonHatuOtis);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let boss = t.spawn(CRIMSON_HATU_OTIS, 100);
        t.enter(1, 60, vec![]);
        t.take();
        (t, boss)
    }

    fn hit(t: &mut ScriptTest, boss: ObjectId) {
        t.world.script_on_attackable_attack(boss, ObjectId(1), 10, None);
        t.world.script_drain();
    }

    #[test]
    fn the_first_hit_starts_the_slash_timer_which_repeats_every_minute() {
        let (mut t, boss) = setup();
        hit(&mut t, boss);
        assert_eq!(i64::from(t.world.npc_script_value(boss)), 1);
        assert_eq!(t.world.quest_timer_count(), 1);
        t.ticks(110); // 5 s: the slash, and the next one in 60 s.
        assert_eq!(t.world.quest_timer_count(), 1);
        t.ticks(1210);
        assert_eq!(t.world.quest_timer_count(), 1);
    }

    #[test]
    fn below_thirty_percent_it_shouts_and_hastes_once() {
        let (mut t, boss) = setup();
        hit(&mut t, boss);
        hit(&mut t, boss);
        assert_eq!(i64::from(t.world.npc_script_value(boss)), 1);
        t.world.npc_mut(boss).unwrap().core.cur_hp = 290.0;
        t.take();
        hit(&mut t, boss);
        assert_eq!(i64::from(t.world.npc_script_value(boss)), 2);
        assert!(names_to(&t.take(), 1).contains(&"NPC_SAY".to_owned()));
        // Not again.
        hit(&mut t, boss);
        assert!(!names_to(&t.take(), 1).contains(&"NPC_SAY".to_owned()));
    }

    #[test]
    fn the_kill_cancels_the_timers() {
        let (mut t, boss) = setup();
        hit(&mut t, boss);
        assert_eq!(t.world.quest_timer_count(), 1);
        t.world.script_on_attackable_kill(boss, Some(ObjectId(1)));
        t.ticks(60); // onKillDelay 2.5 s
        assert_eq!(t.world.quest_timer_count(), 0);
        assert_eq!(i64::from(t.world.npc_script_value(boss)), 1);
    }
}
