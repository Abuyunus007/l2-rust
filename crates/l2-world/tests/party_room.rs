#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! L-03: the party matching rooms, the loot rule vote, the nickname packets and the GM windows.
//!
//! The expected packets and messages are read off the Java sources, not off this code. Java:
//! entity/groups/matching/*.java, entity/groups/Party.java (requestLootChange and friends),
//! network/clientpackets/{RequestPartyMatch*,Request*PartyRoom,AnswerJoinPartyRoom,
//! RequestListPartyMatchingWaitingRoom,RequestExitPartyMatchingWaitingRoom,
//! RequestPartyLootModification,AnswerPartyLootModification,RequestGiveNickName,
//! RequestChangeNicknameColor,RequestResetNickname,RequestGmList,RequestGMCommand}.java,
//! network/serverpackets/{PartyMatchDetail,ExPartyRoomMember,ExManagePartyRoomMember,
//! ListPartyWating,...}.java.

mod party_scene;

use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, CreatureSay, ExAskJoinPartyRoom, ExAskModifyPartyLooting, ExListPartyMatchingWaitingRoom,
    ExManagePartyRoomMember, ExPartyRoomMember, ExSetPartyLooting, GmHennaInfo, GmViewCharacterInfo,
    GmViewQuestInfo, GmViewSkillInfo, GmViewWarehouseWithdrawList, ListPartyWaiting, NicknameChanged,
    PartyDistributionType, PartyRoomInfo, SayBody, SenderField, SystemMessageParam, UserInfo,
};
use l2_world::*;
use party_scene::*;

const GM: i32 = 0x2000_0010;

fn decode_ex<P: l2_net::Decode>(raw: &[u8]) -> P {
    P::decode(&mut l2_net::PacketReader::new(&raw[3..])).unwrap()
}

fn room_input(s: &mut Scene, who: i32, input: PartyRoomInput) {
    s.input(who, ClientInput::PartyRoom(input));
}

/// `who` creates a room for levels `min` to `max`.
fn create_room(s: &mut Scene, who: i32, title: &str, min: i32, max: i32) {
    room_input(
        s,
        who,
        PartyRoomInput::List {
            room_id: 0,
            members_max: 9,
            min_level: min,
            max_level: max,
            loot: 1,
            title: title.into(),
        },
    );
}

fn join_room(s: &mut Scene, who: i32, room_id: i32) {
    room_input(s, who, PartyRoomInput::Detail { room_id });
}

fn members(s: &Scene, room: i32) -> Vec<i32> {
    s.world
        .party_room_members(room)
        .into_iter()
        .map(|m| m.0)
        .collect()
}

fn opcode_name(op: l2_protocol_game::ServerOpcode) -> String {
    op.name().to_owned()
}

fn packets<'a>(sent: &'a Sent, to: i32, op: l2_protocol_game::ServerOpcode) -> Vec<&'a [u8]> {
    of(sent, to, &opcode_name(op))
}

fn say_match_room(s: &mut Scene, who: i32, message: &str) {
    s.input(
        who,
        ClientInput::Say {
            chat_type: ChatType::PartyMatchRoom.client_id(),
            text: message.into(),
            target: None,
        },
    );
}

// ---------------------------------------------------------------------------------- rooms

#[test]
fn creating_a_room_shows_it_to_the_owner() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    create_room(&mut s, P1, "Cruma", 1, 80);
    let sent = s.log.take();

    // PartyMatchDetail, ExPartyRoomMember(room, 1), the message, then the flag in UserInfo.
    let info: PartyRoomInfo = decode(packets(&sent, P1, l2_protocol_game::ServerOpcode::PartyRoomInfo)[0]);
    assert_eq!(
        (
            info.room_id,
            info.max_members,
            info.min_level,
            info.max_level,
            info.loot_type
        ),
        (1, 9, 1, 80, 1)
    );
    assert_eq!(info.title, "Cruma");
    let list: ExPartyRoomMember =
        decode_ex(packets(&sent, P1, l2_protocol_game::ServerOpcode::ExPartyRoomMember)[0]);
    assert_eq!(list.mode, 1);
    assert_eq!(list.members.len(), 1);
    assert_eq!(list.members[0].name, "Alpha");
    // The owner has role 1.
    assert_eq!(list.members[0].role, 1);
    assert!(message_ids(&sent, P1).contains(&SystemMessageId::YouHaveCreatedAPartyRoom));
    let user_info: UserInfo = decode(packets(&sent, P1, l2_protocol_game::ServerOpcode::UserInfo)[0]);
    assert!(user_info.in_party_match_room);

    assert_eq!(s.world.party_room_of(id(P1)), 1);
    assert!(s.world.is_in_party_match_room(id(P1)));
    assert_eq!(members(&s, 1), vec![P1]);

    // The next room is number 2 (Java `_maxid`).
    s.enter(P2, "Bravo");
    create_room(&mut s, P2, "Second", 1, 80);
    assert_eq!(s.world.party_room_ids(), vec![1, 2]);
}

#[test]
fn a_room_made_by_a_party_member_takes_the_whole_party_in() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    create_room(&mut s, P1, "Party", 1, 80);
    assert_eq!(members(&s, 1), vec![P1, P2]);
    assert_eq!(s.world.party_room_of(id(P2)), 1);

    // A member of the owner's party has role 2.
    s.log.take();
    s.input(
        P1,
        ClientInput::PartyRoom(PartyRoomInput::Config {
            auto: 0,
            location: 0,
            level: 0,
        }),
    );
    let sent = s.log.take();
    let list: ExPartyRoomMember =
        decode_ex(packets(&sent, P1, l2_protocol_game::ServerOpcode::ExPartyRoomMember)[0]);
    assert_eq!(list.mode, 2);
    assert_eq!(
        list.members
            .iter()
            .map(|m| (m.name.as_str(), m.role))
            .collect::<Vec<_>>(),
        vec![("Alpha", 1), ("Bravo", 2)]
    );
}

