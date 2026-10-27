//! Party matching rooms: the rooms, the waiting list and their client packets.
// Java: entity/groups/matching/{PartyMatchRoom,PartyMatchRoomList,PartyMatchWaitingList}.java
// Java: entity/actor/Player.java (`_partyroom`, `isPartyWaiting`, `isInPartyMatchRoom`, `deleteMe`)
// Java: network/clientpackets/{RequestPartyMatchConfig,RequestPartyMatchList,RequestPartyMatchDetail,
//       RequestOustFromPartyRoom,RequestDismissPartyRoom,RequestWithdrawPartyRoom,
//       RequestAskJoinPartyRoom,AnswerJoinPartyRoom,RequestListPartyMatchingWaitingRoom,
//       RequestExitPartyMatchingWaitingRoom}.java
// Java: network/serverpackets/{PartyMatchDetail,ExPartyRoomMember,ExManagePartyRoomMember,
//       ExClosePartyRoom,ExAskJoinPartyRoom,ListPartyWating,ExListPartyMatchingWaitingRoom}.java
// Java: dist/game/data/scripts/handlers/chat/channels/ChatPartyMatchRoom.java
//
// # Shape
// Java keeps three singletons: the room list (`id -> PartyMatchRoom`, rooms hold `Player`s), the
// waiting list (a set of `Player`s) and `Player._partyroom` (the id of the room, 0 = none). Here
// all three are one [`MatchingWorld`] in the party state of the world; a room holds the object ids
// of its members, the first is the owner.
//
// The rooms live in memory only: Java stores nothing of them.
//
// # Quirks kept (marked `NOTE(java-quirk)` where they happen)
// - Nothing checks who may change a room: `RequestPartyMatchList` with a room id edits any room
//   and `RequestDismissPartyRoom` closes any room.
// - A room has no member limit check when somebody joins; `max_members` is only shown.
// - `ListPartyWating` deletes the rooms that are empty, whose owner left or whose owner points at
//   another room, while it builds the list.
// - A player can be in two rooms (creating a room does not leave the old one); `_partyroom` holds
//   the last one.
//
// # Left out
// - The faction chat filter of `ChatPartyMatchRoom` (`FactionSystemConfig`, the customs milestone).
// - `MPCC` rooms (`RequestExManageMpccRoom` ...): Java has no handler for their packets.

use std::collections::{BTreeMap, HashMap};

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, CreatureSay, ExAskJoinPartyRoom, ExClosePartyRoom, ExListPartyMatchingWaitingRoom,
    ExManagePartyRoomMember, ExPartyRoomMember, ListPartyWaiting, PartyRoomInfo, RoomMember, SayBody,
    SenderField, SystemMessage, SystemMessageParam, WaitingRoom,
};
use l2_util::ObjectId;

use crate::world::World;

/// Java `PartyMatchRoom`.
#[derive(Debug, Clone)]
pub(crate) struct MatchRoom {
    id: i32,
    title: String,
    loot: i32,
    min_level: i32,
    max_level: i32,
    max_members: i32,
    /// Java `_members`: the owner first.
    members: Vec<ObjectId>,
}

impl MatchRoom {
    fn owner(&self) -> Option<ObjectId> {
        self.members.first().copied()
    }
}

/// What the three Java singletons and `Player._partyroom` hold.
#[derive(Debug, Default)]
pub struct MatchingWorld {
    /// Java `PartyMatchRoomList._rooms`, in id order (a `ConcurrentHashMap` of small ints walks
    /// them in that order).
    rooms: BTreeMap<i32, MatchRoom>,
    /// How many rooms were added: Java `_maxid` is 1 plus this.
    rooms_added: i32,
    /// Java `PartyMatchWaitingList._members`, in the order the players joined it. NOTE: a Java
    /// `ConcurrentHashMap.newKeySet()` walks by identity hash, so its order is arbitrary.
    waiting: Vec<ObjectId>,
    /// Java `Player._partyroom`, only the non-zero values.
    room_of: HashMap<ObjectId, i32>,
}

impl MatchingWorld {
    /// Java `PartyMatchRoomList.getMaxId()`: the id the next room gets.
    fn max_id(&self) -> i32 {
        1 + self.rooms_added
    }

