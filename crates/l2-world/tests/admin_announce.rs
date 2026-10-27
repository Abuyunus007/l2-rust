#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The announcements, the petition system, the packet forge and `//debug` of M7-ADMIN-10.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminAnnouncements,AdminPetition,
//! AdminPForge,AdminDebug}.java, managers/PetitionManager.java, data/sql/AnnouncementsTable.java,
//! mechanics/announce/*.java.

mod admin_common;

use std::sync::Mutex;

use admin_common::*;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, ExShowScreenMessage, ShowBoard, SystemMessageParam};
use l2_world::*;

fn boards_to(out: &Sent, to: i32) -> Vec<ShowBoard> {
    decoded::<ShowBoard>(out, to, "SHOW_BOARD")
}

/// The html of the page (the part `101`) that went to the community board of `to`.
fn board_page(out: &Sent, to: i32) -> String {
    let boards = boards_to(out, to);
    assert_eq!(boards.len(), 3, "a page is three boards");
    let page = &boards[0].content;
    assert!(page.starts_with("101\u{8}"), "{page}");
    assert!(boards[1].content.starts_with("102\u{8}"));
    assert!(boards[2].content.starts_with("103\u{8}"));
    page.trim_start_matches("101\u{8}").to_owned()
}

fn persisted(outs: &[Out]) -> Vec<PersistCommand> {
    outs.iter()
        .filter_map(|o| match o {
            Out::Persist(p) => Some(p.clone()),
            _ => None,
        })
        .collect()
}

fn announcement_writes(outs: &[Out]) -> Vec<AnnouncementWrite> {
    persisted(outs)
        .into_iter()
        .filter_map(|p| match p {
            PersistCommand::Announcement(w) => Some(w),
            _ => None,
        })
        .collect()
}

fn record(id: i32, kind: i32, content: &str) -> AnnouncementRecord {
    AnnouncementRecord {
        id,
        kind,
        initial: 0,
        delay: 0,
        repeat: 0,
        author: "Java GM".to_owned(),
        content: content.to_owned(),
    }
}

// --------------------------------------------------------------------------- announce

#[test]
fn announce_goes_to_everybody() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    s.cmd(GM, "announce  Hello   world");
    let out = s.log.take();
    let expected = vec![(
        ChatType::Announcement.client_id(),
        0,
        String::new(),
        "Hello world".to_owned(),
    )];
    assert_eq!(says_to(&out, P1), expected);
    assert_eq!(says_to(&out, GM), expected);
    // The GM sees the GM page after it.
    assert!(html_to(&out, GM)[0].contains("GM menu"));

    s.cmd(GM, "announce_crit Danger");
    assert_eq!(
        says_to(&s.log.take(), P1),
        [(
            ChatType::CriticalAnnounce.client_id(),
            0,
            String::new(),
            "Danger".to_owned()
        )]
    );

    s.cmd(GM, "announce_screen Look up");
    let out = s.log.take();
    let screen = decoded::<ExShowScreenMessage>(&out, P1, "EX_SHOW_SCREEN_MESSAGE");
    assert_eq!(screen.len(), 1);
    assert_eq!(screen[0].text, "Look up");
    assert_eq!(screen[0].time, 10_000);

    s.cmd(GM, "announce");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Syntax: //announce <text to announce here>"]
    );
}

#[test]
fn the_announcer_name_is_added_with_the_setting() {
    let mut s = Scene::with(AdminSettings {
        gm_announcer_name: true,
        ..AdminSettings::default()
    });
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    s.cmd(GM, "announce Hi");
    assert_eq!(says_to(&s.log.take(), P1)[0].3, "Hi [P100]");
    // The screen message has no name.
    s.cmd(GM, "announce_screen Hi");
    let screen = decoded::<ExShowScreenMessage>(&s.log.take(), P1, "EX_SHOW_SCREEN_MESSAGE");
    assert_eq!(screen[0].text, "Hi");
}

// ------------------------------------------------------------------------------ announces

#[test]
fn adding_waits_for_the_id_of_the_row() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    s.outs.take();

    // Initial delay, delay and repeat, then the text.
    s.cmd(GM, "announces add normal 0 0 0 Hello people");
    let writes = announcement_writes(&s.outs.take());
    let [
        AnnouncementWrite::Insert {
            ticket,
            kind,
            content,
            author,
            auto,
        },
    ] = writes.as_slice()
    else {
        panic!("{writes:?}");
    };
    assert_eq!(
        (*kind, content.as_str(), author.as_str(), *auto),
        (0, "Hello people", "P100", None)
    );
    // Nothing is said until the database answered.
    assert!(texts_to(&s.log.take(), GM).is_empty());
    assert_eq!(s.world.announcement_count(), 0);

    s.world.handle(WorldCommand::AnnouncementStored {
        ticket: *ticket,
        result: Ok(7),
    });
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Announcement has been successfully added!"]);
    let page = board_page(&out, GM);
    assert!(page.contains("<td width=80>7</td>"), "{page}");
    assert!(page.contains("<td width=100>NORMAL</td>"));
    assert!(page.contains("<td width=100>P100</td>"));
    assert_eq!(s.world.announcement_count(), 1);

    // A player who enters is shown it, with its own name as the sender.
    s.enter(P1);
    let out = s.log.take();
    let says = says_to(&out, P1);
    assert!(
        says.iter()
            .any(|s| s.0 == ChatType::Announcement.client_id() && s.2 == "P1" && s.3 == "Hello people"),
        "{says:?}"
    );
}

