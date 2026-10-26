#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The server control and GM tool commands of M7-ADMIN-10: kick, disconnect, `//gm`, GM chat,
//! snooping, `//online`, `//serverinfo`, the login server switches, the shutdown countdown and
//! the reload panel.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminKick,AdminDisconnect,AdminGm,
//! AdminGmChat,AdminOnline,AdminServerInfo,AdminLogin,AdminShutdown,AdminReload}.java,
//! gameserver/Shutdown.java.

mod admin_common;

use std::net::IpAddr;
use std::sync::Arc;

use admin_common::*;
use l2_data::HtmCache;
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, SystemMessageParam};
use l2_world::*;

const NO_COMMAND: &str = "does not exist!";

/// The text of the exception message the dispatcher sends.
fn exception_of(texts: &[String]) -> Option<&String> {
    texts
        .iter()
        .find(|t| t.starts_with("Exception during execution of"))
}

#[test]
fn every_command_of_the_handlers_is_registered() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    // `gm` ends the GM status, so it is the last one.
    for command in COMMANDS.iter().filter(|c| **c != "gm") {
        s.cmd(GM, command);
        let out = s.log.take();
        let texts = texts_to(&out, GM);
        assert!(
            !texts.iter().any(|t| t.contains(NO_COMMAND)),
            "//{command} is not registered: {texts:?}"
        );
    }
    s.cmd(GM, "gm");
    assert!(!texts_to(&s.log.take(), GM).iter().any(|t| t.contains(NO_COMMAND)));
}

#[test]
fn the_commands_of_other_cards_stay_unknown() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    // The HWID commands stay out (card text): Java's answer to an unknown command.
    s.cmd(GM, "hwid");
    s.cmd(GM, "hwinfo");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "The command 'hwid' does not exist!",
            "The command 'hwinfo' does not exist!"
        ]
    );
}

// ------------------------------------------------------------------------------------ kick

#[test]
fn kick_stores_the_player_and_sends_leave_world() {
    let mut s = Scene::new();
    for id in [GM, P1, P2] {
        s.enter(id);
    }
    s.log.take();
    s.outs.take();
    s.lefts.take();

    s.cmd(GM, "kick P1");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["You kicked P1 from the game."]);
    // `storeAndDeleteWith(LeaveWorld.STATIC_PACKET)`: the packet, then the connection closes.
    assert_eq!(count(&out, P1, ServerOpcode::LogOutOk.name()), 1);
    assert_eq!(s.lefts.take(), [(P1, LeaveReason::Logout)]);
    assert!(!s.is_online(P1));
    assert!(s.is_online(P2));
    let outs = s.outs.take();
    assert!(
        outs.iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::StorePlayer(p)) if p.object_id.0 == P1))
    );
    assert!(outs.iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::PlayerLeft { account, logout: true, .. }) if account == "acc1"
    )));

    // An unknown name and a command without a name do nothing.
    s.cmd(GM, "kick Nobody");
    s.cmd(GM, "kick");
    assert!(texts_to(&s.log.take(), GM).is_empty());
    assert!(s.is_online(P2));
}

#[test]
fn kick_non_gm_keeps_the_gms() {
    let mut s = Scene::new();
    for id in [GM, GM2, P1, P2, P3] {
        s.enter(id);
    }
    s.log.take();
    s.cmd(GM, "kick_non_gm");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Kicked 3 players."]);
    for gone in [P1, P2, P3] {
        assert!(!s.is_online(gone));
        assert_eq!(count(&out, gone, ServerOpcode::LogOutOk.name()), 1);
    }
    assert!(s.is_online(GM) && s.is_online(GM2));
}

#[test]
fn character_disconnect_takes_the_target() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    // Without a target nothing happens.
    s.log.take();
    s.cmd(GM, "character_disconnect");
    assert!(texts_to(&s.log.take(), GM).is_empty());

    s.target(GM, P1);
    s.log.take();
    s.cmd(GM, "character_disconnect");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Character P1 disconnected from server."]);
    assert!(!s.is_online(P1));
}

// --------------------------------------------------------------------------------- gm tools

