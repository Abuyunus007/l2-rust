//! The core admin commands: `//admin` (the panels), `//spawn`, `//teleport`..., `//goto`,
//! `//heal`. The combat ones (`//kill`, `//res`) are in `combat.rs`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminAdmin,AdminSpawn,
// Java: AdminTeleport,AdminGoto,AdminHeal,AdminHtml}.java
//
// Only the commands the card names are here (plus the few the panels link to and `//goto`
// needs); every other command of those classes answers "The command '...' does not exist!" like
// a command without handler. A handler gets the command text as typed (`admin_spawn 20101`) and
// does what the Java `onCommand` branch does, including its odd corners.
//
// Left out, with the system that brings it:
// - Spawn persistence: `SpawnData.addNewSpawn` writes the spawn into the datapack's xml and
//   `RaidBossSpawnManager` into `raidboss_spawnlist`; here the spawn lives until the server stops
//   (its respawn is ported, `respawn.rs`), but it is not written anywhere (FOLLOW-UP).
// - Instances, effects (`stopAllEffects`), the DB update of `//recall <offline player>`.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::warn;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::objects::ObjectKind;
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `MasterHandler`: the commands of this card.
pub(crate) fn register_core(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_admin", admin_admin),
        ("admin_admin1", admin_admin),
        ("admin_admin2", admin_admin),
        ("admin_admin3", admin_admin),
        ("admin_admin4", admin_admin),
        ("admin_admin5", admin_admin),
        ("admin_admin6", admin_admin),
        ("admin_admin7", admin_admin),
        ("admin_show_spawns", admin_spawn_commands),
        ("admin_spawnat", admin_spawn_commands),
        ("admin_spawn", admin_spawn_commands),
        ("admin_spawn_monster", admin_spawn_commands),
        ("admin_spawn_once", admin_spawn_commands),
        ("admin_list_spawns", admin_list_spawns),
        ("admin_list_positions", admin_list_spawns),
        ("admin_show_moves", admin_teleport),
        ("admin_show_moves_other", admin_teleport),
        ("admin_show_teleport", admin_teleport),
        ("admin_teleport_to_character", admin_teleport),
        ("admin_teleportto", admin_teleport),
        ("admin_move_to", admin_teleport),
        ("admin_teleport_character", admin_teleport),
        ("admin_recall", admin_teleport),
        ("admin_tele", admin_teleport),
        ("admin_gonorth", admin_teleport),
        ("admin_gosouth", admin_teleport),
        ("admin_goeast", admin_teleport),
        ("admin_gowest", admin_teleport),
        ("admin_goup", admin_teleport),
        ("admin_godown", admin_teleport),
        ("admin_goto", admin_goto),
        ("admin_heal", admin_heal),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `String.split(" ")`: trailing empty strings are dropped.
pub(crate) fn java_split_space(text: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = text.split(' ').collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// Java `String.trim()`: every character up to U+0020.
fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

impl World {
    fn send_system_message(&self, to: ObjectId, id: SystemMessageId) {
        self.send(to, &SystemMessage::new(id));
    }

    fn invalid_target(&self, to: ObjectId) {
        self.send_system_message(to, SystemMessageId::InvalidTarget);
    }

    fn creature_position(&self, id: ObjectId) -> Option<(i32, i32, i32)> {
        self.position_of(id)
    }

    fn is_creature_kind(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|e| e.kind.is_creature())
    }

    fn max_teleport_offset(&self) -> i32 {
        self.settings.teleport.max_offset_on_teleport
    }
}

// ------------------------------------------------------------------------------ AdminAdmin

/// Java `AdminAdmin.onCommand` -> `showMainPage`: `//admin`, `//admin1` ... `//admin7`.
fn admin_admin(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `Integer.parseInt(command.substring(11))`; anything else is the main page.
    let mode = command.get(11..).and_then(|v| parse_int(v).ok()).unwrap_or(0);
    let name = match mode {
        2 => "game",
        3 => "effects",
        4 => "server",
        5 => "mods",
        6 => "char",
        7 => "gm",
        _ => "main",
    };
    world.show_admin_html(gm, &format!("{name}_menu.htm"));
    Ok(())
}

// ------------------------------------------------------------------------------ AdminSpawn

/// Java `AdminSpawn.onCommand` for `show_spawns`, `spawnat`, `spawn`, `spawn_monster` and
/// `spawn_once`.
fn admin_spawn_commands(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_show_spawns" {
        world.show_admin_html(gm, "spawns.htm");
    } else if command.starts_with("admin_spawnat") {
        // Case of wrong or missing monster data: the spawn panel.
        if spawn_at(world, gm, command).is_err() {
            world.show_admin_html(gm, "spawns.htm");
        }
    } else if (command.starts_with("admin_spawn_monster") || command.starts_with("admin_spawn"))
        && spawn_monster_command(world, gm, command).is_err()
    {
        world.show_admin_html(gm, "spawns.htm");
    }
    Ok(())
}

