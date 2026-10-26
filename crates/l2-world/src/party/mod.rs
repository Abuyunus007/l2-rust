//! Parties: the table of parties, who is in one, the party window and the party packets.
//!
//! Design: `docs/design/m6-social.md` (parties are ids, D-11).
// Java: entity/groups/Party.java, entity/groups/AbstractPlayerGroup.java
// Java: entity/groups/{PartyDistributionType,PartyExpType,PartyMessageType}.java
// Java: entity/actor/Player.java (`_party`, `joinParty`, `leaveParty`, `isInPartyWith`,
//       `isInLooterParty`, `_partyDistributionType`)
// Java: network/serverpackets/{PartySmallWindowAll,PartySmallWindowAdd,PartySmallWindowDelete,
//       PartySmallWindowDeleteAll,PartySmallWindowUpdate,PartyMemberPosition,PartySpelled}.java
//
// # Shape
// Java has a `Party` object with a `List<Player>`, and every `Player` points back at its party. Here
// the world owns a table `PartyId -> Party` whose members are `ObjectId`s (`members[0]` is the
// leader, as in Java) and a reverse map `ObjectId -> PartyId` that stands for `Player._party`.
// Both are kept in step by the functions of this module; nothing else changes them.
//
// - `membership.rs`: `Party.addPartyMember`, `removePartyMember`, `disbandParty`, `setLeader`,
//   the packets they send, and the 12 s position broadcast on the timer wheel.
// - `invite.rs`: the five client packets (`RequestJoinParty` ...) and `Player.leaveParty` at
//   logout.
// - `request.rs`: the pending invitation of a party (the request between two players is
//   `trade/request.rs`).
// - `loot.rs`: `distributeItem`, `distributeAdena` and the looter choice of the loot types.
// - `rewards.rs`: `distributeXpAndSp`, the party bonus and the level cut-offs.
// - `buffs.rs`: `RebalanceHP` and the damage `TransferDamage` passes to a party member.
//
// # Left out, with the card that brings it
// - The summon parts (the pet window packets, a summon's `PartySpelled`) are `summon/party.rs`.
// - `command_channel.rs`: command channels (`CommandChannel`, the invitation, the member windows,
//   the channel user commands and chat, the loot right of a raid boss).
// - Dimensional rift, festival, Underground Coliseum, olympiad, events, party matching
//   rooms: later milestones; each is marked where Java asks. Duels are `duel.rs` (it hooks
//   `Party.removePartyMember` and `setLeader`).
// - Changing the loot rule inside a running party (`requestLootChange`, `answerLootChangeRequest`,
//   `ExAskModifyPartyLooting`, `ExSetPartyLooting`): the client packets for it are not ported yet.
// - `BlockList.isBlocked` in `RequestJoinParty` and the party chat: `friends.rs` (M6-FRIEND-01).

mod buffs;
mod command_channel;
mod invite;
mod loot;
mod membership;
mod request;
mod rewards;
mod windows;

use std::collections::HashMap;

use l2_protocol_game::server::PartyDistributionType;
use l2_util::ObjectId;

use crate::timer::TimerId;
use crate::world::World;

pub(crate) use command_channel::{CHANNEL_DELETE_COMMAND, CHANNEL_INFO_COMMAND, CHANNEL_LEAVE_COMMAND};
pub use command_channel::{CommandChannelId, CommandChannelWorld};
pub(crate) use invite::PARTY_INFO_COMMAND;
pub use rewards::PartyExpType;

/// The id of a party in the world's table. Not an object id: Java's `Party` is no `WorldObject`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartyId(pub u64);

/// Java `Party` has room for nine members.
pub const MAX_MEMBERS: usize = 9;

/// Java `Party.BONUS_EXP_SP`, indexed by the member count minus one (capped at the last entry).
const BONUS_EXP_SP: [f64; 9] = [1.0, 1.10, 1.20, 1.30, 1.40, 1.50, 2.0, 2.10, 2.20];

/// Java `PartyMessageType`: why a member leaves, which decides the messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyMessageType {
    /// Expelled by the leader.
    Expelled,
    /// Left by itself.
    Left,
    /// Logged out or lost its connection.
    Disconnected,
    /// The party is being disbanded (Java `NONE`): no message of its own.
    None,
}

