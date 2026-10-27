//! The instance words: `//setinstance`, `//createinstance`, `//destroyinstance`,
//! `//listinstances`, `//instancezone`, `//instancezone_clear`, `//instance_spawns` and
//! `//instancezones` (M8-INST-01).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminInstance.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminInstanceZone.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSpawn.java (admin_instance_spawns)
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminHelp.java (admin_instancezones)

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;
use tracing::info;

use super::html::build_page;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::instance::FIRST_DYNAMIC_INSTANCE_ID;
use crate::objects::ObjectKind;
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminInstance.ADMIN_COMMANDS`, `AdminInstanceZone.ADMIN_COMMANDS`, and the instance words
/// of `AdminSpawn` and `AdminHelp`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_setinstance", admin_instance),
        ("admin_createinstance", admin_instance),
        ("admin_destroyinstance", admin_instance),
        ("admin_listinstances", admin_instance),
        ("admin_instancezone", admin_instance_zone),
        ("admin_instancezone_clear", admin_instance_zone),
        ("admin_instance_spawns", admin_instance_spawns),
        ("admin_instancezones", admin_instance_zones),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminInstance.onCommand`.
fn admin_instance(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_createinstance") {
        // `command.split(" ")`.
        let parts: Vec<&str> = java_split_spaces(command);
        if parts.len() != 3 || !is_numeric(parts[2]) {
            world.send_sys_message(
                gm,
                "Example: //createinstance <id> <templateId> - ids => 300000 are reserved for dynamic instances",
            );
        } else {
            // `Integer.parseInt(parts[1])` and `parts[2]` (numeric, but it may overflow): the
            // exception is "Failed loading".
            match (parse_int(parts[1]), parse_int(parts[2])) {
                (Ok(id), Ok(template_id)) => {
                    if id < FIRST_DYNAMIC_INSTANCE_ID && world.create_instance_from_template(id, template_id)
                    {
                        world.send_sys_message(gm, "Instance created.");
                    } else {
                        world.send_sys_message(gm, "Failed to create instance.");
                    }
                }
                _ => {
                    world.send_sys_message(gm, &format!("Failed loading: {} {}", parts[1], parts[2]));
                }
            }
        }
    } else if command.starts_with("admin_listinstances") {
        // Only the instances with a script world are listed.
        let lines: Vec<String> = world
            .instances()
            .filter_map(|instance| {
                let template = world.instance_world(instance.id)?.template_id;
                Some(format!(
                    "Id: {} Name: {}",
                    instance.id,
                    world.instance_name(template)
                ))
            })
            .collect();
        if lines.is_empty() {
            world.send_sys_message(gm, "No active instances.");
        }
        for line in lines {
            world.send_sys_message(gm, &line);
        }
    } else if command.starts_with("admin_setinstance") {
        let mut st = Tokens::new(command);
        st.next_token();
        // Every exception of the block is "Use //setinstance id".
        let Some(value) = st.next_token().and_then(|t| parse_int(t).ok()) else {
            world.send_sys_message(gm, "Use //setinstance id");
            return Ok(());
        };
        if world.instance(value).is_none() {
            world.send_sys_message(gm, &format!("Instance {value} does not exist."));
            return Ok(());
        }
        let target = world.target_of(gm);
        // "Don't separate summons from masters".
        let Some(target) = target.filter(|t| world.object(*t).is_some_and(|e| e.kind != ObjectKind::Summon))
        else {
            world.send_sys_message(gm, "Incorrect target.");
            return Ok(());
        };
        world.set_instance_id(target, value);
        if world.object(target).is_some_and(|e| e.kind == ObjectKind::Player) {
            world.send_message(target, &format!("Admin set your instance to:{value}"));
            // `player.teleToLocation(player.getLocation())`: the location carries the new id.
            if let Some((x, y, z)) = world.position_of(target) {
                let heading = world.player(target).map_or(0, |p| p.core.heading);
                world.tele_to_location(
                    target,
                    TeleportTarget::at(x, y, z)
                        .with_heading(heading)
                        .with_instance(world.instance_id_of(target)),
                );
            }
        }
        let name = world.object_name(target).unwrap_or_default();
        let now_in = world.instance_id_of(target);
        world.send_sys_message(gm, &format!("Moved {name} to instance {now_in}."));
    } else if command.starts_with("admin_destroyinstance") {
        let mut st = Tokens::new(command);
        st.next_token();
        match st.next_token().and_then(|t| parse_int(t).ok()) {
            Some(value) => {
                world.destroy_instance(value);
                world.send_sys_message(gm, "Instance destroyed");
            }
            None => world.send_sys_message(gm, "Use //destroyinstance id"),
        }
    }
    Ok(())
}

