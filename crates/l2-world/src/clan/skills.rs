//! Clan skills: given to the members, switched off by a negative reputation, learned at the
//! clan trainer (`AcquireSkillType.PLEDGE` and `SUBPLEDGE`) and stored in `clan_skills`.
// Java: entity/clan/Clan.java (addNewSkill, addSkillEffects, removeSkillEffects, skillsStatus,
//       isLearnableSubSkill, isLearnableSubPledgeSkill, SubPledge skills)
// Java: data/xml/SkillTreeData.java (getAvailablePledgeSkills, getAvailableSubPledgeSkills)
// Java: entity/actor/instance/VillageMaster.java (showPledgeSkillList)
// Java: network/clientpackets/{RequestAcquireSkill,RequestAcquireSkillInfo}.java (the PLEDGE and
//       SUBPLEDGE cases, showSubUnitSkillList)
// Java: network/serverpackets/AcquireSkillInfo.java (the constructor without a custom SP)
//
// # Shape
// A clan keeps its skills as `(id, level)` (`Clan::skills`, `sub_pledge_skills`, `SubPledge::skills`).
// A member gets them as ordinary skills of the player (`World::add_skill`, never stored in
// `character_skills`): `add_skill_effects` when the player joins, enters the world or the clan
// learns a skill, `remove_skill_effects` when the player leaves. A clan with a negative reputation
// score has them switched off (`skills_status`, a disabled skill with no end).
//
// The trainer side follows M5-LEARN-01 (`skills/learn.rs` calls in here for the two clan types):
// the lists are made from the clan trees of `SkillTrees` (`pledge_skills`, `sub_pledge_skills`),
// the cost is clan reputation (`levelUpSp`) plus, for the clan skills, `LifeCrystalNeeded`.

use std::sync::Arc;

use l2_data::{SkillLearn, SkillTrees};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    AcquireSkillDone, AcquireSkillInfo, AcquireSkillList, AcquireSkillRequirement, AcquireSkillType,
    ActionFailed, ExSubPledgeSkillAdd, PledgeSkillList, PledgeSkillListAdd, SystemMessage,
    SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use super::model::{
    CLAN_SKILL_SUB_TYPE, Clan, ClanAccess, SUBUNIT_ACADEMY, java_hash_map_order, java_map_order,
};
use super::{ClanWrite, MemberInfo};
use crate::punishments::IllegalActionPunishment;
use crate::world::World;

impl Clan {
    /// `_subPledgeSkills` of the main squad (a `ConcurrentHashMap`) in the map's order.
    fn main_squad_skills(&self) -> Vec<(i32, i32)> {
        let keys: Vec<i32> = self.sub_pledge_skills.iter().map(|s| s.0).collect();
        java_map_order(&keys, keys.len())
            .into_iter()
            .filter_map(|i| self.sub_pledge_skills.get(i).copied())
            .collect()
    }

    /// `SubPledge.getSkills()` (a `HashMap`) in the map's order.
    fn squad_skills(&self, sub_type: i32) -> Option<Vec<(i32, i32)>> {
        let sub = self.sub_pledge(sub_type)?;
        let keys: Vec<i32> = sub.skills.iter().map(|s| s.0).collect();
        Some(
            java_hash_map_order(&keys)
                .into_iter()
                .filter_map(|i| sub.skills.get(i).copied())
                .collect(),
        )
    }

    /// The level of a clan skill (Java `getSkills().get(id)`).
    fn clan_skill_level(&self, skill_id: i32) -> Option<i32> {
        self.skills.iter().find(|s| s.0 == skill_id).map(|s| s.1)
    }

