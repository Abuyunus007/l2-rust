//! Command channels: several parties under one leader, the invitation, the member windows, the
//! user commands, the channel chat and the loot right of a raid boss.
// Java: entity/groups/CommandChannel.java (and AbstractPlayerGroup.broadcast*)
// Java: entity/actor/tasks/attackable/CommandChannelTimer.java
// Java: entity/actor/Attackable.java (reduceCurrentHp: the first command channel that attacked)
// Java: network/clientpackets/{RequestExAskJoinMPCC,RequestExAcceptJoinMPCC,RequestExOustFromMPCC,
//       RequestExMPCCShowPartyMembersInfo}.java
// Java: network/serverpackets/{ExOpenMPCC,ExCloseMPCC,ExAskJoinMPCC,ExMPCCPartyInfoUpdate,
//       ExMPCCShowPartyMemberInfo,ExMultiPartyCommandChannelInfo}.java
// Java: dist/game/data/scripts/handlers/chat/commands/user/{ChannelDelete,ChannelLeave,ChannelInfo}.java
// Java: dist/game/data/scripts/handlers/chat/channels/{ChatPartyRoomAll,ChatPartyRoomCommander}.java
//
// # Shape
// Java's `CommandChannel` holds `Party` objects and a leader `Player`. Here it is a record in a
// table `CommandChannelId -> CommandChannel` of party ids, and `Party.command_channel` is the
// back reference (`Party._commandChannel`). The leader is an `ObjectId`.
//
// There is no client packet to leave or to delete a channel in this Java: those are the user
// commands 96 (`/channelleave`) and 93 (`/channeldelete`), and 97 (`/channelinfo`) shows the
// window. A party also leaves the channel when it breaks up (`membership.rs`).
//
// # Quirks kept
// - NOTE(java-quirk): the parties of a channel are a `ConcurrentHashMap` key set, so their order
//   is arbitrary in Java (it is not even stable); they are kept in the order they joined here.
// - NOTE(java-quirk): `disbandChannel` removes the parties one by one, and each removal under two
//   parties disbands again; the messages go out once per removal, as in Java.
// - NOTE(java-quirk): the channel leader is the player who made it. It follows an explicit leader
//   change of its party (`Party.setLeader`), but not a leader who leaves a party that survives it
//   (`AltLeavePartyLeader`), so it can name a player who left the party or the game.

use std::collections::HashMap;

use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, CreatureSay, ExAskJoinMpcc, ExCloseMpcc, ExMpccPartyInfoUpdate, ExMpccShowPartyMemberInfo,
    ExMultiPartyCommandChannelInfo, ExOpenMpcc, MpccMember, MpccPartyEntry, SayBody, SenderField,
    SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use super::PartyId;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `ChannelDelete.COMMAND_IDS`.
pub(crate) const CHANNEL_DELETE_COMMAND: i32 = 93;
/// `ChannelLeave.COMMAND_IDS`.
pub(crate) const CHANNEL_LEAVE_COMMAND: i32 = 96;
/// `ChannelInfo.COMMAND_IDS`.
pub(crate) const CHANNEL_INFO_COMMAND: i32 = 97;

/// `CommandChannelTimer`: checks every 10 s whether the raid has been left alone.
const CHANNEL_TIMER_MS: u64 = 10_000;
/// The item `RequestExAskJoinMPCC.askJoinMPCC` accepts instead of a clan: the Strategy Guide.
const STRATEGY_GUIDE_ID: i32 = 8871;
/// The skill `RequestExAskJoinMPCC.askJoinMPCC` accepts with pledge class 5: Clan Imperium.
const CLAN_IMPERIUM_SKILL_ID: i32 = 391;
/// The pledge class (Baron) Clan Imperium needs.
const CLAN_IMPERIUM_PLEDGE_CLASS: i32 = 5;
/// The clan level a clan leader needs to open a channel.
const CHANNEL_CLAN_LEVEL: i32 = 5;

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

