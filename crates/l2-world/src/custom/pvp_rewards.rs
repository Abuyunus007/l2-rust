//! `PvpRewardItem.ini` and `PvpAnnounce.ini`: the item a killer gets for a PvP or PK kill and the
//! announcement the whole server hears.
// Java: entity/actor/Player.java (`doDie`), config/custom/PvpRewardItemConfig.java,
// config/custom/PvpAnnounceConfig.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::world::World;
use crate::zones::ZoneId;

/// `PvpRewardItemConfig` (`Custom/PvpRewardItem.ini`). `Default` is the Java default of each key
/// (the shipped `.ini` has the same values: both rewards are off).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvpRewardItemSettings {
    /// `REWARD_PVP_ITEM` (`RewardPvpItem`).
    pub reward_pvp_item: bool,
    /// `REWARD_PVP_ITEM_ID` (`RewardPvpItemId`).
    pub reward_pvp_item_id: i32,
    /// `REWARD_PVP_ITEM_AMOUNT` (`RewardPvpItemAmount`).
    pub reward_pvp_item_amount: i32,
    /// `REWARD_PVP_ITEM_MESSAGE` (`RewardPvpItemMessage`).
    pub reward_pvp_item_message: bool,
    /// `REWARD_PK_ITEM` (`RewardPkItem`).
    pub reward_pk_item: bool,
    /// `REWARD_PK_ITEM_ID` (`RewardPkItemId`).
    pub reward_pk_item_id: i32,
    /// `REWARD_PK_ITEM_AMOUNT` (`RewardPkItemAmount`).
    pub reward_pk_item_amount: i32,
    /// `REWARD_PK_ITEM_MESSAGE` (`RewardPkItemMessage`).
    pub reward_pk_item_message: bool,
    /// `DISABLE_REWARDS_IN_INSTANCES` (`DisableRewardsInInstances`).
    pub disable_rewards_in_instances: bool,
    /// `DISABLE_REWARDS_IN_PVP_ZONES` (`DisableRewardsInPvpZones`).
    pub disable_rewards_in_pvp_zones: bool,
}

impl Default for PvpRewardItemSettings {
    fn default() -> Self {
        Self {
            reward_pvp_item: false,
            reward_pvp_item_id: 57,
            reward_pvp_item_amount: 1000,
            reward_pvp_item_message: true,
            reward_pk_item: false,
            reward_pk_item_id: 57,
            reward_pk_item_amount: 500,
            reward_pk_item_message: true,
            disable_rewards_in_instances: true,
            disable_rewards_in_pvp_zones: true,
        }
    }
}

/// `PvpAnnounceConfig` (`Custom/PvpAnnounce.ini`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvpAnnounceSettings {
    /// `ANNOUNCE_PK_PVP` (`AnnouncePkPvP`).
    pub announce_pk_pvp: bool,
    /// `ANNOUNCE_PK_PVP_NORMAL_MESSAGE` (`AnnouncePkPvPNormalMessage`): a system message (`S1`)
    /// instead of an announcement chat line.
    pub announce_pk_pvp_normal_message: bool,
    /// `ANNOUNCE_PK_MSG` (`AnnouncePkMsg`).
    pub announce_pk_msg: String,
    /// `ANNOUNCE_PVP_MSG` (`AnnouncePvpMsg`).
    pub announce_pvp_msg: String,
}

impl Default for PvpAnnounceSettings {
    fn default() -> Self {
        Self {
            announce_pk_pvp: false,
            announce_pk_pvp_normal_message: true,
            announce_pk_msg: "$killer has slaughtered $target".to_owned(),
            announce_pvp_msg: "$killer has defeated $target".to_owned(),
        }
    }
}

impl World {
    /// The part of `Player.doDie(killer)` that reads the two configs, after the
    /// `ON_PLAYER_PVP_KILL` event: `victim` is the dying player, `killer` whoever dealt the
    /// blow (a summon counts through its owner for the reward, by its own name for the PvP text).
    /// Fake players (`killer.isFakePlayer()`) are not modelled.
    pub(crate) fn custom_pvp_kill_rewards(&mut self, victim: ObjectId, killer: ObjectId) {
        let Some(pk) = self.acting_player(killer) else {
            return;
        };
        let victim_flag = self.pvp_flag_of(victim);

        // pvp/pk item rewards
        let reward = self.settings.pvp.reward.clone();
        if !(reward.disable_rewards_in_instances && self.instance_id_of(victim) != 0)
            && !(reward.disable_rewards_in_pvp_zones && self.is_inside_zone(victim, ZoneId::Pvp))
        {
            // pvp
            if reward.reward_pvp_item && victim_flag != 0 {
                self.add_item(
                    pk,
                    reward.reward_pvp_item_id,
                    i64::from(reward.reward_pvp_item_amount),
                    reward.reward_pvp_item_message,
                );
            }
            // pk
            if reward.reward_pk_item && victim_flag == 0 {
                self.add_item(
                    pk,
                    reward.reward_pk_item_id,
                    i64::from(reward.reward_pk_item_amount),
                    reward.reward_pk_item_message,
                );
            }
        }

        // announce pvp/pk
        let announce = self.settings.pvp.announce.clone();
        if announce.announce_pk_pvp && !self.is_gm(pk) {
            let target = self.creature_name(victim);
            let msg = if victim_flag == 0 {
                announce
                    .announce_pk_msg
                    .replace("$killer", &self.creature_name(pk))
                    .replace("$target", &target)
            } else {
                // NOTE(java-quirk): the PvP text names `killer`, which is the summon when a
                // summon dealt the blow; the PK text names the owner.
                announce
                    .announce_pvp_msg
                    .replace("$killer", &self.creature_name(killer))
                    .replace("$target", &target)
            };
            if announce.announce_pk_pvp_normal_message {
                self.broadcast_to_all_online_players(
                    &SystemMessage::new(SystemMessageId::S1).with(SystemMessageParam::Text(msg)),
                );
            } else {
                self.broadcast_announcement_text(&msg);
            }
        }
    }
}
