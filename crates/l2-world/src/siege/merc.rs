//! Mercenary tickets: the castle lord places a ticket on the castle grounds, which hires a
//! mercenary for the next siege (a `castle_siege_guards` row) and lies there as an item until the
//! siege starts or the lord picks it up again.
// Java: managers/MercTicketManager.java
// Java: dist/game/data/scripts/handlers/items/MercTicket.java
// Java: dist/game/data/scripts/handlers/actions/click/ItemClick.java (the castle lord rule)
// Java: network/clientpackets/RequestPetGetItem.java (no tickets for pets)
// Java: entity/item/instance/Item.java (pickupMe: removeTicket)
// Java: mechanics/sevensigns/SevenSigns.java (checkIsDawnPostingTicket, checkIsRookiePostingTicket)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::info;

use super::SiegeTimer;
use crate::ground_item::GroundItem;
use crate::player::ItemData;
use crate::residence::CastleId;
use crate::world::World;

/// `GUARDIAN_TYPES_COUNT`: the tickets of one castle.
const GUARDIAN_TYPES_COUNT: usize = 52;

/// Java `MercTicketManager.ITEM_IDS`: 52 tickets per castle, Gludio to Schuttgart.
#[rustfmt::skip]
pub const MERC_ITEM_IDS: [i32; 468] = [
    3960, 3961, 3962, 3963, 3964, 3965, 3966, 3967, 3968, 3969, 6115, 6116, 6117, 6118, 6119, 6120, 6121, 6122, 6123, 6124, 6038, 6039, 6040, 6041, 6042, 6043, 6044, 6045, 6046, 6047, 6175, 6176, 6177, 6178, 6179, 6180, 6181, 6182, 6183, 6184, 6235, 6236, 6237, 6238, 6239, 6240, 6241, 6242, 6243, 6244, 6295, 6296, // Gludio
    3973, 3974, 3975, 3976, 3977, 3978, 3979, 3980, 3981, 3982, 6125, 6126, 6127, 6128, 6129, 6130, 6131, 6132, 6133, 6134, 6051, 6052, 6053, 6054, 6055, 6056, 6057, 6058, 6059, 6060, 6185, 6186, 6187, 6188, 6189, 6190, 6191, 6192, 6193, 6194, 6245, 6246, 6247, 6248, 6249, 6250, 6251, 6252, 6253, 6254, 6297, 6298, // Dion
    3986, 3987, 3988, 3989, 3990, 3991, 3992, 3993, 3994, 3995, 6135, 6136, 6137, 6138, 6139, 6140, 6141, 6142, 6143, 6144, 6064, 6065, 6066, 6067, 6068, 6069, 6070, 6071, 6072, 6073, 6195, 6196, 6197, 6198, 6199, 6200, 6201, 6202, 6203, 6204, 6255, 6256, 6257, 6258, 6259, 6260, 6261, 6262, 6263, 6264, 6299, 6300, // Giran
    3999, 4000, 4001, 4002, 4003, 4004, 4005, 4006, 4007, 4008, 6145, 6146, 6147, 6148, 6149, 6150, 6151, 6152, 6153, 6154, 6077, 6078, 6079, 6080, 6081, 6082, 6083, 6084, 6085, 6086, 6205, 6206, 6207, 6208, 6209, 6210, 6211, 6212, 6213, 6214, 6265, 6266, 6267, 6268, 6269, 6270, 6271, 6272, 6273, 6274, 6301, 6302, // Oren
    4012, 4013, 4014, 4015, 4016, 4017, 4018, 4019, 4020, 4021, 6155, 6156, 6157, 6158, 6159, 6160, 6161, 6162, 6163, 6164, 6090, 6091, 6092, 6093, 6094, 6095, 6096, 6097, 6098, 6099, 6215, 6216, 6217, 6218, 6219, 6220, 6221, 6222, 6223, 6224, 6275, 6276, 6277, 6278, 6279, 6280, 6281, 6282, 6283, 6284, 6303, 6304, // Aden
    5205, 5206, 5207, 5208, 5209, 5210, 5211, 5212, 5213, 5214, 6165, 6166, 6167, 6168, 6169, 6170, 6171, 6172, 6173, 6174, 6105, 6106, 6107, 6108, 6109, 6110, 6111, 6112, 6113, 6114, 6225, 6226, 6227, 6228, 6229, 6230, 6231, 6232, 6233, 6234, 6285, 6286, 6287, 6288, 6289, 6290, 6291, 6292, 6293, 6294, 6305, 6306, // Innadril
    6779, 6780, 6781, 6782, 6783, 6784, 6785, 6786, 6787, 6788, 6802, 6803, 6804, 6805, 6806, 6807, 6808, 6809, 6810, 6811, 6792, 6793, 6794, 6795, 6796, 6797, 6798, 6799, 6800, 6801, 6812, 6813, 6814, 6815, 6816, 6817, 6818, 6819, 6820, 6821, 6822, 6823, 6824, 6825, 6826, 6827, 6828, 6829, 6830, 6831, 6832, 6833, // Goddard
    7973, 7974, 7975, 7976, 7977, 7978, 7979, 7980, 7981, 7982, 7998, 7999, 8000, 8001, 8002, 8003, 8004, 8005, 8006, 8007, 7988, 7989, 7990, 7991, 7992, 7993, 7994, 7995, 7996, 7997, 8008, 8009, 8010, 8011, 8012, 8013, 8014, 8015, 8016, 8017, 8018, 8019, 8020, 8021, 8022, 8023, 8024, 8025, 8026, 8027, 8028, 8029, // Rune
    7918, 7919, 7920, 7921, 7922, 7923, 7924, 7925, 7926, 7927, 7941, 7942, 7943, 7944, 7945, 7946, 7947, 7948, 7949, 7950, 7931, 7932, 7933, 7934, 7935, 7936, 7937, 7938, 7939, 7940, 7951, 7952, 7953, 7954, 7955, 7956, 7957, 7958, 7959, 7960, 7961, 7962, 7963, 7964, 7965, 7966, 7967, 7968, 7969, 7970, 7971, 7972, // Schuttgart
];

