//! `RequestBypassToServer` and `RequestLinkHtml`: the commands of html links, and the handler
//! registry they go through.
// Java: network/clientpackets/{RequestBypassToServer,RequestLinkHtml}.java
// Java: handler/BypassHandler.java (the registry: first word, lower case)
// Java: dist/game/data/scripts/handlers/bypass/npc/{Link,ChatLink,SkillList}.java
// Java: entity/actor/Npc.java (onBypassFeedback)
// Java: util/LocationUtil.java (isInsideRangeOfObjectId)
//
// # The registry
// A handler is a function `fn(&mut World, player, command, target) -> bool` registered under the
// first word of the commands it takes (D-05: scripts come last, so only the handlers the M3 cards
// name exist; the API can still change). The two the dialog needs, `Link` and `Chat`, are
// registered by default ([`BypassHandlers::default`]). A command nobody handles is logged and
// ignored, as Java does.
//
// # What is left out, with the system that brings it
// - The community board, `come_here`, `item_` item scripts, `_match`, `_diary`, `_olympiad`,
//   `manor_menu_select`, `report` (captcha): later cards. They are accepted by the validation
//   and then only logged.
// - `admin_` commands do not use this registry: they go to the `admin` module
//   ([`World::admin_command`]), as Java hands them to `AdminCommandHandler`.
// - The flood protectors (`canUseServerBypass`) and the `ON_PLAYER_BYPASS` script event.

use std::collections::HashMap;

use l2_protocol_game::server::ActionFailed;
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use crate::command::LeaveReason;
use crate::dialog::INTERACTION_DISTANCE;
use crate::target::{distance_2d, distance_3d};
use crate::world::World;

/// A bypass command handler (Java `IBypassHandler.onCommand`): the world, the player, the whole
/// command and the creature the html belongs to (`None` when it is not bound to one).
pub type BypassHandler = fn(&mut World, ObjectId, &str, Option<ObjectId>) -> bool;

/// Java `BypassHandler`: command word (lower case) to handler.
pub struct BypassHandlers {
    handlers: HashMap<String, BypassHandler>,
}

impl BypassHandlers {
    /// No handlers at all.
    pub fn empty() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Java `registerHandler`.
    pub fn register(&mut self, command: &str, handler: BypassHandler) {
        self.handlers.insert(command.to_lowercase(), handler);
    }

    /// Java `getHandler(commandValue)`: the handler of the command's first word.
    pub fn get(&self, command: &str) -> Option<BypassHandler> {
        let word = command.split(' ').next().unwrap_or(command);
        self.handlers.get(&word.to_lowercase()).copied()
    }
}

impl Default for BypassHandlers {
    /// The handlers the dialog flow needs: `Link`, `Chat` and `SkillList`.
    fn default() -> Self {
        let mut handlers = Self::empty();
        handlers.register("Link", link_handler);
        handlers.register("Chat", chat_handler);
        // dist/game/data/scripts/handlers/bypass/npc/SkillList.java
        handlers.register("SkillList", crate::skills::learn::skill_list_handler);
        // dist/game/data/scripts/handlers/bypass/npc/ScriptLink.java (M6-CLAN-02: the village
        // master scripts ClanMaster and AllianceMaster)
        handlers.register("Script", crate::clan::script_link_handler);
        // dist/game/data/scripts/handlers/bypass/npc/Multisell.java (M6-MULTISELL-01)
        handlers.register("multisell", crate::multisell::multisell_bypass_handler);
        handlers.register("exc_multisell", crate::multisell::multisell_bypass_handler);
        handlers
    }
}

/// Commands that are not html links: the client sends them without a page that listed them.
const POSSIBLE_NON_HTML_COMMANDS: [&str; 10] = [
    "item_",
    "_bbs",
    "bbs",
    "_mail",
    "_friend",
    "_match",
    "_diary",
    "_olympiad?command",
    "manor_menu_select",
    "report",
];

/// `Link.VALID_LINKS`: the pages `bypass Link` may open.
const VALID_LINKS: [&str; 41] = [
    "adventurer_guildsman/AboutHighLevelGuilds.htm",
    "adventurer_guildsman/AboutNewLifeCrystals.htm",
    "clanHallDoorman/evolve.htm",
    "common/augmentation_01.htm",
    "common/augmentation_02.htm",
    "common/crafting_01.htm",
    "common/duals_01.htm",
    "common/duals_02.htm",
    "common/duals_03.htm",
    "common/g_cube_warehouse001.htm",
    "common/skill_enchant_help.htm",
    "common/skill_enchant_help_01.htm",
    "common/skill_enchant_help_02.htm",
    "common/skill_enchant_help_03.htm",
    "common/weapon_sa_01.htm",
    "common/welcomeback002.htm",
    "common/welcomeback003.htm",
    "default/BlessingOfProtection.htm",
    "default/SupportMagic.htm",
    "default/SupportMagicServitor.htm",
    "fisherman/fishing_championship.htm",
    "fortress/foreman.htm",
    "guard/kamaloka_help.htm",
    "guard/kamaloka_level.htm",
    "olympiad/hero_main2.htm",
    "petmanager/evolve.htm",
    "petmanager/exchange.htm",
    "petmanager/instructions.htm",
    "seven_signs/blkmrkt_1.htm",
    "seven_signs/blkmrkt_2.htm",
    "seven_signs/mammblack_1a.htm",
    "seven_signs/mammblack_1b.htm",
    "seven_signs/mammblack_1c.htm",
    "seven_signs/mammblack_2a.htm",
    "seven_signs/mammblack_2b.htm",
    "seven_signs/mammmerch_1.htm",
    "seven_signs/mammmerch_1a.htm",
    "seven_signs/mammmerch_1b.htm",
    "teleporter/separatedsoul.htm",
    "warehouse/clanwh.htm",
    "warehouse/privatewh.htm",
];

