//! The camera admin commands: `//cam`, `//camex` and `//cam3`. Each sends the special camera packet
//! of its Java `Quest` helper to the GM, the camera turning around the target.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminCamera.java,
// Java: mechanics/script/Quest.java (specialCamera, specialCameraEx, specialCamera3)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SpecialCamera, SystemMessage};
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminCamera.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_cam", admin_cam),
        ("admin_camex", admin_camex),
        ("admin_cam3", admin_cam3),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `Integer.parseInt(com[i])` for the arguments `from..=to`, all of them read before the
/// packet is made (a bad one is the command's exception).
fn numbers(words: &[&str], from: usize, to: usize) -> Result<Vec<i32>, JavaException> {
    words[from..=to].iter().map(|w| parse_int(w)).collect()
}

/// The target of the camera. `None` is Java's `YOUR_TARGET_CANNOT_BE_FOUND` (no creature).
fn camera_target(world: &World, gm: ObjectId) -> Option<ObjectId> {
    world
        .target_of(gm)
        .filter(|t| world.objects.get(t).is_some_and(|e| e.kind.is_creature()))
}

/// The shared start of the three commands: the target check (first in Java), then the words.
/// `Ok(None)` is Java's `return false` (the target or the count was wrong).
fn prepare(
    world: &mut World,
    gm: ObjectId,
    words: &[&str],
    expected_len: usize,
    usage: &str,
) -> Result<Option<ObjectId>, JavaException> {
    let Some(target) = camera_target(world, gm) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::YourTargetCannotBeFound));
        return Ok(None);
    };
    if words.len() != expected_len {
        world.send_sys_message(gm, usage);
        return Ok(None);
    }
    Ok(Some(target))
}

/// Java `AdminCamera`, `admin_cam force angle1 angle2 time range duration relYaw relPitch isWide
/// relAngle`. The command takes eleven numbers, and the last one is read but not used, as in Java.
// NOTE(java-quirk): the usage text lists ten parameters, the length check wants eleven. The packet
// gets `range` (not `duration`) as its duration field, because Java's 11-argument `SpecialCamera`
// constructor swaps the two.
fn admin_cam(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let usage = "Usage: //cam force angle1 angle2 time range duration relYaw relPitch isWide relAngle";
    let Some(target) = prepare(world, gm, &words, 12, usage)? else {
        return Ok(());
    };
    let v = numbers(&words, 1, 10)?;
    let packet = SpecialCamera::from_range_duration(
        target.0, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9],
    );
    world.send(gm, &packet);
    Ok(())
}

/// Java `AdminCamera`, `admin_camex force angle1 angle2 time duration relYaw relPitch isWide
/// relAngle`.
// NOTE(java-quirk): the Ex constructor passes 0 as the packet's duration, so the duration argument
// is parsed and then dropped.
fn admin_camex(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let usage = "Usage: //camex force angle1 angle2 time duration relYaw relPitch isWide relAngle";
    let Some(target) = prepare(world, gm, &words, 10, usage)? else {
        return Ok(());
    };
    let v = numbers(&words, 1, 9)?;
    let packet = SpecialCamera::ex(target.0, v[0], v[1], v[2], v[3], v[5], v[6], v[7], v[8]);
    world.send(gm, &packet);
    Ok(())
}

/// Java `AdminCamera`, `admin_cam3 force angle1 angle2 time range duration relYaw relPitch isWide
/// relAngle unk`: the same packet with the post-C4 parameter.
fn admin_cam3(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let usage = "Usage: //cam3 force angle1 angle2 time range duration relYaw relPitch isWide relAngle unk";
    let Some(target) = prepare(world, gm, &words, 12, usage)? else {
        return Ok(());
    };
    let v = numbers(&words, 1, 11)?;
    let packet = SpecialCamera::new(
        target.0, v[0], v[1], v[2], v[3], v[5], v[6], v[7], v[8], v[9], v[10],
    );
    world.send(gm, &packet);
    Ok(())
}
