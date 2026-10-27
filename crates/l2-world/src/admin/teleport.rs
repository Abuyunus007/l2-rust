//! The rest of the teleport admin commands: `//walk <x> <y> <z>` (the GM walks there) and
//! `//recall_npc` (the targeted monster is moved to the GM's place).
//!
//! `//teleto`, `//teleto r|end` and `//instant_move` set the GM's click-to-teleport mode, which
//! the `MoveToLocation` handler reads (`tele_mode` of `zone_tools.rs`).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminTeleport.java (walk, recallNPC)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::warn;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::objects::ObjectKind;
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminTeleport.ADMIN_COMMANDS`, the words of this file.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_walk", admin_walk),
        ("admin_recall_npc", admin_recall_npc),
        ("admin_sendhome", admin_sendhome),
        ("admin_teleto", admin_teleto),
        ("admin_instant_move", admin_instant_move),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminTeleport.onCommand`, `admin_walk <x> <y> <z>`: the GM's AI moves to the place. Any
/// bad or missing number does nothing (Java's `catch (Exception e)` is empty).
fn admin_walk(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let coords = (|| -> Option<(i32, i32, i32)> {
        let x = parse_int(words.get(1)?).ok()?;
        let y = parse_int(words.get(2)?).ok()?;
        let z = parse_int(words.get(3)?).ok()?;
        Some((x, y, z))
    })();
    if let Some((x, y, z)) = coords {
        world.set_intention_move_to(gm, x, y, z);
    }
    Ok(())
}

/// Java `AdminTeleport.onCommand`, `admin_recall_npc`: the targeted monster is removed and
/// spawned again at the GM's place, with the respawn delay of its spawn. Minions, grand bosses and
/// anything else are the invalid target.
fn admin_recall_npc(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let target = world.target_of(gm);
    let npc = target.filter(|t| world.objects.get(t).is_some_and(|e| e.kind == ObjectKind::Npc));
    let Some(npc) = npc.filter(|t| world.npcs.contains_key(t)) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    let Some(entry) = world.npcs.get(&npc) else {
        return Ok(());
    };
    let (class, leader, spawn_index) = (entry.class, entry.leader, entry.spawn);
    let template_id = entry.npc_id;
    // Java `!isMinion() && !(RaidBoss) && !(GrandBoss)`.
    let raid = class == crate::npc::NpcClass::RaidBoss;
    if leader.is_some() || class == crate::npc::NpcClass::GrandBoss {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    }
    if raid {
        // FOLLOW-UP(M4): a raid boss goes through `RaidBossSpawnManager` and its own table
        // (`raidboss_spawnlist`), which is not ported. The spawn check still answers as Java does.
        if spawn_index.is_none() {
            world.send_sys_message(gm, "Incorrect raid spawn.");
        } else {
            warn!(
                "//recall_npc on raid boss {}: the raid spawn table is not ported",
                npc.0
            );
        }
        return Ok(());
    }
    let Some(index) = spawn_index else {
        world.send_sys_message(gm, "Incorrect monster spawn.");
        return Ok(());
    };
    // Java `spawn.getRespawnDelay() / 1000`: the delay in seconds.
    let respawn_seconds = world.spawn_def(index).map_or(0, |def| def.respawn_min_ms / 1000);
    let name = world
        .npc_world
        .template(template_id)
        .map(|t| t.name.clone())
        .unwrap_or_default();

    // Java `target.deleteMe()`, `spawn.stopRespawn()`, `SpawnData.deleteSpawn(spawn)`: the
    // monster goes without a respawn of its own.
    world.stop_respawn(index);
    world.cancel_decay_task(npc);
    world.on_npc_decay(npc);

    // Java `new Spawn(id)` with the GM's place and heading, amount 1, `addNewSpawn`, `init()`.
    let (x, y, z) = world.position_of(gm).unwrap_or((0, 0, 0));
    let heading = world.players.get(&gm).map_or(0, |p| p.core.heading);
    let created = world.admin_spawn(template_id, (x, y, z), heading, 1, respawn_seconds, true);
    if created.is_empty() {
        world.send(gm, &SystemMessage::new(SystemMessageId::YourTargetCannotBeFound));
        return Ok(());
    }
    world.send_sys_message(gm, &format!("Created {name} on {}.", npc.0));
    Ok(())
}

/// Java `AdminTeleport.onCommand`, `admin_teleto` (mode 1), `admin_teleto r` (mode 2, the
/// "ready" line) and `admin_teleto end` (mode 0).
fn admin_teleto(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    match command {
        "admin_teleto" => world.set_tele_mode(gm, 1),
        "admin_teleto r" => {
            world.send_sys_message(gm, "Instant move ready. Click where you want to go.");
            world.set_tele_mode(gm, 2);
        }
        "admin_teleto end" => world.set_tele_mode(gm, 0),
        _ => {}
    }
    Ok(())
}

/// Java `AdminTeleport.onCommand`, `admin_instant_move`: the next click teleports (mode 1).
fn admin_instant_move(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_instant_move" {
        world.send_sys_message(gm, "Instant move ready. Click where you want to go.");
        world.set_tele_mode(gm, 1);
    }
    Ok(())
}

/// Java `AdminTeleport.onCommand`, `admin_sendhome [name]`: the named player (or the targeted
/// player) goes to the town of its race. A name with more words is the usage text; a name of a
/// player who is not online is `THAT_PLAYER_IS_NOT_ONLINE`.
fn admin_sendhome(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let params = &words[1.min(words.len())..];
    match params {
        [] => {
            let target = world.target_of(gm).filter(|t| world.is_player(*t));
            match target {
                Some(player) => teleport_home(world, player),
                None => {
                    world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
                }
            }
        }
        [name] => match world.player_by_name(name) {
            Some(player) => teleport_home(world, player),
            None => {
                world.send(gm, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            }
        },
        _ => world.send_sys_message(gm, "Usage: //sendhome <playername>"),
    }
    Ok(())
}

/// Java `AdminTeleport.teleportHome(player)`: the spawn of the race's town region (Talking Island
/// for a human or any other race), with a random offset as `teleToLocation(loc, true)` has.
fn teleport_home(world: &mut World, player: ObjectId) {
    let region = match world.players.get(&player).map(|p| p.data.race) {
        Some(1) => "elf_town",
        Some(2) => "darkelf_town",
        Some(3) => "orc_town",
        Some(4) => "dwarf_town",
        Some(5) => "kamael_town",
        _ => "talking_island_town",
    };
    let Some(regions) = world.zoning.map_regions.clone() else {
        return;
    };
    let random_town = world.settings.teleport.random_respawn_in_town_enabled;
    let rng = world.rng();
    let mut pick = |len: usize| {
        if random_town {
            usize::try_from(rng.below(i32::try_from(len).unwrap_or(i32::MAX))).unwrap_or(0)
        } else {
            0
        }
    };
    let Some(loc) = regions.spawn_loc_by_region_name(region, &mut pick) else {
        return;
    };
    let offset = world.settings.teleport.max_offset_on_teleport;
    world.tele_to_location(
        player,
        TeleportTarget::at(loc.x, loc.y, loc.z)
            .with_instance(0)
            .with_random_offset(offset),
    );
    // `player.setIn7sDungeon(false)` (M8-SS-01).
    world.set_in_7s_dungeon(player, false);
}
