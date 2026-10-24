//! Gatekeepers (`Teleporter` NPCs): the list of destinations with their fees, and the teleport.
// Java: entity/actor/instance/Teleporter.java (onBypassFeedback, sendHtmlMessage)
// Java: entity/teleporter/{TeleportHolder,TeleportLocation}.java (showTeleportList, doTeleport,
//       shouldPayFee, calculateFee, getItemName)
//
// # What is left out, with the system that brings it
// - Sieges (`castleId` of a destination, `getCastle().getSiege()`, a gatekeeper on castle ground):
//   castles are M5. No castle is loaded, so none is under siege and none is "on castle ground".
// - `isAffectedBySkill(FROG/CHILD/NATIVE_TRANSFORM)` (effects, M5) and `isCombatFlagEquipped`
//   (territory wars).
//
// NOTE: Java's `Calendar.getInstance()` reads the server's time zone for the Monday/Tuesday
// evening discount; the standard library has no time zone, so UTC is used.

use l2_data::TeleportType;
use l2_data::item::ADENA_ID;
use l2_data::teleporter::{TeleportHolder, TeleportLocation};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::warn;

use crate::teleport::TeleportTarget;
use crate::world::World;

impl World {
    /// Java `Teleporter.onBypassFeedback(player, command)`.
    pub(crate) fn teleporter_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        // StringTokenizer(command, " ")
        let mut tokens = command.split(' ').filter(|t| !t.is_empty());
        let Some(first) = tokens.next() else {
            warn!("Exception processing bypass from {player}: no command left in {command:?}");
            return;
        };
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let noble = self.players.get(&player).is_some_and(|p| p.data.profile.noble);
        match first {
            "showNoblesSelect" => {
                let file = if noble { "nobles_select" } else { "not_nobles" };
                self.send_npc_file(npc, player, &format!("data/html/teleporter/{file}.htm"));
            }
            "showTeleports" => {
                let list_name = tokens.next().unwrap_or(TeleportType::Normal.name());
                let data = self.talk.teleporters.clone();
                let Some(holder) = data.as_ref().and_then(|d| d.get_holder(npc_id, list_name)) else {
                    warn!(
                        "{player} requested show teleports for list with name {list_name} at NPC {npc_id}!"
                    );
                    return;
                };
                self.show_teleport_list(npc, player, holder);
            }
            "teleport" => {
                let rest: Vec<&str> = tokens.collect();
                // Check for required count of params.
                if rest.len() != 2 {
                    warn!("{player} send unhandled teleport command: {command}");
                    return;
                }
                let list_name = rest[0];
                let data = self.talk.teleporters.clone();
                let Some(holder) = data.as_ref().and_then(|d| d.get_holder(npc_id, list_name)) else {
                    warn!("{player} requested unknown teleport list: {list_name} for npc: {npc_id}!");
                    return;
                };
                // `parseNextInt(st, -1)`: a token that is not a number is -1.
                let location_id = if rest[1].chars().all(|c| c.is_ascii_digit()) {
                    rest[1].parse::<i32>().unwrap_or(-1)
                } else {
                    -1
                };
                self.do_teleport(npc, player, holder, location_id);
            }
            // NOTE(java-quirk): the html sends `Chat` and this case is lower case, so it is never
            // reached from the shipped pages; they go through the `Chat` bypass handler.
            "chat" => {
                let value = command
                    .get(5..)
                    .and_then(|rest| rest.parse::<i32>().ok())
                    .unwrap_or(0);
                self.show_chat_window(npc, player, value);
            }
            _ => self.generic_npc_bypass_feedback(npc, player, command),
        }
    }

    /// Java `TeleportHolder.showTeleportList(player, npc)`.
    fn show_teleport_list(&mut self, npc: ObjectId, player: ObjectId, holder: &TeleportHolder) {
        let noble = self.players.get(&player).is_some_and(|p| p.data.profile.noble);
        if holder.is_noblesse() && !noble {
            warn!("{player} requested noblesse teleport without being noble!");
            return;
        }

        // Build html
        let bypass = format!("npc_{}_teleport", npc.0);
        let mut list = String::new();
        for location in holder.locations() {
            // A destination without a name prints as "null" in Java.
            let name = location.name.as_deref().unwrap_or("null");
            let mut final_name = name.to_owned();
            if self.should_pay_fee(player, holder, location) {
                let fee = self.calculate_fee(player, holder, location);
                if fee != 0 {
                    final_name.push_str(&format!(" - {fee} {}", self.fee_item_name(location.fee_id)));
                }
            }
            list.push_str(&format!(
                "<a action=\"bypass -h {bypass} {} {}\" msg=\"811;{name}\">{final_name}</a><br1>",
                holder.name(),
                location.id
            ));
        }

        // Send html message
        let Some(mut message) = self.html_from_file(player, npc.0, "data/html/teleporter/teleports.htm")
        else {
            return;
        };
        message.html = message.html.replace("%locations%", &list);
        self.send_html(player, &message);
    }

    /// Java `TeleportHolder.doTeleport(player, npc, locId)`.
    pub(crate) fn do_teleport(&mut self, _npc: ObjectId, player: ObjectId, holder: &TeleportHolder, location_id: i32) {
        // Check if player is dead before other checks.
        if self.is_alike_dead(player) {
            return;
        }
        let Some((noble, karma)) = self
            .players
            .get(&player)
            .map(|p| (p.data.profile.noble, p.data.karma))
        else {
            return;
        };
        if holder.is_noblesse() && !noble {
            warn!("{player} requested noblesse teleport without being noble!");
            return;
        }
        // NOTE(java-quirk): `getLocation` is `List.get`, which throws for an unknown index; the
        // `null` check after it is never reached. Either way nothing happens.
        let Some(location) = holder.location(location_id) else {
            warn!(
                "{player} requested unknown teleport location {location_id} within list {}!",
                holder.name()
            );
            return;
        };

        // Check if castle is in siege: FOLLOW-UP(M5) `loc.getCastleId()`.

        // Validate conditions for NORMAL teleport.
        if holder.kind() == TeleportType::Normal && !self.settings.talk.karma_player_can_use_gk && karma > 0 {
            self.send_message(player, "Go away, you're not welcome here.");
            return;
        }

        // Check if player should pay fee.
        if self.should_pay_fee(player, holder, location) {
            let fee = self.calculate_fee(player, holder, location);
            if !self.destroy_item_by_item_id(player, location.fee_id, fee, true) {
                if location.fee_id == ADENA_ID {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                    );
                } else {
                    let name = self.fee_item_name(location.fee_id);
                    self.send_message(player, &format!("You do not have enough {name}."));
                }
                return;
            }
        }

        self.tele_to_location(
            player,
            TeleportTarget::at(location.x, location.y, location.z)
                .with_heading(location.heading)
                .with_instance(location.instance_id),
        );
    }

    /// Java `TeleportHolder.shouldPayFee`.
    fn should_pay_fee(&self, player: ObjectId, holder: &TeleportHolder, location: &TeleportLocation) -> bool {
        if holder.kind() != TeleportType::Normal {
            return true;
        }
        let Some(p) = self.players.get(&player) else {
            return false;
        };
        // `isSubClassActive()`: the active class is not the base class.
        let subclass_active = p.data.class_id != p.data.base_class;
        (p.core.level > self.settings.talk.max_free_teleport_level || subclass_active)
            && (location.fee_id != 0 && location.fee_count > 0)
    }

    /// Java `TeleportHolder.calculateFee`: free below the free-teleport level, and half price on
    /// Monday and Tuesday evenings from 8 pm.
    fn calculate_fee(&self, player: ObjectId, holder: &TeleportHolder, location: &TeleportLocation) -> i64 {
        if holder.kind() == TeleportType::Normal {
            let Some(p) = self.players.get(&player) else {
                return location.fee_count;
            };
            let subclass_active = p.data.class_id != p.data.base_class;
            if !subclass_active && p.core.level <= self.settings.talk.max_free_teleport_level {
                return 0;
            }
            let now = self
                .settings
                .talk
                .clock_override_unix_ms
                // Java `Calendar.getInstance()`: the server's local time, not UTC.
                .unwrap_or_else(l2_util::time::local_now_millis);
            let hour = now.div_euclid(3_600_000).rem_euclid(24);
            // 1970-01-01 was a Thursday; 0 = Monday.
            let weekday = now.div_euclid(86_400_000).wrapping_add(3).rem_euclid(7);
            if hour >= 20 && weekday <= 1 {
                return location.fee_count / 2;
            }
        }
        location.fee_count
    }

    /// Java `TeleportHolder.getItemName(itemId)`.
    fn fee_item_name(&self, item_id: i32) -> String {
        if let Some(name) = self.data.item_name(item_id) {
            return name;
        }
        match item_id {
            -100 => "Player Commendation Points".to_owned(),
            -200 => "Clan Reputation Points".to_owned(),
            -300 => "Fame".to_owned(),
            _ => format!("Unknown item: {item_id}"),
        }
    }
}