#[test]
fn gm_chat_reaches_the_gms_only() {
    let mut s = Scene::new();
    for id in [GM, GM2, P1] {
        s.enter(id);
    }
    s.log.take();
    s.cmd(GM, "gmchat hello  there");
    let out = s.log.take();
    // Alliance chat (9) from "P100" with no sender object, the text as typed.
    let expected = vec![(
        ChatType::Alliance.client_id(),
        0,
        "P100".to_owned(),
        "hello  there".to_owned(),
    )];
    assert_eq!(says_to(&out, GM), expected);
    assert_eq!(says_to(&out, GM2), expected);
    assert!(says_to(&out, P1).is_empty());

    // The menu form cuts at 18 and shows the GM page.
    s.cmd(GM, "gmchat_menu menu text");
    let out = s.log.take();
    assert_eq!(says_to(&out, GM2)[0].3, "menu text");
    assert_eq!(html_to(&out, GM).len(), 1);
    assert!(html_to(&out, GM)[0].contains("GM menu"));

    // A command too short for the offset is ignored.
    s.cmd(GM, "gmchat");
    assert!(says_to(&s.log.take(), GM).is_empty());
}

#[test]
fn gm_turns_the_gm_status_off() {
    let mut s = Scene::new();
    for id in [GM, GM2] {
        s.enter(id);
    }
    s.log.take();
    s.cmd(GM, "gm");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["You no longer have GM status."]);
    assert_eq!(s.world.player(ObjectId(GM)).unwrap().data.access_level, 0);
    // No longer a GM: no commands, and not in the GM list.
    s.cmd(GM, "online");
    assert!(texts_to(&s.log.take(), GM).is_empty());
    s.cmd(GM2, "gmchat psst");
    let out = s.log.take();
    assert!(says_to(&out, GM).is_empty());
    assert_eq!(says_to(&out, GM2).len(), 1);
}

#[test]
fn online_reports_the_counters() {
    let mut s = Scene::new();
    let ip1: IpAddr = "10.0.0.1".parse().unwrap();
    let ip2: IpAddr = "10.0.0.2".parse().unwrap();
    s.enter_from(GM, Some(ip1));
    s.enter_from(P1, Some(ip1));
    s.enter_from(P2, Some(ip2));
    s.log.take();
    s.cmd(GM, "online");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Online Player Report",
            "Total count: 3",
            "Total online: 3",
            "Total offline: 0",
            "Max connected: 3",
            "Unique IPs: 2",
            "In peace zone: 0",
            "Not in peace zone: 3",
            "In instances: 0",
            "In combat: 0",
        ]
    );
    // The maximum stays when players leave.
    s.input(P1, ClientInput::Logout);
    s.log.take();
    s.cmd(GM, "online");
    let texts = texts_to(&s.log.take(), GM);
    assert_eq!(texts[1], "Total count: 2");
    assert_eq!(texts[4], "Max connected: 3");
}

#[test]
fn serverinfo_fills_the_page() {
    let mut s = Scene::with(AdminSettings {
        maximum_online_users: 50,
        ..AdminSettings::default()
    });
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    s.cmd(GM, "serverinfo");
    let pages = html_to(&s.log.take(), GM);
    assert_eq!(pages.len(), 1);
    let fields: Vec<&str> = pages[0]
        .trim_start_matches("<html><body>")
        .trim_end_matches("</body></html>")
        .split('|')
        .collect();
    assert_eq!(fields[0], std::env::consts::OS);
    assert_eq!(fields[1], "2/50");
    assert!(fields[2].contains(':'));
    assert!(["Day", "Night"].contains(&fields[3]));
    assert_eq!(fields[4], "Disabled");
    assert_eq!(fields[5], "2");
    assert_eq!(fields[6], "0");
    // The GM is in the GM list.
    assert_eq!(fields[7], "1");
    // Both players have no address: they count as one.
    assert_eq!(fields[8], "1");
    assert!(fields[9].ends_with("Minutes"));
}

