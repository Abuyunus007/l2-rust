//! The page commands: `//html <path>` shows a page of `data/html/admin/`, `//loadhtml <path>` a page
//! of the datapack by its full path, and `//help` the help page.
//!
//! Left out, with the system that brings it: `//commands` (the list of every admin command with its
//! description: the descriptions of `AdminCommands.xml` are not kept by the rights table yet) and
//! `//instancezones` (instances are not ported; M8).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminHtml,AdminHelp}.java

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminHtml.ADMIN_COMMANDS` and the help word of `AdminHelp`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_html", admin_html),
        ("admin_loadhtml", admin_loadhtml),
        ("admin_help", admin_help),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The word after the command (Java `new StringTokenizer(command, " ")`: runs of spaces are one
/// separator).
fn argument(command: &str) -> Option<&str> {
    let mut st = Tokens::spaces(command);
    st.next_token();
    st.next_token()
}

/// Java `AdminHtml.onCommand`, `admin_html <path>`: a page of `data/html/admin/`.
fn admin_html(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    match argument(command) {
        Some(path) => world.show_admin_html(gm, path),
        None => world.send_sys_message(gm, "Usage: //html path"),
    }
    Ok(())
}

/// Java `AdminHtml.onCommand`, `admin_loadhtml <path>`: the page at the datapack path, shown as
/// the text of the file (`showHtml(reload = true)`).
// NOTE(java-quirk): Java reads the file again from the disk here. The cache loaded at start has
// the same text unless the file changed while the server runs; the missing page text is Java's
// own for this path (lower case "text").
fn admin_loadhtml(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(path) = argument(command) else {
        world.send_sys_message(gm, "Usage: //loadhtml path");
        return Ok(());
    };
    let html = world
        .get_htm(gm, path)
        .unwrap_or_else(|| format!("<html><body>My text is missing:<br>{path}</body></html>"));
    if let Some(message) = NpcHtmlMessage::new(0, &html) {
        world.send_html(gm, &message);
    }
    Ok(())
}

/// Java `AdminHelp.onCommand`, `admin_help`: `data/html/admin/help.htm`.
fn admin_help(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.show_admin_html(gm, "help.htm");
    Ok(())
}
