#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Parties (M6-PARTY-01): the invitation, joining, leaving, expelling, the leader change, the
//! party window packets, `PartySpelled`, the party target handlers and the party buffs.
//!
//! The expected packets and messages are read off the Java sources (`Party.java` and the five
//! client packets), not off this code. Java: entity/groups/Party.java,
//! network/clientpackets/{RequestJoinParty,RequestAnswerJoinParty,RequestWithDrawalParty,
//! RequestOustPartyMember,RequestChangePartyLeader}.java.

mod party_scene;

use std::collections::BTreeSet;

use l2_data::skill::EnchantSkillGroups;
use l2_data::{SkillLoadOptions, SkillTemplate, SkillTemplates};
use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    AskJoinParty, ChatType, CreatureSay, JoinParty, PartyDistributionType, PartyMemberPosition,
    PartySmallWindowAdd, PartySmallWindowAll, PartySmallWindowDelete, PartySmallWindowUpdate, PartySpelled,
    SayBody, SenderField, SystemMessageParam,
};
use l2_world::skills::{TargetContext, TargetMessage};
use l2_world::*;
use party_scene::*;

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

/// A scene with three players in a party: Alpha (leader), Bravo, Charlie, all level 10.
fn trio() -> Scene {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.form_party(P1, &[(P2, "Bravo"), (P3, "Charlie")], 0);
    s
}

// ------------------------------------------------------------------ the invitation

/// `RequestJoinParty` asks the target (`AskJoinParty` with the loot rule), the inviter hears
/// "C1 has been invited"; the answer builds the party and tells everybody, in Java's order.
#[test]
fn an_invitation_asks_and_the_answer_makes_the_party() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    // The name is matched ignoring case.
    s.invite(P1, "bRAVO", 3);
    let sent = s.log.take();
    let invited = messages(&sent, P1);
    assert_eq!(invited.len(), 1);
    assert_eq!(invited[0].id, SystemMessageId::C1HasBeenInvitedToTheParty);
    assert_eq!(invited[0].params, vec![text("Bravo")]);
    let ask: AskJoinParty = decode(of(&sent, P2, "ASK_JOIN_PARTY")[0]);
    assert_eq!(ask.requestor_name, "Alpha");
    assert_eq!(ask.distribution_type, PartyDistributionType::ByTurn);
    // No party yet: it is made when the answer is yes.
    assert!(s.party_of(P1).is_none());

    s.answer(P2, 1);
    let sent = s.log.take();
    // The inviter: `JoinParty(1)` first, then the new member's messages and window entry.
    let to_leader = names(&sent, P1);
    assert_eq!(to_leader[0], "JOIN_PARTY");
    let join: JoinParty = decode(of(&sent, P1, "JOIN_PARTY")[0]);
    assert_eq!(join.response, 1);
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::C1HasJoinedTheParty],
        "the leader hears who joined"
    );
    assert_eq!(messages(&sent, P1)[0].params, vec![text("Bravo")]);
    let add: PartySmallWindowAdd = decode(of(&sent, P1, "PARTY_SMALL_WINDOW_ADD")[0]);
    assert_eq!(add.leader_object_id, P1);
    assert_eq!(add.distribution_type, PartyDistributionType::ByTurn);
    assert_eq!(add.member.object_id, P2);
    assert_eq!(add.member.name, "Bravo");
    assert_eq!((add.member.hp, add.member.max_hp), (1000, 1000));
    assert_eq!((add.member.mp, add.member.max_mp), (500, 500));
    assert_eq!((add.member.cp, add.member.max_cp), (200, 200));
    assert_eq!((add.member.level, add.member.class_id), (10, 0));

    // The new member: the whole window (everybody but itself), then "You have joined".
    let to_new = names(&sent, P2);
    assert_eq!(to_new[0], "PARTY_SMALL_WINDOW_ALL");
    let all: PartySmallWindowAll = decode(of(&sent, P2, "PARTY_SMALL_WINDOW_ALL")[0]);
    assert_eq!(all.leader_object_id, P1);
    assert_eq!(all.distribution_type, PartyDistributionType::ByTurn);
    assert_eq!(all.members.len(), 1);
    assert_eq!(all.members[0].vitals.object_id, P1);
    assert_eq!(all.members[0].vitals.name, "Alpha");
    assert!(all.members[0].summon.is_none());
    let joined = &messages(&sent, P2)[0];
    assert_eq!(joined.id, SystemMessageId::YouHaveJoinedS1SParty);
    assert_eq!(joined.params, vec![text("Alpha")]);

    assert_eq!(s.members(P1), vec![P1, P2]);
    assert_eq!(s.members(P2), vec![P1, P2]);
    let party = s.party_of(P1).unwrap();
    assert_eq!(s.world.party_leader(party), Some(id(P1)));
    assert_eq!(
        s.world.party_distribution_type(party),
        Some(PartyDistributionType::ByTurn)
    );
    assert_eq!(s.world.party_level(party), Some(10));
    assert!(s.world.is_party_leader(id(P1)));
    assert!(!s.world.is_party_leader(id(P2)));
    // The request is over: both can be invited again.
    s.enter(P3, "Charlie");
    s.invite(P1, "Charlie", 0);
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::C1HasBeenInvitedToTheParty]
    );
}