    /// Java `PartyMatchRoomList.getPlayerRoom(player)`: the first room (by id) that holds the player.
    fn player_room(&self, player: ObjectId) -> Option<i32> {
        self.rooms
            .values()
            .find(|room| room.members.contains(&player))
            .map(|room| room.id)
    }
}

/// The party matching packets of the client (a [`crate::ClientInput`] variant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyRoomInput {
    /// Java `RequestPartyMatchConfig`.
    Config { auto: i32, location: i32, level: i32 },
    /// Java `RequestPartyMatchList`.
    List {
        room_id: i32,
        members_max: i32,
        min_level: i32,
        max_level: i32,
        loot: i32,
        title: String,
    },
    /// Java `RequestPartyMatchDetail`.
    Detail { room_id: i32 },
    /// Java `RequestOustFromPartyRoom`.
    Oust { char_id: i32 },
    /// Java `RequestDismissPartyRoom`.
    Dismiss { room_id: i32 },
    /// Java `RequestWithdrawPartyRoom`.
    Withdraw { room_id: i32 },
    /// Java `RequestAskJoinPartyRoom`.
    AskJoin { name: String },
    /// Java `AnswerJoinPartyRoom`.
    AnswerJoin { answer: i32 },
    /// Java `RequestListPartyMatchingWaitingRoom`.
    ListWaiting {
        page: i32,
        min_level: i32,
        max_level: i32,
        mode: i32,
    },
    /// Java `RequestExitPartyMatchingWaitingRoom`.
    ExitWaiting,
}

fn name_param(name: &str) -> SystemMessageParam {
    SystemMessageParam::PlayerName(name.to_owned())
}

impl World {
    // ------------------------------------------------------------------ queries

    /// Java `Player.getPartyRoom()`: the id of the room, 0 for none.
    pub fn party_room_of(&self, player: ObjectId) -> i32 {
        self.party.matching.room_of.get(&player).copied().unwrap_or(0)
    }

    /// Java `Player.isInPartyMatchRoom()`: the `UserInfo` and `CharInfo` flag.
    pub fn is_in_party_match_room(&self, player: ObjectId) -> bool {
        self.party_room_of(player) > 0
    }

    /// Java `Player.isPartyWaiting()`: the player is on the waiting list.
    pub fn is_party_waiting(&self, player: ObjectId) -> bool {
        self.party.matching.waiting.contains(&player)
    }

    /// The members of a room, the owner first. Empty for an unknown room.
    pub fn party_room_members(&self, room: i32) -> Vec<ObjectId> {
        self.party
            .matching
            .rooms
            .get(&room)
            .map(|r| r.members.clone())
            .unwrap_or_default()
    }

    /// How many rooms exist.
    pub fn party_room_count(&self) -> usize {
        self.party.matching.rooms.len()
    }

    /// The ids of the rooms, ascending.
    pub fn party_room_ids(&self) -> Vec<i32> {
        self.party.matching.rooms.keys().copied().collect()
    }

    /// The players on the waiting list.
    pub fn party_waiting_players(&self) -> Vec<ObjectId> {
        self.party.matching.waiting.clone()
    }

    fn set_party_room(&mut self, player: ObjectId, room: i32) {
        if room == 0 {
            self.party.matching.room_of.remove(&player);
        } else {
            self.party.matching.room_of.insert(player, room);
        }
    }

    // ------------------------------------------------------------------ packets

    /// Java `PartyMatchRoom.getLocation()`: the bbs id of the region of the owner.
    fn room_location(&self, room: &MatchRoom) -> i32 {
        let Some(owner) = room.owner() else {
            return 0;
        };
        let Some((x, y, _)) = self.position_of(owner) else {
            return 0;
        };
        self.zoning
            .map_regions
            .as_ref()
            .map_or(0, |regions| regions.bbs(x, y))
    }

    /// Java `PartyMatchDetail`.
    fn room_info_packet(&self, room: &MatchRoom) -> PartyRoomInfo {
        PartyRoomInfo {
            room_id: room.id,
            max_members: room.max_members,
            min_level: room.min_level,
            max_level: room.max_level,
            loot_type: room.loot,
            location: self.room_location(room),
            title: room.title.clone(),
        }
    }

