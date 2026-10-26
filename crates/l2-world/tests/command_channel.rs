#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Command channels (M6-FU-13): the invitation, joining, the oust, the leader change, a party that
//! breaks up, the user commands, the channel chat, the member windows and the loot right of a
//! raid boss.
//!
//! The expected packets and messages are read off the Java sources, not off this code. Java:
//! entity/groups/CommandChannel.java, entity/actor/tasks/attackable/CommandChannelTimer.java,
//! network/clientpackets/{RequestExAskJoinMPCC,RequestExAcceptJoinMPCC,RequestExOustFromMPCC,
//! RequestExMPCCShowPartyMembersInfo}.java, handlers/chat/commands/user/Channel*.java.

mod party_scene;

use std::sync::Arc;

use l2_data::{NpcLoadOptions, NpcTemplates, SpawnData};
use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, CreatureSay, ExAskJoinMpcc, ExMpccPartyInfoUpdate, ExMpccShowPartyMemberInfo,
    ExMultiPartyCommandChannelInfo, SayBody, SenderField, SystemMessageParam,
};
use l2_world::*;
use party_scene::*;

const P6: i32 = 0x2000_0006;

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

/// The Strategy Guide: whoever has it may open a channel.
const STRATEGY_GUIDE: i32 = 8871;

/// Alpha (with the Strategy Guide) leads Bravo; Charlie leads Delta; Echo leads Foxtrot.
fn three_parties(guide: bool) -> Scene {
    three_parties_with_karma(guide, 0)
}

/// The same, the players of the other parties having `karma` (a legal target for a debuff).
fn three_parties_with_karma(guide: bool, karma: i32) -> Scene {
    let mut s = Scene::new();
    let mut alpha = player_data(P1, "Alpha", 10, (X, Y, Z));
    if guide {
        alpha.items.push(item(0x3000_0001, STRATEGY_GUIDE, 1));
    }
    s.enter_data(alpha);
    s.enter(P2, "Bravo");
    for (who, name, level) in [
        (P3, "Charlie", 20),
        (P4, "Delta", 10),
        (P5, "Echo", 10),
        (P6, "Foxtrot", 10),
    ] {
        let mut data = player_data(who, name, level, (X, Y, Z));
        data.karma = karma;
        s.enter_data(data);
    }
    s.form_party(P1, &[(P2, "Bravo")], 0);
    s.form_party(P3, &[(P4, "Delta")], 0);
    s.form_party(P5, &[(P6, "Foxtrot")], 0);
    s
}

/// Decodes an ex packet: the `0xFE` and the `u16` ex id come first.
fn decode_ex<P: l2_net::Decode>(raw: &[u8]) -> P {
    P::decode(&mut l2_net::PacketReader::new(&raw[3..])).unwrap()
}

fn ask(s: &mut Scene, from: i32, name: &str) {
    s.input(from, ClientInput::AskJoinMpcc { name: name.into() });
}

fn accept(s: &mut Scene, who: i32, response: i32) {
    s.input(who, ClientInput::AcceptJoinMpcc { response });
}

/// Alpha invites the party of `name`, its leader says yes.
fn join(s: &mut Scene, name: &str, leader: i32) {
    ask(s, P1, name);
    accept(s, leader, 1);
}

fn channel(s: &Scene, who: i32) -> Option<CommandChannelId> {
    s.world.command_channel_of(id(who))
}

fn members(s: &Scene, who: i32) -> Vec<i32> {
    channel(s, who)
        .map(|c| {
            s.world
                .command_channel_members(c)
                .into_iter()
                .map(|m| m.0)
                .collect()
        })
        .unwrap_or_default()
}

fn say(s: &mut Scene, who: i32, chat_type: ChatType, message: &str) {
    s.input(
        who,
        ClientInput::Say {
            chat_type: chat_type.client_id(),
            text: message.into(),
            target: None,
        },
    );
}

fn user_command(s: &mut Scene, who: i32, command: i32) {
    s.input(who, ClientInput::UserCommand { id: command });
}

/// The text of the `$s1` messages one client got.
fn plain_texts(sent: &Sent, to: i32) -> Vec<String> {
    messages(sent, to)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::S12)
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------ the invitation

/// `askJoinMPCC`: the target party's leader is asked (a message and `ExAskJoinMPCC`), the
/// requestor is told who was invited.
#[test]
fn the_invitation_goes_to_the_leader_of_the_target_party() {
    let mut s = three_parties(true);
    // Delta is a member, not the leader: Charlie is asked.
    ask(&mut s, P1, "delta");
    let sent = s.log.take();
    let asked = messages(&sent, P3);
    assert_eq!(asked.len(), 1);
    assert_eq!(
        asked[0].id,
        SystemMessageId::C1IsInvitingYouToACommandChannelDoYouAccept
    );
    assert_eq!(asked[0].params, vec![text("Alpha")]);
    let packet: ExAskJoinMpcc = decode_ex(of(&sent, P3, "EX_ASK_JOIN_MPCC")[0]);
    assert_eq!(packet.requestor_name, "Alpha");
    assert_eq!(
        plain_texts(&sent, P1),
        vec!["You invited Charlie to your Command Channel."]
    );
    assert!(of(&sent, P4, "SYSTEM_MESSAGE").is_empty());
    assert!(
        channel(&s, P1).is_none(),
        "the channel is made when the answer is yes"
    );

    // The leader is busy now.
    ask(&mut s, P1, "Echo");
    ask(&mut s, P1, "Charlie");
    let sent = s.log.take();
    let busy = messages(&sent, P1);
    assert!(
        busy.iter()
            .any(|m| m.id == SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater
                && m.params == vec![text("Charlie")])
    );
}

