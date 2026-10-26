//! `//pccafepoints <set|increase|decrease|rewardOnline> <points> [range]`: the PC cafe points of the
//! targeted player (the GM when there is no target), and the menu `pccafe.htm` that shows them.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminPcCafePoints.java

use l2_protocol_game::server::ExPcCafePointInfo;
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::residence::npc::format_adena;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminPcCafePoints.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    handlers.register("admin_pccafepoints", admin_pccafepoints as AdminHandler);
}

/// Java `AdminPcCafePoints.getTarget`: the targeted player, else the GM.
fn target_or_gm(world: &World, gm: ObjectId) -> ObjectId {
    world.target_of(gm).filter(|t| world.is_player(*t)).unwrap_or(gm)
}

/// Java `AdminPcCafePoints.showMenuHtml`: `pccafe.htm` with the points and the name of the target.
/// Java builds it with `NpcHtmlMessage(0, 1)`: the item id is 1 for this page.
fn show_menu_html(world: &mut World, gm: ObjectId) {
    let target = target_or_gm(world, gm);
    let points = world.pc_cafe_points(target);
    let name = world.object_name(target).unwrap_or_default();
    let mut page = world.admin_html_file(gm, "data/html/admin/pccafe.htm");
    page.replace("%points%", &format_adena(i64::from(points)))
        .replace("%targetName%", &name);
    let mut message = page.message().clone();
    message.item_id = 1;
    world.send_html(gm, &message);
}

/// Java `AdminPcCafePoints.onCommand`, `admin_pccafepoints [action target value]`. The menu is
/// shown at the end of every branch that gets that far; a failed branch returns before it.
fn admin_pccafepoints(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    if words.len() > 1 {
        let action = words[1];
        let target = target_or_gm(world, gm);
        // Java: `if ((target == null) || !st.hasMoreTokens()) return false;`
        let Some(value_text) = words.get(2) else {
            return Ok(());
        };
        let Ok(mut value) = parse_int(value_text) else {
            show_menu_html(world, gm);
            world.send_sys_message(gm, "Invalid Value!");
            return Ok(());
        };
        let max = world.pc_cafe_max_points();
        let name = world.object_name(target).unwrap_or_default();
        match action {
            "set" => {
                if value > max {
                    show_menu_html(world, gm);
                    world.send_sys_message(gm, &format!("You cannot set more than {max} PC points!"));
                    return Ok(());
                }
                if value < 0 {
                    value = 0;
                }
                world.set_pc_cafe_points(target, value);
                world.send_message(target, &format!("Admin set your PC Cafe point(s) to {value}!"));
                world.send_sys_message(gm, &format!("You set {value} PC Cafe point(s) to player {name}"));
                world.send(target, &ExPcCafePointInfo::new(value, value, 0));
            }
            "increase" => {
                if world.pc_cafe_points(target) == max {
                    show_menu_html(world, gm);
                    world.send_message(gm, &format!("{name} already have max count of PC points!"));
                    return Ok(());
                }
                let count = clamp_increase(world.pc_cafe_points(target), value, max);
                world.set_pc_cafe_points(target, count);
                world.send_message(
                    target,
                    &format!("Admin increased your PC Cafe point(s) by {value}!"),
                );
                world.send_sys_message(
                    gm,
                    &format!("You increased PC Cafe point(s) of {name} by {value}"),
                );
                world.send(target, &ExPcCafePointInfo::new(count, value, 0));
            }
            "decrease" => {
                if world.pc_cafe_points(target) == 0 {
                    show_menu_html(world, gm);
                    world.send_message(gm, &format!("{name} already have min count of PC points!"));
                    return Ok(());
                }
                // Java `Math.max(points - value, 0)`, with the int arithmetic of Java.
                let count = world.pc_cafe_points(target).wrapping_sub(value).max(0);
                world.set_pc_cafe_points(target, count);
                world.send_message(
                    target,
                    &format!("Admin decreased your PC Cafe point(s) by {value}!"),
                );
                world.send_sys_message(
                    gm,
                    &format!("You decreased PC Cafe point(s) of {name} by {value}"),
                );
                let now = world.pc_cafe_points(target);
                world.send(target, &ExPcCafePointInfo::new(now, -value, 0));
            }
            "rewardOnline" => {
                // Java: an unparsable range is 0 (the whole server).
                let range = words.get(3).and_then(|w| parse_int(w).ok()).unwrap_or(0);
                if range <= 0 {
                    let players: Vec<ObjectId> = world.players.keys().copied().collect();
                    let count = increase_for_all(world, &players, value);
                    world.send_sys_message(
                        gm,
                        &format!(
                            "You increased PC Cafe point(s) of all online players ({count}) by {value}."
                        ),
                    );
                } else {
                    let players: Vec<ObjectId> = world
                        .visible_objects_in_range(gm, range)
                        .into_iter()
                        .filter(|o| world.is_player(*o))
                        .collect();
                    let count = increase_for_all(world, &players, value);
                    world.send_sys_message(
                        gm,
                        &format!(
                            "You increased PC Cafe point(s) of all players ({count}) in range {range} by {value}."
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    show_menu_html(world, gm);
    Ok(())
}

/// Java `Math.min(points + value, max)` with int overflow; a negative result is `max`.
fn clamp_increase(points: i32, value: i32, max: i32) -> i32 {
    let count = points.wrapping_add(value).min(max);
    if count < 0 { max } else { count }
}

/// Java `AdminPcCafePoints.increaseForAll`: each player whose points are not already at the int
/// maximum gets `value` more. Returns how many did.
// NOTE(java-quirk): Java caps this at `Integer.MAX_VALUE`, not at the configured maximum.
fn increase_for_all(world: &mut World, players: &[ObjectId], value: i32) -> usize {
    let mut counter = 0;
    for player in players {
        if !world.players.contains_key(player) {
            continue;
        }
        let points = world.pc_cafe_points(*player);
        if points == i32::MAX {
            continue;
        }
        let count = points.wrapping_add(value);
        let count = if count < 0 { i32::MAX } else { count };
        world.set_pc_cafe_points(*player, count);
        world.send_message(
            *player,
            &format!("Admin increased your PC Cafe point(s) by {value}!"),
        );
        world.send(*player, &ExPcCafePointInfo::new(count, value, 0));
        counter += 1;
    }
    counter
}