#[test]
fn a_failed_insert_adds_nothing_but_java_still_says_it_worked() {
    let mut s = Scene::new();
    s.enter(GM);
    s.cmd(GM, "announces add critical 0 0 0 Boom");
    let writes = announcement_writes(&s.outs.take());
    let [AnnouncementWrite::Insert { ticket, .. }] = writes.as_slice() else {
        panic!("{writes:?}");
    };
    s.log.take();
    s.world.handle(WorldCommand::AnnouncementStored {
        ticket: *ticket,
        result: Err(()),
    });
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Announcement has been successfully added!"]
    );
    assert_eq!(s.world.announcement_count(), 0);
}

#[test]
fn add_checks_its_arguments() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    s.outs.take();
    let syntax = "Syntax: //announces add <type> <delay> <repeat> <text>";
    for command in [
        "announces add normal",
        "announces add normal x 0 t",
        "announces add normal 0",
        "announces add normal 0 x t",
        "announces add normal 0 0",
        "announces add normal 0 0 y t",
    ] {
        s.cmd(GM, command);
        assert_eq!(texts_to(&s.log.take(), GM), [syntax], "{command}");
    }
    s.cmd(GM, "announces add auto_normal 0 9 0 too fast");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Delay cannot be less then 10 seconds!"]
    );
    // A normal announcement may have any delay.
    s.cmd(GM, "announces add normal 0 9 0 fine");
    assert_eq!(announcement_writes(&s.outs.take()).len(), 1);
    // The bare form shows the page for the form.
    s.cmd(GM, "announces add");
    assert_eq!(board_page(&s.log.take(), GM), "<html><body>add</body></html>");
}

#[test]
fn an_automatic_announcement_runs_on_the_timer_wheel() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    s.outs.take();
    // Initial delay 1 s, delay 10 s, twice.
    s.cmd(GM, "announces add auto_critical 1 10 2 Buy now");
    let writes = announcement_writes(&s.outs.take());
    let [
        AnnouncementWrite::Insert {
            ticket, kind, auto, ..
        },
    ] = writes.as_slice()
    else {
        panic!("{writes:?}");
    };
    assert_eq!((*kind, *auto), (4, Some((1_000, 10_000, 2))));
    s.world.handle(WorldCommand::AnnouncementStored {
        ticket: *ticket,
        result: Ok(8),
    });
    s.log.take();

    let critical = |out: &Sent| {
        says_to(out, P1)
            .into_iter()
            .filter(|s| s.0 == ChatType::CriticalAnnounce.client_id() && s.3 == "Buy now")
            .count()
    };
    assert_eq!(critical(&s.run(900)), 0);
    assert_eq!(critical(&s.run(200)), 1);
    assert_eq!(critical(&s.run(9_800)), 0);
    assert_eq!(critical(&s.run(300)), 1);
    // The repeat is used up.
    assert_eq!(critical(&s.run(60_000)), 0);

    // Restarting starts the repeats again, after the initial delay.
    s.cmd(GM, "announces restart 8");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Auto announcement has been successfully restarted"]
    );
    assert_eq!(critical(&s.run(1_100)), 1);
}

#[test]
fn an_automatic_announcement_from_the_table_splits_its_lines() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    let separator = if cfg!(windows) { "\r\n" } else { "\n" };
    s.world.load_announcements(vec![AnnouncementRecord {
        initial: 1_000,
        delay: 10_000,
        repeat: -1,
        ..record(3, 3, &format!("one{separator}two{separator}"))
    }]);
    s.log.take();
    let out = s.run(1_100);
    let says: Vec<String> = says_to(&out, P1).into_iter().map(|s| s.3).collect();
    assert_eq!(says, ["one", "two"]);
    // Forever.
    let out = s.run(30_000);
    assert_eq!(says_to(&out, P1).len(), 6);
}

#[test]
fn the_table_shows_normal_then_critical_and_skips_events() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.load_announcements(vec![
        record(1, 1, "critical one"),
        record(2, 0, "normal one"),
        record(3, 2, "an event row is not loaded"),
        record(4, 0, "normal two"),
    ]);
    assert_eq!(s.world.announcement_count(), 3);
    s.enter(P1);
    let out = s.log.take();
    let texts: Vec<(i32, String)> = says_to(&out, P1).into_iter().map(|s| (s.0, s.3)).collect();
    assert_eq!(
        texts,
        [
            (ChatType::Announcement.client_id(), "normal one".to_owned()),
            (ChatType::Announcement.client_id(), "normal two".to_owned()),
            (ChatType::CriticalAnnounce.client_id(), "critical one".to_owned()),
        ]
    );
}

