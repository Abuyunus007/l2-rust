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
// # The confirmation dialog
// A command with `confirmDlg="true"` typed as `//command` does not run at once: the GM gets a
// `ConfirmDlg` (`S1_3`, "Are you sure you want execute command '...' ?") and the command text
// waits in `AdminWorld::pending_confirm`. The `DlgAnswer` that comes back with the same message id
// runs it with the confirmation switched off ([`World::admin_confirm_answer`], called by
// `World::on_dlg_answer`). The `admin_` bypasses of the html panels ask the same way; only the
// run after the answer does not ask again.
//
// # What is left out, with the system that brings it
// - Java runs a command on a pool thread and warns when it took more than five seconds; the
//   world runs it inside its own turn.
// - `SendMessageLocalisationData` (translated GM messages): only with multi-language.

pub mod access_level;
pub mod admin_page;
pub mod announce;
pub mod augment;
pub mod camera;
pub mod chsiege;
pub mod clan;
pub mod clan_skills;
pub mod clanhall;
pub mod combat;
pub mod commands;
pub mod config_page;
pub mod control;
pub mod create_item;
pub mod debug;
pub mod delete;
pub mod destroy_items;
pub mod door;
pub mod edit_char;
pub mod effects;
pub mod element;
pub mod enchant;
pub mod exp_sp;
pub mod fence;
pub mod fight_calculator;
pub mod flags;
pub mod forge;
pub mod geodata;
pub mod gm;
pub mod gm_shop;
pub mod gm_tools;
pub mod gm_view;
pub mod html;
pub mod invul;
pub mod karma;
pub mod level;
pub mod manor;
pub mod menu;
pub mod messages;
pub mod page;
pub mod party;
pub mod pccafe;
pub mod petition;
pub mod punishments;
pub mod reload;
pub mod repair;
pub mod rights;
pub mod scan;
pub mod search;
pub mod skill_test;
pub mod skills;
pub mod sound;
pub mod spawn_admin;
pub mod summon;
pub mod teleport;
pub mod transform;
pub mod vitality;
pub mod zone_tools;

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use l2_data::ClassList;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, ConfirmDlg, CreatureSay, SayBody, SenderField, SystemMessageParam};
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
    /// `ClassListData`: the class names the admin pages show.
    pub(crate) class_list: Arc<ClassList>,
    /// `Player.getAdminConfirmCmd()` of each GM who waits for the answer of a `ConfirmDlg`
    /// (Java also sets `PlayerAction.ADMIN_COMMAND`; the entry is that action).
    pub(crate) pending_confirm: HashMap<ObjectId, String>,
    /// `AdminSkill.adminSkills`: the skills a GM had before `//get_skills`, for `//reset_skills`.
    /// Java keeps one such list for the whole server, so this is one list too.
    pub(crate) skill_stash: Option<Vec<(i32, i32)>>,
    /// The state of the server control, GM tool, announcement, petition and debug handlers
    /// (M7-ADMIN-10, see `control.rs`).
    pub(crate) ctl: control::ControlWorld,
    /// `AdminDoorControl.PLAYER_SHOWN_DOORS`: the door ids each GM's `//showdoors` has drawn.
    pub(crate) shown_doors: HashMap<ObjectId, std::collections::BTreeSet<i32>>,
    /// Java `CreatureStat._gmSpeedMultiplier` of each creature `//gmspeed` changed (`gm.rs`).
    pub(crate) gm_speed: HashMap<ObjectId, f64>,
    /// The zone builder and the zone markers (`zone_tools.rs`).
    pub(crate) zones: zone_tools::ZoneAdmin,
    /// The path search of `//path_find` (`geodata.rs`).
    pub(crate) path: geodata::PathAdmin,
}