    /// Java `isLearnableSubSkill(skillId, skillLevel)`: some squad can take the skill.
    pub(crate) fn is_learnable_sub_skill(&self, skill_id: i32, skill_level: i32) -> bool {
        let next_or_first = |current: Option<i32>| match current {
            // Is next level?
            Some(level) => level + 1 == skill_level,
            // Is first level?
            None => skill_level == 1,
        };
        let main = self
            .sub_pledge_skills
            .iter()
            .find(|s| s.0 == skill_id)
            .map(|s| s.1);
        if next_or_first(main) {
            return true;
        }
        // Other sub-pledges.
        self.sub_pledges.iter().any(|sub| {
            // Disable academy.
            sub.id != SUBUNIT_ACADEMY
                && next_or_first(sub.skills.iter().find(|s| s.0 == skill_id).map(|s| s.1))
        })
    }

    /// Java `isLearnableSubPledgeSkill(skill, subType)`. `None` where Java throws: the squad
    /// does not exist (`_subPledges.get(subType)` is null).
    pub(crate) fn is_learnable_sub_pledge_skill(
        &self,
        skill_id: i32,
        skill_level: i32,
        sub_type: i32,
    ) -> Option<bool> {
        // Academy.
        if sub_type == SUBUNIT_ACADEMY {
            return Some(false);
        }
        let current = if sub_type == 0 {
            self.sub_pledge_skills
                .iter()
                .find(|s| s.0 == skill_id)
                .map(|s| s.1)
        } else {
            self.sub_pledge(sub_type)?
                .skills
                .iter()
                .find(|s| s.0 == skill_id)
                .map(|s| s.1)
        };
        Some(match current {
            // Is next level?
            Some(level) => level + 1 == skill_level,
            // Is first level?
            None => skill_level == 1,
        })
    }
}

/// The skills a member of squad `pledge_type` gets: the clan skills, and the squad's (`None`
/// when the squad does not exist: Java returns before it looks at the reputation).
struct MemberSkills {
    clan: Vec<(i32, i32)>,
    squad: Option<Vec<(i32, i32)>>,
    /// The squad skills come from the main squad (`player.getPledgeType() == 0`), the ones
    /// `addSkillEffects` checks against the squad tree.
    main_squad: bool,
}

impl World {
    // ------------------------------------------------------------------ on the members

    /// The skills of a clan for a member of `pledge_type`.
    fn member_skills(&self, clan_id: i32, pledge_type: i32) -> Option<MemberSkills> {
        let clan = self.clan(clan_id)?;
        let main_squad = pledge_type == 0;
        let squad = if main_squad {
            Some(clan.main_squad_skills())
        } else {
            clan.squad_skills(pledge_type)
        };
        Some(MemberSkills {
            clan: clan.all_skills(),
            squad,
            main_squad,
        })
    }

    /// Java `Clan.addSkillEffects(player)`: the clan's skills (those the player's pledge class
    /// reaches) and the skills of the player's squad join the player, not stored. Returns whether
    /// the player got any skill, so a caller can send the new `SkillList`.
    pub(crate) fn clan_add_skill_effects(&mut self, clan_id: i32, player: ObjectId) -> bool {
        if !self.players.contains_key(&player) {
            return false;
        }
        let state = self.player_clan_state(player);
        let Some(skills) = self.member_skills(clan_id, state.pledge_type) else {
            return false;
        };
        let trees = Arc::clone(&self.rewards.skill_trees);
        let player_social_class = state.pledge_class + 1;
        let reaches = |learn: Option<&SkillLearn>| {
            learn
                .and_then(|l| l.social_class)
                .is_none_or(|class| player_social_class >= class.ordinal())
        };
        let mut add: Vec<(i32, i32)> = skills
            .clan
            .iter()
            .copied()
            .filter(|&(id, level)| reaches(trees.pledge_skill(id, level)))
            .collect();
        match &skills.squad {
            Some(squad) if skills.main_squad => add.extend(
                squad
                    .iter()
                    .copied()
                    .filter(|&(id, level)| reaches(trees.sub_pledge_skill(id, level))),
            ),
            Some(squad) => add.extend(squad.iter().copied()),
            None => {}
        }
        let mut added = false;
        for (id, level) in add {
            // Skill is not saved to player DB.
            added |= self.add_skill(player, id, level);
        }
        // `if (subunit == null) return;` comes before the reputation check.
        if skills.squad.is_some() && self.clan(clan_id).is_some_and(|c| c.reputation_score < 0) {
            self.clan_skills_status(clan_id, player, true);
        }
        added
    }

