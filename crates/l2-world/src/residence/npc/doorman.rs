//! The `CastleDoorman` NPC class: the castle's gate keepers open and close the castle doors for
//! the owner clan's members with the right to, and teleport them through the gate.
// Java: entity/actor/instance/CastleDoorman.java
// Java: entity/actor/instance/Doorman.java (onBypassFeedback, showChatWindow, cannotManageDoors)
// Java: mechanics/siege/clanhalls/SiegableHall.java (openCloseDoor): a doorman that stands at a
//       conquerable hall serves the hall's owner instead of the castle's

use l2_protocol_game::server::ActionFailed;
use l2_util::ObjectId;
use tracing::warn;

use crate::clan::model::ClanAccess;
use crate::residence::{ClanHallId, ResidenceRef};
use crate::world::World;

/// `Npc.getConquerableHall()`: `CHSiegeManager.getNearbyClanHall(x, y, 10000)`.
const CONQUERABLE_HALL_DISTANCE: i32 = 10_000;

/// Java `new StringTokenizer(text, ", ")`: the words between commas and spaces.
fn door_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split([',', ' ']).filter(|t| !t.is_empty())
}

impl World {
    /// Whether the NPC is a `CastleDoorman`.
    fn is_castle_doorman(&self, npc: ObjectId) -> bool {
        self.npc_template(npc)
            .is_some_and(|t| t.npc_type == "CastleDoorman")
    }

    /// `getConquerableHall()`.
    fn doorman_conquerable_hall(&self, npc: ObjectId) -> Option<ClanHallId> {
        let (x, y, _) = self.position_of(npc)?;
        self.nearby_siegable_hall(x, y, CONQUERABLE_HALL_DISTANCE)
    }

    /// `CastleDoorman.isOwnerClan(player)`.
    fn castle_doorman_is_owner_clan(&self, npc: ObjectId, player: ObjectId) -> bool {
        if self.clan_of(player).is_none() || !self.has_clan_access(player, ClanAccess::CastleOpenDoor) {
            return false;
        }
        let clan_id = self.clan_id_of(player);
        match self.doorman_conquerable_hall(npc) {
            Some(hall) => self.clan_hall(hall).is_some_and(|h| clan_id == h.owner_id),
            None => self.npc_castle(npc).is_some() && clan_id == self.castle_owner_of_npc(npc),
        }
    }

    /// `CastleDoorman.isUnderSiege()`.
    fn castle_doorman_is_under_siege(&self, npc: ObjectId) -> bool {
        if self.doorman_conquerable_hall(npc).is_some() {
            // M7-CHSIEGE-01: `hall.isInSiege()`.
            return false;
        }
        // `getCastle().getZone().isActive()`.
        self.npc_castle(npc)
            .and_then(|c| self.castle(c))
            .and_then(|c| c.siege_zone)
            .is_some_and(|zone| self.is_siege_zone_active(zone))
    }

    /// `Doorman.showChatWindow(player)`: the click on a castle doorman. `false` for any other NPC.
    pub(crate) fn castle_doorman_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        if !self.is_castle_doorman(npc) {
            return false;
        }
        self.send(player, &ActionFailed);
        let id = self.castle_npc_id(npc);
        let path = if self.castle_doorman_is_owner_clan(npc, player) {
            format!("data/html/doorman/{id}.htm")
        } else {
            format!("data/html/doorman/{id}-no.htm")
        };
        if let Some(mut html) = self.html_from_file(player, npc.0, &path) {
            html.html = html.html.replace("%objectId%", &npc.0.to_string());
            self.send_html(player, &html);
        }
        true
    }

    /// `Doorman.cannotManageDoors(player)`: the "busy" page, without any replacement.
    fn castle_doorman_cannot_manage(&mut self, npc: ObjectId, player: ObjectId) {
        self.send(player, &ActionFailed);
        let path = format!("data/html/doorman/{}-busy.htm", self.castle_npc_id(npc));
        if let Some(html) = self.html_from_file(player, npc.0, &path) {
            self.send_html(player, &html);
        }
    }

    /// `Doorman.onBypassFeedback(player, command)` of a castle doorman. `false` for any other NPC.
    pub(crate) fn castle_doorman_bypass_feedback(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        command: &str,
    ) -> bool {
        if !self.is_castle_doorman(npc) {
            return false;
        }
        if command.starts_with("Chat") {
            self.castle_doorman_show_chat_window(npc, player);
        } else if command.starts_with("open_doors") || command.starts_with("close_doors") {
            let open = command.starts_with("open_doors");
            if self.castle_doorman_is_owner_clan(npc, player) {
                if self.castle_doorman_is_under_siege(npc) {
                    self.castle_doorman_cannot_manage(npc, player);
                } else if self.castle_doorman_doors(npc, player, command, open).is_none() {
                    warn!("Exception processing bypass from {player}: {command} (castle doorman)");
                }
            }
        } else if command.starts_with("tele") {
            if self.castle_doorman_is_owner_clan(npc, player) {
                let npc_id = self.castle_npc_id(npc);
                let teleporters = self.talk.teleporters.clone();
                if let Some(holder) = teleporters
                    .as_ref()
                    .and_then(|d| d.get_holder(npc_id, l2_data::TeleportType::Other.name()))
                {
                    // `Integer.parseInt(command.substring(5).trim())`.
                    let Some(location) = command.get(5..).and_then(|rest| rest.trim().parse::<i32>().ok())
                    else {
                        warn!("Exception processing bypass from {player}: {command} (castle doorman)");
                        return true;
                    };
                    self.do_teleport(npc, player, holder, location);
                }
            }
        } else {
            self.generic_npc_bypass_feedback(npc, player, command);
        }
        true
    }

    /// `CastleDoorman.openDoors` / `closeDoors`: "open_doors 1, 19210005, 19210006". The first
    /// number is skipped (`st.nextToken()`); `None` where Java throws.
    fn castle_doorman_doors(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        command: &str,
        open: bool,
    ) -> Option<()> {
        let skip = if open { 10 } else { 11 };
        let mut tokens = door_tokens(command.get(skip..)?);
        tokens.next()?;
        let hall = self.doorman_conquerable_hall(npc);
        let castle = self.npc_castle(npc);
        for token in tokens {
            let door = token.parse::<i32>().ok()?;
            if let Some(hall) = hall {
                // `SiegableHall.openCloseDoor(doorId, open)`: no owner check.
                if let Some(object) = self.residence_door(ResidenceRef::ClanHall(hall), door) {
                    if open {
                        self.open_door(object);
                    } else {
                        self.close_door(object);
                    }
                }
            } else {
                // `getCastle().openDoor(player, doorId)`: a null castle throws.
                self.residence_open_close_door(player, ResidenceRef::Castle(castle?), door, open);
            }
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_door_list_splits_on_commas_and_spaces() {
        let tokens: Vec<&str> = door_tokens(" 1, 19210005, 19210006").collect();
        assert_eq!(tokens, ["1", "19210005", "19210006"]);
    }
}
