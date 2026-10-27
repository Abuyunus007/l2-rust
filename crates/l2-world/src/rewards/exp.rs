//! Exp, SP and levels: the exp part of `Attackable.calculateRewards`, `addExpAndSp`, levelling up
//! (and down), the skills a level brings, and the exp a death costs.
// Java: entity/actor/Attackable.java (calculateRewards, getMainDamageDealer, calculateExpAndSp)
// Java: entity/actor/Npc.java (getExpReward, getSpReward)
// Java: entity/actor/stat/PlayableStat.java (addExp, removeExp, addLevel, addSp)
// Java: entity/actor/stat/PlayerStat.java (addExp, addExpAndSp, addLevel, addSp, getExpBonusMultiplier)
// Java: entity/actor/Player.java (rewardSkills, giveAvailableAutoGetSkills, checkPlayerSkills,
//       calculateDeathExpPenalty, restoreExp, isLucky)
// Java: data/xml/{ExperienceData,ExperienceLossData,KarmaLossData,SkillTreeData}.java
// Java: mechanics/stats/Formulas.java (calculateKarmaLost)
//
// # What is here
// A player gets exp and SP from a monster in `World::reward_exp_and_sp`, which `calculate_rewards`
// (`death.rs`) calls after the drops. Everything below it is the chain of Java's stat classes:
//
//   `add_exp_and_sp`  PlayerStat.addExpAndSp   (bonuses, messages)
//     `add_exp`       PlayerStat.addExp        (access level, karma, UserInfo)
//       `playable_add_exp`  PlayableStat.addExp  (the cap, the new level)
//         `add_level` PlayerStat.addLevel      (CP, SocialAction, rewardSkills, the packets)
//           `playable_add_level`  PlayableStat.addLevel  (HP and MP to the maximum)
//     `add_sp`        PlayerStat.addSp         (the SP StatusUpdate)
//
// # What is left out, with the system that brings it
// - Parties: `reward_party_share` is the party branch of `calculateRewards`;
//   `Party.distributeXpAndSp` itself is `party/rewards.rs` (M6-PARTY-01). The servitor penalty
//   is missing.
// - Vitality and Nevit are in `vitality.rs` and `nevit.rs`
//   (M4-VIT-01); this file calls them where `PlayerStat.addExpAndSp` and
//   `Attackable.calculateRewards` do.
// - The over-hit bonus (a skill effect,
//   M5), champions (not rolled), premium rates (`PremiumSystemEnabled` is off by default),
//   `DynamicExpRates.xml` (every shipped rate is 1, so Java reports it disabled), subclasses
//   (`isSubClassActive`), clans and the tutorial quest event, scripts (D-05).
// - `AutoLearnSkills = True` is `skills/learn.rs` (`give_available_skills`), with the item
//   requirements of the skill trees.

use std::sync::Arc;

use l2_data::{ClassId, ExperienceLoss, ExperienceTable, KarmaLoss, SkillTrees, Stat};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::status as attr;
use l2_protocol_game::server::{SocialAction, StatusUpdate, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use crate::outbox::PersistCommand;
use crate::player::SkillData;
use crate::stats::java_round;
use crate::target::distance_3d;
use crate::world::World;
use crate::zones::ZoneId;

/// Java `SocialAction.LEVEL_UP`.
const LEVEL_UP_ACTION: i32 = 2122;
/// Java `CommonSkill.LUCKY` (a passive skill of every starting class).
const LUCKY_SKILL_ID: i32 = 194;
/// Java `CommonSkill.EXPERTISE`: the one skill `checkPlayerSkills` does not give 9 levels of grace.
const EXPERTISE_SKILL_ID: i32 = 239;

/// The `.ini` values exp, SP and levels read. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpSettings {
    /// `RatesConfig.RATE_XP` (`RateXp`).
    pub rate_xp: f32,
    /// `RatesConfig.RATE_SP` (`RateSp`).
    pub rate_sp: f32,
    /// `RatesConfig.MONSTER_EXP_MAX_LEVEL_DIFFERENCE` (`MonsterExpMaxLevelDifference`).
    pub monster_exp_max_level_difference: i32,
    /// `RatesConfig.RATE_KARMA_LOST` (`RateKarmaLost`), with Java's post-processing: -1 means
    /// `RateXp`.
    pub rate_karma_lost: f32,
    /// `RatesConfig.RATE_KARMA_EXP_LOST` (`RateKarmaExpLost`).
    pub rate_karma_exp_lost: f32,
    /// `PlayerConfig.PLAYER_DELEVEL` (`Delevel`): a death costs exp.
    pub player_delevel: bool,
    /// `PlayerConfig.DECREASE_SKILL_LEVEL` (`DecreaseSkillOnDelevel`).
    pub decrease_skill_level: bool,
    /// `PlayerConfig.AUTO_LEARN_SKILLS` (`AutoLearnSkills`).
    pub auto_learn_skills: bool,
    /// `PlayerConfig.MAX_SP` (`MaxSp`; a negative value is `Long.MAX_VALUE`).
    pub max_sp: i64,
    /// `PlayerConfig.MAX_BONUS_EXP` (`MaxExpBonus`).
    pub max_bonus_exp: f64,
    /// `PlayerConfig.MAX_BONUS_SP` (`MaxSpBonus`).
    pub max_bonus_sp: f64,
    /// `PlayerConfig.ALT_PARTY_RANGE` (`AltPartyRange`): how far a damage dealer may be from the
    /// monster and still get a share.
    pub alt_party_range: i32,
    /// `FreeMountsConfig.ENABLE_FREE_STRIDER` (`EnableFreeStrider`, `Custom/FreeMounts.ini`).
    pub enable_free_strider: bool,
    /// `FreeMountsConfig.ENABLE_FREE_WYVERN` (`EnableFreeWyvern`).
    pub enable_free_wyvern: bool,
}