#[test]
fn the_list_pages_and_the_buttons() {
    let mut s = Scene::new();
    s.enter(GM);
    let mut rows = vec![AnnouncementRecord {
        initial: 5_000,
        delay: 10_000,
        repeat: -1,
        ..record(1, 3, "auto")
    }];
    for id in 2..=9 {
        rows.push(record(id, 0, "n"));
    }
    s.world.load_announcements(rows);
    s.log.take();

    s.cmd(GM, "announces list");
    let page = board_page(&s.log.take(), GM);
    // Eight rows, then the pager of two pages.
    assert_eq!(page.matches("<tr><td width=5></td>").count(), 8);
    assert!(page.contains("admin_announces restart 1"));
    assert!(page.contains("admin_announces show 1"));
    assert!(page.contains("admin_announces edit 1"));
    assert!(page.contains("admin_announces remove 1"));
    assert!(!page.contains("admin_announces restart 2"));
    assert!(page.contains("Page: 1/2"));
    assert!(page.contains("bypass admin_announces list page=1"));
    assert!(!page.contains("admin_announces show 9"));

    s.cmd(GM, "announces list page=1");
    let page = board_page(&s.log.take(), GM);
    assert_eq!(page.matches("<tr><td width=5></td>").count(), 1);
    assert!(page.contains("admin_announces show 9"));
    assert!(page.contains("Page: 2/2"));
}

#[test]
fn show_edit_and_remove() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.load_announcements(vec![
        record(1, 0, "plain"),
        AnnouncementRecord {
            initial: 5_000,
            delay: 20_000,
            repeat: 3,
            ..record(2, 3, "auto")
        },
    ]);
    s.log.take();
    s.outs.take();

    s.cmd(GM, "announces show 2");
    assert_eq!(
        board_page(&s.log.take(), GM),
        "<html><body>show 2 AUTO_NORMAL 5 20 3 Java GM auto</body></html>"
    );
    s.cmd(GM, "announces show 77");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Announcement does not exist!"]);
    assert!(board_page(&out, GM).contains("<td width=80>1</td>"));
    s.cmd(GM, "announces show x");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Syntax: //announces show <announcement id>"]
    );

    // The edit page, and an edit of a normal one into a critical one.
    s.cmd(GM, "announces edit 1");
    assert_eq!(
        board_page(&s.log.take(), GM),
        "<html><body>edit 1 NORMAL 0 0 0 Java GM plain</body></html>"
    );
    s.cmd(GM, "announces edit 1 critical 0 0 0 changed text");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Announcement has been successfully edited!"]);
    assert_eq!(
        announcement_writes(&s.outs.take()),
        [AnnouncementWrite::Update {
            id: 1,
            kind: 1,
            content: "changed text".to_owned(),
            author: "P100".to_owned(),
            auto: None,
        }]
    );
    // The list that follows has the new type and author.
    let page = board_page(&out, GM);
    assert!(page.contains("<td width=100>CRITICAL</td>"));

    // The type may only change inside its family.
    s.cmd(GM, "announces edit 1 auto_normal 0 20 0");
    s.cmd(GM, "announces edit 2 normal 0 20 0");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Announce type can be changed only to NORMAL or CRITICAL!",
            "Announce type can be changed only to AUTO_NORMAL or AUTO_CRITICAL!"
        ]
    );
    s.cmd(GM, "announces edit 2 auto_critical 0 5 0");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Delay cannot be less then 10 seconds!"]
    );
    // An automatic one: the old text stays, a repeat of 0 is forever.
    s.cmd(GM, "announces edit 2 auto_critical 2 30 0");
    s.log.take();
    assert_eq!(
        announcement_writes(&s.outs.take()),
        [AnnouncementWrite::Update {
            id: 2,
            kind: 4,
            content: "auto".to_owned(),
            author: "P100".to_owned(),
            auto: Some((2_000, 30_000, -1)),
        }]
    );
    s.cmd(GM, "announces edit 9");
    assert_eq!(texts_to(&s.log.take(), GM), ["Announcement does not exist!"]);

    s.cmd(GM, "announces remove 1");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["Announcement has been successfully removed!"]
    );
    assert_eq!(
        announcement_writes(&s.outs.take()),
        [AnnouncementWrite::Delete { id: 1 }]
    );
    assert_eq!(s.world.announcement_count(), 1);
    s.cmd(GM, "announces remove 1");
    assert_eq!(texts_to(&s.log.take(), GM), ["Announcement does not exist!"]);
}

#[test]
fn restart_has_a_message_for_each_case() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.load_announcements(vec![
        record(1, 0, "plain"),
        AnnouncementRecord {
            initial: 5_000,
            delay: 20_000,
            repeat: 3,
            ..record(2, 3, "auto")
        },
    ]);
    s.log.take();
    s.cmd(GM, "announces restart");
    s.cmd(GM, "announces restart 1");
    s.cmd(GM, "announces restart 2");
    s.cmd(GM, "announces restart 99");
    s.cmd(GM, "announces restart x");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Auto announcements has been successfully restarted",
            "This option has effect only on auto announcements!",
            "Auto announcement has been successfully restarted",
            "Announcement does not exist!",
            "Syntax: //announces show <announcement id>",
        ]
    );
}

// ------------------------------------------------------------------------------ petitions

fn petition_scene() -> Scene {
    let mut s = Scene::with(AdminSettings {
        gm_startup_auto_list: true,
        max_petitions_per_player: 2,
        ..AdminSettings::default()
    });
    for id in [GM, P1, P2] {
        s.enter(id);
    }
    s.log.take();
    s.outs.take();
    s
}

fn submit(s: &mut Scene, player: i32, text: &str, kind: i32) {
    s.input(
        player,
        ClientInput::Petition(PetitionInput::Submit {
            content: text.to_owned(),
            kind,
        }),
    );
}

fn cancel(s: &mut Scene, player: i32) {
    s.input(player, ClientInput::Petition(PetitionInput::Cancel));
}

