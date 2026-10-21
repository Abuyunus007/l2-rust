//! The clan commands of the village masters (`npc_<id>_create_clan ...`).
// Java: entity/actor/instance/VillageMaster.java (onBypassFeedback: the clan commands,
//       dissolveClan, recoverClan, createSubPledge, renameSubPledge, assignSubPledgeLeader,
//       isValidName)
// Java: entity/actor/Player.java (calculateDeathExpPenalty, for a dissolution)
//
// The pages that list these commands come from the `ClanMaster` script (scripts come last,
// D-05). The subclass commands of the same NPCs are not ported; they reach the generic NPC
// handler like every other unknown command.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    MagicSkillLaunched, MagicSkillUse, NpcHtmlMessage, SkillPosition, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{debug, warn};

use super::java_int_days_ms;
use super::model::{
    SUBUNIT_ACADEMY, SUBUNIT_KNIGHT1, SUBUNIT_ROYAL1, is_alpha_numeric, java_equals_ignore_case, java_len,
};
use crate::world::{World, unix_ms_now};
use crate::zones::ZoneId;

/// The clan level up effect (`MagicSkillUse(player, 5103, 1, 0, 0)`).
const CLAN_LEVEL_UP_SKILL: i32 = 5103;

/// Java `command.split(" ")`: empty parts in the middle stay, trailing ones go.
fn java_split_space(command: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = command.split(' ').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

impl World {
    /// Whether the NPC is a village master (Java `instanceof VillageMaster`).
    pub(crate) fn is_village_master(&self, npc: ObjectId) -> bool {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .is_some_and(|t| t.npc_type.starts_with("VillageMaster"))
    }

    /// Java `VillageMaster.onBypassFeedback(player, command)`: the clan commands; everything
    /// else goes to the NPC's generic handling (`super.onBypassFeedback`).
    pub(crate) fn village_master_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        let parts = java_split_space(command);
        let actual = parts.first().copied().unwrap_or("");
        let params = parts.get(1).copied().unwrap_or("");
        let params2 = parts.get(2).copied().unwrap_or("");
        let is = |name: &str| actual.eq_ignore_ascii_case(name);

        if is("create_clan") {
            if params.is_empty() {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::PleaseCreateYourClanName),
                );
                return;
            }
            if !params2.is_empty() || !self.is_valid_clan_name(params) {
                self.send(player, &SystemMessage::new(SystemMessageId::ClanNameIsInvalid));
                return;
            }
            self.create_clan(player, params);
        } else if is("create_academy") {
            if params.is_empty() {
                return;
            }
            self.vm_create_sub_pledge(player, params, None, SUBUNIT_ACADEMY, 5);
        } else if is("rename_pledge") {
            if params.is_empty() || params2.is_empty() {
                return;
            }
            match params.parse::<i32>() {
                Ok(pledge_type) => self.vm_rename_sub_pledge(player, pledge_type, params2),
                // `Integer.parseInt` throws; the bypass is logged and dropped.
                Err(_) => warn!("{player}: rename_pledge with a bad pledge type '{params}'"),
            }
        } else if is("create_royal") {
            if params.is_empty() {
                return;
            }
            self.vm_create_sub_pledge(player, params, Some(params2), SUBUNIT_ROYAL1, 6);
        } else if is("create_knight") {
            if params.is_empty() {
                return;
            }
            self.vm_create_sub_pledge(player, params, Some(params2), SUBUNIT_KNIGHT1, 7);
        } else if is("assign_subpl_leader") {
            if params.is_empty() {
                return;
            }
            self.vm_assign_sub_pledge_leader(player, params, params2);
        } else if is("create_ally") {
            if params.is_empty() {
                return;
            }
            if self.clan_of(player).is_none() {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::OnlyClanLeadersMayCreateAlliances),
                );
            } else {
                self.create_ally(player, params);
            }
        } else if is("dissolve_ally") {
            // `player.getClan().dissolveAlly(player)`: Java throws without a clan.
            if self.clan_of(player).is_some() {
                self.dissolve_ally(player);
            }
        } else if is("dissolve_clan") {
            self.vm_dissolve_clan(player);
        } else if is("change_clan_leader") {
            if params.is_empty() {
                return;
            }
            self.vm_change_clan_leader(npc, player, params);
        } else if is("cancel_clan_leader_change") {
            self.vm_cancel_clan_leader_change(npc, player);
        } else if is("recover_clan") {
            // `recoverClan`
            if !self.is_clan_leader(player) {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
                );
                return;
            }
            let clan_id = self.clan_id_of(player);
            if let Some(clan) = self.clan_mut(clan_id) {
                clan.dissolving_expiry_time = 0;
            }
            self.update_clan_in_db(clan_id);
        } else if is("increase_clan_level") {
            if self.level_up_clan(player) {
                self.broadcast_clan_level_up_effect(player);
            }
        } else if is("learn_clan_skills") {
            self.show_pledge_skill_list(player);
        } else {
            self.generic_npc_bypass_feedback(npc, player, command);
        }
    }

    /// Java `VillageMaster.isValidName(name)`: the `ClanNameTemplate` pattern.
    fn is_valid_clan_name(&self, name: &str) -> bool {
        (self.clans.settings.clan_name_template)(name)
    }

    /// `player.broadcastSkillPacket(new MagicSkillUse(player, 5103, 1, 0, 0), player)` and the
    /// `MagicSkillLaunched` of it.
    fn broadcast_clan_level_up_effect(&self, player: ObjectId) {
        let Some((x, y, z)) = self.position_of(player) else {
            return;
        };
        let at = SkillPosition { x, y, z };
        let ground = self
            .cast
            .creatures
            .get(&player)
            .and_then(|s| s.current_skill_world_position)
            .map(|(x, y, z)| SkillPosition { x, y, z });
        self.broadcast_creature_packet(
            player,
            &MagicSkillUse {
                creature_id: player.0,
                target_id: player.0,
                skill_id: CLAN_LEVEL_UP_SKILL,
                skill_level: 1,
                hit_time: 0,
                reuse_delay: 0,
                creature: at,
                ground_location: ground,
                target: at,
            },
        );
        self.broadcast_creature_packet(
            player,
            &MagicSkillLaunched {
                creature_id: player.0,
                skill_id: CLAN_LEVEL_UP_SKILL,
                skill_level: 1,
                target_ids: vec![player.0],
            },
        );
    }

    /// Java `VillageMaster.dissolveClan(player, clanId)`.
    fn vm_dissolve_clan(&mut self, player: ObjectId) {
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let message = if clan.ally_id != 0 {
            Some(SystemMessageId::YouCannotDisperseTheClansInYourAlliance)
        } else if clan.is_at_war() {
            Some(SystemMessageId::YouCannotDissolveAClanWhileEngagedInAWar)
        } else if clan.castle_id != 0 || clan.hideout_id != 0 || clan.fort_id != 0 {
            Some(SystemMessageId::YouCannotDissolveAClanWhileOwningAClanHallOrCastle)
        } else if self.is_inside_zone(player, ZoneId::Siege) {
            // The castle and fortress siege registrations come first in Java (M7: none yet).
            Some(SystemMessageId::YouCannotDissolveAClanDuringASiegeOrWhileProtectingACastle)
        } else if clan.dissolving_expiry_time > unix_ms_now() {
            Some(SystemMessageId::YouHaveAlreadyRequestedTheDissolutionOfYourClan)
        } else {
            None
        };
        if let Some(message) = message {
            self.send(player, &SystemMessage::new(message));
            return;
        }
        let days = self.clans.settings.alt_clan_dissolve_days;
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.dissolving_expiry_time = unix_ms_now().wrapping_add(java_int_days_ms(days));
        }
        self.update_clan_in_db(clan_id);
        // "The clan leader should take the XP penalty of a full death."
        self.dissolve_exp_penalty(player);
        self.schedule_remove_clan(clan_id);
    }

    /// Java `Player.calculateDeathExpPenalty(null, false)`: the exp a death costs, without a
    /// killer (no reduction stat) and not at war.
    // NOTE: unlike the death path (`rewards/exp.rs`), Java does not check `PlayerDelevel` or the
    // lucky state here.
    fn dissolve_exp_penalty(&mut self, player: ObjectId) {
        let (Some(max_level), Some(p)) = (self.java_max_level(), self.players.get(&player)) else {
            return;
        };
        let level = p.core.level;
        let karma = p.data.karma;
        let exp = p.data.exp;
        let mut percent_lost = self.rewards.experience_loss.percent_lost(level);
        // Calculate karma loss configuration.
        if karma > 0 {
            percent_lost *= f64::from(self.settings.exp.rate_karma_exp_lost);
        }
        // Get the total exp for the current player level (events: not modelled).
        let level_exp = |l: i32| self.exp_for_level(l).unwrap_or(0);
        let current_level_exp = if level < max_level {
            level_exp(level + 1) - level_exp(level)
        } else {
            level_exp(max_level) - level_exp(max_level - 1)
        };
        // Calculate the experience loss.
        let mut lost_exp = 0;
        if current_level_exp > 0 {
            lost_exp = crate::stats::java_round((current_level_exp as f64 * percent_lost) / 100.0);
            // Capping loss to 10% of current level experience.
            lost_exp = lost_exp.min(current_level_exp / 10);
            // Festival participants: not modelled. No exp loss for Advent Blessing.
            if self.is_advent_blessing_active(player) {
                lost_exp = 0;
            }
        }
        self.set_exp_before_death(player, exp);
        self.remove_exp(player, lost_exp);
    }

    /// Java `VillageMaster.createSubPledge(player, clanName, leaderName, pledgeType, minClanLvl)`.
    fn vm_create_sub_pledge(
        &mut self,
        player: ObjectId,
        name: &str,
        leader_name: Option<&str>,
        pledge_type: i32,
        min_clan_level: i32,
    ) {
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        if clan.level < min_clan_level {
            let message = if pledge_type == SUBUNIT_ACADEMY {
                SystemMessageId::ToEstablishAClanAcademyYourClanMustBeLevel5OrHigher
            } else {
                SystemMessageId::TheConditionsNecessaryToCreateAMilitaryUnitHaveNotBeenMet
            };
            self.send(player, &SystemMessage::new(message));
            return;
        }
        if !is_alpha_numeric(name) || !self.is_valid_clan_name(name) || 2 > java_len(name) {
            self.send(player, &SystemMessage::new(SystemMessageId::ClanNameIsInvalid));
            return;
        }
        if java_len(name) > 16 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ClanNameSLengthIsIncorrect),
            );
            return;
        }
        if self.clans().any(|c| c.sub_pledge_by_name(name).is_some()) {
            if pledge_type == SUBUNIT_ACADEMY {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::S1AlreadyExists)
                        .with(SystemMessageParam::Text(name.to_owned())),
                );
            } else {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::AnotherMilitaryUnitIsAlreadyUsingThatNamePleaseEnterADifferentName,
                    ),
                );
            }
            return;
        }
        let mut leader_id = 0;
        if pledge_type != SUBUNIT_ACADEMY {
            // `clan.getClanMember(leaderName)`: a null name matches nobody.
            let member = leader_name.and_then(|n| self.clan_member_by_name(clan_id, n));
            let ok = member.as_ref().is_some_and(|m| {
                m.pledge_type == 0
                    && self
                        .clan(clan_id)
                        .is_some_and(|c| c.leader_sub_pledge(m.object_id.0) <= 0)
            });
            if !ok {
                self.send_captain_cannot_be_appointed(player, pledge_type);
                return;
            }
            leader_id = member.map_or(0, |m| m.object_id.0);
        }
        if self
            .create_sub_pledge(player, clan_id, pledge_type, leader_id, name)
            .is_none()
        {
            return;
        }
        let clan_name = self.clan(clan_id).map(|c| c.name.clone()).unwrap_or_default();
        let sm = if pledge_type == SUBUNIT_ACADEMY {
            SystemMessage::new(SystemMessageId::CongratulationsTheS1SClanAcademyHasBeenCreated)
                .with(SystemMessageParam::Text(clan_name))
        } else if pledge_type >= SUBUNIT_KNIGHT1 {
            SystemMessage::new(SystemMessageId::TheKnightsOfS1HaveBeenCreated)
                .with(SystemMessageParam::Text(clan_name))
        } else if pledge_type >= SUBUNIT_ROYAL1 {
            SystemMessage::new(SystemMessageId::TheRoyalGuardOfS1HaveBeenCreated)
                .with(SystemMessageParam::Text(clan_name))
        } else {
            SystemMessage::new(SystemMessageId::YourClanHasBeenCreated)
        };
        self.send(player, &sm);
        if pledge_type != SUBUNIT_ACADEMY {
            let leader = leader_name.and_then(|n| self.clan_member_by_name(clan_id, n));
            if let Some(leader) = leader.filter(|m| m.online) {
                self.set_pledge_class(leader.object_id);
                self.send_user_info(leader.object_id);
            }
        }
    }

    /// The captain messages of `createSubPledge` / `assignSubPledgeLeader`.
    fn send_captain_cannot_be_appointed(&self, player: ObjectId, pledge_type: i32) {
        if pledge_type >= SUBUNIT_KNIGHT1 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheCaptainOfTheOrderOfKnightsCannotBeAppointed),
            );
        } else if pledge_type >= SUBUNIT_ROYAL1 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheCaptainOfTheRoyalGuardCannotBeAppointed),
            );
        }
    }

    /// Java `VillageMaster.renameSubPledge(player, pledgeType, pledgeName)`.
    fn vm_rename_sub_pledge(&mut self, player: ObjectId, pledge_type: i32, name: &str) {
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        let clan_id = self.clan_id_of(player);
        if self
            .clan(clan_id)
            .and_then(|c| c.sub_pledge(pledge_type))
            .is_none()
        {
            self.send_message(player, "Pledge don't exists.");
            return;
        }
        if !is_alpha_numeric(name) || !self.is_valid_clan_name(name) || 2 > java_len(name) {
            self.send(player, &SystemMessage::new(SystemMessageId::ClanNameIsInvalid));
            return;
        }
        if java_len(name) > 16 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ClanNameSLengthIsIncorrect),
            );
            return;
        }
        if let Some(sub) = self.clan_mut(clan_id).and_then(|c| c.sub_pledge_mut(pledge_type)) {
            sub.name = name.to_owned();
        }
        self.update_sub_pledge_in_db(clan_id, pledge_type);
        self.broadcast_clan_status(clan_id);
        self.send_message(player, "Pledge name changed.");
    }

    /// Java `VillageMaster.assignSubPledgeLeader(player, clanName, leaderName)`.
    fn vm_assign_sub_pledge_leader(&mut self, player: ObjectId, sub_name: &str, leader_name: &str) {
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        if java_len(leader_name) > 16 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YourTitleCannotExceed16CharactersInLengthPleaseTryAgain),
            );
            return;
        }
        let my_name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if my_name == leader_name {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheCaptainOfTheRoyalGuardCannotBeAppointed),
            );
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(sub_id) = self
            .clan(clan_id)
            .and_then(|c| c.sub_pledge_by_name(sub_name))
            .map(|s| s.id)
            .filter(|id| *id != SUBUNIT_ACADEMY)
        else {
            self.send(player, &SystemMessage::new(SystemMessageId::ClanNameIsInvalid));
            return;
        };
        let member = self.clan_member_by_name(clan_id, leader_name);
        let ok = member.as_ref().is_some_and(|m| {
            m.pledge_type == 0
                && self
                    .clan(clan_id)
                    .is_some_and(|c| c.leader_sub_pledge(m.object_id.0) <= 0)
        });
        let Some(member) = member.filter(|_| ok) else {
            self.send_captain_cannot_be_appointed(player, sub_id);
            return;
        };
        if let Some(sub) = self.clan_mut(clan_id).and_then(|c| c.sub_pledge_mut(sub_id)) {
            sub.leader_id = member.object_id.0;
        }
        self.update_sub_pledge_in_db(clan_id, sub_id);
        if member.online {
            self.set_pledge_class(member.object_id);
            self.send_user_info(member.object_id);
        }
        self.broadcast_clan_status(clan_id);
        self.broadcast_to_clan(
            clan_id,
            &SystemMessage::new(SystemMessageId::C1HasBeenSelectedAsTheCaptainOfS2)
                .with(SystemMessageParam::Text(leader_name.to_owned()))
                .with(SystemMessageParam::Text(sub_name.to_owned())),
        );
    }

    /// The `change_clan_leader` command.
    fn vm_change_clan_leader(&mut self, npc: ObjectId, player: ObjectId, name: &str) {
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        let my_name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if java_equals_ignore_case(&my_name, name) {
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(member) = self.clan_member_by_name(clan_id, name) else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1DoesNotExist)
                    .with(SystemMessageParam::Text(name.to_owned())),
            );
            return;
        };
        if !member.online {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThatPlayerIsNotCurrentlyOnline),
            );
            return;
        }
        // "To avoid clans with null clan leader, academy members shouldn't be eligible for clan
        // leader."
        if self.is_academy_member(member.object_id) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThatPrivilegeCannotBeGrantedToAClanAcademyMember),
            );
            return;
        }
        if self.clans.settings.alt_clan_leader_instant_activation {
            self.set_new_leader(clan_id, member.object_id);
            return;
        }
        let page = if self.clan(clan_id).is_some_and(|c| c.new_leader_id == 0) {
            if let Some(clan) = self.clan_mut(clan_id) {
                clan.new_leader_id = member.object_id.0;
            }
            self.update_clan_in_db(clan_id);
            "data/scripts/village_master/ClanMaster/9000-07-success.htm"
        } else {
            "data/scripts/village_master/ClanMaster/9000-07-in-progress.htm"
        };
        if let Some(message) = self.html_from_file(player, npc.0, page) {
            self.send_html(player, &message);
        }
    }

    /// The `cancel_clan_leader_change` command.
    fn vm_cancel_clan_leader_change(&mut self, npc: ObjectId, player: ObjectId) {
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        let clan_id = self.clan_id_of(player);
        let message = if self.clan(clan_id).is_some_and(|c| c.new_leader_id != 0) {
            if let Some(clan) = self.clan_mut(clan_id) {
                clan.new_leader_id = 0;
            }
            self.update_clan_in_db(clan_id);
            self.html_from_file(
                player,
                npc.0,
                "data/scripts/village_master/ClanMaster/9000-07-canceled.htm",
            )
        } else {
            NpcHtmlMessage::new(
                npc.0,
                "<html><body>You don't have clan leader delegation applications submitted yet!</body></html>",
            )
        };
        if let Some(message) = message {
            self.send_html(player, &message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::java_split_space;

    #[test]
    fn commands_split_like_java() {
        assert_eq!(java_split_space("create_clan Name"), ["create_clan", "Name"]);
        assert_eq!(java_split_space("create_clan  Name"), ["create_clan", "", "Name"]);
        assert_eq!(java_split_space("create_clan Name  "), ["create_clan", "Name"]);
        assert_eq!(java_split_space("create_clan"), ["create_clan"]);
    }
}