/// The `try` block of the `admin_spawn*` branch: `Err` is any exception Java catches there.
fn spawn_monster_command(world: &mut World, gm: ObjectId, command: &str) -> Result<(), ()> {
    let mut st = Tokens::spaces(command);
    let cmd = st.next_token().ok_or(())?;
    let mut npc_id = st.next_token().ok_or(())?.to_owned();

    // The second token is not a number: look for an NPC name made of the words that follow.
    if !is_numeric(&npc_id) {
        let params = java_split_space(command);
        let mut search = String::new();
        let mut found: Option<i32> = None;
        let mut pos = 1;
        for (i, param) in params.iter().enumerate().skip(1) {
            search.push_str(param);
            search.push(' ');
            if let Some(template) = world.npc_world.template_by_name(java_trim(&search)) {
                found = Some(template.id);
                pos = i;
            }
        }
        if let Some(template_id) = found {
            // Skip the tokens that contain the name.
            for _ in 1..pos {
                st.next_token().ok_or(())?;
            }
            npc_id = template_id.to_string();
        }
    }

    let mut mob_count = 1;
    if st.has_more() {
        mob_count = parse_int(st.next_token().ok_or(())?).map_err(|_| ())?;
    }
    let mut respawn_time = 60;
    if st.has_more() {
        respawn_time = parse_int(st.next_token().ok_or(())?).map_err(|_| ())?;
    }
    let permanent = !cmd.eq_ignore_ascii_case("admin_spawn_once");
    spawn_monster(world, gm, &npc_id, respawn_time, mob_count, permanent)
}

/// Java `AdminSpawn.spawnMonster(activeChar, monsterIdValue, respawnTime, mobCount, permanent)`.
fn spawn_monster(
    world: &mut World,
    gm: ObjectId,
    monster_id: &str,
    respawn_time: i32,
    mob_count: i32,
    permanent: bool,
) -> Result<(), ()> {
    let target = world.target_of(gm).unwrap_or(gm);
    let template = if is_numeric(monster_id) {
        let id = parse_int(monster_id).map_err(|_| ())?;
        world.npc_world.template(id)
    } else {
        // First parameter wasn't just numbers so go by name not ID.
        world.npc_world.template_by_name(&monster_id.replace('_', " "))
    };
    // A template that does not exist makes `template.isFakePlayer()` throw: the panel again.
    let template = template.ok_or(())?;
    let (template_id, name, fake_player) = (template.id, template.name.clone(), template.fake_player);
    // A raid boss goes through `RaidBossSpawnManager` (M8-BOSS-01).
    let is_raid = template.npc_type == "RaidBoss";
    let (base_hp_max, base_mp_max) = (f64::from(template.base_hp_max), f64::from(template.base_mp_max));

    // `FakePlayersConfig.FAKE_PLAYERS_ENABLED` is off (fake players are not ported).
    if fake_player {
        world.send_system_message(gm, SystemMessageId::YourTargetCannotBeFound);
        return Ok(());
    }

    let (x, y, z) = world
        .creature_position(target)
        .or_else(|| world.creature_position(gm))
        .ok_or(())?;
    let heading = world.players.get(&gm).map_or(0, |p| p.core.heading);
    // `// TODO add checks for GrandBossSpawnManager`: a raid boss the manager holds is not spawned
    // a second time.
    if world.raid_boss_defined(template_id) {
        world.send_sys_message(gm, &format!("You cannot spawn another instance of {name}."));
        return Ok(());
    }
    if is_raid {
        world.admin_spawn_raid_boss(
            template_id,
            (x, y, z),
            heading,
            mob_count,
            (base_hp_max, base_mp_max),
            permanent,
            respawn_time,
        );
        world.send_sys_message(gm, &format!("Created {name} on {}", target.0));
        return Ok(());
    }
    // Java: `new Spawn`, `setAmount(mobCount)`, `setRespawnDelay(respawnTime)`, `init()`, and
    // `stopRespawn()` when the spawn is not permanent or has no delay.
    let created = world.admin_spawn(template_id, (x, y, z), heading, mob_count, respawn_time, permanent);
    if i64::try_from(created.len()).unwrap_or(i64::MAX) < i64::from(mob_count) {
        world.send_system_message(gm, SystemMessageId::YourTargetCannotBeFound);
        return Ok(());
    }
    world.send_sys_message(gm, &format!("Created {name} on {}", target.0));
    Ok(())
}

