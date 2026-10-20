//! The admin exp and sp commands: `//add_exp_sp exp sp`, `//remove_exp_sp exp sp` and the menu
//! word `//add_exp_sp_to_character`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminExpSp.java
//
// The handler's own part is here: the target check, the two numbers, the messages to the target
// and to the GM. The change of the target's exp and sp is `Player.addExpAndSp` (M4-EXP-01,
// `World::add_exp_and_sp`), and `//add_exp_sp` clamps the sp to `MaxSp` (`settings.exp.max_sp`).
// The menu `data/html/admin/expsp.htm` follows every command of this handler, with the class name
// of `ClassListData` (`World::set_class_list`).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::skills::player_class_name;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminExpSp` registrations.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_add_exp_sp_to_character", admin_exp_sp),
        ("admin_add_exp_sp", admin_exp_sp),
        ("admin_remove_exp_sp", admin_exp_sp),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `Long.parseLong(text)`: an optional sign and at least one digit, nothing else.
pub(crate) fn parse_long(text: &str) -> Option<i64> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

/// Java `AdminExpSp.onCommand`. The menu page is shown after every branch, as in Java.
fn admin_exp_sp(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_add_exp_sp") {
        // `command.substring(16)`: the text after the command word.
        let val = command.get(16..).unwrap_or("");
        if !add_exp_sp(world, gm, val) {
            world.send_sys_message(gm, "Usage: //add_exp_sp exp sp");
        }
    } else if command.starts_with("admin_remove_exp_sp") {
        // `command.substring(19)`.
        let val = command.get(19..).unwrap_or("");
        if !remove_exp_sp(world, gm, val) {
            world.send_sys_message(gm, "Usage: //remove_exp_sp exp sp");
        }
    }
    show_menu(world, gm)
}

/// The target of the handler: a player, or Java's `INVALID_TARGET` (and `false`).
fn target_player(world: &World, gm: ObjectId) -> Option<ObjectId> {
    let target = world.target_of(gm).filter(|t| world.is_player(*t));
    if target.is_none() {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
    }
    target
}

/// The two numbers of `expSp`: `new StringTokenizer(expSp)` with exactly two tokens.
fn two_tokens(text: &str) -> Option<(&str, &str)> {
    let mut st = Tokens::new(text);
    let first = st.next_token()?;
    let second = st.next_token()?;
    if st.has_more() {
        return None;
    }
    Some((first, second))
}

/// Java `AdminExpSp.adminAddExpSp(activeChar, expSp)`.
fn add_exp_sp(world: &mut World, gm: ObjectId, exp_sp: &str) -> bool {
    let Some(player) = target_player(world, gm) else {
        return false;
    };
    let Some((exp, sp)) = two_tokens(exp_sp) else {
        return false;
    };
    let (Some(expval), Some(spval)) = (parse_long(exp), parse_long(sp)) else {
        return false;
    };
    // `Math.min(Long.parseLong(sp), PlayerConfig.MAX_SP)`.
    let spval = spval.min(world.settings.exp.max_sp);

    if expval != 0 || spval != 0 {
        let name = world.object_name(player).unwrap_or_default();
        // Common character information.
        world.send_message(
            player,
            &format!("Admin is adding you {expval} xp and {spval} sp."),
        );
        // `player.addExpAndSp(expval, spval)`: `Player.addExpAndSp(double, double)` uses no bonuses.
        world.add_exp_and_sp(player, expval as f64, spval as f64, false);
        // Admin information.
        world.send_sys_message(gm, &format!("Added {expval} xp and {spval} sp to {name}."));
    }
    true
}

/// Java `AdminExpSp.adminRemoveExpSP(activeChar, expSp)`.
fn remove_exp_sp(world: &mut World, gm: ObjectId, exp_sp: &str) -> bool {
    let Some(player) = target_player(world, gm) else {
        return false;
    };
    let Some((exp, sp)) = two_tokens(exp_sp) else {
        return false;
    };
    let (Some(expval), Some(spval)) = (parse_long(exp), parse_long(sp)) else {
        return false;
    };

    if expval != 0 || spval != 0 {
        let name = world.object_name(player).unwrap_or_default();
        // Common character information.
        world.send_message(
            player,
            &format!("Admin is removing you {expval} xp and {spval} sp."),
        );
        // `player.removeExpAndSp(expval, spval)`: `sendMessage` is true, so the target is told
        // the amounts again by the two `YOUR_..._DECREASED` messages.
        world.remove_exp_and_sp(player, expval, spval, true);
        // Admin information.
        world.send_sys_message(gm, &format!("Removed {expval} xp and {spval} sp from {name}."));
    }
    true
}

/// Java `AdminExpSp.addExpSp(activeChar)`: the menu of the target (`expsp.htm`), or
/// `INVALID_TARGET`. A class the class list lacks is Java's `NullPointerException`.
fn show_menu(world: &mut World, gm: ObjectId) -> Outcome {
    let Some(player) = target_player(world, gm) else {
        return Ok(());
    };
    let name = world.object_name(player).unwrap_or_default();
    let (level, exp, sp) = world
        .player(player)
        .map_or((0, 0, 0), |p| (p.data.level, p.data.exp, p.data.sp));
    let class = player_class_name(world, player)?;
    let mut html = world.admin_html_file(gm, "data/html/admin/expsp.htm");
    html.replace("%name%", &name);
    html.replace("%level%", &level.to_string());
    html.replace("%xp%", &exp.to_string());
    html.replace("%sp%", &sp.to_string());
    html.replace("%class%", &class);
    world.send_admin_html(gm, &html);
    Ok(())
}
