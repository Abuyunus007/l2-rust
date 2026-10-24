//! The `ClanHallDoorman` NPC: opens and closes the doors of the hall for its clan.
// Java: entity/actor/instance/ClanHallDoorman.java
// Java: entity/actor/instance/Doorman.java (onBypassFeedback, cannotManageDoors, tele)
//
// A doorman answers only the clan that owns the hall it stands at. The halls of `CH_WITH_EVOLVE`
// also offer the pet evolution; `PetManager.doEvolve` is not ported (nothing evolves, the player
// gets the "no" page).

use l2_protocol_game::server::ActionFailed;
use l2_util::ObjectId;
use tracing::warn;

use crate::residence::ClanHallId;
use crate::world::World;

/// Java `ClanHallDoorman.CH_WITH_EVOLVE`: the halls with the evolve function.
const CH_WITH_EVOLVE: [i32; 13] = [36, 37, 38, 39, 40, 41, 51, 52, 53, 54, 55, 56, 57];
/// `ClanTable.getNearbyClanHall(getX(), getY(), 500)`.
const NEARBY_HALL_DISTANCE: i32 = 500;

impl World {
    /// Java `ClanHallDoorman.getClanHall()`: the nearest auctionable hall within 500, found at the
    /// first use and kept (also when there is none).
    fn doorman_hall(&mut self, npc: ObjectId) -> Option<ClanHallId> {
        if let Some(hall) = self.clanhall.npc_halls.get(&npc) {
            return *hall;
        }
        let (x, y, _) = self.position_of(npc)?;
        let hall = self.nearby_auctionable_hall(x, y, NEARBY_HALL_DISTANCE);
        self.clanhall.npc_halls.insert(npc, hall);
        hall
    }

    /// Java `ClanHallDoorman.isOwnerClan(player)`.
    fn doorman_is_owner_clan(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let hall = self.doorman_hall(npc);
        match (self.clan_of(player), hall.and_then(|id| self.clan_hall(id))) {
            (Some(clan), Some(hall)) => clan.id == hall.owner_id,
            _ => false,
        }
    }

    /// Java `ClanHallDoorman.showChatWindow(player)`.
    pub(crate) fn clan_hall_doorman_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) {
        self.send(player, &ActionFailed);
        let Some(hall_id) = self.doorman_hall(npc) else {
            return;
        };
        let Some((owner_id, hall_name)) = self.clan_hall(hall_id).map(|h| (h.owner_id, h.name.clone())) else {
            return;
        };
        let has_evolve = CH_WITH_EVOLVE.contains(&hall_id.0);
        let owner = self
            .clan(owner_id)
            .map(|c| (c.name.clone(), c.leader_member().map(|m| m.name.clone())));
        let html = if self.doorman_is_owner_clan(npc, player) {
            let path = if has_evolve {
                "data/html/clanHallDoorman/doorman2.htm"
            } else {
                "data/html/clanHallDoorman/doorman1.htm"
            };
            self.html_from_file(player, npc.0, path).map(|mut html| {
                let clan_name = owner.as_ref().map(|o| o.0.clone()).unwrap_or_default();
                html.html = html.html.replace("%clanname%", &clan_name);
                html
            })
        } else if let Some((clan_name, Some(leader))) = owner {
            self.html_from_file(player, npc.0, "data/html/clanHallDoorman/doorman-no.htm")
                .map(|mut html| {
                    html.html = html.html.replace("%leadername%", &leader);
                    html.html = html.html.replace("%clanname%", &clan_name);
                    html
                })
        } else {
            self.html_from_file(player, npc.0, "data/html/clanHallDoorman/emptyowner.htm")
                .map(|mut html| {
                    html.html = html.html.replace("%hallname%", &hall_name);
                    html
                })
        };
        if let Some(mut html) = html {
            html.html = html.html.replace("%objectId%", &npc.0.to_string());
            self.send_html(player, &html);
        }
    }

    /// Java `ClanHallDoorman.onBypassFeedback(player, command)` (and `Doorman`'s).
    pub(crate) fn clan_hall_doorman_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        let has_evolve = self
            .doorman_hall(npc)
            .is_some_and(|hall| CH_WITH_EVOLVE.contains(&hall.0));
        if has_evolve && command.starts_with("evolve") && self.doorman_is_owner_clan(npc, player) {
            self.doorman_evolve(npc, player, command);
            return;
        }
        if command.starts_with("Chat") {
            self.clan_hall_doorman_show_chat_window(npc, player);
        } else if command.starts_with("open_doors") {
            if self.doorman_is_owner_clan(npc, player) {
                // `isUnderSiege()` is false for a hall doorman.
                self.doorman_doors(npc, player, true);
            }
        } else if command.starts_with("close_doors") {
            if self.doorman_is_owner_clan(npc, player) {
                self.doorman_doors(npc, player, false);
            }
        } else if command.starts_with("tele") {
            if self.doorman_is_owner_clan(npc, player) {
                let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
                let data = self.talk.teleporters.clone();
                if let Some(holder) = data.as_ref().and_then(|d| d.get_holder(npc_id, "OTHER")) {
                    // `Integer.parseInt(command.substring(5).trim())`.
                    let Some(location) = command.get(5..).and_then(|t| t.trim().parse::<i32>().ok()) else {
                        warn!("ClanHallDoorman: bad tele command {command:?} from {player}");
                        return;
                    };
                    self.do_teleport(npc, player, holder, location);
                }
            }
        } else {
            self.generic_npc_bypass_feedback(npc, player, command);
        }
    }

    /// `openDoors` / `closeDoors` of the hall doorman: every door of the hall, then the page.
    fn doorman_doors(&mut self, npc: ObjectId, player: ObjectId, open: bool) {
        let Some(hall) = self.doorman_hall(npc) else {
            return;
        };
        self.manager_open_close_doors(hall, open);
        let path = if open {
            "data/html/clanHallDoorman/doorman-opened.htm"
        } else {
            "data/html/clanHallDoorman/doorman-closed.htm"
        };
        if let Some(mut html) = self.html_from_file(player, npc.0, path) {
            html.html = html.html.replace("%objectId%", &npc.0.to_string());
            self.send_html(player, &html);
        }
    }

    /// `evolve <n>`: Java calls `PetManager.doEvolve`, which is not ported yet, so the answer is
    /// always the "no" page.
    // FOLLOW-UP(PetManager.doEvolve): the five evolutions (9882/10307, 4422/10308, 4423/10309,
    // 4424/10310 at level 55 and 10426/10611 at level 70).
    fn doorman_evolve(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        let mut tokens = command.split(' ').filter(|t| !t.is_empty());
        if tokens.clone().count() < 2 {
            return;
        }
        tokens.next();
        // `Integer.parseInt` throws for anything else.
        if tokens.next().and_then(|t| t.parse::<i32>().ok()).is_none() {
            warn!("ClanHallDoorman: bad evolve command {command:?} from {player}");
            return;
        }
        if let Some(html) = self.html_from_file(player, npc.0, "data/html/clanHallDoorman/evolve-no.htm") {
            self.send_html(player, &html);
        }
    }
}