/// The `admin_spawnat` branch: `//spawnat <npcId> <x> <y> <z> [heading]`.
fn spawn_at(world: &mut World, gm: ObjectId, command: &str) -> Result<(), ()> {
    let mut st = Tokens::spaces(command);
    st.next_token().ok_or(())?;
    let id = st.next_token().ok_or(())?;
    let x = st.next_token().ok_or(())?;
    let y = st.next_token().ok_or(())?;
    let z = st.next_token().ok_or(())?;
    let mut heading = world.players.get(&gm).map_or(0, |p| p.core.heading);
    if st.has_more() {
        heading = parse_int(st.next_token().ok_or(())?).map_err(|_| ())?;
    }
    let id = parse_int(id).map_err(|_| ())?;
    let x = parse_int(x).map_err(|_| ())?;
    let y = parse_int(y).map_err(|_| ())?;
    let z = parse_int(z).map_err(|_| ())?;

    let target = world.target_of(gm).unwrap_or(gm);
    let template = world.npc_world.template(id).ok_or(())?;
    let (name, fake_player) = (template.name.clone(), template.fake_player);
    if fake_player {
        world.send_system_message(gm, SystemMessageId::YourTargetCannotBeFound);
        return Ok(());
    }
    // Java `spawnMonster(activeChar, id, x, y, z, h)`: `setAmount(1)`, `setRespawnDelay(60)`,
    // `SpawnData.addNewSpawn` (permanent: FOLLOW-UP(M4), the datapack is not written), `init()`.
    // The NPC comes back 60 seconds after it dies.
    match world
        .admin_spawn(id, (x, y, z), heading, 1, 60, true)
        .last()
        .copied()
    {
        Some(npc) => {
            world.broadcast_info(npc);
            world.send_sys_message(gm, &format!("Created {name} on {}", target.0));
        }
        None => world.send_system_message(gm, SystemMessageId::YourTargetCannotBeFound),
    }
    Ok(())
}

/// Java `AdminSpawn.onCommand` for `list_spawns` and `list_positions`.
fn admin_list_spawns(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut npc_id = 0;
    let mut teleport_index = -1;
    // The `try` block: an exception keeps what was assigned before it.
    let _ = (|| -> Result<(), ()> {
        let params = java_split_space(command);
        // The last word is a teleport index only when a search parameter precedes it.
        let has_index = params.len() > 2 && params.last().is_some_and(|p| is_numeric(p));
        let search_end = if has_index { params.len() - 1 } else { params.len() };
        let mut search = String::new();
        for param in params.iter().take(search_end).skip(1) {
            search.push_str(param);
            search.push(' ');
        }
        let search = java_trim(&search);
        if is_numeric(search) {
            npc_id = parse_int(search).map_err(|_| ())?;
        } else {
            // `getTemplateByName(..).getId()` throws for an unknown name.
            npc_id = world.npc_world.template_by_name(search).ok_or(())?.id;
        }
        if has_index {
            teleport_index = parse_int(params.last().ok_or(())?).map_err(|_| ())?;
        }
        Ok(())
    })();
    find_npcs(world, gm, npc_id, teleport_index);
    Ok(())
}

