//! The transformation admin commands: `//transform <id>`, `//untransform`, `//transform_menu`, and
//! the purple maned horse and jet bike of `//ride_horse` and `//ride_bike`, with `//unride`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminTransform.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminRide.java
//
// # Left out, with the system that brings it
// - `//ride_wyvern`, `//ride_strider` and `//ride_wolf`: they mount a pet (`Player.mount`), and
//   mounts are not ported. Those words are not registered, so they answer "does not exist". The
//   `isMounted()` / `dismount()` parts of the other commands are no-ops for the same reason.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, is_numeric, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminRide.PURPLE_MANED_HORSE_TRANSFORMATION_ID`.
const PURPLE_MANED_HORSE_TRANSFORMATION_ID: i32 = 106;
/// Java `AdminRide.JET_BIKE_TRANSFORMATION_ID`.
const JET_BIKE_TRANSFORMATION_ID: i32 = 20001;

/// Java `AdminTransform` and the transformation words of `AdminRide`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_transform", admin_transform),
        ("admin_untransform", admin_untransform),
        ("admin_transform_menu", admin_transform_menu),
        ("admin_ride_horse", admin_ride_horse),
        ("admin_ride_bike", admin_ride_bike),
        ("admin_unride", admin_unride),
        ("admin_unride_wyvern", admin_unride),
        ("admin_unride_strider", admin_unride),
        ("admin_unride_wolf", admin_unride),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `Creature` check (`isCreature()`): a player, an NPC or a summon.
fn is_creature(world: &World, id: ObjectId) -> bool {
    world.objects.get(&id).is_some_and(|e| e.kind.is_creature())
}

/// Java `AdminTransform.onCommand`, `admin_transform_menu`: `transform.htm`.
fn admin_transform_menu(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.show_admin_html(gm, "transform.htm");
    Ok(())
}

/// Java `AdminTransform.onCommand`, `admin_untransform`: the target, or the GM when there is none,
/// loses its transformation. A thing that is not transformed is an invalid target.
fn admin_untransform(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let obj = world.target_of(gm).unwrap_or(gm);
    if !is_creature(world, obj) || !world.is_transformed(obj) {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    }
    world.stop_transformation(obj, true);
    Ok(())
}

/// Java `AdminTransform.onCommand`, `admin_transform [id]`: a player target is transformed into
/// the given id; a transformed target without an id is untransformed.
fn admin_transform(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(player) = world.target_of(gm).filter(|t| world.is_player(*t)) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    if world.is_sitting(gm) {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::YouCannotTransformWhileSitting),
        );
        return Ok(());
    }
    if world.is_transformed(player) || world.is_in_stance(player) {
        if !command.contains(' ') {
            world.untransform(player);
            return Ok(());
        }
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::YouAlreadyPolymorphedAndCannotPolymorphAgain),
        );
        return Ok(());
    }
    // Java `player.isInWater()`.
    if world.stats(player).is_some_and(|v| v.in_water()) {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::YouCannotPolymorphIntoTheDesiredFormInWater),
        );
        return Ok(());
    }
    // Java `isFlyingMounted() || isMounted()`; `isMounted()` is false until mounts exist.
    if world.stats(player).is_some_and(|v| v.is_flying_mounted()) {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::YouCannotPolymorphWhileRidingAPet),
        );
        return Ok(());
    }

    // `command.split(" ")`: exactly `//transform <id>`.
    let parts = java_split_space(command);
    if parts.len() != 2 || !is_numeric(parts[1]) {
        world.send_sys_message(gm, "Usage: //transform <id>");
        return Ok(());
    }
    let id = parse_int(parts[1])?;
    if !world.transform_player(id, player) {
        world.send_message(player, &format!("Unknown transformation ID: {id}"));
    }
    Ok(())
}

/// Java `AdminRide.getRideTarget`: the target when it is another player, else the GM.
fn ride_target(world: &World, gm: ObjectId) -> ObjectId {
    world
        .target_of(gm)
        .filter(|t| *t != gm && world.is_player(*t))
        .unwrap_or(gm)
}

/// Java `AdminRide.onCommand` for `admin_ride_horse` and `admin_ride_bike`: a summon blocks it;
/// otherwise the transformation is applied (no message when it is refused by the table).
fn ride_by_transformation(world: &mut World, gm: ObjectId, transform_id: i32) {
    let player = ride_target(world, gm);
    // Java `isMounted() || hasSummon()`: mounts are not ported, so only a summon counts.
    if world.has_servitor(player) || world.has_pet(player) {
        world.send_sys_message(gm, "Target already have a summon.");
        return;
    }
    if world.is_transformed(player) || world.is_in_stance(player) {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::YouAlreadyPolymorphedAndCannotPolymorphAgain),
        );
    } else {
        world.transform_player(transform_id, player);
    }
}

/// Java `AdminRide`, `admin_ride_horse`.
fn admin_ride_horse(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    ride_by_transformation(world, gm, PURPLE_MANED_HORSE_TRANSFORMATION_ID);
    Ok(())
}

/// Java `AdminRide`, `admin_ride_bike`.
fn admin_ride_bike(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    ride_by_transformation(world, gm, JET_BIKE_TRANSFORMATION_ID);
    Ok(())
}

/// Java `AdminRide.onCommand`, the `admin_unride*` words: the horse and the bike leave by
/// untransforming; the dismount of a mount is a no-op until mounts exist (M6 mounts).
fn admin_unride(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let player = ride_target(world, gm);
    if world.transformation_id(player) == PURPLE_MANED_HORSE_TRANSFORMATION_ID {
        world.untransform(player);
    }
    // Java: a second `if`, then `else dismount()`.
    if world.transformation_id(player) == JET_BIKE_TRANSFORMATION_ID {
        world.untransform(player);
    }
    Ok(())
}