/// The three ways to have the right: the Strategy Guide (8871), a clan leader of level 5 (not
/// here), or pledge class 5 with the Clan Imperium skill (not here). Without any: the message.
#[test]
fn a_channel_needs_the_strategy_guide_or_the_clan() {
    let mut s = three_parties(false);
    ask(&mut s, P1, "Charlie");
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::CommandChannelsCanOnlyBeFormedByAPartyLeaderWhoIsAlsoTheLeaderOfALevel5Clan]
    );
    assert!(of(&sent, P3, "EX_ASK_JOIN_MPCC").is_empty());
}

/// Pledge class 5 (a Baron) and the skill Clan Imperium (391) are the third way.
#[test]
fn clan_imperium_with_pledge_class_five_is_enough() {
    for (pledge_class, skill, allowed) in [(5, true, true), (4, true, false), (5, false, false)] {
        let mut s = Scene::with(Config {
            skills_xml: r#"<list><skill id="391" levels="1" name="Clan Imperium"><operateType>P</operateType></skill></list>"#.into(),
            ..Config::default()
        });
        let mut alpha = player_data(P1, "Alpha", 10, (X, Y, Z));
        alpha.profile.clan = Some(PlayerClanState {
            pledge_class,
            ..PlayerClanState::default()
        });
        if skill {
            alpha.skills.push(SkillData {
                skill_id: 391,
                skill_level: 1,
            });
        }
        s.enter_data(alpha);
        s.enter(P2, "Bravo");
        s.enter(P3, "Charlie");
        s.enter(P4, "Delta");
        s.form_party(P1, &[(P2, "Bravo")], 0);
        s.form_party(P3, &[(P4, "Delta")], 0);
        ask(&mut s, P1, "Charlie");
        let sent = s.log.take();
        assert_eq!(
            of(&sent, P3, "EX_ASK_JOIN_MPCC").len(),
            usize::from(allowed),
            "class {pledge_class}, skill {skill}"
        );
    }
}

/// The checks before the right: only a party leader, a party must exist, the target needs a
/// party that is not in a channel, and yourself is no target.
#[test]
fn who_may_invite_whom() {
    let mut s = three_parties(true);
    // Bravo is no leader.
    ask(&mut s, P2, "Charlie");
    assert_eq!(
        message_ids(&s.log.take(), P2),
        vec![SystemMessageId::YouDoNotHaveAuthorityToInviteSomeoneToTheCommandChannel]
    );
    // The own party, case ignored: nothing at all.
    ask(&mut s, P1, "bravo");
    assert!(s.log.take().is_empty());
    // An unknown name: nothing at all.
    ask(&mut s, P1, "Nobody");
    assert!(s.log.take().is_empty());
    // A player without a party.
    s.enter(0x2000_0007, "Golf");
    ask(&mut s, P1, "Golf");
    assert_eq!(
        plain_texts(&s.log.take(), P1),
        vec!["Golf doesn't have party and cannot be invited to Command Channel."]
    );
    // Somebody without a party asking: nothing at all.
    ask(&mut s, 0x2000_0007, "Charlie");
    assert!(s.log.take().is_empty());

    // A member of a channel who is not its leader.
    join(&mut s, "Charlie", P3);
    s.log.take();
    ask(&mut s, P3, "Echo");
    assert_eq!(
        message_ids(&s.log.take(), P3),
        vec![SystemMessageId::YouDoNotHaveAuthorityToInviteSomeoneToTheCommandChannel]
    );
    // The leader invites a party that is in the channel already.
    ask(&mut s, P1, "Delta");
    let sent = s.log.take();
    let msg = &messages(&sent, P1)[0];
    assert_eq!(
        msg.id,
        SystemMessageId::C1SPartyIsAlreadyAMemberOfTheCommandChannel
    );
    assert_eq!(msg.params, vec![text("Delta")]);
}

/// A "no" tells the requestor in plain text and ends the request.
#[test]
fn a_decline_ends_the_request() {
    let mut s = three_parties(true);
    ask(&mut s, P1, "Charlie");
    s.log.take();
    accept(&mut s, P3, 0);
    let sent = s.log.take();
    assert_eq!(
        plain_texts(&sent, P1),
        vec!["The player declined to join your Command Channel."]
    );
    assert!(channel(&s, P1).is_none());
    // The request is gone: answering again does nothing.
    accept(&mut s, P3, 1);
    assert!(s.log.take().is_empty());
    assert!(channel(&s, P1).is_none());
    // And Alpha can ask again.
    ask(&mut s, P1, "Charlie");
    assert_eq!(of(&s.log.take(), P3, "EX_ASK_JOIN_MPCC").len(), 1);
}

// ------------------------------------------------------------------ joining