/// Java `AdminSpawn.findNpcs` (for `list_positions`; `list_spawns` prints the spawn's own place,
/// which is the NPC's place until NPCs move in M4). A "spawn" is a spawn-list entry with its live NPC; NPCs made by
/// admin commands have no entry and count one by one (Java keeps those in `SpawnTable` too).
fn find_npcs(world: &mut World, gm: ObjectId, npc_id: i32, teleport_index: i32) {
    // (spawn index, object id): the last NPC of a spawn is its `getLastSpawn()`.
    let mut spawns: Vec<(Option<usize>, ObjectId)> = Vec::new();
    for id in world.npc_ids() {
        let Some(npc) = world.npcs.get(&id) else {
            continue;
        };
        if npc.npc_id != npc_id || npc.leader.is_some() {
            continue;
        }
        match npc.spawn {
            Some(index) => match spawns.iter_mut().find(|(s, _)| *s == Some(index)) {
                Some(entry) => entry.1 = id,
                None => spawns.push((Some(index), id)),
            },
            None => spawns.push((None, id)),
        }
    }
    spawns.sort_by_key(|(spawn, id)| (spawn.is_none(), *spawn, id.0));

    let mut index = 0;
    for (_, npc) in &spawns {
        index += 1;
        let Some(core) = world.npcs.get(npc).map(|n| n.core.clone()) else {
            continue;
        };
        let instance = world.objects.get(npc).map_or(0, |e| e.instance_id);
        if teleport_index > -1 {
            if teleport_index == index {
                let offset = world.max_teleport_offset();
                world.tele_to_location(
                    gm,
                    TeleportTarget::at(core.x, core.y, core.z)
                        .with_heading(core.heading)
                        .with_instance(instance)
                        .with_random_offset(offset),
                );
                world.send_sys_message(gm, "You have been teleported.");
            }
        } else {
            let name = world.object_name(*npc).unwrap_or_default();
            // NOTE: `getPoolInfo` (spawn groups) and the spawn's own place are not modelled; NPCs
            // do not move before M4, so the NPC's place is the spawn's.
            world.send_message(
                gm,
                &format!(
                    "{index} - {name} (Spawn ID: {npc_id} X: {} Y: {} Z: {} Heading: {}): {} {} {}",
                    core.x, core.y, core.z, core.heading, core.x, core.y, core.z
                ),
            );
        }
    }

    if index == 0 {
        // `World.getNpc(npcId)`: the first NPC with this template.
        let found = world.npc_ids().into_iter().find(|id| {
            world
                .npcs
                .get(id)
                .is_some_and(|n| n.npc_id == npc_id && npc_id != 0)
        });
        if let Some(npc) = found
            && let Some(core) = world.npcs.get(&npc).map(|n| n.core.clone())
        {
            let instance = world.objects.get(&npc).map_or(0, |e| e.instance_id);
            let offset = world.max_teleport_offset();
            world.tele_to_location(
                gm,
                TeleportTarget::at(core.x, core.y, core.z)
                    .with_heading(core.heading)
                    .with_instance(instance)
                    .with_random_offset(offset),
            );
            world.send_message(gm, "The current spawn is not stored.");
        } else {
            world.send_message(gm, "The specified teleport target does not exist.");
        }
    }
}

// ---------------------------------------------------------------------------- AdminTeleport

/// Java `AdminTeleport.onCommand` for the commands of this card.
fn admin_teleport(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_show_moves" {
        world.show_admin_html(gm, "teleports.htm");
    }

    if command == "admin_show_moves_other" {
        world.show_admin_html(gm, "teleports/OtherLocations.htm");
    } else if command == "admin_show_teleport" {
        show_teleport_char_window(world, gm);
    } else if command == "admin_teleport_to_character" {
        let target = world.target_of(gm);
        teleport_to_character(world, gm, target);
    } else if command.starts_with("admin_move_to") {
        match command.get("admin_move_to".len() + 1..) {
            // Case of empty or missing coordinates.
            None => world.show_admin_html(gm, "teleports.htm"),
            Some(coords) => {
                if teleport_to(world, gm, coords).is_err() {
                    world.send_sys_message(gm, "Usage: //move_to <x> <y> <z>");
                    world.show_admin_html(gm, "teleports.htm");
                }
            }
        }
    } else if command.starts_with("admin_teleport_character") {
        match command.get("admin_teleport_character".len() + 1..) {
            Some(coords) => teleport_character_to_coords(world, gm, coords)?,
            // Case of empty coordinates.
            None => {
                world.send_sys_message(gm, "Wrong or no Coordinates given.");
                show_teleport_char_window(world, gm);
            }
        }
    } else if command.starts_with("admin_teleportto ") {
        let name = command.get("admin_teleportto ".len()..).unwrap_or("");
        let player = world.player_by_name(name);
        teleport_to_character(world, gm, player);
    } else if command.starts_with("admin_recall ") {
        let params = java_split_space(command);
        if params.len() != 2 {
            world.send_sys_message(gm, "Usage: //recall <playername>");
            return Ok(());
        }
        let target_name = params[1];
        match world.player_by_name(target_name) {
            Some(player) => {
                let place = world
                    .players
                    .get(&gm)
                    .map(|p| (p.core.x, p.core.y, p.core.z, p.core.heading));
                let instance = world.objects.get(&gm).map_or(0, |e| e.instance_id);
                if let Some((x, y, z, heading)) = place {
                    recall_player(
                        world,
                        player,
                        TeleportTarget::at(x, y, z)
                            .with_heading(heading)
                            .with_instance(instance),
                        Some(gm),
                    );
                }
            }
            None => change_character_position(world, gm, target_name),
        }
    } else if command == "admin_tele" {
        world.show_admin_html(gm, "move.htm");
    } else if command.starts_with("admin_go") {
        go_direction(world, gm, command);
    }
    Ok(())
}

