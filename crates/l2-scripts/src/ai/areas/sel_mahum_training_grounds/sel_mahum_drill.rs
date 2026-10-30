//! Sel Mahum Training Ground AI for drill groups: the sergeant drills the recruits with social
//! actions, the recruits scatter when he dies.
// Java: dist/game/data/scripts/ai/areas/SelMahumTrainingGrounds/SelMahumDrill.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const MAHUM_CHIEFS: [i32; 3] = [
    22775, // Sel Mahum Drill Sergeant
    22776, // Sel Mahum Training Officer
    22778, // Sel Mahum Drill Sergeant
];
const MAHUM_SOLDIERS: [i32; 5] = [
    22780, // Sel Mahum Recruit
    22782, // Sel Mahum Recruit
    22783, // Sel Mahum Soldier
    22784, // Sel Mahum Recruit
    22785, // Sel Mahum Soldier
];
const CHIEF_SOCIAL_ACTIONS: [i32; 4] = [1, 4, 5, 7];
const CHIEF_FSTRINGS: [NpcStringId; 2] = [
    NpcStringId::HowDareYouAttackMyRecruits,
    NpcStringId::WhoIsDisruptingTheOrder,
];
const SOLDIER_FSTRINGS: [NpcStringId; 2] = [NpcStringId::TheDrillmasterIsDead, NpcStringId::LineUpTheRanks];

// Chiefs event broadcast range
const TRAINING_RANGE: i32 = 1500;

/// Java `Actions`: `(socialActionId, altSocialActionId, repeatCount, repeatInterval)`.
struct Actions(i32, i32, i32, i64);

const SOLDIER_SOCIAL_ACTIONS: [Actions; 4] = [
    Actions(4, -1, 2, 2333), // SCE_TRAINING_ACTION_A
    Actions(1, -1, 2, 4333), // SCE_TRAINING_ACTION_B
    Actions(6, 5, 4, 1000),  // SCE_TRAINING_ACTION_C
    Actions(7, -1, 2, 1000), // SCE_TRAINING_ACTION_D
];

/// Java `npc.getVariables().getInt(name)` (0 when unset).
fn var(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, name: &str) -> i32 {
    ctx.world_mut().npc_variables_mut(npc.object_id).get_int(name, 0)
}

fn set_var(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, name: &str, value: i32) {
    ctx.world_mut()
        .npc_variables_mut(npc.object_id)
        .set_int(name, value);
}

/// Java `handleSocialAction(npc, action, firstCall)`.
fn handle_social_action(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, action: &Actions, first_call: bool) {
    if (var(ctx, npc, "BUSY_STATE") != 0)
        || !ctx.is_intention_active(npc)
        || !ctx.world().npc_stays_in_spawn_loc(npc.object_id)
    {
        return;
    }

    let social_action_id = if var(ctx, npc, "SOCIAL_ACTION_ALT_BEHAVIOR") == 0 {
        action.0
    } else {
        action.1
    };
    if social_action_id < 0 {
        return;
    }

    if first_call {
        set_var(ctx, npc, "SOCIAL_ACTION_REMAINED_COUNT", action.2);
    }

    ctx.broadcast_npc_social_action(npc, social_action_id);

    let remained_count = var(ctx, npc, "SOCIAL_ACTION_REMAINED_COUNT");
    if remained_count > 0 {
        set_var(ctx, npc, "SOCIAL_ACTION_REMAINED_COUNT", remained_count - 1);
        ctx.start_quest_timer("do_social_action", action.3, Some(npc), None, false);
    }
}

/// Java `SOLDIER_SOCIAL_ACTIONS[index]` (an index outside the array throws in Java).
fn soldier_action(index: i32) -> Option<&'static Actions> {
    SOLDIER_SOCIAL_ACTIONS.get(usize::try_from(index).ok()?)
}

/// Java `ai.areas.SelMahumTrainingGrounds.SelMahumDrill`.
pub struct SelMahumDrill;