/// The petition id from the receipt message.
fn receipt(out: &Sent, to: i32) -> i32 {
    system_to(out, to)
        .into_iter()
        .find(|m| m.id == SystemMessageId::YourPetitionApplicationHasBeenAcceptedNReceiptNoIsS1)
        .map(|m| match m.params.as_slice() {
            [SystemMessageParam::IntNumber(id)] => *id,
            other => panic!("{other:?}"),
        })
        .expect("a receipt")
}

#[test]
fn a_petition_needs_a_visible_gm() {
    // The GM list is hidden unless `GMStartupAutoList` is on.
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    submit(&mut s, P1, "help", 1);
    assert_eq!(
        system_to(&s.log.take(), P1)[0].id,
        SystemMessageId::ThereAreNoGmsCurrentlyVisibleInThePublicListAsTheyMayBePerformingOtherFunctionsAtTheMoment
    );
}

#[test]
fn submitting_tells_the_gms_and_the_player() {
    let mut s = petition_scene();
    submit(&mut s, P1, "I am stuck", 3);
    let out = s.log.take();
    let id = receipt(&out, P1);
    let messages = system_to(&out, P1);
    assert_eq!(messages.len(), 3);
    // "You have submitted 1 petition(s), you may submit 1 more" and "1 petition on the list".
    assert_eq!(
        messages[1].params,
        [SystemMessageParam::IntNumber(1), SystemMessageParam::IntNumber(1)]
    );
    assert_eq!(messages[2].params, [SystemMessageParam::IntNumber(1)]);
    // The GM hears it as hero voice from "Petition System".
    let says = says_to(&out, GM);
    assert_eq!(says.len(), 1);
    assert_eq!(says[0].0, ChatType::HeroVoice.client_id());
    assert_eq!(says[0].1, P1);
    assert_eq!(says[0].2, "Petition System");
    assert_eq!(says[0].3, "P1 has submitted a new petition.");

    // One at a time.
    submit(&mut s, P1, "again", 3);
    assert_eq!(
        system_to(&s.log.take(), P1)[0].id,
        SystemMessageId::YouMayOnlySubmitOnePetitionActiveAtATime
    );

    // The GM's list and view.
    s.cmd(GM, "view_petitions");
    let page = &html_to(&s.log.take(), GM)[0];
    assert!(page.contains(&format!("admin_view_petition {id}")));
    assert!(page.contains(&format!("admin_reject_petition {id}")));
    assert!(page.contains(">P1</font>"));
    assert!(page.contains("RECOVERY RELATED") || page.contains("BUG REPORT"));
    s.cmd(GM, &format!("view_petition {id}"));
    let page = &html_to(&s.log.take(), GM)[0];
    assert!(page.contains(&format!("<html><body>{id}|")));
    assert!(page.contains("|BUG REPORT|P1|00FF00|I am stuck"));
    // Nobody else may look.
    s.cmd(GM, "view_petition 12345");
    assert!(html_to(&s.log.take(), GM).is_empty());
}

#[test]
fn the_limits_of_a_petition() {
    let mut s = petition_scene();
    // The type is 1 to 9.
    submit(&mut s, P1, "x", 0);
    submit(&mut s, P1, "x", 10);
    assert!(s.log.take().is_empty());
    // At most 255 characters.
    submit(&mut s, P1, &"x".repeat(256), 1);
    assert_eq!(
        system_to(&s.log.take(), P1)[0].id,
        SystemMessageId::PetitionsCannotExceed255Characters
    );
    // Two a day: pending and completed ones count.
    for _ in 0..2 {
        submit(&mut s, P1, "again", 1);
        cancel(&mut s, P1);
    }
    s.log.take();
    submit(&mut s, P1, "once more", 1);
    let messages = system_to(&s.log.take(), P1);
    assert_eq!(
        messages[0].id,
        SystemMessageId::WeHaveReceivedS1PetitionsFromYouTodayAndThatIsTheMaximumThatYouCanSubmitInOneDayYouCannotSubmitAnyMorePetitions
    );
    assert_eq!(messages[0].params, [SystemMessageParam::IntNumber(3)]);
}

#[test]
fn the_queue_has_a_size_and_a_switch() {
    let mut s = Scene::with(AdminSettings {
        gm_startup_auto_list: true,
        max_petitions_pending: 1,
        ..AdminSettings::default()
    });
    for id in [GM, P1, P2] {
        s.enter(id);
    }
    s.log.take();
    submit(&mut s, P1, "first", 1);
    s.log.take();
    submit(&mut s, P2, "second", 1);
    assert_eq!(
        system_to(&s.log.take(), P2)[0].id,
        SystemMessageId::ThePetitionSystemIsCurrentlyUnavailablePleaseTryAgainLater
    );

    let mut off = Scene::with(AdminSettings {
        gm_startup_auto_list: true,
        petitioning_allowed: false,
        ..AdminSettings::default()
    });
    for id in [GM, P1] {
        off.enter(id);
    }
    off.log.take();
    submit(&mut off, P1, "help", 1);
    assert_eq!(
        system_to(&off.log.take(), P1)[0].id,
        SystemMessageId::TheGameClientEncounteredAnErrorAndWasUnableToConnectToThePetitionServer
    );
}