/// The answer "yes" makes the channel around Alpha's party and adds Charlie's: the messages and
/// windows in Java's order (including the two "formed" messages of the requestor).
#[test]
fn a_yes_makes_the_channel_and_adds_the_party() {
    let mut s = three_parties(true);
    ask(&mut s, P1, "Charlie");
    s.log.take();
    accept(&mut s, P3, 1);
    let sent = s.log.take();

    // Alpha's party: formed + ExOpenMPCC (the constructor); Alpha alone: formed again.
    assert_eq!(
        names(&sent, P1),
        vec![
            "SYSTEM_MESSAGE",
            "EX_OPEN_MPCC",
            "SYSTEM_MESSAGE",
            "EX_MPCC_PARTY_INFO_UPDATE"
        ]
    );
    assert_eq!(
        message_ids(&sent, P1),
        vec![
            SystemMessageId::TheCommandChannelHasBeenFormed,
            SystemMessageId::TheCommandChannelHasBeenFormed
        ]
    );
    assert_eq!(
        names(&sent, P2),
        vec!["SYSTEM_MESSAGE", "EX_OPEN_MPCC", "EX_MPCC_PARTY_INFO_UPDATE"]
    );
    // The existing members learn about the new party: Charlie, 2 members, mode 1 (add).
    let update: ExMpccPartyInfoUpdate = decode_ex(of(&sent, P1, "EX_MPCC_PARTY_INFO_UPDATE")[0]);
    assert_eq!(
        update,
        ExMpccPartyInfoUpdate {
            leader_name: "Charlie".into(),
            leader_object_id: P3,
            member_count: 2,
            mode: 1
        }
    );
    // The new party: "You have joined" and the window; the answering leader hears it once.
    for who in [P3, P4] {
        assert_eq!(
            names(&sent, who),
            vec!["SYSTEM_MESSAGE", "EX_OPEN_MPCC"],
            "{who:x}"
        );
        assert_eq!(
            message_ids(&sent, who),
            vec![SystemMessageId::YouHaveJoinedTheCommandChannel]
        );
    }
    assert!(names(&sent, P5).is_empty());

    let c = channel(&s, P1).unwrap();
    assert_eq!(channel(&s, P4), Some(c));
    assert_eq!(s.world.command_channel_leader(c), Some(id(P1)));
    assert_eq!(members(&s, P1), vec![P1, P2, P3, P4]);
    // The level of the highest party (Charlie is level 20).
    assert_eq!(s.world.command_channel_level(c), 20);
    assert_eq!(s.world.command_channel_member_count(c), 4);
    assert_eq!(s.world.command_channel_count(), 1);

    // A third party joins the running channel: "You have joined" goes to the answering player
    // as well, and the old members get the update.
    ask(&mut s, P1, "Echo");
    s.log.take();
    accept(&mut s, P5, 1);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P5),
        vec![
            SystemMessageId::YouHaveJoinedTheCommandChannel,
            SystemMessageId::YouHaveJoinedTheCommandChannel
        ],
        "once from addParty's broadcast, once from the packet"
    );
    assert_eq!(of(&sent, P6, "EX_OPEN_MPCC").len(), 1);
    for who in [P1, P2, P3, P4] {
        let update: ExMpccPartyInfoUpdate = decode_ex(of(&sent, who, "EX_MPCC_PARTY_INFO_UPDATE")[0]);
        assert_eq!((update.leader_name.as_str(), update.mode), ("Echo", 1));
    }
    assert_eq!(members(&s, P6), vec![P1, P2, P3, P4, P5, P6]);
}

/// A player who joins a party in a channel gets the window.
#[test]
fn a_new_party_member_gets_the_channel_window() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    s.enter(0x2000_0007, "Golf");
    s.log.take();
    s.invite(P3, "Golf", 0);
    s.answer(0x2000_0007, 1);
    let sent = s.log.take();
    assert_eq!(of(&sent, 0x2000_0007, "EX_OPEN_MPCC").len(), 1);
    assert_eq!(members(&s, P1).len(), 5);
}

// ------------------------------------------------------------------ oust, leave, delete

/// The leader dismisses a party of a channel of three: the party is told and the others see the
/// window update and who was dismissed.
#[test]
fn the_leader_ousts_a_party() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    join(&mut s, "Echo", P5);
    s.log.take();
    s.input(P1, ClientInput::OustFromMpcc { name: "Delta".into() });
    let sent = s.log.take();

    // Charlie's party: `ExCloseMPCC` from removeParty, then "dismissed".
    for who in [P3, P4] {
        assert_eq!(
            names(&sent, who),
            vec!["EX_CLOSE_MPCC", "SYSTEM_MESSAGE"],
            "{who:x}"
        );
        assert_eq!(
            message_ids(&sent, who),
            vec![SystemMessageId::YouWereDismissedFromTheCommandChannel]
        );
    }
    // The others: the update with mode 0, then who was dismissed (the leader of that party).
    for who in [P1, P2, P5, P6] {
        assert_eq!(
            names(&sent, who),
            vec!["EX_MPCC_PARTY_INFO_UPDATE", "SYSTEM_MESSAGE"],
            "{who:x}"
        );
        let update: ExMpccPartyInfoUpdate = decode_ex(of(&sent, who, "EX_MPCC_PARTY_INFO_UPDATE")[0]);
        assert_eq!(update.mode, 0);
        assert_eq!(update.leader_name, "Charlie");
        let msg = &messages(&sent, who)[0];
        assert_eq!(
            msg.id,
            SystemMessageId::C1SPartyHasBeenDismissedFromTheCommandChannel
        );
        assert_eq!(msg.params, vec![text("Charlie")]);
    }
    assert!(channel(&s, P3).is_none());
    assert_eq!(members(&s, P1), vec![P1, P2, P5, P6]);
    // The level follows the parties left (Charlie's 20 is gone).
    assert_eq!(s.world.command_channel_level(channel(&s, P1).unwrap()), 10);
}