/// Java `MercTicketManager.NPC_IDS`: the same 52 mercenaries (35010 to 35061) for every castle.
pub const MERC_NPC_IDS: [i32; 468] = merc_npc_ids();

const fn merc_npc_ids() -> [i32; 468] {
    let mut ids = [0; 468];
    let mut i = 0;
    while i < 468 {
        ids[i] = 35010 + (i % GUARDIAN_TYPES_COUNT) as i32;
        i += 1;
    }
    ids
}

/// Java `MercTicketManager.MAX_MERC_PER_TYPE` of a castle (index 0 is Gludio).
const MAX_MERC_PER_TYPE: [i32; 9] = [10, 15, 10, 10, 20, 20, 20, 20, 20];

/// Java `MercTicketManager.MERCS_MAX_PER_CASTLE` before `Siege.ini` (`<Name>MaxMercenaries`).
pub const MERCS_MAX_PER_CASTLE: [i32; 9] = [100, 150, 200, 300, 400, 400, 400, 400, 400];

/// Java `MercTicketManager.getTicketCastleId(itemId)`: the castle of a ticket, -1 for none.
// NOTE(java-quirk): the check of the last two tickets reads Gludio's (6295, 6296) for every castle,
// so those two answer castle 1 and the last two tickets of the other castles answer -1.
pub fn ticket_castle_id(item_id: i32) -> i32 {
    for i in 0..9 {
        for i2 in (0..50).step_by(10) {
            let base = i2 + i * GUARDIAN_TYPES_COUNT;
            if item_id >= MERC_ITEM_IDS[base] && item_id <= MERC_ITEM_IDS[base + 9] {
                return i as i32 + 1;
            }
        }
        if item_id >= MERC_ITEM_IDS[50] && item_id <= MERC_ITEM_IDS[51] {
            return i as i32 + 1;
        }
    }
    -1
}

/// The first index of a ticket in [`MERC_ITEM_IDS`].
fn ticket_index(item_id: i32) -> Option<usize> {
    MERC_ITEM_IDS.iter().position(|id| *id == item_id)
}

/// Java `SevenSigns.checkIsDawnPostingTicket(itemId)`.
fn is_dawn_posting_ticket(item_id: i32) -> bool {
    (6115..6175).contains(&item_id)
        || (6802..6812).contains(&item_id)
        || (7998..8008).contains(&item_id)
        || (7941..7951).contains(&item_id)
        || (6295..6307).contains(&item_id)
        || (6832..6834).contains(&item_id)
        || (8028..8030).contains(&item_id)
        || (7971..7973).contains(&item_id)
}

/// Java `SevenSigns.checkIsRookiePostingTicket(itemId)`.
fn is_rookie_posting_ticket(item_id: i32) -> bool {
    (6175..6295).contains(&item_id)
        || (6812..6832).contains(&item_id)
        || (7951..7971).contains(&item_id)
        || (8008..8028).contains(&item_id)
}