impl Default for ExpSettings {
    fn default() -> Self {
        Self {
            rate_xp: 1.0,
            rate_sp: 1.0,
            monster_exp_max_level_difference: 11,
            rate_karma_lost: 1.0,
            rate_karma_exp_lost: 1.0,
            player_delevel: true,
            decrease_skill_level: true,
            auto_learn_skills: false,
            max_sp: 50_000_000_000,
            max_bonus_exp: 3.5,
            max_bonus_sp: 3.5,
            alt_party_range: 1500,
            enable_free_strider: false,
            enable_free_wyvern: false,
        }
    }
}

/// The datapack tables exp and levels read, installed with [`World::set_exp_data`].
#[derive(Debug, Default)]
pub struct ExpWorld {
    experience: Arc<ExperienceTable>,
    pub(crate) experience_loss: Arc<ExperienceLoss>,
    karma_loss: Arc<KarmaLoss>,
    pub(crate) skill_trees: Arc<SkillTrees>,
    /// The vitality points and level of every player (M4-VIT-01).
    pub(crate) vitality: super::vitality::VitalityWorld,
    /// The Nevit tasks of every player (M4-VIT-01).
    pub(crate) nevit: super::nevit::NevitWorld,
    /// The recommendation tasks and the daily reset (M5-FU-03).
    pub(crate) reco: super::recommendations::RecoWorld,
}

/// What a batch of skill changes writes: `character_skills` rows (Java `storeSkill`).
#[derive(Debug, Default)]
pub(crate) struct SkillWrites {
    upserts: Vec<SkillData>,
    removed: Vec<i32>,
}

impl World {
    /// Installs `experience.xml`, `experienceLoss.xml`, `karmaLoss.xml` and the class skill trees.
    /// Call before players enter. Without them nobody gains a level.
    pub fn set_exp_data(
        &mut self,
        experience: Arc<ExperienceTable>,
        experience_loss: Arc<ExperienceLoss>,
        karma_loss: Arc<KarmaLoss>,
        skill_trees: Arc<SkillTrees>,
    ) {
        // The vitality and Nevit state of players already in the world stays.
        self.rewards.experience = experience;
        self.rewards.experience_loss = experience_loss;
        self.rewards.karma_loss = karma_loss;
        self.rewards.skill_trees = skill_trees;
    }

    /// Java `ExperienceData.getExpForLevel(level)`; `None` without a table.
    pub(crate) fn exp_for_level(&self, level: i32) -> Option<i64> {
        self.rewards.experience.exp_for_level(level)
    }

    /// Java `ExperienceData.getMaxLevel()`: one more than the highest player level, because the
    /// exp of the level above the last one ends its progress bar.
    pub(crate) fn java_max_level(&self) -> Option<i32> {
        self.rewards
            .experience
            .max_level()
            .and_then(|m| i32::try_from(m + 1).ok())
    }

    // ------------------------------------------------------------------ Attackable

    /// Java `Attackable.getMainDamageDealer()`: the attacker who did the most damage and stands
    /// within `AltPartyRange`. Ties go to the first in the aggro list (Java's list is a hash map,
    /// so its tie order is arbitrary).
    pub(crate) fn get_main_damage_dealer(&self, npc: ObjectId) -> Option<ObjectId> {
        let at = self.position_of(npc)?;
        let mut damage = 0;
        let mut dealer = None;
        for info in self.aggro_list(npc) {
            if info.damage > damage
                && self
                    .position_of(info.attacker)
                    .is_some_and(|p| distance_3d(at, p) < f64::from(self.settings.exp.alt_party_range))
            {
                damage = info.damage;
                dealer = Some(info.attacker);
            }
        }
        dealer
    }

    /// Java `Npc.getExpReward(level)`: `(long) (template exp * RATE_XP)`.
    pub(crate) fn npc_exp_reward(&self, npc: ObjectId) -> i64 {
        let exp = self.npc_template_exp_sp(npc).0;
        (exp * f64::from(self.settings.exp.rate_xp)) as i64
    }

    /// Java `Npc.getSpReward(level)`: `(int) (template sp * RATE_SP)`.
    fn npc_sp_reward(&self, npc: ObjectId) -> i32 {
        let sp = self.npc_template_exp_sp(npc).1;
        (sp * f64::from(self.settings.exp.rate_sp)) as i32
    }

    fn npc_template_exp_sp(&self, npc: ObjectId) -> (f64, f64) {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map_or((0.0, 0.0), |t| (t.exp, t.sp))
    }

    /// Java `Attackable.calculateExpAndSp(charLevel, damage, totalDamage)`: the exp and SP of a
    /// damage share, by the level difference.
    fn calculate_exp_and_sp(
        &self,
        npc: ObjectId,
        char_level: i32,
        damage: i64,
        total_damage: i64,
    ) -> (f64, f64) {
        let npc_level = self.stats(npc).map_or(0, |s| s.level());
        let level_diff = char_level - npc_level;
        let max_diff = self.settings.exp.monster_exp_max_level_difference;
        let (mut xp, mut sp) = (0.0, 0.0);
        if level_diff < max_diff && level_diff > -max_diff {
            // NOTE(java-quirk): `(getExpReward(charLevel) * damage) / totalDamage` is a `long`
            // division, so the share is cut to whole points before it becomes a double.
            let share = |reward: i64| {
                reward
                    .wrapping_mul(damage)
                    .checked_div(total_damage)
                    .unwrap_or(0)
                    .max(0)
            };
            xp = share(self.npc_exp_reward(npc)) as f64;
            sp = share(i64::from(self.npc_sp_reward(npc))) as f64;

            if char_level > 84 && level_diff <= -3 {
                let mul = match level_diff {
                    -3 => 0.97,
                    -4 => 0.67,
                    -5 => 0.42,
                    -6 => 0.25,
                    -7 => 0.15,
                    -8 => 0.09,
                    -9 => 0.05,
                    -10 => 0.03,
                    _ => 1.0,
                };
                xp *= mul;
                sp *= mul;
            }
        }
        (xp, sp)
    }

