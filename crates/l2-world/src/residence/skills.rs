//! Residential skills: what the members of a castle's or fortress's owner get (not stored).
// Java: entity/residences/AbstractResidence.java (initResidentialSkills, giveResidentialSkills,
//       removeResidentialSkills)
// Java: mechanics/siege/Castle.java (giveResidentialSkills, removeResidentialSkills: the
//       territory ward skills first)
// Java: network/clientpackets/{EnterWorld,RequestAnswerJoinPledge}.java ("Residential skills
//       support"), entity/clan/Clan.java (removeClanMember: "Remove Residential skills")

use l2_data::SkillTrees;
use l2_util::ObjectId;

use super::ResidenceRef;
use crate::world::World;

/// Java `SkillTreeData.getAvailableResidentialSkills(residenceId)`: the residence lines of the
/// clan tree that name the residence, in the tree's order, as `(id, level)`.
pub(crate) fn residential_skills_of(trees: &SkillTrees, residence_id: i32) -> Vec<(i32, i32)> {
    trees
        .pledge_skills()
        .iter()
        .filter(|s| s.residence_skill && s.residence_ids.contains(&residence_id))
        .map(|s| (s.skill_id, s.skill_level))
        .collect()
}

impl World {
    /// `_residentialSkills` of a castle or fortress (a clan hall has none).
    fn residential_skills(&self, residence: ResidenceRef) -> Vec<(i32, i32)> {
        match residence {
            ResidenceRef::Castle(id) => self.castle(id).map(|c| c.residential_skills.clone()),
            ResidenceRef::Fort(id) => self.fort(id).map(|f| f.residential_skills.clone()),
            ResidenceRef::ClanHall(_) => None,
        }
        .unwrap_or_default()
    }

    /// Java `giveResidentialSkills(player)`: `player.addSkill(skill, false)` for each (not stored).
    /// Returns whether the player got any.
    pub fn give_residential_skills(&mut self, residence: ResidenceRef, player: ObjectId) -> bool {
        // A castle first gives the skills of the territory wards its territory owns
        // (`TerritoryWarManager.getTerritory(id).getOwnedWardIds()`, M7-TW-01).
        let mut added = false;
        if let ResidenceRef::Castle(castle) = residence {
            added |= self.territory_give_castle_ward_skills(castle.0, player);
        }
        for (skill_id, level) in self.residential_skills(residence) {
            added |= self.add_skill(player, skill_id, level);
        }
        added
    }

    /// Java `removeResidentialSkills(player)`: `player.removeSkill(skill, false)` for each (the
    /// effects are cancelled).
    pub fn remove_residential_skills(&mut self, residence: ResidenceRef, player: ObjectId) {
        // A castle first removes the skills of its territory's wards (M7-TW-01).
        if let ResidenceRef::Castle(castle) = residence {
            self.territory_remove_castle_ward_skills(castle.0, player);
        }
        for (skill_id, _) in self.residential_skills(residence) {
            self.remove_skill(player, skill_id, true);
        }
    }

    /// The "Residential skills support" of `EnterWorld` and `RequestAnswerJoinPledge`: the
    /// skills of the clan's castle (`getCastleId() > 0`, `getCastleByOwner`) and fortress.
    /// Returns whether the player got any.
    pub(crate) fn give_clan_residential_skills(&mut self, clan_id: i32, player: ObjectId) -> bool {
        let Some((castle_id, fort_id)) = self.clan(clan_id).map(|c| (c.castle_id, c.fort_id)) else {
            return false;
        };
        let mut added = false;
        if castle_id > 0
            && let Some(castle) = self.castle_by_owner(clan_id)
        {
            added |= self.give_residential_skills(ResidenceRef::Castle(castle), player);
        }
        if fort_id > 0
            && let Some(fort) = self.fort_by_owner(clan_id)
        {
            added |= self.give_residential_skills(ResidenceRef::Fort(fort), player);
        }
        added
    }

    /// `Clan.removeClanMember`'s "Remove Residential skills".
    pub(crate) fn remove_clan_residential_skills(&mut self, clan_id: i32, player: ObjectId) {
        let Some((castle_id, fort_id)) = self.clan(clan_id).map(|c| (c.castle_id, c.fort_id)) else {
            return;
        };
        if castle_id > 0
            && let Some(castle) = self.castle_by_owner(clan_id)
        {
            self.remove_residential_skills(ResidenceRef::Castle(castle), player);
        }
        if fort_id > 0
            && let Some(fort) = self.fort_by_owner(clan_id)
        {
            self.remove_residential_skills(ResidenceRef::Fort(fort), player);
        }
    }
}