    /// Java `Clan.removeSkillEffects(player)`.
    pub(crate) fn clan_remove_skill_effects(&mut self, clan_id: i32, player: ObjectId) {
        if !self.players.contains_key(&player) {
            return;
        }
        let pledge_type = self.player_clan_state(player).pledge_type;
        let Some(skills) = self.member_skills(clan_id, pledge_type) else {
            return;
        };
        for (id, _) in skills.clan.into_iter().chain(skills.squad.into_iter().flatten()) {
            // Skill is not saved to player DB.
            self.remove_skill(player, id, true);
        }
    }

    /// Java `Clan.skillsStatus(player, disable)`: a disabled clan skill cannot be used until it is
    /// enabled again.
    pub(crate) fn clan_skills_status(&mut self, clan_id: i32, player: ObjectId, disable: bool) {
        if !self.players.contains_key(&player) {
            return;
        }
        let pledge_type = self.player_clan_state(player).pledge_type;
        let Some(skills) = self.member_skills(clan_id, pledge_type) else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        for (id, level) in skills.clan.into_iter().chain(skills.squad.into_iter().flatten()) {
            let Some(template) = table.get(id, level) else {
                continue;
            };
            if disable {
                self.disable_skill(player, template, -1);
            } else {
                self.enable_skill(player, template);
            }
        }
    }

    /// `skillsStatus(member.getPlayer(), disable)` for every online member (Java
    /// `Clan.setReputationScore`).
    pub(crate) fn clan_skills_status_for_members(&mut self, clan_id: i32, disable: bool) {
        for member in self.clan_online_members(clan_id, 0) {
            self.clan_skills_status(clan_id, member, disable);
        }
    }

    // ------------------------------------------------------------------ learning

    /// Java `Clan.addNewSkill(newSkill, subType)`: the skill joins the clan (`-2`: the clan, `0`:
    /// the main squad, otherwise the squad of that id), `clan_skills` is written, and the online
    /// members learn it. Returns the old level of the skill, `None` where Java returns null.
    pub(crate) fn clan_add_new_skill(
        &mut self,
        clan_id: i32,
        skill_id: i32,
        skill_level: i32,
        sub_type: i32,
    ) -> Option<i32> {
        let table = self.skill_table()?;
        // `newSkill != null`
        let template = table.get(skill_id, skill_level)?;
        let skill_name = template.name.clone();
        let min_pledge_class = template.min_pledge_class;
        let clan = self.clan_mut(clan_id)?;
        let put = |skills: &mut Vec<(i32, i32)>| match skills.iter_mut().find(|s| s.0 == skill_id) {
            Some(entry) => Some(std::mem::replace(&mut entry.1, skill_level)),
            None => {
                skills.push((skill_id, skill_level));
                None
            }
        };
        let old_level = if sub_type == CLAN_SKILL_SUB_TYPE {
            put(&mut clan.skills)
        } else if sub_type == 0 {
            put(&mut clan.sub_pledge_skills)
        } else {
            let Some(sub) = clan.sub_pledge_mut(sub_type) else {
                warn!("Subpledge {sub_type} does not exist for clan {clan_id}");
                return None;
            };
            put(&mut sub.skills)
        };

        // NOTE(java-quirk): the update does not look at `sub_pledge_id`, so a skill id that the
        // clan and a squad (or two squads) share changes in all of them.
        let write = match old_level {
            Some(_) => ClanWrite::UpdateSkill {
                clan_id,
                skill_id,
                skill_level,
            },
            None => ClanWrite::InsertSkill {
                clan_id,
                skill_id,
                skill_level,
                skill_name,
                sub_pledge_id: sub_type,
            },
        };
        self.clan_write(write);

        let message = SystemMessage::new(SystemMessageId::TheClanSkillS1HasBeenAdded).with(
            SystemMessageParam::SkillName {
                id: skill_id,
                level: 1,
            },
        );
        let members: Vec<MemberInfo> = self
            .clan_members_info(clan_id)
            .into_iter()
            .filter(|m| m.online && self.players.contains_key(&m.object_id))
            .collect();
        for member in members {
            let id = member.object_id;
            if sub_type == CLAN_SKILL_SUB_TYPE {
                let pledge_class = self.player_clan_state(id).pledge_class;
                if min_pledge_class <= pledge_class {
                    // Skill is not saved to player DB.
                    self.add_skill(id, skill_id, skill_level);
                    self.send(
                        id,
                        &PledgeSkillListAdd {
                            id: skill_id,
                            level: skill_level,
                        },
                    );
                    self.send(id, &message);
                    self.send_skill_list(id);
                }
            } else if member.pledge_type == sub_type {
                self.add_skill(id, skill_id, skill_level);
                self.send(
                    id,
                    &ExSubPledgeSkillAdd {
                        sub_type,
                        skill_id,
                        skill_level,
                    },
                );
                self.send(id, &message);
                self.send_skill_list(id);
            }
        }
        old_level
    }