/// Dismissing the second-to-last party ends the channel: everyone gets `ExCloseMPCC` and the
/// "disbanded" message; no "dismissed" message follows because the channel is gone.
#[test]
fn ousting_the_last_other_party_disbands_the_channel() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    s.log.take();
    s.input(
        P1,
        ClientInput::OustFromMpcc {
            name: "Charlie".into(),
        },
    );
    let sent = s.log.take();
    // Charlie's party: close, then (removeParty's disband broadcast reaches Alpha's party only)
    // the dismissal.
    assert_eq!(names(&sent, P3), vec!["EX_CLOSE_MPCC", "SYSTEM_MESSAGE"]);
    assert_eq!(
        message_ids(&sent, P3),
        vec![SystemMessageId::YouWereDismissedFromTheCommandChannel]
    );
    // Alpha's party: "disbanded", then the close of its own window.
    assert_eq!(names(&sent, P1), vec!["SYSTEM_MESSAGE", "EX_CLOSE_MPCC"]);
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::TheCommandChannelHasBeenDisbanded]
    );
    assert!(channel(&s, P1).is_none() && channel(&s, P3).is_none());
    assert_eq!(s.world.command_channel_count(), 0);
}

/// Oust refuses: a non-leader, an outsider, itself (silently), an unknown name.
#[test]
fn oust_needs_the_channel_leader_and_a_member_party() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    s.log.take();
    for (who, name) in [(P3, "Alpha"), (P1, "Echo"), (P1, "Nobody")] {
        s.input(who, ClientInput::OustFromMpcc { name: name.into() });
        assert_eq!(
            message_ids(&s.log.take(), who),
            vec![SystemMessageId::YourTargetCannotBeFound],
            "{who:x} {name}"
        );
    }
    // Itself: the checks pass, then Java returns without a word.
    s.input(P1, ClientInput::OustFromMpcc { name: "Alpha".into() });
    assert!(s.log.take().is_empty());
    assert_eq!(members(&s, P1).len(), 4);
}

/// `/channelleave` (96): only a party leader; the party leaves, the leader and the channel hear.
#[test]
fn channel_leave_takes_the_party_out() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    join(&mut s, "Echo", P5);
    s.log.take();
    // Not a leader.
    user_command(&mut s, P4, 96);
    assert_eq!(
        message_ids(&s.log.take(), P4),
        vec![SystemMessageId::OnlyAPartyLeaderCanLeaveACommandChannel]
    );
    user_command(&mut s, P3, 96);
    let sent = s.log.take();
    assert_eq!(names(&sent, P4), vec!["EX_CLOSE_MPCC"]);
    // Charlie: close, "you have quit".
    assert_eq!(names(&sent, P3), vec!["EX_CLOSE_MPCC", "SYSTEM_MESSAGE"]);
    assert_eq!(
        message_ids(&sent, P3),
        vec![SystemMessageId::YouHaveQuitTheCommandChannel]
    );
    // The rest of the channel: the update, then "C1's party has left" with a player name.
    for who in [P1, P2, P5, P6] {
        assert_eq!(
            names(&sent, who),
            vec!["EX_MPCC_PARTY_INFO_UPDATE", "SYSTEM_MESSAGE"],
            "{who:x}"
        );
        let msg = &messages(&sent, who)[0];
        assert_eq!(msg.id, SystemMessageId::C1SPartyHasLeftTheCommandChannel);
        assert_eq!(msg.params, vec![SystemMessageParam::PlayerName("Charlie".into())]);
    }
    assert_eq!(members(&s, P1), vec![P1, P2, P5, P6]);
    // Not in a channel any more: nothing happens.
    user_command(&mut s, P3, 96);
    assert!(s.log.take().is_empty());
}

/// `/channeldelete` (93): only the leader of the channel's party, the whole channel ends.
#[test]
fn channel_delete_disbands_everything() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    join(&mut s, "Echo", P5);
    s.log.take();
    // The leader of another party: nothing.
    user_command(&mut s, P3, 93);
    assert!(s.log.take().is_empty());
    user_command(&mut s, P1, 93);
    let sent = s.log.take();
    for who in [P1, P2, P3, P4, P5, P6] {
        assert!(
            message_ids(&sent, who).contains(&SystemMessageId::TheCommandChannelHasBeenDisbanded),
            "{who:x}"
        );
        assert!(names(&sent, who).contains(&"EX_CLOSE_MPCC".to_owned()), "{who:x}");
    }
    for who in [P1, P2, P3, P4, P5, P6] {
        assert!(channel(&s, who).is_none(), "{who:x}");
    }
    assert_eq!(s.world.command_channel_count(), 0);
}