#[test]
fn snoop_forwards_the_chat_of_the_snooped_player() {
    let mut s = Scene::new();
    for id in [GM, P1, P2] {
        s.enter(id);
    }
    // No target and no name: select a target.
    s.log.take();
    s.cmd(GM, "snoop");
    let out = s.log.take();
    assert_eq!(system_to(&out, GM)[0].id, SystemMessageId::SelectTarget);

    // Whispers reach nobody but the receiver, so what the GM gets is the snoop.
    let whisper = |s: &mut Scene, from: i32, to: &str, text: &str| {
        s.input(
            from,
            ClientInput::Say {
                chat_type: ChatType::Whisper.client_id(),
                text: text.to_owned(),
                target: Some(to.to_owned()),
            },
        );
    };
    s.cmd(GM, "snoop P1");
    s.log.take();
    whisper(&mut s, P2, "P1", "hi there");
    let out = s.log.take();
    // P1 hears it; the GM gets the same line, then the Snoop packet about P1.
    assert_eq!(says_to(&out, P1)[0].3, "hi there");
    assert_eq!(says_to(&out, GM).len(), 1);
    assert_eq!(says_to(&out, GM)[0].3, "hi there");
    let snoops = decoded::<l2_protocol_game::server::Snoop>(&out, GM, "SNOOP");
    assert_eq!(snoops.len(), 1);
    assert_eq!(snoops[0].convo_id, P1);
    assert_eq!(snoops[0].name, "P1");
    assert_eq!(snoops[0].speaker, "P2");
    assert_eq!(snoops[0].message, "hi there");
    assert_eq!(snoops[0].chat_type, ChatType::Whisper);

    // SnoopQuit ends it.
    s.input(GM, ClientInput::SnoopQuit { snoop_id: P1 });
    s.log.take();
    whisper(&mut s, P2, "P1", "again");
    assert!(says_to(&s.log.take(), GM).is_empty());

    // The snooped player leaving ends it too; a GM can snoop by target.
    s.target(GM, P2);
    s.cmd(GM, "snoop");
    s.log.take();
    whisper(&mut s, P1, "P2", "to p2");
    assert_eq!(says_to(&s.log.take(), GM).len(), 1);
    s.input(P2, ClientInput::Logout);
    s.log.take();
    whisper(&mut s, P1, "P2", "nobody");
    assert!(says_to(&s.log.take(), GM).is_empty());
}

#[test]
fn a_gm_who_snoops_itself_does_not_loop_forever() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.cmd(GM, "snoop P100");
    s.log.take();
    s.say(P1, ChatType::General, "hello");
    s.say(GM, ChatType::General, "loop?");
    assert!(!says_to(&s.log.take(), GM).is_empty());
}

// ------------------------------------------------------------------------------ login switches

#[test]
fn the_login_page_needs_the_registered_name() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    // NOTE(java-quirk): `replace("%server_name%", null)` throws before the login server answered.
    s.cmd(GM, "server_login");
    let texts = texts_to(&s.log.take(), GM);
    assert!(
        exception_of(&texts).is_some_and(|t| t.contains("NullPointerException")),
        "{texts:?}"
    );

    s.world.handle(WorldCommand::LoginServerRegistered {
        name: "Kain".to_owned(),
    });
    s.cmd(GM, "server_login");
    let pages = html_to(&s.log.take(), GM);
    // Name, status (Java never sets it at the registration: Auto), type, brackets, max players.
    assert_eq!(pages, ["<html><body>Kain|Auto|Free|false|2000</body></html>"]);
}

#[test]
fn gm_only_and_all_change_the_list_status() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.handle(WorldCommand::LoginServerRegistered {
        name: "Kain".to_owned(),
    });
    s.log.take();
    s.outs.take();

    s.cmd(GM, "server_gm_only");
    let out = s.log.take();
    assert_eq!(s.ls_events(), [LsEvent::SetServerStatus(5)]);
    assert_eq!(texts_to(&out, GM), ["Server is now GM only"]);
    assert!(html_to(&out, GM)[0].contains("Kain|Gm Only|"));

    s.cmd(GM, "server_all");
    let out = s.log.take();
    assert_eq!(s.ls_events(), [LsEvent::SetServerStatus(0)]);
    assert_eq!(texts_to(&out, GM), ["Server is not GM only anymore"]);
    assert!(html_to(&out, GM)[0].contains("Kain|Auto|"));
}

#[test]
fn max_player_is_sent_to_the_login_server() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.handle(WorldCommand::LoginServerRegistered {
        name: "Kain".to_owned(),
    });
    s.log.take();
    s.outs.take();

    s.cmd(GM, "server_max_player 150");
    let out = s.log.take();
    assert_eq!(s.ls_events(), [LsEvent::SetMaxPlayer(150)]);
    assert_eq!(texts_to(&out, GM), ["maxPlayer set to 150"]);
    assert!(html_to(&out, GM)[0].ends_with("|150</body></html>"));

    s.cmd(GM, "server_max_player lots");
    s.cmd(GM, "server_max_player");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Max players must be a number.",
            "Format is server_max_player <max>"
        ]
    );
    assert!(s.ls_events().is_empty());
}