/// Java `String.split(" ")`: single spaces, trailing empty strings dropped.
fn java_split_spaces(text: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = text.split(' ').collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Java `AdminInstanceZone.onCommand`.
fn admin_instance_zone(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let target = world
        .target_of(gm)
        .and_then(|t| world.object_name(t))
        .unwrap_or_else(|| "no-target".to_owned());
    // `GMAudit.logAction(activeChar.getName(), command, target, "")`: written whatever the
    // `GMAudit` switch says.
    let name = world.object_name(gm).unwrap_or_default();
    info!(target: "gmaudit", "{name}>{command}>{target}>");
    if command.starts_with("admin_instancezone_clear") {
        match clear_instance_time(world, gm, command) {
            Ok(()) => {}
            Err(message) => {
                world.send_sys_message(gm, &format!("Failed clearing instance time: {message}"));
                world.send_sys_message(gm, "Usage: //instancezone_clear <playername> [instanceId]");
            }
        }
        return Ok(());
    }
    let mut st = Tokens::spaces(command);
    st.next_token();
    if let Some(player_name) = st.next_token() {
        match world.player_by_name(player_name) {
            Some(player) => display_instance_times(world, player, gm),
            None => {
                world.send_sys_message(gm, &format!("The player {player_name} is not online"));
                world.send_sys_message(gm, "Usage: //instancezone [playername]");
            }
        }
    } else if let Some(target) = world.target_of(gm) {
        if world.object(target).is_some_and(|e| e.kind == ObjectKind::Player) {
            display_instance_times(world, target, gm);
        }
    } else {
        display_instance_times(world, gm, gm);
    }
    Ok(())
}

/// The `try` block of `admin_instancezone_clear <player> <instanceId>`; the error is the
/// exception's `getMessage()`, in the order Java meets them.
fn clear_instance_time(world: &mut World, gm: ObjectId, command: &str) -> Result<(), String> {
    let mut st = Tokens::spaces(command);
    st.next_token();
    // `st.nextToken()` past the end: NoSuchElementException, whose message is null.
    let name = st.next_token().ok_or("null")?;
    let player = world.player_by_name(name);
    let text = st.next_token().ok_or("null")?;
    let instance_id = parse_int(text).map_err(|_| format!("For input string: \"{text}\""))?;
    let zone = world.instance_name(instance_id);
    let player = player.ok_or(
        "Cannot invoke \"org.l2jmobius.gameserver.entity.actor.Player.getObjectId()\" because \"player\" is null",
    )?;
    world.delete_instance_time(player, instance_id);
    let player_name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(
        gm,
        &format!("Instance zone {zone} cleared for player {player_name}"),
    );
    world.send_message(player, &format!("Admin cleared instance zone {zone} for you"));
    Ok(())
}

/// Java `AdminInstanceZone.display(player, activeChar)`.
fn display_instance_times(world: &mut World, player: ObjectId, gm: ObjectId) {
    let player_name = world.object_name(player).unwrap_or_default();
    let mut html = format!(
        "<html><center><table width=260><tr><td width=40><button value=\"Main\" action=\"bypass admin_admin\" width=40 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td width=180><center>Character Instances</center></td><td width=40><button value=\"Back\" action=\"bypass -h admin_current_player\" width=40 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br><font color=\"LEVEL\">Instances for {player_name}</font><center><br><table><tr><td width=150>Name</td><td width=50>Time</td><td width=70>Action</td></tr>"
    );
    let now = world.instance_now_ms();
    for (instance_id, time) in world.instance_times(player) {
        let remaining = (time - now) / 1000;
        let (hours, minutes) = if remaining > 0 {
            ((remaining / 3600) as i32, ((remaining % 3600) / 60) as i32)
        } else {
            (0, 0)
        };
        let name = world.instance_name(instance_id);
        html.push_str(&format!(
            "<tr><td>{name}</td><td>{hours}:{minutes}</td><td><button value=\"Clear\" action=\"bypass -h admin_instancezone_clear {player_name} {instance_id}\" width=60 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr>"
        ));
    }
    html.push_str("</table></html>");
    if let Some(message) = NpcHtmlMessage::new(0, &html) {
        world.send_html(gm, &message);
    }
}

/// Java `AdminSpawn`, `admin_instance_spawns <instance>`: the living NPCs of a dynamic instance
/// (50 at most, "because of client html limitation"), each with a link to go there.
fn admin_instance_spawns(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token();
    let Some(instance) = st.next_token().and_then(|t| parse_int(t).ok()) else {
        world.send_sys_message(gm, "Usage //instance_spawns <instance_number>");
        return Ok(());
    };
    if instance < FIRST_DYNAMIC_INSTANCE_ID {
        world.send_sys_message(gm, "Invalid instance number.");
        return Ok(());
    }
    let mut html = format!(
        "<html><table width=\"100%\"><tr><td width=45><button value=\"Main\" action=\"bypass admin_admin\" width=45 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td width=180><center><font color=\"LEVEL\">Spawns for {instance}</font></td><td width=45><button value=\"Back\" action=\"bypass -h admin_current_player\" width=45 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br><table width=\"100%\"><tr><td width=200>NpcName</td><td width=70>Action</td></tr>"
    );
    let Some(npcs) = world
        .instance(instance)
        .map(|i| i.npcs.keys().copied().collect::<Vec<_>>())
    else {
        world.send_sys_message(gm, &format!("Cannot find instance {instance}"));
        return Ok(());
    };
    let mut counter = 0;
    let mut skipped = 0;
    for npc in npcs {
        // `!npc.isDead()`: an NPC whose corpse is gone is still in the list; Java reads its last
        // position.
        if !world.npcs.contains_key(&npc) || world.is_dead(npc) {
            continue;
        }
        if counter < 50 {
            let name = world.object_name(npc).unwrap_or_default();
            let (x, y, z) = world.position_of(npc).unwrap_or_default();
            html.push_str(&format!(
                "<tr><td>{name}</td><td><a action=\"bypass -h admin_move_to {x} {y} {z}\">Go</a></td></tr>"
            ));
            counter += 1;
        } else {
            skipped += 1;
        }
    }
    html.push_str(&format!(
        "<tr><td>Skipped:</td><td>{skipped}</td></tr></table></body></html>"
    ));
    if let Some(message) = NpcHtmlMessage::new(0, &html) {
        world.send_html(gm, &message);
    }
    Ok(())
}

/// Java `AdminHelp`, `admin_instancezones [page=N]`: the template ids and names, ten per page.
// NOTE(java-quirk): like `//commands`, the page is `substring(20)` after a plain `contains("page")`
// and the default page is 1 of a list counted from 0.
fn admin_instance_zones(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let page = if command.contains("page") {
        let text: String = command.chars().skip(20).collect();
        let text = text.replace("page=", "");
        parse_int(&text).map_err(|_| JavaException::number_format(&text))?
    } else {
        1
    };
    let names = world.instances.templates.name_list().to_vec();
    let result = build_page(
        &names,
        10,
        "bypass -h admin_instancezones",
        page,
        |(id, name), sb| {
            sb.push_str(&format!(
                "<tr><td width=30 align=left>{id}</td><td width=200>{name}</td></tr>"
            ));
        },
    );
    let mut html = world.admin_html_file(gm, "data/html/admin/help/instancezones.htm");
    html.replace("%pages%", &result.pager_html())
        .replace("%list%", &result.body);
    world.send_admin_html(gm, &html);
    Ok(())
}
