//! Castles: the castle table, the owner, the tax and the treasury, the circlets.
// Java: managers/CastleManager.java
// Java: mechanics/siege/Castle.java (setOwner, removeOwner, updateOwnerInDB, addToTreasury,
//       addToTreasuryNoTax, setTaxPercent, setShowNpcCrest, setTicketBuyCount,
//       updateClansReputation, checkIfInZone, getDistance)
// Java: entity/actor/Npc.java (getCastle, isInTown)

use std::collections::BTreeMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{PlaySound, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::{CastleId, ResidenceRef, ResidenceWrite, Writes};
use crate::world::World;

/// A castle (Java `Castle` without its siege).
#[derive(Debug, Clone, PartialEq)]
pub struct Castle {
    pub id: CastleId,
    /// `name` of the `castle` row ("Gludio" ... "Schuttgart").
    pub name: String,
    /// `_ownerId`: the clan whose `hasCastle` is this castle, 0 for none.
    pub owner_id: i32,
    /// `_taxPercent`.
    pub tax_percent: i32,
    /// `_taxRate`: `taxPercent / 100.0`.
    pub tax_rate: f64,
    /// `_treasury`.
    pub treasury: i64,
    /// `_siegeDate` (epoch milliseconds), for the siege card.
    pub siege_date: i64,
    /// `_isTimeRegistrationOver` (`regTimeOver`).
    pub time_registration_over: bool,
    /// `_siegeTimeRegistrationEndDate` (`regTimeEnd`, epoch milliseconds).
    pub time_registration_over_date: i64,
    /// `_showNpcCrest`.
    pub show_npc_crest: bool,
    /// `_ticketBuyCount`: the mercenary tickets exchanged.
    pub ticket_buy_count: i32,
    /// `_formerOwner`: the clan that owned the castle before the current siege (set by the
    /// first owner change, read by `updateClansReputation`).
    pub former_owner: Option<i32>,
    /// `_isFirstMidVictory`.
    pub first_mid_victory: bool,
    /// `_function`: the current function objects by type (see `functions.rs`).
    pub(crate) functions: BTreeMap<i32, u64>,
    /// `_doors`: the doors inside the castle's siege zone (`activateInstance`).
    pub doors: Vec<ObjectId>,
    /// The `CastleZone` (Java `getResidenceZone()`).
    pub zone: Option<i32>,
    /// The `SiegeZone` (Java `getZone()`).
    pub siege_zone: Option<i32>,
    /// The `ResidenceTeleportZone` (Java `getTeleZone()`).
    pub teleport_zone: Option<i32>,
    /// `_residentialSkills` (`SkillTreeData.getAvailableResidentialSkills(id)`): `(id, level)`.
    pub residential_skills: Vec<(i32, i32)>,
}

impl Castle {
    /// A castle with Java's field defaults (`new Castle(id)` before `load()` reads its row).
    pub fn new(id: CastleId) -> Self {
        Self {
            id,
            name: String::new(),
            owner_id: 0,
            tax_percent: 0,
            tax_rate: 0.0,
            treasury: 0,
            siege_date: 0,
            time_registration_over: true,
            time_registration_over_date: 0,
            show_npc_crest: false,
            ticket_buy_count: 0,
            former_owner: None,
            first_mid_victory: false,
            functions: BTreeMap::new(),
            doors: Vec::new(),
            zone: None,
            siege_zone: None,
            teleport_zone: None,
            residential_skills: Vec::new(),
        }
    }
}

/// `name.equalsIgnoreCase(other)` for the castle names (ASCII).
fn is_named(castle: &Castle, name: &str) -> bool {
    castle.name.eq_ignore_ascii_case(name)
}

impl World {
    // ------------------------------------------------------------------ CastleManager lookups

    /// Java `CastleManager.getCastleById(castleId)`.
    pub fn castle(&self, id: CastleId) -> Option<&Castle> {
        self.residences.castles.iter().find(|c| c.id == id)
    }

    pub(crate) fn castle_mut(&mut self, id: CastleId) -> Option<&mut Castle> {
        self.residences.castles.iter_mut().find(|c| c.id == id)
    }

    /// Java `CastleManager.getCastles()`, in Java's list order.
    pub fn castles(&self) -> &[Castle] {
        &self.residences.castles
    }

    /// Java `CastleManager.getCastleByOwner(clan)`: the first castle whose owner is the clan.
    pub fn castle_by_owner(&self, clan_id: i32) -> Option<CastleId> {
        if clan_id <= 0 {
            return None;
        }
        self.residences
            .castles
            .iter()
            .find(|c| c.owner_id == clan_id)
            .map(|c| c.id)
    }

    /// Java `CastleManager.getCastle(name)`: ignoring case, the name trimmed.
    pub fn castle_by_name(&self, name: &str) -> Option<CastleId> {
        let name = name.trim();
        self.residences
            .castles
            .iter()
            .find(|c| is_named(c, name))
            .map(|c| c.id)
    }

    /// Java `CastleManager.getCastleIndex(castleId)`.
    pub fn castle_index(&self, id: CastleId) -> Option<usize> {
        self.residences.castles.iter().position(|c| c.id == id)
    }

    /// Java `Castle.checkIfInZone(x, y, z)`: inside the castle's siege zone.
    pub fn castle_contains(&self, id: CastleId, x: i32, y: i32, z: i32) -> bool {
        let Some(zone) = self.castle(id).and_then(|c| c.siege_zone) else {
            return false;
        };
        self.zoning
            .zones
            .as_ref()
            .and_then(|zones| zones.get_of_type(zone, "SiegeZone"))
            .is_some_and(|zone| zone.is_inside_zone(x, y, z))
    }

    /// Java `CastleManager.getCastle(x, y, z)` / `getCastleIndex(x, y, z)`: the first castle
    /// whose siege zone holds the point.
    pub fn castle_at(&self, x: i32, y: i32, z: i32) -> Option<CastleId> {
        self.residences
            .castles
            .iter()
            .find(|c| self.castle_contains(c.id, x, y, z))
            .map(|c| c.id)
    }

    /// Java `Castle.getDistance(obj)`: the distance to the castle's siege zone (`None` where Java
    /// throws, a castle without one).
    pub fn castle_distance(&self, id: CastleId, x: i32, y: i32) -> Option<f64> {
        let zone = self.castle(id)?.siege_zone?;
        let zone = self.zoning.zones.as_ref()?.get_of_type(zone, "SiegeZone")?;
        Some(zone.distance_to_zone(x, y))
    }

    /// Java `CastleManager.findNearestCastleIndex(obj, maxDistance)`: the castle the point is in,
    /// else the nearest one closer than `max_distance`.
    pub fn nearest_castle(&self, x: i32, y: i32, z: i32, max_distance: i64) -> Option<CastleId> {
        if let Some(id) = self.castle_at(x, y, z) {
            return Some(id);
        }
        let mut max = max_distance;
        let mut found = None;
        for castle in &self.residences.castles {
            let Some(distance) = self.castle_distance(castle.id, x, y) else {
                continue;
            };
            // `if (maxDistance > distance) { maxDistance = (long) distance; index = i; }`
            if (max as f64) > distance {
                max = distance as i64;
                found = Some(castle.id);
            }
        }
        found
    }

    /// Java `CastleManager.hasOwnedCastle()`.
    pub fn has_owned_castle(&self) -> bool {
        self.residences.castles.iter().any(|c| c.owner_id > 0)
    }

    /// Java `Npc.getCastle()` and `isInTown()`: the castle of the town the NPC stands in (the
    /// `taxById` of its `TownZone`), else the castle whose siege zone holds it, else the nearest
    /// one. The flag is `isInTown()`: the castle came from the town.
    // NOTE: Java caches the answer of the first call in the NPC; a merchant never moves, so
    // asking again gives the same answer.
    pub fn npc_castle_and_town(&self, npc: ObjectId) -> Option<(CastleId, bool)> {
        let (x, y, z) = self.position_of(npc)?;
        let town = self.zoning.zones.as_ref().and_then(|zones| {
            zones
                .zones_at(x, y, z)
                .into_iter()
                .find(|zone| zone.zone_type == "TownZone")
                .map(|zone| zone.int_param("taxById").unwrap_or(0))
        });
        if let Some(tax_by_id) = town
            && let Some(castle) = self.castle(CastleId(tax_by_id))
        {
            return Some((castle.id, true));
        }
        self.nearest_castle(x, y, z, i64::MAX).map(|id| (id, false))
    }

    /// Java `Npc.getCastle()`.
    pub fn npc_castle(&self, npc: ObjectId) -> Option<CastleId> {
        self.npc_castle_and_town(npc).map(|(id, _)| id)
    }

    /// Java `Npc.isInTown()`.
    pub fn npc_is_in_town(&self, npc: ObjectId) -> bool {
        self.npc_castle_and_town(npc).is_some_and(|(_, town)| town)
    }

    // ------------------------------------------------------------------ tax and treasury

    /// Java `Castle.setTaxPercent(taxPercent)`: what the castle lord sets at the chamberlain.
    pub fn castle_set_tax_percent(&mut self, id: CastleId, tax_percent: i32) {
        let Some(castle) = self.castle_mut(id) else {
            return;
        };
        castle.tax_percent = tax_percent;
        castle.tax_rate = f64::from(tax_percent) / 100.0;
        self.residence_write(ResidenceWrite::CastleTaxPercent {
            castle_id: id.0,
            tax_percent,
        });
    }

    /// Java `Castle.addToTreasury(amount)`: the tax income of a shop or a multisell. Schuttgart
    /// and Goddard pay Rune's tax rate of it to Rune, every castle but Aden, Rune, Schuttgart and
    /// Goddard pays Aden's; the rest goes to the castle. Nothing for a castle without an owner.
    /// The treasury rows are written only with `SiegeDatabaseWrites` (D-51: the Java game server
    /// collects the real tax).
    pub fn castle_add_to_treasury(&mut self, id: CastleId, amount: i64) {
        let Some(castle) = self.castle(id) else {
            return;
        };
        if castle.owner_id <= 0 {
            return;
        }
        let name = castle.name.clone();
        let mut amount = amount;
        if name.eq_ignore_ascii_case("Schuttgart") || name.eq_ignore_ascii_case("Goddard") {
            if let Some(rune) = self.castle_by_name("rune").and_then(|r| self.castle(r)) {
                let rune_id = rune.id;
                let rune_tax = (amount as f64 * rune.tax_rate) as i64;
                if rune.owner_id > 0 {
                    self.castle_add_to_treasury(rune_id, rune_tax);
                }
                amount -= rune_tax;
            }
        }
        // If current castle instance is not Aden, Rune, Goddard or Schuttgart.
        let excluded = ["aden", "Rune", "Schuttgart", "Goddard"];
        if !excluded.iter().any(|n| name.eq_ignore_ascii_case(n)) {
            if let Some(aden) = self.castle_by_name("aden").and_then(|a| self.castle(a)) {
                let aden_id = aden.id;
                // Find out what Aden gets from the current castle instance's income.
                let aden_tax = (amount as f64 * aden.tax_rate) as i64;
                if aden.owner_id > 0 {
                    // Only bother to really add the tax to the treasury if not npc owned.
                    self.castle_add_to_treasury(aden_id, aden_tax);
                }
                amount -= aden_tax;
            }
        }
        self.castle_add_to_treasury_no_tax(id, amount, Writes::Job);
    }

    /// Java `Castle.addToTreasuryNoTax(amount)`: a negative amount takes adena out (`false` when
    /// the treasury has less), a positive one stops at `MaxAdena`. The tax income is a
    /// [`Writes::Job`] (D-51); the castle lord's own deposit or withdrawal at the chamberlain is a
    /// [`Writes::Player`].
    pub fn castle_add_to_treasury_no_tax(&mut self, id: CastleId, amount: i64, writes: Writes) -> bool {
        let max_adena = self.settings.ground.max_adena;
        let Some(castle) = self.castle_mut(id) else {
            return false;
        };
        if castle.owner_id <= 0 {
            return false;
        }
        if amount < 0 {
            let amount = amount.wrapping_neg();
            if castle.treasury < amount {
                return false;
            }
            castle.treasury -= amount;
        } else if castle.treasury.saturating_add(amount) > max_adena {
            castle.treasury = max_adena;
        } else {
            castle.treasury += amount;
        }
        let write = ResidenceWrite::CastleTreasury {
            castle_id: id.0,
            treasury: castle.treasury,
        };
        self.write_residence(writes, write);
        true
    }

    /// Java `Castle.setShowNpcCrest(showNpcCrest)`: the castle's crest on its NPCs (the reset of an
    /// owner change is a [`Writes::Job`], the castle lord's choice a [`Writes::Player`]).
    pub fn castle_set_show_npc_crest(&mut self, id: CastleId, show: bool, writes: Writes) {
        let Some(castle) = self.castle_mut(id) else {
            return;
        };
        if castle.show_npc_crest == show {
            return;
        }
        castle.show_npc_crest = show;
        let write = ResidenceWrite::CastleShowNpcCrest {
            castle_id: id.0,
            show,
        };
        self.write_residence(writes, write);
    }

    /// Java `Castle.setTicketBuyCount(count)`: the mercenary tickets exchanged (a player's
    /// purchase, always written).
    pub fn castle_set_ticket_buy_count(&mut self, id: CastleId, count: i32) {
        let Some(castle) = self.castle_mut(id) else {
            return;
        };
        castle.ticket_buy_count = count;
        self.residence_write(ResidenceWrite::CastleTicketBuyCount {
            castle_id: id.0,
            count,
        });
    }

    // ------------------------------------------------------------------ owner

    /// Java `Castle.setOwner(clan)`: the castle goes to `clan` (`None`: to nobody). The old owner
    /// (when another clan) loses the castle, its circlets (`RemoveCastleCirclets`) and the
    /// residential skills; the new owner gets them, with `PledgeShowInfoUpdate` and the victory
    /// sound. The rows only with `SiegeDatabaseWrites` (D-51).
    pub fn castle_set_owner(&mut self, id: CastleId, clan: Option<i32>) {
        let Some(owner_id) = self.castle(id).map(|c| c.owner_id) else {
            return;
        };
        // Remove old owner.
        if owner_id > 0 && clan != Some(owner_id) {
            // Try to find clan instance.
            if self.clan(owner_id).is_some() {
                let former_unset = self.castle(id).is_some_and(|c| c.former_owner.is_none());
                if former_unset {
                    if let Some(castle) = self.castle_mut(id) {
                        castle.former_owner = Some(owner_id);
                    }
                    if self.residences.settings.remove_castle_circlets {
                        self.remove_clan_circlets(owner_id, id.0, Writes::Job);
                    }
                }
                // The old leader on a wyvern dismounts.
                if let Some(leader) = self.clan(owner_id).and_then(|c| c.leader)
                    && self.players.contains_key(&leader)
                    && self.mount_type_of(leader) == crate::mount::MountType::Wyvern
                {
                    self.dismount(leader);
                }
                // Unset has castle flag for old owner.
                if let Some(old) = self.clan_mut(owner_id) {
                    old.castle_id = 0;
                }
                for member in self.clan_online_members(owner_id, 0) {
                    self.remove_residential_skills(ResidenceRef::Castle(id), member);
                    self.send_skill_list(member);
                }
            }
        }
        // Update in database.
        self.castle_update_owner_in_db(id, clan);
        self.castle_set_show_npc_crest(id, false, Writes::Job);
        // If clan have fortress, remove it.
        if let Some(clan_id) = clan
            && self.clan(clan_id).is_some_and(|c| c.fort_id > 0)
            && let Some(fort) = self.fort_by_owner(clan_id)
        {
            self.fort_remove_owner(fort, true);
        }
        // Java: `if (getSiege().isInProgress()) getSiege().midVictory()` (M7-SIEGE-01).
        if self.is_castle_siege_in_progress(id) {
            self.siege_mid_victory(id);
        }
        // `TerritoryWarManager.getTerritory(getResidenceId()).setOwnerClan(clan)` (M7-TW-01).
        self.territory_set_owner_clan(id.0, clan);
        if let Some(clan_id) = clan {
            for member in self.clan_online_members(clan_id, 0) {
                self.give_residential_skills(ResidenceRef::Castle(id), member);
                self.send_skill_list(member);
            }
        }
    }

    /// Java `Castle.removeOwner(clan)`: the clan loses the castle (its circlets, the residential
    /// skills, `PledgeShowInfoUpdate`), nobody owns it, and its functions go.
    pub fn castle_remove_owner(&mut self, id: CastleId, clan: Option<i32>) {
        if self.castle(id).is_none() {
            return;
        }
        if let Some(clan_id) = clan
            && self.clan(clan_id).is_some()
        {
            if let Some(castle) = self.castle_mut(id) {
                castle.former_owner = Some(clan_id);
            }
            if self.residences.settings.remove_castle_circlets {
                self.remove_clan_circlets(clan_id, id.0, Writes::Job);
            }
            for member in self.clan_online_members(clan_id, 0) {
                self.remove_residential_skills(ResidenceRef::Castle(id), member);
                self.send_skill_list(member);
            }
            if let Some(c) = self.clan_mut(clan_id) {
                c.castle_id = 0;
            }
            self.broadcast_clan_info_update(clan_id);
        }
        self.castle_update_owner_in_db(id, None);
        // Java: `if (getSiege().isInProgress()) getSiege().midVictory()` (M7-SIEGE-01).
        if self.is_castle_siege_in_progress(id) {
            self.siege_mid_victory(id);
        }
        self.residence_remove_all_functions(ResidenceRef::Castle(id));
    }

    /// Java `Castle.updateOwnerInDB(clan)`: the owner id, the two `clan_data` updates (D-51:
    /// only with `SiegeDatabaseWrites`), and for a new owner its castle id,
    /// `PledgeShowInfoUpdate` and the `Siege_Victory` sound to its online members.
    fn castle_update_owner_in_db(&mut self, id: CastleId, clan: Option<i32>) {
        let owner_id = clan.unwrap_or(0);
        if let Some(castle) = self.castle_mut(id) {
            castle.owner_id = owner_id;
        }
        if clan.is_none() {
            // Remove owner: the castle loses its manor.
            self.manor_reset_data(id.0);
        }
        let written = self.siege_write(ResidenceWrite::CastleOwner {
            castle_id: id.0,
            clan_id: owner_id,
        });
        self.set_residence_owner_source(ResidenceRef::Castle(id), written);
        // Announce to clan members.
        if let Some(clan_id) = clan
            && self.clan(clan_id).is_some()
        {
            if let Some(c) = self.clan_mut(clan_id) {
                // Set has castle flag for new owner.
                c.castle_id = id.0;
            }
            self.broadcast_clan_info_update(clan_id);
            self.broadcast_to_clan(clan_id, &PlaySound::with_unknown1(1, "Siege_Victory"));
        }
    }

    /// Java `Castle.updateClansReputation()`: after a siege the former owner loses
    /// `LooseCastlePoints` and the new owner gains `TakeCastlePoints` (at most what the former
    /// owner had), or the defender gains `CastleDefendedPoints`; with the messages.
    pub fn castle_update_clans_reputation(&mut self, id: CastleId) {
        let Some((owner, former)) = self.castle(id).map(|c| (c.owner_id, c.former_owner)) else {
            return;
        };
        let owner = self.clan(owner).map(|c| c.id);
        let settings = self.residences.settings.clone();
        let victorious =
            SystemMessageId::SinceYourClanEmergedVictoriousFromTheSiegeS1PointsHaveBeenAddedToYourClanSReputationScore;
        // `_formerOwner` is a clan object in Java; a clan destroyed since then has no score here
        // (its reputation is taken from nobody, the reward is at most 0).
        match former {
            Some(former) if Some(former) != owner => {
                let max_reward = self.clan(former).map_or(0, |c| c.reputation_score.max(0));
                self.take_reputation_score(former, settings.loose_castle_points);
                if let Some(owner) = owner {
                    let reward = settings.take_castle_points.min(max_reward);
                    self.add_reputation_score(owner, reward);
                    self.broadcast_to_clan(
                        owner,
                        &SystemMessage::new(victorious).with(SystemMessageParam::IntNumber(reward)),
                    );
                }
                self.broadcast_to_clan(
                    former,
                    &SystemMessage::new(SystemMessageId::SinceYourClanWasDefeatedInASiegeS1PointsHaveBeenDeductedFromYourClanSReputationScoreAndGivenToTheOpposingClan)
                        .with(SystemMessageParam::IntNumber(settings.loose_castle_points)),
                );
            }
            Some(former) => {
                self.add_reputation_score(former, settings.castle_defended_points);
                self.broadcast_to_clan(
                    former,
                    &SystemMessage::new(victorious)
                        .with(SystemMessageParam::IntNumber(settings.castle_defended_points)),
                );
            }
            None => {
                if let Some(owner) = owner {
                    self.add_reputation_score(owner, settings.take_castle_points);
                    self.broadcast_to_clan(
                        owner,
                        &SystemMessage::new(victorious)
                            .with(SystemMessageParam::IntNumber(settings.take_castle_points)),
                    );
                }
            }
        }
    }

    // ------------------------------------------------------------------ circlets

    /// Java `CastleManager.removeCirclet(clan, castleId)`: every member of the clan loses the
    /// castle's circlet; `writes` decides about the rows of offline members.
    pub(crate) fn remove_clan_circlets(&mut self, clan_id: i32, castle_id: i32, writes: Writes) {
        let members: Vec<ObjectId> = self
            .clan(clan_id)
            .map(|c| c.members().iter().map(|m| m.object_id).collect())
            .unwrap_or_default();
        for member in members {
            self.remove_member_circlet(clan_id, member, castle_id, writes);
        }
    }

    /// Java `CastleManager.removeCirclet(member, castleId)`: an online member's circlet is taken
    /// off and destroyed ("... has disappeared"); an offline member's rows are deleted as
    /// `writes` says.
    pub(crate) fn remove_member_circlet(
        &mut self,
        clan_id: i32,
        member: ObjectId,
        castle_id: i32,
        writes: Writes,
    ) {
        let online = self
            .clan(clan_id)
            .and_then(|c| c.member(member))
            .is_some_and(|m| m.has_player)
            && self.players.contains_key(&member);
        self.remove_circlet_of(member, castle_id, online, writes);
    }

    /// `Clan.removeClanMember`'s `CastleManager.removeCirclet(exMember, getCastleId())`: the member
    /// is out of the clan already; `online` is `exMember.getPlayer() != null`.
    pub(crate) fn remove_member_circlet_of_ex_member(
        &mut self,
        member: ObjectId,
        castle_id: i32,
        online: bool,
    ) {
        self.remove_circlet_of(member, castle_id, online, Writes::Player);
    }

    fn remove_circlet_of(&mut self, member: ObjectId, castle_id: i32, online: bool, writes: Writes) {
        let circlet = l2_data::residence::circlet_by_castle_id(castle_id);
        if circlet == 0 {
            return;
        }
        if online {
            // Online-player circlet removal.
            if let Some(item) = self.inventory_item_by_item_id(member, circlet).cloned() {
                if item.loc == "PAPERDOLL" {
                    self.unequip_item_in_slot_and_record(member, usize::try_from(item.loc_data).unwrap_or(0));
                }
                self.destroy_item_by_item_id(member, circlet, 1, true);
            }
            return;
        }
        // Else offline-player circlet removal.
        let write = ResidenceWrite::DeleteCirclets {
            owner_id: member.0,
            item_id: circlet,
        };
        self.write_residence(writes, write);
    }
}