#[test]
fn the_server_type_is_a_set_of_flags() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.handle(WorldCommand::LoginServerRegistered {
        name: "Kain".to_owned(),
    });
    s.log.take();
    s.outs.take();

    s.cmd(GM, "server_list_type relax test");
    let out = s.log.take();
    assert_eq!(s.ls_events(), [LsEvent::ServerStatus { id: 2, value: 6 }]);
    assert_eq!(texts_to(&out, GM), ["Server Type changed to Relax+Test"]);
    assert!(html_to(&out, GM)[0].contains("|Relax+Test|"));

    // The same again, and by number.
    s.cmd(GM, "server_list_type 6");
    assert_eq!(texts_to(&s.log.take(), GM), ["Server Type is already Relax+Test"]);
    assert!(s.ls_events().is_empty());
    s.cmd(GM, "server_list_type 64");
    assert_eq!(texts_to(&s.log.take(), GM), ["Server Type changed to Free"]);
    assert_eq!(s.ls_events(), [LsEvent::ServerStatus { id: 2, value: 64 }]);

    s.cmd(GM, "server_list_type");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Format is server_list_type <normal/relax/test/nolabel/restricted/event/free>"]
    );
}

#[test]
fn the_server_age_quirk_writes_the_type() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.handle(WorldCommand::LoginServerRegistered {
        name: "Kain".to_owned(),
    });
    s.log.take();
    s.outs.take();

    s.cmd(GM, "server_list_age 18");
    let out = s.log.take();
    // The age attribute is the game server's own number 5.
    assert_eq!(s.ls_events(), [LsEvent::ServerStatus { id: 5, value: 18 }]);
    assert_eq!(texts_to(&out, GM), ["Server Age changed to 18"]);
    // NOTE(java-quirk): the age went into the list type: 18 = Relax + Restricted.
    assert!(html_to(&out, GM)[0].contains("|Relax+Restricted|"));

    // The age itself was never stored, so the same age is "changed" again; 0 is the stored one.
    s.cmd(GM, "server_list_age 18");
    assert_eq!(texts_to(&s.log.take(), GM), ["Server Age changed to 18"]);
    s.cmd(GM, "server_list_age 0");
    assert_eq!(texts_to(&s.log.take(), GM), ["Server Age is already 0"]);
    s.cmd(GM, "server_list_age x");
    s.cmd(GM, "server_list_age");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Age must be a number", "Format is server_list_age <number>"]
    );
}

// ---------------------------------------------------------------------------------- shutdown

fn quit_seconds(out: &Sent, to: i32) -> Vec<i32> {
    system_to(out, to)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::TheServerWillBeComingDownInS1SecondSPleaseFindASafePlaceToLogOut)
        .map(|m| match m.params.as_slice() {
            [SystemMessageParam::IntNumber(seconds)] => *seconds,
            other => panic!("{other:?}"),
        })
        .collect()
}

fn controls(outs: &[Out]) -> Vec<ControlRequest> {
    outs.iter()
        .filter_map(|o| match o {
            Out::Control(c) => Some(c.clone()),
            _ => None,
        })
        .collect()
}

fn statuses(outs: &[Out]) -> Vec<i32> {
    outs.iter()
        .filter_map(|o| match o {
            Out::Ls(LsEvent::SetServerStatus(status)) => Some(*status),
            _ => None,
        })
        .collect()
}

#[test]
fn the_countdown_says_what_java_says_and_then_stops_the_server() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    s.outs.take();

    s.cmd(GM, "server_shutdown 10");
    let mut out = s.log.take();
    let mut outs = s.outs.take();
    out.extend(s.run(9_950));
    outs.extend(s.outs.take());
    // Ten is an announced second: once at the start, then 5 to 1. Everybody hears it.
    assert_eq!(quit_seconds(&out, GM), [10, 5, 4, 3, 2, 1]);
    assert_eq!(quit_seconds(&out, P1), [10, 5, 4, 3, 2, 1]);
    // Within the last minute the server goes down for the login.
    assert_eq!(statuses(&outs), [4]);
    // Not over yet.
    assert!(controls(&outs).is_empty());

    s.run(100);
    outs.extend(s.outs.take());
    assert_eq!(controls(&outs), [ControlRequest::Shutdown { restart: false }]);
    // It happens once.
    s.run(3_000);
    assert!(controls(&s.outs.take()).is_empty());
}