/// `SevenSigns.CABAL_DUSK` / `CABAL_DAWN`.
const CABAL_DUSK: i32 = 1;

impl World {
    /// Java `MercTicketManager.getMaxAllowedMerc(castleId)`.
    pub fn merc_max_allowed(&self, castle_id: i32) -> i32 {
        self.sieges
            .settings
            .castles
            .get(&castle_id)
            .map(|c| c.max_mercenaries)
            .or_else(|| {
                usize::try_from(castle_id - 1)
                    .ok()
                    .and_then(|i| MERCS_MAX_PER_CASTLE.get(i).copied())
            })
            .unwrap_or(0)
    }

    /// `MercTicketManager.DROPPED_TICKETS`.
    pub fn dropped_merc_tickets(&self) -> &[ObjectId] {
        &self.sieges.dropped_tickets
    }

    /// Java `MercTicketManager.load()`: a ticket on the ground for every hired mercenary whose
    /// castle has no siege running (at most `MaxMercenaries` per castle).
    pub(crate) fn merc_tickets_load(&mut self) {
        let rows: Vec<_> = self
            .sieges
            .guard_rows
            .iter()
            .filter(|r| r.is_hired)
            .copied()
            .collect();
        let mut placed = [0_i32; 20];
        // NOTE(java-quirk): the start index of the search is kept from the last row with a castle.
        let mut start = 0;
        for row in rows {
            let castle = self.castle_at(row.x, row.y, row.z);
            if let Some(castle) = castle {
                let index = usize::try_from(castle.0 - 1).unwrap_or(0).min(19);
                if placed[index] >= self.merc_max_allowed(castle.0) {
                    continue;
                }
                start = GUARDIAN_TYPES_COUNT * index;
                placed[index] += 1;
            }
            // Find the FIRST ticket itemId with spawns the saved NPC in the saved location.
            let end = (start + GUARDIAN_TYPES_COUNT).min(MERC_NPC_IDS.len());
            if let Some(i) = (start..end).find(|i| MERC_NPC_IDS[*i] == row.npc_id) {
                // Only handle tickets if a siege is not ongoing in this npc's castle.
                if let Some(castle) = castle
                    && !self.is_castle_siege_in_progress(castle)
                {
                    self.merc_drop_ticket(MERC_ITEM_IDS[i], row.x, row.y, row.z);
                }
            }
        }
        info!(
            "MercTicketManager: Loaded {} Mercenary Tickets",
            self.sieges.dropped_tickets.len()
        );
    }

    /// `new Item(id, itemId)`, `dropMe(null, x, y, z)`, `setDropTime(0)`, `DROPPED_TICKETS.add`.
    fn merc_drop_ticket(&mut self, item_id: i32, x: i32, y: i32, z: i32) {
        let Some(template) = self.item_templates().and_then(|t| t.get(item_id).cloned()) else {
            return;
        };
        let Some(object_id) = self.allocate_item_id() else {
            return;
        };
        let data: ItemData = self.new_item_data(&template, object_id, 1);
        if !self.drop_me(GroundItem::new(data), None, x, y, z) {
            return;
        }
        // Avoids it from being removed by the auto item destroyer.
        if let Some(item) = self.ground_item_mut(object_id) {
            item.drop_time_ms = None;
        }
        self.sieges.dropped_tickets.push(object_id);
    }

    /// Java `isAtTypeLimit(itemId)`.
    fn merc_is_at_type_limit(&self, item_id: i32) -> bool {
        let limit = ticket_index(item_id)
            .map(|i| MAX_MERC_PER_TYPE[i / GUARDIAN_TYPES_COUNT])
            .unwrap_or(-1);
        if limit <= 0 {
            return true;
        }
        let count = self
            .sieges
            .dropped_tickets
            .iter()
            .filter(|t| self.ground_item(**t).is_some_and(|g| g.data.item_id == item_id))
            .count();
        i32::try_from(count).unwrap_or(i32::MAX) >= limit
    }

    /// Java `isAtCasleLimit(itemId)`.
    fn merc_is_at_castle_limit(&self, item_id: i32) -> bool {
        let castle_id = ticket_castle_id(item_id);
        if castle_id <= 0 {
            return true;
        }
        let limit = self.merc_max_allowed(castle_id);
        if limit <= 0 {
            return true;
        }
        let count = self
            .sieges
            .dropped_tickets
            .iter()
            .filter(|t| {
                self.ground_item(**t)
                    .is_some_and(|g| ticket_castle_id(g.data.item_id) == castle_id)
            })
            .count();
        i32::try_from(count).unwrap_or(i32::MAX) >= limit
    }

