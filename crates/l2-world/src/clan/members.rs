//! Members coming and going: a player entering and leaving the world, joining and leaving a
//! clan, and the change of the clan leader.
// Java: entity/actor/Player.java (restore: the clan part, setClan, deleteMe: the clan part)
// Java: network/clientpackets/EnterWorld.java (the clan block, notifyClanMembers,
//       notifySponsorOrApprentice, the clan notice, the dismissal message)
// Java: entity/clan/Clan.java (addClanMember, removeClanMember, setNewLeader)
// Java: entity/clan/ClanMember.java (setPlayer, saveApprenticeAndSponsor)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::pledge::{PledgeShowMemberListDeleteAll, PledgeShowMemberListUpdate};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::model::{COMPLETE_PRIVILEGES_MASK, ClanMember, ClanPrivilegesRef, PlayerClanState};
use super::{ClanRecord, ClanWrite, days_ms};
use crate::world::{World, unix_ms_now};

/// What `deleteMe` tells the other members once the player is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClanLeave {
    clan_id: i32,
    update: PledgeShowMemberListUpdate,
}

impl World {
    // ------------------------------------------------------------------ entering the world

    /// Java `Player.restore`, the clan part: the expired join and create penalties are dropped,
    /// the clan is looked up (`ClanTable.getClan`), and the rank, privileges and pledge class
    /// are set.
    ///
    /// Side by side (D-39) the Java game server may have made the clan, or the membership, after
    /// this server loaded its clans. The session read the character's clan from the database at
    /// `CharacterSelect` (`PlayerProfile::clan_record`): an unknown clan is added to the table and
    /// a member the table misses is added to the clan, then Java's restore goes on. When the clan
    /// cannot be found even so (the read failed, the row is gone), the player is clanless here but
    /// the row's clan columns and title stay as they are (`PlayerClanState::keep_stored`) instead
    /// of Java's `setClan(null)`, which would write `clanid = 0`.
    pub(crate) fn clan_restore_player(&mut self, id: ObjectId) {
        let now = unix_ms_now();
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let record = player.data.profile.clan_record.take();
        let state = player.data.profile.clan.get_or_insert_with(Default::default);
        if state.clan_join_expiry_time < now {
            state.clan_join_expiry_time = 0;
        }
        if state.clan_create_expiry_time < now {
            state.clan_create_expiry_time = 0;
        }
        let clan_id = state.clan_id;
        let mut found = false;
        if clan_id > 0 {
            if let Some(record) = record.filter(|r| r.clan_id == clan_id) {
                self.install_clan_of_record(id, *record, now);
            }
            found = self.clan(clan_id).is_some();
            if !self.clan(clan_id).is_some_and(|c| c.is_member(id.0)) {
                warn!(
                    "Clan: character {} is in clan {clan_id} by its row, but the clan or the membership is not found (not in the database either); the row's clan columns are kept.",
                    id.0
                );
                found = false;
                if let Some(s) = self.player_clan_state_mut(id) {
                    *s = PlayerClanState {
                        clan_join_expiry_time: s.clan_join_expiry_time,
                        clan_create_expiry_time: s.clan_create_expiry_time,
                        wants_peace: s.wants_peace,
                        keep_stored: true,
                        ..PlayerClanState::default()
                    };
                }
            }
            if self.clan(clan_id).is_some_and(|c| c.is_member(id.0)) {
                let leader = self.clan(clan_id).is_some_and(|c| c.leader == Some(id));
                let has = |grade| self.clan(clan_id).is_some_and(|c| c.has_rank(grade));
                let state = self.player_clan_state(id);
                let mut new_state = state;
                if !leader {
                    if new_state.power_grade == 0 {
                        new_state.power_grade = 5;
                    }
                    // `setClanPrivileges(clan.getRankPrivs(grade))`: the rank's own object, or a
                    // new empty one for a rank the clan does not have.
                    new_state.privileges = if has(new_state.power_grade) {
                        ClanPrivilegesRef::Rank(new_state.power_grade)
                    } else {
                        ClanPrivilegesRef::Own(0)
                    };
                } else {
                    // `getClanPrivileges().enableAll()` on the player's own new object.
                    new_state.privileges = ClanPrivilegesRef::Own(COMPLETE_PRIVILEGES_MASK);
                    new_state.power_grade = 1;
                }
                if let Some(s) = self.player_clan_state_mut(id) {
                    *s = new_state;
                }
                self.set_pledge_class(id);
            }
        }
        // Without a clan here (also a kept row, see above): the noble's class 5.
        if !found {
            let noble = self.players.get(&id).is_some_and(|p| p.data.profile.noble);
            if let Some(s) = self.player_clan_state_mut(id) {
                if noble {
                    s.pledge_class = 5;
                }
                // `isHero()`: 8, the heroes are not loaded (M8).
                s.privileges = ClanPrivilegesRef::Own(0);
            }
        }
    }