/// A party of a channel that breaks up leaves it; the channel of two parties ends with it.
#[test]
fn a_party_that_breaks_up_leaves_the_channel() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    join(&mut s, "Echo", P5);
    s.log.take();
    // Delta leaves a party of two: the party disbands, Charlie's party is out of the channel.
    s.input(P4, ClientInput::WithdrawParty);
    let sent = s.log.take();
    assert!(
        names(&sent, P4).contains(&"EX_CLOSE_MPCC".to_owned()),
        "the leaver's window closes"
    );
    assert!(channel(&s, P3).is_none());
    assert_eq!(members(&s, P1), vec![P1, P2, P5, P6]);
    for who in [P1, P2, P5, P6] {
        let update: ExMpccPartyInfoUpdate = decode_ex(of(&sent, who, "EX_MPCC_PARTY_INFO_UPDATE")[0]);
        // Java names the party by its leader at that moment: Charlie is removed first.
        assert_eq!((update.leader_name.as_str(), update.mode), ("Delta", 0));
    }

    // Foxtrot leaves: Echo's party breaks up, and the channel of two parties dies with it.
    s.input(P6, ClientInput::WithdrawParty);
    let sent = s.log.take();
    for who in [P1, P2] {
        assert!(
            message_ids(&sent, who).contains(&SystemMessageId::TheCommandChannelHasBeenDisbanded),
            "{who:x}"
        );
    }
    assert!(channel(&s, P1).is_none());
    assert_eq!(s.world.command_channel_count(), 0);
}

/// The party that opened the channel breaks up.
// NOTE(java-quirk): `Party.removePartyMember` deletes the whole channel only when the *last* member
// left is the channel's leader, but `disbandParty` removes the leader first, so the party just
// leaves the channel (`removeParty`). With three parties the channel goes on, and its leader is
// still the player who left (not in any party now).
#[test]
fn the_channel_leaders_party_breaking_up_only_leaves_the_channel() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    join(&mut s, "Echo", P5);
    s.log.take();
    s.input(P2, ClientInput::WithdrawParty);
    let sent = s.log.take();
    assert!(channel(&s, P1).is_none() && channel(&s, P2).is_none());
    // Both of them have the window closed (the leaver by `removePartyMember`, the last one by
    // `removeParty`).
    assert!(names(&sent, P1).contains(&"EX_CLOSE_MPCC".to_owned()));
    assert!(names(&sent, P2).contains(&"EX_CLOSE_MPCC".to_owned()));
    let c = channel(&s, P3).unwrap();
    assert_eq!(members(&s, P3), vec![P3, P4, P5, P6]);
    assert_eq!(s.world.command_channel_leader(c), Some(id(P1)));
    assert_eq!(s.world.command_channel_count(), 1);
}

/// With only two parties, the opener's party leaving ends the channel.
#[test]
fn the_opener_leaving_a_channel_of_two_ends_it() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    s.log.take();
    s.input(P2, ClientInput::WithdrawParty);
    let sent = s.log.take();
    for who in [P3, P4] {
        assert!(
            message_ids(&sent, who).contains(&SystemMessageId::TheCommandChannelHasBeenDisbanded),
            "{who:x}"
        );
        assert!(names(&sent, who).contains(&"EX_CLOSE_MPCC".to_owned()), "{who:x}");
        assert!(channel(&s, who).is_none());
    }
    assert_eq!(s.world.command_channel_count(), 0);
}

// ------------------------------------------------------------------ the leader

/// The party leader hands over: the channel's leader follows when it was the old leader, and the
/// whole channel is told.
#[test]
fn the_channel_leader_follows_the_party_leader() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    s.log.take();
    s.input(P1, ClientInput::ChangePartyLeader { name: "Bravo".into() });
    let sent = s.log.take();
    let c = channel(&s, P1).unwrap();
    assert_eq!(s.world.command_channel_leader(c), Some(id(P2)));
    for who in [P1, P2, P3, P4] {
        let msg = messages(&sent, who)
            .into_iter()
            .find(|m| m.id == SystemMessageId::CommandChannelAuthorityHasBeenTransferredToC1)
            .unwrap_or_else(|| panic!("{who:x}"));
        assert_eq!(msg.params, vec![text("Bravo")]);
    }

    // A party leader who is not the channel's leader changes: the channel is not told.
    s.input(P3, ClientInput::ChangePartyLeader { name: "Delta".into() });
    let sent = s.log.take();
    assert_eq!(s.world.command_channel_leader(c), Some(id(P2)));
    assert!(
        !message_ids(&sent, P1).contains(&SystemMessageId::CommandChannelAuthorityHasBeenTransferredToC1)
    );
}

// ------------------------------------------------------------------ windows and chat

