//! The spawn admin commands that go with `//spawn`, `//scan` and `//delete`: the day and night
//! switch (`//spawnday`, `//spawnnight`), `//circlespawn`, `//topspawncount`, the NPC index pages
//! (`//show_npcs`, `//npc_index`, `//spawn_index`), `//unspawnall`, and the debug print
//! (`//spawn_debug_menu`, `//spawn_debug_print`, `//spawn_debug_print_menu`).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSpawn.java
//
// `//respawnall` and `//spawn_reload` delete every NPC and read the spawn files again: the game
// server task reads them (`ControlRequest::ReloadSpawns`, D-15) and the world installs the new
// spawn list when `WorldCommand::SpawnsReloaded` comes back, then spawns it like at the start.
//
// # Left out, with the system that brings it
// - `//respawnall`, `//spawn_reload` also unload and reload the scripts (D-16: a restart), and
//   run `AutoSpawnHandler`, `RaidBossSpawnManager` and `SevenSigns.spawnSevenSignsNPC` (M8).
// - `//instance_spawns`: in `admin/instance.rs` (M8-INST-01).
// - Spawn persistence: `SpawnData.deleteSpawn` (`//recall_npc` and the delete commands) does not
//   take the spawn out of the xml; `addNewSpawn` is done (`World::store_new_spawn`).
// - The NPC index and the monster index list the templates in id order; Java lists them in the
//   order of its concurrent hash map.
// - `//unspawnall` unloads and reloads the scripts (M8) and clears the spawn groups, which are
//   not in the world; the NPCs go and their spawns stop respawning.

use l2_util::ObjectId;
use tracing::info;

use super::html::AdminHtml;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::outbox::ControlRequest;
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminSpawn.ADMIN_COMMANDS` (the words of this file).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_spawnday", admin_spawnday),
        ("admin_spawnnight", admin_spawnnight),
        ("admin_show_npcs", admin_show_npcs),
        ("admin_npc_index", admin_npc_index),
        ("admin_spawn_index", admin_spawn_index),
        ("admin_circlespawn", admin_circlespawn),
        ("admin_topspawncount", admin_topspawncount),
        ("admin_top_spawn_count", admin_topspawncount),
        ("admin_unspawnall", admin_unspawnall),
        ("admin_respawnall", admin_respawnall),
        ("admin_spawn_reload", admin_respawnall),
        ("admin_spawn_misc", admin_spawn_misc),
        ("admin_spawnmisc", admin_spawn_misc),
        ("admin_list_pools", admin_list_pools),
        ("admin_spawn_debug_menu", admin_spawn_debug_menu),
        ("admin_spawn_debug_print", admin_spawn_debug_print),
        ("admin_spawn_debug_print_menu", admin_spawn_debug_print),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminHtml.showAdminHtml(player, path)` with the page of this file.
fn show(world: &mut World, gm: ObjectId, path: &str) {
    world.show_admin_html(gm, path);
}

/// Java `new NpcHtmlMessage(html)`: the page with no file (`setHtml`).
fn send_raw_html(world: &mut World, gm: ObjectId, html: &str) {
    let page = AdminHtml::from_html(html);
    world.send_admin_html(gm, &page);
}

/// Java `spawnDayCreatures()`: the night creatures go, the day creatures come.
fn admin_spawnday(world: &mut World, _gm: ObjectId, _command: &str) -> Outcome {
    world.set_night(false);
    Ok(())
}

/// Java `spawnNightCreatures()`: the day creatures go, the night creatures come.
fn admin_spawnnight(world: &mut World, _gm: ObjectId, _command: &str) -> Outcome {
    world.set_night(true);
    Ok(())
}

/// Java `admin_show_npcs`: `npcs.htm`.
fn admin_show_npcs(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    show(world, gm, "npcs.htm");
    Ok(())
}

/// The page size of the monster and NPC lists (Java's `i < mobsCount && j < 50`).
const LIST_PAGE: usize = 50;