/// The `.ini` values parties read (Java `PlayerConfig`, `RatesConfig`, `ClassBalanceConfig`).
/// `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct PartySettings {
    /// `PlayerConfig.ALT_PARTY_RANGE` (`AltPartyRange`): how far a member may be from the monster
    /// for the loot and the exp.
    pub alt_party_range: i32,
    /// `PlayerConfig.ALT_LEAVE_PARTY_LEADER` (`AltLeavePartyLeader`): the party survives its leader
    /// leaving.
    pub alt_leave_party_leader: bool,
    /// `RatesConfig.RATE_PARTY_XP` (`RatePartyXp`).
    pub rate_party_xp: f32,
    /// `RatesConfig.RATE_PARTY_SP` (`RatePartySp`).
    pub rate_party_sp: f32,
    /// `PlayerConfig.PARTY_XP_CUTOFF_METHOD` (`PartyXpCutoffMethod`).
    pub xp_cutoff_method: PartyExpType,
    /// `PlayerConfig.PARTY_XP_CUTOFF_PERCENT` (`PartyXpCutoffPercent`).
    pub xp_cutoff_percent: f64,
    /// `PlayerConfig.PARTY_XP_CUTOFF_LEVEL` (`PartyXpCutoffLevel`).
    pub xp_cutoff_level: i32,
    /// `PlayerConfig.PARTY_XP_CUTOFF_GAPS` (`PartyXpCutoffGaps`): the level gap ranges of the
    /// `highfive` method.
    pub xp_cutoff_gaps: Vec<[i32; 2]>,
    /// `PlayerConfig.PARTY_XP_CUTOFF_GAP_PERCENTS` (`PartyXpCutoffGapPercent`).
    pub xp_cutoff_gap_percents: Vec<i32>,
    /// `PlayerConfig.LOOT_RAIDS_PRIVILEGE_INTERVAL` (`RaidLootRightsInterval` * 1000), milliseconds:
    /// how long a command channel keeps the loot right of a raid boss it stopped attacking.
    pub loot_raids_privilege_interval_ms: i64,
    /// `PlayerConfig.LOOT_RAIDS_PRIVILEGE_CC_SIZE` (`RaidLootRightsCCSize`): members a channel needs
    /// for the loot right.
    pub loot_raids_privilege_cc_size: i32,
    /// `ClassBalanceConfig.EXP_AMOUNT_MULTIPLIERS` (`ExpAmountMultipliers`), by class id.
    pub exp_amount_multipliers: Vec<f32>,
    /// `ClassBalanceConfig.SP_AMOUNT_MULTIPLIERS` (`SpAmountMultipliers`), by class id.
    pub sp_amount_multipliers: Vec<f32>,
}

impl Default for PartySettings {
    fn default() -> Self {
        Self {
            alt_party_range: 1500,
            alt_leave_party_leader: false,
            rate_party_xp: 1.0,
            rate_party_sp: 1.0,
            xp_cutoff_method: PartyExpType::Level,
            xp_cutoff_percent: 3.0,
            xp_cutoff_level: 20,
            xp_cutoff_gaps: vec![[0, 9], [10, 14], [15, 99]],
            xp_cutoff_gap_percents: vec![100, 30, 0],
            loot_raids_privilege_interval_ms: 900_000,
            loot_raids_privilege_cc_size: 45,
            exp_amount_multipliers: vec![1.0; crate::stats::ClassBalance::CLASSES],
            sp_amount_multipliers: vec![1.0; crate::stats::ClassBalance::CLASSES],
        }
    }
}

/// Java `Party`: the members in order (the first is the leader), and the rules of the party.
#[derive(Debug, Clone)]
pub(crate) struct Party {
    /// Java `_members`.
    pub(crate) members: Vec<ObjectId>,
    /// Java `_partyLvl`: the highest member level.
    pub(crate) level: i32,
    /// Java `_distributionType`.
    pub(crate) distribution_type: PartyDistributionType,
    /// Java `_itemLastLoot`: the index of the member who got the last item in the by-turn modes.
    pub(crate) item_last_loot: usize,
    /// Java `_pendingInvitation` and `_pendingInviteTimeout` (a game tick).
    pub(crate) pending_invitation: bool,
    pub(crate) pending_invite_timeout: i64,
    /// Java `_positionBroadcastTask`.
    pub(crate) position_timer: Option<TimerId>,
    /// Java `_disbanding`.
    pub(crate) disbanding: bool,
    /// Java `_commandChannel`.
    pub(crate) command_channel: Option<CommandChannelId>,
}

impl Party {
    /// Java `new Party(leader, distributionType)`.
    fn new(leader: ObjectId, level: i32, distribution_type: PartyDistributionType) -> Self {
        Self {
            members: vec![leader],
            level,
            distribution_type,
            item_last_loot: 0,
            pending_invitation: false,
            pending_invite_timeout: 0,
            position_timer: None,
            disbanding: false,
            command_channel: None,
        }
    }

    /// Java `getLeader()`.
    pub(crate) fn leader(&self) -> Option<ObjectId> {
        self.members.first().copied()
    }
}

/// All the party state of the world.
#[derive(Debug, Default)]
pub struct PartyWorld {
    pub(crate) parties: HashMap<PartyId, Party>,
    /// Java `Player._party`.
    pub(crate) of_player: HashMap<ObjectId, PartyId>,
    next_id: u64,
    /// Java `Player._partyDistributionType`: the loot rule an inviter chose for the party it is
    /// about to create.
    pub(crate) chosen_distribution: HashMap<ObjectId, PartyDistributionType>,
    /// The command channels the parties are in.
    pub(crate) channels: CommandChannelWorld,
}

