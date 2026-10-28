//! The Hall of Suffering, attack and defence: five rooms of monsters with a Tumor of Death each,
//! then the twins Klodekus and Klanikus, who only die together.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/HallOfSufferingAttack/HallOfSufferingAttack.java
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/HallOfSufferingDefence/HallOfSufferingDefence.java
//
// The two Java scripts are the same code with other spawns, template and quest; this module is
// that code over a [`HallData`], and `hall_of_suffering_attack.rs` / `_defence.rs` are the two
// scripts (the `hall_of_suffering!` macro writes their `impl Script`).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::ExSendUiEvent;
use l2_util::ObjectId;
use l2_world::effects::registry::EffectType;
use l2_world::{ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use super::common::{self, Worlds};

// NPCs
pub(crate) const MOUTHOFEKIMUS: i32 = 32537;
pub(crate) const TEPIOS: i32 = 32530;

// mobs
pub(crate) const KLODEKUS: i32 = 25665;
pub(crate) const KLANIKUS: i32 = 25666;
pub(crate) const TUMOR_ALIVE: i32 = 18704;
pub(crate) const TUMOR_DEAD: i32 = 32531;

pub(crate) const TUMOR_MOBIDS: [i32; 7] = [22509, 22510, 22511, 22512, 22513, 22514, 22515];
const TWIN_MOBIDS: [i32; 5] = [22509, 22510, 22511, 22512, 22513];
const TWIN_SPAWNS: [(i32, i32, i32, i32); 2] = [(25665, -173727, 218169, -9536), (25666, -173727, 218049, -9536)];
const TEPIOS_SPAWN: (i32, i32, i32) = (-173727, 218109, -9536);

const BOSS_INVUL_TIME: i64 = 30000;
const BOSS_MINION_SPAWN_TIME: i64 = 60000;
const BOSS_RESSURECT_TIME: i64 = 20000;

const INSTANCEPENALTY: i64 = 24;

/// What the attack and the defence script differ in.
pub(crate) struct HallData {
    pub name: &'static str,
    pub path: &'static str,
    pub instance_id: i32,
    pub quest: &'static str,
    pub enter_teleport: (i32, i32, i32),
    /// `ROOM_1_MOBS` .. `ROOM_5_MOBS`: `{npcId, x, y, z}`.
    pub rooms: [&'static [(i32, i32, i32, i32)]; 5],
    pub tumor_spawns: [(i32, i32, i32); 5],
}

/// Java `HSAWorld` / `HSDWorld`.
#[derive(Debug, Default)]
pub(crate) struct HsWorld {
    /// `npcList`: the NPCs of the room and whether they died.
    npc_list: Vec<(ObjectId, bool)>,
    klodekus: Option<ScriptNpc>,
    klanikus: Option<ScriptNpc>,
    is_bosses_attacked: bool,
    /// 0: instance start, 1: finish time
    store_time: [i64; 2],
}

fn world_of(ctx: &mut ScriptCtx<'_>, object: ObjectId) -> Option<i32> {
    let instance_id = common::world_of(ctx, object)?;
    ctx.state::<Worlds<HsWorld>>()
        .contains_key(&instance_id)
        .then_some(instance_id)
}

fn hs<'a>(ctx: &'a mut ScriptCtx<'_>, instance_id: i32) -> &'a mut HsWorld {
    ctx.state::<Worlds<HsWorld>>().entry(instance_id).or_default()
}

/// The constructor.
pub(crate) fn setup(s: &mut ScriptSetup) {
    s.add_start_npc(&[MOUTHOFEKIMUS]);
    s.add_start_npc(&[TEPIOS]);
    s.add_talk_id(&[MOUTHOFEKIMUS]);
    s.add_talk_id(&[TEPIOS]);

    s.add_kill_id(&[TUMOR_ALIVE]);
    s.add_kill_id(&[KLODEKUS]);
    s.add_kill_id(&[KLANIKUS]);

    s.add_attack_id(&[KLODEKUS]);
    s.add_attack_id(&[KLANIKUS]);

    s.add_skill_see_id(&TUMOR_MOBIDS);
    s.add_kill_id(&TUMOR_MOBIDS);
}

fn check_conditions(data: &HallData, ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    if ctx.is_gm(player) {
        return true;
    }

    if ctx.world().party_of(player).is_none() {
        ctx.send_system_message(player, SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter);
        return false;
    }

    if !common::is_party_leader(ctx, player) {
        ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
        return false;
    }

    let now = ctx.now_ms();
    for party_member in ctx.world().party_members_of(player) {
        let level = ctx.player_level(party_member);
        if !(75..=82).contains(&level) {
            let sm = common::named_message(
                ctx,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                party_member,
            );
            common::broadcast_to_party(ctx, player, &sm);
            return false;
        }

        if !ctx.check_if_in_range(1000, player, party_member, true) {
            let sm = common::named_message(
                ctx,
                SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                party_member,
            );
            common::broadcast_to_party(ctx, player, &sm);
            return false;
        }

        let reentertime = ctx.world().instance_time(party_member, data.instance_id);
        if now < reentertime {
            let sm = common::named_message(ctx, SystemMessageId::C1MayNotReEnterYet, party_member);
            common::broadcast_to_party(ctx, player, &sm);
            return false;
        }

        let cond_1 = ctx
            .player_quest_state(party_member, data.quest)
            .is_some_and(|qs| qs.is_cond(1));
        if !cond_1 {
            let sm = common::named_message(
                ctx,
                SystemMessageId::C1SQuestRequirementIsNotSufficientAndCannotBeEntered,
                party_member,
            );
            common::broadcast_to_party(ctx, player, &sm);
            return false;
        }
    }

    true
}

fn enter_instance(data: &HallData, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    // check for existing instances for this player
    if let Some(instance_id) = common::player_world(ctx, player) {
        // existing instance
        if !ctx.state::<Worlds<HsWorld>>().contains_key(&instance_id) {
            ctx.send_system_message(
                player,
                SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
            );
            return;
        }

        common::teleport_into(ctx, player, data.enter_teleport, instance_id);
        return;
    }

    if check_conditions(data, ctx, player) {
        let instance_id = common::create_world(ctx, data.instance_id);
        let now = ctx.now_ms();
        hs(ctx, instance_id).store_time[0] = now;
        tracing::info!(
            "{} started {} Instance: {instance_id} created by player: {}",
            data.name,
            data.instance_id,
            ctx.player_name(player)
        );
        run_tumors(data, ctx, instance_id);

        if ctx.world().party_of(player).is_none() {
            common::teleport_into(ctx, player, data.enter_teleport, instance_id);
            common::add_allowed(ctx, instance_id, player);
        } else {
            for party_member in ctx.world().party_members_of(player) {
                common::teleport_into(ctx, party_member, data.enter_teleport, instance_id);
                common::add_allowed(ctx, instance_id, party_member);
            }
        }
    }
}

/// Java `checkKillProgress(mob, world)`.
fn check_kill_progress(ctx: &mut ScriptCtx<'_>, mob: ObjectId, instance_id: i32) -> bool {
    let world = hs(ctx, instance_id);
    if let Some(entry) = world.npc_list.iter_mut().find(|(npc, _)| *npc == mob) {
        entry.1 = true;
    }

    world.npc_list.iter().all(|(_, is_dead)| *is_dead)
}

/// Java `runTumors(world)`.
fn run_tumors(data: &HallData, ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let status = common::status(ctx, instance_id);
    let room: &[(i32, i32, i32, i32)] = usize::try_from(status)
        .ok()
        .and_then(|s| data.rooms.get(s))
        .copied()
        .unwrap_or(&[]);
    for mob in room {
        if let Some(npc) = ctx.add_spawn(mob.0, mob.1, mob.2, mob.3, 0, false, 0, instance_id) {
            hs(ctx, instance_id).npc_list.push((npc.object_id, false));
        }
    }

    let Some(tumor) = usize::try_from(status)
        .ok()
        .and_then(|s| data.tumor_spawns.get(s))
        .copied()
    else {
        return;
    };
    if let Some(mob) = ctx.add_spawn(TUMOR_ALIVE, tumor.0, tumor.1, tumor.2, 0, false, 0, instance_id) {
        ctx.disable_core_ai(mob, true);
        ctx.set_immobilized(mob, true);
        common::set_hp_ratio(ctx, mob.object_id, 0.5);
        hs(ctx, instance_id).npc_list.push((mob.object_id, false));
    }
    common::inc_status(ctx, instance_id);
}

/// Java `runTwins(world)`.
fn run_twins(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    common::inc_status(ctx, instance_id);
    let (k, l) = (TWIN_SPAWNS[0], TWIN_SPAWNS[1]);
    let klodekus = ctx.add_spawn(k.0, k.1, k.2, k.3, 0, false, 0, instance_id);
    let klanikus = ctx.add_spawn(l.0, l.1, l.2, l.3, 0, false, 0, instance_id);
    let world = hs(ctx, instance_id);
    world.klodekus = klodekus;
    world.klanikus = klanikus;
    for twin in [klanikus, klodekus].into_iter().flatten() {
        ctx.world_mut().set_mortal(twin.object_id, false);
    }
}

/// Java `bossSimpleDie(boss)`: the boss looks dead without dying.
fn boss_simple_die(ctx: &mut ScriptCtx<'_>, boss: ScriptNpc) {
    // killing is only possible one time
    if ctx.is_dead(boss.object_id) {
        return;
    }

    // now reset currentHp to zero
    let w = ctx.world_mut();
    w.set_current_hp(boss.object_id, 0.0, true);
    w.set_dead(boss.object_id, true);

    // Set target to null and cancel Attack or Cast
    w.set_creature_target(boss.object_id, None);

    // Stop movement
    w.stop_move(boss.object_id, None);

    // Stop HP/MP/CP Regeneration task
    w.stop_hp_mp_regeneration(boss.object_id);

    w.stop_all_effects_except_those_that_last_through_death(boss.object_id);

    // Send the Server->Client packet StatusUpdate with current HP and MP to all other Player to inform
    w.broadcast_status_update(boss.object_id);

    // Notify Creature AI
    w.notify_action_death(boss.object_id);
}

pub(crate) fn on_skill_see(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, caster: ObjectId, skill: SkillKey) {
    if ctx
        .world_mut()
        .script_skill_has_effect_type(skill, &[EffectType::RebalanceHp, EffectType::Heal])
    {
        let mut hate = 2 * ctx.world().script_skill_effect_point(skill);
        if hate < 2 {
            hate = 1000;
        }

        ctx.world_mut()
            .add_damage_hate(npc.object_id, caster, 0, i64::from(hate));
    }
}

pub(crate) fn on_event(ctx: &mut ScriptCtx<'_>, event: &str, npc: Option<ScriptNpc>) -> Option<String> {
    let npc = npc?;
    let instance_id = world_of(ctx, npc.object_id)?;
    let (klanikus, klodekus) = {
        let world = hs(ctx, instance_id);
        (world.klanikus, world.klodekus)
    };
    if event.eq_ignore_ascii_case("spawnBossGuards") {
        let (Some(klanikus), Some(klodekus)) = (klanikus, klodekus) else {
            return None;
        };
        if !ctx.world().is_in_combat(klanikus.object_id) && !ctx.world().is_in_combat(klodekus.object_id) {
            hs(ctx, instance_id).is_bosses_attacked = false;
            return None;
        }

        let npc_instance = ctx.world().instance_id_of(npc.object_id);
        let most_hated = ctx.most_hated(npc);
        if let Some(id) = ctx.get_random_entry(&TWIN_MOBIDS) {
            let at = TWIN_SPAWNS[0];
            if let Some(mob) = ctx.add_spawn(id, at.1, at.2, at.3, 0, false, 0, npc_instance)
                && let Some(hated) = most_hated
            {
                ctx.world_mut().add_damage_hate(mob.object_id, hated, 0, 1);
            }
        }
        if ctx.get_random(100) < 33 {
            if let Some(id) = ctx.get_random_entry(&TWIN_MOBIDS) {
                let at = TWIN_SPAWNS[1];
                let most_hated = ctx.most_hated(npc);
                if let Some(mob) = ctx.add_spawn(id, at.1, at.2, at.3, 0, false, 0, npc_instance)
                    && let Some(hated) = most_hated
                {
                    ctx.world_mut().add_damage_hate(mob.object_id, hated, 0, 1);
                }
            }
        }

        ctx.start_quest_timer("spawnBossGuards", BOSS_MINION_SPAWN_TIME, Some(npc), None, false);
    } else if event.eq_ignore_ascii_case("isTwinSeparated") {
        let (Some(klanikus), Some(klodekus)) = (klanikus, klodekus) else {
            return None;
        };
        let together = ctx.check_if_in_range(500, klanikus.object_id, klodekus.object_id, false);
        ctx.world_mut().set_invul(klanikus.object_id, !together);
        ctx.world_mut().set_invul(klodekus.object_id, !together);

        ctx.start_quest_timer("isTwinSeparated", 10000, Some(npc), None, false);
    } else if event.eq_ignore_ascii_case("ressurectTwin") {
        let skill = SkillKey::new(5824, 1);
        let alive_twin = if klanikus == Some(npc) { klodekus } else { klanikus };
        let Some(alive_twin) = alive_twin else {
            return None;
        };
        ctx.world_mut().do_revive(npc.object_id);
        ctx.world_mut().do_cast(npc.object_id, skill);
        let alive_hp = ctx.world().creature_hp(alive_twin.object_id).map_or(0.0, |(hp, _)| hp);
        ctx.world_mut().set_current_hp(npc.object_id, alive_hp, true);

        // get most hated of other boss
        if let Some(hated) = ctx.most_hated(alive_twin) {
            ctx.world_mut()
                .npc_notify_aggression(npc.object_id, hated, 1000);
        }

        ctx.world_mut().set_invul(alive_twin.object_id, true); // make other boss invul
        ctx.start_quest_timer("uninvul", BOSS_INVUL_TIME, Some(alive_twin), None, false);
    } else if event == "uninvul" {
        ctx.world_mut().set_invul(npc.object_id, false);
    }

    None
}

pub(crate) fn on_attack(data: &HallData, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, attacker: ObjectId, damage: i32) {
    let Some(instance_id) = world_of(ctx, npc.object_id) else {
        return;
    };
    if !hs(ctx, instance_id).is_bosses_attacked {
        hs(ctx, instance_id).is_bosses_attacked = true;
        // set instance reenter time for all allowed players
        common::restrict_entry(ctx, instance_id, data.instance_id, INSTANCEPENALTY, false);

        ctx.start_quest_timer("spawnBossGuards", BOSS_MINION_SPAWN_TIME, Some(npc), None, false);
        ctx.start_quest_timer("isTwinSeparated", 10000, Some(npc), None, false);
    } else if f64::from(damage) >= ctx.world().creature_hp(npc.object_id).map_or(0.0, |(hp, _)| hp) {
        let (klanikus, klodekus) = {
            let world = hs(ctx, instance_id);
            (world.klanikus, world.klodekus)
        };
        let (Some(klanikus), Some(klodekus)) = (klanikus, klodekus) else {
            return;
        };
        if ctx.is_dead(klanikus.object_id) {
            ctx.world_mut().set_dead(klanikus.object_id, false);
            ctx.world_mut().do_die(klanikus.object_id, Some(attacker));
            ctx.world_mut().do_die(klodekus.object_id, Some(attacker));
        } else if ctx.is_dead(klodekus.object_id) {
            ctx.world_mut().set_dead(klodekus.object_id, false);
            ctx.world_mut().do_die(klodekus.object_id, Some(attacker));
            ctx.world_mut().do_die(klanikus.object_id, Some(attacker));
        } else {
            boss_simple_die(ctx, npc);
            ctx.start_quest_timer("ressurectTwin", BOSS_RESSURECT_TIME, Some(npc), None, false);
        }
    }
}

pub(crate) fn on_kill(data: &HallData, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let Some(instance_id) = world_of(ctx, npc.object_id) else {
        return;
    };

    if npc.id == TUMOR_ALIVE {
        let at = common::location_of(ctx, npc.object_id);
        let npc_instance = ctx.world().instance_id_of(npc.object_id);
        ctx.delete_npc(npc);
        if let Some(at) = at {
            common::spawn(ctx, TUMOR_DEAD, at, npc_instance);
        }
    }

    let status = common::status(ctx, instance_id);
    if status < 5 {
        if check_kill_progress(ctx, npc.object_id, instance_id) {
            run_tumors(data, ctx, instance_id);
        }
    } else if status == 5 {
        if check_kill_progress(ctx, npc.object_id, instance_id) {
            run_twins(ctx, instance_id);
        }
    } else if status == 6 && (npc.id == KLODEKUS || npc.id == KLANIKUS) {
        let (klanikus, klodekus) = {
            let world = hs(ctx, instance_id);
            (world.klanikus, world.klodekus)
        };
        let both_dead = klanikus.is_some_and(|k| ctx.is_dead(k.object_id))
            && klodekus.is_some_and(|k| ctx.is_dead(k.object_id));
        if !both_dead {
            return;
        }
        common::inc_status(ctx, instance_id);
        let now = ctx.now_ms();
        let finish_diff = {
            let world = hs(ctx, instance_id);
            world.store_time[1] = now;
            world.store_time[1] - world.store_time[0]
        };
        let tag = match finish_diff {
            d if d < 1260000 => 13777,
            d if d < 1380000 => 13778,
            d if d < 1500000 => 13779,
            d if d < 1620000 => 13780,
            d if d < 1740000 => 13781,
            d if d < 1860000 => 13782,
            d if d < 1980000 => 13783,
            d if d < 2100000 => 13784,
            d if d < 2220000 => 13785,
            _ => 13786,
        };
        common::set_parameter(ctx, instance_id, "tag", &tag.to_string());

        {
            let world = hs(ctx, instance_id);
            world.klanikus = None;
            world.klodekus = None;
        }
        // NOTE(java-quirk): `cancelQuestTimers(name)` stops the timers of every instance.
        ctx.cancel_quest_timers("ressurectTwin");
        ctx.cancel_quest_timers("spawnBossGuards");
        ctx.cancel_quest_timers("isTwinSeparated");
        ctx.add_spawn(
            TEPIOS,
            TEPIOS_SPAWN.0,
            TEPIOS_SPAWN.1,
            TEPIOS_SPAWN.2,
            0,
            false,
            0,
            instance_id,
        );

        for killer in common::allowed(ctx, instance_id) {
            ctx.world().send(
                killer,
                &ExSendUiEvent {
                    object_id: killer.0,
                    hide: true,
                    count_up: true,
                    start_time: 0,
                    end_time: 0,
                    npc_string_id: -1,
                    params: vec![String::new()],
                },
            );
        }

        common::expire_in_five_minutes(ctx, instance_id);
    }
}

pub(crate) fn on_talk(data: &HallData, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
    if npc.id == MOUTHOFEKIMUS {
        enter_instance(data, ctx, player);
        return None;
    }

    None
}

/// Writes the `impl Script` of a Hall of Suffering script over its [`HallData`].
macro_rules! hall_of_suffering {
    ($ty:ident, $data:expr) => {
        impl l2_world::Script for $ty {
            fn name(&self) -> &'static str {
                $data.name
            }

            fn path(&self) -> &'static str {
                $data.path
            }

            fn setup(&self, s: &mut l2_world::ScriptSetup) {
                $crate::ai::areas::gracia::instances::hall_of_suffering::setup(s);
            }

            fn on_skill_see(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                caster: l2_util::ObjectId,
                skill: l2_world::SkillKey,
                _targets: &[l2_util::ObjectId],
                _is_summon: bool,
            ) {
                $crate::ai::areas::gracia::instances::hall_of_suffering::on_skill_see(ctx, npc, caster, skill);
            }

            fn on_event(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                event: &str,
                npc: Option<l2_world::ScriptNpc>,
                _player: Option<l2_util::ObjectId>,
            ) -> Option<String> {
                $crate::ai::areas::gracia::instances::hall_of_suffering::on_event(ctx, event, npc)
            }

            fn on_attack(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                attacker: l2_util::ObjectId,
                damage: i32,
                _is_summon: bool,
                _skill: Option<l2_world::SkillKey>,
            ) {
                $crate::ai::areas::gracia::instances::hall_of_suffering::on_attack(&$data, ctx, npc, attacker, damage);
            }

            fn on_kill(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                _killer: l2_util::ObjectId,
                _is_summon: bool,
            ) {
                $crate::ai::areas::gracia::instances::hall_of_suffering::on_kill(&$data, ctx, npc);
            }

            fn on_talk(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                player: l2_util::ObjectId,
            ) -> Option<String> {
                $crate::ai::areas::gracia::instances::hall_of_suffering::on_talk(&$data, ctx, npc, player)
            }
        }
    };
}

pub(crate) use hall_of_suffering;