/// `RequestExMPCCShowPartyMembersInfo`: the members of the party led by the object id.
#[test]
fn the_party_member_window() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    s.log.take();
    s.input(P1, ClientInput::MpccShowPartyMembersInfo { party_leader_id: P3 });
    let sent = s.log.take();
    let info: ExMpccShowPartyMemberInfo = decode_ex(of(&sent, P1, "EX_MPCC_SHOW_PARTY_MEMBER_INFO")[0]);
    let got: Vec<(String, i32, i32)> = info
        .members
        .iter()
        .map(|m| (m.name.clone(), m.object_id, m.class_id))
        .collect();
    assert_eq!(got, vec![("Charlie".into(), P3, 0), ("Delta".into(), P4, 0)]);
    // An unknown id and a player without a party: nothing.
    s.input(
        P1,
        ClientInput::MpccShowPartyMembersInfo {
            party_leader_id: 12345,
        },
    );
    s.enter(0x2000_0007, "Golf");
    s.log.take();
    s.input(
        P1,
        ClientInput::MpccShowPartyMembersInfo {
            party_leader_id: 0x2000_0007,
        },
    );
    assert!(s.log.take().is_empty());
}

/// `/channelinfo` (97): the leader's name, the loot flag 0, the counts and one line per party.
#[test]
fn the_channel_info_window() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    s.log.take();
    user_command(&mut s, P4, 97);
    let sent = s.log.take();
    let info: ExMultiPartyCommandChannelInfo =
        decode_ex(of(&sent, P4, "EX_MULTI_PARTY_COMMAND_CHANNEL_INFO")[0]);
    assert_eq!(info.leader_name, "Alpha");
    assert_eq!(info.channel_loot, 0);
    assert_eq!(info.member_count, 4);
    let parties: Vec<(String, i32, i32)> = info
        .parties
        .iter()
        .map(|p| (p.leader_name.clone(), p.leader_object_id, p.member_count))
        .collect();
    assert_eq!(parties, vec![("Alpha".into(), P1, 2), ("Charlie".into(), P3, 2)]);
    // Not in a channel: nothing.
    user_command(&mut s, P5, 97);
    assert!(s.log.take().is_empty());
}

/// The `Say2` packets one client got, as bytes.
fn heard(sent: &Sent, to: i32) -> Vec<Vec<u8>> {
    of(sent, to, "SAY2").iter().map(|raw| raw.to_vec()).collect()
}

/// What Java's `new CreatureSay(sender, type, name, text)` writes.
fn say_bytes(sender: i32, chat_type: ChatType, name: &str, message: &str) -> Vec<u8> {
    CreatureSay {
        sender_object_id: sender,
        chat_type,
        sender: SenderField::Name(name.into()),
        message_id: -1,
        body: SayBody::Text(message.into()),
    }
    .to_bytes()
}

/// `PARTYROOM_ALL` is for any party leader of the channel, `PARTYROOM_COMMANDER` for its leader;
/// everybody of the channel hears it, an outsider does not, the others are ignored.
#[test]
fn the_channel_chat_types() {
    let mut s = three_parties(true);
    join(&mut s, "Charlie", P3);
    s.log.take();

    // A party leader of the channel, type PARTYROOM_ALL.
    say(&mut s, P3, ChatType::PartyRoomAll, "hello channel");
    let sent = s.log.take();
    for who in [P1, P2, P3, P4] {
        assert_eq!(
            heard(&sent, who),
            vec![say_bytes(P3, ChatType::PartyRoomAll, "Charlie", "hello channel")],
            "{who:x}"
        );
    }
    assert!(heard(&sent, P5).is_empty());

    // A plain member does not speak to the channel.
    say(&mut s, P4, ChatType::PartyRoomAll, "no");
    assert!(s.log.take().is_empty());
    // PARTYROOM_COMMANDER is for the channel's leader only.
    say(&mut s, P3, ChatType::PartyRoomCommander, "no");
    assert!(s.log.take().is_empty());
    say(&mut s, P1, ChatType::PartyRoomCommander, "orders");
    let sent = s.log.take();
    for who in [P1, P2, P3, P4] {
        assert_eq!(
            heard(&sent, who),
            vec![say_bytes(P1, ChatType::PartyRoomCommander, "Alpha", "orders")],
            "{who:x}"
        );
    }
    // Without a channel the message is dropped.
    say(&mut s, P5, ChatType::PartyRoomAll, "alone");
    assert!(s.log.take().is_empty());
}

// ------------------------------------------------------------------ the world around it

/// A command channel is a group for the loot: a member can take what another member's party
/// owns (`isInLooterParty`), an outsider cannot.
#[test]
fn the_channel_shares_the_looter_right() {
    let mut s = three_parties(true);
    assert!(!s.world.is_in_looter_party(id(P1), id(P3)));
    join(&mut s, "Charlie", P3);
    assert!(s.world.is_in_looter_party(id(P1), id(P3)));
    assert!(s.world.is_in_looter_party(id(P4), id(P2)));
    assert!(!s.world.is_in_looter_party(id(P1), id(P5)));
}

/// Same channel means friends: no auto attack, area offensive skills spare it.
#[test]
fn the_channel_members_do_not_attack_each_other() {
    let mut s = three_parties(true);
    assert!(
        s.world.is_auto_attackable(id(P3), id(P1)) || !s.world.is_in_same_command_channel(id(P1), id(P3))
    );
    join(&mut s, "Charlie", P3);
    assert!(s.world.is_in_same_command_channel(id(P1), id(P3)));
    assert!(s.world.is_in_command_channel_with(id(P1), id(P4)));
    assert!(!s.world.is_in_command_channel_with(id(P1), id(P5)));
    assert!(!s.world.is_auto_attackable(id(P3), id(P1)));
    assert!(!s.world.is_auto_attackable(id(P2), id(P4)));
}

