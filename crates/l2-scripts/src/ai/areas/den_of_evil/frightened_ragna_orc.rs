//! The Frightened Ragna Orc begs for its life and promises adena it rarely pays.
// Java: dist/game/data/scripts/ai/areas/DenOfEvil/FrightenedRagnaOrc.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC ID
const MOB_ID: i32 = 18807;
// Chances
const ADENA: i64 = 10000;
const CHANCE: i32 = 1000;
const ADENA2: i64 = 1000000;
const CHANCE2: i32 = 10;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;
// Skill
const SKILL: SkillKey = SkillKey { id: 6234, level: 1 };

/// Java `ai.areas.DenOfEvil.FrightenedRagnaOrc`.
pub struct FrightenedRagnaOrc;

impl Script for FrightenedRagnaOrc {
    fn name(&self) -> &'static str {
        "FrightenedRagnaOrc"
    }

    fn path(&self) -> &'static str {
        "ai/areas/DenOfEvil"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[MOB_ID]);
        s.add_kill_id(&[MOB_ID]);
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
        if ctx.is_script_value(npc, 0) {
            ctx.set_script_value(npc, 1);
            let delay = i64::from(ctx.get_random(5) + 3) * 1000;
            ctx.start_quest_timer("say", delay, Some(npc), None, true);
        } else if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.2
            && ctx.is_script_value(npc, 1)
        {
            ctx.start_quest_timer("reward", 10000, Some(npc), Some(attacker), false);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::WaitWaitStopSaveMeAndILlGiveYou10000000Adena,
                &[],
            );
            ctx.set_script_value(npc, 2);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let msg = if ctx.get_random_boolean() {
            NpcStringId::UghACurseUponYou
        } else {
            NpcStringId::IReallyDidnTWantToFight
        };
        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, msg, &[]);
        ctx.cancel_quest_timer("say", Some(npc), None);
        ctx.cancel_quest_timer("reward", Some(npc), Some(player));
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
            "say" => {
                if ctx.is_dead(npc.object_id) || !ctx.is_script_value(npc, 1) {
                    ctx.cancel_quest_timer("say", Some(npc), None);
                    return None;
                }
                let msg = if ctx.get_random_boolean() {
                    NpcStringId::IDonTWantToFight
                } else {
                    NpcStringId::IsThisReallyNecessary
                };
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, msg, &[]);
            }
            "reward" => {
                if !ctx.is_dead(npc.object_id) && ctx.is_script_value(npc, 2) {
                    let player = player?;
                    if ctx.get_random(100000) < CHANCE2 {
                        let msg = if ctx.get_random_boolean() {
                            NpcStringId::ThThanksICouldHaveBecomeGoodFriendsWithYou
                        } else {
                            NpcStringId::ILlGiveYou10000000AdenaLikeIPromisedIMightBeAnOrcWhoKeepsMyPromises
                        };
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, msg, &[]);
                        ctx.set_script_value(npc, 3);
                        ctx.world_mut().do_cast(npc.object_id, SKILL);
                        for _ in 0..10 {
                            ctx.drop_item(npc, Some(player), ADENA_ID, ADENA2);
                        }
                    } else if ctx.get_random(100000) < CHANCE {
                        let msg = if ctx.get_random_boolean() {
                            NpcStringId::ThThanksICouldHaveBecomeGoodFriendsWithYou
                        } else {
                            NpcStringId::SorryButThisIsAllIHaveGiveMeABreak
                        };
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, msg, &[]);
                        ctx.set_script_value(npc, 3);
                        ctx.world_mut().do_cast(npc.object_id, SKILL);
                        for _ in 0..10 {
                            ctx.drop_item(npc, Some(player), ADENA_ID, ADENA);
                        }
                    } else {
                        let msg = if ctx.get_random_boolean() {
                            NpcStringId::ThanksButThatThingAbout10000000AdenaWasALieSeeYa
                        } else {
                            NpcStringId::YouRePrettyDumbToBelieveMe
                        };
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, msg, &[]);
                    }
                    ctx.start_quest_timer("despawn", 1000, Some(npc), None, false);
                }
            }
            "despawn" => {
                ctx.set_running(npc);
                if let Some((x, y, z, _)) = ctx.npc_position(npc) {
                    let x = x + ctx.get_random_range(-800, 800);
                    let y = y + ctx.get_random_range(-800, 800);
                    ctx.npc_move_to(npc, x, y, z);
                }
                ctx.delete_npc(npc);
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

    const NPCS: &str = r#"<npc id="18807" type="Monster" level="70" name="Frightened Ragna Orc"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn hit(t: &mut ScriptTest, npc: ScriptNpc) {
        t.world.run_as_script("FrightenedRagnaOrc", |ctx| {
            FrightenedRagnaOrc.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
    }

    #[test]
    fn the_first_hit_starts_the_chatter_and_a_weak_orc_asks_for_a_reward() {
        let mut registry = ScriptRegistry::new();
        registry.add(FrightenedRagnaOrc);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let orc = t.spawn(MOB_ID, 30);
        t.enter(1, 70, vec![]);
        let npc = ScriptNpc {
            object_id: orc,
            id: MOB_ID,
        };
        hit(&mut t, npc);
        assert!(t.world.is_npc_script_value(orc, 1));
        // The test world has no stats, so the "under 20 percent" test cannot be met: the
        // script value is moved by hand to what the second hit sets.
        t.world.set_npc_script_value(orc, 2);
        t.world.run_as_script("FrightenedRagnaOrc", |ctx| {
            ctx.start_quest_timer("reward", 10000, Some(npc), Some(ObjectId(1)), false);
        });
        // 10 s later it pays or lies; a second later it is gone.
        t.wait_ms(10_500);
        t.world.run_as_script("FrightenedRagnaOrc", |ctx| {
            assert!(ctx.has_quest_timer("despawn", Some(npc), None));
        });
        t.wait_ms(1_500);
        assert!(!t.in_world(orc));
    }

    #[test]
    fn a_dead_orc_stops_talking() {
        let mut registry = ScriptRegistry::new();
        registry.add(FrightenedRagnaOrc);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let orc = t.spawn(MOB_ID, 30);
        t.enter(1, 70, vec![]);
        let npc = ScriptNpc {
            object_id: orc,
            id: MOB_ID,
        };
        hit(&mut t, npc);
        assert!(t.world.run_as_script("FrightenedRagnaOrc", |ctx| {
            assert!(ctx.has_quest_timer("say", Some(npc), None));
        }));
        t.kill(1, orc);
        t.world.run_as_script("FrightenedRagnaOrc", |ctx| {
            assert!(!ctx.has_quest_timer("say", Some(npc), None));
        });
    }
}