#[test]
fn the_owner_changes_the_room() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    create_room(&mut s, P1, "Old", 1, 80);
    join_room(&mut s, P2, 1);
    s.log.take();
    room_input(
        &mut s,
        P1,
        PartyRoomInput::List {
            room_id: 1,
            members_max: 4,
            min_level: 5,
            max_level: 60,
            loot: 2,
            title: "New".into(),
        },
    );
    let sent = s.log.take();
    for who in [P1, P2] {
        let info: PartyRoomInfo =
            decode(packets(&sent, who, l2_protocol_game::ServerOpcode::PartyRoomInfo)[0]);
        assert_eq!(
            (
                info.max_members,
                info.min_level,
                info.max_level,
                info.loot_type,
                info.title.as_str()
            ),
            (4, 5, 60, 2, "New")
        );
        assert_eq!(
            message_ids(&sent, who),
            vec![SystemMessageId::ThePartyRoomSInformationHasBeenRevised]
        );
    }
}

#[test]
fn the_room_list_shows_the_rooms_that_fit() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter_level(P2, "Bravo", 10);
    s.enter_level(P3, "Charlie", 50);
    create_room(&mut s, P1, "Low", 1, 20);
    s.log.take();

    // Level 10 fits (limit 0 = filter by the player's level).
    room_input(
        &mut s,
        P2,
        PartyRoomInput::Config {
            auto: 0,
            location: 0,
            level: 0,
        },
    );
    let sent = s.log.take();
    let list: ListPartyWaiting =
        decode(packets(&sent, P2, l2_protocol_game::ServerOpcode::ListPartyWaiting)[0]);
    assert_eq!(list.rooms.len(), 1);
    assert_eq!(list.rooms[0].room_id, 1);
    assert_eq!(list.rooms[0].owner_name, "Alpha");
    assert_eq!(list.rooms[0].members, vec![(0, "Alpha".to_owned())]);
    assert!(s.world.is_party_waiting(id(P2)));

    // Level 50 does not, unless the client asks for every level.
    room_input(
        &mut s,
        P3,
        PartyRoomInput::Config {
            auto: 0,
            location: 0,
            level: 0,
        },
    );
    let sent = s.log.take();
    let list: ListPartyWaiting =
        decode(packets(&sent, P3, l2_protocol_game::ServerOpcode::ListPartyWaiting)[0]);
    assert!(list.rooms.is_empty());
    room_input(
        &mut s,
        P3,
        PartyRoomInput::Config {
            auto: 0,
            location: 0,
            level: 50,
        },
    );
    let sent = s.log.take();
    let list: ListPartyWaiting =
        decode(packets(&sent, P3, l2_protocol_game::ServerOpcode::ListPartyWaiting)[0]);
    assert_eq!(list.rooms.len(), 1);
    // A location the room is not in (the scene has no regions: the room is at 0).
    room_input(
        &mut s,
        P3,
        PartyRoomInput::Config {
            auto: 0,
            location: 5,
            level: 50,
        },
    );
    let sent = s.log.take();
    let list: ListPartyWaiting =
        decode(packets(&sent, P3, l2_protocol_game::ServerOpcode::ListPartyWaiting)[0]);
    assert!(list.rooms.is_empty());
}

#[test]
fn a_party_member_cannot_look_at_the_list() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    room_input(
        &mut s,
        P2,
        PartyRoomInput::Config {
            auto: 0,
            location: 0,
            level: 0,
        },
    );
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P2),
        vec![SystemMessageId::TheListOfPartyRoomsCanOnlyBeViewedByAPersonWhoIsNotPartOfAParty]
    );
    assert!(!s.world.is_party_waiting(id(P2)));
    assert_eq!(names(&sent, P2).last().map(String::as_str), Some("ACTION_FAIL"));
    // The leader may.
    room_input(
        &mut s,
        P1,
        PartyRoomInput::Config {
            auto: 0,
            location: 0,
            level: 0,
        },
    );
    assert!(s.world.is_party_waiting(id(P1)));
}

#[test]
fn a_player_joins_a_room_from_the_list() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter_level(P2, "Bravo", 10);
    s.enter_level(P3, "Charlie", 40);
    create_room(&mut s, P1, "Room", 5, 20);
    room_input(
        &mut s,
        P2,
        PartyRoomInput::Config {
            auto: 0,
            location: 0,
            level: 0,
        },
    );
    s.log.take();

    join_room(&mut s, P2, 1);
    let sent = s.log.take();
    // The joiner gets the window and the members as they were (without itself).
    assert_eq!(
        packets(&sent, P2, l2_protocol_game::ServerOpcode::PartyRoomInfo).len(),
        1
    );
    let list: ExPartyRoomMember =
        decode_ex(packets(&sent, P2, l2_protocol_game::ServerOpcode::ExPartyRoomMember)[0]);
    assert_eq!(list.mode, 0);
    assert_eq!(list.members.len(), 1);
    // The room learns of the new member.
    let manage: ExManagePartyRoomMember =
        decode_ex(packets(&sent, P1, l2_protocol_game::ServerOpcode::ExManagePartyRoomMember)[0]);
    assert_eq!(
        (manage.mode, manage.member.name.as_str(), manage.member.role),
        (0, "Bravo", 0)
    );
    let entered = messages(&sent, P1);
    assert_eq!(entered[0].id, SystemMessageId::C1HasEnteredThePartyRoom);
    // `RequestPartyMatchDetail` adds the name with `addString`.
    assert_eq!(entered[0].params, vec![SystemMessageParam::Text("Bravo".into())]);
    assert_eq!(members(&s, 1), vec![P1, P2]);
    assert_eq!(s.world.party_room_of(id(P2)), 1);
    assert!(!s.world.is_party_waiting(id(P2)));

    // The level does not fit.
    join_room(&mut s, P3, 1);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P3),
        vec![SystemMessageId::YouDoNotMeetTheRequirementsToEnterThatPartyRoom]
    );
    assert_eq!(members(&s, 1), vec![P1, P2]);
    // A room that does not exist is ignored.
    join_room(&mut s, P3, 77);
    assert!(s.log.take().is_empty());
}

