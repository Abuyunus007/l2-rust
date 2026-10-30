//! Quest 727: Hope Within the Darkness (the castle wardens; the party of the castle clan enters
//! the Pailaka of the Rune castle, saves the four victims from three waves of bosses and gets
//! 159 Knight's Epaulettes).
// Java: dist/game/data/scripts/quests/Q00727_HopeWithinTheDarkness/Q00727_HopeWithinTheDarkness.java
//
// Pattern: a quest with its own instance (`CAUWorld`). The Java `Runnable`s on the thread pool
// are timers with the instance id in their parameters; `underAttack` is the world parameter
// `underAttack`, the status is the world status, and the world carries the parameter `script`
// (Java's `instanceof CAUWorld`). The victims buff the players near them and attack the foes in
// their aggro range every five seconds.
//
// FOLLOW-UP(engine): `CastleDungeon.getReEnterTime()` decides on the `CastlePailaka <warden>`
// row (D-54): the in-memory copy is read here, the page cannot wait for an asynchronous re-read.

use l2_protocol_game::server::ChatType;
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::script::{STARTED, TimerParams};
use l2_world::{Intention, Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::instances::common::{
    add_allowed, create_world, expire_in_five_minutes, set_parameter, set_status,
};

// NPCs
const NPC_KNIGHT: i32 = 36562;
const NPC_RANGER: i32 = 36563;
const NPC_MAGE: i32 = 36564;
const NPC_WARRIOR: i32 = 36565;

const BOSSES: [i32; 3] = [25653, 25654, 25655];
const MONSTERS: [i32; 3] = [25656, 25657, 25658];

// Reward
const KNIGHT_EPALUETTE: i32 = 9912;

// Skills
const RAID_CURSE: SkillKey = SkillKey { id: 5456, level: 1 };

// Misc
const MIN_LEVEL: i32 = 80;
const REENTER_INTERVAL: i64 = 14_400_000;
const INITIAL_SPAWN_DELAY: i64 = 120_000;
const WAVE_SPAWN_DELAY: i64 = 480_000;
const PRIVATE_SPAWN_DELAY: i64 = 180_000;
const PLAYER_SPAWN_POINT: (i32, i32, i32) = (48163, -12195, -9140);

/// Java `CASTLE_DUNGEONS`: the warden and the instance template.
const CASTLE_DUNGEONS: [(i32, i32); 9] = [
    (36403, 80),
    (36404, 81),
    (36405, 82),
    (36406, 83),
    (36407, 84),
    (36408, 85),
    (36409, 86),
    (36410, 87),
    (36411, 88),
];

const WORLD_TYPE: &str = "Q00727";

/// Java `Q00727_HopeWithinTheDarkness`.
pub struct Q00727HopeWithinTheDarkness;

fn dungeon_of(npc_id: i32) -> Option<i32> {
    CASTLE_DUNGEONS
        .iter()
        .find(|(warden, _)| *warden == npc_id)
        .map(|(_, template)| *template)
}

fn is_victim(npc_id: i32) -> bool {
    (NPC_KNIGHT..=NPC_WARRIOR).contains(&npc_id)
}

/// Java `NPC_BUFFS.get(npcId)`.
fn victim_buff(npc_id: i32) -> Option<SkillKey> {
    match npc_id {
        NPC_KNIGHT => Some(SkillKey { id: 5970, level: 1 }),
        NPC_RANGER => Some(SkillKey { id: 5971, level: 1 }),
        NPC_MAGE => Some(SkillKey { id: 5972, level: 1 }),
        NPC_WARRIOR => Some(SkillKey { id: 5973, level: 1 }),
        _ => None,
    }
}

/// The global row of `CastleDungeon.getReEnterTime()`.
fn reenter_row(warden: i32) -> String {
    format!("CastlePailaka {warden}")
}

/// `CastleDungeon.getReEnterTime()`: a missing row is written as 0.
fn reenter_time(ctx: &mut ScriptCtx<'_>, warden: i32) -> i64 {
    let row = reenter_row(warden);
    if ctx.global_variables().has_variable(&row) {
        ctx.global_variables().get_long(&row, 0)
    } else {
        ctx.set_global_variable(&row, "0");
        0
    }
}

/// The world a player is in when it is one of this script.
fn world_of_player(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<i32> {
    ctx.world()
        .instance_world_of(player)
        .filter(|w| w.parameters.get("script").map(String::as_str) == Some(WORLD_TYPE))
        .map(|w| w.instance_id)
}

/// The world an NPC is in when it is one of this script.
fn world_of_npc(ctx: &ScriptCtx<'_>, npc: ObjectId) -> Option<i32> {
    world_of_player(ctx, npc)
}

/// `inst.getNpcs()` with their ids.
fn instance_npcs(ctx: &ScriptCtx<'_>, instance: i32) -> Vec<(ObjectId, i32)> {
    ctx.world().instance(instance).map_or_else(Vec::new, |i| {
        i.npcs
            .keys()
            .filter_map(|id| ctx.world().npc(*id).map(|n| (*id, n.npc_id)))
            .collect()
    })
}

fn parameter_is(ctx: &ScriptCtx<'_>, instance: i32, name: &str, value: &str) -> bool {
    ctx.world()
        .instance_world(instance)
        .is_some_and(|w| w.parameters.get(name).map(String::as_str) == Some(value))
}

/// Java `checkEnterConditions(player, npc)`.
fn check_enter_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc) -> Option<String> {
    let castle = ctx.world().npc_castle(npc.object_id);
    let dungeon = dungeon_of(npc.id);
    let (Some(castle), Some(_)) = (castle, dungeon) else {
        return Some("CastleWarden-03.html".to_owned());
    };

    // Check if castle have contract with fortress
    if !ctx.world().forts().iter().any(|fort| fort.castle_id == castle.0) {
        return Some("CastleWarden-13a.html".to_owned());
    }

    // Check if player has quest
    let started = ctx
        .get_quest_state(player, false)
        .filter(|qs| !qs.is_created(ctx));
    let clan_castle = |ctx: &ScriptCtx<'_>, p: ObjectId| ctx.world().clan_of(p).map(|c| c.castle_id);
    if started.is_none() {
        // Check if player is from clan, that owns castle
        if clan_castle(ctx, player) != Some(castle.0) {
            return Some("CastleWarden-08.html".to_owned());
        }
        if ctx.player_level(player) >= MIN_LEVEL {
            return Some("CastleWarden-06.htm".to_owned());
        }
        return Some("CastleWarden-07.html".to_owned());
    }

    let Some(party) = ctx.world().party_of(player) else {
        return Some("CastleWarden-09.html".to_owned());
    };
    let leader = ctx.world().party_leader(party);
    if leader != Some(player) {
        let name = leader.map(|l| ctx.player_name(l)).unwrap_or_default();
        return ctx
            .get_htm(player, "CastleWarden-10.html")
            .map(|page| page.replace("%leader%", &name));
    }
    for member in ctx.world().party_members(party) {
        let member_started = ctx
            .get_quest_state(member, false)
            .filter(|qs| !qs.is_created(ctx));
        let name = ctx.player_name(member);
        // Check if each party member has quest
        if member_started.is_none() {
            return ctx
                .get_htm(player, "CastleWarden-12.html")
                .map(|page| page.replace("%player%", &name));
        }
        // NOTE(java-quirk): Java asks the leader's clan, not the member's.
        if clan_castle(ctx, player) != Some(castle.0) {
            return ctx
                .get_htm(player, "CastleWarden-11.html")
                .map(|page| page.replace("%player%", &name));
        }
        // Check if each party member not very far from leader
        if !ctx.check_if_in_range(1000, player, member, true) {
            return ctx
                .get_htm(player, "CastleWarden-17.html")
                .map(|page| page.replace("%player%", &name));
        }
    }

    if reenter_time(ctx, npc.id) > ctx.now_ms() {
        return Some("CastleWarden-18.html".to_owned());
    }
    None
}

/// Java `enterInstance(player, template, coords, dungeon, ret)`.
fn enter_instance(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    npc: ScriptNpc,
    ret: Option<String>,
) -> Option<String> {
    let (x, y, z) = PLAYER_SPAWN_POINT;
    // Check for existing instances for this player
    if let Some((instance, kind)) = ctx
        .world()
        .player_instance_world(player)
        .map(|w| (w.instance_id, w.parameters.get("script").cloned()))
    {
        if kind.as_deref() != Some(WORLD_TYPE) {
            ctx.send_system_message(
                player,
                SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
            );
            return Some(String::new());
        }
        ctx.teleport_player(player, x, y, z, instance, true);
        return Some(String::new());
    }

    // New instance
    if ret.is_some() {
        return ret;
    }

    let template = dungeon_of(npc.id)?;
    let party = ctx.world().party_of(player);
    let instance = create_world(ctx, template);
    set_parameter(ctx, instance, "script", WORLD_TYPE);
    set_parameter(ctx, instance, "underAttack", "false");
    if let Some(at) = ctx.world().position_of(player)
        && let Some(i) = ctx.world_mut().instance_mut(instance)
    {
        i.exit_location = Some(at);
    }
    let reenter = ctx.now_ms() + REENTER_INTERVAL;
    ctx.set_global_variable(&reenter_row(npc.id), &reenter.to_string());
    ctx.add_timer(
        "SpawnNpcs",
        Some(TimerParams::new().set("instance", instance)),
        INITIAL_SPAWN_DELAY,
        Some(npc),
        Some(player),
    );

    // Teleport players
    let Some(party) = party else {
        return Some("CastleWarden-09.html".to_owned());
    };
    for member in ctx.world().party_members(party) {
        ctx.teleport_player(member, x, y, z, instance, true);
        add_allowed(ctx, instance, member);
        if let Some(qs) = ctx.get_quest_state(member, true) {
            qs.set_cond(ctx, 2, false);
        }
    }

    let clan = ctx
        .world()
        .clan_of(player)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    ctx.get_htm(player, "CastleWarden-13.html")
        .map(|page| page.replace("%clan%", &clan))
}

/// Java `rewardPlayer(player)`.
fn reward_player(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    if let Some(qs) = ctx.get_quest_state(player, false)
        && qs.is_cond(ctx, 2)
    {
        qs.set_cond(ctx, 3, false);
    }
}

impl Script for Q00727HopeWithinTheDarkness {
    fn name(&self) -> &'static str {
        "Q00727_HopeWithinTheDarkness"
    }

    fn path(&self) -> &'static str {
        "quests/Q00727_HopeWithinTheDarkness"
    }

    fn id(&self) -> i32 {
        727
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let wardens = CASTLE_DUNGEONS.map(|(warden, _)| warden);
        s.add_start_npc(&wardens);
        s.add_talk_id(&wardens);

        let victims = [NPC_KNIGHT, NPC_RANGER, NPC_MAGE, NPC_WARRIOR];
        s.add_spawn_id(&victims);
        s.add_kill_id(&victims);
        s.add_attack_id(&victims);
        s.add_talk_id(&victims);
        s.add_first_talk_id(&victims);

        s.add_spawn_id(&BOSSES);
        s.add_kill_id(&BOSSES);
        s.add_attack_id(&BOSSES);

        s.add_kill_id(&MONSTERS);
        s.add_attack_id(&MONSTERS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("enter") {
            let (player, npc) = (player?, npc?);
            let ret = check_enter_conditions(ctx, player, npc);
            return enter_instance(ctx, player, npc, ret);
        } else if event.eq_ignore_ascii_case("suicide") {
            let npc = npc?;
            let instance = world_of_npc(ctx, npc.object_id)?;
            set_status(ctx, instance, 5);
            for (victim, id) in instance_npcs(ctx, instance) {
                if is_victim(id) {
                    let victim = ScriptNpc {
                        object_id: victim,
                        id,
                    };
                    ctx.cancel_quest_timer("check_for_foes", Some(victim), None);
                    ctx.cancel_quest_timer("buff", Some(victim), None);
                    if !ctx.is_dead(victim.object_id) {
                        ctx.world_mut().do_die(victim.object_id, None);
                    }
                }
            }
            // Destroy instance after 5 minutes
            expire_in_five_minutes(ctx, instance);
            return None;
        } else if event.eq_ignore_ascii_case("buff") {
            let npc = npc?;
            if let Some(skill) = victim_buff(npc.id) {
                for pl in ctx.world().visible_players(npc.object_id) {
                    if ctx.check_if_in_range(75, npc.object_id, pl, false) {
                        ctx.cast_skill_at(npc, Some(pl), skill);
                    }
                }
            }
            ctx.start_quest_timer("buff", 120_000, Some(npc), None, false);
            return None;
        } else if event.eq_ignore_ascii_case("check_for_foes") {
            let npc = npc?;
            if ctx.world().intention_of(npc.object_id) != Intention::Attack {
                let range = ctx.world().aggro_range_of(npc.object_id);
                for foe in ctx.world().visible_objects_in_range(npc.object_id, range) {
                    // `foe.isAttackable() && !(foe instanceof QuestGuard)`
                    let quest_guard = ctx.world().npc(foe).is_some_and(|n| is_victim(n.npc_id));
                    if ctx.world().is_attackable_npc(foe) && !quest_guard {
                        ctx.add_attack_desire(npc, foe, 999);
                    }
                }
            }
            ctx.start_quest_timer("check_for_foes", 5000, Some(npc), None, false);
            return None;
        }

        let player = player?;
        let qs = match ctx.get_quest_state(player, false) {
            Some(qs) => qs,
            None => ctx.get_quest_state(player, true)?,
        };
        let cond = qs.get_cond(ctx);
        if event.eq_ignore_ascii_case("CastleWarden-05.htm") {
            if cond == 0 {
                qs.start_quest(ctx);
            }
        } else if event.eq_ignore_ascii_case("CastleWarden-15.htm") {
            qs.exit_quest(ctx, true, true);
        }
        Some(event.to_owned())
    }

    fn on_timer_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        let Some(params) = params else {
            return;
        };
        let instance = params.get_int("instance", 0);
        // The instance is gone: Java's spawn throws and logs.
        if ctx.world().instance_world(instance).is_none() {
            return;
        }
        let status = ctx.world().instance_world(instance).map_or(0, |w| w.status());
        let next = |ctx: &mut ScriptCtx<'_>, event: &str, ms: i64| {
            ctx.add_timer(
                event,
                Some(TimerParams::new().set("instance", instance)),
                ms,
                npc,
                player,
            );
        };
        match event {
            // `spawnNpcs.run()`
            "SpawnNpcs" => {
                let group = match status {
                    0 => {
                        ctx.world_mut().spawn_instance_group(instance, "victims");
                        "bosses_1"
                    }
                    1 => "bosses_2",
                    2 => "bosses_3",
                    _ => return,
                };
                ctx.world_mut().spawn_instance_group(instance, group);
                if status < 2 {
                    next(ctx, "SpawnNpcs", WAVE_SPAWN_DELAY);
                }
                next(ctx, "SpawnPrivates", PRIVATE_SPAWN_DELAY);
            }
            // `spawnPrivates.run()`
            "SpawnPrivates" => {
                match status {
                    0 => {
                        ctx.world_mut()
                            .spawn_instance_group(instance, "monsters_first_wave");
                        set_parameter(ctx, instance, "underAttack", "true");
                    }
                    1 => {
                        ctx.world_mut()
                            .spawn_instance_group(instance, "monsters_second_wave");
                    }
                    2 => {
                        ctx.world_mut()
                            .spawn_instance_group(instance, "monsters_third_wave");
                    }
                    _ => {}
                }
                set_status(ctx, instance, status + 1);
            }
            // `completeDungeon.run()`
            "CompleteDungeon" => {
                if status != 4 {
                    return;
                }
                set_parameter(ctx, instance, "underAttack", "false");
                for (victim, id) in instance_npcs(ctx, instance) {
                    if is_victim(id) {
                        let victim = ScriptNpc {
                            object_id: victim,
                            id,
                        };
                        ctx.cancel_quest_timer("check_for_foes", Some(victim), None);
                        ctx.cancel_quest_timer("buff", Some(victim), None);
                        if id == NPC_KNIGHT {
                            ctx.broadcast_npc_say(
                                victim,
                                ChatType::Shout,
                                NpcStringId::YouVeDoneItWeBelievedInYouWarriorWeWantToShowOurSincerityThoughItIsSmallPleaseGiveMeSomeOfYourTime,
                                &[],
                            );
                        }
                    }
                }
                let Some(killer) = player else {
                    return;
                };
                match ctx.world().party_of(killer) {
                    None => reward_player(ctx, killer),
                    Some(party) => {
                        let killer_instance = ctx.world().instance_id_of(killer);
                        for member in ctx.world().party_members(party) {
                            if ctx.world().instance_id_of(member) == killer_instance {
                                reward_player(ctx, member);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let instance = world_of_player(ctx, player)?;
        if parameter_is(ctx, instance, "underAttack", "true") {
            Some("Victim-02.html".to_owned())
        } else if ctx.world().instance_world(instance).map_or(0, |w| w.status()) == 4 {
            Some("Victim-03.html".to_owned())
        } else {
            Some("Victim-01.html".to_owned())
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;
        if is_victim(npc.id)
            && let Some(instance) = world_of_player(ctx, player)
        {
            if let Some(world) = ctx.world_mut().instance_world_mut(instance) {
                world.remove_allowed(player);
            }
            if let Some((x, y, z)) = ctx.world().instance(instance).and_then(|i| i.exit_location) {
                ctx.teleport_player(player, x, y, z, 0, true);
            }
            return None;
        }

        let castle = ctx.world().npc_castle(npc.object_id);
        let clan_castle = ctx.world().clan_of(player).map(|c| c.castle_id);
        if clan_castle.is_none() || castle.is_none() || clan_castle != castle.map(|c| c.0) {
            return Some("CastleWarden-03.html".to_owned());
        }

        let mut cond = 0;
        if qs.is_created(ctx) {
            qs.set_cond(ctx, 0, false);
        } else {
            cond = qs.get_cond(ctx);
        }

        let dungeon = dungeon_of(npc.id).is_some();
        if dungeon && cond == 0 {
            if ctx.player_level(player) >= 80 {
                htmltext = "CastleWarden-01.htm".to_owned();
            } else {
                htmltext = "CastleWarden-04.html".to_owned();
                qs.exit_quest(ctx, true, false);
            }
        } else if dungeon && cond > 0 && qs.state(ctx) == STARTED {
            if cond == 1 {
                htmltext = "CastleWarden-15.htm".to_owned();
            } else if cond == 3 {
                ctx.reward_items(player, KNIGHT_EPALUETTE, 159);
                qs.exit_quest(ctx, true, true);
                htmltext = "CastleWarden-16.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if is_victim(npc.id) {
            let hp = ctx.npc_current_hp(npc);
            let max = f64::from(ctx.npc_max_hp(npc));
            if hp <= max * 0.1 {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::UghItHurtsDownToTheBones,
                    &[],
                );
            } else if hp <= max * 0.4 {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::YourMindIsGoingBlank, &[]);
            }
            return;
        }

        let character = if is_summon {
            ctx.summon_of(attacker)
        } else {
            Some(attacker)
        };
        let Some(character) = character else {
            return;
        };
        let level_of = |ctx: &ScriptCtx<'_>, id: ObjectId| ctx.world().stats(id).map_or(0, |s| s.level());
        if level_of(ctx, character) - level_of(ctx, npc.object_id) >= 9 {
            let buffed = |ctx: &ScriptCtx<'_>, id: ObjectId| {
                ctx.world()
                    .effect_list(id)
                    .is_some_and(|l| l.buff_count() > 0 || l.dance_count() > 0)
            };
            if buffed(ctx, character) {
                ctx.world_mut().set_npc_target(npc.object_id, Some(character));
                ctx.world_mut().do_simultaneous_cast(npc.object_id, RAID_CURSE);
            } else if let Some(party) = ctx.world().party_of(attacker) {
                for member in ctx.world().party_members(party) {
                    if buffed(ctx, member) {
                        ctx.world_mut().set_npc_target(npc.object_id, Some(member));
                        ctx.world_mut().do_simultaneous_cast(npc.object_id, RAID_CURSE);
                    }
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        if is_victim(npc.id) {
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                match npc.id {
                    NPC_KNIGHT => NpcStringId::ICanTStandItAnymoreAah,
                    NPC_RANGER => NpcStringId::Kyaaak,
                    _ => NpcStringId::GaspHowCanThisBe,
                },
                &[],
            );
            // All other friendly NPCs do suicide - start timer
            ctx.start_quest_timer("suicide", 1500, Some(npc), None, false);
            ctx.cancel_quest_timer("check_for_foes", Some(npc), None);
            ctx.cancel_quest_timer("buff", Some(npc), None);
        }

        let Some(instance) = world_of_npc(ctx, npc.object_id) else {
            return;
        };
        let boss = BOSSES.contains(&npc.id);
        if boss {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::HowDareYou, &[]);
        }
        let status = ctx.world().instance_world(instance).map_or(0, |w| w.status());
        if status == 3 && (boss || MONSTERS.contains(&npc.id)) {
            let mut all_dead = true;
            for (other, id) in instance_npcs(ctx, instance) {
                if !ctx.is_dead(other) && (BOSSES.contains(&id) || MONSTERS.contains(&id)) {
                    all_dead = false;
                    break;
                }
            }
            if all_dead {
                set_status(ctx, instance, 4);
                // Destroy instance after 5 minutes
                expire_in_five_minutes(ctx, instance);
                ctx.add_timer(
                    "CompleteDungeon",
                    Some(TimerParams::new().set("instance", instance)),
                    1500,
                    Some(npc),
                    Some(player),
                );
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        // Buff players every two minutes, check for foes in aggro range
        if is_victim(npc.id) {
            ctx.start_quest_timer("buff", 120_000, Some(npc), None, false);
            ctx.start_quest_timer("check_for_foes", 120_000, Some(npc), None, false);
            if npc.id == NPC_KNIGHT {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::WarriorsHaveYouComeToHelpThoseWhoAreImprisonedHere,
                    &[],
                );
            }
        } else if let Some(index) = BOSSES.iter().position(|b| *b == npc.id) {
            let text = [
                NpcStringId::ILlRipTheFleshFromYourBones,
                NpcStringId::YouLlFlounderInDelusionForTheRestOfYourLife,
                NpcStringId::ThereIsNoEscapeFromThisPlace,
            ][index];
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00727_HopeWithinTheDarkness";
    const DIR: &str = "data/scripts/quests/Q00727_HopeWithinTheDarkness";
    const WARDEN: i32 = 36403;

    const NPCS: &str = r#"
        <npc id="36403" type="Folk" level="1" name="Castle Warden"/>
        <npc id="36562" type="QuestGuard" level="60" name="Victim Knight">
            <stats><vitals hp="1000" hpRegen="0" mp="100" mpRegen="0" /></stats></npc>
        <npc id="36563" type="QuestGuard" level="60" name="Victim Ranger"/>
        <npc id="36564" type="QuestGuard" level="60" name="Victim Mage"/>
        <npc id="36565" type="QuestGuard" level="60" name="Victim Warrior"/>
        <npc id="25653" type="Monster" level="60" name="Boss 1"/>
        <npc id="25654" type="Monster" level="60" name="Boss 2"/>
        <npc id="25655" type="Monster" level="60" name="Boss 3"/>
        <npc id="25656" type="Monster" level="60" name="Monster 1"/>
        <npc id="25657" type="Monster" level="60" name="Monster 2"/>
        <npc id="25658" type="Monster" level="60" name="Monster 3"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9912" type="EtcItem" name="Knight's Epaulette"><set name="is_stackable" val="true"/></item>"#;

    /// `CastlePailaka.xml` with a few of the spawns of every group.
    const TEMPLATE: &str = r#"
        <exitPoint x="10" y="20" z="30" />
        <spawnlist>
            <group name="victims">
                <npc id="36562" x="49093" y="-12077" z="-9395" heading="0" />
                <npc id="36563" x="49094" y="-12238" z="-9386" heading="0" />
                <npc id="36564" x="49093" y="-12401" z="-9388" heading="0" />
                <npc id="36565" x="49232" y="-12239" z="-9386" heading="0" />
            </group>
            <group name="bosses_1"><npc id="25653" x="50943" y="-12224" z="-9321" heading="32768" /></group>
            <group name="bosses_2"><npc id="25654" x="50943" y="-12224" z="-9321" heading="32768" /></group>
            <group name="bosses_3">
                <npc id="25655" x="50943" y="-12004" z="-9321" heading="32768" />
                <npc id="25655" x="50943" y="-12475" z="-9321" heading="32768" />
            </group>
            <group name="monsters_first_wave">
                <npc id="25656" x="50343" y="-12552" z="-9388" heading="32768" />
                <npc id="25656" x="50344" y="-12340" z="-9380" heading="32768" />
            </group>
            <group name="monsters_second_wave"><npc id="25657" x="50341" y="-12134" z="-9381" heading="32768" /></group>
            <group name="monsters_third_wave"><npc id="25658" x="50342" y="-11917" z="-9389" heading="32768" /></group>
        </spawnlist>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00727HopeWithinTheDarkness);
        let pages = Pages::of(&Q00727HopeWithinTheDarkness)
            .plain(&["CastleWarden-05.htm", "CastleWarden-15.htm"])
            .file(
                &format!("{DIR}/CastleWarden-13.html"),
                "<html><body>The clan %clan% enters.</body></html>",
            )
            .npcs(&[WARDEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        t.world
            .set_instance_data(instance_data(&[(80, "Castle Pailaka")], &[(80, TEMPLATE)]));
        let warden = t.spawn(WARDEN, 20);
        (t, warden)
    }

    fn warden_npc(t: &ScriptTest, warden: Oid) -> ScriptNpc {
        ScriptNpc {
            object_id: warden,
            id: t.world.npc(warden).map_or(0, |n| n.npc_id),
        }
    }

    /// Two players of a party with the quest started at cond 1.
    fn party_with_quest(t: &mut ScriptTest) {
        t.enter(1, 82, vec![]);
        t.enter(2, 82, vec![]);
        t.party(1, 2);
        t.world.run_as_script(NAME, |ctx| {
            for p in [1, 2] {
                let qs = ctx.get_quest_state(Oid(p), true).unwrap();
                qs.start_quest(ctx);
            }
        });
        t.take_writes();
    }

    fn enter(t: &mut ScriptTest, warden: Oid) -> Option<String> {
        let npc = warden_npc(t, warden);
        let mut page = None;
        t.world.run_as_script(NAME, |ctx| {
            page = enter_instance(ctx, Oid(1), npc, None);
        });
        page
    }

    fn alive_enemies(t: &ScriptTest) -> Vec<Oid> {
        let mut out = Vec::new();
        for id in BOSSES.iter().chain(MONSTERS.iter()) {
            out.extend(t.npcs_of(*id).into_iter().filter(|n| !t.world.is_dead(*n)));
        }
        out
    }

    #[test]
    fn the_quest_is_started_and_closed_by_the_warden_events() {
        let (mut t, _) = setup();
        t.enter(1, 82, vec![]);
        let mut page = None;
        t.world.run_as_script(NAME, |ctx| {
            page = Q00727HopeWithinTheDarkness.on_event(ctx, "CastleWarden-05.htm", None, Some(Oid(1)));
        });
        assert_eq!(page.as_deref(), Some("CastleWarden-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        t.world.run_as_script(NAME, |ctx| {
            page = Q00727HopeWithinTheDarkness.on_event(ctx, "CastleWarden-15.htm", None, Some(Oid(1)));
        });
        assert_eq!(page.as_deref(), Some("CastleWarden-15.htm"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_warden_far_from_a_castle_sends_the_player_away() {
        let (mut t, warden) = setup();
        t.enter(1, 82, vec![]);
        let npc = warden_npc(&t, warden);
        let mut page = None;
        t.world.run_as_script(NAME, |ctx| {
            page = Q00727HopeWithinTheDarkness.on_talk(ctx, npc, Oid(1));
        });
        assert_eq!(page.as_deref(), Some("CastleWarden-03.html"));
        t.world.run_as_script(NAME, |ctx| {
            page = check_enter_conditions(ctx, Oid(1), npc);
        });
        assert_eq!(page.as_deref(), Some("CastleWarden-03.html"));
    }

    #[test]
    fn the_party_enters_and_the_waves_come_in_order() {
        let (mut t, warden) = setup();
        party_with_quest(&mut t);
        let page = enter(&mut t, warden).unwrap();
        assert!(page.contains("enters"), "{page}");
        let instance = t.world.instance_id_of(Oid(1));
        assert!(instance > 0);
        assert_eq!(t.world.instance_id_of(Oid(2)), instance);
        // Cond 2 for both, the warden is closed for four hours.
        t.world.run_as_script(NAME, |ctx| {
            for p in [1, 2] {
                assert!(ctx.get_quest_state(Oid(p), false).unwrap().is_cond(ctx, 2));
            }
            assert!(reenter_time(ctx, WARDEN) > ctx.now_ms());
        });

        // Two minutes: the victims and the first boss. Three more: the first wave.
        t.wait_ms(119_000);
        assert!(t.npcs_of(NPC_KNIGHT).is_empty());
        t.wait_ms(2_000);
        assert_eq!(t.npcs_of(NPC_KNIGHT).len(), 1);
        assert_eq!(t.npcs_of(25653).len(), 1);
        assert!(t.npcs_of(25656).is_empty());
        assert_eq!(t.world.instance_world(instance).unwrap().status(), 0);
        t.wait_ms(180_000);
        assert_eq!(t.npcs_of(25656).len(), 2);
        let world = t.world.instance_world(instance).unwrap();
        assert_eq!(world.status(), 1);
        assert_eq!(
            world.parameters.get("underAttack").map(String::as_str),
            Some("true")
        );

        // Eight minutes after the first: the second boss, three minutes later its monster.
        t.wait_ms(300_000);
        assert_eq!(t.npcs_of(25654).len(), 1);
        t.wait_ms(180_000);
        assert_eq!(t.npcs_of(25657).len(), 1);
        assert_eq!(t.world.instance_world(instance).unwrap().status(), 2);
        // And the third.
        t.wait_ms(300_000);
        assert_eq!(t.npcs_of(25655).len(), 2);
        t.wait_ms(180_000);
        assert_eq!(t.npcs_of(25658).len(), 1);
        assert_eq!(t.world.instance_world(instance).unwrap().status(), 3);
    }

    #[test]
    fn killing_every_enemy_of_the_third_wave_gives_cond_3_to_the_party() {
        let (mut t, warden) = setup();
        party_with_quest(&mut t);
        enter(&mut t, warden);
        let instance = t.world.instance_id_of(Oid(1));
        // 120 + 180 s; 480 s per boss wave: the third wave is up after 1260 s.
        t.wait_ms(1_262_000);
        assert_eq!(t.world.instance_world(instance).unwrap().status(), 3);
        // Everyone in the party is in the instance when the dungeon is complete.
        let mut kills = 0;
        while let Some(enemy) = alive_enemies(&t).first().copied() {
            t.kill(1, enemy);
            kills += 1;
            assert!(kills < 40, "enemies keep coming");
        }
        assert_eq!(t.world.instance_world(instance).unwrap().status(), 4);
        t.wait_ms(2_000);
        t.world.run_as_script(NAME, |ctx| {
            for p in [1, 2] {
                assert!(ctx.get_quest_state(Oid(p), false).unwrap().is_cond(ctx, 3));
            }
        });
        // The victims stop (their timers are cancelled) and the knight shouts its thanks.
        let knight = t.npcs_of(NPC_KNIGHT)[0];
        t.world.run_as_script(NAME, |ctx| {
            let knight = ctx.script_npc(knight).unwrap();
            assert!(!ctx.has_quest_timer("buff", Some(knight), None));
        });
        let first = t.world.instance_world(instance).unwrap();
        assert_eq!(
            first.parameters.get("underAttack").map(String::as_str),
            Some("false")
        );
    }

    #[test]
    fn the_victim_pages_follow_the_state_of_the_world() {
        let (mut t, warden) = setup();
        party_with_quest(&mut t);
        enter(&mut t, warden);
        let instance = t.world.instance_id_of(Oid(1));
        t.wait_ms(3_000);
        let knight_at = |t: &mut ScriptTest| {
            let mut out = Vec::new();
            t.world.run_as_script(NAME, |ctx| {
                let npc = ScriptNpc {
                    object_id: Oid(0),
                    id: NPC_KNIGHT,
                };
                out.push(Q00727HopeWithinTheDarkness.on_first_talk(ctx, npc, Oid(1)));
            });
            out.pop().unwrap()
        };
        assert_eq!(knight_at(&mut t).as_deref(), Some("Victim-01.html"));
        set_parameter_of(&mut t, instance, "underAttack", "true");
        assert_eq!(knight_at(&mut t).as_deref(), Some("Victim-02.html"));
        set_parameter_of(&mut t, instance, "underAttack", "false");
        t.world.instance_world_mut(instance).unwrap().set_status(4);
        assert_eq!(knight_at(&mut t).as_deref(), Some("Victim-03.html"));
    }

    fn set_parameter_of(t: &mut ScriptTest, instance: i32, name: &str, value: &str) {
        t.world
            .run_as_script(NAME, |ctx| set_parameter(ctx, instance, name, value));
    }

    #[test]
    fn talking_to_a_victim_takes_the_player_out_of_the_instance() {
        let (mut t, warden) = setup();
        party_with_quest(&mut t);
        enter(&mut t, warden);
        let instance = t.world.instance_id_of(Oid(1));
        t.wait_ms(121_000);
        let knight = t.npcs_of(NPC_KNIGHT)[0];
        let npc = ScriptNpc {
            object_id: knight,
            id: NPC_KNIGHT,
        };
        let mut page = Some(String::new());
        t.world.run_as_script(NAME, |ctx| {
            page = Q00727HopeWithinTheDarkness.on_talk(ctx, npc, Oid(1));
        });
        assert_eq!(page, None);
        assert!(!t.world.instance_world(instance).unwrap().is_allowed(Oid(1)));
        t.wait_ms(3_000);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
    }

    #[test]
    fn a_dying_victim_makes_the_others_die_and_the_instance_expire() {
        let (mut t, warden) = setup();
        party_with_quest(&mut t);
        enter(&mut t, warden);
        let instance = t.world.instance_id_of(Oid(1));
        t.wait_ms(121_000);
        let victims = [NPC_KNIGHT, NPC_RANGER, NPC_MAGE, NPC_WARRIOR].map(|id| t.npcs_of(id)[0]);
        t.world.do_die(victims[1], Some(Oid(1)));
        // The kill hook runs 2.5 s later, the suicide timer 1.5 s after it.
        t.wait_ms(2_600);
        assert!(!t.world.is_dead(victims[0]));
        t.wait_ms(1_600);
        for victim in victims {
            assert!(t.world.is_dead(victim));
        }
        assert_eq!(t.world.instance_world(instance).unwrap().status(), 5);
    }

    #[test]
    fn a_wounded_victim_complains() {
        let (mut t, _) = setup();
        t.enter(1, 82, vec![]);
        // A victim next to the player (the instance's own NPCs are not visible to the test world).
        let knight = t.spawn(NPC_KNIGHT, 5);
        t.wait_ms(500);
        let npc = ScriptNpc {
            object_id: knight,
            id: NPC_KNIGHT,
        };
        t.take();
        // Healthy: silent. Below 40%: "Your mind is going blank". Below 10%: "Ugh, it hurts".
        for (hp, says) in [(1000.0, false), (300.0, true), (50.0, true)] {
            t.world.set_current_hp(knight, hp, false);
            t.world.run_as_script(NAME, |ctx| {
                Q00727HopeWithinTheDarkness.on_attack(ctx, npc, Oid(1), 10, false, None);
            });
            let names = names_to(&t.take(), 1);
            assert_eq!(names.contains(&"NPC_SAY".to_owned()), says, "{hp}: {names:?}");
        }
    }
}
