//! The admin command framework: `//command` from the client and `admin_` bypasses from the
//! admin html panels, the access checks, the GM audit line, and the handler registry.
// Java: network/clientpackets/SendBypassBuildCmd.java, handler/AdminCommandHandler.java
// Java: network/clientpackets/RequestBypassToServer.java (the `admin_` branch)
// Java: util/GMAudit.java, entity/actor/Player.java (sendMessage, sendSysMessage)
//
// # How a command runs
// `//spawn 20101` arrives as `SendBypassBuildCmd("spawn 20101")`; the world turns it into
// `admin_spawn 20101` and runs it like a bypass of the admin panel does
// ([`World::admin_command`]): only GMs (`isGM` of the access level), the command must have a
// handler and the character's level must have the right for it (`AdminData.hasAccess`).
//
// # What is left out, with the system that brings it
// - The `ConfirmDlg` round trip for commands with `confirmDlg="true"` (none of the ported
//   commands has it): the GM is told so and nothing runs.
// - Java runs a command on a pool thread and warns when it took more than five seconds; the
//   world runs it inside its own turn.
// - `SendMessageLocalisationData` (translated GM messages): only with multi-language.

pub mod combat;
pub mod commands;
pub mod exp_sp;
pub mod rights;

use std::collections::HashMap;
use std::fmt;

use l2_protocol_game::server::{ChatType, CreatureSay, SayBody, SenderField};
use l2_util::ObjectId;
use tracing::{info, warn};

pub use rights::AdminRights;

use crate::world::World;

/// The text of a Java exception (`Throwable.toString()`) a handler would have thrown. The
/// dispatcher tells the GM, as Java does for a `RuntimeException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaException(pub String);

impl fmt::Display for JavaException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl JavaException {
    /// `java.lang.NumberFormatException` of `Integer.parseInt(text)`.
    pub fn number_format(text: &str) -> Self {
        Self(format!(
            "java.lang.NumberFormatException: For input string: \"{text}\""
        ))
    }
}

/// An admin command handler (Java `IAdminCommandHandler.onCommand`): the world, the GM and the
/// whole command text (`admin_spawn 20101`).
pub type AdminHandler = fn(&mut World, ObjectId, &str) -> Result<(), JavaException>;

/// Java `AdminCommandHandler`: full command word (`admin_kill`, case sensitive) to handler.
#[derive(Default)]
pub struct AdminHandlers {
    handlers: HashMap<String, AdminHandler>,
}

impl AdminHandlers {
    /// Java `registerHandler` for one command word.
    pub fn register(&mut self, command: &str, handler: AdminHandler) {
        self.handlers.insert(command.to_owned(), handler);
    }

    /// Java `getHandler(adminCommand)`: the handler of the command's first word.
    pub fn get(&self, command: &str) -> Option<AdminHandler> {
        let word = command.split(' ').next().unwrap_or(command);
        self.handlers.get(word).copied()
    }

    /// Java `size()`.
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

/// The admin state of the world: the handlers (the core commands are registered from the
/// start, like `MasterHandler` does).
pub(crate) struct AdminWorld {
    pub(crate) handlers: AdminHandlers,
}

impl Default for AdminWorld {
    fn default() -> Self {
        let mut handlers = AdminHandlers::default();
        commands::register_core(&mut handlers);
        combat::register(&mut handlers);
        exp_sp::register(&mut handlers);
        Self { handlers }
    }
}

/// Java `StringTokenizer` with the default delimiters (`" \t\n\r\f"`) or a given set: runs of
/// delimiters separate tokens, empty tokens do not exist.
pub(crate) struct Tokens<'a> {
    rest: &'a str,
    delimiters: &'a [char],
}

const DEFAULT_DELIMITERS: &[char] = &[' ', '\t', '\n', '\r', '\u{c}'];

impl<'a> Tokens<'a> {
    /// `new StringTokenizer(text)`.
    pub(crate) fn new(text: &'a str) -> Self {
        Self {
            rest: text,
            delimiters: DEFAULT_DELIMITERS,
        }
    }

    /// `new StringTokenizer(text, " ")`.
    pub(crate) fn spaces(text: &'a str) -> Self {
        Self {
            rest: text,
            delimiters: &[' '],
        }
    }

    /// `hasMoreTokens()`.
    pub(crate) fn has_more(&self) -> bool {
        self.rest.chars().any(|c| !self.delimiters.contains(&c))
    }

    /// `nextToken()`; `None` is Java's `NoSuchElementException`.
    pub(crate) fn next_token(&mut self) -> Option<&'a str> {
        let trimmed = self.rest.trim_start_matches(|c| self.delimiters.contains(&c));
        if trimmed.is_empty() {
            self.rest = trimmed;
            return None;
        }
        let end = trimmed
            .find(|c| self.delimiters.contains(&c))
            .unwrap_or(trimmed.len());
        let (token, rest) = trimmed.split_at(end);
        self.rest = rest;
        Some(token)
    }
}

/// Java `Integer.parseInt(text)`: an optional sign and at least one digit, nothing else.
pub(crate) fn parse_int(text: &str) -> Result<i32, JavaException> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(JavaException::number_format(text));
    }
    text.parse::<i32>()
        .map_err(|_| JavaException::number_format(text))
}

/// Java `StringUtil.isNumeric(text)`: not empty, every character a digit.
pub(crate) fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

impl World {
    /// Registers an admin command handler (Java `AdminCommandHandler.registerHandler`). `command`
    /// is the full word, `admin_` prefix included. A later registration replaces an earlier one.
    pub fn register_admin_handler(&mut self, command: &str, handler: AdminHandler) {
        self.admin.handlers.register(command, handler);
    }