#[test]
fn the_owner_ousts_a_member() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    create_room(&mut s, P1, "Room", 1, 80);
    join_room(&mut s, P2, 1);
    join_room(&mut s, P3, 1);
    s.log.take();

    // Not the owner: nothing happens.
    room_input(&mut s, P2, PartyRoomInput::Oust { char_id: P3 });
    assert!(s.log.take().is_empty());

    room_input(&mut s, P1, PartyRoomInput::Oust { char_id: P2 });
    let sent = s.log.take();
    assert_eq!(members(&s, 1), vec![P1, P3]);
    assert_eq!(s.world.party_room_of(id(P2)), 0);
    // The ones left are told (not the one who went): "C1 has left" and ExManagePartyRoomMember(2).
    for who in [P1, P3] {
        let manage: ExManagePartyRoomMember = decode_ex(
            packets(
                &sent,
                who,
                l2_protocol_game::ServerOpcode::ExManagePartyRoomMember,
            )[0],
        );
        assert_eq!((manage.mode, manage.member.name.as_str()), (2, "Bravo"));
        assert_eq!(messages(&sent, who)[0].id, SystemMessageId::C1HasLeftThePartyRoom);
    }
    // The ousted gets the close, the list of rooms and the message, and waits for a room.
    assert_eq!(
        packets(&sent, P2, l2_protocol_game::ServerOpcode::ExClosePartyRoom).len(),
        1
    );
    assert_eq!(
        packets(&sent, P2, l2_protocol_game::ServerOpcode::ListPartyWaiting).len(),
        1
    );
    assert_eq!(
        message_ids(&sent, P2),
        vec![SystemMessageId::YouHaveBeenOustedFromThePartyRoom]
    );
    assert!(s.world.is_party_waiting(id(P2)));
}

#[test]
fn a_member_of_the_owners_party_cannot_be_ousted() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    create_room(&mut s, P1, "Room", 1, 80);
    s.log.take();
    room_input(&mut s, P1, PartyRoomInput::Oust { char_id: P2 });
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::YouCannotDismissAPartyMemberByForce]
    );
    assert_eq!(members(&s, 1), vec![P1, P2]);
}

#[test]
fn a_member_withdraws() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    create_room(&mut s, P1, "Room", 1, 80);
    join_room(&mut s, P2, 1);
    s.log.take();

    room_input(&mut s, P2, PartyRoomInput::Withdraw { room_id: 1 });
    let sent = s.log.take();
    assert_eq!(members(&s, 1), vec![P1]);
    assert_eq!(s.world.party_room_of(id(P2)), 0);
    assert_eq!(
        packets(&sent, P2, l2_protocol_game::ServerOpcode::ExClosePartyRoom).len(),
        1
    );
    assert_eq!(
        message_ids(&sent, P2),
        vec![SystemMessageId::YouHaveExitedThePartyRoom]
    );
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::C1HasLeftThePartyRoom]
    );
}

#[test]
fn a_leaving_owner_hands_the_room_to_the_next_member() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    create_room(&mut s, P1, "Room", 1, 80);
    join_room(&mut s, P2, 1);
    join_room(&mut s, P3, 1);
    s.log.take();

    room_input(&mut s, P1, PartyRoomInput::Withdraw { room_id: 1 });
    let sent = s.log.take();
    assert_eq!(members(&s, 1), vec![P2, P3]);
    // Java `changeLeader`: both members get the two changes and "the leader has changed".
    for who in [P2, P3] {
        let ids = message_ids(&sent, who);
        assert_eq!(ids[0], SystemMessageId::TheLeaderOfThePartyRoomHasChanged);
        assert_eq!(ids[1], SystemMessageId::C1HasLeftThePartyRoom);
        let manage: Vec<ExManagePartyRoomMember> = packets(
            &sent,
            who,
            l2_protocol_game::ServerOpcode::ExManagePartyRoomMember,
        )
        .into_iter()
        .map(decode_ex)
        .collect();
        // New leader mode 1, old leader mode 1, then the departure (mode 2).
        assert_eq!(
            manage
                .iter()
                .map(|m| (m.mode, m.member.name.as_str()))
                .collect::<Vec<_>>(),
            vec![(1, "Bravo"), (1, "Alpha"), (2, "Alpha")]
        );
        assert_eq!(manage[0].member.role, 1);
    }
}

#[test]
fn the_last_member_leaving_closes_the_room() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    create_room(&mut s, P1, "Room", 1, 80);
    s.log.take();
    room_input(&mut s, P1, PartyRoomInput::Withdraw { room_id: 1 });
    let sent = s.log.take();
    assert_eq!(s.world.party_room_count(), 0);
    assert_eq!(s.world.party_room_of(id(P1)), 0);
    // `deleteRoom` closes the window and says disbanded, then the withdraw packet adds its own.
    assert_eq!(
        message_ids(&sent, P1),
        vec![
            SystemMessageId::ThePartyRoomHasBeenDisbanded,
            SystemMessageId::YouHaveExitedThePartyRoom
        ]
    );
    assert_eq!(
        packets(&sent, P1, l2_protocol_game::ServerOpcode::ExClosePartyRoom).len(),
        2
    );
}

