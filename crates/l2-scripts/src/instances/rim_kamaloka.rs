//! Rim Kamaloka: the solo Kamaloka of the Rim Kamaloka manager (32484), eleven of them, where
//! Kanabions, Dopplers and Voiders are fought for twenty minutes and graded.
// Java: dist/game/data/scripts/instances/RimKamaloka/RimKamaloka.java
//
// Java keeps the progress of an instance in a `RimKamaWorld extends InstanceWorld`, and runs
// three `ThreadPool` tasks per instance (`FinishTask`, `DespawnTask`, `LockTask`). Here the
// progress is the script's state by instance id ([`RimKamaWorld`]) and the tasks are quest timers
// on the wheel whose event names carry the instance id (`RIM_FINISH <id>`, ...): no player or NPC
// can send them, the timers have neither.

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{InstanceWorld, Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::kamaloka_reset_time;

// NPCs.
const START_NPC: i32 = 32484;
const REWARDER: i32 = 32485;

// Reset time for all Kamaloka Default: 6:30AM on server time: `kamaloka_reset_time`.

const LOCK_TIME: i64 = 10;

// Duration of the instance, minutes.
const DURATION: i64 = 20;

// Time after which instance without players will be destroyed Default: 5 minutes.
const EMPTY_DESTROY_TIME: i32 = 5;

// Time to destroy instance (and eject player away) Default: 10 minutes.
const EXIT_TIME: i32 = 10;

// Maximum level difference between players level and Kamaloka level Default: 5
const MAX_LEVEL_DIFFERENCE: i32 = 5;
const RESPAWN_DELAY: i32 = 30;
const DESPAWN_DELAY: i64 = 10000;

// Hardcoded instance ids for Kamaloka.
const INSTANCE_IDS: [i32; 11] = [46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56];

// Level of the Kamaloka.
const LEVEL: [i32; 11] = [25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75];

// Teleport points into instances x, y, z.
const TELEPORTS: [(i32, i32, i32); 11] = [
    (10025, -219868, -8021),
    (15617, -219883, -8021),
    (22742, -220079, -7802),
    (8559, -212987, -7802),
    (15867, -212994, -7802),
    (23038, -213052, -8007),
    (9139, -205132, -8007),
    (15943, -205740, -8008),
    (22343, -206237, -7991),
    (41496, -219694, -8759),
    (48137, -219716, -8759),
];

const KANABIONS: [[i32; 3]; 11] = [
    [22452, 22453, 22454],
    [22455, 22456, 22457],
    [22458, 22459, 22460],
    [22461, 22462, 22463],
    [22464, 22465, 22466],
    [22467, 22468, 22469],
    [22470, 22471, 22472],
    [22473, 22474, 22475],
    [22476, 22477, 22478],
    [22479, 22480, 22481],
    [22482, 22483, 22484],
];

const SPAWNLIST: [&[(i32, i32, i32)]; 11] = [
    &[(8971, -219546, -8021), (9318, -219644, -8021), (9266, -220208, -8021), (9497, -220054, -8024)],
    &[(16107, -219574, -8021), (16769, -219885, -8021), (16363, -220219, -8021), (16610, -219523, -8021)],
    &[(23019, -219730, -7803), (23351, -220455, -7803), (23900, -219864, -7803), (23851, -220294, -7803)],
    &[(9514, -212478, -7803), (9236, -213348, -7803), (8868, -212683, -7803), (9719, -213042, -7803)],
    &[(16925, -212811, -7803), (16885, -213199, -7802), (16487, -213339, -7803), (16337, -212529, -7803)],
    &[(23958, -213282, -8009), (23292, -212782, -8012), (23844, -212781, -8009), (23533, -213301, -8009)],
    &[(8828, -205518, -8009), (8895, -205989, -8009), (9398, -205967, -8009), (9393, -205409, -8009)],
    &[(16185, -205472, -8009), (16808, -205929, -8009), (16324, -206042, -8009), (16782, -205454, -8009)],
    &[(23476, -206310, -7991), (23230, -205861, -7991), (22644, -205888, -7994), (23078, -206714, -7991)],
    &[
        (42981, -219308, -8759),
        (42320, -220160, -8759),
        (42434, -219181, -8759),
        (42101, -219550, -8759),
        (41859, -220236, -8759),
        (42881, -219942, -8759),
    ],
    &[
        (48770, -219304, -8759),
        (49036, -220190, -8759),
        (49363, -219814, -8759),
        (49393, -219102, -8759),
        (49618, -220490, -8759),
        (48526, -220493, -8759),
    ],
];

pub const REWARDERS: [(i32, i32, i32); 11] = [
    (9261, -219862, -8021),
    (16301, -219806, -8021),
    (23478, -220079, -7799),
    (9290, -212993, -7799),
    (16598, -212997, -7802),
    (23650, -213051, -8007),
    (9136, -205733, -8007),
    (16508, -205737, -8007),
    (23229, -206316, -7991),
    (42638, -219781, -8759),
    (49014, -219737, -8759),
];

/// `{item, count, item, count}` by grade (F to S; F has none), for each Kamaloka.
const REWARDS: [[Option<[i32; 4]>; 6]; 11] = [
    // 20-30
    [
        None,
        Some([13002, 2, 10839, 1]),
        Some([13002, 2, 10838, 1]),
        Some([13002, 2, 10837, 1]),
        Some([13002, 2, 10836, 1]),
        Some([13002, 2, 12824, 1]),
    ],
    // 25-35
    [
        None,
        Some([13002, 3, 10838, 1]),
        Some([13002, 3, 10837, 1]),
        Some([13002, 3, 10836, 1]),
        Some([13002, 3, 10840, 1]),
        Some([13002, 3, 12825, 1]),
    ],
    // 30-40
    [
        None,
        Some([13002, 3, 10841, 1]),
        Some([13002, 3, 10842, 1]),
        Some([13002, 3, 10843, 1]),
        Some([13002, 3, 10844, 1]),
        Some([13002, 3, 12826, 1]),
    ],
    // 35-45
    [
        None,
        Some([13002, 5, 10842, 1]),
        Some([13002, 5, 10843, 1]),
        Some([13002, 5, 10844, 1]),
        Some([13002, 5, 10845, 1]),
        Some([13002, 5, 12827, 1]),
    ],
    // 40-50
    [
        None,
        Some([13002, 7, 10846, 1]),
        Some([13002, 7, 10847, 1]),
        Some([13002, 7, 10848, 1]),
        Some([13002, 7, 10849, 1]),
        Some([13002, 7, 12828, 1]),
    ],
    // 45-55
    [
        None,
        Some([13002, 8, 10847, 1]),
        Some([13002, 8, 10848, 1]),
        Some([13002, 8, 10849, 1]),
        Some([13002, 8, 10850, 1]),
        Some([13002, 8, 12829, 1]),
    ],
    // 50-60
    [
        None,
        Some([13002, 10, 10851, 1]),
        Some([13002, 10, 10852, 1]),
        Some([13002, 10, 10853, 1]),
        Some([13002, 10, 10854, 1]),
        Some([13002, 10, 12830, 1]),
    ],
    // 55-65
    [
        None,
        Some([13002, 12, 10852, 1]),
        Some([13002, 12, 10853, 1]),
        Some([13002, 12, 10854, 1]),
        Some([13002, 12, 10855, 1]),
        Some([13002, 12, 12831, 1]),
    ],
    // 60-70
    [
        None,
        Some([13002, 13, 10856, 1]),
        Some([13002, 13, 10857, 1]),
        Some([13002, 13, 10858, 1]),
        Some([13002, 13, 10859, 1]),
        Some([13002, 13, 12832, 1]),
    ],
    // 65-75
    [
        None,
        Some([13002, 15, 10857, 1]),
        Some([13002, 15, 10858, 1]),
        Some([13002, 15, 10859, 1]),
        Some([13002, 15, 10860, 1]),
        Some([13002, 15, 12833, 1]),
    ],
    // 70-80
    [
        None,
        Some([13002, 17, 10861, 1]),
        Some([13002, 17, 12834, 1]),
        Some([13002, 17, 10862, 1]),
        Some([13002, 17, 10863, 1]),
        Some([13002, 17, 10864, 1]),
    ],
];

/// Java `RimKamaWorld extends InstanceWorld`: what the script knows of one instance.
#[derive(Debug, Default)]
struct RimKamaWorld {
    index: usize,
    kanabion: i32,
    doppler: i32,
    voider: i32,

    kanabions_count: i32,
    dopplers_count: i32,
    voiders_count: i32,
    grade: i32,
    is_finished: bool,
    is_rewarded: bool,

    /// `spawnedMobs`: the monsters the script spawned.
    spawned_mobs: Vec<ObjectId>,
    /// `lastAttack`: object id -> time of the last attack (ms).
    last_attack: HashMap<ObjectId, i64>,
}

/// The `RimKamaWorld`s by instance id.
#[derive(Default)]
struct State {
    worlds: HashMap<i32, RimKamaWorld>,
}

/// Java `instances.RimKamaloka.RimKamaloka`.
pub struct RimKamaloka;

impl Script for RimKamaloka {
    fn name(&self) -> &'static str {
        "RimKamaloka"
    }

    fn path(&self) -> &'static str {
        "instances/RimKamaloka"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[START_NPC]);
        s.add_first_talk_id(&[START_NPC, REWARDER]);
        s.add_talk_id(&[START_NPC]);
        s.add_talk_id(&[REWARDER]);

        for list in KANABIONS {
            s.add_faction_call_id(&[list[0]]);
            s.add_attack_id(&list);
            s.add_kill_id(&list);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // The three tasks of a world (`FinishTask`, `DespawnTask`, `LockTask`).
        if npc.is_none() && player.is_none() {
            if let Some((task, instance_id)) = event.split_once(' ') {
                if let Ok(instance_id) = instance_id.parse::<i32>() {
                    match task {
                        "RIM_FINISH" => finish_task(ctx, instance_id),
                        "RIM_DESPAWN" => despawn_task(ctx, instance_id),
                        "RIM_LOCK" => lock_task(ctx, instance_id),
                        _ => {}
                    }
                }
            }
            return None;
        }
        let (npc, player) = (npc?, player?);

        match event {
            "npc_rim_maker001.htm" | "npc_rim_maker001a.htm" | "npc_rim_maker001b.htm"
            | "npc_rim_maker001ba.htm" | "npc_rim_maker007.htm" | "npc_rim_maker010.htm"
            | "npc_rim_maker011.htm" => {
                return Some(event.to_owned());
            }
            "Exit" => {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                if ctx.state::<State>().worlds.contains_key(&instance_id)
                    && ctx
                        .world()
                        .instance_world(instance_id)
                        .is_some_and(|w| w.is_allowed(player))
                {
                    let exit = ctx.world().instance(instance_id).and_then(|i| i.exit_location);
                    if let Some((x, y, z)) = exit {
                        ctx.teleport_player(player, x, y, z, 0, true);
                    }
                }
            }
            "Reward" => {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                if ctx.state::<State>().worlds.contains_key(&instance_id)
                    && ctx
                        .world()
                        .instance_world(instance_id)
                        .is_some_and(|w| w.is_allowed(player))
                {
                    reward_player(ctx, instance_id);
                }

                return Some("npc_rim_gift_giver003.htm".to_owned());
            }
            "challenge" => {
                let castle = ctx.world().npc_castle(npc.object_id).map_or(0, |c| c.0);
                if let Some(mut htmltext) = ctx.get_htm(player, &format!("npc_rim_maker002_{castle}.htm")) {
                    if !ctx.world().pc_cafe_enabled() {
                        htmltext = htmltext.replace("; <font color=\"LEVEL\">1000 PC Cafe points</font> are required", "");
                    }

                    if let Some(packet) = NpcHtmlMessage::new(npc.object_id.0, &htmltext) {
                        ctx.world().send(player, &packet);
                    }
                }
            }
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" => {
                if let Ok(index) = event.parse::<usize>() {
                    enter_instance(ctx, player, npc, index);
                }
            }
            _ => {}
        }

        None
    }

    fn on_faction_call(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        _npc: ScriptNpc,
        _caller: ScriptNpc,
        _attacker: ObjectId,
        _is_summon: bool,
    ) {
        // Java's body is commented out: the Kanabions do not call each other.
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        if npc.id == START_NPC {
            // NOTE(java-quirk): `npc.getCastle().getName()` throws for an NPC with no castle.
            let castle = ctx.world().npc_castle(npc.object_id)?;
            return Some(format!("{}.htm", ctx.world().castle(castle)?.name));
        } else if npc.id == REWARDER {
            let instance_id = ctx.world().instance_id_of(npc.object_id);
            if let Some(world) = ctx.state::<State>().worlds.get(&instance_id) {
                if !world.is_finished {
                    return Some(String::new());
                }

                return match world.grade {
                    0 => Some("GradeF.htm".to_owned()),
                    1 => Some("GradeD.htm".to_owned()),
                    2 => Some("GradeC.htm".to_owned()),
                    3 => Some("GradeB.htm".to_owned()),
                    4 => Some("GradeA.htm".to_owned()),
                    5 => Some("GradeS.htm".to_owned()),
                    _ => None,
                };
            }
        }

        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        match npc.id {
            START_NPC => Some("npc_rim_maker001.htm".to_owned()),
            REWARDER => Some("npc_rim_gift_giver001.htm".to_owned()),
            id => Some(format!("{id}.htm")),
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let instance_id = ctx.world().instance_id_of(npc.object_id);
        if !ctx.state::<State>().worlds.contains_key(&instance_id) {
            return;
        }
        let now = ctx.now_ms();
        if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
            world.last_attack.insert(npc.object_id, now);
        }

        let Some((hp, max_hp)) = ctx.world().creature_hp(npc.object_id) else {
            return;
        };
        let max_hp = max_hp as i32;
        if hp == f64::from(max_hp) {
            // `(damage * 100) / maxHp` in int arithmetic.
            if max_hp != 0 && (damage.wrapping_mul(100) / max_hp) > 40 {
                let chance = ctx.get_random(100);
                let mut next_id = 0;

                let (kanabion, doppler, voider) = {
                    let world = &ctx.state::<State>().worlds[&instance_id];
                    (world.kanabion, world.doppler, world.voider)
                };
                if npc.id == kanabion {
                    if chance < 5 {
                        next_id = doppler;
                    }
                } else if npc.id == doppler {
                    if chance < 5 {
                        next_id = doppler;
                    } else if chance < 10 {
                        next_id = voider;
                    }
                } else if npc.id == voider {
                    if chance < 5 {
                        next_id = voider;
                    }
                }

                if next_id > 0 {
                    spawn_next_mob(ctx, instance_id, npc, next_id, attacker);
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let instance_id = ctx.world().instance_id_of(npc.object_id);
        if !ctx.state::<State>().worlds.contains_key(&instance_id) {
            return;
        }
        if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
            world.last_attack.remove(&npc.object_id);
        }

        let chance = ctx.get_random(100);
        let mut next_id = 0;

        // NOTE: `Attackable.isOverhit()` is not modelled by the world (M5): every kill is a plain
        // one, so the over-hit chances (30/40, 30/60, 50) never apply.
        let overhit = false;
        let (kanabion, doppler, voider) = {
            let world = &ctx.state::<State>().worlds[&instance_id];
            (world.kanabion, world.doppler, world.voider)
        };
        if npc.id == kanabion {
            if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
                world.kanabions_count += 1;
            }
            if overhit {
                if chance < 30 {
                    next_id = doppler;
                } else if chance < 40 {
                    next_id = voider;
                }
            } else if chance < 15 {
                next_id = doppler;
            }
        } else if npc.id == doppler {
            if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
                world.dopplers_count += 1;
            }
            if overhit {
                if chance < 30 {
                    next_id = doppler;
                } else if chance < 60 {
                    next_id = voider;
                }
            } else {
                if chance < 10 {
                    next_id = doppler;
                } else if chance < 20 {
                    next_id = voider;
                }
            }
        } else if npc.id == voider {
            if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
                world.voiders_count += 1;
            }
            if overhit {
                if chance < 50 {
                    next_id = voider;
                }
            } else if chance < 20 {
                next_id = voider;
            }
        }

        if next_id > 0 {
            spawn_next_mob(ctx, instance_id, npc, next_id, player);
        }
    }
}

