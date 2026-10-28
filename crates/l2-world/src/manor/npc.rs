//! The `ManorManager` script (the merchants of the castle towns that sell seeds and buy crops) and
//! the `manor_menu_select` bypass, Java's `OnNpcManorBypass` event, of it and of the castle
//! chamberlains.
// Java: dist/game/data/scripts/ai/others/ManorManager/ManorManager.java (and its pages)
// Java: dist/game/data/scripts/ai/others/CastleChamberlain/CastleChamberlain.java
//       (`onNpcManorBypass`, `isOwner`; the rest of the chamberlain is M7-NPC-01)
// Java: network/clientpackets/RequestBypassToServer.java (`manor_menu_select`)
// Java: dist/game/data/scripts/handlers/bypass/npc/ChatLink.java, handlers/actions/click/NpcClick.java
//       (the `ON_NPC_FIRST_TALK` and `ON_NPC_QUEST_START` listeners)
// Java: entity/actor/Player.java (processScriptEvent)
//
// D-05: the script API comes last; this is built in like `ClanMaster` (`clan/master_script.rs`).
// `Script ManorManager <page>` shows a help page, the first talk shows the menu (the lord's page
// for the leader of the clan that owns the castle of the town), and `manor_menu_select?ask=..`
// opens the packets of the menu.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{debug, warn};

use crate::dialog::INTERACTION_DISTANCE;
use crate::target::distance_2d;
use crate::world::World;

/// The ids of the `ManorManager` NPCs (`ManorManager.NPC`).
const MANOR_MANAGERS: [i32; 14] = [
    35644, 35645, 35319, 35366, 36456, 35512, 35558, 35229, 35230, 35231, 35277, 35103, 35145, 35187,
];

/// The castle chamberlains (`CastleChamberlain.NPC`), which also listen for `manor_menu_select`.
const CHAMBERLAINS: [i32; 9] = [35100, 35142, 35184, 35226, 35274, 35316, 35363, 35509, 35555];

/// The script folder below `data/scripts` (`Quest.getPath()`).
const MANOR_MANAGER_PATH: &str = "ai/others/ManorManager";

/// The help pages `ManorManager.onEvent` answers with the page itself.
const HELP_PAGES: [&str; 3] = [
    "manager-help-01.htm",
    "manager-help-02.htm",
    "manager-help-03.htm",
];

impl World {
    /// Whether the NPC is one of the `ManorManager` NPCs.
    fn is_manor_manager(&self, npc: ObjectId) -> bool {
        self.npcs
            .get(&npc)
            .is_some_and(|n| MANOR_MANAGERS.contains(&n.npc_id))
    }

    /// Whether the NPC is a castle chamberlain.
    fn is_chamberlain(&self, npc: ObjectId) -> bool {
        self.npcs
            .get(&npc)
            .is_some_and(|n| CHAMBERLAINS.contains(&n.npc_id))
    }

    // ------------------------------------------------------------------ first talk

    /// `NpcClick`'s branch for an NPC with an `ON_NPC_FIRST_TALK` listener (and `ON_NPC_QUEST_START`
    /// for the script's start NPCs): the script answers instead of `showChatWindow`. `true` when
    /// a script took the talk.
    pub(crate) fn npc_first_talk_on_click(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        // The listeners of the script engine (S-API-01).
        if self.script_on_npc_click(npc, player) {
            return true;
        }
        // The conquerable hall scripts (M7-CHSIEGE-01).
        if self.hall_script_first_talk(npc, player) {
            return true;
        }
        // The castle NPCs (M7-NPC-01, `residence/npc/`).
        if self.residence_npc_on_click(npc, player) {
            return true;
        }
        // `SymbolMaker` (L-01, `henna.rs`).
        if self.symbol_maker_first_talk(npc, player, true) {
            return true;
        }
        // The wedding priest (M8-MISC-01).
        if self.wedding_first_talk(npc, player, true) {
            return true;
        }
        // The air ship controllers (M8-VEHICLE-01, `vehicle/controller.rs`).
        if self.air_ship_first_talk(npc, player, true) {
            return true;
        }
        if !self.is_manor_manager(npc) {
            return false;
        }
        // `addStartNpc`: `ON_NPC_QUEST_START` makes the NPC the quest NPC of the player.
        if let Some(p) = self.players.get_mut(&player) {
            p.talk.last_quest_npc_object = npc.0;
        }
        self.manor_manager_first_talk(npc, player);
        true
    }

