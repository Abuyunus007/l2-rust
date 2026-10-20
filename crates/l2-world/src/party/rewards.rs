//! Sharing the exp and SP of a kill: the party bonus, which members count, and the level cut-offs.
// Java: entity/groups/Party.java (distributeXpAndSp, calculateExpSpPartyCutoff, getValidMembers,
//       getExpBonus, getSpBonus, getBaseExpSpBonus)
// Java: entity/groups/PartyExpType.java
// Java: entity/actor/Attackable.java (calculateRewards: the party branch)

use l2_data::Stat;
use l2_util::ObjectId;

use super::base_exp_sp_bonus;
use crate::stats::java_round;
use crate::world::World;

/// Java `PartyExpType`: who of a party is cut from the exp of a kill (`PartyXpCutoffMethod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyExpType {
    None,
    Auto,
    Level,
    Percentage,
    HighFive,
}

impl PartyExpType {
    /// `Enum.valueOf(PartyExpType.class, name.toUpperCase())`; `None` for a name Java would
    /// throw on.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.trim().to_ascii_uppercase().as_str() {
            "NONE" => Self::None,
            "AUTO" => Self::Auto,
            "LEVEL" => Self::Level,
            "PERCENTAGE" => Self::Percentage,
            "HIGHFIVE" => Self::HighFive,
            _ => return Option::None,
        })
    }
}

