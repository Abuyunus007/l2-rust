//! Amaskari's and his prisoners' chat and some skill usage.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Amaskari.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::ai::areas::hellbound::engine;

// NPCs
const AMASKARI: i32 = 22449;
const AMASKARI_PRISONER: i32 = 22450;

// Skills
// private static final SkillHolder INVINCIBILITY = new SkillHolder(5417, 1);
const BUFF_ID: i32 = 4632;

// Misc
const AMASKARI_NPCSTRING_ID: [NpcStringId; 4] = [
    NpcStringId::ILlMakeEveryoneFeelTheSameSufferingAsMe,
    NpcStringId::HaHaYesDieSlowlyWrithingInPainAndAgony,
    NpcStringId::MoreNeedMoreSeverePain,
    NpcStringId::SomethingIsBurningInsideMyBody,
];
const MINIONS_NPCSTRING_ID: [NpcStringId; 4] = [
    NpcStringId::AhhMyLifeIsBeingDrainedOut,
    NpcStringId::ThankYouForSavingMe,
    NpcStringId::ItWillKillEveryone,
    NpcStringId::EeekIFeelSickYow,
];

/// Java `ai.areas.Hellbound.AI.Amaskari`.
pub struct Amaskari;

impl Script for Amaskari {
    fn name(&self) -> &'static str {
        "Amaskari"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[AMASKARI, AMASKARI_PRISONER]);
        s.add_attack_id(&[AMASKARI]);
        s.add_spawn_id(&[AMASKARI_PRISONER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("stop_toggle") {
            let npc = npc?;
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, AMASKARI_NPCSTRING_ID[2], &[]);
            ctx.world_mut().clear_aggro_list(npc.object_id);
            ctx.world_mut().set_intention_active(npc.object_id);
            ctx.world_mut().set_invul(npc.object_id, false);
            // npc.doCast(INVINCIBILITY.getSkill())
        } else if event.eq_ignore_ascii_case("onspawn_msg") && npc.is_some_and(|n| !ctx.is_dead(n.object_id))
        {
            let npc = npc?;
            if ctx.get_random(100) > 20 {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, MINIONS_NPCSTRING_ID[2], &[]);
            } else if ctx.get_random(100) > 40 {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, MINIONS_NPCSTRING_ID[3], &[]);
            }

            let delay = i64::from((ctx.get_random(8) + 1) * 30000);
            ctx.start_quest_timer("onspawn_msg", delay, Some(npc), None, false);
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
        if npc.id == AMASKARI && ctx.get_random(1000) < 25 {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, AMASKARI_NPCSTRING_ID[0], &[]);
            let minions: Vec<ObjectId> = ctx.world().minions_of(npc.object_id).to_vec();
            for minion in minions {
                if !ctx.is_dead(minion) && ctx.get_random(10) == 0 {
                    let Some(minion) = ctx.script_npc(minion) else {
                        continue;
                    };
                    ctx.broadcast_npc_say(minion, ChatType::NpcGeneral, MINIONS_NPCSTRING_ID[0], &[]);
                    let hp = ctx.npc_current_hp(minion);
                    // `minion.setCurrentHp(hp - (hp / 5))`: `double / int`.
                    ctx.world_mut()
                        .set_current_hp(minion.object_id, hp - (hp / 5.0), true);
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        if npc.id == AMASKARI_PRISONER {
            let master = ctx
                .world()
                .script_leader_of(npc.object_id)
                .filter(|m| !ctx.is_dead(*m))
                .and_then(|m| ctx.script_npc(m));
            if let Some(master) = master {
                ctx.broadcast_npc_say(master, ChatType::NpcGeneral, AMASKARI_NPCSTRING_ID[1], &[]);
                let info = ctx
                    .world()
                    .script_buff_abnormal_level_of_skill(master.object_id, BUFF_ID);
                if info == Some(3) && ctx.world().is_invul(master.object_id) {
                    let hp = ctx.npc_current_hp(master);
                    ctx.world_mut()
                        .set_current_hp(master.object_id, hp + (hp / 5.0), true);
                } else {
                    ctx.world_mut().clear_aggro_list(master.object_id);
                    ctx.world_mut().set_intention_active(master.object_id);
                    match info {
                        None => ctx
                            .world_mut()
                            .do_cast(master.object_id, SkillKey::new(BUFF_ID, 1)),
                        Some(level) if level < 3 => {
                            // `BUFF[level]` is the skill one level above the one on the master.
                            ctx.world_mut()
                                .do_cast(master.object_id, SkillKey::new(BUFF_ID, level + 1));
                        }
                        Some(_) => {
                            ctx.broadcast_npc_say(
                                master,
                                ChatType::NpcGeneral,
                                AMASKARI_NPCSTRING_ID[3],
                                &[],
                            );
                            // master.doCast(INVINCIBILITY.getSkill())
                            ctx.world_mut().set_invul(master.object_id, true);
                            ctx.start_quest_timer("stop_toggle", 10000, Some(master), None, false);
                        }
                    }
                }
            }
        } else if npc.id == AMASKARI {
            let minions: Vec<ObjectId> = ctx.world().minions_of(npc.object_id).to_vec();
            for minion in minions {
                if !ctx.is_dead(minion) {
                    let Some(minion_npc) = ctx.script_npc(minion) else {
                        continue;
                    };
                    if ctx.get_random(1000) > 300 {
                        ctx.broadcast_npc_say(minion_npc, ChatType::NpcGeneral, MINIONS_NPCSTRING_ID[1], &[]);
                    }

                    engine::update_trust(ctx, 30, true);
                    ctx.delete_npc(minion_npc);
                }
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let delay = i64::from((ctx.get_random(3) + 1) * 30000);
        ctx.start_quest_timer("onspawn_msg", delay, Some(npc), None, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::set_level;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="22449" type="Monster" level="60" name="Amaskari"/>
        <npc id="22450" type="Monster" level="60" name="Amaskari's Prisoner"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(Amaskari);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t.take();
        t
    }

    #[test]
    fn a_prisoner_talks_from_its_spawn_on() {
        let mut t = setup();
        let prisoner = crate::ai::areas::hellbound::testing::spawn(&mut t, 22450, 400);
        t.world.run_as_script("Amaskari", |ctx| {
            assert!(ctx.has_quest_timer("onspawn_msg", ctx.script_npc(prisoner), None));
        });
    }

    #[test]
    fn the_timer_repeats_while_the_prisoner_lives() {
        let mut t = setup();
        let prisoner = crate::ai::areas::hellbound::testing::spawn(&mut t, 22450, 20);
        t.take();
        t.wait_ms(100_000);
        // Whatever it said, it set the next timer.
        t.world.run_as_script("Amaskari", |ctx| {
            assert!(ctx.has_quest_timer("onspawn_msg", ctx.script_npc(prisoner), None));
        });
    }

    #[test]
    fn the_death_of_amaskari_sets_his_minions_free_for_trust() {
        let mut t = setup();
        set_level(&mut t, "Amaskari", 2);
        let boss = crate::ai::areas::hellbound::testing::spawn(&mut t, 22449, 20);
        let mut minion = None;
        t.world.run_as_script("Amaskari", |ctx| {
            let master = ctx.script_npc(boss).unwrap();
            minion = ctx.spawn_minion(master, 22450);
        });
        let minion = minion.unwrap();
        t.kill(1, boss);
        // Each live minion is worth 30 trust points and is deleted.
        assert_eq!(t.world.global_variables().get_int("HBTrust", 0), 30);
        assert!(!t.in_world(minion.object_id));
    }
}
