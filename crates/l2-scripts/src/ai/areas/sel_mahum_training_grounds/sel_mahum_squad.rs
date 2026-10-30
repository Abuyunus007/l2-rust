//! Sel Mahum Training Ground AI for squads and chefs: the chef feeds the squads around the camp
//! fire, the squad leaders walk to the fire and eat or sleep there.
// Java: dist/game/data/scripts/ai/areas/SelMahumTrainingGrounds/SelMahumSquad.java
//
// `setWalking` / `setRunning` are the engine's `set_walking` / `set_running` (S-API-05).
// `broadcastSay(.., 1250)` (radius) is the plain `broadcast_npc_say`.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::ai::areas::gracia::util::set_display_effect;

// NPCs
const CHEF: i32 = 18908;
const FIRE: i32 = 18927;
const STOVE: i32 = 18933;

const OHS_WEAPON: i32 = 15280;
const THS_WEAPON: i32 = 15281;

// Sel Mahum Squad Leaders
const SQUAD_LEADERS: [i32; 3] = [22786, 22787, 22788];

const CHEF_FSTRINGS: [NpcStringId; 2] = [NpcStringId::IBroughtTheFood, NpcStringId::ComeAndEat];

const FIRE_EFFECT_BURN: i32 = 1;
const FIRE_EFFECT_NONE: i32 = 2;

const MAHUM_EFFECT_EAT: i32 = 1;
const MAHUM_EFFECT_SLEEP: i32 = 2;
const MAHUM_EFFECT_NONE: i32 = 3;

fn var_int(ctx: &ScriptCtx<'_>, npc: ScriptNpc, name: &str) -> i32 {
    ctx.world()
        .npc_variables(npc.object_id)
        .map_or(0, |v| v.get_int(name, 0))
}

fn set_var(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, name: &str, value: i32) {
    ctx.world_mut()
        .npc_variables_mut(npc.object_id)
        .set_int(name, value);
}

fn remove_var(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, name: &str) {
    ctx.world_mut().npc_variables_mut(npc.object_id).remove(name);
}

fn is_dead(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    ctx.is_dead(npc.object_id)
}

/// Java `Npc.isRandomWalkingEnabled()`.
fn random_walking_enabled(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    ctx.is_random_walking_enabled(npc)
}

/// Java `Npc.setDisplayEffect(value)`.
fn display(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, value: i32) {
    set_display_effect(ctx, npc.object_id, value);
}

/// Java `sender.getPointInRange(100, 200)`, then `receiver.stopMove(null)`, the destination
/// variables and `setIntentionMoveTo(loc)`.
fn go_to_the_fire(ctx: &mut ScriptCtx<'_>, receiver: ScriptNpc, sender: ScriptNpc) {
    let Some((sx, sy, sz)) = ctx.world().position_of(sender.object_id) else {
        return;
    };
    let radius = f64::from(ctx.get_random_range(100, 200));
    let angle = ctx.next_double() * 2.0 * std::f64::consts::PI;
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = (
        (f64::from(sx) + radius * angle.cos()) as i32,
        (f64::from(sy) + radius * angle.sin()) as i32,
    );
    ctx.world_mut().stop_move(receiver.object_id, None);
    set_var(ctx, receiver, "DESTINATION_X", x);
    set_var(ctx, receiver, "DESTINATION_Y", y);
    ctx.npc_move_to(receiver, x, y, sz);
}

/// Java `healPlayer(npc, player)`.
fn heal_player(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: Option<ObjectId>) {
    let fighting = ctx.world().script_intention_is_attack(npc.object_id)
        || ctx.world().script_intention_is_cast(npc.object_id);
    if let Some(player) = player
        && !ctx.is_dead(player)
        && var_int(ctx, npc, "INVUL_REMOVE_TIMER_STARTED") != 1
        && fighting
    {
        ctx.cast_skill_at(npc, Some(player), SkillKey { id: 6330, level: 1 });
    } else {
        ctx.cancel_quest_timer("chef_set_invul", Some(npc), None);
        remove_var(ctx, npc, "BUSY_STATE");
        remove_var(ctx, npc, "INVUL_REMOVE_TIMER_STARTED");
        ctx.set_walking(npc);
    }
}