    /// Java `isTooCloseToAnotherTicket(x, y, z)`: another ticket within 25.
    fn merc_is_too_close_to_another_ticket(&self, x: i32, y: i32, z: i32) -> bool {
        self.sieges.dropped_tickets.iter().any(|t| {
            self.ground_item(*t).is_some_and(|g| {
                let (dx, dy, dz) = (f64::from(x - g.x), f64::from(y - g.y), f64::from(z - g.z));
                dx * dx + dy * dy + dz * dz < 25.0 * 25.0
            })
        })
    }

    /// Java `addTicket(itemId, player)`: the mercenary shows for 3 seconds, the hire is saved and
    /// the ticket lies where the lord stands. Returns the mercenary's NPC id, -1 when the lord is
    /// in no castle.
    fn merc_add_ticket(&mut self, item_id: i32, player: ObjectId) -> i32 {
        let Some((x, y, z)) = self.position_of(player) else {
            return -1;
        };
        let Some(castle) = self.castle_at(x, y, z) else {
            return -1;
        };
        let heading = self.heading_of(player);
        let Some(i) = ticket_index(item_id) else {
            return -1;
        };
        let npc_id = MERC_NPC_IDS[i];
        self.merc_spawn_mercenary(npc_id, x, y, z);
        // Hire merc for this castle. NpcId is at the same index as the item used.
        self.siege_hire_merc(castle, x, y, z, heading, npc_id);
        self.merc_drop_ticket(item_id, x, y, z);
        npc_id
    }

    /// Java `spawnMercenary(npcId, x, y, z, 3000)`.
    // NOTE: Java makes a `Defender` whatever the template's type (35061 is a `Folk`); here the
    // template's class is used for the 3 seconds the mercenary shows.
    fn merc_spawn_mercenary(&mut self, npc_id: i32, x: i32, y: i32, z: i32) {
        if let Some(npc) = self.spawn_npc_at(npc_id, x, y, z + 20, 0) {
            self.schedule_siege_timer(SiegeTimer::MercDespawn(npc), 3000);
        }
    }

    /// Java `deleteTickets(castleId)`: the castle's tickets vanish (the siege starts).
    pub(crate) fn merc_delete_tickets(&mut self, castle_id: i32) {
        let tickets = self.sieges.dropped_tickets.clone();
        for ticket in tickets {
            let Some(item_id) = self.ground_item(ticket).map(|g| g.data.item_id) else {
                continue;
            };
            if ticket_castle_id(item_id) == castle_id {
                self.decay_ground_item(ticket);
                self.sieges.dropped_tickets.retain(|t| *t != ticket);
            }
        }
    }

    /// `Item.pickupMe`'s "If this item is a mercenary ticket, remove the spawns!": Java
    /// `removeTicket(item)` (the hire is undone) and `ItemsOnGroundManager.removeObject`.
    pub(crate) fn merc_on_pickup(&mut self, item: &GroundItem) {
        let item_id = item.data.item_id;
        if ticket_castle_id(item_id) <= 0 {
            return;
        }
        // Find the FIRST ticket itemId with spawns the saved NPC in the saved location.
        let npc_id = ticket_index(item_id).map_or(-1, |i| MERC_NPC_IDS[i]);
        let castle = self.castle(CastleId(ticket_castle_id(item_id))).map(|c| c.id);
        if npc_id > 0 && castle.is_some() {
            self.siege_remove_merc(npc_id, item.x, item.y, item.z);
        }
        self.sieges.dropped_tickets.retain(|t| *t != item.object_id());
    }

    /// `ItemClick.onAction`'s rule: only the castle lord, out of a party, picks a ticket up.
    /// `false` when the click is refused (the item becomes the target).
    pub(crate) fn merc_ticket_click_allowed(&mut self, player: ObjectId, item: ObjectId) -> bool {
        let Some(item_id) = self.ground_item(item).map(|g| g.data.item_id) else {
            return true;
        };
        let castle_id = ticket_castle_id(item_id);
        let in_party = self.party_of(player).is_some();
        if castle_id > 0 && (!self.is_castle_lord(player, castle_id) || in_party) {
            if in_party {
                self.send_message(player, "You cannot pickup mercenaries while in a party.");
            } else {
                self.send_message(player, "Only the castle lord can pickup mercenaries.");
            }
            self.set_target(player, Some(item));
            self.set_intention_idle(player);
            return false;
        }
        true
    }