    /// `ChatLink` for `Chat 0`: the first talk when the NPC has a listener for it. `true` when a
    /// script took the talk.
    pub(crate) fn npc_first_talk_on_chat(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        // The listeners of the script engine (S-API-01).
        if self.script_on_chat_first_talk(npc, player) {
            return true;
        }
        if self.hall_script_first_talk(npc, player) {
            return true;
        }
        // The castle NPCs (M7-NPC-01, `residence/npc/`).
        if self.residence_npc_first_talk(npc, player, false) {
            return true;
        }
        // `SymbolMaker` (L-01, `henna.rs`).
        if self.symbol_maker_first_talk(npc, player, false) {
            return true;
        }
        // The wedding priest (M8-MISC-01).
        if self.wedding_first_talk(npc, player, false) {
            return true;
        }
        // The air ship controllers (M8-VEHICLE-01).
        if self.air_ship_first_talk(npc, player, false) {
            return true;
        }
        if !self.is_manor_manager(npc) {
            return false;
        }
        self.manor_manager_first_talk(npc, player);
        true
    }

    /// Java `ManorManager.onFirstTalk`, then `Quest.showResult`.
    fn manor_manager_first_talk(&mut self, npc: ObjectId, player: ObjectId) {
        let result = if self.manor.settings.allow_manor {
            let castle_id = self.npc_manor_id(npc);
            let is_lord = !self.is_gm(player)
                && self.is_clan_leader(player)
                && self.clan_of(player).is_some_and(|c| c.castle_id == castle_id);
            if is_lord {
                "manager-lord.htm".to_owned()
            } else {
                "manager.htm".to_owned()
            }
        } else {
            // `getHtm(player, "data/html/npcdefault.htm")`: the text itself.
            match self.get_htm(player, "data/html/npcdefault.htm") {
                Some(content) => content,
                None => return,
            }
        };
        self.manor_manager_show_result(player, npc, &result);
    }

    /// Java `Quest.showResult(player, res, npc)` of the `ManorManager` script.
    fn manor_manager_show_result(&mut self, player: ObjectId, npc: ObjectId, res: &str) {
        if res.is_empty() {
            return;
        }
        if res.ends_with(".htm") || res.ends_with(".html") {
            // `Quest.getHtm`: the script's folder, then `data/scripts/quests/<name>`.
            let found = [
                format!("data/scripts/{MANOR_MANAGER_PATH}/{res}"),
                format!("data/scripts/quests/ManorManager/{res}"),
            ]
            .iter()
            .find_map(|path| self.get_htm(player, path));
            match found {
                Some(content) => self.manor_manager_send_html(player, npc, &content),
                None => debug!("{player}: page {res} of ManorManager not found."),
            }
        } else if res.starts_with("<html") {
            self.manor_manager_send_html(player, npc, res);
        } else {
            self.send_message(player, res);
        }
    }

