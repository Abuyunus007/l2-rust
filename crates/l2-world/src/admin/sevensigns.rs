//! The Mammon words: `//mammon_find` and `//mammon_respawn` (M8-ADMIN-07).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminMammon.java
//
// NOTE(java-quirk): Java reads `SevenSigns.isSealValidationPeriod()` once, into a field, when the
// handler is created at start-up, so the commands keep answering for the period of that moment.
// Here the period is read when the command runs (the handler has no state of its own).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::sevensigns::{MAMMON_BLACKSMITH_ID, MAMMON_MERCHANT_ID};
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_mammon_find", admin_mammon_find),
        ("admin_mammon_respawn", admin_mammon_respawn),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

fn competition_underway(world: &World, gm: ObjectId) {
    world.send(
        gm,
        &SystemMessage::new(SystemMessageId::TheSsqCompetitionPeriodIsUnderway),
    );
}

/// Java `admin_mammon_find [teleportIndex]`.
fn admin_mammon_find(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut teleport_index = -1;
    if command.len() > 17 {
        // `Integer.parseInt(command.substring(18))`.
        match command.get(18..).map(parse_int) {
            Some(Ok(index)) => teleport_index = index,
            _ => {
                world.send_sys_message(
                    gm,
                    "Usage: //mammon_find [teleportIndex] (where 1 = Blacksmith, 2 = Merchant)",
                );
                return Ok(());
            }
        }
    }
    if !world.ss_is_seal_validation_period() {
        competition_underway(world, gm);
        return Ok(());
    }
    for (npc_id, label, index) in [
        (MAMMON_BLACKSMITH_ID, "Blacksmith", 1),
        (MAMMON_MERCHANT_ID, "Merchant", 2),
    ] {
        match world.auto_spawn_first_info(npc_id) {
            Some((npc, _)) => {
                let position = npc.and_then(|npc| world.position_of(npc));
                if let Some((x, y, z)) = position {
                    world.send_sys_message(gm, &format!("{label} of Mammon: {x} {y} {z}"));
                    if teleport_index == index {
                        let offset = world.settings.teleport.max_offset_on_teleport;
                        world.tele_to_location(gm, TeleportTarget::at(x, y, z).with_random_offset(offset));
                    }
                }
            }
            None => world.send_sys_message(gm, &format!("{label} of Mammon isn't registered for spawn.")),
        }
    }
    Ok(())
}

/// Java `admin_mammon_respawn`.
fn admin_mammon_respawn(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    if !world.ss_is_seal_validation_period() {
        competition_underway(world, gm);
        return Ok(());
    }
    // Java asks the merchant first, then the blacksmith.
    for (npc_id, label) in [
        (MAMMON_MERCHANT_ID, "Merchant"),
        (MAMMON_BLACKSMITH_ID, "Blacksmith"),
    ] {
        match world.auto_spawn_first_info(npc_id) {
            Some((_, millis)) => world.send_sys_message(
                gm,
                &format!(
                    "The {label} of Mammon will respawn in {} minute(s).",
                    millis / 60000
                ),
            ),
            None => world.send_sys_message(gm, &format!("{label} of Mammon isn't registered for spawn.")),
        }
    }
    Ok(())
}