    /// The exp and SP half of Java `Attackable.calculateRewards(lastAttacker)`, for every player
    /// in the aggro list. Called by `calculate_rewards` (`death.rs`) after the drops.
    pub(crate) fn reward_exp_and_sp(&mut self, dying: ObjectId) {
        if !self.is_attackable_npc(dying) {
            return;
        }
        // `if (!getMustRewardExpSP()) return;` (`OpenChest` unlocked the chest: no exp).
        if self.npcs.get(&dying).is_some_and(|n| !n.must_reward_exp_sp) {
            return;
        }
        let Some(at) = self.position_of(dying) else {
            return;
        };
        let range = f64::from(self.settings.exp.alt_party_range);

        // Go through the aggro list of the Attackable: the damage of every player in range.
        // Java keeps them in a hash map by player, so the order of the rewards below is
        // arbitrary there; here it is the order of the aggro list.
        let mut total_damage: i64 = 0;
        let mut rewards: Vec<(ObjectId, i64)> = Vec::new();
        for info in self.aggro_list(dying) {
            // `info.getAttacker().asPlayer()`: a summon counts for its owner; anything else is
            // skipped.
            let Some(attacker) = self.acting_player(info.attacker) else {
                continue;
            };
            // Prevent unwanted behavior.
            if info.damage > 1 {
                // Check if damage dealer isn't too far from this (killed monster).
                let far = self
                    .position_of(attacker)
                    .is_none_or(|p| distance_3d(at, p) > range);
                if far {
                    continue;
                }
                total_damage = total_damage.wrapping_add(info.damage);
                // Calculate real damages (Summoners should get own damage plus summon's damage).
                match rewards.iter_mut().find(|(who, _)| *who == attacker) {
                    Some((_, damage)) => *damage = damage.wrapping_add(info.damage),
                    None => rewards.push((attacker, info.damage)),
                }
            }
        }

        // `getMustRewardExpSP()` is true unless a script switched it off (D-05).
        // Java walks `rewards.values()` while a party's pass removes its members from the map, so
        // a member who was already rewarded with its party is not visited again.
        let order = rewards.clone();
        for (attacker, damage) in order {
            if !rewards.iter().any(|(who, _)| *who == attacker) {
                continue;
            }
            // If there's a party in progress: share with its members.
            if let Some(party) = self.party_of(attacker) {
                self.reward_party_share(dying, party, at, total_damage, &mut rewards);
                continue;
            }
            // Penalty applied to the attacker's XP: "If this attacker have servitor, get Exp
            // Penalty applied for the servitor."
            let penalty = self.servitor_exp_multiplier(attacker);
            if !self.is_in_surrounding_region(dying, attacker) {
                continue;
            }
            // Calculate the difference of level between this attacker and the Attackable.
            let level = self.core_of(attacker).map_or(1, |c| c.level);
            let (exp, sp) = self.calculate_exp_and_sp(dying, level, damage, total_damage);
            // Champions: see the module comment.
            // `exp *= penalty` (a float widened to double).
            let exp = exp * f64::from(penalty);
            // The over-hit bonus: see the module comment.

            // Distribute the Exp and SP between the Player and its Summon.
            if self.is_dead(attacker) {
                continue;
            }
            let Some(view) = self.stats(attacker) else {
                continue;
            };
            let add_exp = java_round(view.calc_stat(Stat::ExpspRate, exp, None, None));
            let add_sp = view.calc_stat(Stat::ExpspRate, sp, None, None) as i32;
            // Premium rates: `PremiumSystemEnabled` is off by default.
            // NOTE(java-quirk): `useVitalityRate()` is the `useBonuses` flag, so a raid boss kill
            // (`RaidbossUseVitality = False` in the shipped config) gets no bonus of any kind,
            // not even the `BONUS_EXP` skills.
            let use_vitality = self.use_vitality_rate(dying);
            self.add_exp_and_sp(attacker, add_exp as f64, f64::from(add_sp), use_vitality);
            if add_exp > 0 && use_vitality {
                let points = self.kill_vitality_points(dying, level, damage);
                self.update_vitality_points(attacker, points, true, false);
                self.give_pc_cafe_point(attacker, exp);
            }
        }
    }

    /// The party branch of `Attackable.calculateRewards` for the party of one attacker: the
    /// members in range are rewarded together by `Party.distributeXpAndSp`, and their entries
    /// leave `rewards`.
    fn reward_party_share(
        &mut self,
        dying: ObjectId,
        party: crate::party::PartyId,
        at: (i32, i32, i32),
        total_damage: i64,
        rewards: &mut Vec<(ObjectId, i64)>,
    ) {
        let range = f64::from(self.settings.exp.alt_party_range);
        // Share with party members.
        let mut party_damage: i64 = 0;
        let mut party_level = 0;
        // Get all Creature that can be rewarded in the party.
        let mut rewarded_members: Vec<ObjectId> = Vec::new();
        // Go through all Player in the party, or in its command channel.
        let channel = self.command_channel_of_party(party);
        let group = match channel {
            Some(channel) => self.command_channel_members(channel),
            None => self.party_members(party),
        };
        for member in group {
            if self.is_dead(member) {
                continue;
            }
            let near = self
                .position_of(member)
                .is_some_and(|p| distance_3d(at, p) < range);
            let level = self.core_of(member).map_or(0, |c| c.level);
            // NOTE(java-quirk): a member's own level decides whether the level is raised, but the
            // value taken is the command channel's level.
            let raised = match channel {
                Some(channel) if level > party_level => self.command_channel_level(channel),
                _ if level > party_level => level,
                _ => party_level,
            };
            // Get the RewardInfo of this Player from the Attackable rewards.
            if let Some(index) = rewards.iter().position(|(who, _)| *who == member) {
                // If the Player is in the Attackable rewards add its damages to party damages.
                if near {
                    party_damage = party_damage.wrapping_add(rewards[index].1);
                    rewarded_members.push(member);
                    party_level = raised;
                }
                // Remove the Player from the Attackable rewards.
                rewards.remove(index);
            } else if near {
                rewarded_members.push(member);
                party_level = raised;
            }
        }

        // If the party didn't killed this Attackable alone.
        let party_mul = if party_damage < total_damage {
            party_damage as f64 / total_damage as f64
        } else {
            1.0
        };
        // Calculate Exp and SP rewards.
        // NOTE(java-quirk): `calculateExpAndSp` already takes `partyDmg / totalDamage` of the
        // reward, and `partyMul` takes the same share once more.
        let (mut exp, mut sp) = self.calculate_exp_and_sp(dying, party_level, party_damage, total_damage);
        // Champions and the over-hit bonus: see the module comment.
        exp *= party_mul;
        sp *= party_mul;
        // Distribute Experience and SP rewards to Player Party members in the known area of the
        // last attacker.
        if party_damage > 0 {
            self.party_distribute_xp_and_sp(exp, sp, &rewarded_members, party_level, party_damage, dying);
        }
    }

