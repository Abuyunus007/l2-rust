#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The region grid with several players: who sees whom, region switches, activation timers.
//! Java: entity/World.java, entity/WorldRegion.java, entity/WorldObject.java.

use std::sync::{Arc, Mutex};

use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{CharInfo, DeleteObject};
use l2_world::*;

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Other,
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

    fn left_world(&self, _reason: LeaveReason) {
        self.1.0.lock().unwrap().push(Out::Other);
    }
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, _command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Other);
    }

    fn ls_event(&mut self, _event: LsEvent) {
        self.0.0.lock().unwrap().push(Out::Other);
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

/// Packet names one player received, in order.
fn names_to(out: &[Out], id: i32) -> Vec<String> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(to, b) if *to == id => Some(name_of(b)),
            _ => None,
        })
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

fn player(id: i32, x: i32, y: i32) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{id}"),
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
        x,
        y,
        z: Z,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 10.0,
        max_cp: 10.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn world(log: &Log) -> World {
    World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())))
}

fn enter(world: &mut World, log: &Log, id: i32, x: i32, y: i32) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(player(id, x, y)),
        sink: Box::new(Sink(id, log.clone())),
        ip: None,
    });
}

fn leave(world: &mut World, id: i32) {
    world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(id).unwrap()),
        input: ClientInput::Logout,
    });
}

fn char_info_of(out: &[Out], to: i32) -> Vec<i32> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(t, b) if *t == to && name_of(b) == "CHAR_INFO" => Some(
                CharInfo::decode(&mut PacketReader::new(&b[1..]))
                    .unwrap()
                    .object_id,
            ),
            _ => None,
        })
        .collect()
}

fn deletes_of(out: &[Out], to: i32) -> Vec<i32> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(t, b) if *t == to && name_of(b) == "DELETE_OBJECT" => Some(
                DeleteObject::decode(&mut PacketReader::new(&b[1..]))
                    .unwrap()
                    .object_id,
            ),
            _ => None,
        })
        .collect()
}

#[test]
fn players_in_the_same_place_see_each_other_at_spawn() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, X, Y);
    log.take();
    enter(&mut w, &log, 2, X + 100, Y);
    let out = log.take();
    // The newcomer learns about player 1 where Java's spawnMe runs: after QUEST_LIST, before
    // EX_ROTATION.
    let names2 = names_to(&out, 2);
    let quest = names2.iter().position(|n| n == "QUEST_LIST").unwrap();
    assert_eq!(
        names2[quest + 1..quest + 5],
        [
            "CHAR_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "RELATION_CHANGED",
            "EX_ROTATION"
        ]
    );
    assert_eq!(char_info_of(&out, 2), [1]);
    // Player 1 learns about the newcomer (Player.sendInfo).
    assert_eq!(
        names_to(&out, 1),
        ["CHAR_INFO", "EX_BR_EXTRA_USER_INFO", "RELATION_CHANGED"]
    );
    assert_eq!(char_info_of(&out, 1), [2]);
    assert_eq!(w.visible_objects(ObjectId(1)), [ObjectId(2)]);
}

#[test]
fn players_far_apart_do_not_see_each_other() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, X, Y);
    enter(&mut w, &log, 2, X + 20_000, Y);
    let out = log.take();
    assert!(char_info_of(&out, 1).is_empty() && char_info_of(&out, 2).is_empty());
    assert!(w.visible_objects(ObjectId(1)).is_empty());
}

#[test]
fn crossing_into_view_and_out_again_sends_info_and_deletes() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, X, Y);
    enter(&mut w, &log, 2, X + 20_000, Y);
    log.take();
    w.set_position(ObjectId(2), X + 1_000, Y, Z);
    let out = log.take();
    assert_eq!(char_info_of(&out, 1), [2]);
    assert_eq!(char_info_of(&out, 2), [1]);
    w.set_position(ObjectId(2), X + 20_000, Y, Z);
    let out = log.take();
    assert_eq!(deletes_of(&out, 1), [2]);
    assert_eq!(deletes_of(&out, 2), [1]);
    // Back again: CharInfo again, but RelationChanged only once per viewer (getKnownRelations).
    w.set_position(ObjectId(2), X + 1_000, Y, Z);
    assert_eq!(names_to(&log.take(), 1), ["CHAR_INFO", "EX_BR_EXTRA_USER_INFO"]);
}

