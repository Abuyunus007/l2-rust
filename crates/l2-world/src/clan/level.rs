//! Clan level, reputation, sub pledges, rank privileges and the clan notice.
// Java: entity/clan/Clan.java (levelUpClan, changeLevel, addReputationScore,
//       takeReputationScore, setReputationScore, createSubPledge, setRankPrivs, storeNotice,
//       setNotice, setNoticeEnabled)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{StatusUpdate, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::ClanWrite;
use super::model::{MAX_NOTICE_LENGTH, SUBUNIT_ACADEMY, SUBUNIT_KNIGHT1, SUBUNIT_ROYAL2, SubPledge};
use super::table::MAX_REPUTATION;
use crate::world::{World, unix_ms_now};

/// Item 1419 (Blood Mark), the price of level 3.
const BLOOD_MARK: i32 = 1419;
/// Item 3874 (Alliance Manifesto), the price of level 4.
const ALLIANCE_MANIFESTO: i32 = 3874;
/// Item 3870 (Seal of Aspiration), the price of level 5.
const SEAL_OF_ASPIRATION: i32 = 3870;
/// Item 9910 (Blood Oath), 150 of them for level 9.
const BLOOD_OATH: i32 = 9910;
/// Item 9911 (Blood Alliance), 5 of them for level 10.
const BLOOD_ALLIANCE: i32 = 9911;

impl World {
    // ------------------------------------------------------------------ level

