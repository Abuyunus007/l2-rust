//! The `ManorManager` NPCs (the merchants of the castle towns that sell seeds and buy crops) and
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
// The first talk and the help pages of `ManorManager` are a script of the engine (S-API-02,
// `l2-scripts/src/ai/others/manor_manager.rs`). What is left here is its `OnNpcManorBypass`
// listener: the engine has no annotation listeners yet, so `manor_menu_select?ask=..` opens the
// packets of the menu from here.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use crate::world::World;

/// The ids of the `ManorManager` NPCs (`ManorManager.NPC`).
const MANOR_MANAGERS: [i32; 14] = [
    35644, 35645, 35319, 35366, 36456, 35512, 35558, 35229, 35230, 35231, 35277, 35103, 35145, 35187,
];

/// The castle chamberlains (`CastleChamberlain.NPC`), which also listen for `manor_menu_select`.
const CHAMBERLAINS: [i32; 9] = [35100, 35142, 35184, 35226, 35274, 35316, 35363, 35509, 35555];

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
        // The air ship controllers (M8-VEHICLE-01, `vehicle/controller.rs`).
        if self.air_ship_first_talk(npc, player, true) {
            return true;
        }
        false
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
        // The air ship controllers (M8-VEHICLE-01).
        if self.air_ship_first_talk(npc, player, false) {
            return true;
        }
        false
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
