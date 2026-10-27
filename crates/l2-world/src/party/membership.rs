//! Who is in a party: creating one, joining, leaving, expelling, disbanding, changing the leader,
//! and the position broadcast of a running party.
// Java: entity/groups/Party.java (Party(), addPartyMember, removePartyMember, disbandParty,
//       setLeader, changePartyLeader, getPlayerByName, recalculatePartyLevel,
//       broadcastToPartyMembersNewLeader, the position broadcast task)
// Java: entity/actor/Player.java (joinParty, leaveParty, setParty)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExCloseMpcc, ExOpenMpcc, PartyDistributionType, PartySmallWindowDelete, PartySmallWindowDeleteAll,
    SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use super::{Party, PartyId, PartyMessageType};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `Party.PARTY_POSITION_BROADCAST_INTERVAL`: 12 s, with the first run after half of it.
const POSITION_BROADCAST_MS: u64 = 12_000;

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

impl World {
    fn name_of_player(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// `new Party(leader, type)` followed by `leader.setParty(party)` (what `RequestAnswerJoinParty`
    /// does when the inviter has no party yet).
    pub(crate) fn create_party(
        &mut self,
        leader: ObjectId,
        distribution_type: PartyDistributionType,
    ) -> PartyId {
        let level = self.players.get(&leader).map_or(0, |p| p.core.level);
        let id = self.party.allocate();
        self.party
            .parties
            .insert(id, Party::new(leader, level, distribution_type));
        self.party.of_player.insert(leader, id);
        id
    }

    /// Java `Player.joinParty(party)`: the player's party is set first, then `addPartyMember`.
    pub(crate) fn join_party(&mut self, player: ObjectId, party: PartyId) {
        if !self.party.parties.contains_key(&party) {
            return;
        }
        self.party.of_player.insert(player, party);
        self.add_party_member(party, player);
    }

    /// Java `Party.addPartyMember(player)`.
    pub(crate) fn add_party_member(&mut self, party: PartyId, player: ObjectId) {
        let Some(p) = self.party.parties.get_mut(&party) else {
            return;
        };
        if p.members.contains(&player) {
            return;
        }
        // `finishLootRequest(false)` "cancel on invite".
        let pending_vote = p.loot_change.is_some();
        if pending_vote {
            self.party_finish_loot_request(party, false);
        }
        let Some(p) = self.party.parties.get_mut(&party) else {
            return;
        };
        p.members.push(player);

        // Sends the new member the whole party window.
        if let Some(window) = self.party_small_window_all(player, party) {
            self.send(player, &window);
        }
        // Sends pets/summons of party members.
        for member in self.party_members(party) {
            if let Some(summon) = self.summon_of(member)
                && let Some(packet) = self.party_pet_window_add(summon)
            {
                self.send(player, &packet);
            }
        }
        let leader_name = self
            .party_leader(party)
            .map(|l| self.name_of_player(l))
            .unwrap_or_default();
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveJoinedS1SParty).with(text(&leader_name)),
        );
        let name = self.name_of_player(player);
        self.broadcast_party_except(
            party,
            player,
            &SystemMessage::new(SystemMessageId::C1HasJoinedTheParty).with(text(&name)),
        );
        if let Some(add) = self.party_small_window_add(player, party) {
            self.broadcast_party_except(party, player, &add);
        }
        // If the new member has a summon, everybody sees it in the pet window.
        if let Some(packet) = self.summon_of(player).and_then(|s| self.party_pet_window_add(s)) {
            self.broadcast_party(party, &packet);
        }

        // Adjust the party level.
        let level = self.players.get(&player).map_or(0, |p| p.core.level);
        if let Some(p) = self.party.parties.get_mut(&party)
            && level > p.level
        {
            p.level = level;
        }

        // Update PartySpelled.
        for member in self.party_members(party) {
            // The icons go to the party only; the member's own list is unchanged.
            self.update_effect_icons(member, true);
            let summon = self.summon_of(member);
            self.broadcast_user_info(member);
            if let Some(summon) = summon {
                self.summon_update_effect_icons_plain(summon);
            }
        }

        // Dimensional rift (`partyMemberInvited`): there is none.
        // Open the CCInformationwindow.
        if self.command_channel_of_party(party).is_some() {
            self.send(player, &ExOpenMpcc);
        }

