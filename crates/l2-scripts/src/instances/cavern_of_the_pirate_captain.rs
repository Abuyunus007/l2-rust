//! Cavern Of The Pirate Captain (Day Dream) instance zone: the party lights the candles to find
//! the room Zaken sleeps in. Templates 133 (day, level 60), 114 (night) and 135 (level 83).
// Java: dist/game/data/scripts/instances/CavernOfThePirateCaptain/CavernOfThePirateCaptain.java
//
// Java keeps the state of a run in the parameters of the `InstanceWorld` (`is83`, `isNight`,
// `storeTime`, `playersInside`, `zakenRoom`, `zaken`, `blueFounded`); so does this port, as
// strings, because `onEnterInstance` runs inside `World::enter_instance` without a `ScriptCtx`.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget, World};

// NPCs
const PATHFINDER: i32 = 32713; // Pathfinder Worker
const ZAKEN_60: i32 = 29176; // Zaken
const ZAKEN_60_NIGHT: i32 = 29022; // Zaken Night
const ZAKEN_83: i32 = 29181; // Zaken
const CANDLE: i32 = 32705; // Zaken's Candle
const DOLL_BLADER_60: i32 = 29023; // Doll Blader
const DOLL_BLADER_83: i32 = 29182; // Doll Blader
const VALE_MASTER_60: i32 = 29024; // Veil Master
const VALE_MASTER_83: i32 = 29183; // Veil Master
const PIRATES_ZOMBIE_60: i32 = 29027; // Pirate Zombie
const PIRATES_ZOMBIE_83: i32 = 29185; // Pirate Zombie
const PIRATES_ZOMBIE_CAPTAIN_60: i32 = 29026; // Pirate Zombie Captain
const PIRATES_ZOMBIE_CAPTAIN_83: i32 = 29184; // Pirate Zombie Captain

// Items
const VORPAL_RING: i32 = 15763; // Sealed Vorpal Ring
const VORPAL_EARRING: i32 = 15764; // Sealed Vorpal Earring
const FIRE: i32 = 15280; // Transparent 1HS (for NPC)
const RED: i32 = 15281; // Transparent 1HS (for NPC)
const BLUE: i32 = 15302; // Transparent Bow (for NPC)

// Locations
const ENTER_LOC: [(i32, i32, i32); 3] = [
    (52684, 219989, -3496),
    (52669, 219120, -3224),
    (52672, 219439, -3312),
];
const CANDLE_LOC: [(i32, i32, i32); 36] = [
    // Floor 1
    (53313, 220133, -3498),
    (53313, 218079, -3498),
    (54240, 221045, -3498),
    (54325, 219095, -3498),
    (54240, 217155, -3498),
    (55257, 220028, -3498),
    (55257, 218172, -3498),
    (56280, 221045, -3498),
    (56195, 219095, -3498),
    (56280, 217155, -3498),
    (57215, 220133, -3498),
    (57215, 218079, -3498),
    // Floor 2
    (53313, 220133, -3226),
    (53313, 218079, -3226),
    (54240, 221045, -3226),
    (54325, 219095, -3226),
    (54240, 217155, -3226),
    (55257, 220028, -3226),
    (55257, 218172, -3226),
    (56280, 221045, -3226),
    (56195, 219095, -3226),
    (56280, 217155, -3226),
    (57215, 220133, -3226),
    (57215, 218079, -3226),
    // Floor 3
    (53313, 220133, -2954),
    (53313, 218079, -2954),
    (54240, 221045, -2954),
    (54325, 219095, -2954),
    (54240, 217155, -2954),
    (55257, 220028, -2954),
    (55257, 218172, -2954),
    (56280, 221045, -2954),
    (56195, 219095, -2954),
    (56280, 217155, -2954),
    (57215, 220133, -2954),
    (57215, 218079, -2954),
];