#[test]
fn cancelling_a_pending_petition() {
    let mut s = petition_scene();
    // Nothing to cancel.
    cancel(&mut s, P1);
    assert_eq!(
        system_to(&s.log.take(), P1)[0].id,
        SystemMessageId::YouHaveNotSubmittedAPetition
    );

    submit(&mut s, P1, "help", 1);
    s.log.take();
    cancel(&mut s, P1);
    let out = s.log.take();
    let messages = system_to(&out, P1);
    // The end of the consultation and the feedback button, then the count.
    assert_eq!(
        messages[0].id,
        SystemMessageId::ThisEndsTheGmPetitionConsultationNpleaseGiveUsFeedbackOnThePetitionService
    );
    assert_eq!(count(&out, P1, "PETITION_VOTE"), 1);
    assert_eq!(
        messages[1].id,
        SystemMessageId::ThePetitionWasCanceledYouMaySubmitS1MorePetitionSToday
    );
    // 2 allowed, 1 used (the cancelled one counts).
    assert_eq!(messages[1].params, [SystemMessageParam::Text("1".to_owned())]);
    assert_eq!(says_to(&out, GM)[0].3, "P1 has canceled a pending petition.");
    s.cmd(GM, "view_petitions");
    assert!(html_to(&s.log.take(), GM)[0].contains("There are no currently pending petitions."));
}

#[test]
fn a_gm_answers_a_petition() {
    let mut s = petition_scene();
    submit(&mut s, P1, "help", 5);
    let id = receipt(&s.log.take(), P1);

    s.cmd(GM, &format!("accept_petition {id}"));
    let out = s.log.take();
    assert_eq!(
        system_to(&out, P1)[0].id,
        SystemMessageId::PetitionApplicationAccepted
    );
    let gm_messages = system_to(&out, GM);
    assert_eq!(
        gm_messages[0].id,
        SystemMessageId::YourPetitionApplicationHasBeenAcceptedNReceiptNoIsS1
    );
    assert_eq!(gm_messages[0].params, [SystemMessageParam::IntNumber(id)]);
    assert_eq!(
        gm_messages[1].id,
        SystemMessageId::StartingPetitionConsultationWithC1
    );
    assert_eq!(gm_messages[1].params, [SystemMessageParam::Text("P1".to_owned())]);

    // The petition is taken: another GM is told, the GM cannot take two.
    s.enter(GM2);
    s.log.take();
    s.cmd(GM2, &format!("accept_petition {id}"));
    assert_eq!(
        system_to(&s.log.take(), GM2)[0].id,
        SystemMessageId::YourPetitionIsBeingProcessed
    );
    submit(&mut s, P2, "me too", 1);
    let id2 = receipt(&s.log.take(), P2);
    s.cmd(GM, &format!("accept_petition {id2}"));
    assert_eq!(
        system_to(&s.log.take(), GM)[0].id,
        SystemMessageId::YouMayOnlySubmitOnePetitionActiveAtATime
    );
    // A petition that does not exist.
    s.cmd(GM2, "accept_petition 4242");
    assert_eq!(
        system_to(&s.log.take(), GM2)[0].id,
        SystemMessageId::NotUnderPetitionConsultation
    );

    // The consultation chat: the petitioner's channel is 6, the GM's answer is turned into 7.
    s.say(P1, ChatType::PetitionPlayer, "I am here");
    let out = s.log.take();
    for to in [P1, GM] {
        assert_eq!(
            says_to(&out, to),
            [(
                ChatType::PetitionPlayer.client_id(),
                P1,
                "P1".to_owned(),
                "I am here".to_owned()
            )]
        );
    }
    s.say(GM, ChatType::PetitionPlayer, "How can I help?");
    let out = s.log.take();
    for to in [P1, GM] {
        assert_eq!(
            says_to(&out, to),
            [(
                ChatType::PetitionGm.client_id(),
                GM,
                "P100".to_owned(),
                "How can I help?".to_owned()
            )]
        );
    }
    // Somebody who is not in a consultation.
    s.say(P2, ChatType::PetitionPlayer, "hello?");
    assert_eq!(
        system_to(&s.log.take(), P2)[0].id,
        SystemMessageId::YouAreCurrentlyNotInAPetitionChat
    );

    // The petitioner cannot end it, the GM can.
    cancel(&mut s, P1);
    assert_eq!(
        system_to(&s.log.take(), P1)[0].id,
        SystemMessageId::YourPetitionIsBeingProcessed
    );
    cancel(&mut s, GM);
    let out = s.log.take();
    assert_eq!(
        system_to(&out, GM)[0].id,
        SystemMessageId::EndingPetitionConsultationWithC1
    );
    assert_eq!(count(&out, P1, "PETITION_VOTE"), 1);
}

#[test]
fn feedback_names_the_gm_who_answered() {
    let mut s = petition_scene();
    // No GM answered yet: nothing is stored.
    s.input(
        P1,
        ClientInput::Petition(PetitionInput::Feedback {
            rate: 4,
            message: "x".to_owned(),
        }),
    );
    assert!(s.outs.take().is_empty());

    submit(&mut s, P1, "help", 5);
    let id = receipt(&s.log.take(), P1);
    s.cmd(GM, &format!("accept_petition {id}"));
    s.outs.take();
    // The rate is 0 to 4.
    s.input(
        P1,
        ClientInput::Petition(PetitionInput::Feedback {
            rate: 5,
            message: "x".to_owned(),
        }),
    );
    assert!(s.outs.take().is_empty());
    s.input(
        P1,
        ClientInput::Petition(PetitionInput::Feedback {
            rate: 4,
            message: "great".to_owned(),
        }),
    );
    let stored = persisted(&s.outs.take());
    assert!(matches!(
        stored.as_slice(),
        [PersistCommand::PetitionFeedback { character, gm, rate: 4, message, .. }]
            if character == "P1" && gm == "P100" && message == "great"
    ));
}

