//! The karma, pk and pvp admin commands: `//nokarma`, `//setkarma <karma>`, `//setpk <count>`,
//! `//setpvp <count>` and `//set_pvp_flag`, on the target.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEditChar.java (admin_nokarma,
// admin_setkarma, setTargetKarma, admin_setpk, admin_setpvp, admin_set_pvp_flag)
//
// # Left out, with the system that brings it
// - `//set_pvp_flag` on a summon: a summon's own pvp flag is not modelled (`update_pvp_flag` only
//   takes players). A summon target does nothing (M6-SUMMON-01 follow-up).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminEditChar` registrations of the karma, pk and pvp words.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_nokarma", admin_nokarma),
        ("admin_setkarma", admin_setkarma),
        ("admin_setpk", admin_setpk),
        ("admin_setpvp", admin_setpvp),
        ("admin_set_pvp_flag", admin_set_pvp_flag),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The target when it is a player (Java `activeChar.getTarget()` and `isPlayer()`).
fn target_player(world: &World, gm: ObjectId) -> Option<ObjectId> {
    world.target_of(gm).filter(|t| world.is_player(*t))
}

/// Java `AdminEditChar.setTargetKarma(activeChar, newKarma)`: nothing unless the target is a
/// player; a negative value is refused with a message to the GM.
fn set_target_karma(world: &mut World, gm: ObjectId, new_karma: i32) {
    let Some(target) = target_player(world, gm) else {
        return;
    };
    if new_karma < 0 {
        world.send_sys_message(gm, "You must enter a value for karma greater than or equal to 0.");
        return;
    }
    // "for display"
    let old_karma = world.karma_of_player(target);
    world.set_karma(target, new_karma);

    // Common character information.
    world.send(
        target,
        &SystemMessage::new(SystemMessageId::YourKarmaHasBeenChangedToS1)
            .with(SystemMessageParam::IntNumber(new_karma)),
    );
    // Admin information.
    let name = world.object_name(target).unwrap_or_default();
    world.send_sys_message(
        gm,
        &format!("Successfully Changed karma for {name} from ({old_karma}) to ({new_karma})."),
    );
}

/// Java `AdminEditChar.onCommand`, `admin_nokarma`: the target's karma becomes 0.
fn admin_nokarma(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_nokarma" {
        set_target_karma(world, gm, 0);
    }
    Ok(())
}

/// Java `admin_setkarma`: `Integer.parseInt(command.substring(15))`; anything else is the usage.
fn admin_setkarma(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    match command.get(15..).and_then(|value| parse_int(value).ok()) {
        Some(karma) => set_target_karma(world, gm, karma),
        None => world.send_sys_message(gm, "Usage: //setkarma <new_karma_value>"),
    }
    Ok(())
}

/// Java `admin_setpk`: `Integer.parseInt(command.substring(12))`, then the pk count of the target
/// player, the target's `UserInfo` and two messages.
fn admin_setpk(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(pk) = command.get(12..).and_then(|value| parse_int(value).ok()) else {
        world.send_sys_message(gm, "Usage: //setpk <pk_count>");
        return Ok(());
    };
    let Some(target) = target_player(world, gm) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    if let Some(player) = world.players.get_mut(&target) {
        player.data.pk_kills = pk;
    }
    world.broadcast_user_info(target);
    world.send_message(target, &format!("A GM changed your PK count to {pk}"));
    let name = world.object_name(target).unwrap_or_default();
    world.send_message(gm, &format!("{name}'s PK count changed to {pk}"));
    Ok(())
}

/// Java `admin_setpvp`: `Integer.parseInt(command.substring(13))`. The title and colour follow the
/// new count (`updatePvpTitleAndColor(false)`), then the `UserInfo` and the messages.
fn admin_setpvp(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(pvp) = command.get(13..).and_then(|value| parse_int(value).ok()) else {
        world.send_sys_message(gm, "Usage: //setpvp <pvp_count>");
        return Ok(());
    };
    let Some(target) = target_player(world, gm) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    if let Some(player) = world.players.get_mut(&target) {
        player.data.pvp_kills = pvp;
    }
    world.update_pvp_title_and_color(target, false);
    world.broadcast_user_info(target);
    world.send_message(target, &format!("A GM changed your PVP count to {pvp}"));
    let name = world.object_name(target).unwrap_or_default();
    world.send_message(gm, &format!("{name}'s PVP count changed to {pvp}"));
    Ok(())
}

/// Java `admin_set_pvp_flag`: `playable.updatePvPFlag(Math.abs(playable.getPvpFlag() - 1))` on the
/// target (a playable: a player here, see the module note for summons).
fn admin_set_pvp_flag(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(target) = world.target_of(gm).filter(|t| world.is_player(*t)) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    let flag = world.pvp_flag_of(target);
    world.update_pvp_flag(target, (flag - 1).abs());
    Ok(())
}
