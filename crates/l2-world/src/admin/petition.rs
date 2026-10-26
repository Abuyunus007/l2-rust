//! The petition commands of the GMs: `//view_petitions`, `//view_petition`, `//accept_petition`,
//! `//reject_petition`, `//reset_petitions` and `//force_peti`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminPetition.java
//
// The queue and the packets are in `petition.rs`.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminPetition` registration.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let handler: AdminHandler = admin_petition;
    for command in [
        "admin_view_petitions",
        "admin_view_petition",
        "admin_accept_petition",
        "admin_reject_petition",
        "admin_reset_petitions",
        "admin_force_peti",
    ] {
        handlers.register(command, handler);
    }
}

/// Java `AdminPetition.onCommand`.
fn admin_petition(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `Integer.parseInt(command.split(" ")[1])`, any exception ignored.
    let mut petition_id = command
        .split(' ')
        .nth(1)
        .and_then(|word| parse_int(word).ok())
        .unwrap_or(-1);

    if command == "admin_view_petitions" {
        world.send_pending_petition_list(gm);
    } else if command.starts_with("admin_view_petition") {
        world.view_petition(gm, petition_id);
    } else if command.starts_with("admin_accept_petition") {
        if world.is_player_in_consultation(gm) {
            world.send(
                gm,
                &SystemMessage::new(SystemMessageId::YouMayOnlySubmitOnePetitionActiveAtATime),
            );
            return Ok(());
        }
        if world.is_petition_in_process(petition_id) {
            world.send(
                gm,
                &SystemMessage::new(SystemMessageId::YourPetitionIsBeingProcessed),
            );
            return Ok(());
        }
        if !world.accept_petition(gm, petition_id) {
            world.send(
                gm,
                &SystemMessage::new(SystemMessageId::NotUnderPetitionConsultation),
            );
        }
    } else if command.starts_with("admin_reject_petition") {
        if !world.reject_petition(gm, petition_id) {
            world.send(
                gm,
                &SystemMessage::new(SystemMessageId::FailedToCancelPetitionPleaseTryAgainLater),
            );
        }
        world.send_pending_petition_list(gm);
    } else if command == "admin_reset_petitions" {
        if world.is_any_petition_in_process() {
            world.send(
                gm,
                &SystemMessage::new(SystemMessageId::YourPetitionIsBeingProcessed),
            );
            return Ok(());
        }
        world.clear_pending_petitions();
        world.send_pending_petition_list(gm);
    } else if command.starts_with("admin_force_peti") {
        let Some(target) = world.target_of(gm).filter(|t| world.players.contains_key(t)) else {
            world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
            return Ok(());
        };
        // NOTE(java-quirk): `command.substring(15)` starts one character too early: the text
        // begins with the last letter of the command ("i text").
        let Some(text) = command.get(15..) else {
            world.send_sys_message(gm, "Usage: //force_peti text");
            return Ok(());
        };
        if let Some(id) = world.submit_petition(target, text, 9) {
            petition_id = id;
            world.accept_petition(gm, petition_id);
        }
    }
    Ok(())
}
