//! The level admin command: `//add_level <level>`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminLevel.java
//
// # Left out, with the system that brings it
// - `//set_level <level>` is not registered, so the GM is told the command does not exist. Java
//   sets the level with `PlayerStat.setLevel`, then `removeExpAndSp` / `addExpAndSp` for the exp
//   difference: `setLevel` and `removeExpAndSp` are not in EXP-01's port (FOLLOW-UP(M4)).

use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminLevel` registrations.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_add_level", admin_add_level)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminLevel.onCommand`, `admin_add_level`: `PlayerStat.addLevel(Byte.parseByte(val))` on
/// the target when it is playable. The number is only read for a playable target.
fn admin_add_level(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `new StringTokenizer(command, " ")`: the command word, then the first argument (or "").
    let mut st = Tokens::spaces(command);
    st.next_token();
    let val = st.next_token().unwrap_or("");

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