/// Java `Link.onCommand`.
fn link_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    let html_path = command.get(4..).unwrap_or("").trim_matches(|c: char| c <= ' ');
    if html_path.is_empty() {
        warn!("{player} sent empty link html!");
        return false;
    }
    if html_path.contains("..") {
        warn!("{player} sent invalid link html: {html_path}");
        return false;
    }

    let mut content = if VALID_LINKS.contains(&html_path) {
        world.get_htm(player, &format!("data/html/{html_path}"))
    } else {
        None
    };

    // Precaution.
    if html_path.starts_with("teleporter/") && !world.is_target_teleporter(player) {
        content = None;
    }

    let object_id = target.map_or(0, |t| t.0);
    // NOTE(java-quirk): without content Java sends an `NpcHtmlMessage` that has no html, which
    // fails while it is written; nothing reaches the client.
    if let Some(content) = content
        && let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(
            object_id,
            &content.replace("%objectId%", &object_id.to_string()),
        )
    {
        world.send_html(player, &message);
    }
    true
}

/// Java `ChatLink.onCommand`.
fn chat_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    // A command that is not bound to an NPC makes Java throw a NullPointerException here.
    let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) else {
        return false;
    };
    let value = command
        .get(5..)
        .and_then(|rest| rest.parse::<i32>().ok())
        .unwrap_or(0);
    // `ON_NPC_FIRST_TALK` listeners are scripts (D-05).
    world.show_chat_window(npc, player, value);
    false
}

impl World {
    /// Java `LocationUtil.isInsideRangeOfObjectId(player, objectId, radius)`.
    fn is_inside_range_of_object_id(&self, player: ObjectId, object_id: i32, radius: i32) -> bool {
        let (Some(target), Some(me)) = (self.find_object(ObjectId(object_id)), self.position_of(player))
        else {
            return false;
        };
        self.position_of(target)
            .is_some_and(|pos| distance_3d(me, pos) <= f64::from(radius))
    }

    /// `player.getTarget() instanceof Teleporter`.
    fn is_target_teleporter(&self, player: ObjectId) -> bool {
        self.target_of(player)
            .and_then(|t| self.npcs.get(&t))
            .and_then(|n| self.npc_world.template(n.npc_id))
            .is_some_and(|t| t.npc_type == "Teleporter")
    }

    /// Java `RequestBypassToServer.runImpl`.
    pub(crate) fn on_bypass_to_server(&mut self, id: ObjectId, command: &str) {
        if !self.players.contains_key(&id) {
            return;
        }
        if command.is_empty() {
            warn!("{id} sent empty bypass!");
            // Disconnection.of(client, player).storeAndDeleteWith(LeaveWorld.STATIC_PACKET)
            self.leave(id, LeaveReason::Logout);
            return;
        }

        let requires_validation = !POSSIBLE_NON_HTML_COMMANDS
            .iter()
            .any(|prefix| command.starts_with(prefix));
        let mut origin = 0;
        if requires_validation {
            origin = self.players.get_mut(&id).map_or(-1, |p| p.talk.validate(command));
            if origin == -1 {
                return;
            }
            if origin > 0 && !self.is_inside_range_of_object_id(id, origin, INTERACTION_DISTANCE) {
                // No logging here, this could be a common case where the player has the html
                // still open and run too far away and then clicks a html action.
                return;
            }
        }
        // FOLLOW-UP: `canUseServerBypass` flood protector.

        if command.starts_with("admin_") {
            self.admin_command(id, command, true);
        } else if command == "come_here" && self.is_gm(id) {
            // FOLLOW-UP(M4): the target NPC walks to the GM (`setIntentionMoveTo`).
            debug!("{id}: come_here needs NPC movement (M4)");
        } else if command.starts_with("npc_") {
            self.npc_bypass(id, command);
        } else if command.starts_with("item_") {
            self.item_bypass(id, command);
        } else if command.starts_with("_match")
            || command.starts_with("_diary")
            || command.starts_with("_olympiad?command")
            || command.starts_with("manor_menu_select")
            || command.starts_with("report")
        {
            debug!("{id}: bypass {command}: heroes, manor and the captcha are not ported yet");
        } else if let Some(handler) = self.talk.bypass_handlers.get(command) {
            let target = if origin > 0 {
                self.find_object(ObjectId(origin))
                    .filter(|o| self.objects.get(o).is_some_and(|e| e.kind.is_creature()))
            } else {
                None
            };
            handler(self, id, command, target);
        } else {
            let session = self
                .player_sessions
                .get(&id)
                .map_or_else(|| "?".to_owned(), |s| format!("{s:?}"));
            warn!("{session} sent not handled RequestBypassToServer: [{command}]");
        }
    }

