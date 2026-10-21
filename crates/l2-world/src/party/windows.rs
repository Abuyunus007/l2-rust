//! The packets of the party window and the ways to send a packet to a party.
// Java: network/serverpackets/{PartySmallWindowAll,PartySmallWindowAdd,PartySmallWindowDelete,
//       PartySmallWindowUpdate,PartyMemberPosition,PartySpelled}.java
// Java: entity/groups/AbstractPlayerGroup.java (broadcastPacket)
// Java: entity/groups/Party.java (broadcastToPartyMembers)

use l2_net::Encode;
use l2_protocol_game::server::{
    EffectIcon, PartyMemberLocation, PartyMemberPosition, PartyMemberVitals, PartySmallWindowAdd,
    PartySmallWindowAll, PartySmallWindowUpdate, PartySpelled, PartyWindowMember,
};
use l2_util::ObjectId;

use super::PartyId;
use crate::world::World;

impl World {
    /// `AbstractPlayerGroup.broadcastPacket(packet)`: every member gets it.
    pub(crate) fn broadcast_party(&self, party: PartyId, packet: &impl Encode) {
        let bytes = packet.to_bytes();
        for member in self.party_members(party) {
            self.send_bytes(member, bytes.clone());
        }
    }

    /// `Party.broadcastToPartyMembers(player, packet)`: every member but `except`.
    pub(crate) fn broadcast_party_except(&self, party: PartyId, except: ObjectId, packet: &impl Encode) {
        let bytes = packet.to_bytes();
        for member in self.party_members(party) {
            if member != except {
                self.send_bytes(member, bytes.clone());
            }
        }
    }

    /// The numbers the three party window packets write for a member: `(int) getCurrentCp()`,
    /// `getMaxCp()`, ..., `getLevel()`, `getPlayerClass().getId()`.
    pub(crate) fn party_member_vitals(&self, id: ObjectId) -> Option<PartyMemberVitals> {
        let player = self.players.get(&id)?;
        let view = self.stats(id)?;
        Some(PartyMemberVitals {
            object_id: id.0,
            name: player.data.name.clone(),
            cp: player.core.cur_cp as i32,
            max_cp: view.max_cp(),
            hp: player.core.cur_hp as i32,
            max_hp: view.max_hp(),
            mp: player.core.cur_mp as i32,
            max_mp: view.max_mp(),
            level: player.core.level,
            class_id: player.data.class_id,
        })
    }

    /// `new PartySmallWindowAll(exclude, party)`.
    pub(crate) fn party_small_window_all(
        &self,
        exclude: ObjectId,
        party: PartyId,
    ) -> Option<PartySmallWindowAll> {
        let p = self.party.parties.get(&party)?;
        let members = p
            .members
            .iter()
            .filter(|m| **m != exclude)
            .filter_map(|m| {
                Some(PartyWindowMember {
                    vitals: self.party_member_vitals(*m)?,
                    race: self.players.get(m)?.data.race,
                    // M6: the member's summon (SUMMON-01).
                    summon: None,
                })
            })
            .collect();
        Some(PartySmallWindowAll {
            leader_object_id: p.leader().map_or(0, |l| l.0),
            distribution_type: p.distribution_type,
            members,
        })
    }

    /// `new PartySmallWindowAdd(member, party)`.
    pub(crate) fn party_small_window_add(
        &self,
        member: ObjectId,
        party: PartyId,
    ) -> Option<PartySmallWindowAdd> {
        let p = self.party.parties.get(&party)?;
        Some(PartySmallWindowAdd {
            leader_object_id: p.leader().map_or(0, |l| l.0),
            distribution_type: p.distribution_type,
            member: self.party_member_vitals(member)?,
        })
    }

    /// Sends `PartySmallWindowUpdate(member)` to the other members of `member`'s party
    /// (`party.broadcastToPartyMembers(this, new PartySmallWindowUpdate(this))`).
    pub(crate) fn send_party_small_window_update(&self, member: ObjectId) {
        let Some(party) = self.party_of(member) else {
            return;
        };
        let Some(vitals) = self.party_member_vitals(member) else {
            return;
        };
        self.broadcast_party_except(party, member, &PartySmallWindowUpdate { member: vitals });
    }

    /// `new PartyMemberPosition(party)`: where every member stands, in party order.
    pub(crate) fn party_member_position(&self, party: PartyId) -> PartyMemberPosition {
        PartyMemberPosition {
            locations: self
                .party_members(party)
                .into_iter()
                .filter_map(|m| {
                    let (x, y, z) = self.position_of(m)?;
                    Some(PartyMemberLocation {
                        object_id: m.0,
                        x,
                        y,
                        z,
                    })
                })
                .collect(),
        }
    }

    /// `new PartySpelled(owner)` filled the way `EffectList.updateEffectIcons` does: every info in
    /// use, a toggle left out (`isSummon || !skill.isToggle()`; summons are SUMMON-01).
    pub(crate) fn party_spelled(&self, owner: ObjectId) -> PartySpelled {
        let effects = self
            .effect_list(owner)
            .map(|list| {
                list.effects()
                    .filter(|i| i.in_use && !i.facts.healing_potion() && !i.facts.toggle)
                    .map(|i| EffectIcon {
                        display_id: i.facts.display_id,
                        display_level: i.facts.display_level,
                        time: i.time(self.tick),
                        in_use: true,
                    })
                    .collect()
            })
            .unwrap_or_default();
        PartySpelled {
            creature_type: PartySpelled::PLAYER,
            creature_id: owner.0,
            effects,
        }
    }
}