/// The party icons: `updateEffectIcons(true)` of every member sends `PartySpelled` to the party
/// 300 ms later. The positions go out 6 s after the party formed, then every 12 s.
#[test]
fn the_party_gets_spelled_icons_and_positions_on_the_wheel() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.invite(P1, "Bravo", 0);
    s.answer(P2, 1);
    s.log.take();

    s.ticks(6);
    let sent = s.log.take();
    for to in [P1, P2] {
        let spelled: Vec<PartySpelled> = of(&sent, to, "PARTY_SPELLED")
            .iter()
            .map(|raw| decode(raw))
            .collect();
        let owners: BTreeSet<i32> = spelled.iter().map(|p| p.creature_id).collect();
        assert_eq!(owners, BTreeSet::from([P1, P2]), "member {to:x}");
        assert!(
            spelled
                .iter()
                .all(|p| p.creature_type == PartySpelled::PLAYER && p.effects.is_empty())
        );
        // The icons are for the party only: no `AbnormalStatusUpdate` of the member's own list.
        assert!(of(&sent, to, "ABNORMAL_STATUS_UPDATE").is_empty());
    }

    // 6 s = 120 ticks after the join (6 ticks have gone).
    s.ticks(113);
    assert!(of(&s.log.take(), P1, "PARTY_MEMBER_POSITION").is_empty());
    s.ticks(1);
    let sent = s.log.take();
    let position: PartyMemberPosition = decode(of(&sent, P1, "PARTY_MEMBER_POSITION")[0]);
    assert_eq!(
        position
            .locations
            .iter()
            .map(|l| (l.object_id, l.x, l.y, l.z))
            .collect::<Vec<_>>(),
        vec![(P1, X, Y, Z), (P2, X, Y, Z)]
    );
    assert_eq!(of(&sent, P2, "PARTY_MEMBER_POSITION").len(), 1);
    // Every 12 s = 240 ticks after that.
    s.ticks(239);
    assert!(of(&s.log.take(), P1, "PARTY_MEMBER_POSITION").is_empty());
    s.ticks(1);
    assert_eq!(of(&s.log.take(), P1, "PARTY_MEMBER_POSITION").len(), 1);
}

/// Declining, and the client that refuses party requests, tell the inviter and end the request.
#[test]
fn a_decline_or_a_refusing_client_ends_the_request() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.invite(P1, "Bravo", 0);
    s.log.take();
    s.answer(P2, 0);
    let sent = s.log.take();
    let join: JoinParty = decode(of(&sent, P1, "JOIN_PARTY")[0]);
    assert_eq!(join.response, 0);
    assert!(messages(&sent, P1).is_empty(), "the client says it");
    assert!(s.party_of(P1).is_none() && s.party_of(P2).is_none());
    // Answering again finds no request.
    s.answer(P2, 1);
    assert!(s.log.take().is_empty());
    assert!(s.party_of(P2).is_none());

    s.invite(P1, "Bravo", 0);
    s.log.take();
    s.answer(P2, -1);
    let sent = s.log.take();
    let join: JoinParty = decode(of(&sent, P1, "JOIN_PARTY")[0]);
    assert_eq!(join.response, -1);
    let msg = &messages(&sent, P1)[0];
    assert_eq!(
        msg.id,
        SystemMessageId::C1IsSetToRefusePartyRequestsAndCannotReceiveAPartyRequest
    );
    assert_eq!(msg.params, vec![SystemMessageParam::PlayerName("Bravo".into())]);
    assert!(s.party_of(P1).is_none());
}

/// The request is a 15 second window (`Player.REQUEST_TIMEOUT`), compared with the game ticks, not a
/// task: 299 ticks later the answer still counts, 300 later it does not.
#[test]
fn an_invitation_runs_out_after_fifteen_seconds() {
    for (wait, party_formed) in [(299, true), (300, false)] {
        let mut s = Scene::new();
        s.enter(P1, "Alpha");
        s.enter(P2, "Bravo");
        s.invite(P1, "Bravo", 0);
        s.ticks(wait);
        s.log.take();
        s.answer(P2, 1);
        assert_eq!(s.party_of(P1).is_some(), party_formed, "after {wait} ticks");
        if !party_formed {
            assert!(s.log.take().is_empty());
            // And the inviter is free to ask again.
            s.invite(P1, "Bravo", 0);
            assert_eq!(
                message_ids(&s.log.take(), P1),
                vec![SystemMessageId::C1HasBeenInvitedToTheParty]
            );
        }
    }
}