impl Default for AdminWorld {
    fn default() -> Self {
        let mut handlers = AdminHandlers::default();
        commands::register_core(&mut handlers);
        effects::register(&mut handlers);
        menu::register(&mut handlers);
        teleport::register(&mut handlers);
        gm::register(&mut handlers);
        camera::register(&mut handlers);
        page::register(&mut handlers);
        messages::register(&mut handlers);
        vitality::register(&mut handlers);
        augment::register(&mut handlers);
        element::register(&mut handlers);
        clan_skills::register(&mut handlers);
        pccafe::register(&mut handlers);
        combat::register(&mut handlers);
        exp_sp::register(&mut handlers);
        invul::register(&mut handlers);
        create_item::register(&mut handlers);
        edit_char::register(&mut handlers);
        door::register(&mut handlers);
        destroy_items::register(&mut handlers);
        gm_shop::register(&mut handlers);
        delete::register(&mut handlers);
        scan::register(&mut handlers);
        search::register(&mut handlers);
        fence::register(&mut handlers);
        fight_calculator::register(&mut handlers);
        flags::register(&mut handlers);
        config_page::register(&mut handlers);
        access_level::register(&mut handlers);
        repair::register(&mut handlers);
        skill_test::register(&mut handlers);
        admin_page::register(&mut handlers);
        spawn_admin::register(&mut handlers);
        enchant::register(&mut handlers);
        level::register(&mut handlers);
        skills::register(&mut handlers);
        punishments::register(&mut handlers);
        clan::register(&mut handlers);
        karma::register(&mut handlers);
        party::register(&mut handlers);
        summon::register(&mut handlers);
        transform::register(&mut handlers);
        crate::siege::register_admin(&mut handlers);
        crate::fort::register_admin(&mut handlers);
        crate::territory::register_admin(&mut handlers);
        chsiege::register(&mut handlers);
        clanhall::register(&mut handlers);
        manor::register(&mut handlers);
        geodata::register(&mut handlers);
        zone_tools::register(&mut handlers);
        // M7-ADMIN-10: server control, GM tools, announcements, petitions, reload, forge, debug.
        control::register(&mut handlers);
        gm_tools::register(&mut handlers);
        announce::register(&mut handlers);
        petition::register(&mut handlers);
        reload::register(&mut handlers);
        forge::register(&mut handlers);
        debug::register(&mut handlers);
        Self {
            handlers,
            class_list: Arc::new(ClassList::default()),
            pending_confirm: HashMap::new(),
            skill_stash: None,
            ctl: control::ControlWorld::default(),
            shown_doors: HashMap::new(),
            gm_speed: HashMap::new(),
            zones: zone_tools::ZoneAdmin::default(),
            path: geodata::PathAdmin::default(),
        }
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
    /// Installs `ClassListData` (`classList.xml`), whose names the admin pages show. Call before
    /// players enter; without it a page that names a class is Java's `NullPointerException`.
    pub fn set_class_list(&mut self, class_list: Arc<ClassList>) {
        self.admin.class_list = class_list;
    }

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
            // Java: `setAdminConfirmCmd`, `ConfirmDlg(S1_3)` with the question as its string,
            // `addAction(ADMIN_COMMAND)`.
            self.admin.pending_confirm.insert(id, full_command.to_owned());
            let dialog = ConfirmDlg::new(SystemMessageId::S13).with(SystemMessageParam::Text(format!(
                "Are you sure you want execute command '{command_no_prefix}' ?"
            )));
            self.send(id, &dialog);
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

    /// Java `DlgAnswer.runImpl`, the `S1_3` branch `removeAction(PlayerAction.ADMIN_COMMAND)`: the
    /// answer to the confirmation of an admin command. A yes runs the command again with the
    /// confirmation off; a no forgets it. Without a waiting command nothing happens.
    pub(crate) fn admin_confirm_answer(&mut self, player: ObjectId, answer: i32) {
        // `PlayerAction.ADMIN_SAVE_ZONE` is checked before `ADMIN_COMMAND`.
        if self.zone_save_answer(player, answer) {
            return;
        }
        let Some(command) = self.admin.pending_confirm.remove(&player) else {
            return;
        };
        if answer == 0 {
            return;
        }
        // The 'useConfirm' must be disabled here, as we don't want to repeat that process.
        self.admin_command(player, &command, false);
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