    /// Java `Player.isCastleLord(castleId)`: the leader of the clan that owns that castle.
    pub fn is_castle_lord(&self, player: ObjectId, castle_id: i32) -> bool {
        let Some(clan) = self.clan_of(player) else {
            return false;
        };
        if clan.leader != Some(player) || !self.is_online_clan_leader(player) {
            return false;
        }
        self.castle_by_owner(clan.id) == Some(CastleId(castle_id))
            && self.castle(CastleId(castle_id)).is_some()
    }

    /// `_clan.getLeader().getPlayer() == this`: the leader is online as this player.
    fn is_online_clan_leader(&self, player: ObjectId) -> bool {
        self.players.contains_key(&player) && self.is_clan_leader(player)
    }

    /// Java `MercTicket.onItemUse(playable, item, forceUse)`.
    pub(crate) fn merc_ticket_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        if !self.is_player(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryThisItem),
            );
            return false;
        }
        let item_id = item.item_id;
        let castle = self.position_of(id).and_then(|(x, y, z)| self.castle_at(x, y, z));
        let castle_id = castle.map_or(-1, |c| c.0);
        let refuse = |world: &Self, message: SystemMessageId| {
            world.send(id, &SystemMessage::new(message));
            false
        };
        // Add check that certain tickets can only be placed in certain castles.
        if ticket_castle_id(item_id) != castle_id {
            return refuse(self, SystemMessageId::MercenariesCannotBePositionedHere);
        } else if !self.is_castle_lord(id, castle_id) {
            return refuse(
                self,
                SystemMessageId::YouDoNotHaveTheAuthorityToPositionMercenaries,
            );
        } else if castle.is_some_and(|c| self.is_castle_siege_in_progress(c)) {
            return refuse(self, SystemMessageId::ThisMercenaryCannotBePositionedAnymore);
        }
        // Checking Seven Signs Quest Period.
        if !self.seven_signs_is_seal_validation() {
            return refuse(self, SystemMessageId::ThisMercenaryCannotBePositionedAnymore);
        }
        // Checking the Seal of Strife status.
        match self.seven_signs_strife_owner() {
            0 if is_dawn_posting_ticket(item_id) => {
                return refuse(self, SystemMessageId::ThisMercenaryCannotBePositionedAnymore);
            }
            CABAL_DUSK if !is_rookie_posting_ticket(item_id) => {
                return refuse(self, SystemMessageId::ThisMercenaryCannotBePositionedAnymore);
            }
            _ => {}
        }
        let Some((x, y, z)) = self.position_of(id) else {
            return false;
        };
        if self.merc_is_at_castle_limit(item_id) || self.merc_is_at_type_limit(item_id) {
            return refuse(self, SystemMessageId::ThisMercenaryCannotBePositionedAnymore);
        } else if self.merc_is_too_close_to_another_ticket(x, y, z) {
            return refuse(
                self,
                SystemMessageId::PositioningCannotBeDoneHereBecauseTheDistanceBetweenMercenariesIsTooShort,
            );
        }
        self.merc_add_ticket(item_id, id);
        // Remove item from char's inventory.
        self.destroy_item_by_object_id(id, item.object_id, 1, false);
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::PlaceS1InTheCurrentLocationAndDirectionDoYouWishToContinue)
                .with(SystemMessageParam::ItemName(item_id)),
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ticket_castles_follow_javas_lookup() {
        assert_eq!(ticket_castle_id(3960), 1);
        assert_eq!(ticket_castle_id(6244), 1);
        assert_eq!(ticket_castle_id(3973), 2);
        assert_eq!(ticket_castle_id(7970), 9);
        assert_eq!(ticket_castle_id(7972), -1);
        // Gludio's last two answer castle 1, the other castles' last two answer -1.
        assert_eq!(ticket_castle_id(6296), 1);
        assert_eq!(ticket_castle_id(6297), -1);
        assert_eq!(ticket_castle_id(57), -1);
        assert_eq!(MERC_NPC_IDS[0], 35010);
        assert_eq!(MERC_NPC_IDS[51], 35061);
        assert_eq!(MERC_NPC_IDS[52], 35010);
    }

    #[test]
    fn the_seven_signs_ticket_groups() {
        assert!(is_dawn_posting_ticket(6115) && !is_dawn_posting_ticket(6114));
        assert!(is_rookie_posting_ticket(6175) && !is_rookie_posting_ticket(6295));
    }
}