impl Script for SelMahumDrill {
    fn name(&self) -> &'static str {
        "SelMahumDrill"
    }

    fn path(&self) -> &'static str {
        "ai/areas/SelMahumTrainingGrounds"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&MAHUM_CHIEFS);
        s.add_attack_id(&MAHUM_SOLDIERS);
        s.add_kill_id(&MAHUM_CHIEFS);
        s.add_event_received_id(&MAHUM_CHIEFS);
        s.add_event_received_id(&MAHUM_SOLDIERS);
        s.add_spawn_id(&MAHUM_CHIEFS);
        s.add_spawn_id(&MAHUM_SOLDIERS);
    }

    // The Java constructor starts the global return home timer.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        ctx.start_quest_timer("return_home", 120_000, None, None, true);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "do_social_action" => {
                if let Some(npc) = npc
                    && !ctx.is_dead(npc.object_id)
                {
                    if MAHUM_CHIEFS.contains(&npc.id) {
                        if (var(ctx, npc, "BUSY_STATE") == 0)
                            && ctx.is_intention_active(npc)
                            && ctx.world().npc_stays_in_spawn_loc(npc.object_id)
                        {
                            let idx = ctx.get_random(6);
                            if idx <= (CHIEF_SOCIAL_ACTIONS.len() as i32 - 1) {
                                ctx.broadcast_npc_social_action(npc, CHIEF_SOCIAL_ACTIONS[idx as usize]);
                                // Pass social action index to soldiers via script value
                                set_var(ctx, npc, "SOCIAL_ACTION_NEXT_INDEX", idx);
                                ctx.world_mut().broadcast_event(
                                    npc.object_id,
                                    "do_social_action",
                                    TRAINING_RANGE,
                                    None,
                                );
                            }
                        }

                        ctx.start_quest_timer("do_social_action", 15_000, Some(npc), None, false);
                    } else if MAHUM_SOLDIERS.contains(&npc.id) {
                        let index = var(ctx, npc, "SOCIAL_ACTION_NEXT_INDEX");
                        if let Some(action) = soldier_action(index) {
                            handle_social_action(ctx, npc, action, false);
                        }
                    }
                }
            }
            "reset_busy_state" => {
                if let Some(npc) = npc {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .remove("BUSY_STATE");
                    ctx.disable_core_ai(npc, false);
                }
            }
            "return_home" => {
                for npc_id in MAHUM_SOLDIERS {
                    for spawn in ctx.spawns_named(npc_id, "smtg_drill_group") {
                        let Some(soldier) = spawn.last_spawn.and_then(|id| ctx.script_npc(id)) else {
                            continue;
                        };
                        if !ctx.is_dead(soldier.object_id)
                            && !ctx.world().npc_stays_in_spawn_loc(soldier.object_id)
                            && (ctx.is_intention_active(soldier) || ctx.is_intention_idle(soldier))
                        {
                            ctx.set_heading_and_validate(soldier, spawn.heading);
                            let (x, y, z) = spawn.location;
                            ctx.tele_npc_to(soldier, x, y, z);
                        }
                    }
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
        if (ctx.get_random(10) < 1) && MAHUM_SOLDIERS.contains(&npc.id) {
            ctx.world_mut()
                .broadcast_event(npc.object_id, "ATTACKED", 1000, None);
        }
    }

    fn on_event_received(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_name: &str,
        sender: ScriptNpc,
        receiver: ScriptNpc,
        _reference: Option<ObjectId>,
    ) -> Option<String> {
        if !ctx.is_dead(receiver.object_id) && ctx.is_in_my_spawn_group(receiver, sender) {
            match event_name {
                "do_social_action" => {
                    if MAHUM_SOLDIERS.contains(&receiver.id) {
                        let action_index = var(ctx, sender, "SOCIAL_ACTION_NEXT_INDEX");
                        set_var(ctx, receiver, "SOCIAL_ACTION_NEXT_INDEX", action_index);
                        if let Some(action) = soldier_action(action_index) {
                            handle_social_action(ctx, receiver, action, true);
                        }
                    }
                }
                "CHIEF_DIED" => {
                    if !ctx.is_attackable(receiver) {
                        return None;
                    }

                    if MAHUM_SOLDIERS.contains(&receiver.id) {
                        if ctx.get_random(4) < 1 {
                            if let Some(text) = ctx.get_random_entry(&SOLDIER_FSTRINGS) {
                                ctx.broadcast_npc_say(receiver, ChatType::NpcGeneral, text, &[]);
                            }
                        }

                        // `receiver.canBeAttacked()` is true for an attackable NPC.
                        ctx.world_mut().clear_aggro_list(receiver.object_id);

                        ctx.disable_core_ai(receiver, true);
                        set_var(ctx, receiver, "BUSY_STATE", 1);
                        ctx.set_running(receiver);
                        if let Some((x, y, z, _)) = ctx.npc_position(receiver) {
                            let new_x = x + ctx.get_random_range(-800, 800);
                            let new_y = y + ctx.get_random_range(-800, 800);
                            ctx.npc_move_to(receiver, new_x, new_y, z);
                        }
                        ctx.start_quest_timer("reset_busy_state", 5000, Some(receiver), None, false);
                    }
                }
                "ATTACKED" => {
                    if MAHUM_CHIEFS.contains(&receiver.id) {
                        if let Some(text) = ctx.get_random_entry(&CHIEF_FSTRINGS) {
                            ctx.broadcast_npc_say(receiver, ChatType::NpcGeneral, text, &[]);
                        }
                    }
                }
                _ => {}
            }
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.world_mut()
            .broadcast_event(npc.object_id, "CHIEF_DIED", TRAINING_RANGE, None);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if MAHUM_CHIEFS.contains(&npc.id) {
            ctx.cancel_quest_timer("do_social_action", Some(npc), None);
            ctx.start_quest_timer("do_social_action", 15_000, Some(npc), None, false);
        } else if (ctx.get_random(18) < 1) && MAHUM_SOLDIERS.contains(&npc.id) {
            set_var(ctx, npc, "SOCIAL_ACTION_ALT_BEHAVIOR", 1);
        }

        // Restore AI handling by core
        ctx.disable_core_ai(npc, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22775" type="Monster" level="70" name="Sel Mahum Drill Sergeant"/>
        <npc id="22780" type="Monster" level="70" name="Sel Mahum Recruit"/>"#;

    #[test]
    fn the_sergeant_drills_every_15_seconds_and_the_recruit_follows_the_index() {
        let mut registry = ScriptRegistry::new();
        registry.add(SelMahumDrill);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let sergeant = t.spawn(22775, 20);
        let recruit = t.spawn(22780, 30);
        t.ticks(2);
        t.world.run_as_script("SelMahumDrill", |ctx| {
            assert!(ctx.has_quest_timer("return_home", None, None));
            let npc = ctx.script_npc(sergeant).unwrap();
            assert!(ctx.has_quest_timer("do_social_action", Some(npc), None));

            // A recruit performs the action its sergeant announced (index 2: action C, 4 times).
            let recruit = ctx.script_npc(recruit).unwrap();
            ctx.world_mut()
                .npc_variables_mut(recruit.object_id)
                .set_int("SOCIAL_ACTION_NEXT_INDEX", 2);
            let _ = SelMahumDrill.on_event(ctx, "do_social_action", Some(recruit), None);
        });
    }

    #[test]
    fn the_actions_are_java_s() {
        assert_eq!(
            soldier_action(2).map(|a| (a.0, a.1, a.2, a.3)),
            Some((6, 5, 4, 1000))
        );
        assert!(soldier_action(4).is_none());
    }
}
