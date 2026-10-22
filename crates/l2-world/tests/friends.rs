#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The friend list and the block list (M6-FRIEND-01): the friend request, the answer, the list
//! window, the removal, the private messages, the `/block` commands and the message refusal, plus
//! the markers the party invitation, the trade request and the chat channels read the block list
//! from.
//!
//! The expected packets and messages are read off the Java sources (`BlockList.java`, the
//! `RequestFriend*`, `RequestSendFriendMsg` and `RequestBlock` client packets, `EnterWorld.java`,
//! `Player.notifyFriends`, `ChatWhisper.java`, `ChatGeneral.java`), not off the code.

use std::sync::{Arc, Mutex};

use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::enterworld::{EtcStatusUpdate, FriendList};
use l2_protocol_game::server::{
    FriendAddRequest, FriendPacket, FriendStatusPacket, L2FriendSay, SystemMessage, SystemMessageParam,
};
use l2_world::*;

// ------------------------------------------------------------------------------- harness

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Persisted {
    fn take(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct RecOutbox(Persisted);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

const ALICE: i32 = 0x1000_0001;
const BOB: i32 = 0x1000_0002;
const CAROL: i32 = 0x1000_0003;

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
}

impl Scene {
    fn new() -> Self {
        let log = Log::default();
        let persisted = Persisted::default();
        let world = World::new(
            l2_util::WorldRng::from_seed(1),
            Box::new(RecOutbox(persisted.clone())),
        );
        Self {
            world,
            log,
            persisted,
        }
    }

    /// Enters a character with the lists the session would have read.
    fn enter(&mut self, id: i32, name: &str, friends: &[(i32, &str)], blocked: &[(i32, &str)]) {
        let mut data = PlayerData {
            object_id: ObjectId(id),
            account: format!("acc{id}"),
            name: name.into(),
            title: String::new(),
            class_id: 0,
            base_class: 0,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            // The default `MinimumChatLevel` is 20: general chat needs it.
            level: 20,
            exp: 0,
            sp: 0,
            karma: 0,
            pvp_kills: 0,
            pk_kills: 0,
            access_level: 0,
            x: -71338,
            y: 258271,
            z: -3104,
            heading: 0,
            cur_hp: 100.0,
            max_hp: 100.0,
            cur_mp: 50.0,
            max_mp: 50.0,
            cur_cp: 0.0,
            max_cp: 50.0,
            online_time: 10,
            items: vec![],
            skills: vec![],
            shortcuts: vec![],
            profile: Default::default(),
        };
        let link = |(id, name): &(i32, &str)| FriendEntry {
            object_id: *id,
            name: Some((*name).to_owned()),
        };
        data.profile.friends = friends.iter().map(link).collect();
        data.profile.block_list = blocked.iter().map(link).collect();
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(id as u64),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn say(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(id as u64),
            input,
        });
    }

    fn take(&self) -> Sent {
        self.log.take()
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

/// The packets of one kind one client got, in order.
fn of<'a>(sent: &'a Sent, to: i32, name: &str) -> Vec<&'a [u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| b.as_slice())
        .collect()
}

/// The system messages one client got, as (id, params).
fn messages(sent: &Sent, to: i32) -> Vec<(SystemMessageId, Vec<SystemMessageParam>)> {
    of(sent, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| {
            let m: SystemMessage = decode(raw);
            (m.id, m.params)
        })
        .collect()
}

fn message_ids(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    messages(sent, to).into_iter().map(|(id, _)| id).collect()
}

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