#[test]
fn a_restart_exits_with_the_restart_request() {
    let mut s = Scene::new();
    s.enter(GM);
    s.outs.take();
    s.cmd(GM, "server_restart 2");
    s.run(2_100);
    assert_eq!(
        controls(&s.outs.take()),
        [ControlRequest::Shutdown { restart: true }]
    );
}

#[test]
fn a_time_that_is_not_announced_is_said_at_once() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    s.cmd(GM, "server_shutdown 45");
    let out = s.log.take();
    assert_eq!(quit_seconds(&out, P1), [45]);
    // The first step does not say it again, and 45 is within the last minute.
    let out = s.run(100);
    assert!(quit_seconds(&out, P1).is_empty());
    assert_eq!(statuses(&s.outs.take()), [4]);
}

#[test]
fn zero_seconds_stops_at_once() {
    let mut s = Scene::new();
    s.enter(GM);
    s.outs.take();
    s.cmd(GM, "server_shutdown 0");
    assert_eq!(
        controls(&s.outs.take()),
        [ControlRequest::Shutdown { restart: false }]
    );
}

#[test]
fn abort_stops_the_countdown_and_frees_the_login() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    s.outs.take();
    s.cmd(GM, "server_shutdown 30");
    s.run(2_000);
    s.log.take();
    s.outs.take();

    s.cmd(GM, "server_abort");
    let out = s.log.take();
    // NOTE(java-quirk): the mode is already ABORT when the text is made.
    let expected = vec![(
        ChatType::Announcement.client_id(),
        0,
        String::new(),
        "Server aborts aborting and continues normal operation!".to_owned(),
    )];
    assert_eq!(says_to(&out, P1), expected);
    assert_eq!(says_to(&out, GM), expected);

    // The counter notices within a second: the list status Down comes back, and `ABORT` ends
    // with Auto.
    let out = s.run(1_100);
    let outs = s.outs.take();
    assert_eq!(statuses(&outs), [0, 0]);
    assert!(controls(&outs).is_empty());
    assert!(quit_seconds(&out, P1).is_empty());
    // Nothing happens later.
    s.run(40_000);
    assert!(controls(&s.outs.take()).is_empty());
}

#[test]
fn abort_rehabilitates_a_gm_only_server() {
    let mut s = Scene::new();
    s.enter(GM);
    s.world.handle(WorldCommand::LoginServerRegistered {
        name: "Kain".to_owned(),
    });
    s.cmd(GM, "server_gm_only");
    s.cmd(GM, "server_shutdown 30");
    s.outs.take();
    s.cmd(GM, "server_abort");
    s.run(1_100);
    // Down -> Gm Only (the config flag) -> Auto (the end of an ABORT).
    assert_eq!(statuses(&s.outs.take()), [5, 0]);
}

#[test]
fn abort_after_the_end_is_refused() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.cmd(GM, "server_shutdown 1");
    s.run(1_100);
    s.log.take();
    s.cmd(GM, "server_abort");
    assert!(says_to(&s.log.take(), P1).is_empty());
}

#[test]
fn a_new_countdown_replaces_the_running_one() {
    let mut s = Scene::new();
    s.enter(GM);
    s.outs.take();
    s.cmd(GM, "server_shutdown 100");
    s.run(3_000);
    s.cmd(GM, "server_restart 2");
    s.run(2_100);
    // One request, and it is the restart.
    assert_eq!(
        controls(&s.outs.take()),
        [ControlRequest::Shutdown { restart: true }]
    );
}

#[test]
fn the_shutdown_form_and_its_usage() {
    let mut s = Scene::new();
    for id in [GM, P1] {
        s.enter(id);
    }
    s.log.take();
    s.cmd(GM, "server_shutdown");
    let pages = html_to(&s.log.take(), GM);
    assert_eq!(pages.len(), 1);
    assert!(pages[0].contains("count=2"), "{}", pages[0]);
    assert!(pages[0].contains(" AM") || pages[0].contains(" PM"));

    s.cmd(GM, "server_restart soon");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Usage: //server_restart <seconds>"]);
    assert_eq!(html_to(&out, GM).len(), 1);
    s.cmd(GM, "server_shutdown now");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //server_shutdown <seconds>"]
    );

    // A number Java cannot parse is its exception.
    s.cmd(GM, "server_shutdown 99999999999");
    let texts = texts_to(&s.log.take(), GM);
    assert_eq!(
        exception_of(&texts).unwrap(),
        "Exception during execution of  'admin_server_shutdown 99999999999': java.lang.NumberFormatException: For input string: \"99999999999\""
    );
    assert!(s.outs.take().iter().all(|o| !matches!(o, Out::Control(_))));
}