/// The refusals of `RequestJoinParty`, in Java's order, and what each one says.
#[test]
fn an_invitation_can_be_refused_in_many_ways() {
    let mut s = Scene::new();
    for (i, name) in ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"]
        .iter()
        .enumerate()
    {
        s.enter(P1 + i32::try_from(i).unwrap(), name);
    }
    let foxtrot = P1 + 5;

    // Nobody of that name.
    s.invite(P1, "Nobody", 0);
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::YouMustFirstSelectAUserToInviteToYourParty]
    );
    // Yourself.
    s.invite(P1, "Alpha", 0);
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::YouHaveInvitedTheWrongTarget]
    );

    // An unknown loot rule: "C1 has been invited" went out, and then nothing.
    s.invite(P1, "Bravo", 9);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::C1HasBeenInvitedToTheParty]
    );
    assert!(of(&sent, P2, "ASK_JOIN_PARTY").is_empty());

    // Charlie and Delta are in a party; Alpha cannot invite them.
    s.invite(P3, "Delta", 0);
    s.answer(P4, 1);
    s.log.take();
    s.invite(P1, "Delta", 0);
    let sent = s.log.take();
    let refused = &messages(&sent, P1)[0];
    assert_eq!(
        refused.id,
        SystemMessageId::C1IsAMemberOfAnotherPartyAndCannotBeInvited
    );
    assert_eq!(refused.params, vec![text("Delta")]);

    // Alpha invites Bravo; while that is open, Alpha cannot ask anybody else, and nobody can ask
    // Bravo.
    s.invite(P1, "Bravo", 0);
    s.log.take();
    s.invite(P1, "Echo", 0);
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::WaitingForAnotherReply]
    );
    s.invite(foxtrot, "Bravo", 0);
    let sent = s.log.take();
    let busy = &messages(&sent, foxtrot)[0];
    assert_eq!(busy.id, SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater);
    assert_eq!(busy.params, vec![text("Bravo")]);

    // Only the leader invites.
    s.answer(P2, 1);
    s.log.take();
    s.invite(P2, "Echo", 0);
    assert_eq!(
        message_ids(&s.log.take(), P2),
        vec![SystemMessageId::OnlyTheLeaderCanGiveOutInvitations]
    );
}

/// A party holds nine: the leader of a full party hears `THE_PARTY_IS_FULL`.
#[test]
fn a_party_of_nine_is_full() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    for i in 2..=9 {
        let id = 0x2000_0000 + i;
        let name = format!("M{i}");
        s.enter(id, &name);
        s.invite(P1, &name, 0);
        s.answer(id, 1);
    }
    assert_eq!(s.members(P1).len(), 9);
    s.enter(0x2000_000A, "Late");
    s.log.take();
    s.invite(P1, "Late", 0);
    // "C1 has been invited" goes out before the party is asked for room (`RequestJoinParty`).
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![
            SystemMessageId::C1HasBeenInvitedToTheParty,
            SystemMessageId::ThePartyIsFull
        ]
    );
    // The place in the party decides the relation bits: leader, then the 8 other places.
    let bits: Vec<i32> = (1..=9)
        .map(|i| s.world.party_relation_bits(id(0x2000_0000 + i), id(P1)))
        .collect();
    assert_eq!(bits, vec![0x30, 0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21]);
    assert_eq!(s.world.party_relation_bits(id(0x2000_000A), id(P1)), 0);
}

/// While a player is asked to join a party, clicking on things is refused
/// (`Action.runImpl`: `getActiveRequester() != null`).
#[test]
fn the_invited_player_cannot_act_until_it_answers() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.invite(P1, "Bravo", 0);
    s.log.take();
    s.input(
        P2,
        ClientInput::Action {
            object_id: P1,
            action_id: 0,
        },
    );
    assert_eq!(names(&s.log.take(), P2), vec!["ACTION_FAIL"]);
    s.answer(P2, 0);
    s.log.take();
    s.input(
        P2,
        ClientInput::Action {
            object_id: P1,
            action_id: 0,
        },
    );
    assert!(names(&s.log.take(), P2).contains(&"MY_TARGET_SELECTED".to_string()));
}

/// A player who logs out in the middle of an invitation takes the request with it.
#[test]
fn a_request_ends_when_its_player_leaves() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.invite(P1, "Bravo", 0);
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P1).unwrap()),
    });
    s.log.take();
    s.answer(P2, 1);
    assert!(s.party_of(P2).is_none());
    assert!(s.log.take().is_empty());
}

// ------------------------------------------------------------------ leaving

/// A member of a bigger party leaves: it hears "You have withdrawn", the others "C1 has left", and
/// everybody's window loses it. The party level is the highest of those who stay.
#[test]
fn a_member_leaves_a_party_of_three() {
    let mut s = Scene::new();
    s.enter_level(P1, "Alpha", 10);
    s.enter_level(P2, "Bravo", 12);
    s.enter_level(P3, "Charlie", 20);
    s.form_party(P1, &[(P2, "Bravo"), (P3, "Charlie")], 0);
    let party = s.party_of(P1).unwrap();
    assert_eq!(s.world.party_level(party), Some(20));

    s.input(P3, ClientInput::WithdrawParty);
    let sent = s.log.take();
    assert_eq!(
        names(&sent, P3),
        vec!["SYSTEM_MESSAGE", "PARTY_SMALL_WINDOW_DELETE_ALL"]
    );
    assert_eq!(
        message_ids(&sent, P3),
        vec![SystemMessageId::YouHaveWithdrawnFromTheParty]
    );
    for to in [P1, P2] {
        assert_eq!(
            names(&sent, to),
            vec!["SYSTEM_MESSAGE", "PARTY_SMALL_WINDOW_DELETE"],
            "member {to:x}"
        );
        let left = &messages(&sent, to)[0];
        assert_eq!(left.id, SystemMessageId::C1HasLeftTheParty);
        assert_eq!(left.params, vec![text("Charlie")]);
        let delete: PartySmallWindowDelete = decode(of(&sent, to, "PARTY_SMALL_WINDOW_DELETE")[0]);
        assert_eq!((delete.object_id, delete.name.as_str()), (P3, "Charlie"));
    }
    assert_eq!(s.members(P1), vec![P1, P2]);
    assert!(s.party_of(P3).is_none());
    assert_eq!(s.world.party_level(party), Some(12), "recalculatePartyLevel");
}