/// Java `showNpcs(player, starting, from)`: the Folk NPCs whose name starts with `starting`.
fn show_npcs(world: &mut World, gm: ObjectId, starting: &str, from: usize) {
    let mobs: Vec<(i32, String)> = world
        .npc_world
        .templates_arc()
        .map(|templates| {
            templates
                .iter()
                .filter(|t| t.npc_type == "Folk" && t.name.starts_with(starting))
                .map(|t| (t.id, t.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    send_list(
        world,
        gm,
        &format!(
            "<html><title>Spawn Monster:</title><body><p> There are {} Npcs whose name starts with {starting}:<br>",
            mobs.len()
        ),
        &mobs,
        from,
        "<br><center><button value=\"Back\" action=\"bypass -h admin_show_npcs\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>",
        |next| {
            format!(
                "<br><center><button value=\"Next\" action=\"bypass -h admin_npc_index {starting} {next}\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"><button value=\"Back\" action=\"bypass -h admin_show_npcs\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>"
            )
        },
    );
}

/// Java `showMonsters(player, level, from)`: the Monster NPCs of a level.
fn show_monsters(world: &mut World, gm: ObjectId, level: i32, from: usize) {
    let mobs: Vec<(i32, String)> = world
        .npc_world
        .templates_arc()
        .map(|templates| {
            templates
                .iter()
                .filter(|t| i32::from(t.level) == level && t.npc_type == "Monster")
                .map(|t| (t.id, t.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    send_list(
        world,
        gm,
        &format!(
            "<html><title>Spawn Monster:</title><body><p> Level : {level}<br>Total NPCs : {}<br>",
            mobs.len()
        ),
        &mobs,
        from,
        "<br><center><button value=\"Back\" action=\"bypass -h admin_show_spawns\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>",
        |next| {
            format!(
                "<br><center><button value=\"Next\" action=\"bypass -h admin_spawn_index {level} {next}\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"><button value=\"Back\" action=\"bypass -h admin_show_spawns\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>"
            )
        },
    );
}

/// The loop of `showNpcs` and `showMonsters`: up to 50 links from `from`, then Back, or Next with
/// the index where the loop stopped.
fn send_list(
    world: &mut World,
    gm: ObjectId,
    head: &str,
    mobs: &[(i32, String)],
    from: usize,
    last: &str,
    next: impl Fn(usize) -> String,
) {
    let mut html = String::from(head);
    let mut i = from;
    let mut shown = 0;
    while i < mobs.len() && shown < LIST_PAGE {
        let (id, name) = &mobs[i];
        html.push_str(&format!(
            "<a action=\"bypass -h admin_spawn_monster {id}\">{name}</a><br1>"
        ));
        i += 1;
        shown += 1;
    }
    if i == mobs.len() {
        html.push_str(last);
    } else {
        html.push_str(&next(i));
    }
    send_raw_html(world, gm, &html);
}

/// Java `admin_npc_index <letter> [from]`: a bad number or no letter shows `npcs.htm`.
fn admin_npc_index(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let Some(letter) = st.next_token() else {
        show(world, gm, "npcs.htm");
        return Ok(());
    };
    let from = match st.next_token() {
        Some(value) => match parse_int(value) {
            Ok(from) => from,
            Err(_) => {
                show(world, gm, "npcs.htm");
                return Ok(());
            }
        },
        None => 0,
    };
    show_npcs(world, gm, letter, usize::try_from(from).unwrap_or(0));
    Ok(())
}

/// Java `admin_spawn_index <level> [from]`: a bad number or no level shows `spawns.htm`.
fn admin_spawn_index(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let level = st.next_token().and_then(|v| parse_int(v).ok());
    let from = match st.next_token() {
        Some(value) => parse_int(value).ok(),
        None => Some(0),
    };
    match (level, from) {
        (Some(level), Some(from)) => {
            show_monsters(world, gm, level, usize::try_from(from).unwrap_or(0));
        }
        _ => show(world, gm, "spawns.htm"),
    }
    Ok(())
}

/// Java `admin_circlespawn <npcId|name> <count> <radius> [respawn] [permanent]`: `count` NPCs on
/// a circle of `radius` around the GM. Any bad word is the usage line.
fn admin_circlespawn(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let usage = "Us: //circlespawn <npcId|name> <quantity> <radio> [respawn] [permanent]";
    let parsed = (|| -> Option<(String, i32, i32, i32, bool)> {
        let mut st = Tokens::spaces(command);
        st.next_token()?;
        let npc = st.next_token()?.to_owned();
        let count = parse_int(st.next_token()?).ok()?;
        let radius = parse_int(st.next_token()?).ok()?;
        let respawn = match st.next_token() {
            Some(value) => parse_int(value).ok()?,
            None => 60,
        };
        // `Boolean.parseBoolean`: true only for "true", ignoring the case.
        let permanent = match st.next_token() {
            Some(value) => value.eq_ignore_ascii_case("true"),
            None => true,
        };
        Some((npc, count, radius, respawn, permanent))
    })();
    let Some((npc, count, radius, respawn, permanent)) = parsed else {
        world.send_message(gm, usage);
        return Ok(());
    };
    let template = if npc.bytes().all(|b| b.is_ascii_digit()) && !npc.is_empty() {
        parse_int(&npc)
            .ok()
            .and_then(|id| world.npc_world.template(id).map(|t| (t.id, t.name.clone())))
    } else {
        world
            .npc_world
            .template_by_name(&npc.replace('_', " "))
            .map(|t| (t.id, t.name.clone()))
    };
    let Some((template_id, name)) = template else {
        world.send_message(gm, "NPC not found.");
        return Ok(());
    };
    let Some((cx, cy, cz)) = world.position_of(gm) else {
        return Ok(());
    };
    let heading = world.players.get(&gm).map_or(0, |p| p.core.heading);
    for i in 0..count.max(0) {
        // `(2 * Math.PI * i) / count` and the `(int)` casts (truncation toward zero).
        let angle = (2.0 * std::f64::consts::PI * f64::from(i)) / f64::from(count);
        let x = cx + (f64::from(radius) * angle.cos()) as i32;
        let y = cy + (f64::from(radius) * angle.sin()) as i32;
        // `SpawnData.addNewSpawn` when permanent; `stopRespawn()` when the spawn is not permanent or
        // has no delay.
        if permanent {
            world.store_new_spawn(template_id, (x, y, cz), heading, 1, respawn);
        }
        world.admin_spawn(
            template_id,
            (x, y, cz),
            heading,
            1,
            respawn,
            permanent && respawn > 0,
        );
    }
    world.send_message(gm, &format!("Spawn circle created: {count}{name}"));
    Ok(())
}

/// Java `admin_topspawncount [count]`: the NPC templates with the most NPCs in the world.
fn admin_topspawncount(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let mut count: i64 = 5;
    if let Some(value) = st.next_token() {
        if value.bytes().all(|b| b.is_ascii_digit()) && !value.is_empty() {
            count = value.parse::<i64>().unwrap_or(i64::MAX);
        }
        if count <= 0 {
            return Ok(());
        }
    }
    let mut found: std::collections::BTreeMap<i32, i64> = std::collections::BTreeMap::new();
    for id in world.npc_ids() {
        if let Some(npc) = world.npc(id) {
            *found.entry(npc.npc_id).or_insert(0) += 1;
        }
    }
    let mut ranked: Vec<(i32, i64)> = found.into_iter().collect();
    // `MapUtil.sortByValue(map, true)`: the most first; equal counts by id here.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    world.send_sys_message(gm, &format!("Top {count} spawn count."));
    let mut left = count;
    for (npc_id, n) in ranked {
        left -= 1;
        if left < 0 {
            break;
        }
        let name = world
            .npc_world
            .template(npc_id)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        world.send_sys_message(gm, &format!("{name} ({npc_id}): {n}"));
    }
    Ok(())
}

/// Java `admin_unspawnall`: everyone is told, every NPC goes and no spawn comes back.
fn admin_unspawnall(world: &mut World, _gm: ObjectId, _command: &str) -> Outcome {
    let notice = l2_protocol_game::server::SystemMessage::new(
        l2_protocol_game::SystemMessageId::TheNpcServerIsNotOperatingAtThisTime,
    );
    world.broadcast_to_all_online_players(&notice);
    // `RaidBossSpawnManager.getInstance().cleanUp()` (M8-BOSS-01).
    world.raid_clean_up();
    // Every spawn stops respawning first (the delete loop of Java fires the death hooks).
    for index in world.all_spawn_indices() {
        world.stop_respawn(index);
    }
    for id in world.npc_ids() {
        world.delete_npc(id);
    }
    world.broadcast_message_to_gms("NPC unspawn completed!");
    Ok(())
}

/// Java `admin_respawnall` and `admin_spawn_reload`: the spawn groups stop, every NPC and spawn
/// is deleted, the spawn files are read again and spawned; the GMs hear "NPC respawn completed!"
/// when that is done (see `World::on_spawns_reloaded`).
fn admin_respawnall(world: &mut World, _gm: ObjectId, _command: &str) -> Outcome {
    world.unspawn_all_spawns();
    world.outbox.control(ControlRequest::ReloadSpawns);
    Ok(())
}

/// Java `admin_spawn_misc` and `admin_spawnmisc` (exactly these words): every NPC template gets one
/// immobile NPC without core AI in the GM misc room, 200 units apart, and the GM goes there.
// NOTE(java-quirk): the room is spawned at the same place for every call; a second call adds a
// second set of NPCs. Templates whose display id is another id, grand bosses and fake players are
// skipped. A minion a monster brought along is `decayMe`d in Java (hidden, still existing); here
// it is deleted.
fn admin_spawn_misc(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command != "admin_spawn_misc" && command != "admin_spawnmisc" {
        return Ok(());
    }
    const NPC_COUNT: i32 = 100_000;
    const START_X: i32 = -128_670;
    const START_Y: i32 = -230_115;
    const SPAWN_Z: i32 = -15_536;
    const END_X: i32 = -100_432;
    const DISTANCE: i32 = 200;

    let name = world.object_name(gm).unwrap_or_default();
    info!("{name} is spawning GM misc room...");
    world.send_sys_message(
        gm,
        "WARNING: There are several known issues regarding this feature. Spawning all NPCs during runtime is STRONGLY NOT RECOMMENDED for live servers, just for developing environments.",
    );

    let (mut current_x, mut current_y) = (START_X, START_Y);
    for id in 1..=NPC_COUNT {
        let Some(template) = world.npc_world.template(id) else {
            continue;
        };
        if template.id != template.display_id || template.is_type("GrandBoss") || template.fake_player {
            continue;
        }
        // `spawn.setRespawnDelay(60)`, `init()` and `stopRespawn()`: not permanent here.
        let created = world.admin_spawn(id, (current_x, current_y, SPAWN_Z), 0, 1, 60, false);
        if let Some(&npc) = created.last() {
            world.set_immobilized(npc, true);
            world.disable_core_ai(npc, true);
            world.broadcast_info(npc);
            for minion in world.minions_of(npc).to_vec() {
                world.delete_npc(minion);
            }
        }
        current_x += DISTANCE;
        if current_x > END_X {
            current_x = START_X;
            // Move to the next line.
            current_y -= DISTANCE;
        }
    }

    // Teleport player at GM Misc.
    world.tele_to_location(gm, TeleportTarget::at(current_x, current_y, SPAWN_Z));
    info!(
        "{name} spawned all NPCs at GM misc room.\nWARNING: There are several known issues regarding this feature. Spawning all NPCs during runtime is STRONGLY NOT RECOMMENDED for live servers, just for developing environments."
    );
    Ok(())
}

/// Java `listPools`: every spawn group with the units alive or reserved, and each of its entries.
fn admin_list_pools(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(spawns) = world.npc_world.spawn_data().cloned() else {
        world.send_sys_message(gm, "There are no spawn groups.");
        return Ok(());
    };
    if spawns.groups().is_empty() {
        world.send_sys_message(gm, "There are no spawn groups.");
        return Ok(());
    }
    let mut lines = Vec::new();
    for (index, group) in spawns.groups().iter().enumerate() {
        let state = world.death.groups.get(&index);
        let selection = match group.selection {
            l2_data::spawn::SpawnSelection::All => "ALL",
            l2_data::spawn::SpawnSelection::Random => "RANDOM",
            l2_data::spawn::SpawnSelection::RandomFill => "RANDOM_FILL",
        };
        let cap = if group.maximum_npc > 0 {
            format!(" cap {}", group.maximum_npc)
        } else {
            String::new()
        };
        lines.push(format!(
            "{} [{selection}{cap}] alive {}",
            group.name,
            state.map_or(0, |s| s.alive)
        ));
        for (i, entry) in group.entries.iter().enumerate() {
            // `getSlots()`: the slots Java could build.
            let slots = state.and_then(|s| s.slots.get(i)).unwrap_or(&entry.slots);
            // `slots.get(0)` of an entry with no slot is Java's IndexOutOfBoundsException.
            let Some(slot) = slots.first().and_then(|slot| spawns.spawns().get(*slot)) else {
                for line in lines.drain(..) {
                    world.send_sys_message(gm, &line);
                }
                return Err(JavaException(
                    "java.lang.IndexOutOfBoundsException: Index 0 out of bounds for length 0".to_owned(),
                ));
            };
            let template_name = world
                .npc_world
                .template(slot.npc_id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            lines.push(format!(
                "- {template_name} ({}) {}/{} in {} territories",
                slot.npc_id,
                state.and_then(|s| s.entry_alive.get(i)).copied().unwrap_or(0),
                entry.total,
                slots.len()
            ));
        }
    }
    for line in lines {
        world.send_sys_message(gm, &line);
    }
    Ok(())
}

/// Java `admin_spawn_debug_menu`: `spawns_debug.htm`.
fn admin_spawn_debug_menu(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    show(world, gm, "spawns_debug.htm");
    Ok(())
}

/// Java `admin_spawn_debug_print <type> [_menu]`: the spawn of the target NPC, printed to the log
/// in one of three formats. A target that is not an NPC is the invalid target; anything missing
/// (no type, no spawn) does nothing.
fn admin_spawn_debug_print(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let npc = world
        .target_of(gm)
        .and_then(|target| world.npc(target).map(|n| (n.npc_id, n.spawn)));
    let Some(npc) = npc else {
        world.send(
            gm,
            &l2_protocol_game::server::SystemMessage::new(l2_protocol_game::SystemMessageId::InvalidTarget),
        );
        return Ok(());
    };
    let mut st = Tokens::spaces(command);
    st.next_token();
    let Some(kind) = st.next_token().and_then(|v| parse_int(v).ok()) else {
        return Ok(());
    };
    let Some(index) = npc.1 else {
        return Ok(());
    };
    let Some(spawn) = world.spawn_def(index) else {
        return Ok(());
    };
    let (x, y, z, h) = (
        spawn.location.x,
        spawn.location.y,
        spawn.location.z,
        spawn.heading,
    );
    let id = npc.0;
    match kind {
        1 => info!("<spawn npcId=\"{id}\" x=\"{x}\" y=\"{y}\" z=\"{z}\" heading=\"{h}\" respawn=\"0\" />"),
        2 => info!("{{ {id}, {x}, {y}, {z}, {h} }},"),
        _ => info!("('',1,{id},{x},{y},{z},0,0,{h},60,0,0),"),
    }
    if command.contains("_menu") {
        show(world, gm, "spawns_debug.htm");
    }
    Ok(())
}
