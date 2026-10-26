//! The GM shop admin commands: `//gmshop` (the page of the shops) and `//buy <listId>` (the buy
//! list of a shop, opened for the GM).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminShop.java
//
// The buy list is built like the NPC shops build it (`shop/packets.rs`), with no tax. An id that
// is not a number, or a list that does not exist, sends only the `ActionFailed` (Java logs it).

use l2_protocol_game::server::ActionFailed;
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminShop.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_buy", admin_buy), ("admin_gmshop", admin_gmshop)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminShop.onCommand`, `admin_gmshop`: `gmshops.htm`.
fn admin_gmshop(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.show_admin_html(gm, "gmshops.htm");
    Ok(())
}

/// Java `AdminShop.onCommand`, `admin_buy <listId>`: `command.substring(10)`, so a command with no
/// text after `admin_buy` is the "Please specify buylist." message.
fn admin_buy(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(value) = command.get(10..) else {
        world.send_sys_message(gm, "Please specify buylist.");
        return Ok(());
    };
    // `Integer.parseInt(command)`: a value that is not a number is -1, which no list has.
    let list_id = parse_int(value).unwrap_or(-1);
    if let Some(packet) = world.buy_list_packet(gm, list_id, 0.0) {
        world.send(gm, &packet);
        world.send_ex_buy_sell_list(gm, false);
    }
    world.send(gm, &ActionFailed);
    Ok(())
}