    /// The entry of `ExPartyRoomMember` / `ExManagePartyRoomMember`: 1 for the owner, 2 for a
    /// member of the owner's party, else 0.
    fn room_member(&self, room: &MatchRoom, player: ObjectId) -> RoomMember {
        let (name, class_id, level) = self.players.get(&player).map_or((String::new(), 0, 0), |p| {
            (p.data.name.clone(), p.data.class_id, p.core.level)
        });
        let owner = room.owner();
        let role = if owner == Some(player) {
            1
        } else if owner.is_some_and(|o| self.is_in_party_with(o, player)) {
            2
        } else {
            0
        };
        RoomMember {
            object_id: player.0,
            name,
            class_id,
            level,
            location: self.room_location(room),
            role,
        }
    }

    /// Java `ExPartyRoomMember(room, mode)`.
    fn room_members_packet(&self, room: &MatchRoom, mode: i32) -> ExPartyRoomMember {
        ExPartyRoomMember {
            mode,
            members: room.members.iter().map(|m| self.room_member(room, *m)).collect(),
        }
    }

    fn manage_member_packet(&self, room: &MatchRoom, player: ObjectId, mode: i32) -> ExManagePartyRoomMember {
        ExManagePartyRoomMember {
            mode,
            member: self.room_member(room, player),
        }
    }

    /// Java `new ListPartyWating(player, auto, location, limit)` and its `writeImpl`: the rooms
    /// that fit the filters. NOTE(java-quirk): building the list deletes the rooms that are
    /// empty, whose owner is offline or whose owner has another `_partyroom`.
    fn list_party_waiting(&mut self, player: ObjectId, location: i32, limit: i32) -> ListPartyWaiting {
        let level = self.players.get(&player).map_or(0, |p| p.core.level);
        let mut rooms = Vec::new();
        let ids: Vec<i32> = self.party.matching.rooms.keys().copied().collect();
        for id in ids {
            let Some(room) = self.party.matching.rooms.get(&id).cloned() else {
                continue;
            };
            let owner = room.owner();
            if owner.is_none_or(|o| !self.players.contains_key(&o) || self.party_room_of(o) != room.id) {
                self.delete_party_room(room.id);
                continue;
            }
            let room_location = self.room_location(&room);
            if location > 0 && location != room_location {
                continue;
            }
            if limit == 0 && (level < room.min_level || level > room.max_level) {
                continue;
            }
            let members = room
                .members
                .iter()
                .map(|m| match self.players.get(m) {
                    Some(p) => (p.data.class_id, p.data.name.clone()),
                    None => (0, "Not Found".to_owned()),
                })
                .collect();
            rooms.push(WaitingRoom {
                room_id: room.id,
                title: room.title.clone(),
                location: room_location,
                min_level: room.min_level,
                max_level: room.max_level,
                max_members: room.max_members,
                owner_name: owner
                    .and_then(|o| self.players.get(&o))
                    .map(|p| p.data.name.clone())
                    .unwrap_or_default(),
                members,
            });
        }
        ListPartyWaiting { rooms }
    }

    // ------------------------------------------------------------------ the room

    /// Java `PartyMatchRoom.notifyMembersAboutExit(player)`: the ones left in the room are told.
    fn room_notify_members_about_exit(&self, room: &MatchRoom, player: ObjectId) {
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        for member in &room.members {
            self.send(
                *member,
                &SystemMessage::new(SystemMessageId::C1HasLeftThePartyRoom)
                    .with(SystemMessageParam::Text(name.clone())),
            );
            self.send(*member, &self.manage_member_packet(room, player, 2));
        }
    }

    /// Java `PartyMatchRoom.changeLeader(newLeader)`.
    pub(crate) fn party_room_change_leader(&mut self, room_id: i32, new_leader: ObjectId) {
        let Some(room) = self.party.matching.rooms.get_mut(&room_id) else {
            return;
        };
        let Some(old_leader) = room.members.first().copied() else {
            return;
        };
        // Remove new leader, move him to the first position, add the old leader as a normal member.
        // NOTE(java-quirk): `_members.remove(newLeader)` takes the first match; when the new leader
        // already is the owner, `set(0, ...)` overwrites the member behind him and the owner is
        // listed twice, and with no other member `set(0, ...)` throws and the room stays empty.
        if let Some(pos) = room.members.iter().position(|m| *m == new_leader) {
            room.members.remove(pos);
        }
        if room.members.is_empty() {
            return;
        }
        room.members[0] = new_leader;
        room.members.push(old_leader);
        let room = room.clone();
        // Broadcast change.
        for member in &room.members {
            self.send(*member, &self.manage_member_packet(&room, new_leader, 1));
            self.send(*member, &self.manage_member_packet(&room, old_leader, 1));
            self.send(
                *member,
                &SystemMessage::new(SystemMessageId::TheLeaderOfThePartyRoomHasChanged),
            );
        }
    }

