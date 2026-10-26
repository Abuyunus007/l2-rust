//! The vitality admin commands: `//set_vitality <points>`, `//set_vitality_level <0-4>`,
//! `//full_vitality`, `//empty_vitality` and `//get_vitality`, for the targeted player.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminVitality.java

use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::rewards::vitality::{MAX_VITALITY_POINTS, MIN_VITALITY_POINTS, VITALITY_LEVELS};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminVitality.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_set_vitality", admin_vitality),
        ("admin_set_vitality_level", admin_vitality),
        ("admin_full_vitality", admin_vitality),
        ("admin_empty_vitality", admin_vitality),
        ("admin_get_vitality", admin_vitality),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminVitality.onCommand`. The vitality of the target player is changed or shown; the
/// set commands go on with 0 (or level 0) when the number does not parse, as Java does.
fn admin_vitality(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if !world.settings.vitality.enable_vitality {
        world.send_sys_message(gm, "Vitality is not enabled on the server!");
        return Ok(());
    }
    let words = java_split_space(command);
    let cmd = words.first().copied().unwrap_or("");
    let target = world.target_of(gm).filter(|t| world.is_player(*t));
    let Some(target) = target else {
        world.send_sys_message(gm, "Target not found or not a player");
        return Ok(());
    };

    match cmd {
        "admin_set_vitality" => {
            let vitality = match words.get(1).map(|w| parse_int(w)) {
                Some(Ok(value)) => value,
                _ => {
                    world.send_sys_message(gm, "Incorrect vitality");
                    0
                }
            };
            world.set_vitality_points(target, vitality, true);
            world.send_message(target, &format!("Admin set your Vitality points to {vitality}"));
        }
        "admin_set_vitality_level" => {
            // NOTE(java-quirk): a bad level prints "Incorrect vitality level" and still goes on
            // with level 0, which sets the minimum points and says "level 0".
            let level = match words.get(1).map(|w| parse_int(w)) {
                Some(Ok(value)) => value,
                _ => {
                    world.send_sys_message(gm, "Incorrect vitality level (0-4)");
                    0
                }
            };
            if (0..=4).contains(&level) {
                let vitality = if level == 0 {
                    MIN_VITALITY_POINTS
                } else {
                    VITALITY_LEVELS[(level - 1) as usize]
                };
                world.set_vitality_points(target, vitality, true);
                world.send_message(target, &format!("Admin set your Vitality level to {level}"));
            } else {
                world.send_sys_message(gm, "Incorrect vitality level (0-4)");
            }
        }
        "admin_full_vitality" => {
            world.set_vitality_points(target, MAX_VITALITY_POINTS, true);
            world.send_message(target, "Admin completly recharged your Vitality");
        }
        "admin_empty_vitality" => {
            world.set_vitality_points(target, MIN_VITALITY_POINTS, true);
            world.send_message(target, "Admin completly emptied your Vitality");
        }
        "admin_get_vitality" => {
            let level = world.vitality_level(target);
            let points = world.vitality_points(target);
            world.send_sys_message(gm, &format!("Player vitality level: {level}"));
            world.send_sys_message(gm, &format!("Player vitality points: {points}"));
        }
        _ => {}
    }
    Ok(())
}
