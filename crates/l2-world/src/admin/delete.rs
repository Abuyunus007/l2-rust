//! `//delete [range]` and `//delete_group`: the NPC of the target (or every NPC in a range) is
//! deleted and its spawn stops respawning; for a territory spawn the whole territory goes.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminDelete.java
//
// # Left out, with the system that brings it
// - `SpawnData.deleteSpawn` (and `RaidBossSpawnManager.deleteSpawn`) remove the spawn from the
//   datapack's xml. The spawn list is read-only here: the spawn stops respawning until the server
//   stops, and it comes back at the next start (the same follow-up as `//spawn`).

use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, is_numeric, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminDelete.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_delete", admin_delete), ("admin_delete_group", admin_delete)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminDelete.onCommand`: a word with `group` deletes a territory group, else
/// `//delete [range]`.
fn admin_delete(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.contains("group") {
        handle_delete_group(world, gm);
        return Ok(());
    }
    let split = java_split_space(command);
    let range = match split.get(1) {
        Some(text) if is_numeric(text) => parse_int(text)?,
        _ => 0,
    };
    handle_delete(world, gm, range);
    Ok(())
}

/// Java `handleDelete(player, range)`: every NPC within `range` when it is positive, else the
/// target NPC.
fn handle_delete(world: &mut World, gm: ObjectId, range: i32) {
    if range > 0 {
        let npcs: Vec<ObjectId> = world
            .visible_objects_in_range(gm, range)
            .into_iter()
            .filter(|id| world.npcs.contains_key(id))
            .collect();
        for npc in npcs {
            delete_npc(world, gm, npc);
        }
        return;
    }
    match world.target_of(gm).filter(|t| world.npcs.contains_key(t)) {
        Some(npc) => delete_npc(world, gm, npc),
        None => world.send_sys_message(gm, "Incorrect target."),
    }
}

/// Java `handleDeleteGroup(player)`.
fn handle_delete_group(world: &mut World, gm: ObjectId) {
    match world.target_of(gm).filter(|t| world.npcs.contains_key(t)) {
        Some(npc) => delete_group(world, gm, npc),
        None => world.send_sys_message(gm, "Incorrect target."),
    }
}

/// The spawn index of an NPC, with its territory name (`None` when it is not a territory spawn).
fn spawn_of(world: &World, npc: ObjectId) -> Option<(usize, Option<String>)> {
    let index = world.npcs.get(&npc)?.spawn?;
    let territory = world
        .spawn_def(index)
        .and_then(|s| s.territory.clone());
    Some((index, territory))
}

/// Java `AdminDelete.deleteNpc(player, target)`: a territory NPC is handed to `//delete_group`
/// with the confirmation, another NPC is deleted and its spawn stops respawning.
fn delete_npc(world: &mut World, gm: ObjectId, npc: ObjectId) {
    let Some((index, territory)) = spawn_of(world, npc) else {
        return;
    };
    if territory.is_some() {
        world.admin_command(gm, "admin_delete_group", true);
        return;
    }
    let name = world.object_name(npc).unwrap_or_default();
    world.stop_respawn(index);
    world.delete_npc(npc);
    world.send_sys_message(gm, &format!("Deleted {name} from {}.", npc.0));
}

/// Java `AdminDelete.deleteGroup(player, target)`: the target and every NPC of its territory.
fn delete_group(world: &mut World, gm: ObjectId, npc: ObjectId) {
    let Some((index, territory)) = spawn_of(world, npc) else {
        return;
    };
    let Some(territory) = territory else {
        world.send_sys_message(gm, "Incorrect target.");
        return;
    };
    let name = world.object_name(npc).unwrap_or_default();
    world.delete_npc(npc);
    world.stop_respawn(index);
    // Every NPC of the same territory (the visible ones, as Java's `World.getVisibleObjects`).
    let others: Vec<(ObjectId, usize)> = world
        .npc_ids()
        .into_iter()
        .filter_map(|id| {
            let (other_index, other_territory) = spawn_of(world, id)?;
            (other_territory.as_deref() == Some(territory.as_str())).then_some((id, other_index))
        })
        .collect();
    for (other, other_index) in others {
        world.delete_npc(other);
        world.stop_respawn(other_index);
    }
    world.send_sys_message(gm, &format!("Deleted {name} group from {}.", npc.0));
}