impl World {
    fn member_level(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.core.level)
    }

    fn party_instance_of(&self, id: ObjectId) -> i32 {
        self.objects.get(&id).map_or(0, |e| e.instance_id)
    }

    /// Java `Party.getValidMembers(members, topLvl, target)`: the members the cut-off method lets
    /// share the exp.
    fn party_valid_members(&self, members: &[ObjectId], top_level: i32, target: ObjectId) -> Vec<ObjectId> {
        let settings = &self.settings.party;
        let instance = self.party_instance_of(target);
        let same_instance = |m: &ObjectId| self.party_instance_of(*m) == instance;
        let level_of = |m: &ObjectId| self.member_level(*m);
        match settings.xp_cutoff_method {
            PartyExpType::Level => members
                .iter()
                .filter(|m| same_instance(m) && top_level - level_of(m) <= settings.xp_cutoff_level)
                .copied()
                .collect(),
            PartyExpType::Percentage => {
                let sq_level_sum: i32 = members
                    .iter()
                    .filter(|m| same_instance(m))
                    .map(|m| level_of(m).wrapping_mul(level_of(m)))
                    .fold(0, i32::wrapping_add);
                members
                    .iter()
                    .filter(|m| {
                        let sq_level = level_of(m).wrapping_mul(level_of(m));
                        same_instance(m)
                            && f64::from(sq_level.wrapping_mul(100))
                                >= f64::from(sq_level_sum) * settings.xp_cutoff_percent
                    })
                    .copied()
                    .collect()
            }
            PartyExpType::Auto => {
                let sq_level_sum: i32 = members
                    .iter()
                    .map(|m| level_of(m).wrapping_mul(level_of(m)))
                    .fold(0, i32::wrapping_add);
                // `int i = members.size() - 1; if (i < 1) return members;`
                if members.len() < 2 {
                    return members.to_vec();
                }
                let count = i32::try_from(members.len()).unwrap_or(i32::MAX);
                members
                    .iter()
                    .filter(|m| {
                        let sq_level = level_of(m).wrapping_mul(level_of(m));
                        same_instance(m) && sq_level >= sq_level_sum / count.wrapping_mul(count)
                    })
                    .copied()
                    .collect()
            }
            PartyExpType::HighFive | PartyExpType::None => {
                members.iter().filter(|m| same_instance(m)).copied().collect()
            }
        }
    }

    /// Java `Party.getExpBonus(membersCount)`.
    fn party_exp_bonus(&self, members: usize) -> f64 {
        if members < 2 {
            base_exp_sp_bonus(members)
        } else {
            base_exp_sp_bonus(members) * f64::from(self.settings.party.rate_party_xp)
        }
    }

    /// Java `Party.getSpBonus(membersCount)`.
    fn party_sp_bonus(&self, members: usize) -> f64 {
        if members < 2 {
            base_exp_sp_bonus(members)
        } else {
            base_exp_sp_bonus(members) * f64::from(self.settings.party.rate_party_sp)
        }
    }

    /// Java `Party.distributeXpAndSp(xp, sp, rewardedMembers, topLvl, partyDmg, target)`: the exp
    /// and SP of the party's share of a kill, split by the squares of the levels.
    pub(crate) fn party_distribute_xp_and_sp(
        &mut self,
        xp_reward_value: f64,
        sp_reward_value: f64,
        rewarded_members: &[ObjectId],
        top_level: i32,
        party_damage: i64,
        target: ObjectId,
    ) {
        let valid_members = self.party_valid_members(rewarded_members, top_level, target);
        let xp_reward = xp_reward_value * self.party_exp_bonus(valid_members.len());
        let sp_reward = sp_reward_value * self.party_sp_bonus(valid_members.len());
        let mut sq_level_sum: i32 = 0;
        let mut average_level: i32 = 0;
        for member in &valid_members {
            let level = self.member_level(*member);
            sq_level_sum = sq_level_sum.wrapping_add(level.wrapping_mul(level));
            average_level = average_level.wrapping_add(level);
        }
        average_level /= i32::try_from(valid_members.len().max(1)).unwrap_or(i32::MAX);

        // `float / int`: with no valid member this is an infinity or NaN that nobody uses.
        let vitality_points = (self.kill_vitality_points(target, average_level, party_damage)
            * self.settings.party.rate_party_xp)
            / valid_members.len() as f32;
        let use_vitality_rate = self.use_vitality_rate(target);
        for &member in rewarded_members {
            if self.is_dead(member) {
                continue;
            }
            // Calculate and add the EXP and SP reward to the member.
            if valid_members.contains(&member) {
                // M6: the servitor penalty (`getExpMultiplier()`), 1 without a summon.
                let penalty = 1.0;
                let level = f64::from(self.member_level(member));
                let sq_level = level * level;
                let pre_calculation = (sq_level / f64::from(sq_level_sum)) * penalty;

                // Add the XP/SP points to the requested party member.
                let Some(view) = self.stats(member) else {
                    continue;
                };
                let add_exp =
                    java_round(view.calc_stat(Stat::ExpspRate, xp_reward * pre_calculation, None, None))
                        as f64;
                let add_sp = view.calc_stat(Stat::ExpspRate, sp_reward * pre_calculation, None, None);
                let add_exp = self.party_exp_sp_cutoff(member, top_level, add_exp, add_sp, use_vitality_rate);
                if add_exp > 0.0 {
                    self.update_vitality_points(member, vitality_points, true, false);
                    // `PcCafePointsManager.givePcCafePoint`: no PC cafe.
                }
            } else {
                self.add_exp_and_sp(member, 0.0, 0.0, false);
            }
        }
    }

    /// Java `Party.calculateExpSpPartyCutoff(player, topLvl, addExp, addSp, vit)`: the class
    /// multipliers, then (for the `highfive` method) the percentage of the level gap. Returns the
    /// exp the vitality check looks at.
    fn party_exp_sp_cutoff(
        &mut self,
        member: ObjectId,
        top_level: i32,
        add_exp_value: f64,
        add_sp_value: f64,
        vitality: bool,
    ) -> f64 {
        let class_id = self.players.get(&member).map_or(0, |p| p.data.class_id);
        let settings = self.settings.party.clone();
        let multiplier = |table: &[f32]| {
            usize::try_from(class_id)
                .ok()
                .and_then(|c| table.get(c))
                .copied()
                .unwrap_or(1.0)
        };
        // Premium rates: `PremiumSystemEnabled` is off by default and nobody has a premium status.
        let add_exp = add_exp_value * f64::from(multiplier(&settings.exp_amount_multipliers));
        let add_sp = add_sp_value * f64::from(multiplier(&settings.sp_amount_multipliers));

        let mut xp = add_exp;
        if settings.xp_cutoff_method == PartyExpType::HighFive {
            let level_diff = top_level - self.member_level(member);
            for (i, gap) in settings.xp_cutoff_gaps.iter().enumerate() {
                if level_diff >= gap[0] && level_diff <= gap[1] {
                    // NOTE(java-quirk): a gap without a percentage entry is an
                    // `ArrayIndexOutOfBoundsException` in Java; here it counts as 0.
                    let percent = f64::from(settings.xp_cutoff_gap_percents.get(i).copied().unwrap_or(0));
                    xp = (add_exp * percent) / 100.0;
                    let sp = (add_sp * percent) / 100.0;
                    self.add_exp_and_sp(member, xp, sp, vitality);
                    break;
                }
            }
            // NOTE(java-quirk): a member whose level gap is in no range gets nothing added, but the
            // method still returns the exp before the cut (`xp = addExp`), so the vitality below
            // is used up for it.
        } else {
            self.add_exp_and_sp(member, add_exp, add_sp, vitality);
        }
        xp
    }

    /// Java `Party.recalculatePartyLevel()` for the party of `id`, if any (what
    /// `PlayerStat.addLevel` does after a level change).
    pub(crate) fn recalculate_party_level_of(&mut self, id: ObjectId) {
        if let Some(party) = self.party_of(id) {
            self.recalculate_party_level(party);
        }
    }
}