/// `AdminTeleport.teleportTo(activeChar, coords)`. `Err` is the `NumberFormatException` the
/// caller reports; missing numbers are answered here.
fn teleport_to(world: &mut World, gm: ObjectId, coords: &str) -> Result<(), JavaException> {
    let mut st = Tokens::new(coords);
    let mut values = [0; 3];
    for value in &mut values {
        match st.next_token() {
            Some(token) => *value = parse_int(token)?,
            None => {
                world.send_sys_message(gm, "Wrong or no Coordinates given.");
                return Ok(());
            }
        }
    }
    world.set_intention_idle(gm);
    world.tele_to_location(gm, TeleportTarget::at(values[0], values[1], values[2]));
    world.send_sys_message(gm, &format!("You have been teleported to {coords}"));
    Ok(())
}

/// `AdminTeleport.showTeleportCharWindow`.
fn show_teleport_char_window(world: &mut World, gm: ObjectId) {
    let Some(target) = world.target_of(gm).filter(|t| world.players.contains_key(t)) else {
        world.invalid_target(gm);
        return;
    };
    let name = world
        .players
        .get(&target)
        .map(|p| p.data.name.clone())
        .unwrap_or_default();
    let (x, y, z) = world.creature_position(gm).unwrap_or_default();
    let html = format!(
        "<html><title>Teleport Character</title><body>The character you will teleport is {name}.<br>Co-ordinate x<edit var=\"char_cord_x\" width=110>Co-ordinate y<edit var=\"char_cord_y\" width=110>Co-ordinate z<edit var=\"char_cord_z\" width=110><button value=\"Teleport\" action=\"bypass -h admin_teleport_character $char_cord_x $char_cord_y $char_cord_z\" width=60 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"><button value=\"Teleport near you\" action=\"bypass -h admin_teleport_character {x} {y} {z}\" width=115 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"><center><button value=\"Back\" action=\"bypass -h admin_current_player\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>"
    );
    if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(0, &html) {
        world.send_html(gm, &message);
    }
}

/// `AdminTeleport.teleportCharacter(activeChar, coords)`: the targeted player goes to the
/// coordinates. A bad number is Java's uncaught `NumberFormatException`.
fn teleport_character_to_coords(world: &mut World, gm: ObjectId, coords: &str) -> Outcome {
    let Some(player) = world.target_of(gm).filter(|t| world.players.contains_key(t)) else {
        world.invalid_target(gm);
        return Ok(());
    };
    if player == gm {
        world.send_system_message(player, SystemMessageId::YouCannotUseThisOnYourself);
        return Ok(());
    }
    let mut st = Tokens::new(coords);
    let mut values = [0; 3];
    for value in &mut values {
        match st.next_token() {
            Some(token) => *value = parse_int(token)?,
            // NoSuchElementException: "Not important."
            None => return Ok(()),
        }
    }
    // `new Location(x, y, z)`: heading 0, instance 0.
    recall_player(
        world,
        player,
        TeleportTarget::at(values[0], values[1], values[2]).with_instance(0),
        None,
    );
    Ok(())
}

/// `AdminTeleport.teleportCharacter(player, loc, activeChar)`.
fn recall_player(world: &mut World, player: ObjectId, place: TeleportTarget, gm: Option<ObjectId>) {
    // Check for jail.
    if world.is_jailed(player) {
        let name = world
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if let Some(gm) = gm {
            world.send_sys_message(gm, &format!("Sorry, player {name} is in Jail."));
        }
        return;
    }
    if let Some(gm) = gm {
        // `player.setInstanceId(activeChar.getInstanceId())` is part of the teleport target.
        let name = world
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        world.send_sys_message(gm, &format!("You have recalled {name}"));
    }
    world.send_message(player, "Admin is teleporting you.");
    world.set_intention_idle(player);
    let offset = world.max_teleport_offset();
    world.tele_to_location(player, place.with_random_offset(offset));
}

