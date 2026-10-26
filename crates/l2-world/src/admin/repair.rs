//! `//repair <name>` and `//restore <name>`: put a stuck character back in Talking Island.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminRepairChar.java
//
// Java writes the rows at once and says nothing to the GM, not even when the name is unknown.
// The statements go to the persistence task (D-15, `PersistCommand::RepairChar`); the character
// must be offline to keep the change (an online character stores its own position again).

use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException};
use crate::outbox::PersistCommand;
use crate::world::World;

type Outcome = Result<(), JavaException>;

pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_restore", admin_repair), ("admin_repair", admin_repair)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

fn admin_repair(world: &mut World, _gm: ObjectId, command: &str) -> Outcome {
    let parts = java_split_space(command);
    if parts.len() != 2 {
        return Ok(());
    }
    world.outbox.persist(PersistCommand::RepairChar {
        name: parts[1].to_owned(),
    });
    Ok(())
}