/// Check if party with player as leader allowed to enter
fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc, index: usize) -> bool {
    if ctx.world().party_of(player).is_some() {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::YouCannotEnterDueToThePartyHavingExceededTheLimit),
        );
        return false;
    }

    // Get level of the instance.
    let level = LEVEL[index];

    // and client name
    let instance_name = ctx.world().instance_name(INSTANCE_IDS[index]);

    // Player level must be in range.
    if (ctx.player_level(player) - level).abs() > MAX_LEVEL_DIFFERENCE {
        let name = ctx.player_name(player);
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry)
                .with(SystemMessageParam::PlayerName(name)),
        );

        send_html(ctx, player, npc, "npc_rim_maker004.htm");
        return false;
    }

    // Get instances reenter times for player.
    let now = ctx.now_ms();
    for (id, time) in ctx.world().instance_times(player) {
        // Find instance with same name (Kamaloka or labyrinth).
        if instance_name != ctx.world().instance_name(id) {
            continue;
        }

        // If found instance still can't be reentered - exit.
        if now < time {
            let name = ctx.player_name(player);
            ctx.world().send(
                player,
                &SystemMessage::new(SystemMessageId::C1MayNotReEnterYet)
                    .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }
    }

    if ctx.world().pc_cafe_enabled() {
        let points = ctx.world().pc_cafe_points(player);
        if points < 1000 {
            send_html(ctx, player, npc, "npc_rim_maker003.htm");
            return false;
        }

        ctx.world_mut().set_pc_cafe_points(player, points - 1000);
        let points = ctx.world().pc_cafe_points(player);
        ctx.world().send_pc_cafe_point_info(player, points, -1000, 0);
    }

    true
}