/// `AdminTeleport.teleportToCharacter`.
fn teleport_to_character(world: &mut World, gm: ObjectId, target: Option<ObjectId>) {
    let Some(player) = target.filter(|t| world.players.contains_key(t)) else {
        world.invalid_target(gm);
        return;
    };
    if player == gm {
        world.send_system_message(player, SystemMessageId::YouCannotUseThisOnYourself);
        return;
    }
    let Some((x, y, z)) = world.creature_position(player) else {
        return;
    };
    let name = world
        .players
        .get(&player)
        .map(|p| p.data.name.clone())
        .unwrap_or_default();
    world.set_intention_idle(gm);
    // `teleToLocation(new Location(x, y, z), true)`: instance 0 (instances are not modelled).
    let offset = world.max_teleport_offset();
    world.tele_to_location(
        gm,
        TeleportTarget::at(x, y, z)
            .with_instance(0)
            .with_random_offset(offset),
    );
    world.send_sys_message(gm, &format!("You have teleported to character {name}."));
}

/// `AdminTeleport.changeCharacterPosition`: `UPDATE characters SET x=?, y=?, z=? WHERE
/// char_name=?` for a player who is not online.
fn change_character_position(world: &mut World, gm: ObjectId, name: &str) {
    // FOLLOW-UP: the world never touches the database (D-15); this needs a persistence command
    // that writes exactly what Java writes (D-39).
    warn!("{gm}: //recall {name}: the player is offline; the position update is not ported yet");
    world.send_sys_message(gm, "SQLException while changing offline character's position");
}

/// The `admin_go<direction>` branch.
fn go_direction(world: &mut World, gm: ObjectId, command: &str) {
    let usage = "Usage: //go<north|south|east|west|up|down> [offset] (default 150)";
    let Some((mut x, mut y, mut z)) = world.creature_position(gm) else {
        return;
    };
    let mut offset = 150;
    let parsed = (|| -> Result<(), ()> {
        let value = command.get("admin_go".len()..).ok_or(())?;
        let mut st = Tokens::new(value);
        let direction = st.next_token().ok_or(())?;
        if st.has_more() {
            offset = parse_int(st.next_token().ok_or(())?).map_err(|_| ())?;
        }
        match direction {
            "east" => x = x.wrapping_add(offset),
            "west" => x = x.wrapping_sub(offset),
            "north" => y = y.wrapping_sub(offset),
            "south" => y = y.wrapping_add(offset),
            "up" => z = z.wrapping_add(offset),
            "down" => z = z.wrapping_sub(offset),
            _ => {}
        }
        Ok(())
    })();
    if parsed.is_err() {
        world.send_sys_message(gm, usage);
        return;
    }
    world.tele_to_location(gm, TeleportTarget::at(x, y, z).with_instance(0));
    world.show_admin_html(gm, "move.htm");
}

// ------------------------------------------------------------------------------- AdminGoto

