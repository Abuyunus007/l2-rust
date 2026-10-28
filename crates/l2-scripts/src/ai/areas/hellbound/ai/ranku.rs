//! Ranku's AI: his minions are strangled while he lives and Ranku's second kind comes up when
//! one of them dies.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Ranku.java

use std::collections::HashSet;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const RANKU: i32 = 25542;
const MINION: i32 = 32305;
const MINION_2: i32 = 25543;

/// Java `MY_TRACKING_SET`.
#[derive(Default)]
struct RankuState {
    my_tracking_set: HashSet<ObjectId>,
}

/// Java `ai.areas.Hellbound.AI.Ranku`.
pub struct Ranku;

impl Script for Ranku {
    fn name(&self) -> &'static str {
        "Ranku"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[RANKU]);
        s.add_kill_id(&[RANKU, MINION]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        if event.eq_ignore_ascii_case("checkup") && npc.id == RANKU && !ctx.is_dead(npc.object_id) {
            let minions: Vec<ObjectId> = ctx.world().minions_of(npc.object_id).to_vec();
            for minion in minions {
                if !ctx.is_dead(minion) && ctx.state::<RankuState>().my_tracking_set.contains(&minion) {
                    let players = ctx.world().visible_players(minion);
                    let killer = ctx.get_random_entry(&players);
                    let max_hp = ctx.world().stats(minion).map_or(0, |s| s.max_hp());
                    ctx.world_mut()
                        .reduce_current_hp(minion, f64::from(max_hp / 100), killer);
                }
            }

            ctx.start_quest_timer("checkup", 1000, Some(npc), None, false);
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
        if npc.id == RANKU {
            let minions: Vec<ObjectId> = ctx.world().minions_of(npc.object_id).to_vec();
            for minion in minions {
                if !ctx.is_dead(minion) && !ctx.state::<RankuState>().my_tracking_set.contains(&minion) {
                    if let Some(minion_npc) = ctx.script_npc(minion) {
                        ctx.broadcast_npc_say(
                            minion_npc,
                            ChatType::NpcGeneral,
                            NpcStringId::DonTKillMePleaseSomethingSStranglingMe,
                            &[],
                        );
                    }
                    ctx.start_quest_timer("checkup", 1000, Some(npc), None, false);
                    ctx.state::<RankuState>().my_tracking_set.insert(minion);
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        if npc.id == MINION {
            ctx.state::<RankuState>().my_tracking_set.remove(&npc.object_id);

            let master = ctx
                .world()
                .script_leader_of(npc.object_id)
                .filter(|m| !ctx.is_dead(*m))
                .and_then(|m| ctx.script_npc(m));
            if let Some(master) = master {
                let at = ctx.world().position_of(npc.object_id);
                if let Some(minion2) = ctx.spawn_minion(master, MINION_2)
                    && let Some((x, y, z)) = at
                {
                    ctx.tele_npc_to(minion2, x, y, z);
                }
            }
        } else if npc.id == RANKU {
            let minions: Vec<ObjectId> = ctx.world().minions_of(npc.object_id).to_vec();
            for minion in minions {
                ctx.state::<RankuState>().my_tracking_set.remove(&minion);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25542" type="Monster" level="80" name="Ranku"/>
        <npc id="32305" type="Monster" level="80" name="Ranku's Minion"/>
        <npc id="25543" type="Monster" level="80" name="Ranku's Second Minion"/>"#;

    #[test]
    fn a_hit_on_ranku_starts_the_strangling_and_a_dead_minion_becomes_the_second_kind() {
        let mut registry = ScriptRegistry::new();
        registry.add(Ranku);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let ranku = t.spawn(RANKU, 20);
        let mut minion = None;
        t.world.run_as_script("Ranku", |ctx| {
            let master = ctx.script_npc(ranku).unwrap();
            minion = ctx.spawn_minion(master, MINION);
        });
        let minion = minion.unwrap();
        t.world.run_as_script("Ranku", |ctx| {
            let npc = ctx.script_npc(ranku).unwrap();
            Ranku.on_attack(ctx, npc, ObjectId(1), 5, false, None);
            assert!(ctx.has_quest_timer("checkup", Some(npc), None));
            assert!(
                ctx.state::<RankuState>()
                    .my_tracking_set
                    .contains(&minion.object_id)
            );
        });
        t.kill(1, minion.object_id);
        assert_eq!(t.npcs_of(MINION_2).len(), 1);
        t.world.run_as_script("Ranku", |ctx| {
            assert!(ctx.state::<RankuState>().my_tracking_set.is_empty());
        });
    }
}