/// `NpcHtmlMessage htmlPacket = new NpcHtmlMessage(npc.getObjectId()); htmlPacket.setHtml(getHtm(player, file));
/// player.sendPacket(htmlPacket);`
fn send_html(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc, file: &str) {
    if let Some(html) = ctx.get_htm(player, file) {
        if let Some(packet) = NpcHtmlMessage::new(npc.object_id.0, &html) {
            ctx.world().send(player, &packet);
        }
    }
}

/// Handling enter of the players into Kamaloka
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc, index: usize) {
    let Some(&template_id) = INSTANCE_IDS.get(index) else {
        return;
    };

    // Check for existing instances for this player.
    let existing = ctx
        .world()
        .player_instance_world(player)
        .map(|w| (w.instance_id, w.template_id));
    if let Some((instance_id, world_template)) = existing {
        // Player already in the instance. But not in kamaloka.
        let rim_index = ctx.state::<State>().worlds.get(&instance_id).map(|w| w.index);
        let Some(rim_index) = rim_index.filter(|_| world_template == template_id) else {
            ctx.world().send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                ),
            );
            return;
        };

        // Check for level difference again on reenter.
        if (ctx.player_level(player) - LEVEL[rim_index]).abs() > MAX_LEVEL_DIFFERENCE {
            let name = ctx.player_name(player);
            ctx.world().send(
                player,
                &SystemMessage::new(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry)
                    .with(SystemMessageParam::PlayerName(name)),
            );
            return;
        }

        // Check what instance still exist.
        if ctx.world().instance(instance_id).is_some() {
            let to = TELEPORTS[index];
            ctx.teleport_player(player, to.0, to.1, to.2, instance_id, true);
        }
    } else {
        // Creating new kamaloka instance.
        if !check_conditions(ctx, player, npc, index) {
            return;
        }

        // Creating new instanceWorld, using our instanceId and templateId.
        let instance_id = ctx.world_mut().create_dynamic_instance(template_id);
        let created_template = ctx
            .world()
            .instance(instance_id)
            .map_or(-1, |i| i.template_id);
        ctx.world_mut()
            .add_instance_world(InstanceWorld::new(instance_id, created_template));

        // Set return location.
        let exit = ctx.world().position_of(player);
        if let Some(instance) = ctx.world_mut().instance_mut(instance_id) {
            instance.exit_location = exit;
        }

        // Set index for easy access to the arrays.
        ctx.state::<State>().worlds.insert(
            instance_id,
            RimKamaWorld {
                index,
                ..RimKamaWorld::default()
            },
        );

        // Spawn NPCs.
        spawn_kama(ctx, instance_id);
        ctx.start_quest_timer(
            &format!("RIM_FINISH {instance_id}"),
            DURATION * 60000,
            None,
            None,
            false,
        );
        ctx.start_quest_timer(&format!("RIM_DESPAWN {instance_id}"), 1000, None, None, true);
        ctx.start_quest_timer(
            &format!("RIM_LOCK {instance_id}"),
            LOCK_TIME * 60000,
            None,
            None,
            false,
        );

        if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
            world.add_allowed(player);
        }

        let to = TELEPORTS[index];
        ctx.teleport_player(player, to.0, to.1, to.2, instance_id, true);
    }
}