/// `AdminGoto.LOCATIONS`. NOTE(java-quirk): "jail" is put twice; the later place wins.
const LOCATIONS: &[(&str, (i32, i32, i32))] = &[
    // Town
    ("gludio", (-14225, 123540, -3121)),
    ("gludin", (-83063, 150791, -3133)),
    ("giran", (82698, 148638, -3473)),
    ("giranharbor", (47938, 186864, -3420)),
    ("dion", (18748, 145437, -3132)),
    ("hunter", (116589, 76268, -2734)),
    ("huntervillage", (116589, 76268, -2734)),
    ("hunters", (116589, 76268, -2734)),
    ("huntersvillage", (116589, 76268, -2734)),
    ("oren", (82321, 55139, -1529)),
    ("aden", (147450, 27064, -2208)),
    ("ti", (-82687, 243157, -3734)),
    ("talkingisland", (-82687, 243157, -3734)),
    ("dwarf", (116551, -182493, -1525)),
    ("dwarfvillage", (116551, -182493, -1525)),
    ("dwarven", (116551, -182493, -1525)),
    ("dwarvenvillage", (116551, -182493, -1525)),
    ("orc", (-44211, -113521, -241)),
    ("orcvillage", (-44211, -113521, -241)),
    ("de", (12428, 16551, -4588)),
    ("delf", (12428, 16551, -4588)),
    ("darkelf", (12428, 16551, -4588)),
    ("darkelfvillage", (12428, 16551, -4588)),
    ("darkelven", (12428, 16551, -4588)),
    ("darkelvenvillage", (12428, 16551, -4588)),
    ("elf", (45873, 49288, -3064)),
    ("elfvillage", (45873, 49288, -3064)),
    ("elven", (45873, 49288, -3064)),
    ("elvenvillage", (45873, 49288, -3064)),
    ("kamael", (-116934, 46616, 368)),
    ("kamaelvillage", (-116934, 46616, 368)),
    ("floran", (17144, 170156, -3502)),
    ("heine", (111115, 219017, -3547)),
    ("rune", (44070, -50243, -796)),
    ("runeharbor", (36839, -38435, -3640)),
    ("goddard", (147725, -56517, -2780)),
    ("sc", (87358, -141982, -1341)),
    ("schuttgart", (87358, -141982, -1341)),
    // World
    ("arena", (12312, 182752, -3558)),
    ("racetrack", (12312, 182752, -3558)),
    ("racetrackarena", (12312, 182752, -3558)),
    ("gludioarena", (12312, 182752, -3558)),
    ("gludinarena", (-86979, 142402, -3643)),
    ("giranarena", (73890, 142656, -3778)),
    ("coliseum", (147451, 46728, -3410)),
    ("coloseum", (147451, 46728, -3410)),
    ("jail", (-114598, -249431, -2984)),
    ("gm", (-114542, -251005, -2992)),
    ("gmroom", (-114542, -251005, -2992)),
    ("misc", (-113360, -244676, -15536)),
    ("jail", (80151, -15445, -1805)),
    ("fg", (188611, 20588, -3696)),
    ("forbiddengateway", (188611, 20588, -3696)),
    ("cemetary", (167047, 20304, -3328)),
    ("toi1", (121685, 15749, -3852)),
    ("toi2", (114665, 12697, -3609)),
    ("toi3", (111249, 16031, -2127)),
    ("toi4", (114605, 19371, -645)),
    ("toi5", (117996, 16103, 843)),
    ("toi6", (114743, 19707, 1947)),
    ("toi7", (114552, 12354, 2957)),
    ("toi8", (110963, 16147, 3967)),
    ("toi9", (117356, 18462, 4977)),
    ("toi10", (118250, 15858, 5897)),
    ("toi11", (115824, 17242, 6760)),
    ("toi12", (113288, 14692, 7997)),
    ("toi13", (115322, 16756, 9007)),
    ("valleyofsaints", (65797, -71510, -3744)),
    ("forestofthedead", (52107, -54328, -3158)),
    ("beastfarm", (43805, -88010, -2780)),
    ("hotsprings", (144880, -113468, -2560)),
    ("rainbowsprings", (139997, -124860, -1896)),
    ("ketra", (146990, -67128, -3640)),
    ("varka", (125740, -40864, -3736)),
    ("it", (186699, -75915, -2826)),
    ("imperialtomb", (186699, -75915, -2826)),
    ("pagan", (36603, -51202, 712)),
    ("pagantemple", (36603, -51202, 712)),
    ("denofevil", (68693, -110438, -1946)),
    ("mithrilmines", (171946, -173352, 3440)),
    ("pi", (10468, -24569, -3650)),
    ("primeval", (10468, -24569, -3650)),
    ("primevalisle", (10468, -24569, -3650)),
    ("primevalisland", (10468, -24569, -3650)),
    ("gracia", (-186742, 244167, 2670)),
    // Grandboss
    ("qa", (-21610, 181594, -5734)),
    ("queenant", (-21610, 181594, -5734)),
    ("antqueen", (-21610, 181594, -5734)),
    ("core", (17723, 108915, -6493)),
    ("orfen", (55024, 17368, -5412)),
    ("zaken", (55312, 219168, -3223)),
    ("baium", (115213, 16623, 10080)),
    ("antharas", (183409, 114824, -8020)),
    ("valakas", (213896, -115436, -1644)),
    ("teza", (174238, -88011, -5115)),
    ("tezza", (174238, -88011, -5115)),
    ("frintezza", (174238, -88011, -5115)),
    ("tiat", (-247012, 251804, 4340)),
    ("freya", (114712, -114811, -11210)),
    // Find if a predefined location exists.
    // Find if a player with that name exists.
    // When no predefined location or player found, try to move to NPC.
];

