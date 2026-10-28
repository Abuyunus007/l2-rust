//! The clan window packets and the clan broadcasts.
// Java: network/serverpackets/{PledgeShowMemberListAll,PledgeShowMemberListUpdate,
//       PledgeShowMemberListAdd,PledgeShowInfoUpdate,PledgeStatusChanged,PledgeInfo,
//       PledgeSkillList,PledgeReceiveSubPledgeCreated,PledgeReceiveMemberInfo,
//       PledgeReceivePowerInfo,PledgePowerGradeList}.java
// Java: entity/clan/Clan.java (broadcastToOnlineMembers, broadcastToOtherOnlineMembers,
//       broadcastClanStatus)
//
// Java builds most of these packets from the `Clan` when they are written; here they are built
// when sent, which is the same state (the world is one thread).

use l2_net::Encode;
use l2_protocol_game::server::pledge::{
    PledgeInfo, PledgeMember, PledgeMemberOnline, PledgePowerGradeList, PledgeReceiveMemberInfo,
    PledgeReceivePowerInfo, PledgeReceiveSubPledgeCreated, PledgeShowInfoUpdate, PledgeShowMemberListAdd,
    PledgeShowMemberListAll, PledgeShowMemberListDeleteAll, PledgeShowMemberListUpdate, PledgeSkillEntry,
    PledgeSkillList, PledgeStatusChanged, RankPrivileges, SubPledgeSkill,
};
use l2_util::ObjectId;
use tracing::warn;

use super::MemberInfo;
use super::model::{Clan, SUBUNIT_ACADEMY};
use crate::world::World;

impl World {
    /// Java `new PledgeShowInfoUpdate(clan)`.
    pub(crate) fn pledge_show_info_update(&self, clan: &Clan) -> PledgeShowInfoUpdate {
        PledgeShowInfoUpdate {
            clan_id: clan.id,
            crest_id: clan.crest_id,
            level: clan.level,
            castle_id: clan.castle_id,
            hideout_id: clan.hideout_id,
            fort_id: clan.fort_id,
            rank: clan.rank,
            reputation_score: clan.reputation_score,
            ally_id: clan.ally_id,
            // `writeString(null)` writes only the terminator, like an empty string.
            ally_name: clan.ally_name.clone().unwrap_or_default(),
            ally_crest_id: clan.ally_crest_id,
            at_war: clan.is_at_war(),
        }
    }

    /// Java `new PledgeStatusChanged(clan)`.
    pub(crate) fn pledge_status_changed(clan: &Clan) -> PledgeStatusChanged {
        PledgeStatusChanged {
            leader_id: clan.leader_id(),
            clan_id: clan.id,
            crest_id: clan.crest_id,
            ally_id: clan.ally_id,
            ally_crest_id: clan.ally_crest_id,
        }
    }

    /// Java `new PledgeInfo(clan)`.
    pub(crate) fn pledge_info(clan: &Clan) -> PledgeInfo {
        PledgeInfo {
            clan_id: clan.id,
            clan_name: clan.name.clone(),
            ally_name: clan.ally_name.clone().unwrap_or_default(),
        }
    }

    /// Java `new PledgeSkillList(clan)`: the clan skills by display id and level, then the squad
    /// skills.
    pub(crate) fn pledge_skill_list(&self, clan: &Clan) -> PledgeSkillList {
        let table = self.skill_table();
        let skills = clan
            .all_skills()
            .into_iter()
            .map(|(id, level)| {
                let template = table.as_deref().and_then(|t| t.get(id, level));
                PledgeSkillEntry {
                    display_id: template.map_or(id, |t| t.display_id),
                    display_level: template.map_or(level, |t| t.display_level),
                }
            })
            .collect();
        let sub_skills = clan
            .all_sub_skills()
            .into_iter()
            .map(|(sub_type, skill_id, skill_level)| SubPledgeSkill {
                sub_type,
                skill_id,
                skill_level,
            })
            .collect();
        PledgeSkillList { skills, sub_skills }
    }