/// Spawn all NPCs in kamaloka
fn spawn_kama(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let index = {
        let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) else {
            return;
        };
        world.kanabion = KANABIONS[world.index][0];
        world.doppler = KANABIONS[world.index][1];
        world.voider = KANABIONS[world.index][2];
        world.index
    };

    let mob1 = KANABIONS[index][0];
    for loc in SPAWNLIST[index] {
        // `new Spawn(mob1)`, `setHeading(-1)`, `setRespawnDelay(30)`, `setAmount(1)`,
        // `startRespawn()`, `doSpawn()`.
        if let Some(npc) = ctx.add_spawn(mob1, loc.0, loc.1, loc.2, -1, false, 0, instance_id) {
            if let Some(spawn) = ctx.world().npc_spawn_index(npc.object_id) {
                ctx.world_mut().set_spawn_respawn_delay(spawn, RESPAWN_DELAY);
                ctx.world_mut().start_spawn_respawn(spawn);
            }
        }
    }
}

fn spawn_next_mob(ctx: &mut ScriptCtx<'_>, instance_id: i32, old_npc: ScriptNpc, npc_id: i32, player: ObjectId) {
    if ctx
        .state::<State>()
        .worlds
        .get(&instance_id)
        .is_none_or(|w| w.is_finished)
    {
        return;
    }

    // Java reuses a decayed `Monster` of the same id from `spawnedMobs`; a new NPC of the same
    // template behaves the same (only the object id differs).
    let Some((x, y, z)) = ctx.world().position_of(old_npc.object_id) else {
        return;
    };
    let heading = ctx.world().object_heading(old_npc.object_id);
    let old_instance = ctx.world().instance_id_of(old_npc.object_id);
    let Some(monster) = ctx.add_spawn(npc_id, x, y, z + 20, heading, false, 0, old_instance) else {
        return;
    };
    let now = ctx.now_ms();
    if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
        world.spawned_mobs.push(monster.object_id);
        world.last_attack.insert(monster.object_id, now);
    }

    // `monster.setRunning(); monster.addDamageHate(player, 0, 9999); setIntentionAttack(player)`.
    ctx.add_attack_desire(monster, player, 9999);
}