/// With two left, anybody leaving takes the party down (`_members.size() == 2`). Java sends the
/// last member no `PartySmallWindowDeleteAll`: its party is only cleared.
#[test]
fn a_party_of_two_disperses_when_one_leaves() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    s.input(P2, ClientInput::WithdrawParty);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::ThePartyHasDispersed]
    );
    assert_eq!(
        message_ids(&sent, P2),
        vec![SystemMessageId::ThePartyHasDispersed]
    );
    // The leader is removed first: its window closes, and the other one is told to drop it.
    assert_eq!(
        names(&sent, P1),
        vec!["SYSTEM_MESSAGE", "PARTY_SMALL_WINDOW_DELETE_ALL"]
    );
    assert_eq!(
        names(&sent, P2),
        vec!["SYSTEM_MESSAGE", "PARTY_SMALL_WINDOW_DELETE"]
    );
    assert!(s.party_of(P1).is_none() && s.party_of(P2).is_none());
    assert_eq!(s.world.party_count(), 0, "the party is gone from the table");
}

/// The leader leaves a party of three: without `AltLeavePartyLeader` it takes the party down; with
/// it, the next member leads and every window is rebuilt.
#[test]
fn the_leader_leaving_disperses_the_party_unless_the_config_says_otherwise() {
    let mut s = trio();
    s.input(P1, ClientInput::WithdrawParty);
    for who in [P1, P2, P3] {
        assert!(s.party_of(who).is_none(), "member {who:x}");
    }
    assert!(s.log.take().iter().any(|(_, b)| name_of(b) == "SYSTEM_MESSAGE"));

    let mut config = Config::default();
    config.settings.party.alt_leave_party_leader = true;
    let mut s = Scene::with(config);
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.form_party(P1, &[(P2, "Bravo"), (P3, "Charlie")], 0);
    s.input(P1, ClientInput::WithdrawParty);
    let sent = s.log.take();
    assert_eq!(s.members(P2), vec![P2, P3]);
    assert!(s.party_of(P1).is_none());
    for to in [P2, P3] {
        let ids = message_ids(&sent, to);
        assert_eq!(
            ids,
            vec![
                SystemMessageId::C1HasLeftTheParty,
                SystemMessageId::C1HasBecomeThePartyLeader
            ],
            "member {to:x}"
        );
        assert_eq!(messages(&sent, to)[1].params, vec![text("Bravo")]);
        // `broadcastToPartyMembersNewLeader`: the window is rebuilt around the new leader.
        let all: PartySmallWindowAll = decode(of(&sent, to, "PARTY_SMALL_WINDOW_ALL")[0]);
        assert_eq!(all.leader_object_id, P2);
        assert_eq!(all.members.len(), 1);
        assert!(!of(&sent, to, "PARTY_SMALL_WINDOW_DELETE_ALL").is_empty());
    }
}

/// A member who logs out leaves as `DISCONNECTED` (the leader too: the next one leads even without
/// `AltLeavePartyLeader`).
#[test]
fn a_disconnect_removes_the_member_and_passes_the_lead_on() {
    let mut s = trio();
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P1).unwrap()),
    });
    let sent = s.log.take();
    assert_eq!(s.members(P2), vec![P2, P3]);
    for to in [P2, P3] {
        assert_eq!(
            message_ids(&sent, to),
            vec![
                SystemMessageId::C1HasLeftTheParty,
                SystemMessageId::C1HasBecomeThePartyLeader
            ]
        );
    }
    assert!(s.world.is_party_leader(id(P2)));
    // And the last two disperse when one more goes.
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P3).unwrap()),
    });
    assert!(s.party_of(P2).is_none());
    assert!(
        s.log
            .take()
            .iter()
            .any(|(to, b)| *to == P2 && name_of(b) == "SYSTEM_MESSAGE")
    );
}

// ------------------------------------------------------------------ expel and leader

#[test]
fn only_the_leader_expels() {
    let mut s = trio();
    s.input(
        P2,
        ClientInput::OustPartyMember {
            name: "charlie".into(),
        },
    );
    assert!(s.log.take().is_empty());
    assert_eq!(s.members(P1).len(), 3);

    s.input(
        P1,
        ClientInput::OustPartyMember {
            name: "charlie".into(),
        },
    );
    let sent = s.log.take();
    assert_eq!(
        names(&sent, P3),
        vec!["SYSTEM_MESSAGE", "PARTY_SMALL_WINDOW_DELETE_ALL"]
    );
    assert_eq!(
        message_ids(&sent, P3),
        vec![SystemMessageId::YouHaveBeenExpelledFromTheParty]
    );
    for to in [P1, P2] {
        let msg = &messages(&sent, to)[0];
        assert_eq!(msg.id, SystemMessageId::C1WasExpelledFromTheParty);
        assert_eq!(msg.params, vec![text("Charlie")]);
        assert_eq!(
            names(&sent, to),
            vec!["SYSTEM_MESSAGE", "PARTY_SMALL_WINDOW_DELETE"]
        );
    }
    assert_eq!(s.members(P1), vec![P1, P2]);
    // A name that is not in the party does nothing.
    s.input(
        P1,
        ClientInput::OustPartyMember {
            name: "Nobody".into(),
        },
    );
    assert!(s.log.take().is_empty());
}