    /// `clan.broadcastToOnlineMembers(new PledgeSkillList(clan))`.
    fn broadcast_pledge_skill_list(&self, clan_id: i32) {
        if let Some(clan) = self.clan(clan_id) {
            let list: PledgeSkillList = self.pledge_skill_list(clan);
            self.broadcast_to_clan(clan_id, &list);
        }
    }

    /// Java `SkillTreeData.getAvailablePledgeSkills(clan)`: the lines of the clan tree the clan can
    /// learn now (not the castle skills), in the tree's map order.
    fn available_pledge_skills(trees: &SkillTrees, clan: &Clan) -> Vec<SkillLearn> {
        let mut result = Vec::new();
        for skill in trees.pledge_skills() {
            if skill.residence_skill || clan.level < skill.get_level {
                continue;
            }
            match clan.clan_skill_level(skill.skill_id) {
                Some(old) => {
                    if old + 1 == skill.skill_level {
                        result.push(skill.clone());
                    }
                }
                None => {
                    if skill.skill_level == 1 {
                        result.push(skill.clone());
                    }
                }
            }
        }
        result
    }

    /// Java `SkillTreeData.getAvailableSubPledgeSkills(clan)`.
    fn available_sub_pledge_skills(trees: &SkillTrees, clan: &Clan) -> Vec<SkillLearn> {
        trees
            .sub_pledge_skills()
            .iter()
            .filter(|skill| {
                clan.level >= skill.get_level
                    && clan.is_learnable_sub_skill(skill.skill_id, skill.skill_level)
            })
            .cloned()
            .collect()
    }