    /// The clan the session read for a character: a clan the table does not know is restored
    /// from it (`new Clan(clanId)`, its dissolution scheduled like at startup); a member of a known
    /// clan that the table misses is added from its row.
    // NOTE: the clan wars of a clan added this way are not read (`restoreClanWars` runs at
    // startup); they come with the next start.
    fn install_clan_of_record(&mut self, id: ObjectId, record: ClanRecord, now: i64) {
        let clan_id = record.clan_id;
        if self.clan(clan_id).is_none() {
            if record.data.is_none() {
                return;
            }
            let clan = self.restore_clan(record, now);
            let dissolving = clan.dissolving_expiry_time;
            info!(
                "ClanTable: Restored clan {}[{clan_id}] of character {} from the database.",
                clan.name, id.0
            );
            self.clans.clans.insert(clan_id, clan);
            if dissolving != 0 {
                self.schedule_remove_clan(clan_id);
            }
            return;
        }
        if self.clan(clan_id).is_some_and(|c| c.is_member(id.0)) {
            return;
        }
        let Some(row) = record.members.into_iter().find(|m| m.object_id == id.0) else {
            return;
        };
        info!(
            "Clan: character {} joined clan {clan_id} on another server; added from the database.",
            id.0
        );
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.put_member(ClanMember {
                object_id: id,
                name: row.name,
                title: row.title,
                power_grade: row.power_grade,
                level: row.level,
                class_id: row.class_id,
                female: row.female,
                race: row.race,
                pledge_type: row.pledge_type,
                apprentice: row.apprentice,
                sponsor: row.sponsor,
                has_player: false,
            });
        }
    }

    /// Java `Player.setClan(null)`: the clan columns go back to zero and the title is cleared.
    pub(crate) fn leave_clan_state(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.data.title.clear();
        let state = player.data.profile.clan.get_or_insert_with(Default::default);
        *state = PlayerClanState {
            clan_join_expiry_time: state.clan_join_expiry_time,
            clan_create_expiry_time: state.clan_create_expiry_time,
            wants_peace: state.wants_peace,
            pledge_class: state.pledge_class,
            ..PlayerClanState::default()
        };
        // `_activeWarehouse = null`.
        self.clear_active_warehouse(id);
    }

    /// Java `EnterWorld`'s clan block, after the first `UserInfo`: the clan skills, the other
    /// members and the apprentice or sponsor are told, the member list, `PledgeStatusChanged`.
    pub(crate) fn clan_on_enter_world(&mut self, id: ObjectId) {
        let clan_id = self.clan_id_of(id);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let skills = self.pledge_skill_list(clan);
        self.send(id, &skills);
        self.notify_clan_members(clan_id, id);
        self.notify_sponsor_or_apprentice(id);
        // The clan hall payment (M7-RES-01): an auctionable hall of the clan that is not paid.
        if let Some(hall) = self
            .auctionable_hall_by_owner(clan_id)
            .and_then(|hall| self.clan_hall(hall))
            .filter(|hall| !hall.paid())
        {
            let message = SystemMessage::new(
                SystemMessageId::PaymentForYourClanHallHasNotBeenMadePleaseMakePaymentToYourClanWarehouseByS1Tomorrow,
            )
            .with(SystemMessageParam::IntNumber(hall.lease()));
            self.send(id, &message);
        }
        // The siege state of a clan in a castle siege (M7-SIEGE-01); fortress sieges, siegable
        // halls and Territory War: M7-FORT-01, M7-CHSIEGE-01, M7-TW-01.
        self.siege_on_enter_world(id, clan_id);
        self.send_pledge_show_member_list_all(clan_id, id);
        if let Some(clan) = self.clan(clan_id) {
            let status = Self::pledge_status_changed(clan);
            self.send(id, &status);
        }
        // Residential skills support (M7-RES-01). The `SkillList` of the end of `EnterWorld` is
        // sent from the saved skills, so the client gets these with a second one.
        if self.give_clan_residential_skills(clan_id, id) {
            self.send_skill_list(id);
        }
    }

    /// Java `EnterWorld.notifyClanMembers`.
    fn notify_clan_members(&mut self, clan_id: i32, id: ObjectId) {
        // `clan.getClanMember(objectId).setPlayer(player)`
        let leader = self.clan(clan_id).is_some_and(|c| c.leader == Some(id));
        if let Some(clan) = self.clan_mut(clan_id) {
            if let Some(member) = clan.member_mut(id) {
                member.has_player = true;
            }
            if leader {
                // `_clan.setLeader(this)`
                clan.leader = Some(id);
            }
        }
        // `setPlayer` -> `_clan.addSkillEffects(player)`, and the siege skills of a leader of a
        // clan above level 3 (M7-SIEGE-01). The `SkillList` of the end of `EnterWorld` is sent
        // from the player's saved skills only, so the client gets the skills that joined with a
        // second one.
        let mut added = self.clan_add_skill_effects(clan_id, id);
        if self.clan(clan_id).is_some_and(|c| c.level > 3) && self.is_clan_leader(id) {
            self.add_siege_skills(id);
            added = true;
        }
        if added {
            self.send_skill_list(id);
        }
        let name = self
            .players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        self.broadcast_to_other_clan_members(
            clan_id,
            &SystemMessage::new(SystemMessageId::ClanMemberS1HasLoggedIntoGame)
                .with(SystemMessageParam::Text(name)),
            id,
        );
        if let Some(update) = self.pledge_member_update_of_player(id) {
            self.broadcast_to_other_clan_members(clan_id, &update, id);
        }
    }

    /// Java `EnterWorld.notifySponsorOrApprentice`.
    fn notify_sponsor_or_apprentice(&self, id: ObjectId) {
        let state = self.player_clan_state(id);
        let name = self
            .players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if state.sponsor != 0 {
            let sponsor = ObjectId(state.sponsor);
            if self.players.contains_key(&sponsor) {
                self.send(
                    sponsor,
                    &SystemMessage::new(SystemMessageId::YourApprenticeS1HasLoggedIn)
                        .with(SystemMessageParam::Text(name)),
                );
            }
        } else if state.apprentice != 0 {
            let apprentice = ObjectId(state.apprentice);
            if self.players.contains_key(&apprentice) {
                self.send(
                    apprentice,
                    &SystemMessage::new(SystemMessageId::YourSponsorC1HasLoggedIn)
                        .with(SystemMessageParam::Text(name)),
                );
            }
        }
    }

    /// Java `EnterWorld`: `if (showClanNotice)` the clan notice page (`data/html/clanNotice.htm`,
    /// validation off). `None` when the clan has no notice switched on.
    pub(crate) fn clan_notice_page(&self, id: ObjectId) -> Option<NpcHtmlMessage> {
        let clan = self.clan_of(id)?;
        if !clan.notice_enabled {
            return None;
        }
        let mut page = self.html_from_file(id, 0, "data/html/clanNotice.htm")?;
        page.html = page
            .html
            .replace("%clan_name%", &clan.name)
            .replace("%notice_text%", &replace_line_ends(&clan.notice_text()));
        Some(page)
    }

    /// Java `EnterWorld`: "You have recently been dismissed from a clan..." while the join
    /// penalty lasts.
    pub(crate) fn clan_join_penalty_message(&self, id: ObjectId) {
        if self.player_clan_state(id).clan_join_expiry_time > unix_ms_now() {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouHaveRecentlyBeenDismissedFromAClanYouAreNotAllowedToJoinAnotherClanFor24Hours,
                ),
            );
        }
    }

    // ------------------------------------------------------------------ leaving the world

    /// Java `Player.deleteMe`, the clan part before the player goes: `clanMember.setPlayer(null)`
    /// keeps the player's values in the member, and the update the others get is made (the
    /// player is offline by then: `0, 0`).
    pub(crate) fn clan_before_leave(&mut self, id: ObjectId) -> Option<ClanLeave> {
        let clan_id = self.clan_id_of(id);
        self.clan(clan_id)?;
        let player = self.players.get(&id)?;
        let state = player.data.profile.clan.unwrap_or_default();
        let (name, level, class_id, title) = (
            player.data.name.clone(),
            player.core.level,
            player.data.class_id,
            player.data.title.clone(),
        );
        let (female, race) = (player.data.sex != 0, player.data.race);
        if let Some(member) = self.clan_mut(clan_id).and_then(|c| c.member_mut(id))
            && member.has_player
        {
            // "This is here to keep the data when the player logs off."
            member.name = name.clone();
            member.level = level;
            member.class_id = class_id;
            member.power_grade = state.power_grade;
            member.pledge_type = state.pledge_type;
            member.title = title;
            member.apprentice = state.apprentice;
            member.sponsor = state.sponsor;
            member.female = female;
            member.race = race;
            member.has_player = false;
        }
        Some(ClanLeave {
            clan_id,
            update: PledgeShowMemberListUpdate {
                name,
                level,
                class_id,
                female,
                race,
                online: None,
                has_sponsor: state.pledge_type == super::model::SUBUNIT_ACADEMY && state.sponsor != 0,
            },
        })
    }

    /// Java `Player.deleteMe`: `_clan.broadcastToOtherOnlineMembers(new
    /// PledgeShowMemberListUpdate(this), this)` once the player is out of the world.
    pub(crate) fn clan_after_leave(&mut self, id: ObjectId, leave: Option<ClanLeave>) {
        if let Some(leave) = leave {
            self.broadcast_to_other_clan_members(leave.clan_id, &leave.update, id);
        }
    }

    // ------------------------------------------------------------------ joining, leaving

    /// Java `Clan.addClanMember(player)`.
    pub(crate) fn add_clan_member(&mut self, clan_id: i32, player: ObjectId) {
        let Some(member) = self.member_of_player(player) else {
            return;
        };
        let Some(clan) = self.clan_mut(clan_id) else {
            return;
        };
        clan.put_member(member);
        // `member.setPlayer(player)` -> `_clan.addSkillEffects(player)`: the player is not in the
        // clan yet (its pledge class is the old one).
        self.clan_add_skill_effects(clan_id, player);
        // `player.setClan(this)`
        if let Some(s) = self.player_clan_state_mut(player) {
            s.clan_id = clan_id;
            s.keep_stored = false;
        }
        self.set_pledge_class(player);
        if let Some(update) = self.pledge_member_update_of_player(player) {
            self.send(player, &update);
        }
        if let Some(clan) = self.clan(clan_id) {
            let skills = self.pledge_skill_list(clan);
            self.send(player, &skills);
        }
        self.clan_add_skill_effects(clan_id, player);
    }

    /// Java `Clan.removeClanMember(objectId, clanJoinExpiryTime)`. `write_db` false leaves the
    /// rows alone (a scheduled dissolution side by side, D-49).
    pub(crate) fn remove_clan_member(
        &mut self,
        clan_id: i32,
        object_id: ObjectId,
        clan_join_expiry_time: i64,
        write_db: bool,
    ) {
        let Some(ex_member) = self.clan(clan_id).and_then(|c| c.member(object_id).cloned()) else {
            warn!(
                "Member Object ID: {} not found in clan while trying to remove",
                object_id.0
            );
            return;
        };
        // The answers of the member while its player is still linked (`getApprentice()`, ...).
        let ex_info = self.member_info(&ex_member);
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.take_member(object_id);
        }

        let leads_sub_pledge = self.clan(clan_id).map_or(0, |c| c.leader_sub_pledge(object_id.0));
        if leads_sub_pledge != 0 {
            // Sub-unit leader withdraws, position becomes vacant and leader should appoint new via
            // NPC.
            if let Some(sub) = self
                .clan_mut(clan_id)
                .and_then(|c| c.sub_pledge_mut(leads_sub_pledge))
            {
                sub.leader_id = 0;
            }
            if write_db {
                self.update_sub_pledge_in_db(clan_id, leads_sub_pledge);
            }
        }

        if ex_info.apprentice != 0 {
            self.clear_apprentice_or_sponsor(clan_id, ObjectId(ex_info.apprentice), true, write_db);
        }
        if ex_info.sponsor != 0 {
            self.clear_apprentice_or_sponsor(clan_id, ObjectId(ex_info.sponsor), false, write_db);
        }
        if write_db {
            self.clan_write(ClanWrite::CharApprenticeSponsor {
                char_id: object_id.0,
                apprentice: 0,
                sponsor: 0,
            });
        }
        // `REMOVE_CASTLE_CIRCLETS` (`CastleManager.removeCirclet`, M7-RES-01). An offline member's
        // rows follow the rest of the leave (`write_db`).
        if self.residences.settings.remove_castle_circlets {
            let castle_id = self.clan(clan_id).map_or(0, |c| c.castle_id);
            let online = ex_member.has_player && self.players.contains_key(&object_id);
            if online || write_db {
                self.remove_member_circlet_of_ex_member(object_id, castle_id, online);
            }
        }

        let online = ex_member.has_player && self.players.contains_key(&object_id);
        if online {
            let leader = self.clan(clan_id).is_some_and(|c| c.leader == Some(object_id));
            // `if (!player.isNoble()) player.setTitle("")`, then `setApprentice(0)`,
            // `setSponsor(0)`; `setClan(null)` below clears all of them again.
            if let Some(s) = self.player_clan_state_mut(object_id) {
                s.apprentice = 0;
                s.sponsor = 0;
            }
            if leader {
                self.remove_siege_skills(object_id);
                let create_days = self.clans.settings.alt_clan_create_days;
                if let Some(s) = self.player_clan_state_mut(object_id) {
                    s.clan_create_expiry_time = unix_ms_now().saturating_add(days_ms(create_days));
                }
            }
            // Remove Clan skills from Player.
            self.clan_remove_skill_effects(clan_id, object_id);
            // Remove Residential skills (M7-RES-01).
            self.remove_clan_residential_skills(clan_id, object_id);
            self.send_skill_list(object_id);
            self.leave_clan_state(object_id);
            // NOTE(java-quirk): "Players leaving from clan academy have no penalty" reads
            // `exMember.getPledgeType()`, which asks the player, whose type `setClan(null)` has
            // just set to 0: an online academy member gets the penalty too.
            if let Some(s) = self.player_clan_state_mut(object_id) {
                s.clan_join_expiry_time = clan_join_expiry_time;
            }
            self.set_pledge_class(object_id);
            self.broadcast_user_info(object_id);
            // Disable clan tab.
            self.send(object_id, &PledgeShowMemberListDeleteAll);
        } else if write_db {
            let leader_id = self.clan(clan_id).map_or(0, |c| c.leader_id());
            let create_expiry = if leader_id == object_id.0 {
                unix_ms_now().saturating_add(days_ms(self.clans.settings.alt_clan_create_days))
            } else {
                0
            };
            self.clan_write(ClanWrite::RemoveMember {
                char_id: object_id.0,
                clan_join_expiry_time,
                clan_create_expiry_time: create_expiry,
            });
        }
    }

    /// The apprentice (or sponsor) of a member that leaves loses its link: the player's value
    /// when online, the member's values otherwise, and the row (`saveApprenticeAndSponsor(0, 0)`).
    fn clear_apprentice_or_sponsor(
        &mut self,
        clan_id: i32,
        other: ObjectId,
        is_apprentice: bool,
        write_db: bool,
    ) {
        let Some(member) = self.clan(clan_id).and_then(|c| c.member(other).cloned()) else {
            return;
        };
        if member.has_player && self.players.contains_key(&other) {
            if let Some(s) = self.player_clan_state_mut(other) {
                if is_apprentice {
                    s.sponsor = 0;
                } else {
                    s.apprentice = 0;
                }
            }
        } else if let Some(m) = self.clan_mut(clan_id).and_then(|c| c.member_mut(other)) {
            m.apprentice = 0;
            m.sponsor = 0;
        }
        if write_db {
            self.clan_write(ClanWrite::CharApprenticeSponsor {
                char_id: other.0,
                apprentice: 0,
                sponsor: 0,
            });
        }
    }

    /// Java `Clan.updateSubPledgeInDB(pledgeType)`.
    pub(crate) fn update_sub_pledge_in_db(&mut self, clan_id: i32, pledge_type: i32) {
        let Some(sub) = self.clan(clan_id).and_then(|c| c.sub_pledge(pledge_type)) else {
            return;
        };
        let write = ClanWrite::UpdateSubPledge {
            clan_id,
            sub_pledge_id: pledge_type,
            name: sub.name.clone(),
            leader_id: sub.leader_id,
        };
        self.clan_write(write);
    }

    // ------------------------------------------------------------------ the leader

    /// Java `Clan.setNewLeader(member)`.
    pub(crate) fn set_new_leader(&mut self, clan_id: i32, member_id: ObjectId) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let Some(ex_member) = clan.leader_member().cloned() else {
            // `exMember.getPlayer()` on a null leader throws.
            return;
        };
        let Some(member) = clan.member(member_id).cloned() else {
            return;
        };
        let new_leader_online = member.has_player && self.players.contains_key(&member_id);
        let ex_leader = ex_member.object_id;
        let ex_leader_online = ex_member.has_player && self.players.contains_key(&ex_leader);
        // Scripts: `OnPlayerClanLeaderChange` (D-05).

        if ex_leader_online {
            // The old leader's wyvern lands and loses the siege skills (M7-SIEGE-01).
            if self.is_flying(ex_leader) {
                self.dismount(ex_leader);
            }
            if self
                .clan(clan_id)
                .is_some_and(|c| c.level >= self.siege_settings().siege_clan_min_level)
            {
                self.remove_siege_skills(ex_leader);
            }
            self.set_player_privileges_mask(ex_leader, 0);
            self.broadcast_user_info(ex_leader);
        } else {
            self.clan_write(ClanWrite::CharClanPrivs {
                char_id: ex_leader.0,
                privs: 0,
            });
        }

        // `setLeader(member)`
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.leader = Some(member_id);
            if clan.new_leader_id != 0 {
                // `setNewLeaderId(0, true)`
                clan.new_leader_id = 0;
                self.update_clan_in_db(clan_id);
            }
        }
        self.update_clan_in_db(clan_id);

        if ex_leader_online {
            self.set_pledge_class(ex_leader);
            self.broadcast_user_info(ex_leader);
            let mut changed = Vec::new();
            self.check_item_restriction(ex_leader, &mut changed);
        }

        if new_leader_online {
            self.set_pledge_class(member_id);
            self.set_player_privileges_mask(member_id, COMPLETE_PRIVILEGES_MASK);
            // The siege skills of a clan of the siege level (M7-SIEGE-01).
            if self
                .clan(clan_id)
                .is_some_and(|c| c.level >= self.siege_settings().siege_clan_min_level)
            {
                self.add_siege_skills(member_id);
            }
            self.broadcast_user_info(member_id);
        } else {
            self.clan_write(ClanWrite::CharClanPrivs {
                char_id: member_id.0,
                privs: COMPLETE_PRIVILEGES_MASK,
            });
        }

        self.broadcast_clan_status(clan_id);
        let new_name = self.member_info(&member).name;
        self.broadcast_to_clan(
            clan_id,
            &SystemMessage::new(SystemMessageId::ClanLordPrivilegesHaveBeenTransferredToC1)
                .with(SystemMessageParam::Text(new_name.clone())),
        );
        info!(
            "Leader of Clan: {} changed to: {new_name} ex leader: {}",
            self.clan(clan_id).map(|c| c.name.clone()).unwrap_or_default(),
            self.member_info(&ex_member).name
        );
    }
}

/// `replaceAll("(\r\n|\n)", "<br>")`.
fn replace_line_ends(text: &str) -> String {
    text.replace("\r\n", "<br>").replace('\n', "<br>")
}