impl PartyWorld {
    fn allocate(&mut self) -> PartyId {
        self.next_id += 1;
        PartyId(self.next_id)
    }

    /// Parties in the table (for tests and tools).
    pub fn len(&self) -> usize {
        self.parties.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parties.is_empty()
    }
}

impl World {
    /// Java `Player.getParty()`: the party a player is in; a summon's is its owner's
    /// (`Summon.getParty()`).
    pub fn party_of(&self, id: ObjectId) -> Option<PartyId> {
        let id = self.summon_owner(id).unwrap_or(id);
        self.party.of_player.get(&id).copied()
    }

    /// How many parties exist (for tests and tools).
    pub fn party_count(&self) -> usize {
        self.party.len()
    }

    /// Java `Player.isInParty()`.
    pub fn is_in_party(&self, id: ObjectId) -> bool {
        self.party.of_player.contains_key(&id)
    }

    /// Java `Party.getMembers()`: the members in order, leader first. Empty for an unknown party.
    pub fn party_members(&self, party: PartyId) -> Vec<ObjectId> {
        self.party
            .parties
            .get(&party)
            .map(|p| p.members.clone())
            .unwrap_or_default()
    }

    /// Java `Party.getLeader()`.
    pub fn party_leader(&self, party: PartyId) -> Option<ObjectId> {
        self.party.parties.get(&party).and_then(Party::leader)
    }

    /// Java `Party.getLevel()`.
    pub fn party_level(&self, party: PartyId) -> Option<i32> {
        self.party.parties.get(&party).map(|p| p.level)
    }

    /// Java `Party.getDistributionType()`.
    pub fn party_distribution_type(&self, party: PartyId) -> Option<PartyDistributionType> {
        self.party.parties.get(&party).map(|p| p.distribution_type)
    }

    /// The members of the party `id` is in, `id` included (just `id` itself without a party).
    /// This is the "party" the target handlers read.
    pub fn party_members_of(&self, id: ObjectId) -> Vec<ObjectId> {
        match self.party_of(id) {
            Some(party) => self.party_members(party),
            None => vec![id],
        }
    }

    /// Java `Player.isInPartyWith(target)`: both are in a party and the leaders are the same.
    // NOTE: Java compares the leaders' object ids, not the party objects.
    pub fn is_in_party_with(&self, id: ObjectId, other: ObjectId) -> bool {
        match (self.party_of(id), self.party_of(other)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Java `Player.isInLooterParty(looterId)`: `looter` is online and in the same party.
    pub fn is_in_looter_party(&self, id: ObjectId, looter: ObjectId) -> bool {
        if !self.players.contains_key(&looter) {
            return false;
        }
        // If the player is in a CommandChannel, its members are the looters.
        if let Some(channel) = self.command_channel_of(id).filter(|_| self.is_in_party(id)) {
            return self.command_channel_contains(channel, looter);
        }
        self.is_in_party_with(id, looter)
    }

    /// Java `Party.isLeader(player)`.
    pub fn is_party_leader(&self, id: ObjectId) -> bool {
        self.party_of(id)
            .and_then(|party| self.party_leader(party))
            .is_some_and(|leader| leader == id)
    }
}

/// Java `Party.getBaseExpSpBonus(membersCount)`.
pub(crate) fn base_exp_sp_bonus(members: usize) -> f64 {
    let Some(mut i) = members.checked_sub(1).filter(|i| *i >= 1) else {
        return 1.0;
    };
    if i >= BONUS_EXP_SP.len() {
        i = BONUS_EXP_SP.len() - 1;
    }
    BONUS_EXP_SP[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `getBaseExpSpBonus`: 1.0 for one or two members' index below 1, then the table, capped.
    #[test]
    fn the_party_bonus_table() {
        assert_eq!(base_exp_sp_bonus(0), 1.0);
        assert_eq!(base_exp_sp_bonus(1), 1.0);
        assert_eq!(base_exp_sp_bonus(2), 1.10);
        assert_eq!(base_exp_sp_bonus(3), 1.20);
        assert_eq!(base_exp_sp_bonus(6), 1.50);
        assert_eq!(base_exp_sp_bonus(7), 2.0);
        assert_eq!(base_exp_sp_bonus(9), 2.20);
        // Java caps the index at the last entry.
        assert_eq!(base_exp_sp_bonus(12), 2.20);
    }

    #[test]
    fn the_defaults_are_javas() {
        let s = PartySettings::default();
        assert_eq!(s.alt_party_range, 1500);
        assert!(!s.alt_leave_party_leader);
        assert_eq!(s.xp_cutoff_method, PartyExpType::Level);
        assert_eq!(s.xp_cutoff_gaps, vec![[0, 9], [10, 14], [15, 99]]);
        assert_eq!(s.xp_cutoff_gap_percents, vec![100, 30, 0]);
        assert_eq!(s.exp_amount_multipliers.len(), 137);
    }
}
