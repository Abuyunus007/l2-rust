//! The invulnerability commands: `//invul` (the GM) and `//setinvul` (the target player).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminInvul.java
//
// The flag is `Creature.setInvul`; the damage code reads `World::is_invul` (`status.rs`), which
// is Java's `isInvul()`, so a teleporting character is invulnerable as well.
//
// # Left out, with the system that brings it
// - `Player.isTeleportProtected()` (spawn protection) is part of Java's `Player.isInvul()`; it is
//   not modelled yet, so it is not part of the answer here either.

use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminInvul` registrations.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_invul", admin_invul), ("admin_setinvul", admin_invul)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminInvul.onCommand`: the command must be exactly the word (Java's `switch`).
fn admin_invul(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    match command {
        "admin_invul" => toggle_invul(world, gm, gm),
        "admin_setinvul" => match world.target_of(gm).filter(|t| world.is_player(*t)) {
            Some(target) => toggle_invul(world, target, gm),
            None => world.send_sys_message(gm, "You must target a player."),
        },
        _ => {}
    }
    Ok(())
}

/// Java `AdminInvul.toggleInvul(target, admin)`.
fn toggle_invul(world: &mut World, target: ObjectId, admin: ObjectId) {
    // NOTE(java-quirk): Java reads `isInvul()` (which includes the teleporting state) and writes
    // `_isInvul`, so toggling a teleporting character leaves the flag on.
    let invul = !world.is_invul(target);
    world.set_invul(target, invul);

    // Notify target.
    let state = if invul { "invulnerable." } else { "mortal." };
    world.send_sys_message(target, &format!("You are now {state}"));

    // Notify admin.
    if admin != target {
        let name = world.object_name(target).unwrap_or_default();
        world.send_sys_message(admin, &format!("You made {name} {state}"));
    }
}