fn reward_player(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) else {
        return;
    };
    if !world.is_finished || world.is_rewarded {
        return;
    }

    world.is_rewarded = true;

    let all_rewards = &REWARDS[world.index];
    world.grade = world.grade.min(i32::try_from(all_rewards.len()).unwrap_or(6));
    let grade = world.grade;
    // NOTE(java-quirk): `Math.min(grade, allRewards.length)` would index past the end for a
    // grade of 6; the grade never exceeds 5.
    let Some(Some(reward)) = usize::try_from(grade).ok().and_then(|g| all_rewards.get(g)) else {
        return;
    };
    let reward = *reward;

    let allowed = ctx
        .world()
        .instance_world(instance_id)
        .map(|w| w.allowed.clone())
        .unwrap_or_default();
    for player in allowed {
        if ctx.world().player(player).is_some() {
            ctx.world()
                .send(player, &SystemMessage::text(&format!("Grade:{grade}")));
            for pair in reward.chunks(2) {
                // `player.addItem(ItemProcessType.REWARD, reward[i], reward[i + 1], npc, true)`.
                ctx.world_mut()
                    .add_item(player, pair[0], i64::from(pair[1]), true);
            }
        }
    }
}

/// Java `LockTask.run()`: the re-entry time for the players still online, and an instance nobody
/// is in any more ends.
fn lock_task(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let Some((template_id, allowed)) = ctx
        .world()
        .instance_world(instance_id)
        .map(|w| (w.template_id, w.allowed.clone()))
    else {
        return;
    };
    let now = ctx.now_ms();
    let offset = l2_util::time::local_offset_millis(now);
    let reenter = kamaloka_reset_time(now, offset, true);
    let name = ctx.world().instance_name(template_id);

    // Set instance reenter time for all allowed players.
    let mut found = false;
    for player in allowed {
        if ctx.world().player(player).is_some() {
            found = true;
            ctx.world_mut().set_instance_time(player, template_id, reenter);
            ctx.world().send(
                player,
                &SystemMessage::new(
                    SystemMessageId::InstantZoneS1SEntryHasBeenRestrictedYouCanCheckTheNextPossibleEntryTimeByUsingTheCommandInstancezone,
                )
                .with(SystemMessageParam::Text(name.clone())),
            );
        }
    }

    if !found {
        if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
            world.is_finished = true;
            world.spawned_mobs.clear();
            world.last_attack.clear();
        }
        ctx.cancel_quest_timer(&format!("RIM_FINISH {instance_id}"), None, None);
        ctx.cancel_quest_timer(&format!("RIM_DESPAWN {instance_id}"), None, None);
        ctx.world_mut().destroy_instance(instance_id);
        ctx.state::<State>().worlds.remove(&instance_id);
    }
}