    /// Java `PartyMatchRoom.deleteMember(player)`.
    pub(crate) fn party_room_delete_member(&mut self, room_id: i32, player: ObjectId) {
        let Some(room) = self.party.matching.rooms.get(&room_id) else {
            return;
        };
        if room.owner() != Some(player) {
            let Some(room) = self.party.matching.rooms.get_mut(&room_id) else {
                return;
            };
            if let Some(pos) = room.members.iter().position(|m| *m == player) {
                room.members.remove(pos);
            }
            let room = room.clone();
            self.room_notify_members_about_exit(&room, player);
        } else if room.members.len() == 1 {
            self.delete_party_room(room_id);
        } else {
            let second = room.members[1];
            self.party_room_change_leader(room_id, second);
            self.party_room_delete_member(room_id, player);
        }
    }

    /// Java `PartyMatchRoomList.deleteRoom(id)`: every member is told and leaves the room.
    pub(crate) fn delete_party_room(&mut self, room_id: i32) {
        let Some(room) = self.party.matching.rooms.get(&room_id).cloned() else {
            return;
        };
        for member in &room.members {
            self.send(*member, &ExClosePartyRoom);
            self.send(
                *member,
                &SystemMessage::new(SystemMessageId::ThePartyRoomHasBeenDisbanded),
            );
            self.set_party_room(*member, 0);
            self.broadcast_user_info(*member);
        }
        self.party.matching.rooms.remove(&room_id);
    }

    /// Java `Player.deleteMe()`, the matching part: off the waiting list, out of the room.
    pub(crate) fn party_room_on_player_leave(&mut self, player: ObjectId) {
        self.party.matching.waiting.retain(|p| *p != player);
        let room = self.party_room_of(player);
        if room != 0 {
            self.party_room_delete_member(room, player);
        }
        self.party.matching.room_of.remove(&player);
    }

    /// `Party.setLeader`: `if (player.isInPartyMatchRoom()) getPlayerRoom(player).changeLeader(player)`.
    pub(crate) fn party_room_on_party_leader_change(&mut self, player: ObjectId) {
        if !self.is_in_party_match_room(player) {
            return;
        }
        if let Some(room) = self.party.matching.player_room(player) {
            self.party_room_change_leader(room, player);
        }
    }

    /// `RequestAnswerJoinParty`, after the new member joined: the room of the inviter learns of it.
    pub(crate) fn party_room_on_party_join(&mut self, requestor: ObjectId, player: ObjectId) {
        let requestor_in = self.is_in_party_match_room(requestor);
        let player_in = self.is_in_party_match_room(player);
        if requestor_in && player_in {
            let matching = &self.party.matching;
            if matching.player_room(requestor) == matching.player_room(player) {
                let Some(room_id) = matching.player_room(requestor) else {
                    return;
                };
                self.party_room_tell_member_changed(room_id, player);
            }
        } else if requestor_in && !player_in {
            let Some(room_id) = self.party.matching.player_room(requestor) else {
                return;
            };
            if let Some(room) = self.party.matching.rooms.get_mut(&room_id) {
                room.members.push(player);
            }
            self.party_room_tell_member_changed(room_id, player);
            self.set_party_room(player, room_id);
            self.broadcast_user_info(player);
        }
    }

    /// The `ExManagePartyRoomMember(player, room, 1)` every member of the room gets.
    fn party_room_tell_member_changed(&self, room_id: i32, player: ObjectId) {
        let Some(room) = self.party.matching.rooms.get(&room_id) else {
            return;
        };
        let packet = self.manage_member_packet(room, player, 1);
        for member in &room.members {
            self.send(*member, &packet);
        }
    }