#[test]
fn the_leader_hands_the_party_over() {
    let mut s = trio();
    // Not the leader: refused, but only when the name is a member.
    s.input(
        P2,
        ClientInput::ChangePartyLeader {
            name: "Charlie".into(),
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), P2),
        vec![SystemMessageId::OnlyTheLeaderOfThePartyCanTransferPartyLeadershipToAnotherPlayer]
    );
    s.input(
        P2,
        ClientInput::ChangePartyLeader {
            name: "Nobody".into(),
        },
    );
    assert!(s.log.take().is_empty());
    // The leader names somebody who is not in the party.
    s.input(
        P1,
        ClientInput::ChangePartyLeader {
            name: "Nobody".into(),
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::YouMayOnlyTransferPartyLeadershipToAnotherMemberOfTheParty]
    );
    // The leader names itself.
    s.input(P1, ClientInput::ChangePartyLeader { name: "alpha".into() });
    assert_eq!(
        message_ids(&s.log.take(), P1),
        vec![SystemMessageId::SlowDownYouAreAlreadyThePartyLeader]
    );
    assert_eq!(s.members(P1), vec![P1, P2, P3]);

    // Charlie takes over: the two places swap (not a rotation).
    s.input(
        P1,
        ClientInput::ChangePartyLeader {
            name: "charlie".into(),
        },
    );
    let sent = s.log.take();
    assert_eq!(s.members(P1), vec![P3, P2, P1]);
    for to in [P1, P2, P3] {
        let msg = &messages(&sent, to)[0];
        assert_eq!(msg.id, SystemMessageId::C1HasBecomeThePartyLeader);
        assert_eq!(msg.params, vec![text("Charlie")]);
        let n = names(&sent, to);
        assert_eq!(
            &n[1..3],
            ["PARTY_SMALL_WINDOW_DELETE_ALL", "PARTY_SMALL_WINDOW_ALL"],
            "member {to:x}"
        );
        let all: PartySmallWindowAll = decode(of(&sent, to, "PARTY_SMALL_WINDOW_ALL")[0]);
        assert_eq!(all.leader_object_id, P3);
        assert_eq!(all.members.len(), 2);
        assert!(all.members.iter().all(|m| m.vitals.object_id != to));
        assert!(n.contains(&"USER_INFO".to_string()), "broadcastUserInfo");
    }
}

// ------------------------------------------------------------------ the window follows the vitals

#[test]
fn the_party_window_follows_hp_changes() {
    let mut s = trio();
    s.world.set_current_hp(id(P2), 640.0, true);
    s.ticks(2);
    let sent = s.log.take();
    for to in [P1, P3] {
        let update: PartySmallWindowUpdate = decode(of(&sent, to, "PARTY_SMALL_WINDOW_UPDATE")[0]);
        assert_eq!(update.member.object_id, P2);
        assert_eq!((update.member.hp, update.member.max_hp), (640, 1000));
    }
    assert!(
        of(&sent, P2, "PARTY_SMALL_WINDOW_UPDATE").is_empty(),
        "the member itself gets the StatusUpdate"
    );
    assert!(!of(&sent, P2, "STATUS_UPDATE").is_empty());
}

// ------------------------------------------------------------------ the target handlers

fn skill(target_type: &str, tags: &str, effects: &str) -> SkillTemplate {
    let xml = format!(
        "<list><skill id=\"1\" levels=\"1\" name=\"T\"><operateType>A1</operateType>\
         <targetType>{target_type}</targetType>{tags}<effects>{effects}</effects></skill></list>"
    );
    let table = SkillTemplates::from_xml(&xml, &EnchantSkillGroups::default(), &SkillLoadOptions::default());
    table.get(1, 1).unwrap().clone()
}

const POSITIVE: &str = "<effectPoint>100</effectPoint>";
const NEGATIVE: &str = "<effectPoint>-100</effectPoint>";

fn cast(
    s: &mut Scene,
    skill: &SkillTemplate,
    caster: i32,
    only_first: bool,
    target: Option<i32>,
) -> (Vec<i32>, Vec<TargetMessage>) {
    let r = s.world.skill_target_list(
        skill,
        id(caster),
        only_first,
        target.map(id),
        &TargetContext::default(),
    );
    (r.targets.iter().map(|t| t.0).collect(), r.messages)
}

/// Alpha leads Bravo and Charlie; Delta (no party) and Echo (a party member 2000 away) are around.
fn party_around() -> Scene {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter_at(P2, "Bravo", 10, (X + 100, Y, Z));
    s.enter_at(P3, "Charlie", 10, (X + 300, Y, Z));
    s.enter_at(P5, "Echo", 10, (X + 2_000, Y, Z));
    s.enter_at(P4, "Delta", 10, (X + 150, Y, Z));
    s.form_party(P1, &[(P2, "Bravo"), (P3, "Charlie"), (P5, "Echo")], 0);
    s
}

