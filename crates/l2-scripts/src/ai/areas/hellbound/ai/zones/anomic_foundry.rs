//! The Anomic Foundry: Laborers call their guards, the Evil spirits spawn while the Laborers
//! live and die.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Zones/AnomicFoundry/AnomicFoundry.java

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::ai::areas::hellbound::engine;

// NPCs
const LABORER: i32 = 22396;
const FOREMAN: i32 = 22397;
const LESSER_EVIL: i32 = 22398;
const GREATER_EVIL: i32 = 22399;

// npcId, x, y, z, heading, max count
const SPAWNS: [(i32, i32, i32, i32, i32, i32); 5] = [
    (LESSER_EVIL, 27883, 248613, -3209, -13248, 5),
    (LESSER_EVIL, 26142, 246442, -3216, 7064, 5),
    (LESSER_EVIL, 27335, 246217, -3668, -7992, 5),
    (LESSER_EVIL, 28486, 245913, -3698, 0, 10),
    (GREATER_EVIL, 28684, 244118, -3700, -22560, 10),
];

const RESPAWN_MIN: i32 = 20000;
const RESPAWN_MAX: i32 = 300000;

/// The fields of Java's `AnomicFoundry`.
struct AnomicFoundryState {
    /// `_atkIndex`.
    atk_index: HashMap<ObjectId, i32>,
    /// `respawnTime`.
    respawn_time: i32,
    /// `_spawned`.
    spawned: [i32; 5],
}

impl Default for AnomicFoundryState {
    fn default() -> Self {
        Self {
            atk_index: HashMap::new(),
            respawn_time: 60000,
            spawned: [0; 5],
        }
    }
}

/// Java `ai.areas.Hellbound.AI.Zones.AnomicFoundry.AnomicFoundry`.
pub struct AnomicFoundry;

