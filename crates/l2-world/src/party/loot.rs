//! Sharing loot: who gets an item or the adena of a kill, by the loot rule of the party.
// Java: entity/groups/Party.java (getCheckedRandomMember, getCheckedNextLooter, getActualLooter,
//       distributeItem, distributeAdena)
// Java: entity/actor/Player.java (doAutoLoot, addItem with `ItemProcessType.SWEEP`)

use l2_data::item::ADENA_ID;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{PartyDistributionType, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::PartyId;
use crate::inventory::item_name_param;
use crate::player::ItemData;
use crate::world::World;

impl World {
    /// Java `Party.getCheckedRandomMember(itemId, target)`: a random member with room for the
    /// item who stands within `AltPartyRange` of `target`.
    fn party_checked_random_member(
        &mut self,
        party: PartyId,
        item_id: i32,
        target: ObjectId,
    ) -> Option<ObjectId> {
        let range = self.settings.party.alt_party_range;
        let available: Vec<ObjectId> = self
            .party_members(party)
            .into_iter()
            .filter(|m| {
                self.validate_capacity_by_item_id(*m, item_id)
                    && self.check_if_in_range(range, target, *m, true)
            })
            .collect();
        if available.is_empty() {
            return None;
        }
        let index = self
            .rng()
            .below(i32::try_from(available.len()).unwrap_or(i32::MAX));
        usize::try_from(index)
            .ok()
            .and_then(|i| available.get(i).copied())
    }

    /// Java `Party.getCheckedNextLooter(itemId, target)`: the next member in turn who has room and
    /// is in range; `_itemLastLoot` moves on even when a member is skipped.
    fn party_checked_next_looter(
        &mut self,
        party: PartyId,
        item_id: i32,
        target: ObjectId,
    ) -> Option<ObjectId> {
        let range = self.settings.party.alt_party_range;
        let count = self.party_members(party).len();
        for _ in 0..count {
            let members = self.party_members(party);
            let p = self.party.parties.get_mut(&party)?;
            p.item_last_loot += 1;
            if p.item_last_loot >= count {
                p.item_last_loot = 0;
            }
            // "continue, take another member if this just logged off"
            let Some(member) = members.get(p.item_last_loot).copied() else {
                continue;
            };
            if self.validate_capacity_by_item_id(member, item_id)
                && self.check_if_in_range(range, target, member, true)
            {
                return Some(member);
            }
        }
        None
    }

    /// Java `Party.getActualLooter(player, itemId, spoil, target)`.
    fn party_actual_looter(
        &mut self,
        party: PartyId,
        player: ObjectId,
        item_id: i32,
        spoil: bool,
        target: ObjectId,
    ) -> ObjectId {
        let looter = match self.party_distribution_type(party) {
            Some(PartyDistributionType::Random) if !spoil => {
                self.party_checked_random_member(party, item_id, target)
            }
            Some(PartyDistributionType::RandomIncludingSpoil) => {
                self.party_checked_random_member(party, item_id, target)
            }
            Some(PartyDistributionType::ByTurn) if !spoil => {
                self.party_checked_next_looter(party, item_id, target)
            }
            Some(PartyDistributionType::ByTurnIncludingSpoil) => {
                self.party_checked_next_looter(party, item_id, target)
            }
            _ => None,
        };
        looter.unwrap_or(player)
    }

    /// Java `Party.distributeAdena(player, adena, target)`: the members in range of `target` split
    /// the adena; the rest goes one by one from a random member on.
    pub(crate) fn party_distribute_adena(&mut self, party: PartyId, adena: i64, target: ObjectId) {
        let range = self.settings.party.alt_party_range;
        let to_reward: Vec<ObjectId> = self
            .party_members(party)
            .into_iter()
            .filter(|m| self.check_if_in_range(range, target, *m, true))
            .collect();
        // If no members are eligible or there is no adena to distribute, exit early.
        if to_reward.is_empty() || adena <= 0 {
            return;
        }
        let count = i64::try_from(to_reward.len()).unwrap_or(i64::MAX);
        let base_share = adena / count;
        let remainder = adena % count;
        let mut shares = vec![base_share; to_reward.len()];
        // Distribute the leftover adena (1 adena each) to some members starting from a random
        // index.
        if remainder > 0 {
            let start = usize::try_from(
                self.rng()
                    .below(i32::try_from(to_reward.len()).unwrap_or(i32::MAX)),
            )
            .unwrap_or(0);
            for i in 0..usize::try_from(remainder).unwrap_or(0) {
                shares[(start + i) % to_reward.len()] += 1;
            }
        }
        // NOTE(java-quirk): Java hands the shares out in the iteration order of a `HashMap`; here
        // it is the party order.
        for (member, amount) in to_reward.into_iter().zip(shares) {
            if amount > 0 {
                self.add_adena(member, amount, true);
            }
        }
    }

    /// Java `Party.distributeItem(player, itemId, itemCount, spoil, target)`: an item of a kill
    /// (auto loot or sweep) goes to the member the loot rule picks, and the others hear who got it.
    pub(crate) fn party_distribute_item(
        &mut self,
        player: ObjectId,
        item_id: i32,
        count: i64,
        spoil: bool,
        target: ObjectId,
    ) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        if item_id == ADENA_ID {
            self.party_distribute_adena(party, count, target);
            return;
        }
        let looter = self.party_actual_looter(party, player, item_id, spoil, target);
        if spoil {
            self.add_swept_item(looter, item_id, count);
        } else {
            self.add_item(looter, item_id, count, true);
        }

        // Send messages to other party members about the reward.
        let name = SystemMessageParam::Text(
            self.players
                .get(&looter)
                .map(|p| p.data.name.clone())
                .unwrap_or_default(),
        );
        let item = item_name_param(self.item_templates().as_deref(), item_id);
        let message = if count > 1 {
            SystemMessage::new(if spoil {
                SystemMessageId::C1HasObtainedS3S2ByUsingSweeper
            } else {
                SystemMessageId::C1HasObtainedS3S2
            })
            .with(name)
            .with(item)
            .with(SystemMessageParam::LongNumber(count))
        } else {
            SystemMessage::new(if spoil {
                SystemMessageId::C1HasObtainedS2ByUsingSweeper
            } else {
                SystemMessageId::C1HasObtainedS2
            })
            .with(name)
            .with(item)
        };
        self.broadcast_party_except(party, looter, &message);
    }

    /// Java `Party.distributeItem(player, item)`: an item picked up from the ground. The adena is
    /// shared around the picker and its item destroyed; any other item goes to the looter the
    /// rule picks, who gets the item itself.
    pub(crate) fn party_distribute_ground_item(&mut self, player: ObjectId, item: ItemData) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        if item.item_id == ADENA_ID {
            self.party_distribute_adena(party, item.count, player);
            // `ItemManager.destroyItem(LOOT, item, player, null)`.
            self.release_item_id(item.object_id);
            return;
        }
        let looter = self.party_actual_looter(party, player, item.item_id, false, player);
        let (item_id, count) = (item.item_id, item.count);
        self.add_item_object(looter, item, true);

        let name = SystemMessageParam::Text(
            self.players
                .get(&looter)
                .map(|p| p.data.name.clone())
                .unwrap_or_default(),
        );
        let item_name = item_name_param(self.item_templates().as_deref(), item_id);
        let message = if count > 1 {
            SystemMessage::new(SystemMessageId::C1HasObtainedS3S2)
                .with(name)
                .with(item_name)
                .with(SystemMessageParam::LongNumber(count))
        } else {
            SystemMessage::new(SystemMessageId::C1HasObtainedS2)
                .with(name)
                .with(item_name)
        };
        self.broadcast_party_except(party, looter, &message);
    }

    /// Java `Player.addItem(ItemProcessType.SWEEP, itemId, count, reference, true)`: the same as
    /// [`World::add_item`], but the message is "You have earned" (`SWEEP` and `QUEST` say so,
    /// every other process says "You have obtained").
    pub(crate) fn add_swept_item(&mut self, player: ObjectId, item_id: i32, count: i64) {
        if count <= 0 {
            return;
        }
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            tracing::error!("Item doesn't exist so cannot be added. Item ID: {item_id}");
            return;
        };
        // Sends message to client: `(!isCastingNow() && hasExImmediateEffect()) || !hasExImmediateEffect()`.
        if !template.ex_immediate_effect || !self.is_casting_now(player) {
            let name = item_name_param(Some(&templates), item_id);
            let message = if count > 1 {
                SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                    .with(name)
                    .with(SystemMessageParam::LongNumber(count))
            } else {
                SystemMessage::new(SystemMessageId::YouHaveEarnedS1).with(name)
            };
            self.send(player, &message);
        }
        self.add_item(player, item_id, count, false);
    }

    /// Java `Player.doAutoLoot(target, itemId, itemCount)`: with a party the loot rule decides who
    /// gets it (herbs excepted); otherwise the killer does.
    pub(crate) fn do_auto_loot_for(&mut self, player: ObjectId, npc: ObjectId, item_id: i32, count: i64) {
        let herb = self
            .item_templates()
            .and_then(|t| t.get(item_id).map(|t| t.ex_immediate_effect))
            .unwrap_or(false);
        if self.is_in_party(player) && !herb {
            self.party_distribute_item(player, item_id, count, false, npc);
        } else if item_id == ADENA_ID {
            self.add_adena(player, count, true);
        } else {
            self.add_item(player, item_id, count, true);
        }
    }
}