/// `PARTY`: the caster first, then the members within the affect range, in party order; a dead
/// one is left out, `onlyFirst` stops after the caster.
#[test]
fn the_party_target_is_the_members_in_range() {
    let mut s = party_around();
    let sk = skill("PARTY", "<affectRange>1000</affectRange>", POSITIVE);
    assert_eq!(cast(&mut s, &sk, P1, false, None).0, vec![P1, P2, P3]);
    assert_eq!(cast(&mut s, &sk, P1, true, None).0, vec![P1]);
    // From Charlie's place, Echo is within 1700.
    assert_eq!(cast(&mut s, &sk, P3, false, None).0, vec![P3, P1, P2]);
    s.world.set_dead(id(P2), true);
    assert_eq!(cast(&mut s, &sk, P1, false, None).0, vec![P1, P3]);
    // Delta has no party: only itself.
    assert_eq!(cast(&mut s, &sk, P4, false, None).0, vec![P4]);
    // A range of 0 means "everywhere" (Skill.addCharacter): Echo joins.
    let wide = skill("PARTY", "<affectRange>0</affectRange>", POSITIVE);
    assert_eq!(cast(&mut s, &wide, P1, false, None).0, vec![P1, P3, P5]);
}

#[test]
fn party_member_takes_a_living_member_of_the_party() {
    let mut s = party_around();
    let sk = skill("PARTY_MEMBER", POSITIVE, "");
    assert_eq!(cast(&mut s, &sk, P1, false, Some(P2)).0, vec![P2]);
    assert_eq!(cast(&mut s, &sk, P1, false, Some(P1)).0, vec![P1]);
    // Delta is a player outside the party: the "unsuitable terms" message.
    let (targets, messages) = cast(&mut s, &sk, P1, false, Some(P4));
    assert!(targets.is_empty());
    assert_eq!(messages, vec![TargetMessage::UnsuitableTerms]);
    // A dead member is refused with the same message.
    s.world.set_dead(id(P2), true);
    let (targets, messages) = cast(&mut s, &sk, P1, false, Some(P2));
    assert!(targets.is_empty());
    assert_eq!(messages, vec![TargetMessage::UnsuitableTerms]);
}

#[test]
fn party_not_me_is_the_other_living_members_in_both_ranges() {
    let mut s = party_around();
    let sk = skill("PARTY_NOTME", "<affectRange>250</affectRange>", POSITIVE);
    // Charlie is 300 away: outside the affect range. Echo is 2000 away: outside AltPartyRange 1500
    // and the range.
    assert_eq!(cast(&mut s, &sk, P1, false, None).0, vec![P2]);
    // An affect range of 0 only has AltPartyRange: Charlie yes, Echo no.
    let sk = skill("PARTY_NOTME", POSITIVE, "");
    assert_eq!(cast(&mut s, &sk, P1, false, None).0, vec![P2, P3]);
    s.world.set_dead(id(P3), true);
    assert_eq!(cast(&mut s, &sk, P1, false, None).0, vec![P2]);
    assert!(
        cast(&mut s, &sk, P4, false, None).0.is_empty(),
        "no party, no targets"
    );
}

#[test]
fn party_other_is_one_member_other_than_the_caster() {
    let mut s = party_around();
    let sk = skill("PARTY_OTHER", POSITIVE, "");
    assert_eq!(cast(&mut s, &sk, P1, false, Some(P2)).0, vec![P2]);
    for target in [None, Some(P1), Some(P4)] {
        let (targets, messages) = cast(&mut s, &sk, P1, false, target);
        assert!(targets.is_empty(), "{target:?}");
        assert_eq!(
            messages,
            vec![TargetMessage::System(SystemMessageId::ThatIsAnIncorrectTarget)]
        );
    }
    // A dead member: an empty list and no message.
    s.world.set_dead(id(P2), true);
    assert_eq!(cast(&mut s, &sk, P1, false, Some(P2)), (vec![], vec![]));
}

/// `TARGET_PARTY`: the party of the *selected* player, the members within the affect range of it.
#[test]
fn target_party_is_the_party_of_the_selected_player() {
    let mut s = party_around();
    let sk = skill("TARGET_PARTY", "<affectRange>500</affectRange>", POSITIVE);
    // Delta selects Charlie: Alpha, Bravo and Charlie are within 500 of Charlie, Echo is not.
    s.world.set_target(id(P4), Some(id(P3)));
    assert_eq!(cast(&mut s, &sk, P4, false, Some(P3)).0, vec![P1, P2, P3]);
}

#[test]
fn command_channel_and_party_clan_take_the_party_in_range() {
    let mut s = party_around();
    let sk = skill(
        "COMMAND_CHANNEL",
        "<affectRange>1000</affectRange><affectLimit>5-0</affectLimit>",
        POSITIVE,
    );
    assert_eq!(cast(&mut s, &sk, P1, false, None).0, vec![P1, P2, P3]);
    // NOTE(java-quirk): `size >= maxTargets` with a limit of 0 stops after the first member.
    let sk = skill(
        "COMMAND_CHANNEL",
        "<affectRange>1000</affectRange><affectLimit>0-0</affectLimit>",
        POSITIVE,
    );
    assert_eq!(cast(&mut s, &sk, P1, false, None).0, vec![P1, P2]);

    let sk = skill("PARTY_CLAN", "<affectRange>1000</affectRange>", POSITIVE);
    let (targets, _) = cast(&mut s, &sk, P1, false, None);
    // The caster first, then the party members around it (Delta is no member).
    assert_eq!(targets[0], P1);
    assert_eq!(
        targets[1..].iter().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([P2, P3])
    );
    assert_eq!(cast(&mut s, &sk, P1, true, None).0, vec![P1]);
    // Without a party or a clan the caster stays alone.
    assert_eq!(cast(&mut s, &sk, P4, false, None).0, vec![P4]);
}