// Misc
const MIN_LV_60: i32 = 55;
const MIN_LV_83: i32 = 78;
const PLAYERS_60_MIN: usize = 9;
const PLAYERS_60_MAX: usize = 27;
const PLAYERS_60_NIGHT_MIN: usize = 72;
const PLAYERS_60_NIGHT_MAX: usize = 450;
const PLAYERS_83_MIN: usize = 9;
const PLAYERS_83_MAX: usize = 27;
const TEMPLATE_ID_60: i32 = 133;
const TEMPLATE_ID_60_NIGHT: i32 = 114;
const TEMPLATE_ID_83: i32 = 135;
const ROOM_DATA: [[i32; 7]; 15] = [
    // Floor 1
    [54240, 220133, -3498, 1, 3, 4, 6],
    [54240, 218073, -3498, 2, 5, 4, 7],
    [55265, 219095, -3498, 4, 9, 6, 7],
    [56289, 220133, -3498, 8, 11, 6, 9],
    [56289, 218073, -3498, 10, 12, 7, 9],
    // Floor 2
    [54240, 220133, -3226, 13, 15, 16, 18],
    [54240, 218073, -3226, 14, 17, 16, 19],
    [55265, 219095, -3226, 21, 16, 19, 18],
    [56289, 220133, -3226, 20, 23, 21, 18],
    [56289, 218073, -3226, 22, 24, 19, 21],
    // Floor 3
    [54240, 220133, -2954, 25, 27, 28, 30],
    [54240, 218073, -2954, 26, 29, 28, 31],
    [55265, 219095, -2954, 33, 28, 31, 30],
    [56289, 220133, -2954, 32, 35, 30, 33],
    [56289, 218073, -2954, 34, 36, 31, 33],
];

/// Java `instances.CavernOfThePirateCaptain.CavernOfThePirateCaptain`.
pub struct CavernOfThePirateCaptain;