/// The plain text lines (`SystemMessage(String)`, id `$s1`) one client got.
fn texts(sent: &Sent, to: i32) -> Vec<String> {
    messages(sent, to)
        .into_iter()
        .filter(|(id, _)| *id == SystemMessageId::S12)
        .filter_map(|(_, params)| match params.first() {
            Some(SystemMessageParam::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------------------- the request

/// `RequestFriendInvite`: the target gets `FriendAddRequest` with the requestor's name, the
/// requestor is told "you have requested C1 to be on your friends list" with the typed name.
/// Answering yes writes both rows and tells each side in Java's order.
#[test]
fn an_invitation_is_asked_and_answered_yes() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();

    s.say(ALICE, ClientInput::FriendInvite { name: "bob".into() });
    let sent = s.take();
    // The name is matched ignoring case; the message uses the name as typed.
    let ask: FriendAddRequest = decode(of(&sent, BOB, "FRIEND_ADD_REQUEST")[0]);
    assert_eq!(ask.requestor_name, "Alice");
    assert_eq!(
        messages(&sent, ALICE),
        vec![(
            SystemMessageId::YouVeRequestedC1ToBeOnYourFriendsList,
            vec![text("bob")]
        )]
    );
    assert!(
        s.persisted.take().is_empty(),
        "nothing is written before the answer"
    );

    s.say(BOB, ClientInput::AnswerFriendInvite { response: 1 });
    let sent = s.take();
    assert_eq!(
        s.persisted.take(),
        vec![PersistCommand::Friend(FriendWrite::AddFriends {
            first: ObjectId(ALICE),
            second: ObjectId(BOB),
        })]
    );
    // The requestor (Alice): the success line, "S1 has been added", then the window entry.
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![
            SystemMessageId::ThatPersonHasBeenSuccessfullyAddedToYourFriendList,
            SystemMessageId::S1HasBeenAddedToYourFriendsList,
        ]
    );
    assert_eq!(messages(&sent, ALICE)[1].1, vec![text("Bob")]);
    let to_alice: FriendPacket = decode(of(&sent, ALICE, "L2_FRIEND")[0]);
    assert!(to_alice.added);
    assert_eq!(to_alice.object_id, BOB);
    assert_eq!(to_alice.name, "Bob");
    assert!(to_alice.online);
    // The answering side (Bob): "S1 has joined as a friend", then its window entry.
    assert_eq!(
        message_ids(&sent, BOB),
        vec![SystemMessageId::S1HasJoinedAsAFriend]
    );
    assert_eq!(messages(&sent, BOB)[0].1, vec![text("Alice")]);
    let to_bob: FriendPacket = decode(of(&sent, BOB, "L2_FRIEND")[0]);
    assert_eq!((to_bob.object_id, to_bob.name.as_str()), (ALICE, "Alice"));

    // Both lists now hold the other: the friends window shows Bob online to Alice.
    s.say(ALICE, ClientInput::FriendList);
    let sent = s.take();
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![
            SystemMessageId::FriendsList,
            SystemMessageId::S1CurrentlyOnline,
            SystemMessageId::Empty3,
        ]
    );
    assert_eq!(messages(&sent, ALICE)[1].1, vec![text("Bob")]);
}

/// Answering no tells the requestor only, and the request is over.
#[test]
fn a_refused_invitation_is_only_reported_to_the_requestor() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.say(ALICE, ClientInput::FriendInvite { name: "Bob".into() });
    s.take();

    s.say(BOB, ClientInput::AnswerFriendInvite { response: 0 });
    let sent = s.take();
    assert!(s.persisted.take().is_empty());
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![SystemMessageId::YouHaveFailedToAddAFriendToYourFriendsList]
    );
    assert!(message_ids(&sent, BOB).is_empty());
    // The request is answered, so a second answer finds nobody.
    s.say(BOB, ClientInput::AnswerFriendInvite { response: 1 });
    assert!(s.take().is_empty());
    assert!(s.persisted.take().is_empty());
}

/// The refusals of `RequestFriendInvite`, each with Java's message.
#[test]
fn invitation_refusals_use_javas_messages() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[(BOB, "Bob")], &[]);
    s.enter(BOB, "Bob", &[(ALICE, "Alice")], &[]);
    s.enter(CAROL, "Carol", &[], &[]);
    s.take();

    s.say(
        ALICE,
        ClientInput::FriendInvite {
            name: "Nobody".into(),
        },
    );
    assert_eq!(
        message_ids(&s.take(), ALICE),
        vec![SystemMessageId::TheUserWhoRequestedToBecomeFriendsIsNotFoundInTheGame]
    );

    s.say(ALICE, ClientInput::FriendInvite { name: "Alice".into() });
    assert_eq!(
        message_ids(&s.take(), ALICE),
        vec![SystemMessageId::YouCannotAddYourselfToYourOwnFriendList]
    );

    s.say(ALICE, ClientInput::FriendInvite { name: "Bob".into() });
    assert_eq!(
        message_ids(&s.take(), ALICE),
        vec![SystemMessageId::ThisPlayerIsAlreadyRegisteredOnYourFriendsList]
    );

    // Carol blocks Alice: Alice is told it is in the target's block list (a plain line).
    s.say(
        CAROL,
        ClientInput::Block {
            kind: 0,
            name: Some("Alice".into()),
        },
    );
    s.take();
    s.say(ALICE, ClientInput::FriendInvite { name: "Carol".into() });
    assert_eq!(
        texts(&s.take(), ALICE),
        vec!["You are in target's block list.".to_owned()]
    );

    // Alice blocks Carol: Alice is told it has blocked C1 (the block is checked after the first).
    s.say(
        ALICE,
        ClientInput::Block {
            kind: 0,
            name: Some("Carol".into()),
        },
    );
    s.take();
    s.say(CAROL, ClientInput::FriendInvite { name: "Alice".into() });
    assert_eq!(
        texts(&s.take(), CAROL),
        vec!["You are in target's block list.".to_owned()]
    );
}

