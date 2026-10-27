//! The character menu of the admin panels (`charmanage.htm`): `//char_manage`, the teleport of a
//! character to coordinates, `//goto_char_menu`, `//kick_menu`, and the ban and unban buttons of
//! the menu. The recall and kill buttons of the same menu are in `party.rs`, `clan.rs` and
//! `combat.rs`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminMenu.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException};
use crate::command::LeaveReason;
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminMenu.ADMIN_COMMANDS`, the words this file owns (the recall and kill words are
/// registered by their own files).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_char_manage", admin_char_manage),
        (
            "admin_teleport_character_to_menu",
            admin_teleport_character_to_menu,
        ),
        ("admin_goto_char_menu", admin_goto_char_menu),
        ("admin_kick_menu", admin_kick_menu),
        ("admin_ban_menu", admin_ban_menu),
        ("admin_unban_menu", admin_unban_menu),
        ("admin_kill_menu", admin_kill_menu),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// `AdminMenu.showMainPage`: `charmanage.htm`.
fn show_main_page(world: &mut World, gm: ObjectId) {
    world.show_admin_html(gm, "charmanage.htm");
}

/// Java `AdminMenu.onCommand`, `admin_char_manage`.
fn admin_char_manage(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    show_main_page(world, gm);
    Ok(())
}

/// Java `AdminMenu.teleportCharacter(player, loc, activeChar, message)`: the message, then the
/// teleport to `loc` with a random offset. No jail check, as in Java; the GM's page follows.
fn teleport_character(
    world: &mut World,
    player: ObjectId,
    place: TeleportTarget,
    gm: ObjectId,
    message: &str,
) {
    world.send_message(player, message);
    let offset = world.settings.teleport.max_offset_on_teleport;
    world.tele_to_location(player, place.with_random_offset(offset));
    show_main_page(world, gm);
}

/// Java `player.getLocation()`: a character's place with its heading and instance.
fn gm_place_of(world: &World, id: ObjectId) -> Option<TeleportTarget> {
    world.players.get(&id).map(|p| {
        TeleportTarget::at(p.core.x, p.core.y, p.core.z)
            .with_heading(p.core.heading)
            .with_instance(world.instance_id_of(id))
    })
}

/// Java `AdminMenu.onCommand`, `admin_teleport_character_to_menu <name> <x> <y> <z>`: the
/// character goes to the coordinates when the command has exactly four words and the name is
/// online. The numbers are `Integer.parseInt`, so a bad one is the exception of the command.
fn admin_teleport_character_to_menu(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let data = java_split_space(command);
    if data.len() == 5 {
        if let Some(player) = world.player_by_name(data[1]) {
            let x = super::parse_int(data[2])?;
            let y = super::parse_int(data[3])?;
            let z = super::parse_int(data[4])?;
            // `new Location(x, y, z)`: heading 0, instance 0.
            teleport_character(
                world,
                player,
                TeleportTarget::at(x, y, z).with_instance(0),
                gm,
                "Admin is teleporting you.",
            );
            return Ok(());
        }
    }
    show_main_page(world, gm);
    Ok(())
}

/// Java `AdminMenu.onCommand`, `admin_goto_char_menu <name>`: the GM goes to the character. A
/// missing name (a command shorter than `admin_goto_char_menu `) does nothing, as Java's
/// `StringIndexOutOfBoundsException` is ignored.
fn admin_goto_char_menu(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.chars().count() < 21 {
        return Ok(());
    }
    let name: String = command.chars().skip(21).collect();
    let Some(player) = world.player_by_name(&name) else {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::ThatPlayerIsNotCurrentlyOnline),
        );
        return Ok(());
    };
    // Java `teleportToCharacter(activeChar, player)`.
    if player == gm {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouCannotUseThisOnYourself),
        );
    } else if let Some(place) = gm_place_of(world, player) {
        // `activeChar.setInstanceId(player.getInstanceId())` (in `onCommand` and again in
        // `teleportToCharacter`), then `teleToLocation(player.getLocation(), true)`.
        world.set_instance_id(gm, place.instance_id);
        let offset = world.settings.teleport.max_offset_on_teleport;
        world.tele_to_location(gm, place.with_random_offset(offset));
        let name = world.object_name(player).unwrap_or_default();
        world.send_sys_message(gm, &format!("You're teleporting yourself to character {name}"));
    }
    show_main_page(world, gm);
    Ok(())
}

/// Java `AdminMenu.onCommand`, `admin_kick_menu <name>`: the character is stored and kicked, and
/// the GM is told in a plain message. Java's `Disconnection.storeAndDeleteWith(LeaveWorld)` ends
/// with the `LeaveWorld` packet, which is the logout ending here.
// NOTE(java-quirk): the kick is the Logout ending of `leave` (LeaveWorld, then the connection is
// closed), not the `Kicked` ending of the login server's kick, which sends no last packet.
fn admin_kick_menu(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    if words.len() > 1 {
        let name = words[1];
        let text = match world.player_by_name(name) {
            Some(player) => {
                let shown = world.object_name(player).unwrap_or_default();
                world.leave(player, LeaveReason::Logout);
                format!("You kicked {shown} from the game.")
            }
            None => format!("Player {name} was not found in the game."),
        };
        world.send_message(gm, &text);
    }
    show_main_page(world, gm);
    Ok(())
}

/// Java `AdminMenu.onCommand`, `admin_ban_menu <name>`: `admin_ban_char <name>`, run through the
/// command handler with its confirmation on (the GM is asked first when the command has one).
fn admin_ban_menu(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if java_split_space(command).len() > 1 {
        // Java `command.substring(14)`: the text after `admin_ban_menu`.
        let rest = command.get(14..).unwrap_or("");
        world.admin_command(gm, &format!("admin_ban_char{rest}"), true);
    }
    show_main_page(world, gm);
    Ok(())
}

/// Java `AdminMenu.onCommand`, `admin_unban_menu <name>`: `admin_unban_char <name>` with the
/// confirmation on, as `admin_ban_menu`.
fn admin_unban_menu(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if java_split_space(command).len() > 1 {
        // Java `command.substring(16)`: the text after `admin_unban_menu`.
        let rest = command.get(16..).unwrap_or("");
        world.admin_command(gm, &format!("admin_unban_char{rest}"), true);
    }
    show_main_page(world, gm);
    Ok(())
}

/// Java `AdminMenu.onCommand`, `admin_kill_menu` (exactly that word): the targeted creature loses
/// its maximum hit points (and its maximum CP and 1 more, for a player), then the page of the
/// target is shown. A target that is not a creature is the invalid target, with `main_menu.htm`.
// NOTE(java-quirk): Java's `handleKill` calls `obj.asCreature()` on the target without a check, so
// a command without any target is Java's NullPointerException. Champion monsters are not scaled
// here, as the `//kill` command does not scale them either.
fn admin_kill_menu(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command != "admin_kill_menu" {
        return Ok(());
    }
    let Some(target) = world.target_of(gm) else {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    };
    let is_creature = world.objects.get(&target).is_some_and(|e| e.kind.is_creature());
    let mut file = "main_menu.htm";
    if is_creature {
        let is_player = world.is_player(target);
        if let Some(view) = world.stats(target) {
            let max_hp = f64::from(view.max_hp());
            let amount = if is_player {
                max_hp + f64::from(view.max_cp()) + 1.0
            } else {
                max_hp + 1.0
            };
            world.reduce_current_hp(target, amount, Some(gm));
        }
        if is_player {
            file = "charmanage.htm";
        }
    } else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
    }
    world.show_admin_html(gm, file);
    Ok(())
}