    /// `RequestWithDrawalParty`, after the player left the party: the player's room window is
    /// shown once more and the player is taken out of the room.
    pub(crate) fn party_room_on_party_withdraw(&mut self, player: ObjectId) {
        if !self.is_in_party_match_room(player) {
            return;
        }
        if let Some(room_id) = self.party.matching.player_room(player)
            && let Some(room) = self.party.matching.rooms.get(&room_id).cloned()
        {
            self.send(player, &self.room_info_packet(&room));
            self.send(player, &self.room_members_packet(&room, 0));
            self.send(player, &ExClosePartyRoom);
            self.party_room_delete_member(room_id, player);
        }
        self.set_party_room(player, 0);
        self.broadcast_user_info(player);
    }

    // ------------------------------------------------------------------ chat

    /// Java `ChatPartyMatchRoom.onChat`: the whole room hears the speaker.
    pub(crate) fn chat_party_match_room(&self, id: ObjectId, message: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if !self.is_in_party_match_room(id) {
            return;
        }
        let Some(room_id) = self.party.matching.player_room(id) else {
            return;
        };
        let chat_type = ChatType::PartyMatchRoom;
        if self.is_chat_banned(id) && self.punishment_settings().ban_chat_channels.contains(&chat_type) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ChattingIsCurrentlyProhibitedIfYouTryToChatBeforeTheProhibitionIsRemovedTheProhibitionTimeWillIncreaseEvenFurther),
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
        // NOTE: unlike the party chat, the block list is not asked here (`member.sendPacket(cs)`).
        if let Some(room) = self.party.matching.rooms.get(&room_id) {
            for member in &room.members {
                self.send(*member, &packet);
            }
        }
    }

    // ------------------------------------------------------------------ the client packets

    pub(crate) fn on_party_room_input(&mut self, id: ObjectId, input: PartyRoomInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            PartyRoomInput::Config {
                auto,
                location,
                level,
            } => self.on_request_party_match_config(id, auto, location, level),
            PartyRoomInput::List {
                room_id,
                members_max,
                min_level,
                max_level,
                loot,
                title,
            } => {
                self.on_request_party_match_list(id, room_id, members_max, min_level, max_level, loot, title)
            }
            PartyRoomInput::Detail { room_id } => self.on_request_party_match_detail(id, room_id),
            PartyRoomInput::Oust { char_id } => self.on_request_oust_from_party_room(id, ObjectId(char_id)),
            PartyRoomInput::Dismiss { room_id } => self.on_request_dismiss_party_room(room_id),
            PartyRoomInput::Withdraw { room_id } => self.on_request_withdraw_party_room(id, room_id),
            PartyRoomInput::AskJoin { name } => self.on_request_ask_join_party_room(id, &name),
            PartyRoomInput::AnswerJoin { answer } => self.on_answer_join_party_room(id, answer),
            PartyRoomInput::ListWaiting {
                page,
                min_level,
                max_level,
                mode,
            } => self.on_request_list_party_matching_waiting_room(id, page, min_level, max_level, mode),
            PartyRoomInput::ExitWaiting => self.party.matching.waiting.retain(|p| *p != id),
        }
    }

    /// Java `RequestPartyMatchConfig.runImpl`.
    fn on_request_party_match_config(&mut self, id: ObjectId, _auto: i32, location: i32, level: i32) {
        let in_room = self.is_in_party_match_room(id);
        if !in_room && self.is_in_party(id) && !self.is_party_leader(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::TheListOfPartyRoomsCanOnlyBeViewedByAPersonWhoIsNotPartOfAParty,
                ),
            );
            self.send(id, &l2_protocol_game::server::ActionFailed);
            return;
        }
        if in_room {
            // If the player is in a room, show him the room, not the list.
            let Some(room_id) = self.party.matching.player_room(id) else {
                return;
            };
            let Some(room) = self.party.matching.rooms.get(&room_id).cloned() else {
                return;
            };
            self.send(id, &self.room_info_packet(&room));
            self.send(id, &self.room_members_packet(&room, 2));
            self.set_party_room(id, room.id);
            self.broadcast_user_info(id);
        } else {
            // Add to the waiting list.
            if !self.party.matching.waiting.contains(&id) {
                self.party.matching.waiting.push(id);
            }
            // Send the room list.
            let list = self.list_party_waiting(id, location, level);
            self.send(id, &list);
        }
    }

    /// Java `RequestPartyMatchList.runImpl`: a room is changed or created.
    #[allow(clippy::too_many_arguments)]
    fn on_request_party_match_list(
        &mut self,
        id: ObjectId,
        room_id: i32,
        members_max: i32,
        min_level: i32,
        max_level: i32,
        loot: i32,
        title: String,
    ) {
        if room_id > 0 {
            // NOTE(java-quirk): any player can change any room by its id.
            let Some(room) = self.party.matching.rooms.get_mut(&room_id) else {
                return;
            };
            room.max_members = members_max;
            room.min_level = min_level;
            room.max_level = max_level;
            room.loot = loot;
            room.title = title;
            let room = room.clone();
            for member in &room.members {
                self.send(*member, &self.room_info_packet(&room));
                self.send(
                    *member,
                    &SystemMessage::new(SystemMessageId::ThePartyRoomSInformationHasBeenRevised),
                );
            }
            return;
        }
        let new_id = self.party.matching.max_id();
        let mut room = MatchRoom {
            id: new_id,
            title,
            loot,
            min_level,
            max_level,
            max_members: members_max,
            members: vec![id],
        };
        // Remove from the waiting list.
        self.party.matching.waiting.retain(|p| *p != id);
        // `addPartyMatchRoom` comes before the party members are added; nothing reads the room
        // in between.
        if let Some(party) = self.party_of(id) {
            for member in self.party_members(party) {
                if member == id {
                    continue;
                }
                self.set_party_room(member, new_id);
                room.members.push(member);
            }
        }
        self.party.matching.rooms.insert(new_id, room.clone());
        self.party.matching.rooms_added += 1;

        self.send(id, &self.room_info_packet(&room));
        self.send(id, &self.room_members_packet(&room, 1));
        self.send(id, &SystemMessage::new(SystemMessageId::YouHaveCreatedAPartyRoom));
        self.set_party_room(id, new_id);
        self.broadcast_user_info(id);
    }

    /// Java `RequestPartyMatchDetail.runImpl` and the joining half of `AnswerJoinPartyRoom`.
    ///
    /// `pc_name`: `AnswerJoinPartyRoom` adds the name with `addPcName`, `RequestPartyMatchDetail`
    /// with `addString`; both are a string on the wire, but the client sees another parameter type.
    fn party_room_join(&mut self, player: ObjectId, room_id: i32, pc_name: bool) {
        let Some(room) = self.party.matching.rooms.get(&room_id).cloned() else {
            return;
        };
        let level = self.players.get(&player).map_or(0, |p| p.core.level);
        if level < room.min_level || level > room.max_level {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotMeetTheRequirementsToEnterThatPartyRoom),
            );
            return;
        }
        // Remove from the waiting list.
        self.party.matching.waiting.retain(|p| *p != player);
        self.set_party_room(player, room_id);
        // NOTE(java-quirk): no member limit is checked and the member list sent to the joiner does
        // not hold the joiner yet.
        self.send(player, &self.room_info_packet(&room));
        self.send(player, &self.room_members_packet(&room, 0));
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let name_param = if pc_name {
            SystemMessageParam::PlayerName(name)
        } else {
            SystemMessageParam::Text(name)
        };
        for member in &room.members {
            self.send(*member, &self.manage_member_packet(&room, player, 0));
            self.send(
                *member,
                &SystemMessage::new(SystemMessageId::C1HasEnteredThePartyRoom).with(name_param.clone()),
            );
        }
        if let Some(room) = self.party.matching.rooms.get_mut(&room_id) {
            room.members.push(player);
        }
        // Info broadcast.
        self.broadcast_user_info(player);
    }

    /// Java `RequestPartyMatchDetail.runImpl`.
    fn on_request_party_match_detail(&mut self, id: ObjectId, room_id: i32) {
        self.party_room_join(id, room_id, false);
    }

    /// Java `RequestOustFromPartyRoom.runImpl`.
    fn on_request_oust_from_party_room(&mut self, id: ObjectId, member: ObjectId) {
        if !self.players.contains_key(&member) {
            return;
        }
        let Some(room_id) = self.party.matching.player_room(member) else {
            return;
        };
        if self.party.matching.rooms.get(&room_id).and_then(MatchRoom::owner) != Some(id) {
            return;
        }
        if self.is_in_party_with(id, member) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotDismissAPartyMemberByForce),
            );
            return;
        }
        // Remove the member from the party room.
        self.party_room_delete_member(room_id, member);
        self.set_party_room(member, 0);
        // Close the party room window.
        self.send(member, &ExClosePartyRoom);
        // Add the player back on the waiting list.
        if !self.party.matching.waiting.contains(&member) {
            self.party.matching.waiting.push(member);
        }
        // Send the room list: the location filter is "TODO: closest town" (0), the level is the
        // member's own, which switches the level filter off.
        let level = self.players.get(&member).map_or(0, |p| p.core.level);
        let list = self.list_party_waiting(member, 0, level);
        self.send(member, &list);
        // Clean the player's LFP title.
        self.broadcast_user_info(member);
        self.send(
            member,
            &SystemMessage::new(SystemMessageId::YouHaveBeenOustedFromThePartyRoom),
        );
    }

    /// Java `RequestDismissPartyRoom.runImpl`.
    fn on_request_dismiss_party_room(&mut self, room_id: i32) {
        // NOTE(java-quirk): nobody checks that the sender owns the room.
        if self.party.matching.rooms.contains_key(&room_id) {
            self.delete_party_room(room_id);
        }
    }

    /// Java `RequestWithdrawPartyRoom.runImpl`.
    fn on_request_withdraw_party_room(&mut self, id: ObjectId, room_id: i32) {
        let Some(owner) = self.party.matching.rooms.get(&room_id).and_then(MatchRoom::owner) else {
            return;
        };
        if self.is_in_party_with(id, owner) {
            // If the user is in the party of the room owner he is not removed from the room.
            self.broadcast_user_info(id);
        } else {
            self.party_room_delete_member(room_id, id);
            self.set_party_room(id, 0);
            self.send(id, &ExClosePartyRoom);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExitedThePartyRoom),
            );
        }
    }

    /// Java `RequestAskJoinPartyRoom.runImpl`.
    fn on_request_ask_join_party_room(&mut self, id: ObjectId, name: &str) {
        let Some(target) = self.player_by_name(name) else {
            self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            return;
        };
        if !self.is_processing_request(target) {
            self.on_transaction_request(id, target);
            let requestor_name = self
                .players
                .get(&id)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            self.send(target, &ExAskJoinPartyRoom { requestor_name });
        } else {
            let target_name = self
                .players
                .get(&target)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater)
                    .with(name_param(&target_name)),
            );
        }
    }

    /// Java `AnswerJoinPartyRoom.runImpl`.
    fn on_answer_join_party_room(&mut self, id: ObjectId, answer: i32) {
        let partner = self.active_requester(id).filter(|p| self.players.contains_key(p));
        let Some(partner) = partner else {
            // The partner was not found: cancel the invitation.
            self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            self.set_active_requester(id, None);
            return;
        };
        // If the answer is positive, join the room of the requester.
        if answer == 1 && !self.is_request_expired(partner) {
            let room_id = self.party_room_of(partner);
            // NOTE(java-quirk): a requester without a room ends the packet here, the request
            // stays open.
            if !self.party.matching.rooms.contains_key(&room_id) {
                return;
            }
            self.party_room_join(id, room_id, true);
        } else {
            self.send(
                partner,
                &SystemMessage::new(
                    SystemMessageId::TheRecipientOfYourInvitationDidNotAcceptThePartyMatchingInvitation,
                ),
            );
        }
        // Reset the transaction timers.
        self.set_active_requester(id, None);
        self.on_transaction_response(partner);
    }

    /// Java `RequestListPartyMatchingWaitingRoom.runImpl` and `ExListPartyMatchingWaitingRoom`.
    fn on_request_list_party_matching_waiting_room(
        &mut self,
        id: ObjectId,
        _page: i32,
        min_level: i32,
        max_level: i32,
        mode: i32,
    ) {
        if mode == 0 {
            self.send(id, &ExListPartyMatchingWaitingRoom::default());
            return;
        }
        let members = self
            .party
            .matching
            .waiting
            .iter()
            .filter(|cha| **cha != id)
            .filter_map(|cha| self.players.get(cha))
            .filter(|p| p.core.level >= min_level && p.core.level <= max_level)
            .map(|p| (p.data.name.clone(), p.data.class_id, p.core.level))
            .collect();
        // The page is always 1 in Java ("Page?").
        self.send(id, &ExListPartyMatchingWaitingRoom { page: 1, members });
    }
}