#[test]
fn moving_inside_the_same_neighbourhood_sends_nothing() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, X, Y);
    enter(&mut w, &log, 2, X + 100, Y);
    log.take();
    // Into the next region, still within the old region's neighbours: nothing to say.
    w.set_position(ObjectId(2), X + 2_100, Y, Z);
    assert!(log.take().iter().all(|o| !matches!(o, Out::Packet(..))));
}

#[test]
fn leaving_deletes_the_player_for_the_others() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, X, Y);
    enter(&mut w, &log, 2, X + 100, Y);
    log.take();
    leave(&mut w, 2);
    let out = log.take();
    assert_eq!(deletes_of(&out, 1), [2]);
    assert!(w.visible_objects(ObjectId(1)).is_empty());
    assert!(w.object(ObjectId(2)).is_none());
}

#[test]
fn a_player_activates_its_region_now_and_the_neighbours_after_a_second() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, X, Y);
    let here = region_of(X, Y, Z).unwrap();
    let next = RegionId {
        x: here.x + 1,
        ..here
    };
    assert!(w.is_region_active(here));
    assert!(!w.is_region_active(next));
    // GridNeighborTurnOnTime = 1 s = 20 ticks.
    for _ in 0..19 {
        w.tick();
    }
    assert!(!w.is_region_active(next));
    w.tick();
    assert!(w.is_region_active(next));
    // Leaving: GridNeighborTurnOffTime = 90 s, then everything around goes dark.
    leave(&mut w, 1);
    for _ in 0..1_799 {
        w.tick();
    }
    assert!(w.is_region_active(here));
    w.tick();
    assert!(!w.is_region_active(here) && !w.is_region_active(next));
}

#[test]
fn grids_always_on_keeps_every_region_active() {
    let log = Log::default();
    let mut w = world(&log);
    w.set_settings(WorldSettings {
        grids_always_on: true,
        ..WorldSettings::default()
    });
    let far = region_of(0, 0, 0).unwrap();
    assert!(w.is_region_active(far));
    enter(&mut w, &log, 1, X, Y);
    leave(&mut w, 1);
    for _ in 0..2_000 {
        w.tick();
    }
    assert!(w.is_region_active(region_of(X, Y, Z).unwrap()));
}

#[test]
fn spawning_clamps_into_the_world_like_java() {
    let log = Log::default();
    let mut w = world(&log);
    enter(&mut w, &log, 1, 10_000_000, Y);
    assert_eq!(w.position_of(ObjectId(1)).unwrap().0, region::WORLD_X_MAX - 5_000);
    assert!(w.object(ObjectId(1)).unwrap().spawned);
}

#[test]
fn general_chat_reaches_only_players_within_1250() {
    let log = Log::default();
    let mut w = world(&log);
    // The test characters are level 1; MinimumChatLevel would refuse them.
    w.set_settings(WorldSettings {
        minimum_chat_level: 0,
        ..WorldSettings::default()
    });
    enter(&mut w, &log, 1, X, Y);
    enter(&mut w, &log, 2, X + 1_000, Y);
    enter(&mut w, &log, 3, X + 1_400, Y);
    log.take();
    w.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Say {
            chat_type: 0,
            text: "hi".into(),
            target: None,
        },
    });
    let out = log.take();
    let said = |to: i32| names_to(&out, to).iter().any(|n| n == "SAY2");
    assert!(said(1) && said(2), "speaker and the player 1000 away");
    assert!(!said(3), "1400 away is out of the 1250 range");
}