    /// The `npc_` branch: `npc_<objectId>_<command for the npc>`.
    fn npc_bypass(&mut self, id: ObjectId, command: &str) {
        // `command.indexOf('_', 5)`
        let end_of_id = command.get(5..).and_then(|rest| rest.find('_')).map(|i| i + 5);
        let id_text = match end_of_id {
            Some(end) => &command[4..end],
            None => command.get(4..).unwrap_or(""),
        };
        if !id_text.is_empty() && id_text.chars().all(|c| c.is_ascii_digit()) {
            match id_text.parse::<i32>() {
                Ok(object_id) => {
                    let npc = self
                        .find_object(ObjectId(object_id))
                        .filter(|o| self.npcs.contains_key(o));
                    if let (Some(npc), Some(end)) = (npc, end_of_id)
                        && let (Some(me), Some(pos)) = (self.position_of(id), self.position_of(npc))
                        && distance_2d(me, pos) < f64::from(INTERACTION_DISTANCE)
                    {
                        self.npc_bypass_feedback(npc, id, &command[end + 1..]);
                    }
                }
                // `Integer.parseInt` throws; Java logs the exception and goes on.
                Err(_) => warn!("Exception processing bypass from {id}: {command} (bad object id)"),
            }
        }
        self.send(id, &ActionFailed);
    }

    /// The `item_` branch: `item_<objectId>_<command for the item>`.
    fn item_bypass(&mut self, id: ObjectId, command: &str) {
        let end_of_id = command.get(5..).and_then(|rest| rest.find('_')).map(|i| i + 5);
        let id_text = match end_of_id {
            Some(end) => &command[5..end],
            None => &command[5..],
        };
        match id_text.parse::<i32>() {
            Ok(_) => {
                // FOLLOW-UP(items): `item.onBypassFeedback(player, command)` (item scripts).
                self.send(id, &ActionFailed);
            }
            Err(_) => warn!("NFE for command [{command}] (bad item object id)"),
        }
    }

    /// Java `Npc.onBypassFeedback(player, command)` with the override of the NPC's class.
    pub(crate) fn npc_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        let is_teleporter = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .is_some_and(|t| t.npc_type == "Teleporter");
        if is_teleporter {
            self.teleporter_bypass_feedback(npc, player, command);
        } else if self.is_village_master(npc) {
            // Java: VillageMaster.onBypassFeedback (the clan commands, `clan/village_master.rs`).
            self.village_master_bypass_feedback(npc, player, command);
        } else {
            self.generic_npc_bypass_feedback(npc, player, command);
        }
    }

    /// `Npc.onBypassFeedback`: the registry.
    pub(crate) fn generic_npc_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        if !self.can_interact(npc, player) {
            return;
        }
        // FOLLOW-UP: a busy NPC (`_isBusy`) answers with `npcbusy.htm`; nothing sets it yet.
        match self.talk.bypass_handlers.get(command) {
            Some(handler) => {
                handler(self, player, command, Some(npc));
            }
            None => {
                let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
                info!("Npc: Unknown NPC bypass: \"{command}\" NpcId: {npc_id}");
            }
        }
    }

    /// Java `RequestLinkHtml.runImpl`.
    pub(crate) fn on_link_html(&mut self, id: ObjectId, link: &str) {
        if !self.players.contains_key(&id) {
            return;
        }
        if link.is_empty() {
            warn!("{id} sent empty html link!");
            return;
        }
        if link.contains("..") {
            warn!("{id} sent invalid html link: link {link}");
            return;
        }
        let html_object_id = self
            .players
            .get_mut(&id)
            .map_or(-1, |p| p.talk.validate(&format!("link {link}")));
        if html_object_id == -1 {
            warn!("{id} sent non cached html link: link {link}");
            return;
        }
        if html_object_id > 0 && !self.is_inside_range_of_object_id(id, html_object_id, INTERACTION_DISTANCE)
        {
            // No logging here, this could be a common case.
            return;
        }
        let filename = format!("data/html/{link}");
        if let Some(message) = self.html_from_file(id, html_object_id, &filename) {
            self.send_html(id, &message);
        }
    }
}
