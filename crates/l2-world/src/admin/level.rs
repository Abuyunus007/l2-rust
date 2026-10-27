//! The level admin commands: `//add_level <level>` and `//set_level <level>`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminLevel.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminLevel` registrations.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_add_level", admin_add_level),
        ("admin_set_level", admin_set_level),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminLevel.onCommand`: `new StringTokenizer(command, " ")`, the command word, then the
/// first argument (or "").
fn argument(command: &str) -> &str {
    let mut st = Tokens::spaces(command);
    st.next_token();
    st.next_token().unwrap_or("")
}

/// Java `AdminLevel.onCommand`, `admin_add_level`: `PlayerStat.addLevel(Byte.parseByte(val))` on
/// the target when it is playable. The number is only read for a playable target.
fn admin_add_level(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let val = argument(command);
    let Some(target) = world.target_of(gm).filter(|t| world.is_player(*t)) else {
        return Ok(());
    };
    // Java `Byte.parseByte(val)`: a sign and digits, in the range of a byte.
    match val.parse::<i8>() {
        Ok(amount) => {
            world.add_level(target, i32::from(amount));
        }
        Err(_) => world.send_sys_message(gm, "Wrong Number Format"),
    }
    Ok(())
}

/// Java `AdminLevel.onCommand`, `admin_set_level`: the exact level of the target. Its exp is set
/// to the exp of that level: removed when it has more, added when it has less. Then full HP, MP
/// and CP, and the user info.
fn admin_set_level(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let val = argument(command);
    let Some(target) = world.target_of(gm).filter(|t| world.is_player(*t)) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
        return Ok(());
    };
    // `ExperienceData.getMaxLevel()`, or `MaxSubclassLevel` on an active subclass (S-SUB-01).
    let mut max_level = world.exp_table_max_level().unwrap_or(0);
    let max_subclass_level = world.settings.subclass.max_subclass_level;
    if world.is_subclass_active(target) && max_subclass_level < max_level {
        max_level = max_subclass_level;
    }
    let level = match val.parse::<i8>() {
        Ok(level) if (1..=max_level).contains(&i32::from(level)) => i32::from(level),
        _ => {
            world.send_sys_message(gm, &format!("You must specify level between 1 and {max_level}."));
            return Ok(());
        }
    };

    let Some(player_exp) = world.player(target).map(|p| p.data.exp) else {
        return Ok(());
    };
    let Some(target_exp) = world.exp_needed_for_level(level) else {
        return Ok(());
    };
    if player_exp > target_exp {
        world.set_level(target, level);
        world.remove_exp_and_sp(target, player_exp - target_exp, 0, true);
        world.send_sys_message(gm, &format!("Removed {} exp.", player_exp - target_exp));
    } else if player_exp < target_exp {
        // `Player.addExpAndSp(double, double)`: no bonuses. The level follows the exp.
        world.add_exp_and_sp(target, (target_exp - player_exp) as f64, 0.0, false);
        world.send_sys_message(gm, &format!("Added {} exp.", target_exp - player_exp));
    }

    let (max_hp, max_mp, max_cp) = world.stats(target).map_or((0.0, 0.0, 0.0), |v| {
        (
            f64::from(v.max_hp()),
            f64::from(v.max_mp()),
            f64::from(v.max_cp()),
        )
    });
    world.set_current_hp_mp(target, max_hp, max_mp);
    world.set_current_cp(target, max_cp, true);
    world.broadcast_user_info(target);
    Ok(())
}
