//! `//changelvl`: change the access level of the target, or of a named character.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminChangeAccessLevel.java
// Java: entity/actor/Player.java (setAccessLevel)
//
// # Left out, with the system that brings it
// - A character who is not online: Java runs `UPDATE characters SET accesslevel=? WHERE
//   char_name=?` and tells the GM from the update count. The world cannot wait for the database
//   (D-15), so it asks its name table (`char_id_by_name`, the online players and the startup
//   names) and queues `PersistCommand::CharAccessLevel`; a name the table does not know is
//   "not found", which is the update count 0 of Java (a character created after the start is
//   not in the table).
// - `CharInfoTable.addName` and the warnings Java logs for an unregistered or non-zero level.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::outbox::PersistCommand;
use crate::command::LeaveReason;
use crate::world::World;

type Outcome = Result<(), JavaException>;

pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_changelvl", admin_changelvl)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

fn admin_changelvl(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let parts = java_split_space(command);
    match parts.len() {
        2 => {
            // Everything inside Java's `try` (the parse included) ends in the usage message.
            let Ok(level) = parse_int(parts[1]) else {
                usage(world, gm);
                return Ok(());
            };
            match world.target_of(gm).filter(|t| world.is_player(*t)) {
                None => {
                    world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
                }
                Some(target) => online_change(world, gm, target, level),
            }
        }
        3 => {
            // Outside the `try`: a bad number reaches the dispatcher as an exception.
            let level = parse_int(parts[2])?;
            let name = parts[1];
            if let Some(player) = world.player_by_name(name) {
                online_change(world, gm, player, level);
            } else if let Some(char_id) = world.char_id_by_name(name) {
                world
                    .outbox
                    .persist(PersistCommand::CharAccessLevel { char_id, level });
                world.send_sys_message(gm, &format!("Character's access level is now set to {level}"));
            } else {
                world.send_sys_message(gm, "Character not found or access level unaltered.");
            }
        }
        _ => {}
    }
    Ok(())
}

fn usage(world: &World, gm: ObjectId) {
    world.send_sys_message(
        gm,
        "Usage: //changelvl <target_new_level> | <player_name> <new_level>",
    );
}

/// Java `AdminChangeAccessLevel.onlineChange`.
fn online_change(world: &mut World, gm: ObjectId, player: ObjectId, level: i32) {
    let name = world.object_name(player).unwrap_or_default();
    if level >= 0 {
        let rights = world.data.admin_rights();
        if rights.is_some_and(|r| r.has_level(level)) {
            let level_name = rights.map(|r| r.level_name(level)).unwrap_or_default();
            set_access_level(world, player, level);
            world.send_message(
                player,
                &format!("Your access level has been changed to {level_name} ({level})."),
            );
            world.send_message(
                gm,
                &format!("{name}'s access level has been changed to {level_name} ({level})."),
            );
        } else {
            world.send_sys_message(
                gm,
                &format!("You are trying to set unexisting access level: {level} please try again with a valid one!"),
            );
        }
    } else {
        set_access_level(world, player, level);
        world.send_message(player, "Your character has been banned. Bye.");
        // `Disconnection.of(player).storeAndDeleteWith(LeaveWorld.STATIC_PACKET)`.
        world.leave(player, LeaveReason::Logout);
    }
}

/// Java `Player.setAccessLevel(level)`: the level, the colours of the level, `UserInfo` to the
/// player and its neighbours.
fn set_access_level(world: &mut World, id: ObjectId, level: i32) {
    if let Some(player) = world.players.get_mut(&id) {
        player.data.access_level = level;
        // `setNameColor(accessLevel.getNameColor())` replaces a colour set by `//setcolor`.
        player.gm.name_color = None;
        // The title colour is the level's again: the world shows it while the profile has the
        // default (see `build_user_info`).
        player.data.profile.title_color = crate::data::DEFAULT_TITLE_COLOR;
    }
    world.broadcast_user_info(id);
}