    /// Java `Player.sendSysMessage(text)`: a system message, or a "SYS" line in the chat window
    /// with `GMStartupBuilderHide`.
    pub(crate) fn send_sys_message(&self, to: ObjectId, text: &str) {
        if self.settings.gm_startup_builder_hide {
            self.send(
                to,
                &CreatureSay {
                    sender_object_id: 0,
                    chat_type: ChatType::General,
                    sender: SenderField::Name("SYS".to_owned()),
                    message_id: -1,
                    body: SayBody::Text(text.to_owned()),
                },
            );
        } else {
            self.send_message(to, text);
        }
    }

    /// Java `World.getPlayer(name)`: an online character by name, ignoring case.
    pub(crate) fn player_by_name(&self, name: &str) -> Option<ObjectId> {
        let wanted = name.to_lowercase();
        self.players
            .values()
            .find(|p| p.data.name.to_lowercase() == wanted)
            .map(|p| p.core.object_id)
    }

    /// Java `WorldObject.getName()` for the objects an admin can target.
    pub(crate) fn object_name(&self, id: ObjectId) -> Option<String> {
        if let Some(player) = self.players.get(&id) {
            return Some(player.data.name.clone());
        }
        let npc = self.npcs.get(&id)?;
        self.npc_world.template(npc.npc_id).map(|t| t.name.clone())
    }

    /// Java `SendBypassBuildCmd.runImpl`: `//command` is `admin_command` with a confirmation.
    pub(crate) fn on_build_cmd(&mut self, id: ObjectId, command: &str) {
        if !self.players.contains_key(&id) {
            return;
        }
        self.admin_command(id, &format!("admin_{command}"), true);
    }

    /// Java `AdminCommandHandler.onCommand(player, fullCommand, useConfirm)`.
    pub fn admin_command(&mut self, id: ObjectId, full_command: &str, use_confirm: bool) {
        if !self.is_gm(id) {
            return;
        }
        let Some(name) = self.players.get(&id).map(|p| p.data.name.clone()) else {
            return;
        };
        let level = self.players.get(&id).map_or(0, |p| p.data.access_level);

        let command = full_command.split(' ').next().unwrap_or(full_command);
        let command_no_prefix = command.get(6..).unwrap_or("");
        let Some(handler) = self.admin.handlers.get(command) else {
            self.send_message(id, &format!("The command '{command_no_prefix}' does not exist!"));
            warn!("No handler registered for admin command '{command}'");
            return;
        };

        let has_access = self
            .data
            .admin_rights()
            .is_some_and(|rights| rights.has_access(command, level));
        if !has_access {
            self.send_message(id, "You don't have the access rights to use this command!");
            warn!("{name} tried to use admin command '{command}', without proper access level!");
            return;
        }

        let needs_confirm = use_confirm
            && self
                .data
                .admin_rights()
                .is_some_and(|rights| rights.requires_confirm(command, level));
        if needs_confirm {
            // FOLLOW-UP: `ConfirmDlg` + `RequestDlgAnswer` (`PlayerAction.ADMIN_COMMAND`).
            warn!("{name}: admin command '{command}' needs a confirmation dialog, which is not ported yet");
            self.send_message(
                id,
                &format!("The command '{command_no_prefix}' needs a confirmation dialog, which is not available yet."),
            );
            return;
        }

        if self.settings.gm_audit {
            let target = self
                .target_of(id)
                .and_then(|t| self.object_name(t))
                .unwrap_or_else(|| "no-target".to_owned());
            // Java `GMAudit.logAction(name [objectId], command, target)`; the file target
            // `log/GMAudit/<name>.txt` comes with the logging card (D-25).
            info!(target: "gmaudit", "{name} [{}]>{full_command}>{target}>", id.0);
        }

        if let Err(exception) = handler(self, id, full_command) {
            self.send_message(
                id,
                &format!("Exception during execution of  '{full_command}': {exception}"),
            );
            warn!("Exception during execution of {full_command}: {exception}");
        }
    }

    /// Java `AdminHtml.showAdminHtml(player, path)`: a page of `data/html/admin/`, or Java's
    /// "text is missing" page.
    pub(crate) fn show_admin_html(&mut self, id: ObjectId, path: &str) {
        let full = format!("data/html/admin/{path}");
        let html = self
            .get_htm(id, &full)
            .unwrap_or_else(|| format!("<html><body>My text is missing:<br>{full}</body></html>"));
        if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(0, &html) {
            self.send_html(id, &message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_skip_runs_of_delimiters() {
        let mut t = Tokens::new("  a \t b  ");
        assert!(t.has_more());
        assert_eq!(t.next_token(), Some("a"));
        assert_eq!(t.next_token(), Some("b"));
        assert!(!t.has_more());
        assert_eq!(t.next_token(), None);
        // The space-only tokenizer keeps a tab inside a token.
        let mut t = Tokens::spaces("x\ty  z");
        assert_eq!(t.next_token(), Some("x\ty"));
        assert_eq!(t.next_token(), Some("z"));
    }

    #[test]
    fn parse_int_is_java_parse_int() {
        assert_eq!(parse_int("12").unwrap(), 12);
        assert_eq!(parse_int("-3").unwrap(), -3);
        assert_eq!(parse_int("+7").unwrap(), 7);
        for bad in ["", "-", "1 2", " 1", "1.5", "99999999999", "0x10"] {
            assert_eq!(
                parse_int(bad).unwrap_err().0,
                format!("java.lang.NumberFormatException: For input string: \"{bad}\"")
            );
        }
    }

    #[test]
    fn is_numeric_needs_a_digit() {
        assert!(is_numeric("007"));
        assert!(!is_numeric(""));
        assert!(!is_numeric("-1"));
    }
}