/// Area skills never hit the caster's own party: `checkForAreaOffensiveSkills` and
/// `checkPvpSkill` (a force-attack damage skill is the exception).
#[test]
fn offensive_area_skills_spare_the_party() {
    let mut s = party_around();
    // Delta has karma, so it is a legal target of a debuff.
    let mut delta = player_data(P4, "Delta", 10, (X + 150, Y, Z));
    delta.karma = 100;
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P4).unwrap()),
    });
    s.enter_data(delta);
    let sk = skill("AURA", "<affectRange>1000</affectRange>", NEGATIVE);
    let (targets, _) = cast(&mut s, &sk, P1, false, None);
    assert_eq!(targets, vec![P4], "Bravo and Charlie are in the party");

    // The friendly area skills include the party member even when it stands in a PvP zone, and
    // exclude flagged outsiders.
    let friendly = skill("AURA_FRIENDLY", POSITIVE, "");
    let (targets, _) = cast(&mut s, &friendly, P1, false, None);
    let set: BTreeSet<i32> = targets.into_iter().collect();
    assert!(set.contains(&P2) && set.contains(&P3));
    assert!(!set.contains(&P4), "Delta has karma");
}

// ------------------------------------------------------------------ the party buffs

fn skills(xml_body: &str) -> String {
    format!("<list>{xml_body}</list>")
}

/// `RebalanceHP`: the members in range end up with the same share of their maximum HP.
#[test]
fn rebalance_hp_levels_the_party() {
    let config = Config {
        skills_xml: skills(
            r#"<skill id="1335" levels="1" name="Balance Life"><operateType>A1</operateType>
        <targetType>PARTY</targetType><affectRange>1000</affectRange><effectPoint>100</effectPoint>
        <effects><effect name="RebalanceHP"/></effects></skill>"#,
        ),
        ..Config::default()
    };
    let mut s = Scene::with(config);
    s.enter(P1, "Alpha");
    s.enter_at(P2, "Bravo", 10, (X + 100, Y, Z));
    s.enter_at(P3, "Charlie", 10, (X + 200, Y, Z));
    s.enter_at(P5, "Echo", 10, (X + 3_000, Y, Z));
    s.form_party(P1, &[(P2, "Bravo"), (P3, "Charlie"), (P5, "Echo")], 0);
    s.world.set_current_hp(id(P1), 200.0, true);
    s.world.set_current_hp(id(P2), 800.0, true);
    // Charlie is full; Echo (out of range) is not part of it.
    s.world.set_current_hp(id(P5), 100.0, true);

    s.world.apply_effects(id(P1), id(P1), SkillKey::new(1335, 1));
    // (200 + 800 + 1000) of 3000: two thirds of every maximum.
    for who in [P1, P2, P3] {
        let hp = s.player(who).core.cur_hp;
        assert!((hp - 2000.0 / 3.0).abs() < 1e-9, "member {who:x} has {hp}");
    }
    assert_eq!(s.player(P5).core.cur_hp, 100.0);

    // A caster outside a party gets nothing.
    s.enter(P4, "Delta");
    s.world.set_current_hp(id(P4), 100.0, true);
    s.world.apply_effects(id(P4), id(P4), SkillKey::new(1335, 1));
    assert_eq!(s.player(P4).core.cur_hp, 100.0);
}

/// `TransferDamage`: the caster of the buff takes 90 percent of what the buffed member would
/// take, shared by the other members in range; the member takes the rest.
#[test]
fn transfer_damage_hands_the_blow_to_the_caster() {
    let config = Config {
        skills_xml: skills(
            r#"<skill id="528" levels="1" name="Transfer Pain"><operateType>A2</operateType>
        <targetType>PARTY</targetType><affectRange>1000</affectRange><abnormalType>TRANSFER_DAMAGE</abnormalType>
        <abnormalLevel>1</abnormalLevel><abnormalTime>15</abnormalTime><effectPoint>100</effectPoint>
        <effects><effect name="TransferDamage"><add stat="transDamToPlayer">90</add></effect></effects></skill>"#,
        ),
        ..Config::default()
    };
    let mut s = Scene::with(config);
    s.enter(P1, "Alpha");
    s.enter_at(P2, "Bravo", 10, (X + 100, Y, Z));
    s.enter_at(P3, "Charlie", 10, (X + 200, Y, Z));
    s.form_party(P1, &[(P2, "Bravo"), (P3, "Charlie")], 0);
    // Alpha (the caster) buffs Bravo.
    s.world.apply_effects(id(P1), id(P2), SkillKey::new(528, 1));
    assert_eq!(s.world.transfer_damage_to(id(P2)), Some(id(P1)));

    // A monster hits Bravo for 100: Java passes `min(1000 - 1, 100 * 90 / 100)` = 90 on to Alpha.
    // Two others are in range of Alpha, so Alpha loses 90 / 2 = 45 HP; Bravo takes 100 - 90 = 10.
    let monster = s.spawn_monster();
    s.world.reduce_current_hp(id(P2), 100.0, Some(monster));
    assert_eq!(s.player(P1).core.cur_hp, 1000.0 - 45.0);
    assert_eq!(s.player(P2).core.cur_hp, 1000.0 - 10.0);
    // A member out of the 1000 range of the caster does not transfer.
    s.world.player_mut(id(P2)).unwrap().core.x += 2_000;
    s.world.reduce_current_hp(id(P2), 100.0, Some(monster));
    assert_eq!(s.player(P1).core.cur_hp, 1000.0 - 45.0);
    assert_eq!(s.player(P2).core.cur_hp, 1000.0 - 110.0);
}