// ------------------------------------------------------------------------------------ reload

#[test]
fn the_reload_panel_pages() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    s.cmd(GM, "reload_menu");
    let pages = html_to(&s.log.take(), GM);
    assert_eq!(pages.len(), 1);
    assert!(pages[0].contains("admin_reload access"));
    assert!(pages[0].contains("Page 1 / 4"));
    // `//reload` alone is the panel too.
    s.cmd(GM, "reload");
    assert_eq!(html_to(&s.log.take(), GM).len(), 1);
    s.cmd(GM, "reload_menu_page 3");
    let pages = html_to(&s.log.take(), GM);
    assert!(pages[0].contains("Page 4 / 4"));
    assert!(pages[0].contains("admin_reload zone"));
    // A page before the first one is Java's exception.
    s.cmd(GM, "reload_menu_page -1");
    let texts = texts_to(&s.log.take(), GM);
    assert!(
        exception_of(&texts)
            .unwrap()
            .contains("ArrayIndexOutOfBoundsException")
    );
}

#[test]
fn reload_answers_what_it_cannot_do() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    s.cmd(GM, "reload frobnicate");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //reload <type> [parameters]"]
    );
    s.cmd(GM, "reload SKILL");
    let texts = texts_to(&s.log.take(), GM);
    assert_eq!(texts.len(), 2);
    assert!(texts[0].contains("'skill' cannot be reloaded"));
    assert!(texts[1].starts_with("WARNING: There are several known issues"));
}

#[test]
fn html_is_read_by_the_server_and_swapped_in() {
    let mut s = Scene::new();
    for id in [GM, GM2] {
        s.enter(id);
    }
    s.outs.take();
    s.log.take();

    s.cmd(GM, "reload html");
    let requests = controls(&s.outs.take());
    assert_eq!(
        requests,
        [ControlRequest::ReloadHtml {
            requester: ObjectId(GM),
            path: None
        }]
    );
    // The page changed on disk; the answer carries the cache read again.
    std::fs::write(
        s.root.path().join("data/html/admin/reload.htm"),
        "<html><body>RELOADED %buttons%</body></html>",
    )
    .unwrap();
    let cache = Arc::new(HtmCache::load(s.root.path(), false));
    let files = cache.loaded_files();
    s.world.handle(WorldCommand::HtmlReloaded {
        requester: ObjectId(GM),
        path: None,
        result: HtmlReloadResult::Reloaded(cache),
    });
    let out = s.log.take();
    let texts = texts_to(&out, GM);
    assert!(texts[0].starts_with("Cache[HTML]: "));
    assert!(texts[0].ends_with(&format!("megabytes on {files} files loaded")));
    // Every GM, the requester too, hears who did it; the warning ends the command.
    assert_eq!(texts[1], "P100: Reloaded Htms.");
    assert!(texts[2].starts_with("WARNING: There are several known issues"));
    assert!(texts_to(&out, GM2).contains(&"P100: Reloaded Htms.".to_owned()));
    s.cmd(GM, "reload_menu");
    assert!(html_to(&s.log.take(), GM)[0].contains("RELOADED"));

    // With a path the file is named, a missing one is refused.
    s.cmd(GM, "reload htm admin/gm_menu.htm");
    assert_eq!(
        controls(&s.outs.take()),
        [ControlRequest::ReloadHtml {
            requester: ObjectId(GM),
            path: Some("admin/gm_menu.htm".to_owned())
        }]
    );
    s.world.handle(WorldCommand::HtmlReloaded {
        requester: ObjectId(GM),
        path: Some("admin/gm_menu.htm".to_owned()),
        result: HtmlReloadResult::Reloaded(Arc::new(HtmCache::load(s.root.path(), false))),
    });
    assert!(texts_to(&s.log.take(), GM2).contains(&"P100: Reloaded Htm File:gm_menu.htm.".to_owned()));
    s.world.handle(WorldCommand::HtmlReloaded {
        requester: ObjectId(GM),
        path: Some("nope.htm".to_owned()),
        result: HtmlReloadResult::MissingFile,
    });
    assert_eq!(
        texts_to(&s.log.take(), GM)[0],
        "File or Directory does not exist."
    );
}
