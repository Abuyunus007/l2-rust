//! The GM's own commands and the small target ones: `//superhaste <0-4> [seconds]` (and `//speed`),
//! `//hide on|off`, `//target <name>` and `//targetsay <text>`, with their menu words.
//!
//! Left out, with the system that brings it: `//gmspeed` (the GM speed multiplier is not a stat
//! of the creature here yet, and `UseSuperHasteAsGMSpeed` defaults to off, so the usual path needs
//! it), and the silence mode of `//hide` (`Player.setSilenceMode`, the refusal of private messages,
//! is not modelled, so `//hide` leaves it out).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminSuperHaste,AdminHide,
// Java: AdminTarget,AdminTargetSay}.java, entity/actor/Player.java (setHiding)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, CreatureSay, SayBody, SenderField, SystemMessage};
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::effects::{SkillFinishType, SkillKey};
use crate::objects::ObjectKind;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminSuperHaste.SUPER_HASTE_ID`.
const SUPER_HASTE_ID: i32 = 7029;

/// Java `AdminSuperHaste`, `AdminHide`, `AdminTarget` and `AdminTargetSay`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_superhaste", admin_superhaste),
        ("admin_superhaste_menu", admin_superhaste_menu),
        ("admin_speed", admin_superhaste),
        ("admin_speed_menu", admin_superhaste_menu),
        ("admin_hide", admin_hide),
        ("admin_target", admin_target),
        ("admin_targetsay", admin_targetsay),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminSuperHaste.onCommand`, `admin_superhaste <level 0-4> [seconds]` (and `admin_speed`).
/// The old haste goes first; a level of 1 to 4 gives the new one, for `seconds` (0 when missing).
/// Anything that does not parse, and a level without a skill template (Java's `NullPointerException`),
/// is the usage text.
fn admin_superhaste(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let usage = "Usage: //superhaste <Effect level (0-4)> <Time in seconds>";
    let Some(level) = words.get(1).and_then(|w| parse_int(w).ok()) else {
        world.send_message(gm, usage);
        return Ok(());
    };
    // Java: `isAffectedBySkill(...)` decides REMOVED (a message) only for level 0.
    let was_affected = world.is_affected_by_skill(gm, SUPER_HASTE_ID);
    let finish = if level == 0 && was_affected {
        SkillFinishType::Removed
    } else {
        SkillFinishType::Normal
    };
    world.stop_skill_effects(gm, finish, SUPER_HASTE_ID);
    if (1..=4).contains(&level) {
        let time = match words.get(2) {
            Some(text) => match parse_int(text) {
                Ok(time) => time,
                Err(_) => {
                    world.send_message(gm, usage);
                    return Ok(());
                }
            },
            None => 0,
        };
        let has_template = world
            .data
            .stat_data()
            .is_some_and(|s| s.skills.get(SUPER_HASTE_ID, level).is_some());
        if !has_template {
            world.send_message(gm, usage);
            return Ok(());
        }
        world.apply_effects_timed(gm, gm, SkillKey::new(SUPER_HASTE_ID, level), true, time);
    }
    Ok(())
}

/// Java `AdminSuperHaste`, `admin_superhaste_menu` and `admin_speed_menu`: `gm_menu.htm`.
fn admin_superhaste_menu(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.show_admin_html(gm, "gm_menu.htm");
    Ok(())
}

/// Java `Player.setHiding(hide)`: `false` (and nothing changes) when the GM is not a GM or already
/// is in that state. Silence mode is not modelled (see the module comment).
fn set_hiding(world: &mut World, gm: ObjectId, hide: bool) -> bool {
    if !world.is_gm(gm) {
        return false;
    }
    let invisible = world.objects.get(&gm).is_some_and(|e| e.invisible);
    if invisible && hide {
        return false;
    }
    if !invisible && !hide {
        return false;
    }
    world.set_invul(gm, hide);
    world.set_invisible(gm, hide);
    if hide {
        // Java: the players who target the hidden GM lose the target.
        for player in world.visible_players(gm) {
            if world.target_of(player) == Some(gm) {
                world.set_target(player, None);
            }
        }
    }
    world.broadcast_user_info(gm);
    true
}

/// Java `AdminHide.onCommand`, `admin_hide on|off`.
fn admin_hide(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let usage = "//hide [on|off]";
    match words.get(1).copied() {
        Some("on") => {
            if set_hiding(world, gm, true) {
                world.send_sys_message(gm, "Now, you cannot be seen.");
            } else {
                world.send_sys_message(gm, "Currently, you cannot be seen.");
            }
        }
        Some("off") => {
            if set_hiding(world, gm, false) {
                world.send_sys_message(gm, "Now, you can be seen.");
            } else {
                world.send_sys_message(gm, "Currently, you can be seen.");
            }
        }
        _ => world.send_sys_message(gm, usage),
    }
    Ok(())
}

/// Java `AdminTarget.handleTarget`, `admin_target <name>`: the named player is clicked as if the
/// GM had clicked it. Java's `substring(13)` fails for a command without the name.
fn admin_target(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.chars().count() < 13 {
        world.send_sys_message(gm, "Please specify correct name.");
        return Ok(());
    }
    let name: String = command.chars().skip(13).collect();
    match world.player_by_name(&name) {
        Some(player) => world.on_action_object(gm, player, true),
        None => world.send_sys_message(gm, &format!("Player {name} not found")),
    }
    Ok(())
}

/// Java `AdminTargetSay.onCommand`, `admin_targetsay <text>`: the target says the text to the
/// players around it, as a general chat line for a player and an NPC chat line otherwise.
// NOTE(java-quirk): with no target at all Java's `obj.isCreature()` is a NullPointerException, which
// the framework reports to the GM; the text of that exception is not reproduced exactly here.
fn admin_targetsay(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(obj) = world.target_of(gm) else {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    };
    let kind = world.objects.get(&obj).map(|e| e.kind);
    if kind == Some(ObjectKind::StaticObject) || !kind.is_some_and(|k| k.is_creature()) {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    }
    if command.chars().count() < 16 {
        world.send_sys_message(gm, "Usage: //targetsay <text>");
        return Ok(());
    }
    let message: String = command.chars().skip(16).collect();
    let name = world.object_name(obj).unwrap_or_default();
    let chat_type = if kind == Some(ObjectKind::Player) {
        ChatType::General
    } else {
        ChatType::NpcGeneral
    };
    world.broadcast_creature_packet(
        obj,
        &CreatureSay {
            sender_object_id: obj.0,
            chat_type,
            sender: SenderField::Name(name),
            message_id: -1,
            body: SayBody::Text(message),
        },
    );
    Ok(())
}
