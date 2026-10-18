#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! Drives the world synchronously with recording sinks and outbox.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use l2_world::*;

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Left(i32, LeaveReason),
    Persist(PersistCommand),
    Ls(LsEvent),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push(Out::Packet(self.0, bytes));
    }
    fn left_world(&self, reason: LeaveReason) {
        self.1.0.lock().unwrap().push(Out::Left(self.0, reason));
    }
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, event: LsEvent) {
        self.0.0.lock().unwrap().push(Out::Ls(event));
    }
}

fn data(id: i32, account: &str, name: &str) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: account.into(),
        name: name.into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 1,
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
    }
}

fn world(log: &Log) -> World {
    World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())))
}

fn enter(world: &mut World, log: &Log, session: u64, id: i32, account: &str, name: &str) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(session),
        player: Box::new(data(id, account, name)),
        sink: Box::new(Sink(id, log.clone())),
        ip: None,
    });
}

#[test]
fn enter_tells_the_login_server() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, 0x6000_0001, "acc", "Hero");
    assert_eq!(w.online_count(), 1);
    assert_eq!(w.player_of_session(SessionId(1)), Some(ObjectId(0x6000_0001)));
    assert_eq!(
        log.take(),
        vec![Out::Ls(LsEvent::PlayerInGame {
            account: "acc".into()
        })]
    );
}

#[test]
fn logout_stores_then_reports_left_then_tells_the_session() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, 7, "acc", "Hero");
    log.take();
    w.player_mut(ObjectId(7)).unwrap().core.x = 1234;
    for _ in 0..40 {
        w.tick(); // 2 seconds
    }
    w.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Logout,
    });
    let out = log.take();
    let Out::Persist(PersistCommand::StorePlayer(stored)) = &out[0] else {
        panic!("{out:?}")
    };
    assert_eq!(stored.x, 1234, "live position is stored");
    assert_eq!(stored.online_time, 12, "10 s before + 2 s online");
    assert_eq!(
        out[1],
        Out::Persist(PersistCommand::PlayerLeft {
            account: "acc".into(),
            object_id: ObjectId(7)
        })
    );
    assert_eq!(out[2], Out::Left(7, LeaveReason::Logout));
    assert_eq!(w.online_count(), 0);
    assert_eq!(w.player_of_session(SessionId(1)), None);
}

#[test]
fn restart_and_disconnect_and_kick() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, 1, "a", "A");
    enter(&mut w, &log, 2, 2, "b", "B");
    enter(&mut w, &log, 3, 3, "c", "C");
    log.take();
    w.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Restart,
    });
    w.handle(WorldCommand::Disconnected {
        session: SessionId(2),
    });
    w.handle(WorldCommand::Kick { account: "c".into() });
    let lefts: Vec<Out> = log
        .take()
        .into_iter()
        .filter(|o| matches!(o, Out::Left(..)))
        .collect();
    assert_eq!(
        lefts,
        vec![
            Out::Left(1, LeaveReason::Restart),
            Out::Left(2, LeaveReason::Disconnected),
            Out::Left(3, LeaveReason::Kicked)
        ]
    );
    assert_eq!(w.online_count(), 0);
}

#[test]
fn second_instance_of_a_character_replaces_the_first() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, 9, "acc", "Hero");
    enter(&mut w, &log, 2, 9, "acc", "Hero");
    assert!(log.take().contains(&Out::Left(9, LeaveReason::Kicked)));
    assert_eq!(w.player_of_session(SessionId(2)), Some(ObjectId(9)));
    assert_eq!(w.player_of_session(SessionId(1)), None);
}

#[test]
fn input_without_character_is_ignored() {
    let log = Log::default();
    let mut w = world(&log);
    w.handle(WorldCommand::Input {
        session: SessionId(5),
        input: ClientInput::Logout,
    });
    assert!(log.take().is_empty());
}

#[test]
fn autosave_timer_stores_without_removing() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, 4, "acc", "Hero");
    log.take();
    let due = w.tick_count() + TimerWheel::ms_to_ticks(100);
    w.timers().schedule_at(due, TimerKind::Autosave(ObjectId(4)));
    w.tick();
    assert!(log.take().is_empty());
    w.tick();
    let out = log.take();
    assert!(
        matches!(&out[..], [Out::Persist(PersistCommand::StorePlayer(_))]),
        "{out:?}"
    );
    assert_eq!(w.online_count(), 1);
}

#[test]
fn entering_arms_an_autosave_that_repeats_until_the_player_leaves() {
    let log = Log::default();
    let mut w = world(&log);
    w.set_autosave_interval(Duration::from_millis(200)); // 4 ticks
    enter(&mut w, &log, 1, 4, "acc", "Hero");
    log.take();
    for _ in 0..3 {
        w.tick();
    }
    assert!(log.take().is_empty());
    w.tick();
    let out = log.take();
    assert!(
        matches!(&out[..], [Out::Persist(PersistCommand::StorePlayer(p))] if p.name == "Hero"),
        "{out:?}"
    );
    // Due again one interval later, and only once.
    for _ in 0..3 {
        w.tick();
    }
    assert!(log.take().is_empty());
    w.tick();
    assert_eq!(log.take().len(), 1);
    // Leaving cancels it: only the final store and PlayerLeft follow, then silence.
    w.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    let out = log.take();
    assert!(matches!(
        &out[..],
        [
            Out::Persist(PersistCommand::StorePlayer(_)),
            Out::Persist(PersistCommand::PlayerLeft { .. }),
            Out::Left(..)
        ]
    ));
    for _ in 0..20 {
        w.tick();
    }
    assert!(log.take().is_empty());
}

