//! The page commands: `//html <path>` shows a page of `data/html/admin/`, `//loadhtml <path>` a page
//! of the datapack by its full path, and `//help` the help page.
//!
//! `//commands [page=N]` lists every admin command of the rights table with its description, five
//! per page. `//instancezones` is in `admin/instance.rs` (M8-INST-01).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminHtml,AdminHelp}.java

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;

use super::html::build_page;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::outbox::ControlRequest;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminHtml.ADMIN_COMMANDS` and the help words of `AdminHelp`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_html", admin_html),
        ("admin_loadhtml", admin_loadhtml),
        ("admin_help", admin_help),
        ("admin_commands", admin_commands),
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
// NOTE(java-quirk): Java reads the file again from the disk here and shows that text (and keeps
// it in its cache; the Rust cache is not changed). The file is read by the game server task (D-15,
// no file IO inside a tick): the page is sent when `WorldCommand::HtmlFileLoaded` comes back. The
// missing page text is Java's own for this path (lower case "text").
fn admin_loadhtml(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(path) = argument(command) else {
        world.send_sys_message(gm, "Usage: //loadhtml path");
        return Ok(());
    };
    world.outbox.control(ControlRequest::LoadHtmlFile {
        requester: gm,
        path: path.to_owned(),
    });
    Ok(())
}

impl World {
    /// The answer to `ControlRequest::LoadHtmlFile`: Java `showHtml(player, path, true)` ends here.
    pub(crate) fn on_html_file_loaded(&mut self, requester: ObjectId, path: &str, content: Option<String>) {
        if !self.players.contains_key(&requester) {
            return;
        }
        let html =
            content.unwrap_or_else(|| format!("<html><body>My text is missing:<br>{path}</body></html>"));
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(requester, &message);
        }
    }
}

/// Java `AdminHelp.onCommand`, `admin_help`: `data/html/admin/help.htm`.
fn admin_help(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.show_admin_html(gm, "help.htm");
    Ok(())
}

/// Java `AdminHelp.onCommand`, `admin_commands [page=N]`: the rows of the rights table, five per
/// page, in `commands.htm`. The page number is `Integer.parseInt` (a bad one is the exception).
// NOTE(java-quirk): Java cuts the page from `substring(15)` after a plain `contains("page")`, and
// its default page is 1, which `PageBuilder` counts from 0: `//commands` alone shows the second
// page of five, and `page=0` the first.
fn admin_commands(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let page = if command.contains("page") {
        let text: String = command.chars().skip(15).collect();
        let text = text.replace("page=", "");
        parse_int(&text).map_err(|_| JavaException::number_format(&text))?
    } else {
        1
    };
    let rows = world
        .data
        .admin_rights()
        .map(|rights| rights.command_rows())
        .unwrap_or_default();
    let result = build_page(
        &rows,
        5,
        "bypass -h admin_commands",
        page,
        |(full, description), sb| {
            let name = full.get(6..).unwrap_or("");
            sb.push_str(&format!(
            "<tr><td height=40 width=400><font color=\"LEVEL\">//{name}</font></td><td height=40 width=400>{description}</td></tr>"
        ));
        },
    );
    let mut html = world.admin_html_file(gm, "data/html/admin/help/commands.htm");
    html.replace("%pages%", &result.pager_html())
        .replace("%list%", &result.body);
    world.send_admin_html(gm, &html);
    Ok(())
}