/// The id of a command channel in the world's table (Java `CommandChannel` is no `WorldObject`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandChannelId(pub u64);

/// Java `CommandChannel`.
#[derive(Debug, Clone)]
pub(crate) struct CommandChannel {
    /// Java `_parties`.
    parties: Vec<PartyId>,
    /// Java `_commandLeader`.
    leader: ObjectId,
    /// Java `_channelLvl`: the highest party level.
    level: i32,
}

/// What an `Attackable` keeps of the channel that attacked it first: Java `_firstCommandChannelAttacked`,
/// `_commandChannelLastAttack` and `_commandChannelTimer`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RaidLootRight {
    channel: CommandChannelId,
    /// The leader the channel had when it was last seen. Java keeps the object, whose
    /// `getLeaderObjectId()` still answers after the channel disbanded.
    leader: ObjectId,
    /// `System.currentTimeMillis()` of the last attack, as world milliseconds.
    last_attack_ms: i64,
    timer: Option<TimerId>,
}

/// All the command channel state of the world.
#[derive(Debug, Default)]
pub struct CommandChannelWorld {
    channels: HashMap<CommandChannelId, CommandChannel>,
    next_id: u64,
    /// By raid boss.
    raid_rights: HashMap<ObjectId, RaidLootRight>,
}

impl CommandChannelWorld {
    /// Channels in the table (for tests and tools).
    pub fn len(&self) -> usize {
        self.channels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }
}

impl World {
    // ------------------------------------------------------------------ queries

    /// Java `Party.getCommandChannel()`.
    pub fn command_channel_of_party(&self, party: PartyId) -> Option<CommandChannelId> {
        self.party.parties.get(&party).and_then(|p| p.command_channel)
    }

    /// Java `player.getParty().getCommandChannel()`.
    pub fn command_channel_of(&self, player: ObjectId) -> Option<CommandChannelId> {
        self.party_of(player)
            .and_then(|party| self.command_channel_of_party(party))
    }

    /// Java `player.isInParty() && player.getParty().isInCommandChannel()`.
    pub fn is_in_command_channel(&self, player: ObjectId) -> bool {
        self.command_channel_of(player).is_some()
    }

    /// How many command channels exist (for tests and tools).
    pub fn command_channel_count(&self) -> usize {
        self.party.channels.len()
    }

    /// Java `CommandChannel.getParties()`.
    pub fn command_channel_parties(&self, channel: CommandChannelId) -> Vec<PartyId> {
        self.party
            .channels
            .channels
            .get(&channel)
            .map(|c| c.parties.clone())
            .unwrap_or_default()
    }

    /// Java `CommandChannel.getLeader()`.
    pub fn command_channel_leader(&self, channel: CommandChannelId) -> Option<ObjectId> {
        self.party.channels.channels.get(&channel).map(|c| c.leader)
    }

    /// Java `CommandChannel.getLevel()`.
    pub fn command_channel_level(&self, channel: CommandChannelId) -> i32 {
        self.party.channels.channels.get(&channel).map_or(0, |c| c.level)
    }

    /// Java `CommandChannel.getMembers()`: the members of every party, party by party.
    pub fn command_channel_members(&self, channel: CommandChannelId) -> Vec<ObjectId> {
        self.command_channel_parties(channel)
            .into_iter()
            .flat_map(|party| self.party_members(party))
            .collect()
    }

    /// Java `CommandChannel.getMemberCount()`.
    pub fn command_channel_member_count(&self, channel: CommandChannelId) -> usize {
        self.command_channel_members(channel).len()
    }

    /// Java `CommandChannel.containsPlayer(player)`.
    pub fn command_channel_contains(&self, channel: CommandChannelId, player: ObjectId) -> bool {
        self.command_channel_members(channel).contains(&player)
    }