#[test]
fn dismissing_closes_the_room_for_everybody() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    create_room(&mut s, P1, "Room", 1, 80);
    join_room(&mut s, P2, 1);
    s.log.take();
    // NOTE(java-quirk): Java does not check that the sender owns the room: Bravo closes it.
    room_input(&mut s, P2, PartyRoomInput::Dismiss { room_id: 1 });
    let sent = s.log.take();
    assert_eq!(s.world.party_room_count(), 0);
    for who in [P1, P2] {
        assert_eq!(
            packets(&sent, who, l2_protocol_game::ServerOpcode::ExClosePartyRoom).len(),
            1
        );
        assert_eq!(
            message_ids(&sent, who),
            vec![SystemMessageId::ThePartyRoomHasBeenDisbanded]
        );
        assert!(!s.world.is_in_party_match_room(id(who)));
        // The flag goes out in a fresh UserInfo.
        let info: UserInfo = decode(packets(&sent, who, l2_protocol_game::ServerOpcode::UserInfo)[0]);
        assert!(!info.in_party_match_room);
    }
}

// ---------------------------------------------------------------------------------- invitations

#[test]
fn an_invitation_to_the_room() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    create_room(&mut s, P1, "Room", 1, 80);
    s.log.take();

    room_input(&mut s, P1, PartyRoomInput::AskJoin { name: "bravo".into() });
    let sent = s.log.take();
    let ask: ExAskJoinPartyRoom =
        decode_ex(packets(&sent, P2, l2_protocol_game::ServerOpcode::ExAskJoinPartyRoom)[0]);
    assert_eq!(ask.requestor_name, "Alpha");

    room_input(&mut s, P2, PartyRoomInput::AnswerJoin { answer: 1 });
    let sent = s.log.take();
    assert_eq!(members(&s, 1), vec![P1, P2]);
    // `AnswerJoinPartyRoom` adds the name with `addPcName`.
    let entered = messages(&sent, P1);
    assert_eq!(entered[0].id, SystemMessageId::C1HasEnteredThePartyRoom);
    assert_eq!(
        entered[0].params,
        vec![SystemMessageParam::PlayerName("Bravo".into())]
    );
}

#[test]
fn an_invitation_that_is_refused_or_cannot_be_made() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    create_room(&mut s, P1, "Room", 1, 80);
    s.log.take();

    room_input(
        &mut s,
        P1,
        PartyRoomInput::AskJoin {
            name: "Nobody".into(),
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::ThatPlayerIsNotOnline]
    );

    room_input(&mut s, P1, PartyRoomInput::AskJoin { name: "Bravo".into() });
    s.log.take();
    // Bravo is busy answering: Charlie's invitation is refused.
    room_input(&mut s, P3, PartyRoomInput::AskJoin { name: "Bravo".into() });
    let sent = s.log.take();
    let busy = messages(&sent, P3);
    assert_eq!(busy[0].id, SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater);
    assert_eq!(
        busy[0].params,
        vec![SystemMessageParam::PlayerName("Bravo".into())]
    );

    room_input(&mut s, P2, PartyRoomInput::AnswerJoin { answer: 0 });
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::TheRecipientOfYourInvitationDidNotAcceptThePartyMatchingInvitation]
    );
    assert_eq!(members(&s, 1), vec![P1]);

    // An answer without an invitation.
    room_input(&mut s, P2, PartyRoomInput::AnswerJoin { answer: 1 });
    assert_eq!(
        message_ids(&s.log.take(), P2),
        vec![SystemMessageId::ThatPlayerIsNotOnline]
    );
}

// ---------------------------------------------------------------------------------- waiting list

#[test]
fn the_waiting_list() {
    let mut s = Scene::new();
    s.enter_level(P1, "Alpha", 30);
    s.enter_level(P2, "Bravo", 20);
    s.enter_level(P3, "Charlie", 60);
    for who in [P2, P3] {
        room_input(
            &mut s,
            who,
            PartyRoomInput::Config {
                auto: 0,
                location: 0,
                level: 0,
            },
        );
    }
    assert_eq!(s.world.party_waiting_players(), vec![id(P2), id(P3)]);
    s.log.take();

    room_input(
        &mut s,
        P1,
        PartyRoomInput::ListWaiting {
            page: 1,
            min_level: 1,
            max_level: 40,
            mode: 1,
        },
    );
    let sent = s.log.take();
    let list: ExListPartyMatchingWaitingRoom = decode_ex(
        packets(
            &sent,
            P1,
            l2_protocol_game::ServerOpcode::ExListPartyMatchingWaitingRoom,
        )[0],
    );
    assert_eq!(list.page, 1);
    assert_eq!(list.members, vec![("Bravo".to_owned(), 0, 20)]);

    // The list skips the one who asks; mode 0 is the empty answer.
    room_input(
        &mut s,
        P2,
        PartyRoomInput::ListWaiting {
            page: 1,
            min_level: 1,
            max_level: 85,
            mode: 1,
        },
    );
    let sent = s.log.take();
    let list: ExListPartyMatchingWaitingRoom = decode_ex(
        packets(
            &sent,
            P2,
            l2_protocol_game::ServerOpcode::ExListPartyMatchingWaitingRoom,
        )[0],
    );
    assert_eq!(list.members, vec![("Charlie".to_owned(), 0, 60)]);
    room_input(
        &mut s,
        P2,
        PartyRoomInput::ListWaiting {
            page: 1,
            min_level: 1,
            max_level: 85,
            mode: 0,
        },
    );
    let sent = s.log.take();
    let list: ExListPartyMatchingWaitingRoom = decode_ex(
        packets(
            &sent,
            P2,
            l2_protocol_game::ServerOpcode::ExListPartyMatchingWaitingRoom,
        )[0],
    );
    assert_eq!((list.page, list.members.len()), (0, 0));

    room_input(&mut s, P3, PartyRoomInput::ExitWaiting);
    assert_eq!(s.world.party_waiting_players(), vec![id(P2)]);
}

// ---------------------------------------------------------------------------------- hooks