/// Java `handlePreAttackMotion(attacked)`.
fn handle_pre_attack_motion(ctx: &mut ScriptCtx<'_>, attacked: ScriptNpc) {
    ctx.cancel_quest_timer("remove_effects", Some(attacked), None);
    remove_var(ctx, attacked, "BUSY_STATE");
    ctx.set_random_walking(attacked, true);
    display(ctx, attacked, MAHUM_EFFECT_NONE);
    if ctx.world().script_npc_right_hand(attacked.object_id) == OHS_WEAPON {
        ctx.world_mut().set_npc_right_hand(attacked.object_id, THS_WEAPON);
    }
}

/// Java `ai.areas.SelMahumTrainingGrounds.SelMahumSquad`.
pub struct SelMahumSquad;

impl Script for SelMahumSquad {
    fn name(&self) -> &'static str {
        "SelMahumSquad"
    }

    fn path(&self) -> &'static str {
        "ai/areas/SelMahumTrainingGrounds"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[CHEF]);
        s.add_attack_id(&SQUAD_LEADERS);
        s.add_event_received_id(&[CHEF, FIRE, STOVE]);
        s.add_event_received_id(&SQUAD_LEADERS);
        s.add_faction_call_id(&SQUAD_LEADERS);
        s.add_kill_id(&[CHEF]);
        s.add_move_finished_id(&SQUAD_LEADERS);
        s.add_node_arrived_id(&[CHEF]);
        s.add_skill_see_id(&[STOVE]);
        s.add_spawn_id(&[CHEF, FIRE]);
        s.add_spawn_id(&SQUAD_LEADERS);
        s.add_spell_finished_id(&[CHEF]);
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
            "chef_disable_reward" => set_var(ctx, npc, "REWARD_TIME_GONE", 1),
            "chef_heal_player" => heal_player(ctx, npc, player),
            "chef_remove_invul" => {
                if ctx.world().is_monster_npc(npc.object_id) {
                    ctx.world_mut().set_invul(npc.object_id, false);
                    remove_var(ctx, npc, "INVUL_REMOVE_TIMER_STARTED");
                    if let Some(player) = player
                        && !ctx.is_dead(player)
                        && ctx.world().script_is_in_surrounding_region(npc.object_id, player)
                    {
                        ctx.add_attack_desire(npc, player, 999);
                    }
                }
            }
            "chef_set_invul" => {
                if !is_dead(ctx, npc) {
                    ctx.world_mut().set_invul(npc.object_id, true);
                }
            }
            "fire" => {
                let delay = 30_000 + i64::from(ctx.get_random(5000));
                ctx.start_quest_timer("fire", delay, Some(npc), None, false);
                display(ctx, npc, FIRE_EFFECT_NONE);

                let bound = if ctx.world().script_is_night() { 2 } else { 4 };
                if ctx.get_random(bound) < 1 {
                    display(ctx, npc, FIRE_EFFECT_BURN); // fire burns
                    ctx.world_mut()
                        .broadcast_event(npc.object_id, "SCE_CAMPFIRE_START", 600, None);
                } else {
                    display(ctx, npc, FIRE_EFFECT_NONE); // fire goes out
                    ctx.world_mut()
                        .broadcast_event(npc.object_id, "SCE_CAMPFIRE_END", 600, None);
                }
            }
            "fire_arrived" => {
                // myself.i_quest0 = 1;
                ctx.set_walking(npc);
                ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));

                if !random_walking_enabled(ctx, npc) {
                    ctx.world_mut()
                        .do_cast(npc.object_id, SkillKey { id: 6331, level: 1 });
                    display(ctx, npc, MAHUM_EFFECT_SLEEP);
                }

                if var_int(ctx, npc, "BUSY_STATE") == 1 {
                    // Eating
                    ctx.world_mut()
                        .do_cast(npc.object_id, SkillKey { id: 6332, level: 1 });
                    display(ctx, npc, MAHUM_EFFECT_EAT);
                }

                ctx.start_quest_timer("remove_effects", 300_000, Some(npc), None, false);
            }
            "notify_dinner" => {
                ctx.world_mut()
                    .broadcast_event(npc.object_id, "SCE_DINNER_EAT", 600, None);
            }
            "remove_effects" => {
                // myself.i_quest0 = 0;
                ctx.set_running(npc);
                display(ctx, npc, MAHUM_EFFECT_NONE);
            }
            "reset_full_bottle_prize" => remove_var(ctx, npc, "FULL_BARREL_REWARDING_PLAYER"),
            "return_from_fire" => {
                if ctx.world().is_monster_npc(npc.object_id) && !is_dead(ctx, npc) {
                    ctx.return_home(npc);
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
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if npc.id == CHEF && var_int(ctx, npc, "BUSY_STATE") == 0 {
            if var_int(ctx, npc, "INVUL_REMOVE_TIMER_STARTED") == 0 {
                ctx.start_quest_timer("chef_remove_invul", 180_000, Some(npc), Some(attacker), false);
                ctx.start_quest_timer("chef_disable_reward", 60_000, Some(npc), None, false);
                set_var(ctx, npc, "INVUL_REMOVE_TIMER_STARTED", 1);
            }

            ctx.start_quest_timer("chef_heal_player", 1000, Some(npc), Some(attacker), false);
            ctx.start_quest_timer("chef_set_invul", 60_000, Some(npc), None, false);
            set_var(ctx, npc, "BUSY_STATE", 1);
        } else if SQUAD_LEADERS.contains(&npc.id) {
            handle_pre_attack_motion(ctx, npc);
        }
    }

    fn on_faction_call(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _caller: ScriptNpc,
        _attacker: ObjectId,
        _is_summon: bool,
    ) {
        handle_pre_attack_motion(ctx, npc);
    }

    fn on_event_received(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_name: &str,
        sender: ScriptNpc,
        receiver: ScriptNpc,
        reference: Option<ObjectId>,
    ) -> Option<String> {
        let leader = SQUAD_LEADERS.contains(&receiver.id);
        match event_name {
            "SCE_DINNER_CHECK" => {
                if receiver.id == FIRE {
                    display(ctx, receiver, FIRE_EFFECT_BURN);
                    if let Some((x, y, z)) = ctx.world().position_of(receiver.object_id) {
                        ctx.add_spawn_summoned(receiver, STOVE, x, y, z + 100, 0, false, 0, 0);
                    }
                    // @SCE_DINNER_EAT
                    ctx.start_quest_timer("notify_dinner", 2000, Some(receiver), None, false);
                    let pick = usize::try_from(ctx.get_random(2)).unwrap_or(0);
                    ctx.broadcast_npc_say(sender, ChatType::NpcGeneral, CHEF_FSTRINGS[pick], &[]);
                }
            }
            "SCE_CAMPFIRE_START" => {
                if random_walking_enabled(ctx, receiver)
                    && !is_dead(ctx, receiver)
                    && !ctx.world().script_intention_is_attack(receiver.object_id)
                    && leader
                {
                    ctx.set_random_walking(receiver, false); // Moving to fire - i_ai0 = 1
                    ctx.set_running(receiver);
                    go_to_the_fire(ctx, receiver, sender);
                }
            }
            "SCE_CAMPFIRE_END" => {
                if receiver.id == STOVE && ctx.summoner(receiver) == Some(sender.object_id) {
                    ctx.delete_npc(receiver);
                } else if !ctx.world().script_intention_is_attack(receiver.object_id) && leader {
                    ctx.set_random_walking(receiver, true);
                    remove_var(ctx, receiver, "BUSY_STATE");
                    ctx.world_mut().set_npc_right_hand(receiver.object_id, THS_WEAPON);
                    ctx.start_quest_timer("return_from_fire", 3000, Some(receiver), None, false);
                }
            }
            "SCE_DINNER_EAT" => {
                if !is_dead(ctx, receiver)
                    && !ctx.world().script_intention_is_attack(receiver.object_id)
                    && var_int(ctx, receiver, "BUSY_STATE") == 0
                    && leader
                {
                    if !random_walking_enabled(ctx, receiver) {
                        // i_ai0 == 1
                        ctx.world_mut().set_npc_right_hand(receiver.object_id, THS_WEAPON);
                    }

                    ctx.set_random_walking(receiver, false); // Moving to fire - i_ai0 = 1
                    set_var(ctx, receiver, "BUSY_STATE", 1); // Eating - i_ai3 = 1
                    ctx.set_running(receiver);
                    let text = if ctx.get_random(3) < 1 {
                        NpcStringId::LooksDelicious
                    } else {
                        NpcStringId::LetSGoEat
                    };
                    ctx.broadcast_npc_say(receiver, ChatType::NpcGeneral, text, &[]);
                    go_to_the_fire(ctx, receiver, sender);
                }
            }
            "SCE_SOUP_FAILURE" => {
                if leader {
                    // TODO (Java): use it in quest 289.
                    // NOTE(java-quirk): `reference.getObjectId()` throws without a reference.
                    set_var(ctx, receiver, "FULL_BARREL_REWARDING_PLAYER", reference?.0);
                    ctx.start_quest_timer("reset_full_bottle_prize", 180_000, Some(receiver), None, false);
                }
            }
            _ => {}
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.world().is_monster_npc(npc.object_id) && var_int(ctx, npc, "REWARD_TIME_GONE") == 0 {
            ctx.drop_item(npc, Some(killer), 15492, 1);
        }

        ctx.cancel_quest_timer("chef_remove_invul", Some(npc), None);
        ctx.cancel_quest_timer("chef_disable_reward", Some(npc), None);
        ctx.cancel_quest_timer("chef_heal_player", Some(npc), None);
        ctx.cancel_quest_timer("chef_set_invul", Some(npc), None);
    }

    fn on_move_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        // NPC moves to fire
        let at = ctx.world().position_of(npc.object_id);
        if !random_walking_enabled(ctx, npc)
            && at.is_some_and(|(x, y, _)| {
                x == var_int(ctx, npc, "DESTINATION_X") && y == var_int(ctx, npc, "DESTINATION_Y")
            })
        {
            ctx.world_mut().set_npc_right_hand(npc.object_id, OHS_WEAPON);
            ctx.start_quest_timer("fire_arrived", 3000, Some(npc), None, false);
        }
    }

    fn on_node_arrived(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut()
            .broadcast_event(npc.object_id, "SCE_DINNER_CHECK", 300, None);
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if npc.id == STOVE && skill.id == 9075 && targets.contains(&npc.object_id) {
            ctx.world_mut()
                .do_cast(npc.object_id, SkillKey { id: 6688, level: 1 });
            ctx.world_mut()
                .broadcast_event(npc.object_id, "SCE_SOUP_FAILURE", 600, Some(caster));
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == CHEF {
            ctx.world_mut().set_invul(npc.object_id, false);
        } else if npc.id == FIRE {
            ctx.cancel_quest_timer("fire", Some(npc), None);
            ctx.start_quest_timer("fire", 1000, Some(npc), None, false);
        } else if SQUAD_LEADERS.contains(&npc.id) {
            display(ctx, npc, 3);
            ctx.set_random_walking(npc, true);
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if skill.id == 6330 {
            heal_player(ctx, npc, player);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="18908" type="Monster" level="70" name="Chef"/>
        <npc id="18927" type="Folk" level="70" name="Fire"/>
        <npc id="18933" type="Folk" level="70" name="Stove"/>
        <npc id="22786" type="Monster" level="70" name="Leader"/>"#;

    fn setup() -> (ScriptTest, ObjectId, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(SelMahumSquad);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let chef = t.spawn(CHEF, 20);
        let fire = t.spawn(FIRE, 30);
        let leader = t.spawn(22786, 40);
        t.enter(1, 70, vec![]);
        (t, chef, fire, leader)
    }

    fn run(t: &mut ScriptTest, f: impl FnOnce(&mut ScriptCtx<'_>)) {
        t.world.run_as_script("SelMahumSquad", f);
    }

    #[test]
    fn the_fire_starts_its_cycle_at_spawn_and_sets_an_effect() {
        let (mut t, _, fire, _) = setup();
        t.wait_ms(1_100);
        let effect = t.world.npc(fire).unwrap().display_effect;
        assert!(effect == FIRE_EFFECT_BURN || effect == FIRE_EFFECT_NONE);
        run(&mut t, |ctx| {
            let npc = ctx.script_npc(fire).unwrap();
            assert!(ctx.has_quest_timer("fire", Some(npc), None));
        });
    }

    #[test]
    fn a_squad_leader_goes_to_eat_when_the_dinner_is_ready() {
        let (mut t, _, fire, leader) = setup();
        run(&mut t, |ctx| {
            let (fire, leader) = (ctx.script_npc(fire).unwrap(), ctx.script_npc(leader).unwrap());
            SelMahumSquad.on_event_received(ctx, "SCE_DINNER_EAT", fire, leader, None);
            assert_eq!(var_int(ctx, leader, "BUSY_STATE"), 1);
            assert!(!random_walking_enabled(ctx, leader));
            assert_ne!(var_int(ctx, leader, "DESTINATION_X"), 0);
            // A busy leader ignores the next call.
            SelMahumSquad.on_event_received(ctx, "SCE_DINNER_EAT", fire, leader, None);
        });
        assert_eq!(t.world.script_npc_right_hand(leader), 0);
    }

    #[test]
    fn the_dinner_check_spawns_a_stove_that_the_campfire_end_removes() {
        let (mut t, _, fire, _) = setup();
        let chef = t.npcs_of(CHEF)[0];
        run(&mut t, |ctx| {
            let (fire, chef) = (ctx.script_npc(fire).unwrap(), ctx.script_npc(chef).unwrap());
            SelMahumSquad.on_event_received(ctx, "SCE_DINNER_CHECK", chef, fire, None);
        });
        let stoves = t.npcs_of(STOVE);
        assert_eq!(stoves.len(), 1);
        run(&mut t, |ctx| {
            let (fire, stove) = (ctx.script_npc(fire).unwrap(), ctx.script_npc(stoves[0]).unwrap());
            SelMahumSquad.on_event_received(ctx, "SCE_CAMPFIRE_END", fire, stove, None);
        });
        assert!(t.npcs_of(STOVE).is_empty());
    }

    #[test]
    fn the_chef_heals_cancel_the_invul_timer_and_the_reward_goes_after_a_minute() {
        let (mut t, chef, _, _) = setup();
        run(&mut t, |ctx| {
            let chef = ctx.script_npc(chef).unwrap();
            SelMahumSquad.on_attack(ctx, chef, ObjectId(1), 10, false, None);
            assert_eq!(var_int(ctx, chef, "BUSY_STATE"), 1);
            assert_eq!(var_int(ctx, chef, "INVUL_REMOVE_TIMER_STARTED"), 1);
        });
        t.wait_ms(1_100);
        // NOTE(java-quirk): `healPlayer` finds INVUL_REMOVE_TIMER_STARTED == 1 at once, so it
        // takes the "stop" branch: BUSY_STATE and the flag go and `chef_set_invul` is cancelled.
        run(&mut t, |ctx| {
            let chef = ctx.script_npc(chef).unwrap();
            assert_eq!(var_int(ctx, chef, "BUSY_STATE"), 0);
            assert!(!ctx.has_quest_timer("chef_set_invul", Some(chef), None));
        });
        t.wait_ms(60_000);
        assert!(!t.world.is_invul(chef));
        run(&mut t, |ctx| {
            let chef = ctx.script_npc(chef).unwrap();
            assert_eq!(var_int(ctx, chef, "REWARD_TIME_GONE"), 1);
        });
    }

    #[test]
    fn the_soup_failure_marks_the_leader_for_the_barrel_prize() {
        let (mut t, _, fire, leader) = setup();
        run(&mut t, |ctx| {
            let (fire, leader) = (ctx.script_npc(fire).unwrap(), ctx.script_npc(leader).unwrap());
            SelMahumSquad.on_event_received(ctx, "SCE_SOUP_FAILURE", fire, leader, Some(ObjectId(1)));
            assert_eq!(var_int(ctx, leader, "FULL_BARREL_REWARDING_PLAYER"), 1);
        });
        t.wait_ms(181_000);
        run(&mut t, |ctx| {
            let leader = ctx.script_npc(leader).unwrap();
            assert_eq!(var_int(ctx, leader, "FULL_BARREL_REWARDING_PLAYER"), 0);
        });
    }
}