    /// Java `VillageMaster.showPledgeSkillList(player)`: the clan skills the clan can learn, or
    /// why it cannot.
    pub fn show_pledge_skill_list(&mut self, player: ObjectId) {
        if !self.is_clan_leader(player) {
            if let Some(html) = self.html_from_file(player, 0, "data/html/villagemaster/NotClanLeader.htm") {
                self.send_html(player, &html);
            }
            self.send(player, &ActionFailed);
            return;
        }
        let clan_id = self.clan_id_of(player);
        let trees = Arc::clone(&self.rewards.skill_trees);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let clan_level = clan.level;
        let skills = Self::available_pledge_skills(&trees, clan);
        let mut list = AcquireSkillList::new(AcquireSkillType::Pledge);
        for s in &skills {
            // NOTE(java-quirk): `getSocialClass()` is null for a line without a `socialClass`
            // and Java throws; every shipped line has one.
            let social_class = s.social_class.map_or(0, |c| c.ordinal());
            list.add_skill(
                s.skill_id,
                s.skill_level,
                s.skill_level,
                s.level_up_sp,
                social_class,
            );
        }
        if skills.is_empty() {
            if clan_level < 8 {
                let level = if clan_level < 5 { 5 } else { clan_level + 1 };
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YouDoNotHaveAnyFurtherSkillsToLearnComeBackWhenYouHaveReachedLevelS1,
                    )
                    .with(SystemMessageParam::IntNumber(level)),
                );
            } else if let Some(html) =
                self.html_from_file(player, 0, "data/html/villagemaster/NoMoreSkills.htm")
            {
                self.send_html(player, &html);
            }
        } else {
            self.send(player, &list);
        }
    }

    /// Java `RequestAcquireSkill.showSubUnitSkillList(player)` (the castle court magician and the
    /// support unit captain scripts call it too).
    pub fn show_sub_unit_skill_list(&mut self, player: ObjectId) {
        let trees = Arc::clone(&self.rewards.skill_trees);
        let Some(clan) = self.clan_of(player) else {
            return;
        };
        let skills = Self::available_sub_pledge_skills(&trees, clan);
        let mut list = AcquireSkillList::new(AcquireSkillType::Subpledge);
        let mut count = 0;
        for s in &skills {
            if self.skill_exists(s.skill_id, s.skill_level) {
                list.add_skill(s.skill_id, s.skill_level, s.skill_level, s.level_up_sp, 0);
                count += 1;
            }
        }
        if count == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThereAreNoOtherSkillsToLearn),
            );
        } else {
            self.send(player, &list);
        }
    }

    fn skill_exists(&self, skill_id: i32, level: i32) -> bool {
        self.skill_table()
            .is_some_and(|t| t.get(skill_id, level).is_some())
    }

    /// The `RequestAcquireSkillInfo` of a clan skill or a squad skill (Java: the `PLEDGE` and
    /// `SUBPLEDGE` cases, after the common checks).
    pub(crate) fn on_pledge_acquire_skill_info(
        &mut self,
        player: ObjectId,
        skill_id: i32,
        level: i32,
        skill_type: AcquireSkillType,
    ) {
        let trees = Arc::clone(&self.rewards.skill_trees);
        let learn = match skill_type {
            AcquireSkillType::Pledge => trees.pledge_skill(skill_id, level),
            _ => trees.sub_pledge_skill(skill_id, level),
        };
        let Some(learn) = learn else {
            return;
        };
        if skill_type == AcquireSkillType::Pledge {
            if !self.is_clan_leader(player) {
                return;
            }
        } else if !self.is_clan_leader(player) || !self.has_clan_access(player, ClanAccess::MemberFame) {
            return;
        }
        let life_crystal_needed = self.clans.settings.life_crystal_needed;
        let divine_sp_book_needed = self.settings.learn.divine_sp_book_needed;
        let mut requirements = Vec::new();
        if skill_type != AcquireSkillType::Pledge || life_crystal_needed {
            for item in &learn.required_items {
                if !divine_sp_book_needed && skill_id == l2_data::skill_tree::DIVINE_INSPIRATION_ID {
                    continue;
                }
                requirements.push(AcquireSkillRequirement::item(item.item_id, item.count));
            }
        }
        self.send(
            player,
            &AcquireSkillInfo {
                skill_type,
                skill_id: learn.skill_id,
                level: learn.skill_level,
                sp_cost: learn.level_up_sp,
                requirements,
            },
        );
    }

    /// Java `RequestAcquireSkill`'s `PLEDGE` case: the clan leader buys a clan skill with clan
    /// reputation (and the life crystal the tree lists).
    pub(crate) fn on_acquire_pledge_skill(&mut self, player: ObjectId, skill_id: i32, level: i32) {
        let trees = Arc::clone(&self.rewards.skill_trees);
        let Some(learn) = trees.pledge_skill(skill_id, level) else {
            return;
        };
        if !self.is_clan_leader(player) {
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(reputation) = self.clan(clan_id).map(|c| c.reputation_score) else {
            return;
        };
        let rep_cost = learn.level_up_sp;
        if reputation < rep_cost {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::TheAttemptToAcquireTheSkillHasFailedBecauseOfAnInsufficientClanReputationScore,
                ),
            );
            self.show_pledge_skill_list(player);
            return;
        }
        if self.clans.settings.life_crystal_needed {
            for item in &learn.required_items {
                // NOTE(java-quirk): the items are taken one after the other; one that is missing
                // leaves the ones before it destroyed.
                if !self.destroy_item_by_item_id(player, item.item_id, item.count, false) {
                    // Doesn't have required item.
                    self.send(
                        player,
                        &SystemMessage::new(
                            SystemMessageId::YouDoNotHaveTheNecessaryMaterialsOrPrerequisitesToLearnThisSkill,
                        ),
                    );
                    self.show_pledge_skill_list(player);
                    return;
                }
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                        .with(SystemMessageParam::ItemName(item.item_id))
                        .with(SystemMessageParam::LongNumber(item.count)),
                );
            }
        }
        self.take_reputation_score(clan_id, rep_cost);
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore)
                .with(SystemMessageParam::IntNumber(rep_cost)),
        );
        self.clan_add_new_skill(clan_id, skill_id, level, CLAN_SKILL_SUB_TYPE);
        self.broadcast_pledge_skill_list(clan_id);
        self.send(player, &AcquireSkillDone);
        self.show_pledge_skill_list(player);
    }

    /// Java `RequestAcquireSkill`'s `SUBPLEDGE` case: a squad skill of a clan with a castle or a
    /// fortress, bought by a leader who may handle fame.
    pub(crate) fn on_acquire_sub_pledge_skill(
        &mut self,
        player: ObjectId,
        skill_id: i32,
        level: i32,
        sub_type: i32,
    ) {
        let trees = Arc::clone(&self.rewards.skill_trees);
        let Some(learn) = trees.sub_pledge_skill(skill_id, level) else {
            return;
        };
        if !self.is_clan_leader(player) || !self.has_clan_access(player, ClanAccess::MemberFame) {
            return;
        }
        let clan_id = self.clan_id_of(player);
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        if clan.fort_id == 0 && clan.castle_id == 0 {
            return;
        }
        let reputation = clan.reputation_score;
        // Hack check. Check if SubPledge can accept the new skill.
        // NOTE(java-quirk): a squad that does not exist throws a NullPointerException in
        // `isLearnableSubPledgeSkill`: the request ends without an answer.
        let Some(learnable) = clan.is_learnable_sub_pledge_skill(skill_id, level, sub_type) else {
            return;
        };
        if !learnable {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThisSquadSkillHasAlreadyBeenAcquired),
            );
            let text = self.player_text(player);
            self.handle_illegal_player_action(
                player,
                &format!("{text} is requesting skill Id: {skill_id} level {level} without knowing it's previous level!"),
                IllegalActionPunishment::None,
            );
            return;
        }
        let rep_cost = learn.level_up_sp;
        if reputation < rep_cost {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::TheAttemptToAcquireTheSkillHasFailedBecauseOfAnInsufficientClanReputationScore,
                ),
            );
            return;
        }
        for item in &learn.required_items {
            if !self.destroy_item_by_item_id(player, item.item_id, item.count, false) {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YouDoNotHaveTheNecessaryMaterialsOrPrerequisitesToLearnThisSkill,
                    ),
                );
                return;
            }
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S2S1HasDisappeared)
                    .with(SystemMessageParam::ItemName(item.item_id))
                    .with(SystemMessageParam::LongNumber(item.count)),
            );
        }
        if rep_cost > 0 {
            self.take_reputation_score(clan_id, rep_cost);
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore)
                    .with(SystemMessageParam::IntNumber(rep_cost)),
            );
        }
        self.clan_add_new_skill(clan_id, skill_id, level, sub_type);
        self.broadcast_pledge_skill_list(clan_id);
        self.send(player, &AcquireSkillDone);
        self.show_sub_unit_skill_list(player);
    }
}