    /// `showHtmlFile`'s send: `%objectId%` and `%playername%` replaced, then `ActionFailed`.
    fn manor_manager_send_html(&mut self, player: ObjectId, npc: ObjectId, content: &str) {
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let html = content
            .replace("%objectId%", &npc.0.to_string())
            .replace("%playername%", &name);
        if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
            self.send_html(player, &message);
        }
        self.send(player, &ActionFailed);
    }

    // ------------------------------------------------------------------ Script ManorManager <page>

    /// Java `Player.processScriptEvent("ManorManager", event)`: the NPC the player talked to
    /// within reach, else the quest NPC, answers the event (`ManorManager.onEvent`).
    pub(crate) fn manor_process_script_event(&mut self, player: ObjectId, event: &str) {
        if event.is_empty() {
            return;
        }
        let Some(talk) = self.players.get(&player).map(|p| &p.talk) else {
            return;
        };
        let (folk, quest_npc) = (talk.last_folk_npc, talk.last_quest_npc_object);
        let in_range = |world: &Self, npc: ObjectId| match (world.position_of(player), world.position_of(npc))
        {
            (Some(me), Some(pos)) => distance_2d(me, pos) < f64::from(INTERACTION_DISTANCE),
            _ => false,
        };
        let npc = match folk.filter(|n| self.npcs.contains_key(n)) {
            Some(target) if in_range(self, target) => target,
            _ if quest_npc > 0 => match self
                .find_object(ObjectId(quest_npc))
                .filter(|o| self.npcs.contains_key(o))
            {
                Some(npc) if in_range(self, npc) => npc,
                _ => return,
            },
            _ => return,
        };
        // `ManorManager.onEvent`: the three help pages answer with themselves.
        if HELP_PAGES.contains(&event) {
            self.manor_manager_show_result(player, npc, event);
        }
    }

    // ------------------------------------------------------------------ manor_menu_select

    /// Java `RequestBypassToServer`'s `manor_menu_select?ask=..&state=..&time=..`.
    pub(crate) fn manor_menu_select(&mut self, player: ObjectId, command: &str) {
        let npc = self.players.get(&player).and_then(|p| p.talk.last_folk_npc);
        let listener = npc.filter(|n| self.is_manor_manager(*n) || self.is_chamberlain(*n));
        let Some(npc) = listener else {
            return;
        };
        if !self.manor.settings.allow_manor || !self.can_interact(npc, player) {
            return;
        }
        // `_command.substring(_command.indexOf('?') + 1).split("&")`, each `name=value`.
        let query = command.split_once('?').map_or(command, |(_, query)| query);
        let mut values = query.split('&').map(|part| part.split('=').nth(1));
        let ask = values.next().flatten().and_then(|v| v.parse::<i32>().ok());
        let state = values.next().flatten().and_then(|v| v.parse::<i32>().ok());
        let time = values.next().flatten().map(|v| v == "1");
        let (Some(ask), Some(state), Some(time)) = (ask, state, time) else {
            // Java throws and `runImpl` logs it.
            warn!("{player}: bad manor bypass {command}");
            return;
        };
        self.on_npc_manor_bypass(player, npc, ask, state, time);
    }

    /// Java `OnNpcManorBypass` for the listener of the NPC.
    pub fn on_npc_manor_bypass(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        request: i32,
        manor_id: i32,
        next_period: bool,
    ) {
        if self.is_manor_manager(npc) {
            self.manor_manager_bypass(player, npc, request, manor_id, next_period);
        } else if self.is_chamberlain(npc) {
            self.chamberlain_manor_bypass(player, npc, request, manor_id, next_period);
        }
    }

    /// `ManorManager.onNpcManorBypass`.
    fn manor_manager_bypass(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        request: i32,
        manor_id: i32,
        next_period: bool,
    ) {
        if self.manor_is_under_maintenance() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheManorSystemIsCurrentlyUnderMaintenance),
            );
            return;
        }

        let template_id = self.npc_manor_id(npc);
        let castle_id = if manor_id == -1 { template_id } else { manor_id };
        match request {
            // Seed purchase
            1 => {
                if template_id != castle_id {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::HereYouCanBuyOnlySeedsOfS1Manor)
                            .with(SystemMessageParam::CastleName(template_id)),
                    );
                    return;
                }
                if let Some(packet) = self.manor_buy_list_seed_packet(player, castle_id) {
                    self.send(player, &packet);
                }
            }
            // Crop sales
            2 => {
                if let Some(packet) = self.manor_sell_crop_list_packet(player, castle_id) {
                    self.send(player, &packet);
                }
            }
            // Seed info
            3 => {
                let packet = self.manor_seed_info_packet(castle_id, next_period, false);
                self.send(player, &packet);
            }
            // Crop info
            4 => {
                let packet = self.manor_crop_info_packet(castle_id, next_period, false);
                self.send(player, &packet);
            }
            // Basic info
            5 => {
                let packet = self.manor_default_info_packet(false);
                self.send(player, &packet);
            }
            // Buy harvester
            6 => {
                let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
                self.show_buy_window(npc, player, 300_000 + npc_id);
            }
            // Edit sales (Crop sales)
            9 => {
                let packet = self.manor_procure_crop_detail_packet(manor_id);
                self.send(player, &packet);
            }
            _ => warn!("ManorManager: {player} send unknown request id {request}!"),
        }
    }

    /// `CastleChamberlain.isOwner`.
    fn chamberlain_is_owner(&self, player: ObjectId, npc: ObjectId) -> bool {
        self.is_gm(player)
            || (self.clan_of(player).is_some()
                && self
                    .npc_castle(npc)
                    .and_then(|c| self.castle(c))
                    .is_some_and(|castle| castle.owner_id == self.clan_id_of(player)))
    }

    /// `CastleChamberlain.onNpcManorBypass`: the pages of the manor the chamberlain opens for the
    /// castle owner (the packets without the purchase and sale buttons).
    fn chamberlain_manor_bypass(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        request: i32,
        manor_id: i32,
        next_period: bool,
    ) {
        if !self.chamberlain_is_owner(player, npc) {
            return;
        }
        if self.manor_is_under_maintenance() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheManorSystemIsCurrentlyUnderMaintenance),
            );
            return;
        }

        let castle_id = if manor_id == -1 {
            self.npc_castle(npc).map_or(0, |c| c.0)
        } else {
            manor_id
        };
        match request {
            // Seed info
            3 => {
                let packet = self.manor_seed_info_packet(castle_id, next_period, true);
                self.send(player, &packet);
            }
            // Crop info
            4 => {
                let packet = self.manor_crop_info_packet(castle_id, next_period, true);
                self.send(player, &packet);
            }
            // Basic info
            5 => {
                let packet = self.manor_default_info_packet(true);
                self.send(player, &packet);
            }
            // Seed settings
            7 => {
                if self.manor_is_approved() {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::AManorCannotBeSetUpBetween430AmAnd8Pm),
                    );
                    return;
                }
                let packet = self.manor_seed_setting_packet(castle_id);
                self.send(player, &packet);
            }
            // Crop settings
            8 => {
                if self.manor_is_approved() {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::AManorCannotBeSetUpBetween430AmAnd8Pm),
                    );
                    return;
                }
                let packet = self.manor_crop_setting_packet(castle_id);
                self.send(player, &packet);
            }
            _ => warn!("CastleChamberlain: {player} send unknown request id {request}!"),
        }
    }
}