#[test]
fn rejecting_and_resetting() {
    let mut s = petition_scene();
    submit(&mut s, P1, "help", 1);
    let id = receipt(&s.log.take(), P1);
    submit(&mut s, P2, "me too", 1);
    let id2 = receipt(&s.log.take(), P2);

    s.cmd(GM, &format!("reject_petition {id}"));
    let out = s.log.take();
    assert!(texts_to(&out, P1).contains(&"Your petition was rejected. Please try again later.".to_owned()));
    assert_eq!(count(&out, P1, "PETITION_VOTE"), 1);
    // The list that follows has only the other one.
    let page = &html_to(&out, GM)[0];
    assert!(page.contains(&format!("admin_view_petition {id2}")));
    assert!(!page.contains(&format!("admin_view_petition {id}")));
    // Rejecting what is gone.
    s.cmd(GM, &format!("reject_petition {id}"));
    let out = s.log.take();
    assert_eq!(
        system_to(&out, GM)[0].id,
        SystemMessageId::FailedToCancelPetitionPleaseTryAgainLater
    );

    // Reset is refused while one is in process, and clears the queue otherwise.
    s.cmd(GM, &format!("accept_petition {id2}"));
    s.log.take();
    s.cmd(GM, "reset_petitions");
    assert_eq!(
        system_to(&s.log.take(), GM)[0].id,
        SystemMessageId::YourPetitionIsBeingProcessed
    );
    cancel(&mut s, GM);
    submit(&mut s, P1, "last", 1);
    s.log.take();
    s.cmd(GM, "reset_petitions");
    assert!(html_to(&s.log.take(), GM)[0].contains("There are no currently pending petitions."));
}

#[test]
fn force_peti_opens_a_consultation_with_the_target() {
    let mut s = petition_scene();
    s.cmd(GM, "force_peti hello");
    assert_eq!(
        system_to(&s.log.take(), GM)[0].id,
        SystemMessageId::ThatIsAnIncorrectTarget
    );
    s.target(GM, P1);
    s.log.take();
    s.cmd(GM, "force_peti hello");
    let out = s.log.take();
    assert_eq!(
        system_to(&out, P1)[0].id,
        SystemMessageId::PetitionApplicationAccepted
    );
    assert_eq!(
        system_to(&out, GM)[1].id,
        SystemMessageId::StartingPetitionConsultationWithC1
    );
    // NOTE(java-quirk): the text starts with the last letter of the command.
    s.cmd(GM, "view_petitions");
    let page = html_to(&s.log.take(), GM).remove(0);
    // The petition is in process: no buttons, the GM's name instead, and the type 9.
    assert!(page.contains(">P100</font>"));
    assert!(page.contains("OTHER"));
}

#[test]
fn a_petitioner_who_comes_back_gets_the_chat_again() {
    let mut s = petition_scene();
    submit(&mut s, P1, "help", 5);
    let id = receipt(&s.log.take(), P1);
    s.cmd(GM, &format!("accept_petition {id}"));
    s.say(GM, ChatType::PetitionPlayer, "Wait a moment");
    s.input(P1, ClientInput::Logout);
    s.log.take();
    s.enter(P1);
    let out = s.log.take();
    let says = says_to(&out, P1);
    assert!(
        says.contains(&(
            ChatType::PetitionGm.client_id(),
            GM,
            "P100".to_owned(),
            "Wait a moment".to_owned()
        )),
        "{says:?}"
    );
}

// --------------------------------------------------------------------------------- forge

/// The raw packets sent to `to` whose first byte is `opcode`.
fn raw_to(out: &Sent, to: i32, opcode: u8) -> Vec<Vec<u8>> {
    out.iter()
        .filter(|(t, b)| *t == to && b[0] == opcode)
        .map(|(_, b)| b.clone())
        .collect()
}

#[test]
fn the_forge_pages() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    s.cmd(GM, "forge");
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><body>forge main</body></html>"]
    );

    // One op code and a format: the editors and the bypass the page sends.
    s.cmd(GM, "forge_values 0x4a ; cdh");
    let page = html_to(&s.log.take(), GM).remove(0);
    assert_eq!(
        page,
        "<html><body>c|0x4a|cdh|<tr>c:0</tr><tr>d:1</tr><tr>h:2</tr>|0x4a cdh $v0 $v1 $v2</body></html>"
    );
    // Three op codes without a format.
    s.cmd(GM, "forge_values 1 2 3");
    assert_eq!(
        html_to(&s.log.take(), GM)[0],
        "<html><body>chd|1;2;3|||1;2;3</body></html>"
    );
    // The usage of a bad op code, a bad format and nothing at all.
    s.cmd(GM, "forge_values 300");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        [
            "Invalid op codes!",
            "Usage: //forge_values opcode1[ opcode2[ opcode3]] ;[ format]"
        ]
    );
    assert_eq!(html_to(&out, GM), ["<html><body>forge main</body></html>"]);
    s.cmd(GM, "forge_values 5 ; z");
    assert_eq!(texts_to(&s.log.take(), GM)[0], "Format invalid!");
    s.cmd(GM, "forge_values ");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //forge_values opcode1[ opcode2[ opcode3]] ;[ format]"]
    );
    // NOTE(java-quirk): a word is accepted as a second op code.
    s.cmd(GM, "forge_values 1 abc");
    assert_eq!(
        html_to(&s.log.take(), GM)[0],
        "<html><body>ch|1;abc|||1;abc</body></html>"
    );
}