        if self
            .party
            .parties
            .get(&party)
            .is_some_and(|p| p.position_timer.is_none())
        {
            let first = POSITION_BROADCAST_MS / 2;
            let due = self.tick + TimerWheel::ms_to_ticks(first);
            let timer = self.timers.schedule_at(due, TimerKind::PartyPosition(party));
            if let Some(p) = self.party.parties.get_mut(&party) {
                p.position_timer = Some(timer);
            }
        }
    }

    /// The position broadcast of Java's `scheduleAtFixedRate`: every member learns where the
    /// others are, then the next run is armed.
    pub(crate) fn on_party_position_timer(&mut self, party: PartyId) {
        let Some(p) = self.party.parties.get_mut(&party) else {
            return;
        };
        p.position_timer = None;
        let packet = self.party_member_position(party);
        self.broadcast_party(party, &packet);
        let due = self.tick + TimerWheel::ms_to_ticks(POSITION_BROADCAST_MS);
        let timer = self.timers.schedule_at(due, TimerKind::PartyPosition(party));
        if let Some(p) = self.party.parties.get_mut(&party) {
            p.position_timer = Some(timer);
        }
    }

    /// Java `Party.recalculatePartyLevel()`.
    pub(crate) fn recalculate_party_level(&mut self, party: PartyId) {
        let members = self.party_members(party);
        let level = members
            .iter()
            .filter_map(|m| self.players.get(m).map(|p| p.core.level))
            .fold(0, i32::max);
        if let Some(p) = self.party.parties.get_mut(&party) {
            p.level = level;
        }
    }

    /// `Party.getPlayerByName(name)`: a member by name, ignoring case.
    pub(crate) fn party_member_by_name(&self, party: PartyId, name: &str) -> Option<ObjectId> {
        self.party_members(party).into_iter().find(|m| {
            self.players
                .get(m)
                .is_some_and(|p| p.data.name.eq_ignore_ascii_case(name))
        })
    }

    /// `Party.broadcastToPartyMembersNewLeader()`: the window of everybody is rebuilt.
    fn broadcast_to_party_members_new_leader(&mut self, party: PartyId) {
        for member in self.party_members(party) {
            self.send(member, &PartySmallWindowDeleteAll);
            if let Some(window) = self.party_small_window_all(member, party) {
                self.send(member, &window);
            }
            self.broadcast_user_info(member);
        }
    }

    /// Java `Party.removePartyMember(player, type)`.
    pub(crate) fn remove_party_member(&mut self, party: PartyId, player: ObjectId, kind: PartyMessageType) {
        let Some(p) = self.party.parties.get(&party) else {
            return;
        };
        if !p.members.contains(&player) {
            return;
        }
        let is_leader = p.leader() == Some(player);
        let alt_leave = self.settings.party.alt_leave_party_leader;
        // NOTE(java-quirk): the leader of a party that survives its leader (`AltLeavePartyLeader`)
        // still takes the party down when only two are left.
        if !p.disbanding
            && (p.members.len() == 2 || (is_leader && !alt_leave && kind != PartyMessageType::Disconnected))
        {
            self.disband_party(party);
            return;
        }

        if let Some(p) = self.party.parties.get_mut(&party) {
            p.members.retain(|m| *m != player);
        }
        self.recalculate_party_level(party);

        // Festival participants (`SevenSignsFestival.updateParticipants`): no Seven Signs yet.
        if self.is_in_duel(player) {
            self.duel_on_remove_from_party(player);
        }
        // Channeling a player!
        if self.is_channeling(player) && self.has_channelized(player) {
            self.abort_cast(player);
        } else if self.is_channelized(player) {
            self.abort_channelization(player);
        }

        let name = self.name_of_player(player);
        match kind {
            PartyMessageType::Expelled => {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveBeenExpelledFromTheParty),
                );
                self.broadcast_party(
                    party,
                    &SystemMessage::new(SystemMessageId::C1WasExpelledFromTheParty).with(text(&name)),
                );
            }
            PartyMessageType::Left | PartyMessageType::Disconnected => {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::YouHaveWithdrawnFromTheParty),
                );
                self.broadcast_party(
                    party,
                    &SystemMessage::new(SystemMessageId::C1HasLeftTheParty).with(text(&name)),
                );
            }
            PartyMessageType::None => {}
        }

        // UI update.
        self.send(player, &PartySmallWindowDeleteAll);
        self.party.of_player.remove(&player);
        self.broadcast_party(
            party,
            &PartySmallWindowDelete {
                object_id: player.0,
                name,
            },
        );
        if let Some(packet) = self
            .summon_of(player)
            .and_then(|s| self.party_pet_window_delete(s))
        {
            self.broadcast_party(party, &packet);
        }
        // Dimensional rift (`partyMemberExited`): there is none.
        // Close the CCInfoWindow.
        if self.command_channel_of_party(party).is_some() {
            self.send(player, &ExCloseMpcc);
        }

        let remaining = self.party_members(party);
        if is_leader && remaining.len() > 1 && (alt_leave || kind == PartyMessageType::Disconnected) {
            let leader_name = remaining
                .first()
                .map(|l| self.name_of_player(*l))
                .unwrap_or_default();
            self.broadcast_party(
                party,
                &SystemMessage::new(SystemMessageId::C1HasBecomeThePartyLeader).with(text(&leader_name)),
            );
            self.broadcast_to_party_members_new_leader(party);
        } else if remaining.len() == 1 {
            if let Some(channel) = self.command_channel_of_party(party) {
                // Delete the whole command channel when the party who opened the channel is
                // disbanded.
                if self.command_channel_leader(channel) == remaining.first().copied() {
                    self.command_channel_disband(channel);
                } else {
                    self.command_channel_remove_party(channel, party);
                }
            }
            if let Some(leader) = remaining.first() {
                // `leader.setParty(null)`; a leader in a duel leaves it (`onRemoveFromParty`).
                self.party.of_player.remove(leader);
                if self.is_in_duel(*leader) {
                    self.duel_on_remove_from_party(*leader);
                }
            }
            // The loot rule change task is cancelled (not finished: nobody is told).
            if let Some(change) = self
                .party
                .parties
                .get_mut(&party)
                .and_then(|p| p.loot_change.take())
            {
                self.timers.cancel(change.timer);
            }
            if let Some(timer) = self
                .party
                .parties
                .get_mut(&party)
                .and_then(|p| p.position_timer.take())
            {
                self.timers.cancel(timer);
            }
            if let Some(p) = self.party.parties.get_mut(&party) {
                p.members.clear();
            }
        }
        // The party object is garbage in Java once its members are cleared.
        if self
            .party
            .parties
            .get(&party)
            .is_some_and(|p| p.members.is_empty())
        {
            self.party.parties.remove(&party);
        }
    }

    /// Java `Party.disbandParty()`.
    pub(crate) fn disband_party(&mut self, party: PartyId) {
        if let Some(p) = self.party.parties.get_mut(&party) {
            p.disbanding = true;
        }
        self.broadcast_party(party, &SystemMessage::new(SystemMessageId::ThePartyHasDispersed));
        // `CopyOnWriteArrayList`: the loop walks the members the party had when it started.
        for member in self.party_members(party) {
            self.remove_party_member(party, member, PartyMessageType::None);
        }
        // The loop ends with one member left, whom `removePartyMember` cleared; nothing is left
        // in the table.
        self.party.parties.remove(&party);
    }

    /// Java `Player.leaveParty()` (logout): `removePartyMember(this, DISCONNECTED)`, then
    /// `_party = null`.
    pub(crate) fn leave_party(&mut self, player: ObjectId) {
        // The loot rule an inviter chose is only kept until the party exists.
        self.party.chosen_distribution.remove(&player);
        let Some(party) = self.party_of(player) else {
            return;
        };
        self.remove_party_member(party, player, PartyMessageType::Disconnected);
        self.party.of_player.remove(&player);
    }

    /// Java `Party.setLeader(player)` (and `changePartyLeader(name)` through
    /// `getPlayerByName`).
    pub(crate) fn set_party_leader(&mut self, party: PartyId, player: Option<ObjectId>) {
        // `player != null && !player.isInDuel()`: a duelist cannot get the party, and Java says
        // nothing.
        let Some(player) = player.filter(|player| !self.is_in_duel(*player)) else {
            return;
        };
        let Some(p) = self.party.parties.get_mut(&party) else {
            return;
        };
        let Some(index) = p.members.iter().position(|m| *m == player) else {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouMayOnlyTransferPartyLeadershipToAnotherMemberOfTheParty,
                ),
            );
            return;
        };
        if index == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::SlowDownYouAreAlreadyThePartyLeader),
            );
            return;
        }
        // Swap party members.
        let old_leader = p.members[0];
        p.members.swap(0, index);
        let leader_name = self.name_of_player(player);
        self.broadcast_party(
            party,
            &SystemMessage::new(SystemMessageId::C1HasBecomeThePartyLeader).with(text(&leader_name)),
        );
        self.broadcast_to_party_members_new_leader(party);
        if let Some(channel) = self.command_channel_of_party(party)
            && self.is_command_channel_leader(channel, old_leader)
        {
            self.command_channel_set_leader(channel, player);
            let name = self.name_of_player(player);
            self.broadcast_command_channel(
                channel,
                &SystemMessage::new(SystemMessageId::CommandChannelAuthorityHasBeenTransferredToC1)
                    .with(text(&name)),
            );
        }
        // `if (player.isInPartyMatchRoom())`: the party matching room follows the new leader.
        self.party_room_on_party_leader_change(player);
    }
}