    /// Java `CommandChannel.isLeader(player)`.
    pub fn is_command_channel_leader(&self, channel: CommandChannelId, player: ObjectId) -> bool {
        self.command_channel_leader(channel) == Some(player)
    }

    /// Java `a.getParty().getCommandChannel() == b.getParty().getCommandChannel()` with both
    /// channels there (the tests of `Player.isAutoAttackable`, `Skill.checkForAreaOffensiveSkills`
    /// and `Player.checkIfPvP` ask for a shared channel, never for "both have none").
    pub fn is_in_same_command_channel(&self, a: ObjectId, b: ObjectId) -> bool {
        match (self.command_channel_of(a), self.command_channel_of(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }

    /// Java `Player.isInCommandChannelWith(target)`: both parties are in a channel and the two
    /// channel leaders are the same player.
    pub fn is_in_command_channel_with(&self, player: ObjectId, target: ObjectId) -> bool {
        match (self.command_channel_of(player), self.command_channel_of(target)) {
            (Some(a), Some(b)) => a == b || self.command_channel_leader(a) == self.command_channel_leader(b),
            _ => false,
        }
    }

    /// Java `CommandChannel.meetRaidWarCondition(obj)`: `obj` is a raid and the channel has
    /// `RaidLootRightsCCSize` members.
    pub fn meet_raid_war_condition(&self, channel: CommandChannelId, obj: ObjectId) -> bool {
        if !self.is_raid(obj) {
            return false;
        }
        self.command_channel_member_count(channel)
            >= usize::try_from(self.settings.party.loot_raids_privilege_cc_size).unwrap_or(0)
    }

    fn cc_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------ packets

    /// `AbstractPlayerGroup.broadcastPacket(packet)` of a channel.
    pub(crate) fn broadcast_command_channel(&self, channel: CommandChannelId, packet: &impl Encode) {
        let bytes = packet.to_bytes();
        for member in self.command_channel_members(channel) {
            self.send_bytes(member, bytes.clone());
        }
    }

    /// `new ExMPCCPartyInfoUpdate(party, mode)`.
    fn mpcc_party_info_update(&self, party: PartyId, mode: i32) -> ExMpccPartyInfoUpdate {
        let leader = self.party_leader(party);
        ExMpccPartyInfoUpdate {
            leader_name: leader.map(|l| self.cc_name(l)).unwrap_or_default(),
            leader_object_id: leader.map_or(0, |l| l.0),
            member_count: self.party_members(party).len() as i32,
            mode,
        }
    }

    /// `new ExMPCCShowPartyMemberInfo(party)`.
    fn mpcc_show_party_member_info(&self, party: PartyId) -> ExMpccShowPartyMemberInfo {
        ExMpccShowPartyMemberInfo {
            members: self
                .party_members(party)
                .into_iter()
                .map(|m| MpccMember {
                    name: self.cc_name(m),
                    object_id: m.0,
                    class_id: self.players.get(&m).map_or(0, |p| p.data.class_id),
                })
                .collect(),
        }
    }

    /// `new ExMultiPartyCommandChannelInfo(channel)`.
    fn multi_party_command_channel_info(
        &self,
        channel: CommandChannelId,
    ) -> Option<ExMultiPartyCommandChannelInfo> {
        let leader = self.command_channel_leader(channel)?;
        Some(ExMultiPartyCommandChannelInfo {
            leader_name: self.cc_name(leader),
            // "Channel loot 0 or 1": Java always writes 0.
            channel_loot: 0,
            member_count: self.command_channel_member_count(channel) as i32,
            parties: self
                .command_channel_parties(channel)
                .into_iter()
                .map(|party| {
                    let leader = self.party_leader(party);
                    MpccPartyEntry {
                        leader_name: leader.map(|l| self.cc_name(l)).unwrap_or_default(),
                        leader_object_id: leader.map_or(0, |l| l.0),
                        member_count: self.party_members(party).len() as i32,
                    }
                })
                .collect(),
        })
    }

    // ------------------------------------------------------------------ CommandChannel

    /// Java `new CommandChannel(leader)`: the leader's party is the first one. `None` when the
    /// leader has no party (Java would throw).
    pub(crate) fn create_command_channel(&mut self, leader: ObjectId) -> Option<CommandChannelId> {
        let party = self.party_of(leader)?;
        let level = self.party_level(party)?;
        let channels = &mut self.party.channels;
        channels.next_id += 1;
        let id = CommandChannelId(channels.next_id);
        channels.channels.insert(
            id,
            CommandChannel {
                parties: vec![party],
                leader,
                level,
            },
        );
        if let Some(p) = self.party.parties.get_mut(&party) {
            p.command_channel = Some(id);
        }
        self.broadcast_party(
            party,
            &SystemMessage::new(SystemMessageId::TheCommandChannelHasBeenFormed),
        );
        self.broadcast_party(party, &ExOpenMpcc);
        Some(id)
    }

    /// Java `CommandChannel.addParty(party)`.
    pub(crate) fn command_channel_add_party(&mut self, channel: CommandChannelId, party: PartyId) {
        if !self.party.channels.channels.contains_key(&channel) {
            return;
        }
        // Update the CCinfo for existing players.
        let update = self.mpcc_party_info_update(party, 1);
        self.broadcast_command_channel(channel, &update);
        let party_level = self.party_level(party).unwrap_or(0);
        if let Some(c) = self.party.channels.channels.get_mut(&channel) {
            if !c.parties.contains(&party) {
                c.parties.push(party);
            }
            if party_level > c.level {
                c.level = party_level;
            }
        }
        if let Some(p) = self.party.parties.get_mut(&party) {
            p.command_channel = Some(channel);
        }
        self.broadcast_party(
            party,
            &SystemMessage::new(SystemMessageId::YouHaveJoinedTheCommandChannel),
        );
        self.broadcast_party(party, &ExOpenMpcc);
    }

    /// Java `CommandChannel.removeParty(party)`.
    pub(crate) fn command_channel_remove_party(&mut self, channel: CommandChannelId, party: PartyId) {
        let levels: Vec<(PartyId, i32)> = self
            .command_channel_parties(channel)
            .into_iter()
            .filter(|p| *p != party)
            .map(|p| (p, self.party_level(p).unwrap_or(0)))
            .collect();
        let Some(c) = self.party.channels.channels.get_mut(&channel) else {
            return;
        };
        c.parties.retain(|p| *p != party);
        c.level = levels.iter().map(|(_, level)| *level).fold(0, i32::max);
        let remaining = c.parties.len();

        if let Some(p) = self.party.parties.get_mut(&party) {
            p.command_channel = None;
        }
        self.broadcast_party(party, &ExCloseMpcc);
        if remaining < 2 {
            self.broadcast_command_channel(
                channel,
                &SystemMessage::new(SystemMessageId::TheCommandChannelHasBeenDisbanded),
            );
            self.command_channel_disband(channel);
        } else {
            // Update the CCinfo for existing players.
            let update = self.mpcc_party_info_update(party, 0);
            self.broadcast_command_channel(channel, &update);
        }
    }

    /// Java `CommandChannel.disbandChannel()`.
    pub(crate) fn command_channel_disband(&mut self, channel: CommandChannelId) {
        for party in self.command_channel_parties(channel) {
            // A removal inside this loop may have taken the rest of the channel already.
            if !self.command_channel_parties(channel).contains(&party) {
                continue;
            }
            self.command_channel_remove_party(channel, party);
        }
        self.party.channels.channels.remove(&channel);
    }

    /// Java `CommandChannel.setLeader(leader)`.
    pub(crate) fn command_channel_set_leader(&mut self, channel: CommandChannelId, leader: ObjectId) {
        let level = self.players.get(&leader).map_or(0, |p| p.core.level);
        if let Some(c) = self.party.channels.channels.get_mut(&channel) {
            c.leader = leader;
            if level > c.level {
                c.level = level;
            }
            if let Some(right) = self
                .party
                .channels
                .raid_rights
                .values_mut()
                .find(|r| r.channel == channel)
            {
                right.leader = leader;
            }
        }
    }

    // ------------------------------------------------------------------ RequestExAskJoinMPCC

    /// Java `RequestExAskJoinMPCC.runImpl`.
    pub(crate) fn on_request_ex_ask_join_mpcc(&mut self, player: ObjectId, name: &str) {
        if !self.players.contains_key(&player) {
            return;
        }
        let Some(target) = self.player_by_name(name) else {
            return;
        };
        // invite yourself? ;)
        if self.is_in_party_with(player, target) {
            return;
        }
        // activeChar is in a Party?
        let Some(active_party) = self.party_of(player) else {
            return;
        };
        let target_name = self.cc_name(target);
        // activeChar is PartyLeader?
        if !self.is_party_leader(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveAuthorityToInviteSomeoneToTheCommandChannel),
            );
            return;
        }
        if let Some(channel) = self.command_channel_of_party(active_party)
            && !self.is_command_channel_leader(channel, player)
        {
            // In a channel, but not its leader.
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveAuthorityToInviteSomeoneToTheCommandChannel),
            );
            return;
        }
        // Not in a channel, or its leader: the same rules.
        // target in a party?
        let Some(target_party) = self.party_of(target) else {
            self.send_message(
                player,
                &format!("{target_name} doesn't have party and cannot be invited to Command Channel."),
            );
            return;
        };
        // targets party already in a CChannel?
        if self.command_channel_of_party(target_party).is_some() {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::C1SPartyIsAlreadyAMemberOfTheCommandChannel)
                    .with(text(&target_name)),
            );
        } else {
            // ready to open a new CC: send request to targets Party's PartyLeader
            self.ask_join_mpcc(player, target_party);
        }
    }

    /// Java `RequestExAskJoinMPCC.askJoinMPCC(requestor, target)`; `target_party` is the party of
    /// `target`.
    fn ask_join_mpcc(&mut self, requestor: ObjectId, target_party: PartyId) {
        let mut has_right = false;
        if self.is_clan_leader(requestor)
            && self
                .clan_of(requestor)
                .is_some_and(|c| c.level >= CHANNEL_CLAN_LEVEL)
        {
            // Clan leader of level5 Clan or higher.
            has_right = true;
        } else if self
            .inventory_item_by_item_id(requestor, STRATEGY_GUIDE_ID)
            .is_some()
        {
            // 8871 Strategy Guide.
            // TODO(java): should be destroyed after a successful invite?
            has_right = true;
        } else if self.pledge_class_of(requestor) >= CLAN_IMPERIUM_PLEDGE_CLASS
            && self.known_skill(requestor, CLAN_IMPERIUM_SKILL_ID).is_some()
        {
            // At least Baron or higher and the skill Clan Imperium.
            has_right = true;
        }
        if !has_right {
            self.send(
                requestor,
                &SystemMessage::new(
                    SystemMessageId::CommandChannelsCanOnlyBeFormedByAPartyLeaderWhoIsAlsoTheLeaderOfALevel5Clan,
                ),
            );
            return;
        }

        // Get the target's party leader, and do whole actions on him.
        let Some(target_leader) = self.party_leader(target_party) else {
            return;
        };
        let leader_name = self.cc_name(target_leader);
        if !self.is_processing_request(target_leader) {
            self.on_transaction_request(requestor, target_leader);
            let requestor_name = self.cc_name(requestor);
            self.send(
                target_leader,
                &SystemMessage::new(SystemMessageId::C1IsInvitingYouToACommandChannelDoYouAccept)
                    .with(text(&requestor_name)),
            );
            self.send(target_leader, &ExAskJoinMpcc { requestor_name });
            self.send_message(
                requestor,
                &format!("You invited {leader_name} to your Command Channel."),
            );
        } else {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater)
                    .with(text(&leader_name)),
            );
        }
    }

    // ------------------------------------------------------------------ RequestExAcceptJoinMPCC

    /// Java `RequestExAcceptJoinMPCC.runImpl`.
    pub(crate) fn on_request_ex_accept_join_mpcc(&mut self, player: ObjectId, response: i32) {
        if !self.players.contains_key(&player) {
            return;
        }
        let Some(requestor) = self.active_requester(player) else {
            return;
        };
        // NOTE: Java keeps the requestor object of a player who logged out, and a requestor who
        // left the party makes `requestor.getParty()` throw; the request ends here instead.
        let Some(requestor_party) = self
            .party_of(requestor)
            .filter(|_| self.players.contains_key(&requestor))
        else {
            self.set_active_requester(player, None);
            return;
        };

        if response == 1 {
            let mut new_cc = false;
            let channel = match self.command_channel_of_party(requestor_party) {
                Some(channel) => channel,
                None => {
                    // Create new CC.
                    let Some(channel) = self.create_command_channel(requestor) else {
                        return;
                    };
                    self.send(
                        requestor,
                        &SystemMessage::new(SystemMessageId::TheCommandChannelHasBeenFormed),
                    );
                    new_cc = true;
                    channel
                }
            };
            // `addParty(null)` does nothing: the answering player left its party meanwhile.
            if let Some(party) = self.party_of(player) {
                self.command_channel_add_party(channel, party);
            }
            if !new_cc {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveJoinedTheCommandChannel),
                );
            }
        } else {
            self.send_message(requestor, "The player declined to join your Command Channel.");
        }

        self.set_active_requester(player, None);
        self.on_transaction_response(requestor);
    }

    // ------------------------------------------------------------------ RequestExOustFromMPCC

    /// Java `RequestExOustFromMPCC.runImpl`.
    pub(crate) fn on_request_ex_oust_from_mpcc(&mut self, player: ObjectId, name: &str) {
        if !self.players.contains_key(&player) {
            return;
        }
        let target = self.player_by_name(name);
        let party = self.party_of(player);
        let allowed = match (target, party) {
            (Some(target), Some(party)) => {
                let target_party = self.party_of(target);
                let channel = self.command_channel_of_party(party);
                let target_channel = target_party.and_then(|p| self.command_channel_of_party(p));
                match (channel, target_channel, target_party) {
                    (Some(channel), Some(target_channel), Some(target_party))
                        if self.is_command_channel_leader(channel, player)
                            && self.command_channel_leader(channel)
                                == self.command_channel_leader(target_channel) =>
                    {
                        Some((target, party, channel, target_party))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let Some((target, party, channel, target_party)) = allowed else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YourTargetCannotBeFound),
            );
            return;
        };
        if player == target {
            return;
        }

        self.command_channel_remove_party(channel, target_party);
        self.broadcast_party(
            target_party,
            &SystemMessage::new(SystemMessageId::YouWereDismissedFromTheCommandChannel),
        );

        // Check if CC has not been canceled.
        if let Some(channel) = self.command_channel_of_party(party) {
            let leader_name = self
                .party_leader(target_party)
                .map(|l| self.cc_name(l))
                .unwrap_or_default();
            self.broadcast_command_channel(
                channel,
                &SystemMessage::new(SystemMessageId::C1SPartyHasBeenDismissedFromTheCommandChannel)
                    .with(text(&leader_name)),
            );
        }
    }

    // ------------------------------------------------------------------ RequestExMPCCShowPartyMembersInfo

    /// Java `RequestExMPCCShowPartyMembersInfo.runImpl`.
    pub(crate) fn on_request_ex_mpcc_show_party_members_info(
        &mut self,
        player: ObjectId,
        party_leader_id: i32,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        let target = ObjectId(party_leader_id);
        if !self.players.contains_key(&target) {
            return;
        }
        if let Some(party) = self.party_of(target) {
            let packet = self.mpcc_show_party_member_info(party);
            self.send(player, &packet);
        }
    }

    // ------------------------------------------------------------------ user commands

    /// The three channel user commands. Java's handlers answer `true` when they did something; a
    /// `false` has no effect for the client.
    pub(crate) fn user_command_channel(&mut self, player: ObjectId, command: i32) -> bool {
        match command {
            CHANNEL_DELETE_COMMAND => self.user_command_channel_delete(player),
            CHANNEL_LEAVE_COMMAND => self.user_command_channel_leave(player),
            CHANNEL_INFO_COMMAND => self.user_command_channel_info(player),
            _ => false,
        }
    }

    /// Java `ChannelDelete.onCommand`.
    fn user_command_channel_delete(&mut self, player: ObjectId) -> bool {
        let Some(party) = self.party_of(player) else {
            return false;
        };
        let Some(channel) = self.command_channel_of_party(party) else {
            return false;
        };
        if !self.is_party_leader(player) || !self.is_command_channel_leader(channel, player) {
            return false;
        }
        self.broadcast_command_channel(
            channel,
            &SystemMessage::new(SystemMessageId::TheCommandChannelHasBeenDisbanded),
        );
        self.command_channel_disband(channel);
        true
    }

    /// Java `ChannelLeave.onCommand`.
    fn user_command_channel_leave(&mut self, player: ObjectId) -> bool {
        let Some(party) = self.party_of(player).filter(|_| self.is_party_leader(player)) else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanLeaveACommandChannel),
            );
            return false;
        };
        let Some(channel) = self.command_channel_of_party(party) else {
            return false;
        };
        self.command_channel_remove_party(channel, party);
        if let Some(leader) = self.party_leader(party) {
            self.send(
                leader,
                &SystemMessage::new(SystemMessageId::YouHaveQuitTheCommandChannel),
            );
            let name = self.cc_name(leader);
            self.broadcast_command_channel(
                channel,
                &SystemMessage::new(SystemMessageId::C1SPartyHasLeftTheCommandChannel)
                    .with(SystemMessageParam::PlayerName(name)),
            );
        }
        true
    }

    /// Java `ChannelInfo.onCommand`.
    fn user_command_channel_info(&mut self, player: ObjectId) -> bool {
        let Some(channel) = self.command_channel_of(player) else {
            return false;
        };
        let Some(info) = self.multi_party_command_channel_info(channel) else {
            return false;
        };
        self.send(player, &info);
        true
    }

    // ------------------------------------------------------------------ chat

    /// Java `ChatPartyRoomAll.onChat` (type `PARTYROOM_ALL`) and `ChatPartyRoomCommander.onChat`
    /// (`PARTYROOM_COMMANDER`): the party leader / the channel leader speaks to the whole channel.
    pub(crate) fn chat_command_channel(&self, id: ObjectId, chat_type: ChatType, message: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let Some(channel) = self.command_channel_of(id) else {
            return;
        };
        let allowed = match chat_type {
            ChatType::PartyRoomAll => self.is_party_leader(id),
            ChatType::PartyRoomCommander => self.is_command_channel_leader(channel, id),
            _ => false,
        };
        if !allowed {
            return;
        }
        if self.is_chat_banned(id) && self.punishment_settings().ban_chat_channels.contains(&chat_type) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ChattingIsCurrentlyProhibited),
            );
            return;
        }
        let is_gm = self.data.access_level(player.data.access_level).is_gm;
        if self.punishment_settings().jail_disable_chat && self.is_jailed(id) && !is_gm {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ChattingIsCurrentlyProhibited),
            );
            return;
        }
        let packet = CreatureSay {
            sender_object_id: id.0,
            chat_type,
            sender: SenderField::Name(player.data.name.clone()),
            message_id: -1,
            body: SayBody::Text(message.to_owned()),
        };
        // `broadcastCreatureSay(msg, activeChar)`: not to the ones who block the speaker.
        for member in self.command_channel_members(channel) {
            if !self.is_blocked(member, id) {
                self.send(member, &packet);
            }
        }
    }

    // ------------------------------------------------------------------ the loot right of a raid

    /// The raid branch at the top of `Attackable.reduceCurrentHp`: the first command channel
    /// with enough members that attacks a raid boss gets the loot right, and keeps it while
    /// somebody of it hits the boss at least every `RaidLootRightsInterval`.
    pub(crate) fn raid_command_channel_attacked(&mut self, raid: ObjectId, attacker: ObjectId) {
        // `_isRaid && !isMinion()`.
        if !self.is_raid(raid) || self.leader_of(raid).is_some() {
            return;
        }
        let Some(channel) = self.command_channel_of(attacker) else {
            return;
        };
        if !self.meet_raid_war_condition(channel, raid) {
            return;
        }
        let now = self.world_ms();
        let leader = self.command_channel_leader(channel).unwrap_or(attacker);
        match self.party.channels.raid_rights.get(&raid).copied() {
            // Looting right isn't set.
            None => {
                let due = self.tick + TimerWheel::ms_to_ticks(CHANNEL_TIMER_MS);
                let timer = self.timers.schedule_at(due, TimerKind::CommandChannelTimer(raid));
                self.party.channels.raid_rights.insert(
                    raid,
                    RaidLootRight {
                        channel,
                        leader,
                        last_attack_ms: now,
                        timer: Some(timer),
                    },
                );
                // TODO(java): "retail msg".
                let packet = CreatureSay {
                    sender_object_id: 0,
                    chat_type: ChatType::PartyRoomAll,
                    sender: SenderField::Name(String::new()),
                    message_id: -1,
                    body: SayBody::Text("You have looting rights!".to_owned()),
                };
                self.broadcast_command_channel(channel, &packet);
            }
            // Is in the same channel: update the last attack time. `CommandChannel.equals` is
            // the same leader.
            Some(right) if self.right_leader(&right) == leader => {
                if let Some(r) = self.party.channels.raid_rights.get_mut(&raid) {
                    r.last_attack_ms = now;
                    r.leader = leader;
                }
            }
            Some(_) => {}
        }
    }

    /// `_firstCommandChannelAttacked.getLeaderObjectId()`: the channel's leader, or the last one
    /// seen when the channel is gone.
    fn right_leader(&self, right: &RaidLootRight) -> ObjectId {
        self.command_channel_leader(right.channel).unwrap_or(right.leader)
    }

    /// `raid.getFirstCommandChannelAttacked().getLeaderObjectId()`: who `ItemManager.createItem`
    /// names as the owner of the loot of a raid boss. `None` when no channel has the right.
    pub fn raid_loot_owner(&self, raid: ObjectId) -> Option<ObjectId> {
        self.party
            .channels
            .raid_rights
            .get(&raid)
            .map(|right| self.right_leader(right))
    }

    /// Java `CommandChannelTimer.run()`.
    pub(crate) fn on_command_channel_timer(&mut self, raid: ObjectId) {
        let Some(right) = self.party.channels.raid_rights.get(&raid).copied() else {
            return;
        };
        let interval = self.settings.party.loot_raids_privilege_interval_ms;
        // The NPC being gone ends the right too: Java's timer would only run into the interval.
        if !self.npcs.contains_key(&raid) || self.world_ms() - right.last_attack_ms > interval {
            self.party.channels.raid_rights.remove(&raid);
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(CHANNEL_TIMER_MS);
        let timer = self.timers.schedule_at(due, TimerKind::CommandChannelTimer(raid));
        if let Some(r) = self.party.channels.raid_rights.get_mut(&raid) {
            r.timer = Some(timer);
        }
    }
}