#[test]
fn forge_send_builds_the_packet() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    // c 7, d 70000, s "Hello" after the op code 0x49.
    s.cmd(GM, "forge_send sc 0x49 cds 7 70000 Hello");
    let out = s.log.take();
    let mut expected = vec![0x49, 7];
    expected.extend_from_slice(&70_000_i32.to_le_bytes());
    for unit in "Hello".encode_utf16() {
        expected.extend_from_slice(&unit.to_le_bytes());
    }
    expected.extend_from_slice(&[0, 0]);
    assert_eq!(
        raw_to(&out, GM, 0x49).iter().filter(|b| **b == expected).count(),
        1
    );
    // Not broadcast.
    assert!(raw_to(&out, P1, 0x49).iter().all(|b| *b != expected));
    // The values page follows.
    assert!(html_to(&out, GM)[0].contains("|0x49|cds|"));

    // `$oid` is the GM's object id; the broadcast form reaches the players who see the GM too.
    s.cmd(GM, "forge_send sb 0x10;0x2000 d $oid");
    let out = s.log.take();
    let mut expected = vec![0x10, 0x00, 0x20];
    expected.extend_from_slice(&GM.to_le_bytes());
    assert!(raw_to(&out, GM, 0x10).contains(&expected));
    assert!(raw_to(&out, P1, 0x10).contains(&expected));

    // A decimal big number and a long.
    s.cmd(GM, "forge_send sc 0x11 bq 255 0x0102030405060708");
    let out = s.log.take();
    let mut expected = vec![0x11, 0x00, 0xFF];
    expected.extend_from_slice(&0x0102_0304_0506_0708_i64.to_le_bytes());
    assert!(raw_to(&out, GM, 0x11).contains(&expected));
}

#[test]
fn forge_send_complains_like_java() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    let usage = "Usage: //forge_send sc|sb|cs opcode1[;opcode2[;opcode3]][ format value1 ... valueN] ";
    s.cmd(GM, "forge_send zz 1");
    assert_eq!(texts_to(&s.log.take(), GM), ["Invalid method!", usage]);
    s.cmd(GM, "forge_send sc 300");
    assert_eq!(texts_to(&s.log.take(), GM), ["Invalid op codes!", usage]);
    s.cmd(GM, "forge_send sc 1 q");
    assert_eq!(texts_to(&s.log.take(), GM), ["Not enough values!", usage]);
    s.cmd(GM, "forge_send sc 1 z");
    assert_eq!(texts_to(&s.log.take(), GM), ["Format invalid!", usage]);
    s.cmd(GM, "forge_send sc");
    assert_eq!(texts_to(&s.log.take(), GM), [usage]);
    // `cs` is "not implemented yet": the usage.
    s.cmd(GM, "forge_send cs 1 d 5");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), [usage]);
    // A value that is not a number sends nothing (see the module notes of the forge).
    s.cmd(GM, "forge_send sc 0x12 d nope");
    let out = s.log.take();
    assert!(raw_to(&out, GM, 0x12).is_empty());
}

// ---------------------------------------------------------------------------------- debug

static DEBUG_CALLS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn record_call(_world: &mut World, _gm: ObjectId, command: &str) -> Result<(), JavaException> {
    DEBUG_CALLS.lock().unwrap().push(command.to_owned());
    Ok(())
}

fn calls() -> Vec<String> {
    std::mem::take(&mut DEBUG_CALLS.lock().unwrap())
}

/// The debug tests share `DEBUG_CALLS`; run them one at a time.
static DEBUG_SERIAL: Mutex<()> = Mutex::new(());

fn debug_serial() -> std::sync::MutexGuard<'static, ()> {
    DEBUG_SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn debug_menu_and_switches() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.register_admin_handler("admin_showdoors", record_call);
    s.world.register_admin_handler("admin_geogrid", record_call);
    let _serial = debug_serial();
    calls();
    s.log.take();

    s.cmd(GM, "debug");
    assert_eq!(
        html_to(&s.log.take(), GM)[0],
        "<html><body>Enable|packets on|Enable|doors on|Enable|geodata on|Enable|movement on</body></html>"
    );

    // The panel's buttons: a value, and "menu" shows the panel again.
    s.cmd(GM, "debug packets on menu");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Packet debugging on console is enabled."]);
    assert!(html_to(&out, GM)[0].starts_with("<html><body>Disable|packets off|"));
    // Without a value it toggles and shows nothing.
    s.cmd(GM, "debug packets");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Packet debugging on console is disabled."]);
    assert!(html_to(&out, GM).is_empty());

    s.cmd(GM, "debug nothing");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //debug <parameter> <value>"]
    );
}