/// Java `FinishTask.run()`: the NPCs go, the rewarder comes, the grade is counted.
fn finish_task(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let index = {
        let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) else {
            return;
        };
        world.is_finished = true;
        world.spawned_mobs.clear();
        world.last_attack.clear();
        world.index
    };
    ctx.cancel_quest_timer(&format!("RIM_DESPAWN {instance_id}"), None, None);

    // Destroy instance after EXIT_TIME.
    if ctx.world().instance(instance_id).is_some() {
        ctx.world_mut().remove_instance_npcs(instance_id);
        ctx.world_mut().set_instance_duration(instance_id, EXIT_TIME * 60000);
        let empty = ctx
            .world()
            .instance(instance_id)
            .is_some_and(|i| i.players.is_empty());
        if empty {
            ctx.world_mut()
                .set_instance_duration(instance_id, EMPTY_DESTROY_TIME * 60000);
        } else {
            ctx.world_mut().set_instance_duration(instance_id, EXIT_TIME * 60000);
            ctx.world_mut()
                .set_instance_empty_destroy_time(instance_id, i64::from(EMPTY_DESTROY_TIME) * 60000);
        }
    }

    // Calculate reward.
    if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
        if world.kanabions_count < 10 {
            world.grade = 0;
        } else {
            world.grade = (((world.dopplers_count + (2 * world.voiders_count)) / world.kanabions_count) + 1).min(5);
        }
    }

    // Spawn rewarder NPC.
    // NOTE(java-quirk): when the instance is gone already (it was empty for five minutes) the
    // rewarder is spawned into the open world at these coordinates, as in Java.
    let at = REWARDERS[index];
    ctx.add_spawn(REWARDER, at.0, at.1, at.2, 0, false, 0, instance_id);
}