#[test]
fn the_room_chat() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    create_room(&mut s, P1, "Room", 1, 80);
    join_room(&mut s, P2, 1);
    s.log.take();

    say_match_room(&mut s, P2, "hello room");
    let sent = s.log.take();
    let expected = CreatureSay {
        sender_object_id: P2,
        chat_type: ChatType::PartyMatchRoom,
        sender: SenderField::Name("Bravo".into()),
        message_id: -1,
        body: SayBody::Text("hello room".into()),
    }
    .to_bytes();
    for who in [P1, P2] {
        assert_eq!(
            packets(&sent, who, l2_protocol_game::ServerOpcode::Say2),
            vec![&expected[..]]
        );
    }
    assert!(packets(&sent, P3, l2_protocol_game::ServerOpcode::Say2).is_empty());

    // Not in a room: nothing.
    say_match_room(&mut s, P3, "anybody?");
    assert!(s.log.take().is_empty());
}

#[test]
fn logging_out_leaves_the_room_and_the_waiting_list() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    create_room(&mut s, P1, "Room", 1, 80);
    join_room(&mut s, P2, 1);
    room_input(
        &mut s,
        P3,
        PartyRoomInput::Config {
            auto: 0,
            location: 0,
            level: 0,
        },
    );
    s.log.take();

    s.input(P3, ClientInput::Logout);
    assert!(s.world.party_waiting_players().is_empty());

    s.input(P2, ClientInput::Logout);
    let sent = s.log.take();
    assert_eq!(members(&s, 1), vec![P1]);
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::C1HasLeftThePartyRoom]
    );

    // The owner alone: the room goes.
    s.input(P1, ClientInput::Logout);
    assert_eq!(s.world.party_room_count(), 0);
}

#[test]
fn a_party_that_grows_brings_its_member_into_the_room() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    create_room(&mut s, P1, "Room", 1, 80);
    s.log.take();
    // Alpha (owner, in the room) invites Bravo (not in the room) into a party.
    s.form_party(P1, &[(P2, "Bravo")], 0);
    assert_eq!(members(&s, 1), vec![P1, P2]);
    assert_eq!(s.world.party_room_of(id(P2)), 1);
    assert!(s.world.is_in_party_match_room(id(P2)));
}

#[test]
fn leaving_the_party_leaves_the_room() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.form_party(P1, &[(P2, "Bravo"), (P3, "Charlie")], 0);
    create_room(&mut s, P1, "Room", 1, 80);
    s.log.take();

    s.input(P2, ClientInput::WithdrawParty);
    let sent = s.log.take();
    assert_eq!(members(&s, 1), vec![P1, P3]);
    assert_eq!(s.world.party_room_of(id(P2)), 0);
    // The window is shown once more (PartyMatchDetail, ExPartyRoomMember(0)) and then closed.
    let order: Vec<String> = names(&sent, P2)
        .into_iter()
        .filter(|n| n.contains("ROOM"))
        .collect();
    assert_eq!(
        order,
        vec!["PARTY_ROOM_INFO", "EX_PARTY_ROOM_MEMBER", "EX_CLOSE_PARTY_ROOM"]
    );
}

#[test]
fn a_new_party_leader_becomes_the_room_owner() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    create_room(&mut s, P1, "Room", 1, 80);
    s.log.take();

    s.input(P1, ClientInput::ChangePartyLeader { name: "Bravo".into() });
    let sent = s.log.take();
    assert_eq!(members(&s, 1), vec![P2, P1]);
    assert!(message_ids(&sent, P1).contains(&SystemMessageId::TheLeaderOfThePartyRoomHasChanged));
}

#[test]
fn the_room_flag_is_in_the_char_info_of_others() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.log.take();
    create_room(&mut s, P1, "Room", 1, 80);
    s.ticks(10);
    let sent = s.log.take();
    // The broadcast of the player's info reaches the ones who see the player too.
    let infos = packets(&sent, P2, l2_protocol_game::ServerOpcode::CharInfo);
    assert!(!infos.is_empty());
    let info: l2_protocol_game::server::CharInfo = decode(infos.last().unwrap());
    assert!(info.in_party_match_room);
}

// ---------------------------------------------------------------------------------- loot vote

fn three_in_a_party() -> Scene {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.form_party(P1, &[(P2, "Bravo"), (P3, "Charlie")], 0);
    s
}

fn ask_loot(s: &mut Scene, who: i32, type_id: i32) {
    s.input(
        who,
        ClientInput::PartyLootModification {
            distribution_type_id: type_id,
        },
    );
}

fn answer_loot(s: &mut Scene, who: i32, answer: i32) {
    s.input(who, ClientInput::AnswerPartyLootModification { answer });
}

fn rule(s: &Scene) -> PartyDistributionType {
    let party = s.party_of(P1).unwrap();
    s.world.party_distribution_type(party).unwrap()
}