#[test]
fn debug_doors_and_geodata_run_their_commands_when_the_gm_moved() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.register_admin_handler("admin_showdoors", record_call);
    s.world.register_admin_handler("admin_geogrid", record_call);
    let _serial = debug_serial();
    calls();
    s.log.take();

    s.cmd(GM, "debug doors on");
    s.cmd(GM, "debug geodata on");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Door debugging is enabled.", "Geodata debugging is enabled."]
    );
    assert_eq!(calls(), ["admin_showdoors", "admin_geogrid"]);
    // Standing still: nothing for a while.
    s.run(6_000);
    assert!(calls().is_empty());
    // 16 units away: the next checks run the commands again.
    s.world.player_mut(ObjectId(GM)).unwrap().core.x += 100;
    s.run(3_100);
    let mut now = calls();
    now.sort();
    assert_eq!(now, ["admin_geogrid", "admin_showdoors"]);

    // Off: the packets of the displays are taken away a moment later.
    s.cmd(GM, "debug doors off");
    s.cmd(GM, "debug geodata off");
    assert_eq!(calls(), Vec::<String>::new());
    s.run(1_700);
    assert_eq!(calls(), ["admin_geogrid off"]);
    s.run(1_500);
    assert_eq!(calls(), ["admin_showdoors off"]);
    // And the checks are over.
    s.world.player_mut(ObjectId(GM)).unwrap().core.x += 100;
    s.run(6_000);
    assert!(calls().is_empty());
}

#[test]
fn debug_movement_clears_its_line() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    s.cmd(GM, "debug movement on");
    assert_eq!(texts_to(&s.log.take(), GM), ["Movement debugging is enabled."]);
    s.cmd(GM, "debug movement off");
    let out = s.run(1_700);
    assert_eq!(texts_to(&out, GM), ["Movement debugging is disabled."]);
    // The black line that wipes the drawing.
    let primitives = decoded::<l2_protocol_game::server::ExServerPrimitive>(&out, GM, "EX_SERVER_PRIMITIVE");
    assert_eq!(primitives.len(), 1);
    assert_eq!(primitives[0].name, "DebugMove");
    assert_eq!(primitives[0].lines().len(), 1);
    assert_eq!(primitives[0].lines()[0].color.0 & 0xFF_FFFF, 0);
}

#[test]
fn debug_stops_when_the_gm_leaves() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.register_admin_handler("admin_showdoors", record_call);
    let _serial = debug_serial();
    calls();
    s.cmd(GM, "debug doors on");
    s.input(GM, ClientInput::Logout);
    calls();
    s.run(10_000);
    assert!(calls().is_empty());
}

// ------------------------------------------------------------------------------- the pages

/// Every `bypass` link of the pages the handlers send must reach a command. The links to
/// commands of other cards (and the dead ones of Java) are listed; every other one is ours and
/// has to be registered.
#[test]
fn the_links_of_the_pages_reach_registered_commands() {
    let dir = std::path::Path::new(r"C:\l2-rust\l2jmobius\dist\game\data\html\admin");
    if !dir.is_dir() {
        eprintln!("skipped: no Java html at {}", dir.display());
        return;
    }
    let pages = [
        "login.htm",
        "shutdown.htm",
        "gm_menu.htm",
        "announces-list.htm",
        "announces-show.htm",
        "announces-edit.htm",
        "announces-add.htm",
        "petition.htm",
        "reload.htm",
        "serverinfo.htm",
        "debug.htm",
        "pforge/main.htm",
        "pforge/values.htm",
        "pforge/inc/editor.htm",
    ];
    // Commands of other cards, and `server_list_clock`, a link Java's AdminLogin does not handle.
    let others = [
        "admin_admin",
        "admin_admin2",
        "admin_admin3",
        "admin_admin4",
        "admin_admin5",
        "admin_admin6",
        "admin_admin7",
        "admin_html",
        "admin_help",
        "admin_superhaste",
        "admin_gmliston",
        "admin_gmlistoff",
        "admin_diet",
        "admin_invis_menu",
        "admin_silence",
        "admin_tradeoff",
        "admin_invul",
        "admin_exceptions",
        "admin_enchant",
        "admin_gmshop",
        "admin_event_menu",
        "admin_ride_wolf",
        "admin_ride_strider",
        "admin_ride_wyvern",
        "admin_ride_horse",
        "admin_ride_bike",
        "admin_server_list_clock",
        "admin_showdoors",
        "admin_geogrid",
        "admin_config_server",
        "admin_msg",
        "admin_openall",
        "admin_closeall",
        "admin_geo_z",
        "admin_geo_type",
        "admin_geo_position",
        "admin_geo_nswe",
        "admin_geo_los",
        "admin_zone_check",
        "admin_zone_build",
        "admin_spawn_debug_menu",
        "admin_skill_test",
        "admin_fight_calculator_show",
        "admin_admin1",
    ];
    let mut ours = std::collections::BTreeSet::new();
    for page in pages {
        let text = std::fs::read_to_string(dir.join(page)).unwrap();
        for (start, _) in text.match_indices("admin_") {
            let word: String = text[start..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !others.contains(&word.as_str()) {
                ours.insert(word);
            }
        }
    }
    let known: Vec<String> = COMMANDS.iter().map(|c| format!("admin_{c}")).collect();
    let unknown: Vec<&String> = ours.iter().filter(|w| !known.contains(w)).collect();
    assert!(
        unknown.is_empty(),
        "links to commands nobody registers: {unknown:?}"
    );
}