impl Script for CavernOfThePirateCaptain {
    fn name(&self) -> &'static str {
        "CavernOfThePirateCaptain"
    }

    fn path(&self) -> &'static str {
        "instances/CavernOfThePirateCaptain"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PATHFINDER]);
        s.add_talk_id(&[PATHFINDER]);
        s.add_kill_id(&[ZAKEN_60, ZAKEN_60_NIGHT, ZAKEN_83]);
        s.add_first_talk_id(&[CANDLE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "enter60" {
            let player = player?;
            if check_conditions(ctx, player, TEMPLATE_ID_60) {
                enter_instance(ctx, player, TEMPLATE_ID_60);
            }
        } else if event == "enter60night" {
            let player = player?;
            if check_conditions(ctx, player, TEMPLATE_ID_60_NIGHT) {
                enter_instance(ctx, player, TEMPLATE_ID_60_NIGHT);
            }
        } else if event == "enter83" {
            let player = player?;
            if check_conditions(ctx, player, TEMPLATE_ID_83) {
                enter_instance(ctx, player, TEMPLATE_ID_83);
            }
        } else {
            let npc = npc?;
            let instance_id = ctx
                .world()
                .instance_world_of(npc.object_id)
                .map(|w| w.instance_id);
            if let Some(instance_id) = instance_id {
                let is83 = parameter_bool(ctx.world(), instance_id, "is83");
                match event {
                    "BURN_BLUE" => {
                        if right_hand_of(ctx, npc) == 0 {
                            ctx.world_mut().set_npc_right_hand(npc.object_id, FIRE);
                            ctx.start_quest_timer("BURN_BLUE2", 3000, Some(npc), player, false);
                            if parameter_int(ctx.world(), instance_id, "blueFounded") == 4 {
                                ctx.start_quest_timer("SHOW_ZAKEN", 5000, Some(npc), player, false);
                            }
                        }
                    }
                    "BURN_BLUE2" => {
                        if right_hand_of(ctx, npc) == FIRE {
                            ctx.world_mut().set_npc_right_hand(npc.object_id, BLUE);
                        }
                    }
                    "BURN_RED" => {
                        if right_hand_of(ctx, npc) == 0 {
                            ctx.world_mut().set_npc_right_hand(npc.object_id, FIRE);
                            ctx.start_quest_timer("BURN_RED2", 3000, Some(npc), player, false);
                        }
                    }
                    "BURN_RED2" => {
                        if right_hand_of(ctx, npc) == FIRE {
                            let room = get_room_by_candle(ctx, npc);
                            ctx.world_mut().set_npc_right_hand(npc.object_id, RED);
                            manage_screen_msg(
                                ctx,
                                instance_id,
                                NpcStringId::TheCandlesCanLeadYouToZakenDestroyHim,
                            );
                            for mob in [
                                if is83 { DOLL_BLADER_83 } else { DOLL_BLADER_60 },
                                if is83 { VALE_MASTER_83 } else { VALE_MASTER_60 },
                                if is83 {
                                    PIRATES_ZOMBIE_83
                                } else {
                                    PIRATES_ZOMBIE_60
                                },
                                if is83 {
                                    PIRATES_ZOMBIE_CAPTAIN_83
                                } else {
                                    PIRATES_ZOMBIE_CAPTAIN_60
                                },
                            ] {
                                spawn_attacker(ctx, mob, room, player, instance_id);
                            }
                        }
                    }
                    "SHOW_ZAKEN" => {
                        if is83 {
                            manage_screen_msg(ctx, instance_id, NpcStringId::WhoDaresAwakenTheMightyZaken);
                        }

                        // NOTE(java-quirk): the Zaken of the world is used without a null check.
                        let zaken = parameter_int(ctx.world(), instance_id, "zaken");
                        ctx.world_mut().set_invisible(ObjectId(zaken), false);
                        ctx.world_mut().set_creature_paralyzed(ObjectId(zaken), false);
                        let zaken_room = parameter_int(ctx.world(), instance_id, "zakenRoom");
                        for mob in [
                            if is83 { DOLL_BLADER_83 } else { DOLL_BLADER_60 },
                            if is83 {
                                PIRATES_ZOMBIE_83
                            } else {
                                PIRATES_ZOMBIE_60
                            },
                            if is83 {
                                PIRATES_ZOMBIE_CAPTAIN_83
                            } else {
                                PIRATES_ZOMBIE_CAPTAIN_60
                            },
                        ] {
                            spawn_attacker(ctx, mob, zaken_room, player, instance_id);
                        }
                    }
                    _ => {}
                }
            }
        }

        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let Some(instance_id) = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id)
        else {
            return;
        };
        if npc.id == ZAKEN_83 {
            let store_time = parameter(ctx.world(), instance_id, "storeTime")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            for players_inside in players_inside_of(ctx.world(), instance_id) {
                let near = ctx.world().instance_id_of(players_inside) == instance_id
                    && ctx
                        .distance_3d(players_inside, npc.object_id)
                        .is_some_and(|d| d <= 1500.0);
                if near {
                    let time = ctx.world().instance_clock_ms() - store_time;
                    if time <= 300_000 {
                        // 5 minutes
                        if ctx.get_random_boolean() {
                            ctx.give_items(players_inside, VORPAL_RING, 1);
                        }
                    } else if time <= 600_000 {
                        // 10 minutes
                        if ctx.get_random(100) < 30 {
                            ctx.give_items(players_inside, VORPAL_EARRING, 1);
                        }
                    } else if time <= 900_000 {
                        // 15 minutes
                        if ctx.get_random(100) < 25 {
                            ctx.give_items(players_inside, VORPAL_RING, 1);
                        }
                    }
                }
            }
        }

        ctx.world_mut().finish_instance(instance_id);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let instance_id = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id);
        if let Some(instance_id) = instance_id {
            let is_blue = ctx
                .world()
                .npc_variables(npc.object_id)
                .is_some_and(|v| v.get_int("isBlue", 0) == 1);
            if ctx.is_script_value(npc, 0) {
                if is_blue {
                    let blue_founded = parameter_int(ctx.world(), instance_id, "blueFounded") + 1;
                    set_parameter(
                        ctx.world_mut(),
                        instance_id,
                        "blueFounded",
                        blue_founded.to_string(),
                    );
                    ctx.start_quest_timer("BURN_BLUE", 500, Some(npc), Some(player), false);
                } else {
                    ctx.start_quest_timer("BURN_RED", 500, Some(npc), Some(player), false);
                }

                ctx.set_script_value(npc, 1);
            }
        }

        None
    }
}