/// Java `getSpawnGroup(npc)`: the entry of `SPAWNS` the NPC's spawn is, or -1.
fn get_spawn_group(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> i32 {
    let Some((x, y, _)) = ctx.npc_spawn_point(npc) else {
        return -1;
    };
    for (i, spawn) in SPAWNS.iter().enumerate() {
        if spawn.0 == npc.id && spawn.1 == x && spawn.2 == y {
            return i as i32;
        }
    }

    -1
}

/// Java `addSpawn(SPAWNS[i][0], .., false, 0, false)`.
fn add_group_spawn(ctx: &mut ScriptCtx<'_>, i: usize) {
    let (npc_id, x, y, z, heading, _) = SPAWNS[i];
    ctx.add_spawn(npc_id, x, y, z, heading, false, 0, 0);
}

// Zoey76: TODO: This should be done with onFactionCall(..)
/// Java `requestHelp(requester, aggressor, range, helperId)`: Java walks the spawns of the id
/// and takes the last NPC of each; here every live monster of the id counts.
fn request_help(
    ctx: &mut ScriptCtx<'_>,
    requester: ScriptNpc,
    aggressor: ObjectId,
    range: i32,
    helper_id: i32,
) {
    let Some((x, y, z)) = ctx.world().position_of(requester.object_id) else {
        return;
    };
    let helpers: Vec<ScriptNpc> = ctx
        .world()
        .npc_ids()
        .into_iter()
        .filter(|id| {
            ctx.world()
                .npc(*id)
                .is_some_and(|n| n.npc_id == helper_id && n.class.is_monster() && !n.decayed)
        })
        .filter_map(|id| ctx.script_npc(id))
        .collect();
    for monster in helpers {
        if !ctx.is_dead(monster.object_id)
            && ctx.is_inside_radius_3d(monster, x, y, z, range)
            && !ctx.is_dead(aggressor)
        {
            ctx.world_mut()
                .add_damage_hate(monster.object_id, aggressor, 0, 1000);
        }
    }
}

impl Script for AnomicFoundry {
    fn name(&self) -> &'static str {
        "AnomicFoundry"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/Zones/AnomicFoundry"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_aggro_range_enter_id(&[LABORER]);
        s.add_attack_id(&[LABORER]);
        s.add_kill_id(&[LABORER, LESSER_EVIL, GREATER_EVIL]);
        s.add_spawn_id(&[LABORER, LESSER_EVIL, GREATER_EVIL]);
        s.add_teleport_id(&[LABORER, LESSER_EVIL, GREATER_EVIL]);
    }

    // The Java constructor starts the first spawn timer.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let respawn_time = ctx.state::<AnomicFoundryState>().respawn_time;
        ctx.start_quest_timer("make_spawn_1", i64::from(respawn_time), None, None, false);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("make_spawn_1") {
            // NOTE(java-quirk): below level 10 the timer is not started again, so the spawns
            // never come back until the script is loaded again.
            if engine::level(ctx) >= 10 {
                let idx = ctx.get_random(3) as usize;
                if ctx.state::<AnomicFoundryState>().spawned[idx] < SPAWNS[idx].5 {
                    add_group_spawn(ctx, idx);
                    ctx.state::<AnomicFoundryState>().respawn_time += 10000;
                }

                let respawn_time = ctx.state::<AnomicFoundryState>().respawn_time;
                ctx.start_quest_timer("make_spawn_1", i64::from(respawn_time), None, None, false);
            }
        } else if event.eq_ignore_ascii_case("make_spawn_2") {
            if ctx.state::<AnomicFoundryState>().spawned[4] < SPAWNS[4].5 {
                add_group_spawn(ctx, 4);
            }
        } else if event.eq_ignore_ascii_case("return_laborer") {
            if let Some(npc) = npc
                && !ctx.is_dead(npc.object_id)
            {
                ctx.world_mut().return_home(npc.object_id);
            }
        } else if event.eq_ignore_ascii_case("reset_respawn_time") {
            ctx.state::<AnomicFoundryState>().respawn_time = 60000;
        }

        None
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) {
        if ctx.get_random(10000) < 2000 {
            request_help(ctx, npc, player, 500, FOREMAN);
            request_help(ctx, npc, player, 500, LESSER_EVIL);
            request_help(ctx, npc, player, 500, GREATER_EVIL);
        }
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
        let mut atk_index = ctx
            .state::<AnomicFoundryState>()
            .atk_index
            .get(&npc.object_id)
            .copied()
            .unwrap_or(0);
        if atk_index == 0 {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::EnemyInvasionHurryUp, &[]);
            ctx.cancel_quest_timer("return_laborer", Some(npc), None);
            ctx.start_quest_timer("return_laborer", 60000, Some(npc), None, false);
            let respawn_time = ctx.state::<AnomicFoundryState>().respawn_time;
            if respawn_time > RESPAWN_MIN {
                ctx.state::<AnomicFoundryState>().respawn_time -= 5000;
            } else if respawn_time <= RESPAWN_MIN && !ctx.has_quest_timer("reset_respawn_time", None, None) {
                ctx.start_quest_timer("reset_respawn_time", 600000, None, None, false);
            }
        }

        if ctx.get_random(10000) < 2000 {
            atk_index += 1;
            ctx.state::<AnomicFoundryState>()
                .atk_index
                .insert(npc.object_id, atk_index);
            request_help(ctx, npc, attacker, 1000 * atk_index, FOREMAN);
            request_help(ctx, npc, attacker, 1000 * atk_index, LESSER_EVIL);
            request_help(ctx, npc, attacker, 1000 * atk_index, GREATER_EVIL);
            if ctx.get_random(10) < 1 {
                if let Some((x, y, z, _)) = ctx.npc_position(npc) {
                    let dx = ctx.get_random_range(-800, 800);
                    let dy = ctx.get_random_range(-800, 800);
                    ctx.npc_move_to(npc, x + dx, y + dy, z);
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let group = get_spawn_group(ctx, npc);
        if group >= 0 {
            ctx.state::<AnomicFoundryState>().spawned[group as usize] -= 1;
            // `SpawnTable.removeSpawn(npc.getSpawn())`: the spawn of `addSpawn` is not kept.
        } else if npc.id == LABORER {
            if ctx.get_random(10000) < 8000 {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::ProcessShouldnTBeDelayedBecauseOfMe,
                    &[],
                );
                let respawn_time = ctx.state::<AnomicFoundryState>().respawn_time;
                if respawn_time < RESPAWN_MAX {
                    ctx.state::<AnomicFoundryState>().respawn_time += 10000;
                } else if respawn_time >= RESPAWN_MAX
                    && !ctx.has_quest_timer("reset_respawn_time", None, None)
                {
                    ctx.start_quest_timer("reset_respawn_time", 600000, None, None, false);
                }
            }

            ctx.state::<AnomicFoundryState>().atk_index.remove(&npc.object_id);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        // `SpawnTable.getInstance().addSpawn(npc.getSpawn())`: nothing to register.
        let group = get_spawn_group(ctx, npc);
        if group >= 0 {
            ctx.state::<AnomicFoundryState>().spawned[group as usize] += 1;
        }

        if npc.id == LABORER {
            ctx.set_random_walking(npc, false);
        }
    }

    fn on_teleport(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let group = get_spawn_group(ctx, npc);
        if (0..=2).contains(&group) {
            ctx.state::<AnomicFoundryState>().spawned[group as usize] -= 1;
            ctx.world_mut().script_schedule_despawn(npc.object_id, 100);
            if ctx.state::<AnomicFoundryState>().spawned[3] < SPAWNS[3].5 {
                add_group_spawn(ctx, 3);
            }
        } else if group == 3 {
            let respawn_time = ctx.state::<AnomicFoundryState>().respawn_time;
            ctx.start_quest_timer("make_spawn_2", i64::from(respawn_time) * 2, None, None, false);
            ctx.state::<AnomicFoundryState>().spawned[3] -= 1;
            ctx.world_mut().script_schedule_despawn(npc.object_id, 100);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::set_level;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="22396" type="Monster" level="80" name="Laborer"/>
        <npc id="22397" type="Monster" level="80" name="Foreman"/>
        <npc id="22398" type="Monster" level="80" name="Lesser Evil"/>
        <npc id="22399" type="Monster" level="80" name="Greater Evil"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(AnomicFoundry);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t
    }

    #[test]
    fn the_first_evil_comes_a_minute_after_the_load_at_level_ten() {
        let mut t = setup();
        set_level(&mut t, "AnomicFoundry", 10);
        t.wait_ms(60_100);
        // One of the three Lesser Evil spawns, and the delay grew by ten seconds.
        assert_eq!(t.npcs_of(LESSER_EVIL).len(), 1);
        t.world.run_as_script("AnomicFoundry", |ctx| {
            let state = ctx.state::<AnomicFoundryState>();
            assert_eq!(state.respawn_time, 70000);
            assert_eq!(state.spawned.iter().sum::<i32>(), 1);
        });
    }

    #[test]
    fn below_level_ten_the_timer_ends() {
        let mut t = setup();
        t.wait_ms(60_100);
        assert!(t.npcs_of(LESSER_EVIL).is_empty());
        t.world.run_as_script("AnomicFoundry", |ctx| {
            assert!(!ctx.has_quest_timer("make_spawn_1", None, None));
        });
    }

    #[test]
    fn a_dead_evil_frees_its_place() {
        let mut t = setup();
        set_level(&mut t, "AnomicFoundry", 10);
        t.wait_ms(60_100);
        let evil = t.npcs_of(LESSER_EVIL)[0];
        t.kill(1, evil);
        t.world.run_as_script("AnomicFoundry", |ctx| {
            assert_eq!(ctx.state::<AnomicFoundryState>().spawned.iter().sum::<i32>(), 0);
        });
    }

    #[test]
    fn the_first_hit_on_a_laborer_calls_for_help_and_shortens_the_wait() {
        let mut t = setup();
        let laborer = crate::ai::areas::hellbound::testing::spawn(&mut t, LABORER, 20);
        assert!(t.world.npc_random_walking_forbidden(laborer));
        t.world.run_as_script("AnomicFoundry", |ctx| {
            let npc = ctx.script_npc(laborer).unwrap();
            AnomicFoundry.on_attack(ctx, npc, ObjectId(1), 5, false, None);
            assert!(ctx.has_quest_timer("return_laborer", Some(npc), None));
            assert_eq!(ctx.state::<AnomicFoundryState>().respawn_time, 55000);
        });
        // A dead laborer forgets its attack index.
        t.kill(1, laborer);
        t.world.run_as_script("AnomicFoundry", |ctx| {
            assert!(ctx.state::<AnomicFoundryState>().atk_index.is_empty());
        });
    }
}