    /// Java `new PledgeShowMemberListUpdate(player)`.
    pub(crate) fn pledge_member_update_of_player(&self, id: ObjectId) -> Option<PledgeShowMemberListUpdate> {
        let player = self.players.get(&id)?;
        let state = player.data.profile.clan.unwrap_or_default();
        Some(PledgeShowMemberListUpdate {
            name: player.data.name.clone(),
            level: player.core.level,
            class_id: player.data.class_id,
            female: player.data.sex != 0,
            race: player.data.race,
            // `player.isOnline()`: true while it is in the world.
            online: Some(PledgeMemberOnline {
                object_id: id.0,
                pledge_type: state.pledge_type,
            }),
            has_sponsor: state.pledge_type == SUBUNIT_ACADEMY && state.sponsor != 0,
        })
    }

    /// Java `new PledgeShowMemberListAdd(player)`.
    pub(crate) fn pledge_member_add_of_player(&self, id: ObjectId) -> Option<PledgeShowMemberListAdd> {
        let player = self.players.get(&id)?;
        let state = player.data.profile.clan.unwrap_or_default();
        Some(PledgeShowMemberListAdd {
            name: player.data.name.clone(),
            level: player.core.level,
            class_id: player.data.class_id,
            online_object_id: id.0,
            pledge_type: state.pledge_type,
        })
    }

    /// Java `new PledgeShowMemberListAdd(member)`.
    pub(crate) fn pledge_member_add_of_member(info: &MemberInfo) -> PledgeShowMemberListAdd {
        PledgeShowMemberListAdd {
            name: info.name.clone(),
            level: info.level,
            class_id: info.class_id,
            online_object_id: if info.online { info.object_id.0 } else { 0 },
            pledge_type: info.pledge_type,
        }
    }

    /// Java `new PledgeReceiveSubPledgeCreated(subPledge, clan)` of the sub pledge `sub_id`.
    pub(crate) fn pledge_sub_pledge_created(
        &self,
        clan_id: i32,
        sub_id: i32,
    ) -> Option<PledgeReceiveSubPledgeCreated> {
        let clan = self.clan(clan_id)?;
        let sub = clan.sub_pledge(sub_id)?;
        let leader_name = if sub.id == SUBUNIT_ACADEMY || sub.leader_id == 0 {
            String::new()
        } else {
            match self.clan_member_info(clan_id, ObjectId(sub.leader_id)) {
                Some(leader) => leader.name,
                None => {
                    warn!(
                        "SubPledgeLeader: {} is missing from clan: {}[{}]",
                        sub.leader_id, clan.name, clan.id
                    );
                    String::new()
                }
            }
        };
        Some(PledgeReceiveSubPledgeCreated {
            id: sub.id,
            name: sub.name.clone(),
            leader_name,
        })
    }

    /// Java `new PledgeReceiveMemberInfo(member)`. `None` for a member of a sub pledge the clan
    /// does not have: Java's `writeImpl` throws on it and the packet is not sent.
    pub(crate) fn pledge_receive_member_info(
        &self,
        clan_id: i32,
        info: &MemberInfo,
    ) -> Option<PledgeReceiveMemberInfo> {
        let clan = self.clan(clan_id)?;
        // Clan or subpledge name.
        let pledge_name = if info.pledge_type != 0 {
            clan.sub_pledge(info.pledge_type)?.name.clone()
        } else {
            clan.name.clone()
        };
        Some(PledgeReceiveMemberInfo {
            pledge_type: info.pledge_type,
            name: info.name.clone(),
            title: info.title.clone(),
            power_grade: info.power_grade,
            pledge_name,
            apprentice_or_sponsor_name: self.apprentice_or_sponsor_name(clan_id, info),
        })
    }

    /// Java `ClanMember.getApprenticeOrSponsorName()`.
    fn apprentice_or_sponsor_name(&self, clan_id: i32, info: &MemberInfo) -> String {
        let name_of = |id: i32| {
            self.clan_member_info(clan_id, ObjectId(id))
                .map_or_else(|| "Error".to_owned(), |m| m.name)
        };
        if info.apprentice != 0 {
            return name_of(info.apprentice);
        }
        if info.sponsor != 0 {
            return name_of(info.sponsor);
        }
        String::new()
    }

    /// Java `new PledgeReceivePowerInfo(member)`.
    pub(crate) fn pledge_receive_power_info(
        &self,
        clan_id: i32,
        info: &MemberInfo,
    ) -> PledgeReceivePowerInfo {
        PledgeReceivePowerInfo {
            power_grade: info.power_grade,
            name: info.name.clone(),
            privileges: self.clan(clan_id).map_or(0, |c| c.rank_privs(info.power_grade)),
        }
    }