    // ------------------------------------------------------------------ PlayerStat

    /// Java `AccessLevel.canGainExp()` of the player.
    fn can_gain_exp(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| self.data.access_level_can_gain_exp(p.data.access_level))
    }

    /// Java `PlayerStat.getExpBonusMultiplier()` / `getSpBonusMultiplier()`: 1 plus what the
    /// vitality level, the Nevit hourglass and the `BONUS_EXP` / `BONUS_SP` stat add above 1,
    /// capped by `MaxExpBonus` / `MaxSpBonus`.
    fn exp_sp_bonus_multiplier(&self, id: ObjectId, stat: Stat, max_bonus: f64) -> f64 {
        let mut bonus = 1.0;
        // Bonus from Vitality System.
        let vitality = self.vitality_multiplier(id);
        // Bonus from Nevit's Blessing.
        let nevits = self.nevit_hourglass_multiplier(id);
        // Bonus from Nevit's Hunting: a `TODO` in Java.
        // Bonus exp from skills.
        let skill_bonus = 1.0 + self.stats(id).map_or(0.0, |v| v.calc_stat(stat, 0.0, None, None)) / 100.0;
        if vitality > 1.0 {
            bonus += vitality - 1.0;
        }
        if nevits > 1.0 {
            bonus += nevits - 1.0;
        }
        if skill_bonus > 1.0 {
            bonus += skill_bonus - 1.0;
        }
        // Check for abnormal bonuses.
        bonus = bonus.max(1.0);
        if max_bonus > 0.0 {
            bonus = bonus.min(max_bonus);
        }
        bonus
    }

    /// Java `PlayerStat.addExpAndSp(addToExp, addToSp, useBonuses)`: the bonuses, the exp, the SP
    /// and the message about them. `false` when the access level may not gain exp.
    pub fn add_exp_and_sp(
        &mut self,
        id: ObjectId,
        add_to_exp: f64,
        add_to_sp: f64,
        use_bonuses: bool,
    ) -> bool {
        // Allowed to gain exp/sp?
        if !self.can_gain_exp(id) {
            return false;
        }
        let base_exp = add_to_exp;
        let base_sp = add_to_sp;
        let (mut bonus_exp, mut bonus_sp) = (1.0, 1.0);
        // Start Nevit's Hourglass.
        self.nevit_on_exp_gain_start_hourglass(id, add_to_exp);
        if use_bonuses {
            bonus_exp = self.exp_sp_bonus_multiplier(id, Stat::BonusExp, self.settings.exp.max_bonus_exp);
            bonus_sp = self.exp_sp_bonus_multiplier(id, Stat::BonusSp, self.settings.exp.max_bonus_sp);
        }
        self.nevit_on_exp_gain_start_advent(id, add_to_exp);
        let add_to_exp = add_to_exp * bonus_exp;
        let add_to_sp = add_to_sp * bonus_sp;
        // If this player has a pet and it is in his range he takes from the owner's Exp, give the
        // pet Exp now.
        let (add_to_exp, add_to_sp) = self.pet_take_owner_share(id, add_to_exp, add_to_sp);

        let final_exp = java_round(add_to_exp);
        let final_sp = java_round(add_to_sp);
        let exp_added = self.add_exp(id, final_exp);
        let sp_added = self.add_sp(id, final_sp);
        if !exp_added && sp_added {
            let sm = SystemMessage::new(SystemMessageId::YouHaveAcquiredS1Sp)
                .with(SystemMessageParam::LongNumber(final_sp));
            self.send(id, &sm);
        } else if exp_added && !sp_added {
            let sm = SystemMessage::new(SystemMessageId::YouHaveEarnedS1Experience)
                .with(SystemMessageParam::LongNumber(final_exp));
            self.send(id, &sm);
        } else if final_exp > 0 || final_sp > 0 {
            let sm = if add_to_exp - base_exp > 0.0 {
                SystemMessage::new(SystemMessageId::YouHaveAcquiredS1ExpBonusS2AndS3SpBonusS4)
                    .with(SystemMessageParam::LongNumber(final_exp))
                    .with(SystemMessageParam::LongNumber(java_round(add_to_exp - base_exp)))
                    .with(SystemMessageParam::LongNumber(final_sp))
                    .with(SystemMessageParam::LongNumber(java_round(add_to_sp - base_sp)))
            } else {
                // NOTE(java-quirk): this message shows the truncated value, the one above the
                // rounded one.
                SystemMessage::new(SystemMessageId::YouHaveEarnedS1ExperienceAndS2Sp)
                    .with(SystemMessageParam::LongNumber(add_to_exp as i64))
                    .with(SystemMessageParam::LongNumber(add_to_sp as i64))
            };
            self.send(id, &sm);
        }
        true
    }

    /// Java `PlayerStat.addExp(value)`. `false` when the player may not gain exp.
    pub fn add_exp(&mut self, id: ObjectId, value: i64) -> bool {
        // Allowed to gain exp?
        if !self.can_gain_exp(id) {
            return false;
        }
        if !self.playable_add_exp(id, value) {
            return false;
        }
        // Set new karma.
        let karma = self.players.get(&id).map_or(0, |p| p.data.karma);
        if !self.is_cursed_weapon_equipped(id) && karma > 0 && !self.is_inside_zone(id, ZoneId::Pvp) {
            let karma_lost = self.calculate_karma_lost(id, value);
            if karma_lost > 0 {
                self.set_karma(id, karma.wrapping_sub(karma_lost));
                let now = self.players.get(&id).map_or(0, |p| p.data.karma);
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YourKarmaHasBeenChangedToS1)
                        .with(SystemMessageParam::IntNumber(now)),
                );
            }
        }
        // EXP status update currently not used in retail.
        self.send_user_info(id);
        true
    }

    /// Java `Formulas.calculateKarmaLost(player, finalExp)`.
    fn calculate_karma_lost(&self, id: ObjectId, final_exp: i64) -> i32 {
        let level = self.core_of(id).map_or(1, |c| c.level);
        let karma_loss_mod = self.rewards.karma_loss.modifier(level);
        if final_exp > 0 {
            // Received experience. `finalExp / RATE_KARMA_LOST` is a float division.
            let scaled = (final_exp as f32 / self.settings.exp.rate_karma_lost).abs();
            return ((f64::from(scaled) / karma_loss_mod) / 30.0) as i32;
        }
        ((final_exp.wrapping_abs() as f64 / karma_loss_mod) / 30.0) as i32
    }

    /// Java `PlayerStat.addSp(value)`: the SP, then a `StatusUpdate` with the new total.
    pub fn add_sp(&mut self, id: ObjectId, value: i64) -> bool {
        if !self.playable_add_sp(id, value) {
            return false;
        }
        let sp = self.players.get(&id).map_or(0, |p| p.data.sp);
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(attr::SP, sp as i32);
        self.send(id, &update);
        true
    }

    /// Java `PlayableStat.addSp(amount)`: capped at `MaxSp`.
    fn playable_add_sp(&mut self, id: ObjectId, amount: i64) -> bool {
        if amount < 0 {
            warn!("wrong usage");
            return false;
        }
        let max_sp = self.settings.exp.max_sp;
        let Some(player) = self.players.get_mut(&id) else {
            return false;
        };
        let current = player.data.sp;
        if current >= max_sp {
            return false;
        }
        let mut value = amount;
        if current > max_sp.wrapping_sub(value) {
            value = max_sp.wrapping_sub(current);
        }
        player.data.sp = current.wrapping_add(value);
        true
    }

    // ------------------------------------------------------------------ PlayableStat

    /// Java `PlayableStat.addExp(amount)`: the exp (never past the last level), then the level it
    /// makes. `false` only without an experience table.
    fn playable_add_exp(&mut self, id: ObjectId, amount: i64) -> bool {
        // ON_PLAYABLE_EXP_CHANGED: the `SoulEating` listeners hear the gain first.
        self.soul_eating_on_exp_changed(id, amount);
        let (Some(max_level), Some(player)) = (self.java_max_level(), self.players.get(&id)) else {
            return false;
        };
        let Some(cap) = self.exp_for_level(max_level) else {
            return false;
        };
        let exp = player.data.exp;
        if exp.wrapping_add(amount) < 0 || (amount > 0 && exp == cap - 1) {
            return true;
        }
        let mut value = amount;
        if exp.wrapping_add(value) >= cap {
            value = cap - 1 - exp;
        }
        self.set_exp(id, exp.wrapping_add(value));
        self.adjust_level_to_exp(id);
        true
    }

    /// Java `PlayableStat.removeExp(amount)`: never below 1 exp, then the level it leaves.
    pub fn remove_exp(&mut self, id: ObjectId, amount: i64) -> bool {
        let Some(exp) = self.players.get(&id).map(|p| p.data.exp) else {
            return false;
        };
        if self.java_max_level().is_none() {
            return false;
        }
        let mut value = amount;
        if exp.wrapping_sub(value) < 0 {
            value = exp - 1;
        }
        self.set_exp(id, exp.wrapping_sub(value));
        self.adjust_level_to_exp(id);
        true
    }

    /// Java `PlayableStat.removeSp(amount)`: the SP minus the amount, never below 0. Java's
    /// `setSp` (`CreatureStat`) is not capped, so this only ever lowers the SP.
    pub fn remove_sp(&mut self, id: ObjectId, amount: i64) -> bool {
        let Some(player) = self.players.get_mut(&id) else {
            return false;
        };
        let sp = player.data.sp;
        player.data.sp = if sp < amount { 0 } else { sp.wrapping_sub(amount) };
        true
    }

    /// Java `PlayerStat.removeExpAndSp(removeExp, removeSp, sendMessage)`: the exp and the SP
    /// (`PlayableStat.removeExpAndSp`, each only when above 0). With `send_message`, the two
    /// messages of the amounts asked for, and a status update when the level went down. `false`
    /// when nothing was removed (then nothing is said).
    pub fn remove_exp_and_sp(
        &mut self,
        id: ObjectId,
        remove_exp: i64,
        remove_sp: i64,
        send_message: bool,
    ) -> bool {
        let level_before = self.core_of(id).map_or(0, |c| c.level);
        let mut exp_removed = false;
        let mut sp_removed = false;
        if remove_exp > 0 {
            exp_removed = self.remove_exp(id, remove_exp);
        }
        if remove_sp > 0 {
            sp_removed = self.remove_sp(id, remove_sp);
        }
        if !(exp_removed || sp_removed) {
            return false;
        }

        if send_message {
            // Java: `YOUR_EXPERIENCE_HAS_DECREASED_BY_S1` and `YOUR_SP_HAS_DECREASED_BY_S1`, both
            // sent even when one of the two amounts is 0.
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourExperienceHasDecreasedByS1)
                    .with(SystemMessageParam::LongNumber(remove_exp)),
            );
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourSpHasDecreasedByS1)
                    .with(SystemMessageParam::LongNumber(remove_sp)),
            );
            if self.core_of(id).map_or(0, |c| c.level) < level_before {
                self.broadcast_status_update(id);
            }
        }
        true
    }

    /// Java `ExperienceData.getMaxLevel()` (the table's last level plus one), for the admin
    /// commands that read it.
    pub(crate) fn exp_table_max_level(&self) -> Option<i32> {
        self.java_max_level()
    }

    /// Java `ExperienceData.getExpForLevel(level)`, for the admin commands.
    pub(crate) fn exp_needed_for_level(&self, level: i32) -> Option<i64> {
        self.exp_for_level(level)
    }

    /// Java `ExperienceData.getMaxPetLevel()`: one more than the highest pet level.
    pub(crate) fn exp_table_max_pet_level(&self) -> Option<i32> {
        self.rewards
            .experience
            .max_pet_level()
            .and_then(|m| i32::try_from(m + 1).ok())
    }

    fn set_exp(&mut self, id: ObjectId, value: i64) {
        if let Some(player) = self.players.get_mut(&id) {
            player.data.exp = value;
        }
    }

    /// The level loop of `PlayableStat.addExp` / `removeExp`: the highest level whose exp the
    /// player has, then `addLevel(difference)`.
    fn adjust_level_to_exp(&mut self, id: ObjectId) {
        let (Some(max_level), Some(player)) = (self.java_max_level(), self.players.get(&id)) else {
            return;
        };
        let exp = player.data.exp;
        let current = player.core.level;
        // NOTE(java-quirk): when no level's exp is above the player's, Java keeps the minimum
        // level 1 (the loop never sets another).
        let mut level = 1;
        for tmp in 1..=max_level {
            if self.exp_for_level(tmp).is_some_and(|needed| exp >= needed) {
                continue;
            }
            level = tmp - 1;
            break;
        }
        if level != current && level >= 1 {
            self.add_level(id, level - current);
        }
    }

    /// Java `PlayableStat.addLevel(amount)`: the new level, the exp that goes with it, and the
    /// full HP and MP of a level gained. `true` when the level went up.
    fn playable_add_level(&mut self, id: ObjectId, amount: i32) -> bool {
        let (Some(max_level), Some(level)) = (self.java_max_level(), self.core_of(id).map(|c| c.level))
        else {
            return false;
        };
        let mut value = amount;
        if level + value > max_level - 1 {
            if level < max_level - 1 {
                value = max_level - 1 - level;
            } else {
                return false;
            }
        }
        let level_increased = level + value > level;
        self.set_level(id, level + value);

        // Sync up exp with current level.
        let new_level = level + value;
        let exp = self.players.get(&id).map_or(0, |p| p.data.exp);
        if let (Some(this), Some(next)) = (self.exp_for_level(new_level), self.exp_for_level(new_level + 1))
            && (exp >= next || this > exp)
        {
            self.set_exp(id, this);
        }

        // The maximum HP, MP and CP follow the level (Java's "stats watch" in `getMaxHp()`, which
        // the packets below and `setCurrentHp(getMaxHp())` call), up or down.
        self.refresh_max_vitals(id, false);

        if !level_increased && !self.is_gm(id) && self.settings.exp.decrease_skill_level {
            let mut writes = SkillWrites::default();
            self.check_player_skills(id, &mut writes);
            self.flush_skill_writes(id, writes);
        }
        if !level_increased {
            return false;
        }

        let (max_hp, max_mp) = self
            .stats(id)
            .map_or((0.0, 0.0), |v| (f64::from(v.max_hp()), f64::from(v.max_mp())));
        self.set_current_hp(id, max_hp, true);
        self.set_current_mp(id, max_mp, true);
        true
    }

    /// Java `PlayerStat.setLevel(value)`: capped at the last level.
    pub(crate) fn set_level(&mut self, id: ObjectId, value: i32) {
        let Some(max_level) = self.java_max_level() else {
            return;
        };
        let level = value.min(max_level - 1);
        if let Some(player) = self.players.get_mut(&id) {
            player.core.level = level;
            player.data.level = level;
        }
    }

    /// Java `PlayerStat.addLevel(amount)`: the level, then everything a new level changes for the
    /// player and the ones who watch it. `true` when the level went up.
    pub fn add_level(&mut self, id: ObjectId, amount: i32) -> bool {
        let (Some(max_level), Some(level)) = (self.java_max_level(), self.core_of(id).map(|c| c.level))
        else {
            return false;
        };
        if level + amount > max_level - 1 {
            return false;
        }
        // ON_PLAYER_LEVEL_CHANGED: script event (D-05).
        let level_increased = self.playable_add_level(id, amount);
        if level_increased {
            // The tutorial quest event "CE40" is a script (D-05).
            let max_cp = self.stats(id).map_or(0.0, |v| f64::from(v.max_cp()));
            self.set_current_cp(id, max_cp, true);
            self.broadcast_creature_packet(
                id,
                &SocialAction {
                    object_id: id.0,
                    action_id: LEVEL_UP_ACTION,
                },
            );
            self.send(id, &SystemMessage::new(SystemMessageId::YourLevelHasIncreased));
        }

        // Give AutoGet skills and all normal skills if Auto-Learn is activated.
        self.reward_skills(id);

        // Transformations: the additional skills of the new level (`Transform.onLevelUp`).
        self.transformation_level_up(id);
        // The party level.
        self.recalculate_party_level_of(id);
        // Synchronize level with pet if possible.
        self.pet_sync_level_with_owner(id);
        // Clan member list: not modelled.
        let (level, max_cp, max_hp, max_mp) = match (self.core_of(id), self.stats(id)) {
            (Some(core), Some(view)) => (core.level, view.max_cp(), view.max_hp(), view.max_mp()),
            _ => return level_increased,
        };
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(attr::LEVEL, level);
        update.add_attribute(attr::MAX_CP, max_cp);
        update.add_attribute(attr::MAX_HP, max_hp);
        update.add_attribute(attr::MAX_MP, max_mp);
        self.send(id, &update);

        // Update the overloaded status of the Player.
        self.refresh_weight(id);
        // Update the expertise status of the Player.
        self.refresh_expertise(id);
        // Send a Server->Client packet UserInfo to the Player.
        self.send_user_info(id);

        // Nevit System.
        if self.settings.nevit_enabled {
            self.send_vote_system_info(id);
            self.nevit_add_points(id, super::nevit::LEVEL_UP_POINTS);
        }
        level_increased
    }

    // ------------------------------------------------------------------ skills

    /// Java `Player.rewardSkills()`: the skills a level brings, the ones it takes away, the item
    /// restrictions, and the new skill list.
    pub(crate) fn reward_skills(&mut self, id: ObjectId) {
        let mut writes = SkillWrites::default();
        // Give all normal skills if activated Auto-Learn is activated, included AutoGet skills.
        if self.settings.exp.auto_learn_skills {
            self.give_available_skills(
                id,
                self.settings.learn.auto_learn_fs_skills,
                true,
                self.settings.learn.auto_learn_skills_without_items,
                &mut writes,
            );
        } else {
            self.give_available_auto_get_skills(id, &mut writes);
        }

        if self.settings.exp.decrease_skill_level && !self.is_gm(id) {
            self.check_player_skills(id, &mut writes);
        }
        self.give_free_mount_skills(id);
        // The `RebirthConfig` skills: a Custom config, off by default.

        self.flush_skill_writes(id, writes);

        let mut changed = Vec::new();
        self.check_item_restriction(id, &mut changed);
        self.send_skill_list(id);
    }

    /// Java `Player.giveAvailableAutoGetSkills()`: every auto-get skill of the class tree whose
    /// level the player reached and does not know in that level yet, with `addSkill(skill, true)`.
    fn give_available_auto_get_skills(&mut self, id: ObjectId, writes: &mut SkillWrites) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let level = player.core.level;
        let Some(class) = u16::try_from(player.data.class_id).ok().map(ClassId) else {
            return;
        };
        // The list is made first, against the skills known now, like Java's.
        // NOTE: Java walks the tree in hash order; a skill that is listed in two levels at once
        // ends at the last one added, here the highest.
        let mut learn: Vec<(i32, i32)> = Vec::new();
        let mut empty = true;
        for skill in self.rewards.skill_trees.complete_class_tree(class) {
            empty = false;
            if !(skill.auto_get && level >= skill.get_level) {
                continue;
            }
            let known = player
                .core
                .skills
                .iter()
                .find(|(skill_id, _)| *skill_id == skill.skill_id)
                .map(|(_, l)| *l);
            if known.is_none_or(|old| old < skill.skill_level) {
                learn.push((skill.skill_id, skill.skill_level));
            }
        }
        if empty {
            // The Skill Tree for this class is undefined, so we return an empty list.
            warn!(
                "SkillTreeData: Skill Tree for this class Id({}) is not defined!",
                class.0
            );
        }
        for (skill_id, skill_level) in learn {
            if !self.learn_skill(id, skill_id, skill_level, writes) {
                warn!("Skipping null auto-get skill for {id}");
            }
        }
    }

    /// Java `Player.addSkill(skill, true)`: the skill joins the player and the next store writes
    /// it (`writes` is the immediate `storeSkill`). `false` when no such skill exists.
    pub(crate) fn learn_skill(
        &mut self,
        id: ObjectId,
        skill_id: i32,
        level: i32,
        writes: &mut SkillWrites,
    ) -> bool {
        if !self.add_skill(id, skill_id, level) {
            return false;
        }
        if let Some(player) = self.players.get_mut(&id) {
            match player.data.skills.iter_mut().find(|s| s.skill_id == skill_id) {
                Some(entry) => entry.skill_level = level,
                None => player.data.skills.push(SkillData {
                    skill_id,
                    skill_level: level,
                }),
            }
        }
        writes.removed.retain(|removed| *removed != skill_id);
        match writes.upserts.iter_mut().find(|s| s.skill_id == skill_id) {
            Some(entry) => entry.skill_level = level,
            None => writes.upserts.push(SkillData {
                skill_id,
                skill_level: level,
            }),
        }
        true
    }

    /// Java `Player.removeSkill(skill, true)`: the skill goes, its row is deleted.
    // NOTE(java-quirk): the shortcuts of the skill go whether or not the skill was known
    // (`Player.removeSkill` deletes them for any non-null `Skill`); there is no `ShortCutDelete`
    // packet, `deleteShortcut` resends the list.
    fn forget_skill(&mut self, id: ObjectId, skill_id: i32, writes: &mut SkillWrites) {
        let removed = self.remove_skill(id, skill_id, true);
        self.delete_skill_shortcuts(id, skill_id);
        if !removed {
            return;
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.data.skills.retain(|s| s.skill_id != skill_id);
        }
        writes.upserts.retain(|s| s.skill_id != skill_id);
        if !writes.removed.contains(&skill_id) {
            writes.removed.push(skill_id);
        }
    }

    /// Hands the rows of a batch of skill changes to the persistence task.
    pub(crate) fn flush_skill_writes(&mut self, id: ObjectId, writes: SkillWrites) {
        if writes.upserts.is_empty() && writes.removed.is_empty() {
            return;
        }
        self.outbox.persist(PersistCommand::SkillChanges {
            owner: id,
            upserts: writes.upserts,
            removed: writes.removed,
        });
    }

    /// Java `Player.checkPlayerSkills()`: a skill whose level needs more than the player has (9
    /// levels of grace, none for Expertise) goes down to the highest level he may have, or goes.
    fn check_player_skills(&mut self, id: ObjectId, writes: &mut SkillWrites) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let level = player.core.level;
        let Some(class) = u16::try_from(player.data.class_id).ok().map(ClassId) else {
            return;
        };
        let known = player.core.skills.clone();
        for (skill_id, skill_level) in known {
            // `getClassSkill(id, level % 100, class)`: the tree line of that skill level.
            let learn = self
                .rewards
                .skill_trees
                .complete_class_tree(class)
                .find(|s| s.skill_id == skill_id && s.skill_level == skill_level % 100)
                .map(|s| s.get_level);
            let Some(get_level) = learn else {
                continue;
            };
            let level_diff = if skill_id == EXPERTISE_SKILL_ID { 0 } else { 9 };
            if level < get_level - level_diff {
                self.decrease_skill_level(id, skill_id, class, level_diff, writes);
            }
        }
    }

    /// Java `Player.deacreaseSkillLevel(skill, levelDiff)`.
    fn decrease_skill_level(
        &mut self,
        id: ObjectId,
        skill_id: i32,
        class: ClassId,
        level_diff: i32,
        writes: &mut SkillWrites,
    ) {
        let level = self.core_of(id).map_or(1, |c| c.level);
        let mut next_level = -1;
        for sl in self.rewards.skill_trees.complete_class_tree(class) {
            if sl.skill_id == skill_id && next_level < sl.skill_level && level >= sl.get_level - level_diff {
                // Next possible skill level.
                next_level = sl.skill_level;
            }
        }
        if next_level == -1 {
            info!("Removing skill {skill_id} from {id}");
            // There is no lower skill.
            self.forget_skill(id, skill_id, writes);
        } else {
            info!("Decreasing skill {skill_id} to {next_level} for {id}");
            // Replace with lower one.
            if !self.learn_skill(id, skill_id, next_level, writes) {
                debug!("No skill {skill_id} level {next_level}.");
            }
        }
    }

    // ------------------------------------------------------------------ death

    /// Java `Player.isLucky()`: below level 10 and under the passive skill Lucky.
    pub(crate) fn is_lucky(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.core.level <= 9) && self.has_passive(id, LUCKY_SKILL_ID)
    }

    /// Java `Player.setExpBeforeDeath(exp)`.
    pub(crate) fn set_exp_before_death(&mut self, id: ObjectId, exp: i64) {
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.exp_before_death = exp;
        }
    }

    /// Java `Player.calculateDeathExpPenalty(killer, atWar)`, with the conditions `Player.doDie`
    /// puts around the call (`Delevel`, Lucky, Nevit's advent blessing, events). The zone check is
    /// the caller's.
    pub(crate) fn player_calculate_death_exp_penalty(&mut self, id: ObjectId, killer: ObjectId) {
        // `!_nevitSystem.isAdventBlessingActive()` is part of the same `doDie` condition; inside
        // `calculateDeathExpPenalty` Java sets the loss to 0 for it as well, which comes to the
        // same.
        if !self.settings.exp.player_delevel || self.is_lucky(id) || self.is_advent_blessing_active(id) {
            return;
        }
        // Events: not modelled. `atWar` needs clans (M5).
        let (Some(max_level), Some(player)) = (self.java_max_level(), self.players.get(&id)) else {
            return;
        };
        let level = player.core.level;
        let karma = player.data.karma;
        let exp = player.data.exp;
        let mut percent_lost = self.rewards.experience_loss.percent_lost(level);

        // Calculate reduce exp lost stats.
        let reduce = |stat| self.stats(id).map_or(1.0, |v| v.calc_stat(stat, 1.0, None, None));
        if self.stats(killer).is_some_and(|k| k.is_raid()) {
            percent_lost *= reduce(Stat::ReduceExpLostByRaid);
        } else if self.is_monster(killer) {
            percent_lost *= reduce(Stat::ReduceExpLostByMob);
        } else if self.is_player(killer) {
            percent_lost *= reduce(Stat::ReduceExpLostByPvp);
        }

        // Calculate karma loss configuration.
        if karma > 0 {
            percent_lost *= f64::from(self.settings.exp.rate_karma_exp_lost);
        }

        // Get the total exp for the current player level.
        let level_exp = |l: i32| self.exp_for_level(l).unwrap_or(0);
        let current_level_exp = if level < max_level {
            level_exp(level + 1) - level_exp(level)
        } else {
            level_exp(max_level) - level_exp(max_level - 1)
        };

        // Calculate the experience loss.
        let mut lost_exp = 0;
        if current_level_exp > 0 {
            lost_exp = java_round((current_level_exp as f64 * percent_lost) / 100.0);
            // Capping loss to 10% of current level experience.
            lost_exp = lost_exp.min(current_level_exp / 10);
            // Reduced for festival and war kills (`isAtWarWith(pk)`), none for the advent
            // blessing: the festival is not modelled.
            if self.is_at_war_with(id, killer) {
                lost_exp /= 4;
            }
        }

        self.set_exp_before_death(id, exp);
        self.remove_exp(id, lost_exp);
    }

    /// Java `Player.restoreExp(restorePercent)`: gives back that share of the exp lost at death.
    pub(crate) fn player_restore_exp(&mut self, id: ObjectId, percent: f64) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let before = player.data.profile.exp_before_death;
        if before > 0 {
            // Restore the specified % of lost experience.
            let amount = java_round(((before - player.data.exp) as f64 * percent) / 100.0);
            self.add_exp(id, amount);
            self.set_exp_before_death(id, 0);
        }
    }
}