/// `npc.getRightHandItem()`.
fn right_hand_of(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> i32 {
    ctx.world().npc(npc.object_id).map_or(0, |n| n.right_hand)
}

fn parameter(world: &World, instance_id: i32, key: &str) -> Option<String> {
    world
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get(key).cloned())
}

/// `world.getParameters().getInt(key, 0)`.
fn parameter_int(world: &World, instance_id: i32, key: &str) -> i32 {
    parameter(world, instance_id, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// `world.getParameters().getBoolean(key, false)`.
fn parameter_bool(world: &World, instance_id: i32, key: &str) -> bool {
    parameter(world, instance_id, key).is_some_and(|v| v == "true")
}

/// `world.setParameter(key, value)`.
fn set_parameter(world: &mut World, instance_id: i32, key: &str, value: String) {
    if let Some(instance_world) = world.instance_world_mut(instance_id) {
        instance_world.parameters.insert(key.to_owned(), value);
    }
}

/// `world.getParameters().getList("playersInside", Player.class)`.
fn players_inside_of(world: &World, instance_id: i32) -> Vec<ObjectId> {
    parameter(world, instance_id, "playersInside")
        .map(|list| {
            list.split(',')
                .filter_map(|id| id.parse().ok())
                .map(ObjectId)
                .collect()
        })
        .unwrap_or_default()
}

/// Java `checkConditions(player, templateId)`.
fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId, template_id: i32) -> bool {
    if ctx.is_gm(player) {
        return true;
    }

    let Some(party) = ctx.world().party_of(player) else {
        ctx.world().broadcast_player_packet(
            player,
            &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter),
        );
        return false;
    };

    let is83 = template_id == TEMPLATE_ID_83;
    let is_night = template_id == TEMPLATE_ID_60_NIGHT;
    let channel = ctx.world().command_channel_of_party(party);
    let is_party_leader = match channel {
        Some(channel) => ctx.world().is_command_channel_leader(channel, player),
        None => ctx.world().party_leader(party) == Some(player),
    };
    if !is_party_leader {
        ctx.world().broadcast_player_packet(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    let members = match channel {
        Some(channel) => ctx.world().command_channel_members(channel),
        None => ctx.world().party_members(party),
    };

    let (min, max) = if is83 {
        (PLAYERS_83_MIN, PLAYERS_83_MAX)
    } else if is_night {
        (PLAYERS_60_NIGHT_MIN, PLAYERS_60_NIGHT_MAX)
    } else {
        (PLAYERS_60_MIN, PLAYERS_60_MAX)
    };
    if members.len() < min || members.len() > max {
        ctx.world().broadcast_player_packet(
            player,
            &SystemMessage::new(SystemMessageId::YouCannotEnterDueToThePartyHavingExceededTheLimit),
        );
        return false;
    }

    for group_member in &members {
        let group_member = *group_member;
        if ctx.player_level(group_member) < if is83 { MIN_LV_83 } else { MIN_LV_60 } {
            broadcast_to_group(
                ctx,
                &members,
                group_member,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
            );
            return false;
        }

        if !ctx.distance_3d(player, group_member).is_some_and(|d| d <= 1000.0) {
            broadcast_to_group(
                ctx,
                &members,
                group_member,
                SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
            );
            return false;
        }

        let reentertime = ctx.world().instance_time(group_member, template_id);
        if ctx.world().instance_clock_ms() < reentertime {
            broadcast_to_group(ctx, &members, group_member, SystemMessageId::C1MayNotReEnterYet);
            return false;
        }
    }

    true
}

/// `broadcastSystemMessage(player, member, msgId, true)`: `party.broadcastPacket(sm)` or
/// `commandChannel.broadcastPacket(sm)` with the name of `member`.
fn broadcast_to_group(ctx: &ScriptCtx<'_>, members: &[ObjectId], member: ObjectId, id: SystemMessageId) {
    let sm = SystemMessage::new(id).with(SystemMessageParam::PlayerName(ctx.player_name(member)));
    for receiver in members {
        ctx.world().send(*receiver, &sm);
    }
}

/// Java `enterInstance(player, templateId)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId, template_id: i32) {
    ctx.world_mut().enter_instance(
        player,
        template_id,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                let template = world.instance_world(instance_id).map_or(0, |w| w.template_id);
                set_parameter(
                    world,
                    instance_id,
                    "is83",
                    (template == TEMPLATE_ID_83).to_string(),
                );
                set_parameter(
                    world,
                    instance_id,
                    "isNight",
                    (template == TEMPLATE_ID_60_NIGHT).to_string(),
                );
                // `System.currentTimeMillis()`.
                let now = world.instance_clock_ms();
                set_parameter(world, instance_id, "storeTime", now.to_string());

                let mut players_inside = Vec::new();
                match world.party_of(player) {
                    None => {
                        players_inside.push(player);
                        manage_player_enter(world, player, instance_id);
                    }
                    Some(party) => {
                        let members = match world.command_channel_of_party(party) {
                            Some(channel) => world.command_channel_members(channel),
                            None => world.party_members(party),
                        };
                        for member in members {
                            players_inside.push(member);
                            manage_player_enter(world, member, instance_id);
                        }
                    }
                }

                let list: Vec<String> = players_inside.iter().map(|p| p.0.to_string()).collect();
                set_parameter(world, instance_id, "playersInside", list.join(","));
                manage_npc_spawn(world, instance_id);
            } else {
                teleport_to_random_entry(world, player, instance_id);
            }
        },
    );
}