    /// Java `Clan.levelUpClan(player)`. `true` when the clan reached the next level.
    pub fn level_up_clan(&mut self, player: ObjectId) -> bool {
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return false;
        };
        let (level, reputation, members, dissolving) = (
            clan.level,
            clan.reputation_score,
            clan.members_count(),
            clan.dissolving_expiry_time,
        );
        if !self.is_clan_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return false;
        }
        if unix_ms_now() < dissolving {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::AsYouAreCurrentlyScheduleForClanDissolutionYourClanLevelCannotBeIncreased,
                ),
            );
            return false;
        }

        let sp = self.players.get(&player).map_or(0, |p| p.data.sp);
        let settings = self.clans.settings.clone();
        let mut increase = false;
        match level {
            // Upgrade to 1 and 2: SP and adena.
            0 | 1 => {
                let (sp_cost, adena) = if level == 0 {
                    (20_000, 650_000)
                } else {
                    (100_000, 2_500_000)
                };
                if sp >= sp_cost && self.adena_of(player) >= adena && self.reduce_adena(player, adena, true) {
                    self.take_sp(player, sp_cost);
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YourSpHasDecreasedByS1)
                            .with(SystemMessageParam::IntNumber(sp_cost as i32)),
                    );
                    increase = true;
                }
            }
            // Upgrade to 3, 4 and 5: SP and one item.
            2..=4 => {
                let (sp_cost, item) = match level {
                    2 => (350_000, BLOOD_MARK),
                    3 => (1_000_000, ALLIANCE_MANIFESTO),
                    _ => (2_500_000, SEAL_OF_ASPIRATION),
                };
                if sp >= sp_cost
                    && self.has_inventory_item(player, item)
                    && self.destroy_item_by_item_id(player, item, 1, false)
                {
                    self.take_sp(player, sp_cost);
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YourSpHasDecreasedByS1)
                            .with(SystemMessageParam::IntNumber(sp_cost as i32)),
                    );
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::S1HasDisappeared)
                            .with(SystemMessageParam::ItemName(item)),
                    );
                    increase = true;
                }
            }
            // Upgrade to 6, 7 and 8: reputation and members.
            5..=7 => {
                let index = (level - 5) as usize;
                let cost = settings.clan_level_costs[index];
                if reputation >= cost && members_enough(members, settings.clan_level_requirements[index]) {
                    self.set_reputation_score(clan_id, reputation - cost);
                    self.send_reputation_deducted(player, cost);
                    increase = true;
                }
            }
            // Upgrade to 9 (itemId 9910 = Blood Oath).
            8 => {
                let cost = settings.clan_level_costs[3];
                if reputation >= cost
                    && self.has_inventory_item(player, BLOOD_OATH)
                    && members_enough(members, settings.clan_level_requirements[3])
                    && self.destroy_item_by_item_id(player, BLOOD_OATH, 150, false)
                {
                    self.set_reputation_score(clan_id, reputation - cost);
                    self.send_reputation_deducted(player, cost);
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                            .with(SystemMessageParam::ItemName(BLOOD_OATH))
                            .with(SystemMessageParam::LongNumber(150)),
                    );
                    increase = true;
                }
            }
            // Upgrade to 10 (itemId 9911 = Blood Alliance).
            9 => {
                let cost = settings.clan_level_costs[4];
                if reputation >= cost
                    && self.has_inventory_item(player, BLOOD_ALLIANCE)
                    && members_enough(members, settings.clan_level_requirements[4])
                    && self.destroy_item_by_item_id(player, BLOOD_ALLIANCE, 5, false)
                {
                    self.set_reputation_score(clan_id, reputation - cost);
                    self.send_reputation_deducted(player, cost);
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                            .with(SystemMessageParam::ItemName(BLOOD_ALLIANCE))
                            .with(SystemMessageParam::LongNumber(5)),
                    );
                    increase = true;
                }
            }
            // Upgrade to 11: a territory (`TerritoryWarManager`, M7) is needed; no clan owns one
            // yet, so the conditions are never met.
            10 => {}
            _ => return false,
        }

        if !increase {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::TheConditionsNecessaryToIncreaseTheClanSLevelHaveNotBeenMet,
                ),
            );
            return false;
        }

        // "the player should know that he has less sp now :p"
        let sp_now = self.players.get(&player).map_or(0, |p| p.data.sp);
        let mut su = StatusUpdate::new(player.0);
        su.add_attribute(status::SP, sp_now as i32);
        self.send(player, &su);
        self.send_item_list(player, false);
        self.change_clan_level(clan_id, level + 1);
        // Scripts: `OnPlayerClanLvlUp` (D-05).
        true
    }

    /// `player.setSp(player.getSp() - cost)` (not below 0).
    fn take_sp(&mut self, player: ObjectId, cost: i64) {
        if let Some(p) = self.players.get_mut(&player) {
            p.data.sp = (p.data.sp - cost).max(0);
        }
    }

    fn send_reputation_deducted(&self, player: ObjectId, cost: i32) {
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore)
                .with(SystemMessageParam::IntNumber(cost)),
        );
    }

    /// `player.getInventory().getItemByItemId(itemId) != null`.
    fn has_inventory_item(&self, player: ObjectId, item_id: i32) -> bool {
        self.players.get(&player).is_some_and(|p| {
            p.data
                .items
                .iter()
                .any(|i| i.item_id == item_id && (i.loc == "INVENTORY" || i.loc == "PAPERDOLL"))
        })
    }

    /// Java `Clan.changeLevel(level)`.
    pub fn change_clan_level(&mut self, clan_id: i32, level: i32) {
        self.clan_write(ClanWrite::Level { clan_id, level });
        // `setLevel` (the community board forum: not ported) and `setRank`.
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.level = level;
        }
        let rank = self.clan_rank(clan_id);
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.rank = rank;
        }
        let leader = self.clan(clan_id).and_then(|c| c.leader_member().cloned());
        if let Some(leader) = leader
            && self.member_info(&leader).online
        {
            if level > 4 {
                // The siege skills (M7-SIEGE-01).
                self.add_siege_skills(leader.object_id);
                self.send(
                    leader.object_id,
                    &SystemMessage::new(
                        SystemMessageId::NowThatYourClanLevelIsAboveLevel5ItCanAccumulateClanReputationPoints,
                    ),
                );
            } else {
                self.remove_siege_skills(leader.object_id);
            }
        }
        // Notify all the members about it.
        self.broadcast_to_clan(
            clan_id,
            &SystemMessage::new(SystemMessageId::YourClanSLevelHasIncreased),
        );
        self.broadcast_clan_info_update(clan_id);
    }

    // ------------------------------------------------------------------ reputation

    /// Java `Clan.addReputationScore(value)`.
    pub fn add_reputation_score(&mut self, clan_id: i32, value: i32) {
        if let Some(score) = self.clan(clan_id).map(|c| c.reputation_score) {
            self.set_reputation_score(clan_id, score.wrapping_add(value));
        }
    }

    /// Java `Clan.takeReputationScore(value)`.
    pub fn take_reputation_score(&mut self, clan_id: i32, value: i32) {
        if let Some(score) = self.clan(clan_id).map(|c| c.reputation_score) {
            self.set_reputation_score(clan_id, score.wrapping_sub(value));
        }
    }

    /// Java `Clan.setReputationScore(value)`: crossing 0 switches the clan skills off or on, the
    /// score stays within ±100,000,000, and the members see the new score. Java writes it with
    /// the next `updateClanInDB` (at the latest at shutdown).
    pub(crate) fn set_reputation_score(&mut self, clan_id: i32, value: i32) {
        let Some(old) = self.clan(clan_id).map(|c| c.reputation_score) else {
            return;
        };
        if old >= 0 && value < 0 {
            self.broadcast_to_clan(
                clan_id,
                &SystemMessage::new(
                    SystemMessageId::SinceTheClanReputationScoreHasDroppedTo0OrLowerYourClanSkillSWillBeDeActivated,
                ),
            );
            self.clan_skills_status_for_members(clan_id, true);
        } else if old < 0 && value >= 0 {
            self.broadcast_to_clan(
                clan_id,
                &SystemMessage::new(
                    SystemMessageId::ClanSkillsWillNowBeActivatedSinceTheClanSReputationScoreIs0OrHigher,
                ),
            );
            self.clan_skills_status_for_members(clan_id, false);
        }
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.reputation_score = value.clamp(-MAX_REPUTATION, MAX_REPUTATION);
            clan.changed = true;
        }
        self.broadcast_clan_info_update(clan_id);
    }

    // ------------------------------------------------------------------ sub pledges

    /// Java `Clan.createSubPledge(player, pledgeType, leaderId, subPledgeName)`. The id of the new
    /// unit, `None` when it could not be made.
    pub(crate) fn create_sub_pledge(
        &mut self,
        player: ObjectId,
        clan_id: i32,
        pledge_type_value: i32,
        leader_id: i32,
        name: &str,
    ) -> Option<i32> {
        let clan = self.clan(clan_id)?;
        let pledge_type = clan.available_pledge_types(pledge_type_value);
        if pledge_type == 0 {
            // NOTE(java-quirk): `pledgeType == SUBUNIT_ACADEMY` is tested after `pledgeType == 0`,
            // so the academy message is never sent.
            if pledge_type == SUBUNIT_ACADEMY {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YourClanHasAlreadyEstablishedAClanAcademy),
                );
            } else {
                self.send_message(player, "You can't create any more sub-units of this type");
            }
            return None;
        }
        if clan.leader_id() == leader_id {
            self.send_message(player, "Leader is not correct");
            return None;
        }
        let reputation = clan.reputation_score;
        let settings = &self.clans.settings;
        // Royal Guard 5000 points per each, Order of Knights 10000 points per each.
        if pledge_type != -1
            && ((reputation < settings.royal_guard_cost && pledge_type < SUBUNIT_KNIGHT1)
                || (reputation < settings.knight_unit_cost && pledge_type > SUBUNIT_ROYAL2))
        {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheClanReputationScoreIsTooLow),
            );
            return None;
        }
        let (royal_cost, knight_cost) = (settings.royal_guard_cost, settings.knight_unit_cost);

        self.clan_write(ClanWrite::InsertSubPledge {
            clan_id,
            sub_pledge_id: pledge_type,
            name: name.to_owned(),
            leader_id: if pledge_type != -1 { leader_id } else { 0 },
        });
        if let Some(clan) = self.clan_mut(clan_id) {
            // NOTE: the object keeps the leader id given, also for the academy (only the row gets
            // 0).
            clan.put_sub_pledge(SubPledge {
                id: pledge_type,
                name: name.to_owned(),
                leader_id,
                skills: Vec::new(),
            });
        }
        if pledge_type != -1 {
            let cost = if pledge_type < SUBUNIT_KNIGHT1 {
                royal_cost
            } else {
                // TODO in Java: a clan of level 9 or more reinforces knights cheaper
                // (`KNIGHT_REINFORCE_COST`).
                knight_cost
            };
            self.set_reputation_score(clan_id, reputation - cost);
        }
        self.broadcast_clan_info_update(clan_id);
        if let Some(packet) = self.pledge_sub_pledge_created(clan_id, pledge_type) {
            self.broadcast_to_clan(clan_id, &packet);
        }
        Some(pledge_type)
    }

    // ------------------------------------------------------------------ rank privileges

    /// Java `Clan.setRankPrivs(rank, privs)`.
    pub(crate) fn set_rank_privs(&mut self, clan_id: i32, rank: i32, privs: i32) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let known = clan.has_rank(rank);
        if let Some(clan) = self.clan_mut(clan_id) {
            clan.put_rank_privs(rank, privs);
        }
        self.clan_write(ClanWrite::RankPrivs { clan_id, rank, privs });
        if !known {
            return;
        }
        for member in self.clan_members_info(clan_id) {
            if member.online && member.power_grade == rank {
                self.set_player_privileges_mask(member.object_id, privs);
                self.send_user_info(member.object_id);
            }
        }
        self.broadcast_clan_status(clan_id);
    }

    // ------------------------------------------------------------------ notice

    /// Java `Clan.setNotice(notice)`.
    pub fn set_clan_notice(&mut self, clan_id: i32, notice: &str) {
        let enabled = self.clan(clan_id).is_some_and(|c| c.notice_enabled);
        self.store_clan_notice(clan_id, notice, enabled);
    }

    /// Java `Clan.setNoticeEnabled(enabled)`: stores `getNotice()` (the text without tags).
    pub fn set_clan_notice_enabled(&mut self, clan_id: i32, enabled: bool) {
        let Some(notice) = self.clan(clan_id).map(|c| c.notice_text()) else {
            return;
        };
        self.store_clan_notice(clan_id, &notice, enabled);
    }

    /// Java `Clan.storeNotice(notice, enabled)`.
    fn store_clan_notice(&mut self, clan_id: i32, notice: &str, enabled: bool) {
        // NOTE(java-quirk): a notice longer than `MAX_NOTICE_LENGTH` is cut to one character less
        // (`substring(0, MAX_NOTICE_LENGTH - 1)`), in UTF-16 units.
        let units: Vec<u16> = notice.encode_utf16().collect();
        let notice = if units.len() > MAX_NOTICE_LENGTH {
            String::from_utf16_lossy(&units[..MAX_NOTICE_LENGTH - 1])
        } else {
            notice.to_owned()
        };
        self.clan_write(ClanWrite::Notice {
            clan_id,
            notice: notice.clone(),
            enabled,
        });
        match self.clan_mut(clan_id) {
            Some(clan) => {
                clan.notice = Some(notice);
                clan.notice_enabled = enabled;
            }
            None => warn!("Clan {clan_id}: a notice for a clan that does not exist."),
        }
    }
}

/// `_members.size() >= requirement`.
fn members_enough(members: usize, requirement: i32) -> bool {
    i64::try_from(members).unwrap_or(i64::MAX) >= i64::from(requirement)
}