/// A target that is already asked gets "C1 is on another task", with the typed name.
#[test]
fn a_busy_target_is_reported() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.enter(CAROL, "Carol", &[], &[]);
    s.say(CAROL, ClientInput::FriendInvite { name: "Bob".into() });
    s.take();
    s.say(ALICE, ClientInput::FriendInvite { name: "bob".into() });
    let sent = s.take();
    assert_eq!(
        messages(&sent, ALICE),
        vec![(
            SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater,
            vec![text("bob")]
        )]
    );
}

// ----------------------------------------------------------------- enter and leave

/// `EnterWorld`: the friend list packet with each friend's online flag, the login notice to the
/// friends who are online, and the status packet of `notifyFriends`.
#[test]
fn entering_shows_the_list_and_tells_the_online_friends() {
    let mut s = Scene::new();
    s.enter(BOB, "Bob", &[], &[]);
    s.take();
    s.enter(ALICE, "Alice", &[(BOB, "Bob"), (CAROL, "Carol")], &[]);
    let sent = s.take();

    let list: FriendList = decode(of(&sent, ALICE, "L2_FRIEND_LIST")[0]);
    let shown: Vec<(i32, &str, bool)> = list
        .friends
        .iter()
        .map(|f| (f.object_id, f.name.as_str(), f.online))
        .collect();
    assert_eq!(shown, vec![(BOB, "Bob", true), (CAROL, "Carol", false)]);

    // Bob hears that Alice logged in, then gets the status packet.
    assert_eq!(
        message_ids(&sent, BOB),
        vec![SystemMessageId::YourFriendS1JustLoggedIn]
    );
    assert_eq!(messages(&sent, BOB)[0].1, vec![text("Alice")]);
    let status: FriendStatusPacket = decode(of(&sent, BOB, "L2_FRIEND_STATUS")[0]);
    assert!(status.online);
    assert_eq!((status.name.as_str(), status.object_id), ("Alice", ALICE));
    // Carol is offline: nobody is told.
    assert!(!sent.iter().any(|(to, _)| *to == CAROL));
}

/// `Player.deleteMe` -> `notifyFriends()`: the online friends see the character go offline.
#[test]
fn logging_out_tells_the_online_friends() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[(BOB, "Bob")], &[]);
    s.enter(BOB, "Bob", &[(ALICE, "Alice")], &[]);
    s.take();
    s.say(ALICE, ClientInput::Logout);
    let sent = s.take();
    let status: FriendStatusPacket = decode(of(&sent, BOB, "L2_FRIEND_STATUS")[0]);
    assert!(
        !status.online,
        "the leaving character is already out of the world"
    );
    assert_eq!((status.name.as_str(), status.object_id), ("Alice", ALICE));
}

// ---------------------------------------------------------------- the list and removal