/// The buffs of a member show in the party window (`PartySpelled`) 300 ms after they land.
#[test]
fn a_buff_is_shown_to_the_party() {
    let config = Config {
        skills_xml: skills(
            r#"<skill id="1204" levels="1" name="Wind Walk"><operateType>A2</operateType>
        <targetType>SELF</targetType><abnormalType>SPEED_UP</abnormalType><abnormalLevel>1</abnormalLevel>
        <abnormalTime>120</abnormalTime><effectPoint>100</effectPoint>
        <effects><effect name="Buff"><add stat="runSpd">33</add></effect></effects></skill>"#,
        ),
        ..Config::default()
    };
    let mut s = Scene::with(config);
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    s.ticks(10);
    s.log.take();

    s.world.apply_effects(id(P2), id(P2), SkillKey::new(1204, 1));
    s.ticks(7);
    let sent = s.log.take();
    for to in [P1, P2] {
        let spelled: Vec<PartySpelled> = of(&sent, to, "PARTY_SPELLED").iter().map(|r| decode(r)).collect();
        assert_eq!(spelled.len(), 1, "member {to:x}");
        assert_eq!(spelled[0].creature_id, P2);
        assert_eq!(spelled[0].effects.len(), 1);
        assert_eq!(spelled[0].effects[0].display_id, 1204);
        assert_eq!(spelled[0].effects[0].display_level, 1);
    }
    // Only the buffed member's own list got an `AbnormalStatusUpdate`.
    assert!(of(&sent, P1, "ABNORMAL_STATUS_UPDATE").is_empty());
    assert!(!of(&sent, P2, "ABNORMAL_STATUS_UPDATE").is_empty());
}

// ------------------------------------------------------------------ chat and /partyinfo

/// Party chat (`ChatParty`): every member hears it, the speaker too; outsiders do not; without a
/// party the speaker is told so.
#[test]
fn party_chat_reaches_the_members() {
    let mut s = trio();
    s.enter(P4, "Delta");
    s.input(
        P2,
        ClientInput::Say {
            chat_type: 3,
            text: "hello party".into(),
            target: None,
        },
    );
    let sent = s.log.take();
    for to in [P1, P2, P3] {
        let raw = of(&sent, to, "SAY2");
        assert_eq!(raw.len(), 1, "member {to:x}");
        let mut expected = CreatureSay {
            sender_object_id: P2,
            chat_type: ChatType::Party,
            sender: SenderField::Name("Bravo".into()),
            message_id: -1,
            body: SayBody::Text("hello party".into()),
        }
        .to_bytes();
        assert_eq!(raw[0], expected.as_mut_slice());
    }
    assert!(of(&sent, P4, "SAY2").is_empty());

    s.input(
        P4,
        ClientInput::Say {
            chat_type: 3,
            text: "anybody?".into(),
            target: None,
        },
    );
    let sent = s.log.take();
    let told = &messages(&sent, P4)[0];
    assert_eq!(told.params, vec![text("You are not in a party.")]);
    assert!(of(&sent, P1, "SAY2").is_empty());
}

/// `/partyinfo` (user command 81): the title, the loot rule, the leader (for a member), the
/// member count and the closing line.
#[test]
fn partyinfo_lists_the_loot_rule_and_the_leader() {
    let mut s = Scene::new();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.invite(P1, "Bravo", 4);
    s.answer(P2, 1);
    s.log.take();

    s.input(P2, ClientInput::UserCommand { id: 81 });
    let sent = s.log.take();
    let msgs = messages(&sent, P2);
    assert_eq!(
        msgs.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![
            SystemMessageId::PartyInformation,
            SystemMessageId::LootingMethodByTurnIncludingSpoil,
            SystemMessageId::PartyLeaderC1,
            SystemMessageId::S12,
            SystemMessageId::Empty3
        ]
    );
    assert_eq!(
        msgs[2].params,
        vec![SystemMessageParam::PlayerName("Alpha".into())]
    );
    assert_eq!(msgs[3].params, vec![text("Members: 2/9")]);

    // The leader gets no leader line; without a party only the title and the closing line.
    s.input(P1, ClientInput::UserCommand { id: 81 });
    assert_eq!(message_ids(&s.log.take(), P1).len(), 4);
    s.enter(P3, "Charlie");
    s.input(P3, ClientInput::UserCommand { id: 81 });
    assert_eq!(
        message_ids(&s.log.take(), P3),
        vec![SystemMessageId::PartyInformation, SystemMessageId::Empty3]
    );
}