/// Java `DespawnTask.run()`: a monster nobody has hit for ten seconds goes.
fn despawn_task(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let time = ctx.now_ms();
    let doomed: Vec<ObjectId> = match ctx.state::<State>().worlds.get(&instance_id) {
        Some(world) if !world.is_finished && !world.last_attack.is_empty() && !world.spawned_mobs.is_empty() => {
            world
                .spawned_mobs
                .iter()
                .filter(|mob| world.last_attack.get(mob).is_some_and(|t| (time - t) > DESPAWN_DELAY))
                .copied()
                .collect()
        }
        _ => return,
    };
    for mob in doomed {
        if ctx.world().is_dead(mob) || ctx.world().is_object_invisible(mob) {
            continue;
        }

        ctx.world_mut().delete_npc(mob);
        if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
            world.last_attack.remove(&mob);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{ClientInput, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, X, Y, Z, names_to, pages_to};

    const NPCS: &str = r#"
        <npc id="32484" type="Folk" level="70" name="Rim Kamaloka Manager"/>
        <npc id="32485" type="Folk" level="70" name="Rim Kamaloka Rewarder"/>
        <npc id="22452" type="Monster" level="25" name="Kanabion">
            <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
                <vitals hp="1000" hpRegen="2" mp="400" mpRegen="0.9" />
            </stats>
        </npc>
        <npc id="22453" type="Monster" level="25" name="Doppler"/>
        <npc id="22454" type="Monster" level="25" name="Voider"/>"#;
    const ITEMS: &str = r#"
        <item id="13002" type="EtcItem" name="Kamaloka Chest"><set name="is_stackable" val="true"/></item>
        <item id="10838" type="EtcItem" name="Reward"/>"#;
    const DIR: &str = "data/scripts/instances/RimKamaloka";

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(RimKamaloka);
        let mut pages: Vec<(String, String)> = [
            "npc_rim_maker003.htm",
            "npc_rim_maker004.htm",
            "npc_rim_gift_giver001.htm",
            "npc_rim_gift_giver003.htm",
            "GradeF.htm",
            "GradeD.htm",
            "GradeC.htm",
        ]
        .iter()
        .map(|p| (format!("{DIR}/{p}"), format!("<html><body>{p}</body></html>")))
        .collect();
        pages.push((
            format!("{DIR}/npc_rim_maker001.htm"),
            r#"<html><body>npc_rim_maker001.htm <a action="bypass Script RimKamaloka 0">0</a><a action="bypass Script RimKamaloka 1">1</a></body></html>"#.to_owned(),
        ));
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        t.world.set_instance_data(instance_data(
            &[(46, "Rim Kamaloka"), (47, "Rim Kamaloka")],
            &[(46, r#"<exitPoint x="43000" y="-47000" z="-3000" />"#), (47, "")],
        ));
        let maker = t.spawn(START_NPC, 20);
        (t, maker)
    }

    fn state<R>(t: &mut ScriptTest, f: impl FnOnce(&mut State) -> R) -> R {
        let mut result = None;
        t.world
            .run_as_script("RimKamaloka", |ctx| result = Some(f(ctx.state::<State>())));
        result.unwrap()
    }

    fn npcs_of(t: &ScriptTest, instance_id: i32, template: i32) -> Vec<Oid> {
        t.world
            .instance(instance_id)
            .unwrap()
            .npcs
            .keys()
            .copied()
            .filter(|n| t.world.npc(*n).is_some_and(|n| n.npc_id == template))
            .collect()
    }

    fn enter(t: &mut ScriptTest, maker: Oid, index: i32) {
        t.talk(1, maker);
        t.bypass(1, &format!("Script RimKamaloka {index}"));
        t.ticks(60);
        t.input(1, ClientInput::Appearing);
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
    }

    #[test]
    fn the_first_talk_page_and_the_enter_make_an_instance_with_four_kanabions() {
        let (mut t, maker) = setup();
        t.enter(1, 25, vec![]);
        t.take();
        t.talk(1, maker);
        assert!(
            pages_to(&t.take(), 1)
                .last()
                .is_some_and(|p| p.contains("npc_rim_maker001.htm"))
        );
        t.bypass(1, "Script RimKamaloka 0");
        t.ticks(60);

        let id = t.world.instance_id_of(Oid(1));
        assert_eq!(id, 300_000);
        let world = t.world.instance_world(id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (46, vec![Oid(1)]));
        // The exit point is where the player stood.
        assert_eq!(t.world.instance(id).unwrap().exit_location, Some((X, Y, Z)));
        // Four Kanabions that come back after 30 seconds.
        let kanabions = npcs_of(&t, id, 22452);
        assert_eq!(kanabions.len(), 4);
        for k in &kanabions {
            let spawn = t.world.npc_spawn_index(*k).unwrap();
            assert!(t.world.is_respawn_enabled(spawn));
        }
        // The finish, despawn and lock tasks.
        assert_eq!(t.world.quest_timer_count(), 3);
        // Entering again takes him to the same instance.
        t.input(1, ClientInput::Appearing);
        t.world
            .tele_to_location(Oid(1), l2_world::TeleportTarget::at(X, Y, Z).with_instance(0));
        t.ticks(60);
        t.input(1, ClientInput::Appearing);
        enter(&mut t, maker, 0);
        assert_eq!(t.world.instance_id_of(Oid(1)), id);
        assert_eq!(t.world.quest_timer_count(), 3);
    }

    #[test]
    fn a_player_of_the_wrong_level_or_with_a_reentry_time_is_turned_away() {
        let (mut t, maker) = setup();
        t.enter(1, 25 + MAX_LEVEL_DIFFERENCE + 1, vec![]);
        t.take();
        enter(&mut t, maker, 0);
        let out = t.take();
        assert_eq!(names_to(&out, 1).iter().filter(|n| *n == "SYSTEM_MESSAGE").count(), 1);
        assert!(pages_to(&out, 1).iter().any(|p| p.contains("npc_rim_maker004.htm")));
        assert!(t.world.instances().all(|i| i.id <= 0));

        // The other Rim Kamaloka (47) has the same name: its time keeps him out of this one.
        let (mut t, maker) = setup();
        t.enter(1, 25, vec![]);
        t.world.set_instance_time(Oid(1), 47, now() + 60_000);
        t.take();
        enter(&mut t, maker, 0);
        assert_eq!(names_to(&t.take(), 1).iter().filter(|n| *n == "SYSTEM_MESSAGE").count(), 1);
        assert!(t.world.instances().all(|i| i.id <= 0));
    }

    #[test]
    fn strong_hits_and_kills_call_a_doppler_or_a_voider() {
        let (mut t, maker) = setup();
        t.enter(1, 25, vec![]);
        enter(&mut t, maker, 0);
        let id = 300_000;
        let kanabion = npcs_of(&t, id, 22452)[0];
        let npc = ScriptNpc {
            object_id: kanabion,
            id: 22452,
        };
        // A hit of 50% of the maximum on a monster at full health (5% for a Doppler): over many
        // tries it happens, and the new monster hunts the attacker.
        for _ in 0..100 {
            t.world.run_as_script("RimKamaloka", |ctx| {
                RimKamaloka.on_attack(ctx, npc, Oid(1), 500, false, None);
            });
        }
        let dopplers = npcs_of(&t, id, 22453);
        assert!(!dopplers.is_empty() && dopplers.len() < 30, "{}", dopplers.len());
        assert_eq!(state(&mut t, |s| s.worlds[&id].spawned_mobs.len()), dopplers.len());
        // A weak hit does nothing.
        let before = dopplers.len();
        t.world.run_as_script("RimKamaloka", |ctx| {
            RimKamaloka.on_attack(ctx, npc, Oid(1), 100, false, None);
        });
        assert_eq!(npcs_of(&t, id, 22453).len(), before);

        // Kills count and call more.
        for _ in 0..60 {
            t.world.run_as_script("RimKamaloka", |ctx| {
                RimKamaloka.on_kill(ctx, npc, Oid(1), false);
            });
        }
        assert_eq!(state(&mut t, |s| s.worlds[&id].kanabions_count), 60);
        assert!(npcs_of(&t, id, 22453).len() > before);
        for d in npcs_of(&t, id, 22453) {
            t.world.run_as_script("RimKamaloka", |ctx| {
                RimKamaloka.on_kill(
                    ctx,
                    ScriptNpc {
                        object_id: d,
                        id: 22453,
                    },
                    Oid(1),
                    false,
                );
            });
        }
        assert!(state(&mut t, |s| s.worlds[&id].dopplers_count) > 0);

        // A monster nobody hit for ten seconds goes at the next second (the Kanabions of the
        // spawn list stay). The task reads the wall clock, so the last attack is moved back.
        let doppler = state(&mut t, |s| {
            let w = s.worlds.get_mut(&id).unwrap();
            let mob = *w.spawned_mobs.iter().find(|m| w.last_attack.contains_key(*m)).unwrap();
            *w.last_attack.get_mut(&mob).unwrap() -= 11_000;
            mob
        });
        assert!(t.world.npc(doppler).is_some());
        t.ticks(40);
        assert!(t.world.npc(doppler).is_none());
        assert!(state(&mut t, |s| !s.worlds[&id].last_attack.contains_key(&doppler)));
        assert_eq!(npcs_of(&t, id, 22452).len(), 4);
    }

    #[test]
    fn after_twenty_minutes_the_rewarder_grades_and_pays_once() {
        let (mut t, maker) = setup();
        t.enter(1, 25, vec![]);
        enter(&mut t, maker, 0);
        let id = 300_000;
        // 10 Kanabions, 10 Dopplers and a Voider: grade (10 + 2) / 10 + 1 = 2 (C).
        state(&mut t, |s| {
            let w = s.worlds.get_mut(&id).unwrap();
            w.kanabions_count = 10;
            w.dopplers_count = 10;
            w.voiders_count = 1;
        });

        // At ten minutes the re-entry time is set for the player who is online.
        t.take();
        t.ticks(10 * 60 * 20);
        // (On the last day of a month after 06:00 the rolled date is in the past, see
        // `kamaloka_reset_time`: only the write is checked.)
        assert!(t.world.instance_time(Oid(1), 46) > 0);
        assert_eq!(
            names_to(&t.take(), 1).iter().filter(|n| *n == "SYSTEM_MESSAGE").count(),
            1
        );

        // At twenty the world is finished: the Kanabions are gone, the rewarder stands there.
        t.ticks(10 * 60 * 20);
        assert!(state(&mut t, |s| s.worlds[&id].is_finished));
        assert_eq!(state(&mut t, |s| s.worlds[&id].grade), 2);
        assert!(npcs_of(&t, id, 22452).iter().all(|k| t.world.npc(*k).is_none()));
        let rewarder = npcs_of(&t, id, REWARDER)[0];
        assert_eq!(
            t.world.position_of(rewarder).map(|p| (p.0, p.1)),
            Some((REWARDERS[0].0, REWARDERS[0].1))
        );
        // Ten minutes to leave.
        let instance = t.world.instance(id).unwrap();
        assert!(instance.end_time_ms - now() <= 10 * 60_000 + 5_000);
        assert_eq!(instance.empty_destroy_time_ms, 5 * 60_000);

        // The rewarder's page by grade.
        let ran = t.world.run_as_script("RimKamaloka", |ctx| {
            let npc = ScriptNpc {
                object_id: rewarder,
                id: REWARDER,
            };
            assert_eq!(RimKamaloka.on_talk(ctx, npc, Oid(1)).as_deref(), Some("GradeC.htm"));
            // "Reward": 2 of item 13002 and 1 of 10838, once.
            for _ in 0..2 {
                assert_eq!(
                    RimKamaloka
                        .on_event(ctx, "Reward", Some(npc), Some(Oid(1)))
                        .as_deref(),
                    Some("npc_rim_gift_giver003.htm")
                );
            }
        });
        assert!(ran);
        assert_eq!(t.count(1, 13002), 2);
        assert_eq!(t.count(1, 10838), 1);

        // "Exit" takes the player to the exit point.
        t.world.run_as_script("RimKamaloka", |ctx| {
            let npc = ScriptNpc {
                object_id: rewarder,
                id: REWARDER,
            };
            RimKamaloka.on_event(ctx, "Exit", Some(npc), Some(Oid(1)));
        });
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
    }

    #[test]
    fn an_unfinished_world_has_no_page_and_no_reward() {
        let (mut t, maker) = setup();
        t.enter(1, 25, vec![]);
        enter(&mut t, maker, 0);
        let id = 300_000;
        let rewarder = t
            .world
            .script_add_spawn(REWARDER, (X + 50, Y, Z), 0, false, 0, id)
            .unwrap();
        t.world.run_as_script("RimKamaloka", |ctx| {
            let npc = ScriptNpc {
                object_id: rewarder,
                id: REWARDER,
            };
            assert_eq!(RimKamaloka.on_talk(ctx, npc, Oid(1)).as_deref(), Some(""));
            RimKamaloka.on_event(ctx, "Reward", Some(npc), Some(Oid(1)));
        });
        assert_eq!(t.count(1, 13002), 0);
        assert!(!state(&mut t, |s| s.worlds[&id].is_rewarded));
    }

    #[test]
    fn the_lock_task_ends_an_instance_nobody_is_in() {
        let (mut t, maker) = setup();
        t.enter(1, 25, vec![]);
        enter(&mut t, maker, 0);
        // The player disconnects before the lock task.
        t.world.handle(l2_world::WorldCommand::Disconnected {
            session: l2_world::SessionId(1),
        });
        t.ticks(10 * 60 * 20);
        assert!(t.world.instance(300_000).is_none());
        assert_eq!(t.world.quest_timer_count(), 0);
    }
}