// ------------------------------------------------------------------ the loot right of a raid

fn raid_scene(cc_size: i32) -> (Scene, ObjectId) {
    let mut c = Config::default();
    c.settings.party.loot_raids_privilege_cc_size = cc_size;
    c.settings.party.loot_raids_privilege_interval_ms = 60_000;
    let mut s = Scene::with(c);
    let alpha = {
        let mut a = player_data(P1, "Alpha", 10, (X, Y, Z));
        a.items.push(item(0x3000_0001, STRATEGY_GUIDE, 1));
        a
    };
    s.enter_data(alpha);
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.enter(P4, "Delta");
    s.enter(P5, "Echo");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    s.form_party(P3, &[(P4, "Delta")], 0);
    // The scene's monster, and a raid boss with the same drops.
    let raid =
        monster_xml(10, 1000, 100).replace(r#"id="9001" type="Monster""#, r#"id="9002" type="RaidBoss""#);
    let npcs = NpcTemplates::from_xml(
        &format!("<list>{}{raid}</list>", monster_xml(10, 1000, 100)),
        &NpcLoadOptions::default(),
    )
    .unwrap();
    s.world
        .set_npc_data(Arc::new(npcs), Arc::new(SpawnData::default()));
    let boss = s.world.spawn_npc_at(9002, X, Y, Z, 0).unwrap();
    (s, boss)
}

fn damage(s: &mut Scene, boss: ObjectId, who: i32) {
    s.world.reduce_current_hp(boss, 1.0, Some(id(who)));
}

/// The first channel with enough members to hit the raid boss gets the right, and its members hear
/// it as a `PARTYROOM_ALL` line from nobody.
#[test]
fn the_first_big_enough_channel_gets_the_looting_right() {
    let (mut s, boss) = raid_scene(4);
    // No channel: no right.
    damage(&mut s, boss, P1);
    assert_eq!(s.world.raid_loot_owner(boss), None);
    join(&mut s, "Charlie", P3);
    s.log.take();

    damage(&mut s, boss, P4);
    let sent = s.log.take();
    for who in [P1, P2, P3, P4] {
        assert_eq!(
            heard(&sent, who),
            vec![say_bytes(
                0,
                ChatType::PartyRoomAll,
                "",
                "You have looting rights!"
            )],
            "{who:x}"
        );
    }
    assert_eq!(s.world.raid_loot_owner(boss), Some(id(P1)));

    // Once is enough: a second hit does not announce it again.
    damage(&mut s, boss, P2);
    assert!(heard(&s.log.take(), P1).is_empty());
}

/// A channel below `RaidLootRightsCCSize` gets nothing, and neither does a monster that is no raid.
#[test]
fn a_small_channel_or_a_plain_monster_gives_no_right() {
    let (mut s, boss) = raid_scene(5);
    join(&mut s, "Charlie", P3);
    s.log.take();
    damage(&mut s, boss, P1);
    assert_eq!(s.world.raid_loot_owner(boss), None);
    assert!(heard(&s.log.take(), P1).is_empty());

    let (mut s, _) = raid_scene(4);
    join(&mut s, "Charlie", P3);
    let monster = s.spawn_monster();
    damage(&mut s, monster, P1);
    assert_eq!(s.world.raid_loot_owner(monster), None);
}

/// `CommandChannelTimer`: checked every 10 s; the right ends when nobody of the channel hit the
/// boss for `RaidLootRightsInterval` (60 s here); a hit in between keeps it.
#[test]
fn the_right_ends_when_the_channel_stops_attacking() {
    let (mut s, boss) = raid_scene(4);
    join(&mut s, "Charlie", P3);
    damage(&mut s, boss, P1);
    assert_eq!(s.world.raid_loot_owner(boss), Some(id(P1)));

    // 50 s: still there. A hit at 50 s moves the end to 110 s.
    s.ticks(50 * 20);
    assert_eq!(s.world.raid_loot_owner(boss), Some(id(P1)));
    damage(&mut s, boss, P3);
    s.ticks(60 * 20);
    assert_eq!(
        s.world.raid_loot_owner(boss),
        Some(id(P1)),
        "110 s: the timer is not past the interval yet"
    );
    // The check at 120 s sees 70 s without a hit.
    s.ticks(10 * 20);
    assert_eq!(s.world.raid_loot_owner(boss), None);

    // Another channel (or the same one) may start over.
    damage(&mut s, boss, P1);
    assert_eq!(s.world.raid_loot_owner(boss), Some(id(P1)));
}

/// `ItemManager.createItem`: the loot of a raid belongs to the leader of the first channel,
/// for `RaidLootRightsInterval`; without a channel nobody owns it; `AutoLootRaids` skips it.
#[test]
fn the_loot_of_a_raid_belongs_to_the_channel_leader() {
    let (mut s, boss) = raid_scene(4);
    join(&mut s, "Charlie", P3);
    damage(&mut s, boss, P1);
    // Charlie's party does the killing blow.
    s.hit(boss, P3, 1);
    assert!(s.world.do_die(boss, Some(id(P3))));
    let ids = s.world.ground_item_ids();
    assert!(!ids.is_empty());
    for g in &ids {
        assert_eq!(s.world.ground_item(*g).unwrap().owner, Some(id(P1)), "{g:?}");
    }
    // The owner is a looter of every member of the channel.
    assert!(s.world.is_in_looter_party(id(P4), id(P1)));
    // After the interval (60 s) the loot is free.
    s.ticks(60 * 20 + 1);
    for g in s.world.ground_item_ids() {
        assert_eq!(s.world.ground_item(g).unwrap().owner, None);
    }
}

#[test]
fn a_raid_nobody_channeled_drops_free_loot() {
    let (mut s, boss) = raid_scene(4);
    s.hit(boss, P1, 1);
    assert!(s.world.do_die(boss, Some(id(P1))));
    let ids = s.world.ground_item_ids();
    assert!(!ids.is_empty());
    for g in ids {
        assert_eq!(s.world.ground_item(g).unwrap().owner, None);
    }
}

/// The exp of a channel kill: the whole channel is rewarded, at the level of the channel.
#[test]
fn the_exp_goes_to_the_whole_channel_at_the_channel_level() {
    let mut s = Scene::new();
    let mut alpha = player_data(P1, "Alpha", 10, (X, Y, Z));
    alpha.items.push(item(0x3000_0001, STRATEGY_GUIDE, 1));
    s.enter_data(alpha);
    s.enter(P2, "Bravo");
    s.enter_level(P3, "Charlie", 14);
    s.enter(P4, "Delta");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    s.form_party(P3, &[(P4, "Delta")], 0);
    let before = |s: &Scene, who: i32| s.player(who).data.exp;
    let start: Vec<i64> = [P1, P2, P3, P4].iter().map(|w| before(&s, *w)).collect();
    join(&mut s, "Charlie", P3);
    let monster = s.spawn_monster();
    s.hit(monster, P1, 5);
    assert!(s.world.do_die(monster, Some(id(P1))));
    let gain = |s: &Scene, who: i32, i: usize| s.player(who).data.exp - start[i];
    // Charlie's party took no damage but is in the group: everybody in range is rewarded.
    assert!(gain(&s, P3, 2) > 0, "a member of the other party gets exp");
    assert!(gain(&s, P4, 3) > 0);
    assert!(gain(&s, P1, 0) > 0 && gain(&s, P2, 1) > 0);
}

// ------------------------------------------------------------------ the skill targets

fn skill(target_type: &str, tags: &str, effects: &str) -> l2_data::SkillTemplate {
    use l2_data::skill::EnchantSkillGroups;
    use l2_data::{SkillLoadOptions, SkillTemplates};
    let xml = format!(
        "<list><skill id=\"1\" levels=\"1\" name=\"T\"><operateType>A1</operateType>\
         <targetType>{target_type}</targetType>{tags}<effects>{effects}</effects></skill></list>"
    );
    let table = SkillTemplates::from_xml(&xml, &EnchantSkillGroups::default(), &SkillLoadOptions::default());
    table.get(1, 1).unwrap().clone()
}

fn targets_of(s: &mut Scene, skill: &l2_data::SkillTemplate, caster: i32) -> Vec<i32> {
    use l2_world::skills::TargetContext;
    s.world
        .skill_target_list(skill, id(caster), false, None, &TargetContext::default())
        .targets
        .iter()
        .map(|t| t.0)
        .collect()
}

/// `COMMAND_CHANNEL` takes the members of the whole channel, not just the party.
#[test]
fn the_command_channel_target_reaches_the_other_parties() {
    let mut s = three_parties(true);
    let sk = skill(
        "COMMAND_CHANNEL",
        "<affectRange>1000</affectRange><affectLimit>9-0</affectLimit><effectPoint>100</effectPoint>",
        "",
    );
    assert_eq!(targets_of(&mut s, &sk, P1), vec![P1, P2]);
    join(&mut s, "Charlie", P3);
    assert_eq!(targets_of(&mut s, &sk, P1), vec![P1, P2, P3, P4]);
    // Echo's party is out of it.
    assert_eq!(targets_of(&mut s, &sk, P5), vec![P5, P6]);
}

/// Area offensive skills spare the channel; the friendly area skills include it.
#[test]
fn area_skills_treat_the_channel_as_friends() {
    let mut s = three_parties_with_karma(true, 100);
    let hostile = skill(
        "AURA",
        "<affectRange>1000</affectRange><effectPoint>-100</effectPoint>",
        "",
    );
    let friendly = skill("AURA_FRIENDLY", "<effectPoint>100</effectPoint>", "");
    let before = targets_of(&mut s, &hostile, P1);
    assert!(before.contains(&P3) && before.contains(&P4), "{before:?}");
    join(&mut s, "Charlie", P3);
    let after = targets_of(&mut s, &hostile, P1);
    assert!(!after.contains(&P3) && !after.contains(&P4), "{after:?}");
    assert!(after.contains(&P5), "Echo's party is no friend");
    let friends = targets_of(&mut s, &friendly, P1);
    assert!(
        friends.contains(&P3) && friends.contains(&P4) && !friends.contains(&P5),
        "{friends:?}"
    );
}