/// `RequestFriendDel`: both sides lose the friend, the window and the statement are Java's.
#[test]
fn removing_a_friend_is_written_and_both_sides_hear_it() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[(BOB, "Bob")], &[]);
    s.enter(BOB, "Bob", &[(ALICE, "Alice")], &[]);
    s.take();

    s.say(ALICE, ClientInput::FriendDel { name: "Bob".into() });
    let sent = s.take();
    assert_eq!(
        s.persisted.take(),
        vec![PersistCommand::Friend(FriendWrite::RemoveFriends {
            first: ObjectId(ALICE),
            second: ObjectId(BOB),
        })]
    );
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![SystemMessageId::S1HasBeenDeletedFromYourFriendsList]
    );
    let window: FriendPacket = decode(of(&sent, ALICE, "L2_FRIEND")[0]);
    assert!(!window.added);
    assert_eq!((window.object_id, window.name.as_str()), (BOB, "Bob"));
    let other: FriendPacket = decode(of(&sent, BOB, "L2_FRIEND")[0]);
    assert!(!other.added);
    assert_eq!(other.object_id, ALICE);

    // Not a friend any more: the same name is "is not on your friend list".
    s.say(ALICE, ClientInput::FriendDel { name: "Bob".into() });
    assert_eq!(
        message_ids(&s.take(), ALICE),
        vec![SystemMessageId::C1IsNotOnYourFriendList]
    );
}

/// `RequestFriendList`: "Friends list", one line per friend, then the end line.
#[test]
fn the_list_window_says_online_or_offline_per_friend() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[(BOB, "Bob"), (CAROL, "Carol")], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();
    s.say(ALICE, ClientInput::FriendList);
    let sent = s.take();
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![
            SystemMessageId::FriendsList,
            SystemMessageId::S1CurrentlyOnline,
            SystemMessageId::S1CurrentlyOffline,
            SystemMessageId::Empty3,
        ]
    );
    assert_eq!(messages(&sent, ALICE)[2].1, vec![text("Carol")]);
}

/// `RequestSendFriendMsg`: only a friend who is online gets the line; the text is limited to 300
/// UTF-16 units.
#[test]
fn private_messages_go_to_online_friends_only() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[(BOB, "Bob")], &[]);
    s.enter(BOB, "Bob", &[(ALICE, "Alice")], &[]);
    s.enter(CAROL, "Carol", &[], &[]);
    s.take();

    s.say(
        ALICE,
        ClientInput::SendFriendMsg {
            message: "hi".into(),
            receiver: "bob".into(),
        },
    );
    let sent = s.take();
    let say: L2FriendSay = decode(of(&sent, BOB, "L2_FRIEND_SAY")[0]);
    assert_eq!(say.sender, "Alice");
    assert_eq!(say.receiver, "bob", "the receiver as typed");
    assert_eq!(say.message, "hi");

    // Carol has no Alice as a friend.
    s.say(
        ALICE,
        ClientInput::SendFriendMsg {
            message: "hi".into(),
            receiver: "Carol".into(),
        },
    );
    assert_eq!(
        message_ids(&s.take(), ALICE),
        vec![SystemMessageId::ThatPlayerIsNotOnline]
    );

    // Too long (301 units) or empty: nothing happens.
    let long = "x".repeat(301);
    s.say(
        ALICE,
        ClientInput::SendFriendMsg {
            message: long,
            receiver: "Bob".into(),
        },
    );
    s.say(
        ALICE,
        ClientInput::SendFriendMsg {
            message: String::new(),
            receiver: "Bob".into(),
        },
    );
    assert!(s.take().is_empty());
}

// ------------------------------------------------------------------------ block list

/// `/block`, `/blocklist`, `/unblock` with Java's messages and rows.
#[test]
fn the_block_list_commands_follow_java() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();

    s.say(
        ALICE,
        ClientInput::Block {
            kind: 0,
            name: Some("Bob".into()),
        },
    );
    let sent = s.take();
    assert_eq!(
        s.persisted.take(),
        vec![PersistCommand::Friend(FriendWrite::AddBlock {
            owner: ObjectId(ALICE),
            target: ObjectId(BOB),
        })]
    );
    assert_eq!(
        messages(&sent, ALICE),
        vec![(SystemMessageId::S1HasBeenAddedToYourIgnoreList, vec![text("Bob")])]
    );
    assert_eq!(
        messages(&sent, BOB),
        vec![(
            SystemMessageId::S1HasPlacedYouOnHisHerIgnoreList,
            vec![text("Alice")]
        )]
    );

    // Blocking again: "Already in ignore list." as a plain line, nothing written.
    s.say(
        ALICE,
        ClientInput::Block {
            kind: 0,
            name: Some("Bob".into()),
        },
    );
    assert_eq!(
        texts(&s.take(), ALICE),
        vec!["Already in ignore list.".to_owned()]
    );
    assert!(s.persisted.take().is_empty());

    // The list: "Ignore list", "1. Bob", then the end line.
    s.say(ALICE, ClientInput::Block { kind: 2, name: None });
    let sent = s.take();
    // The list line is a plain text line (`$s1`), so its id is S12 too.
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![
            SystemMessageId::IgnoreList,
            SystemMessageId::S12,
            SystemMessageId::Empty3
        ]
    );
    assert_eq!(texts(&sent, ALICE), vec!["1. Bob".to_owned()]);

    s.say(
        ALICE,
        ClientInput::Block {
            kind: 1,
            name: Some("Bob".into()),
        },
    );
    let sent = s.take();
    assert_eq!(
        s.persisted.take(),
        vec![PersistCommand::Friend(FriendWrite::RemoveBlock {
            owner: ObjectId(ALICE),
            target: ObjectId(BOB),
        })]
    );
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![SystemMessageId::S1HasBeenRemovedFromYourIgnoreList]
    );

    // Not on the list any more.
    s.say(
        ALICE,
        ClientInput::Block {
            kind: 1,
            name: Some("Bob".into()),
        },
    );
    assert_eq!(
        message_ids(&s.take(), ALICE),
        vec![SystemMessageId::ThatIsAnIncorrectTarget]
    );
}