#[test]
fn the_loot_vote_passes_when_everybody_agrees() {
    let mut s = three_in_a_party();
    assert_eq!(rule(&s), PartyDistributionType::FindersKeepers);
    ask_loot(&mut s, P1, 2);
    let sent = s.log.take();
    for who in [P2, P3] {
        let ask: ExAskModifyPartyLooting = decode_ex(
            packets(
                &sent,
                who,
                l2_protocol_game::ServerOpcode::ExAskModifyPartyLooting,
            )[0],
        );
        assert_eq!(ask.requestor, "Alpha");
        assert_eq!(ask.distribution_type, PartyDistributionType::RandomIncludingSpoil);
    }
    // The leader does not get the question, but "requesting approval".
    assert!(packets(&sent, P1, l2_protocol_game::ServerOpcode::ExAskModifyPartyLooting).is_empty());
    let approval = messages(&sent, P1);
    assert_eq!(
        approval[0].id,
        SystemMessageId::RequestingApprovalForChangingPartyLootToS1
    );
    assert_eq!(approval[0].params, vec![SystemMessageParam::SystemString(798)]);

    answer_loot(&mut s, P2, 1);
    assert_eq!(rule(&s), PartyDistributionType::FindersKeepers);
    answer_loot(&mut s, P3, 1);
    assert_eq!(rule(&s), PartyDistributionType::RandomIncludingSpoil);
    let sent = s.log.take();
    for who in [P1, P2, P3] {
        let set: ExSetPartyLooting =
            decode_ex(packets(&sent, who, l2_protocol_game::ServerOpcode::ExSetPartyLooting)[0]);
        assert_eq!(
            (set.result, set.distribution_type),
            (1, PartyDistributionType::RandomIncludingSpoil)
        );
        let msg = messages(&sent, who);
        assert_eq!(msg[0].id, SystemMessageId::PartyLootWasChangedToS1);
        assert_eq!(msg[0].params, vec![SystemMessageParam::SystemString(798)]);
    }
}

#[test]
fn one_no_cancels_the_vote() {
    let mut s = three_in_a_party();
    ask_loot(&mut s, P1, 1);
    s.log.take();
    answer_loot(&mut s, P2, 0);
    let sent = s.log.take();
    assert_eq!(rule(&s), PartyDistributionType::FindersKeepers);
    for who in [P1, P2, P3] {
        let set: ExSetPartyLooting =
            decode_ex(packets(&sent, who, l2_protocol_game::ServerOpcode::ExSetPartyLooting)[0]);
        // Result 0 and the rule that stays.
        assert_eq!(
            (set.result, set.distribution_type),
            (0, PartyDistributionType::FindersKeepers)
        );
        assert_eq!(
            message_ids(&sent, who),
            vec![SystemMessageId::PartyLootChangeWasCancelled]
        );
    }
    // The vote is over: a late yes changes nothing.
    answer_loot(&mut s, P3, 1);
    assert_eq!(rule(&s), PartyDistributionType::FindersKeepers);
    assert!(s.log.take().is_empty());
}

#[test]
fn the_vote_runs_out_after_fifteen_seconds() {
    let mut s = three_in_a_party();
    ask_loot(&mut s, P1, 1);
    answer_loot(&mut s, P2, 1);
    s.log.take();
    // 14.5 s: still open.
    s.ticks(290);
    let sent = s.log.take();
    assert!(packets(&sent, P3, l2_protocol_game::ServerOpcode::ExSetPartyLooting).is_empty());
    assert!(!message_ids(&sent, P3).contains(&SystemMessageId::PartyLootChangeWasCancelled));
    s.ticks(11);
    let sent = s.log.take();
    assert_eq!(rule(&s), PartyDistributionType::FindersKeepers);
    assert_eq!(
        message_ids(&sent, P3),
        vec![SystemMessageId::PartyLootChangeWasCancelled]
    );
    // A new vote can start.
    ask_loot(&mut s, P1, 1);
    assert_eq!(
        packets(
            &s.log.take(),
            P2,
            l2_protocol_game::ServerOpcode::ExAskModifyPartyLooting
        )
        .len(),
        1
    );
}

#[test]
fn only_the_leader_asks_and_only_for_another_rule() {
    let mut s = three_in_a_party();
    // A member, the rule the party has, an unknown rule, and a second request while one is open.
    ask_loot(&mut s, P2, 1);
    ask_loot(&mut s, P1, 0);
    ask_loot(&mut s, P1, 9);
    assert!(s.log.take().is_empty());
    ask_loot(&mut s, P1, 1);
    s.log.take();
    ask_loot(&mut s, P1, 3);
    assert!(s.log.take().is_empty());
    // Somebody without a party answering is ignored.
    s.enter(P4, "Delta");
    answer_loot(&mut s, P4, 1);
    assert!(s.log.take().is_empty());
}

#[test]
fn a_new_member_cancels_the_vote() {
    let mut s = three_in_a_party();
    s.enter(P4, "Delta");
    ask_loot(&mut s, P1, 1);
    s.log.take();
    s.invite(P1, "Delta", 0);
    s.answer(P4, 1);
    let sent = s.log.take();
    assert_eq!(rule(&s), PartyDistributionType::FindersKeepers);
    // The members of the party before Delta joined are told of the cancel.
    assert!(message_ids(&sent, P2).contains(&SystemMessageId::PartyLootChangeWasCancelled));
    assert!(!message_ids(&sent, P4).contains(&SystemMessageId::PartyLootChangeWasCancelled));
}

// ---------------------------------------------------------------------------------- nicknames

#[test]
fn reset_nickname() {
    let mut s = Scene::new();
    let mut data = player_data(P1, "Alpha", 10, (X, Y, Z));
    data.title = "Old title".into();
    s.enter_data(data);
    s.enter(P2, "Bravo");
    s.log.take();

    s.input(P1, ClientInput::ResetNickname);
    let sent = s.log.take();
    assert_eq!(s.player(P1).data.title, "");
    // NOTE(java-quirk): the colour is 0xFFFF77, not the default title colour.
    assert_eq!(s.player(P1).data.profile.title_color, 0x00FF_FF77);
    // `broadcastTitleInfo`: UserInfo to the player, NicknameChanged to everybody.
    assert_eq!(
        packets(&sent, P1, l2_protocol_game::ServerOpcode::UserInfo).len(),
        1
    );
    for who in [P1, P2] {
        let changed: NicknameChanged =
            decode(packets(&sent, who, l2_protocol_game::ServerOpcode::NickNameChanged)[0]);
        assert_eq!((changed.object_id, changed.title.as_str()), (P1, ""));
    }
}