#[test]
fn autosave_snapshot_counts_the_session_in_the_online_time() {
    let log = Log::default();
    let mut w = world(&log);
    w.set_autosave_interval(Duration::from_secs(60)); // 1200 ticks
    enter(&mut w, &log, 1, 4, "acc", "Hero");
    log.take();
    for _ in 0..1200 {
        w.tick();
    }
    let out = log.take();
    let [Out::Persist(PersistCommand::StorePlayer(p))] = &out[..] else {
        panic!("{out:?}");
    };
    // 10 s were stored when the player entered, plus the 60 s of this session.
    assert_eq!(p.online_time, 70);
}

#[test]
fn a_character_that_enters_twice_keeps_a_single_autosave() {
    let log = Log::default();
    let mut w = world(&log);
    w.set_autosave_interval(Duration::from_millis(100)); // 2 ticks
    enter(&mut w, &log, 1, 4, "acc", "Hero");
    enter(&mut w, &log, 2, 4, "acc", "Hero"); // replaces the first instance
    log.take();
    w.tick();
    w.tick();
    let stores = log
        .take()
        .into_iter()
        .filter(|o| matches!(o, Out::Persist(PersistCommand::StorePlayer(_))))
        .count();
    assert_eq!(stores, 1);
}

#[test]
fn admin_lists_online_players_sorted() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, 1, "a", "Zed");
    enter(&mut w, &log, 2, 2, "b", "Amy");
    let got = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&got);
    w.handle(WorldCommand::Admin(AdminQuery::OnlinePlayers(Box::new(
        move |list| {
            *sink.lock().unwrap() = list;
        },
    ))));
    let names: Vec<String> = got.lock().unwrap().iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, ["Amy", "Zed"]);
}

#[test]
fn admin_reports_the_client_address_and_entry_time() {
    let log = Log::default();
    let mut w = world(&log);
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    w.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(data(1, "a", "Known")),
        sink: Box::new(Sink(1, log.clone())),
        ip: Some(std::net::IpAddr::from([127, 0, 0, 1])),
    });
    enter(&mut w, &log, 2, 2, "b", "Unknown");
    let got = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&got);
    w.handle(WorldCommand::Admin(AdminQuery::OnlinePlayers(Box::new(
        move |list| {
            *sink.lock().unwrap() = list;
        },
    ))));
    let list = got.lock().unwrap().clone();
    let known = list.iter().find(|p| p.name == "Known").unwrap();
    let unknown = list.iter().find(|p| p.name == "Unknown").unwrap();
    assert_eq!(known.ip, Some(std::net::IpAddr::from([127, 0, 0, 1])));
    assert_eq!(unknown.ip, None);
    assert!(known.online_since_unix_ms >= before);
}

#[test]
fn shutdown_stores_everyone_and_stops() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, 1, "a", "A");
    enter(&mut w, &log, 2, 2, "b", "B");
    log.take();
    let done = Arc::new(Mutex::new(false));
    let flag = Arc::clone(&done);
    w.handle(WorldCommand::Shutdown {
        done: Box::new(move || *flag.lock().unwrap() = true),
    });
    let out = log.take();
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, Out::Persist(PersistCommand::StorePlayer(_))))
            .count(),
        2
    );
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, Out::Left(_, LeaveReason::ServerShutdown)))
            .count(),
        2
    );
    assert!(*done.lock().unwrap());
    assert!(w.is_stopped());
    enter(&mut w, &log, 3, 3, "c", "C");
    assert_eq!(w.online_count(), 0, "a stopped world accepts nothing");
}

#[test]
fn same_seed_same_world() {
    let log = Log::default();
    let mut a = World::new(WorldRng::from_seed(42), Box::new(RecOutbox(log.clone())));
    let mut b = World::new(WorldRng::from_seed(42), Box::new(RecOutbox(log.clone())));
    let xs: Vec<i32> = (0..10).map(|_| a.rng().next_i32()).collect();
    let ys: Vec<i32> = (0..10).map(|_| b.rng().next_i32()).collect();
    assert_eq!(xs, ys);
}

#[test]
fn runner_ticks_handles_commands_and_stops_on_shutdown() {
    let log = Log::default();
    let (tx, rx) = mpsc::channel();
    let handle = runner::spawn(world(&log), rx).unwrap();
    tx.send(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(data(1, "a", "A")),
        sink: Box::new(Sink(1, log.clone())),
        ip: None,
    })
    .unwrap();
    // Generous wait: under parallel test load (many rustc/test processes) 300 ms gave < 3 ticks.
    std::thread::sleep(Duration::from_millis(1000));
    let (done_tx, done_rx) = mpsc::channel();
    tx.send(WorldCommand::Shutdown {
        done: Box::new(move || done_tx.send(()).unwrap()),
    })
    .unwrap();
    done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let world = handle.join().unwrap();
    assert!(
        world.tick_count() >= 3,
        "ticked while waiting (got {})",
        world.tick_count()
    );
    assert!(log.take().contains(&Out::Left(1, LeaveReason::ServerShutdown)));
}

#[test]
fn runner_stores_players_when_all_senders_are_gone() {
    let log = Log::default();
    let (tx, rx) = mpsc::channel();
    let handle = runner::spawn(world(&log), rx).unwrap();
    tx.send(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(data(1, "a", "A")),
        sink: Box::new(Sink(1, log.clone())),
        ip: None,
    })
    .unwrap();
    drop(tx);
    let world = handle.join().unwrap();
    assert!(world.is_stopped());
    assert!(
        log.take()
            .iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::StorePlayer(_))))
    );
}