    /// Java `new PledgePowerGradeList(clan.getAllRankPrivs())`.
    pub(crate) fn pledge_power_grade_list(clan: &Clan) -> PledgePowerGradeList {
        PledgePowerGradeList {
            privs: clan
                .all_rank_privs()
                .into_iter()
                .map(|p| RankPrivileges {
                    rank: p.rank,
                    // NOTE(java-quirk): Java writes `getParty()` (always 0), not the privileges.
                    privileges: p.party,
                })
                .collect(),
        }
    }

    /// Java `player.sendPacket(new PledgeShowMemberListAll(clan, player))`: the main clan with
    /// its members, then (from `writeImpl`) every sub pledge, every member of a sub pledge and
    /// the player's `UserInfo`.
    ///
    /// NOTE: Java queues the follow-up packets while it writes the first one, so on the wire they
    /// come after whatever was queued before the write; here they follow at once.
    pub(crate) fn send_pledge_show_member_list_all(&mut self, clan_id: i32, to: ObjectId) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let members = self.clan_members_info(clan_id);
        let main = PledgeShowMemberListAll {
            main_or_subpledge: 0,
            clan_id: clan.id,
            pledge_type: 0,
            clan_name: clan.name.clone(),
            leader_name: self.clan_leader_name(clan_id),
            crest_id: clan.crest_id,
            level: clan.level,
            castle_id: clan.castle_id,
            hideout_id: clan.hideout_id,
            fort_id: clan.fort_id,
            rank: clan.rank,
            reputation_score: clan.reputation_score,
            ally_id: clan.ally_id,
            ally_name: clan.ally_name.clone().unwrap_or_default(),
            ally_crest_id: clan.ally_crest_id,
            at_war: clan.is_at_war(),
            members: members
                .iter()
                .filter(|m| m.pledge_type == 0)
                .map(|m| {
                    // `m.getPlayer() != null`: the player's sex and race, else 1 and 1 ("no
                    // visible effect").
                    let (sex, race) = if m.online {
                        (i32::from(m.female), m.race)
                    } else {
                        (1, 1)
                    };
                    PledgeMember {
                        name: m.name.clone(),
                        level: m.level,
                        class_id: m.class_id,
                        sex,
                        race,
                        online_object_id: if m.online { m.object_id.0 } else { 0 },
                        has_sponsor: m.sponsor != 0,
                    }
                })
                .collect(),
        };
        let sub_ids: Vec<i32> = clan.all_sub_pledges().iter().map(|s| s.id).collect();
        self.send(to, &main);
        for sub_id in sub_ids {
            if let Some(packet) = self.pledge_sub_pledge_created(clan_id, sub_id) {
                self.send(to, &packet);
            }
        }
        for m in members.iter().filter(|m| m.pledge_type != 0) {
            self.send(to, &Self::pledge_member_add_of_member(m));
        }
        // "unless this is sent sometimes, the client doesn't recognise the player as the leader"
        self.send_user_info(to);
    }

    /// Java `Clan.broadcastToOnlineMembers(packet)`.
    pub(crate) fn broadcast_to_clan(&self, clan_id: i32, packet: &impl Encode) {
        let bytes = packet.to_bytes();
        for id in self.clan_online_members(clan_id, 0) {
            self.send_bytes(id, bytes.clone());
        }
    }

    /// Java `Clan.broadcastToOtherOnlineMembers(packet, player)`.
    pub(crate) fn broadcast_to_other_clan_members(
        &self,
        clan_id: i32,
        packet: &impl Encode,
        except: ObjectId,
    ) {
        let bytes = packet.to_bytes();
        for id in self.clan_online_members(clan_id, except.0) {
            self.send_bytes(id, bytes.clone());
        }
    }

    /// Java `Clan.broadcastClanStatus()`: every online member gets the member list again.
    pub fn broadcast_clan_status(&mut self, clan_id: i32) {
        for id in self.clan_online_members(clan_id, 0) {
            self.send(id, &PledgeShowMemberListDeleteAll);
            self.send_pledge_show_member_list_all(clan_id, id);
        }
    }

    /// Java `broadcastToOnlineMembers(new PledgeShowInfoUpdate(clan))`.
    pub(crate) fn broadcast_clan_info_update(&self, clan_id: i32) {
        if let Some(clan) = self.clan(clan_id) {
            let packet = self.pledge_show_info_update(clan);
            self.broadcast_to_clan(clan_id, &packet);
        }
    }
}