#[test]
fn a_noblesse_gives_itself_a_title() {
    let mut s = Scene::new();
    let mut data = player_data(P1, "Alpha", 10, (X, Y, Z));
    data.profile.noble = true;
    s.enter_data(data);
    s.enter(P2, "Bravo");
    s.log.take();

    s.input(
        P1,
        ClientInput::GiveNickName {
            target: "ALPHA".into(),
            title: "The Noble".into(),
        },
    );
    let sent = s.log.take();
    assert_eq!(s.player(P1).data.title, "The Noble");
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::YourTitleHasBeenChanged]
    );
    assert_eq!(
        packets(&sent, P2, l2_protocol_game::ServerOpcode::NickNameChanged).len(),
        1
    );

    // Anybody else needs the clan right.
    s.input(
        P2,
        ClientInput::GiveNickName {
            target: "Bravo".into(),
            title: "Mine".into(),
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), P2),
        vec![SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
    assert_eq!(s.player(P2).data.title, "");

    // A title longer than 21 characters keeps its first 20 (Java `Creature.setTitle`).
    s.input(
        P1,
        ClientInput::GiveNickName {
            target: "Alpha".into(),
            title: "abcdefghijklmnopqrstuv".into(),
        },
    );
    assert_eq!(s.player(P1).data.title, "abcdefghijklmnopqrst");
}

fn clan_scene(level: i32) -> Scene {
    use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
    let mut s = Scene::new();
    let member = |id: i32, name: &str, grade: i32| ClanMemberRecord {
        object_id: id,
        name: name.into(),
        level: 40,
        class_id: 0,
        title: String::new(),
        power_grade: grade,
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
        female: false,
        race: 0,
    };
    s.world.load_clans(
        vec![ClanRecord {
            clan_id: 500,
            data: Some(ClanDataRecord {
                name: Some("Lions".into()),
                level,
                leader_id: P1,
                ..ClanDataRecord::default()
            }),
            members: vec![
                member(P1, "Alpha", 1),
                member(P2, "Bravo", 5),
                member(P3, "Charlie", 5),
            ],
            ..ClanRecord::default()
        }],
        Vec::new(),
    );
    for (who, name) in [(P1, "Alpha"), (P2, "Bravo")] {
        let mut data = player_data(who, name, 40, (X, Y, Z));
        data.profile.clan = Some(PlayerClanState {
            clan_id: 500,
            power_grade: if who == P1 { 1 } else { 5 },
            ..PlayerClanState::default()
        });
        s.enter_data(data);
    }
    s.log.take();
    s
}