/// Java `AdminGoto.onCommand`.
fn admin_goto(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let target_name = command.to_lowercase().replace("admin_goto ", "");
    let no_spaces = target_name.replace(' ', "");
    // NOTE(java-quirk): `//goto` alone leaves "admin_goto" in the name (the replaced text has a
    // space), so the usage line only shows for a command that ends in spaces.
    if no_spaces.is_empty() {
        world.send_message(gm, "Usage: //goto <area name, player name, npc name or npc id>");
        return Ok(());
    }

    // A predefined place. `teleToLocation(Location)`: heading 0, instance 0.
    if let Some((_, (x, y, z))) = LOCATIONS.iter().rev().find(|(name, _)| *name == no_spaces) {
        world.tele_to_location(gm, TeleportTarget::at(*x, *y, *z).with_instance(0));
        world.send_sys_message(gm, "You have been teleported.");
        return Ok(());
    }

    // A player. `teleToLocation(foundPlayer)`: its heading and instance.
    if let Some(found) = world.player_by_name(&no_spaces).filter(|p| *p != gm) {
        let place = world
            .players
            .get(&found)
            .map(|p| (p.core.x, p.core.y, p.core.z, p.core.heading));
        let instance = world.objects.get(&found).map_or(0, |e| e.instance_id);
        if let Some((x, y, z, heading)) = place {
            world.tele_to_location(
                gm,
                TeleportTarget::at(x, y, z)
                    .with_heading(heading)
                    .with_instance(instance),
            );
            world.send_sys_message(gm, "You have been teleported.");
            return Ok(());
        }
    }

    // "When no predefined location or player found, try to move to NPC."
    world.admin_command(gm, &format!("admin_list_positions {target_name} 1"), false);
    Ok(())
}

// -------------------------------------------------------------------------------- AdminHeal

/// Java `AdminHeal.onCommand`.
fn admin_heal(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_heal" {
        handle_heal(world, gm, None);
    } else if let Some(target) = command.get("admin_heal".len() + 1..) {
        handle_heal(world, gm, Some(target));
    } else {
        world.send_sys_message(gm, "Incorrect target/radius specified.");
    }
    Ok(())
}

/// Java `AdminHeal.handleHeal(activeChar, player)`.
fn handle_heal(world: &mut World, gm: ObjectId, param: Option<&str>) {
    let mut obj = world.target_of(gm);
    if let Some(param) = param {
        if let Some(player) = world.player_by_name(param) {
            obj = Some(player);
        } else if let Ok(radius) = parse_int(param) {
            // NOTE(java-quirk): the radius is only printed; every visible creature is healed.
            let all: Vec<ObjectId> = world
                .visible_objects(gm)
                .into_iter()
                .filter(|o| world.is_creature_kind(*o))
                .collect();
            for creature in all {
                heal_creature(world, creature);
            }
            world.send_sys_message(gm, &format!("Healed within {radius} unit radius."));
            return;
        }
    }

    let obj = obj.unwrap_or(gm);
    if world.is_creature_kind(obj) {
        heal_creature(world, obj);
    } else {
        world.invalid_target(gm);
    }
}

/// Java `AdminHeal`: `setCurrentHpMp(getMaxHp(), getMaxMp())`, then for a player
/// `setCurrentCp(getMaxCp())` and `updateUserInfo()`. The setters send the status updates: a
/// player's own 50 ms later, an NPC's to the players who target it. A creature without hit points
/// counts as dead and is left alone (death is M4-DEATH-01, which will set the dead flag, and then
/// the setters refuse by themselves); doors and static objects have no mana to restore.
fn heal_creature(world: &mut World, id: ObjectId) {
    let Some(view) = world.stats(id) else {
        return;
    };
    let alike_dead = view.core().cur_hp < 0.5;
    let (max_hp, max_mp, max_cp) = (
        f64::from(view.max_hp()),
        f64::from(view.max_mp()),
        f64::from(view.max_cp()),
    );
    match world.objects.get(&id).map(|e| e.kind) {
        Some(ObjectKind::Player) => {
            if !alike_dead {
                world.set_current_hp_mp(id, max_hp, max_mp);
                world.set_current_cp(id, max_cp, true);
            }
            world.send_user_info(id);
        }
        Some(ObjectKind::Npc) if !alike_dead => world.set_current_hp_mp(id, max_hp, max_mp),
        _ => {}
    }
}