/// A name that is not a character, a GM target, and blocking yourself.
#[test]
fn block_refusals() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();

    s.say(
        ALICE,
        ClientInput::Block {
            kind: 0,
            name: Some("Nobody".into()),
        },
    );
    assert_eq!(
        message_ids(&s.take(), ALICE),
        vec![SystemMessageId::YouHaveFailedToRegisterTheUserToYourIgnoreList]
    );

    s.say(
        ALICE,
        ClientInput::Block {
            kind: 0,
            name: Some("Alice".into()),
        },
    );
    assert!(s.take().is_empty(), "blocking yourself does nothing");
    assert!(s.persisted.take().is_empty());
}

/// A friend cannot be put on the block list: Java answers with the friend message.
#[test]
fn a_friend_cannot_be_blocked() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[(BOB, "Bob")], &[]);
    s.enter(BOB, "Bob", &[(ALICE, "Alice")], &[]);
    s.take();
    s.say(
        ALICE,
        ClientInput::Block {
            kind: 0,
            name: Some("Bob".into()),
        },
    );
    assert_eq!(
        message_ids(&s.take(), ALICE),
        vec![SystemMessageId::ThisPlayerIsAlreadyRegisteredOnYourFriendsList]
    );
}

/// `/allblock` refuses every message: the status icon is set, the trade and the whisper are
/// refused, and `/allunblock` gives it back.
#[test]
fn message_refusal_mode() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();

    s.say(BOB, ClientInput::Block { kind: 3, name: None });
    let sent = s.take();
    assert_eq!(message_ids(&sent, BOB), vec![SystemMessageId::MessageRefusalMode]);
    let status: EtcStatusUpdate = decode(of(&sent, BOB, "ETC_STATUS_UPDATE")[0]);
    assert!(status.chat_blocked, "the refusal shows in EtcStatusUpdate");
    assert!(s.world.message_refusal(ObjectId(BOB)));

    // A whisper to Bob: Bob hears nothing, Alice is told it is in refusal mode.
    s.say(
        ALICE,
        ClientInput::Say {
            chat_type: 2,
            text: "psst".into(),
            target: Some("Bob".into()),
        },
    );
    let sent = s.take();
    assert!(of(&sent, BOB, "SAY2").is_empty());
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![SystemMessageId::ThatPersonIsInMessageRefusalMode]
    );

    // A general chat line: Bob is not among the hearers.
    s.say(
        ALICE,
        ClientInput::Say {
            chat_type: 0,
            text: "hello".into(),
            target: None,
        },
    );
    let sent = s.take();
    assert!(of(&sent, BOB, "SAY2").is_empty());
    // The speaker's own copy: the chat type is the first byte after the opcode (the sender id
    // follows it), General is 0.
    let heard = of(&sent, ALICE, "SAY2");
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0][1..5], 0x1000_0001_i32.to_le_bytes());

    s.say(BOB, ClientInput::Block { kind: 4, name: None });
    let sent = s.take();
    assert_eq!(
        message_ids(&sent, BOB),
        vec![SystemMessageId::MessageAcceptanceMode]
    );
    assert!(!s.world.message_refusal(ObjectId(BOB)));
}