/// `teleportPlayer(player, getRandomEntry(ENTER_LOC), instanceId, false)`.
fn teleport_to_random_entry(world: &mut World, player: ObjectId, instance_id: i32) {
    let index = world.rng().below(ENTER_LOC.len() as i32) as usize;
    let (x, y, z) = ENTER_LOC[index.min(ENTER_LOC.len() - 1)];
    world.tele_to_location(player, TeleportTarget::at(x, y, z).with_instance(instance_id));
}

/// Java `managePlayerEnter(player, world)`.
fn manage_player_enter(world: &mut World, player: ObjectId, instance_id: i32) {
    if let Some(instance_world) = world.instance_world_mut(instance_id) {
        instance_world.add_allowed(player);
    }
    teleport_to_random_entry(world, player, instance_id);
}

/// Java `getRoomByCandle(npc)`.
fn get_room_by_candle(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> i32 {
    let candle_id = ctx
        .world()
        .npc_variables(npc.object_id)
        .map_or(0, |v| v.get_int("candleId", 0));
    for (i, room) in ROOM_DATA.iter().enumerate() {
        if room[3] == candle_id || room[4] == candle_id {
            return i as i32 + 1;
        }
    }

    if candle_id == 6 || candle_id == 7 {
        return 3;
    } else if candle_id == 18 || candle_id == 19 {
        return 8;
    } else if candle_id == 30 || candle_id == 31 {
        return 13;
    }

    0
}

/// Java `manageScreenMsg(world, stringId)`.
fn manage_screen_msg(ctx: &mut ScriptCtx<'_>, instance_id: i32, string_id: NpcStringId) {
    for players in players_inside_of(ctx.world(), instance_id) {
        if ctx.world().player(players).is_some() && ctx.world().instance_id_of(players) == instance_id {
            ctx.show_on_screen_npc_string(players, string_id, 2, 6000, &[]);
        }
    }
}

/// Java `spawnNpc(npcId, roomId, player, world)`.
fn spawn_npc(
    world: &mut World,
    npc_id: i32,
    room_id: i32,
    player: Option<ObjectId>,
    instance_id: i32,
) -> Option<ObjectId> {
    // NOTE(java-quirk): a room of 0 (a candle no room knows) throws in Java.
    let room = ROOM_DATA.get(usize::try_from(room_id - 1).ok()?)?;
    if player.is_some() && npc_id != ZAKEN_60 && npc_id != ZAKEN_83 {
        let x = room[0] + world.rng().below(350);
        let y = room[1] + world.rng().below(350);
        return world.script_add_spawn(npc_id, (x, y, room[2]), 0, false, 0, instance_id);
    }

    world.script_add_spawn(npc_id, (room[0], room[1], room[2]), 0, false, 0, instance_id)
}

/// `spawnNpc(..)` with a player, then `addAttackDesire(mob, player)`.
fn spawn_attacker(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    room_id: i32,
    player: Option<ObjectId>,
    instance_id: i32,
) {
    if let Some(mob) = spawn_npc(ctx.world_mut(), npc_id, room_id, player, instance_id)
        && let Some(player) = player
    {
        ctx.add_attack_desire(
            ScriptNpc {
                object_id: mob,
                id: npc_id,
            },
            player,
            999,
        );
    }
}

/// Java `manageNpcSpawn(world)`.
fn manage_npc_spawn(world: &mut World, instance_id: i32) {
    let zaken_room = world.rng().get(1, 15);
    set_parameter(world, instance_id, "zakenRoom", zaken_room.to_string());
    let mut candles = Vec::new();
    for (i, (x, y, z)) in CANDLE_LOC.iter().enumerate() {
        let candle = world.script_add_spawn(CANDLE, (*x, *y, *z), 0, false, 0, instance_id);
        if let Some(candle) = candle {
            world.npc_variables_mut(candle).set_int("candleId", i as i32 + 1);
        }
        candles.push(candle);
    }

    for i in 3..7 {
        let index = ROOM_DATA[(zaken_room - 1) as usize][i] - 1;
        if let Some(Some(candle)) = candles.get(index as usize) {
            world.npc_variables_mut(*candle).set_int("isBlue", 1);
        }
    }

    let is83 = parameter_bool(world, instance_id, "is83");
    let is_night = parameter_bool(world, instance_id, "isNight");
    let zaken_id = if is83 {
        ZAKEN_83
    } else if is_night {
        ZAKEN_60_NIGHT
    } else {
        ZAKEN_60
    };
    if let Some(zaken) = spawn_npc(world, zaken_id, zaken_room, None, instance_id) {
        world.set_invisible(zaken, true);
        world.set_creature_paralyzed(zaken, true);
        set_parameter(world, instance_id, "zaken", zaken.0.to_string());
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, item};

    const NAME: &str = "CavernOfThePirateCaptain";
    const NPCS: &str = r#"
        <npc id="32713" type="Folk" level="70" name="Pathfinder Worker"/>
        <npc id="32705" type="Folk" level="70" name="Zaken's Candle"/>
        <npc id="29176" type="RaidBoss" level="60" name="Zaken"/>
        <npc id="29022" type="RaidBoss" level="60" name="Zaken Night"/>
        <npc id="29181" type="RaidBoss" level="83" name="Zaken 83"/>
        <npc id="29023" type="Monster" level="60" name="Doll Blader"/>
        <npc id="29182" type="Monster" level="83" name="Doll Blader 83"/>
        <npc id="29024" type="Monster" level="60" name="Veil Master"/>
        <npc id="29183" type="Monster" level="83" name="Veil Master 83"/>
        <npc id="29027" type="Monster" level="60" name="Pirate Zombie"/>
        <npc id="29185" type="Monster" level="83" name="Pirate Zombie 83"/>
        <npc id="29026" type="Monster" level="60" name="Pirate Zombie Captain"/>
        <npc id="29184" type="Monster" level="83" name="Pirate Zombie Captain 83"/>"#;
    const ITEMS: &str = r#"
        <item id="15763" type="EtcItem" name="Sealed Vorpal Ring"/>
        <item id="15764" type="EtcItem" name="Sealed Vorpal Earring"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(CavernOfThePirateCaptain);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.world.set_instance_data(instance_data(
            &[(133, "Cavern 60"), (114, "Cavern Night"), (135, "Cavern 83")],
            &[
                (133, r#"<exitPoint x="52212" y="218998" z="-3208" />"#),
                (114, r#"<exitPoint x="52212" y="218998" z="-3208" />"#),
                (135, r#"<exitPoint x="52212" y="218998" z="-3208" />"#),
            ],
        ));
        t
    }

    fn enter(t: &mut ScriptTest, player: i32, template_id: i32) -> i32 {
        t.world
            .run_as_script(NAME, |ctx| enter_instance(ctx, Oid(player), template_id));
        t.ticks(60);
        t.world.instance_id_of(Oid(player))
    }

    fn candles(t: &ScriptTest) -> Vec<Oid> {
        let mut found = t.npcs_of(CANDLE);
        found.sort_by_key(|c| t.world.npc_variables(*c).map_or(0, |v| v.get_int("candleId", 0)));
        found
    }

    fn is_blue(t: &ScriptTest, candle: Oid) -> bool {
        t.world
            .npc_variables(candle)
            .is_some_and(|v| v.get_int("isBlue", 0) == 1)
    }

    fn first_talk(t: &mut ScriptTest, candle: Oid) {
        let npc = ScriptNpc {
            object_id: candle,
            id: CANDLE,
        };
        t.world.run_as_script(NAME, |ctx| {
            CavernOfThePirateCaptain.on_first_talk(ctx, npc, Oid(1));
        });
    }

    #[test]
    fn a_player_without_a_party_is_refused() {
        let mut t = setup();
        t.enter(1, 82, vec![]);
        t.take();
        t.world.run_as_script(NAME, |ctx| {
            assert!(!check_conditions(ctx, Oid(1), TEMPLATE_ID_60));
        });
        // The refusal went to the player: "You are not currently in a party".
        assert!(!t.take().is_empty());
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
    }

    #[test]
    fn enter_makes_the_candles_and_hides_zaken_in_one_room() {
        let mut t = setup();
        t.enter(1, 82, vec![]);
        let instance_id = enter(&mut t, 1, TEMPLATE_ID_60);
        assert!(instance_id > 0);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (133, vec![Oid(1)]));
        assert_eq!(world.parameters.get("is83").map(String::as_str), Some("false"));
        assert_eq!(
            world.parameters.get("playersInside").map(String::as_str),
            Some("1")
        );
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert!(ENTER_LOC.iter().any(|(ex, ey, _)| (*ex, *ey) == (x, y)));

        // 36 candles in the instance, the blue ones are the four of Zaken's room.
        let all = candles(&t);
        assert_eq!(all.len(), 36);
        let mut blue: Vec<i32> = all
            .iter()
            .filter(|c| is_blue(&t, **c))
            .map(|c| t.world.npc_variables(*c).unwrap().get_int("candleId", 0))
            .collect();
        blue.sort_unstable();
        let room = parameter_int(&t.world, instance_id, "zakenRoom");
        assert!((1..=15).contains(&room));
        let mut expected: Vec<i32> = (3..7).map(|i| ROOM_DATA[(room - 1) as usize][i]).collect();
        expected.sort_unstable();
        expected.dedup();
        assert_eq!(blue, expected);
        assert!(all.iter().all(|c| t.world.instance_id_of(*c) == instance_id));

        // Zaken waits invisible in that room.
        let zaken = Oid(parameter_int(&t.world, instance_id, "zaken"));
        assert_eq!(t.world.npc(zaken).unwrap().npc_id, ZAKEN_60);
        assert!(t.world.is_object_invisible(zaken));

        // A second enter teleports back into the same world without a second set of candles.
        enter(&mut t, 1, TEMPLATE_ID_60);
        assert_eq!(candles(&t).len(), 36);
    }

    #[test]
    fn a_red_candle_burns_and_sends_four_monsters_at_the_player() {
        let mut t = setup();
        t.enter(1, 82, vec![]);
        let instance_id = enter(&mut t, 1, TEMPLATE_ID_60);
        let red = candles(&t).into_iter().find(|c| !is_blue(&t, *c)).unwrap();
        first_talk(&mut t, red);
        // A second click does nothing: the script value is 1.
        first_talk(&mut t, red);
        assert_eq!(t.world.quest_timer_count(), 1);
        t.wait_ms(600);
        assert_eq!(t.world.npc(red).unwrap().right_hand, FIRE);
        t.wait_ms(3_100);
        assert_eq!(t.world.npc(red).unwrap().right_hand, RED);
        for id in [
            DOLL_BLADER_60,
            VALE_MASTER_60,
            PIRATES_ZOMBIE_60,
            PIRATES_ZOMBIE_CAPTAIN_60,
        ] {
            let found = t.npcs_of(id);
            assert_eq!(found.len(), 1, "{id}");
            assert_eq!(t.world.instance_id_of(found[0]), instance_id);
            t.world.delete_npc(found[0]);
        }
    }

    #[test]
    fn the_fourth_blue_candle_wakes_zaken() {
        let mut t = setup();
        t.enter(1, 82, vec![]);
        let instance_id = enter(&mut t, 1, TEMPLATE_ID_60);
        let blue: Vec<Oid> = candles(&t).into_iter().filter(|c| is_blue(&t, *c)).collect();
        assert_eq!(blue.len(), 4);
        for candle in &blue {
            first_talk(&mut t, *candle);
            // The count is read when BURN_BLUE fires, half a second later.
            t.wait_ms(600);
        }
        assert_eq!(parameter_int(&t.world, instance_id, "blueFounded"), 4);
        t.wait_ms(5_600);
        assert!(blue.iter().all(|c| t.world.npc(*c).unwrap().right_hand == BLUE));
        let zaken = Oid(parameter_int(&t.world, instance_id, "zaken"));
        assert!(!t.world.is_object_invisible(zaken));
        // The room's guards stand next to Zaken.
        assert_eq!(t.npcs_of(DOLL_BLADER_60).len(), 1);
        for id in [DOLL_BLADER_60, PIRATES_ZOMBIE_60, PIRATES_ZOMBIE_CAPTAIN_60] {
            for mob in t.npcs_of(id) {
                t.world.delete_npc(mob);
            }
        }
    }

    #[test]
    fn killing_zaken_finishes_the_instance() {
        let mut t = setup();
        t.enter(1, 82, vec![item(900_001, 15763, 0)]);
        let instance_id = enter(&mut t, 1, TEMPLATE_ID_83);
        let zaken = Oid(parameter_int(&t.world, instance_id, "zaken"));
        assert_eq!(t.world.npc(zaken).unwrap().npc_id, ZAKEN_83);
        let before = t.world.instance(instance_id).unwrap().empty_destroy_time_ms;
        assert_ne!(before, 0);
        t.world.run_as_script(NAME, |ctx| {
            CavernOfThePirateCaptain.on_kill(
                ctx,
                ScriptNpc {
                    object_id: zaken,
                    id: ZAKEN_83,
                },
                Oid(1),
                false,
            );
        });
        assert_eq!(t.world.instance(instance_id).unwrap().empty_destroy_time_ms, 0);
        // Zaken's room is more than 1500 away from the entrance: no reward.
        assert_eq!(t.count(1, 15764), 0);
    }
}