#[test]
fn the_clan_leader_gives_a_member_a_title() {
    let mut s = clan_scene(3);
    s.input(
        P1,
        ClientInput::GiveNickName {
            target: "Bravo".into(),
            title: "Scout".into(),
        },
    );
    let sent = s.log.take();
    assert_eq!(s.player(P2).data.title, "Scout");
    assert_eq!(
        message_ids(&sent, P2),
        vec![SystemMessageId::YourTitleHasBeenChanged]
    );
    assert_eq!(
        packets(&sent, P1, l2_protocol_game::ServerOpcode::NickNameChanged).len(),
        1
    );

    // A member of the clan who is offline.
    s.input(
        P1,
        ClientInput::GiveNickName {
            target: "Charlie".into(),
            title: "Scout".into(),
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::ThatPlayerIsNotOnline]
    );
    // Somebody who is not in the clan (the name is case sensitive).
    s.input(
        P1,
        ClientInput::GiveNickName {
            target: "bravo".into(),
            title: "Scout".into(),
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::TheTargetMustBeAClanMember]
    );
    // Bravo has no right of its own.
    s.input(
        P2,
        ClientInput::GiveNickName {
            target: "Alpha".into(),
            title: "Boss".into(),
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), P2),
        vec![SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
}

#[test]
fn a_clan_below_level_three_gives_no_titles() {
    let mut s = clan_scene(2);
    s.input(
        P1,
        ClientInput::GiveNickName {
            target: "Bravo".into(),
            title: "Scout".into(),
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::APlayerCanOnlyBeGrantedATitleIfTheClanIsLevel3OrAbove]
    );
}

#[test]
fn a_title_colour_item() {
    let mut s = Scene::new();
    let mut alpha = player_data(P1, "Alpha", 10, (X, Y, Z));
    alpha.items.push(item(0x3000_0001, 13016, 2));
    s.enter_data(alpha);
    s.enter(P2, "Bravo");
    s.log.take();

    // Using the item opens the window.
    s.input(
        P1,
        ClientInput::UseItem {
            object_id: 0x3000_0001,
            ctrl_pressed: false,
        },
    );
    let sent = s.log.take();
    let window: l2_protocol_game::server::ExRequestChangeNicknameColor = decode_ex(
        packets(
            &sent,
            P1,
            l2_protocol_game::ServerOpcode::ExRequestChangeNicknameColor,
        )[0],
    );
    assert_eq!(window.item_object_id, 0x3000_0001);

    // A colour number out of range, or an item that is not a colour scroll: nothing.
    for (color_num, object_id) in [(10, 0x3000_0001), (-1, 0x3000_0001), (3, 0x3000_0099)] {
        s.input(
            P1,
            ClientInput::ChangeNicknameColor {
                color_num,
                title: "No".into(),
                item_object_id: object_id,
            },
        );
    }
    assert!(s.log.take().is_empty());
    assert_eq!(s.inventory_count(P1, 13016), 2);

    s.input(
        P1,
        ClientInput::ChangeNicknameColor {
            color_num: 3,
            title: "Lilac".into(),
            item_object_id: 0x3000_0001,
        },
    );
    let sent = s.log.take();
    assert_eq!(s.inventory_count(P1, 13016), 1);
    assert_eq!(s.player(P1).data.title, "Lilac");
    // COLORS[3].
    assert_eq!(s.player(P1).data.profile.title_color, 0xFA9AEE);
    assert!(!packets(&sent, P1, l2_protocol_game::ServerOpcode::UserInfo).is_empty());
    let info: UserInfo = decode(
        packets(&sent, P1, l2_protocol_game::ServerOpcode::UserInfo)
            .last()
            .unwrap(),
    );
    assert_eq!((info.title.as_str(), info.title_color), ("Lilac", 0xFA9AEE));
    // `broadcastUserInfo`, not `broadcastTitleInfo`: no NicknameChanged.
    assert!(packets(&sent, P2, l2_protocol_game::ServerOpcode::NickNameChanged).is_empty());
}

// ---------------------------------------------------------------------------------- GM windows

fn gm_scene() -> Scene {
    let mut s = Scene::with(Config {
        skills_xml: r#"<list><skill id="1216" levels="3" name="Test Passive"><operateType>P</operateType></skill></list>"#
            .into(),
        ..Config::default()
    });
    let mut gm = player_data(GM, "Gm", 80, (X, Y, Z));
    gm.access_level = 100;
    s.enter_data(gm);
    let mut target = player_data(P1, "Target", 40, (X, Y, Z));
    target.skills = vec![SkillData {
        skill_id: 1216,
        skill_level: 3,
    }];
    target.items.push(item(0x3000_0001, 1060, 5));
    let mut stored = item(0x3000_0002, 57, 1234);
    stored.loc = "WAREHOUSE".into();
    target.profile.warehouse = WarehouseLoad {
        private: vec![stored],
        ..WarehouseLoad::default()
    };
    s.enter_data(target);
    s.enter(P2, "Plain");
    s.log.take();
    s
}

fn gm_command(s: &mut Scene, who: i32, target: &str, command: i32) {
    s.input(
        who,
        ClientInput::GmCommand {
            target_name: target.into(),
            command,
        },
    );
}

#[test]
fn the_gm_sees_the_status_of_a_player() {
    let mut s = gm_scene();
    gm_command(&mut s, GM, "target", 1);
    let sent = s.log.take();
    assert_eq!(
        names(&sent, GM),
        vec!["GM_VIEW_CHARACTER_INFO".to_owned(), "GM_HENNA_INFO".to_owned()]
    );
    let info: GmViewCharacterInfo = decode(&of(&sent, GM, "GM_VIEW_CHARACTER_INFO")[0]);
    assert_eq!(
        (info.object_id, info.name.as_str(), info.level),
        (P1, "Target", 40)
    );
    assert_eq!(info.max_hp, 4000);
    let henna: GmHennaInfo = decode(&of(&sent, GM, "GM_HENNA_INFO")[0]);
    assert!(henna.dye_ids.is_empty());
}

#[test]
fn the_gm_sees_the_skills_quests_inventory_and_warehouse() {
    let mut s = gm_scene();
    gm_command(&mut s, GM, "Target", 3);
    let sent = s.log.take();
    let skills: GmViewSkillInfo = decode(&of(&sent, GM, "GM_VIEW_SKILL_INFO")[0]);
    assert_eq!(skills.player_name, "Target");
    assert_eq!(skills.skills.len(), 1);
    assert_eq!(
        (skills.skills[0].display_id, skills.skills[0].display_level),
        (1216, 3)
    );

    gm_command(&mut s, GM, "Target", 4);
    let sent = s.log.take();
    let quests: GmViewQuestInfo = decode(&of(&sent, GM, "GM_VIEW_QUEST_INFO")[0]);
    assert_eq!((quests.player_name.as_str(), quests.quests.len()), ("Target", 0));

    gm_command(&mut s, GM, "Target", 5);
    let sent = s.log.take();
    assert_eq!(
        names(&sent, GM),
        vec!["GM_VIEW_ITEM_LIST".to_owned(), "GM_HENNA_INFO".to_owned()]
    );
    let items: l2_protocol_game::server::GmViewItemList = decode(&of(&sent, GM, "GM_VIEW_ITEM_LIST")[0]);
    assert_eq!(items.items.len(), 1);
    assert_eq!(items.items[0].item_id, 1060);

    gm_command(&mut s, GM, "Target", 6);
    let sent = s.log.take();
    let wh: GmViewWarehouseWithdrawList = decode(&of(&sent, GM, "GM_VIEW_WAREHOUSE_WITHDRAW_LIST")[0]);
    assert_eq!(wh.owner_name, "Target");
    assert_eq!(wh.adena, 1234);
    assert_eq!(wh.items.len(), 1);
}

#[test]
fn a_gm_command_needs_a_gm_with_alt_g_and_a_known_player() {
    let mut s = gm_scene();
    // A plain player.
    gm_command(&mut s, P2, "Target", 1);
    assert!(s.log.take().is_empty());
    // An unknown name; a clan view of a name that is no clan.
    gm_command(&mut s, GM, "Nobody", 1);
    gm_command(&mut s, GM, "Nobody", 6);
    // The clan view of a player without a clan shows nothing.
    gm_command(&mut s, GM, "Target", 2);
    // A command that does not exist.
    gm_command(&mut s, GM, "Target", 9);
    assert!(s.log.take().is_empty());
}

#[test]
fn the_gm_list() {
    let mut s = gm_scene();
    // `GmStartupAutoList` is off: the GM is hidden from the public list.
    s.input(P2, ClientInput::GmList);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P2),
        vec![SystemMessageId::ThereAreNoGmsCurrentlyVisibleInThePublicListAsTheyMayBePerformingOtherFunctionsAtTheMoment]
    );
    // A GM sees the hidden ones too.
    s.input(GM, ClientInput::GmList);
    let sent = s.log.take();
    let ids = message_ids(&sent, GM);
    assert_eq!(ids, vec![SystemMessageId::GmList, SystemMessageId::GmC1]);
    assert_eq!(
        messages(&sent, GM)[1].params,
        vec![SystemMessageParam::Text("Gm".into())]
    );
}