/// `ChatWhisper`: a receiver who blocks the sender gets nothing, the sender is told why.
#[test]
fn a_whisper_from_a_blocked_sender_is_refused() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();
    s.say(
        BOB,
        ClientInput::Block {
            kind: 0,
            name: Some("Alice".into()),
        },
    );
    s.take();

    s.say(
        ALICE,
        ClientInput::Say {
            chat_type: 2,
            text: "psst".into(),
            target: Some("Bob".into()),
        },
    );
    let sent = s.take();
    assert!(of(&sent, BOB, "SAY2").is_empty());
    assert_eq!(
        message_ids(&sent, ALICE),
        vec![SystemMessageId::ThatPersonIsInMessageRefusalMode]
    );
}

/// `ChatGeneral`: the listeners who block the speaker are skipped; the speaker still hears itself.
#[test]
fn a_listener_who_blocks_the_speaker_hears_no_general_chat() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();
    // Bob blocks Alice: Bob does not hear Alice in general chat.
    s.say(
        BOB,
        ClientInput::Block {
            kind: 0,
            name: Some("Alice".into()),
        },
    );
    s.take();
    s.say(
        ALICE,
        ClientInput::Say {
            chat_type: 0,
            text: "hi all".into(),
            target: None,
        },
    );
    let sent = s.take();
    assert!(of(&sent, BOB, "SAY2").is_empty());
    assert_eq!(of(&sent, ALICE, "SAY2").len(), 1, "the speaker hears itself");
}

/// The party invitation of a player who blocks the requestor: "S1 has placed you on his/her
/// ignore list" to the requestor, and no invitation is sent.
#[test]
fn the_party_invitation_respects_the_block_list() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();
    s.say(
        BOB,
        ClientInput::Block {
            kind: 0,
            name: Some("Alice".into()),
        },
    );
    s.take();

    s.say(
        ALICE,
        ClientInput::JoinParty {
            name: "Bob".into(),
            distribution_type_id: 0,
        },
    );
    let sent = s.take();
    assert_eq!(
        messages(&sent, ALICE),
        vec![(
            SystemMessageId::S1HasPlacedYouOnHisHerIgnoreList,
            vec![text("Bob")]
        )]
    );
    assert!(of(&sent, BOB, "ASK_JOIN_PARTY").is_empty());
}

/// The trade request of a partner who blocks the player (`TradeRequest.runImpl`).
#[test]
fn the_trade_request_respects_the_block_list() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();
    s.say(
        BOB,
        ClientInput::Block {
            kind: 0,
            name: Some("Alice".into()),
        },
    );
    s.take();

    s.say(ALICE, ClientInput::TradeRequest { object_id: BOB });
    let sent = s.take();
    assert_eq!(
        messages(&sent, ALICE),
        vec![(
            SystemMessageId::S1HasPlacedYouOnHisHerIgnoreList,
            vec![text("Bob")]
        )]
    );
    assert!(of(&sent, BOB, "SEND_TRADE_REQUEST").is_empty());
}

/// A refused invitation makes no request, so a later answer has nothing to answer.
#[test]
fn friend_lists_are_untouched_by_refused_requests() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();
    s.say(
        BOB,
        ClientInput::Block {
            kind: 0,
            name: Some("Alice".into()),
        },
    );
    s.take();
    s.persisted.take();
    s.say(ALICE, ClientInput::FriendInvite { name: "Bob".into() });
    s.take();
    s.say(BOB, ClientInput::AnswerFriendInvite { response: 1 });
    // No request was made, so the answer finds nobody and writes nothing.
    assert!(s.persisted.take().is_empty());
}

/// Friend-list rows reach the database through the persistence outbox only.
#[test]
fn the_persistence_writes_are_the_java_statements() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", &[], &[]);
    s.enter(BOB, "Bob", &[], &[]);
    s.take();
    s.say(ALICE, ClientInput::FriendInvite { name: "Bob".into() });
    s.say(BOB, ClientInput::AnswerFriendInvite { response: 1 });
    let writes = s.persisted.take();
    assert_eq!(
        writes,
        vec![PersistCommand::Friend(FriendWrite::AddFriends {
            first: ObjectId(ALICE),
            second: ObjectId(BOB),
        })]
    );
}
