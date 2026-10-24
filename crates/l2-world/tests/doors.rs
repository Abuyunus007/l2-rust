#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Doors, fences and static objects in the world: spawning from data, the packets players get,
//! open / close, and the geoengine's door and fence checks over the world and over the snapshot.
//! Java: Door.java, Fence.java, StaticObject.java, DoorData.java, FenceData.java, WorldRegion.java.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::{DoorTemplates, FenceState, FenceTemplates, StaticObjectTemplates};
use l2_geo::{DynamicObstacles, GeoEngine, GeoEngineOptions, GeoSet, Location};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{
    DeleteObject, DoorStatusUpdate, ExColosseumFenceInfo, OnEventTrigger, StaticObjectInfo,
};
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

/// Access level 100 is a GM; everything else is a plain player.
struct GmData;

impl StaticData for GmData {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }

    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }

    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }

    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level == 100 {
            AccessLevelStatic {
                is_gm: true,
                ..AccessLevelStatic::USER
            }
        } else {
            AccessLevelStatic::USER
        }
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

/// The bodies of the packets with this name that `id` received, in order.
fn bodies_to(out: &[Out], id: i32, name: &str) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(to, b) if *to == id && name_of(b) == name => Some(b.clone()),
            _ => None,
        })
        .collect()
}

/// Names of the door-related packets `id` received, in order.
fn door_names_to(out: &[Out], id: i32) -> Vec<String> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(to, b) if *to == id => Some(name_of(b)),
            _ => None,
        })
        .filter(|n| {
            matches!(
                n.as_str(),
                "STATIC_OBJECT"
                    | "DOOR_STATUS_UPDATE"
                    | "EVENT_TRIGGER"
                    | "EX_COLOSSEUM_FENCE_INFO"
                    | "DELETE_OBJECT"
            )
        })
        .collect()
}

fn static_infos(out: &[Out], to: i32) -> Vec<StaticObjectInfo> {
    bodies_to(out, to, "STATIC_OBJECT")
        .iter()
        .map(|b| StaticObjectInfo::decode(&mut PacketReader::new(&b[1..])).unwrap())
        .collect()
}

fn statuses(out: &[Out], to: i32) -> Vec<DoorStatusUpdate> {
    bodies_to(out, to, "DOOR_STATUS_UPDATE")
        .iter()
        .map(|b| DoorStatusUpdate::decode(&mut PacketReader::new(&b[1..])).unwrap())
        .collect()
}

fn triggers(out: &[Out], to: i32) -> Vec<OnEventTrigger> {
    bodies_to(out, to, "EVENT_TRIGGER")
        .iter()
        .map(|b| OnEventTrigger::decode(&mut PacketReader::new(&b[1..])).unwrap())
        .collect()
}

fn fence_infos(out: &[Out], to: i32) -> Vec<ExColosseumFenceInfo> {
    bodies_to(out, to, "EX_COLOSSEUM_FENCE_INFO")
        .iter()
        .map(|b| ExColosseumFenceInfo::decode(&mut PacketReader::new(&b[3..])).unwrap())
        .collect()
}

fn deletes(out: &[Out], to: i32) -> Vec<i32> {
    bodies_to(out, to, "DELETE_OBJECT")
        .iter()
        .map(|b| {
            DeleteObject::decode(&mut PacketReader::new(&b[1..]))
                .unwrap()
                .object_id
        })
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

fn player(id: i32, x: i32, y: i32, access_level: i32) -> PlayerData {
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
        access_level,
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
    let mut w = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
    w.set_static_data(Box::new(GmData));
    w
}

fn enter_as(world: &mut World, log: &Log, id: i32, x: i32, y: i32, access_level: i32) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(player(id, x, y, access_level)),
        sink: Box::new(Sink(id, log.clone())),
        ip: None,
    });
}

fn enter(world: &mut World, log: &Log, id: i32, x: i32, y: i32) {
    enter_as(world, log, id, x, y, 0);
}

fn ids() -> IdFactory {
    IdFactory::new([]).unwrap()
}

/// A `<door>` element: a wall 20 wide (x) and 100 long (y) around (X, Y), 150 high from
/// `Z - 10`. `extra` adds attributes.
fn door_xml(id: i32, extra: &str) -> String {
    format!(
        r#"<door id="{id}" name="door{id}" height="150" baseHpMax="1000" pos="{X},{Y},{Z}"
            node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}" {extra}/>"#,
        X - 10,
        Y - 50,
        X + 10,
        Y - 50,
        X + 10,
        Y + 50,
        X - 10,
        Y + 50,
        Z - 10
    )
}

fn doors(xml: &str) -> DoorTemplates {
    DoorTemplates::from_xml(&format!("<list>{xml}</list>")).unwrap()
}

fn loc(x: i32, y: i32, z: i32) -> Location {
    Location::new(x, y, z)
}

fn flat_engine() -> GeoEngine {
    // No geodata at all: every cell is open, so only doors and fences decide.
    GeoEngine::new(GeoSet::new(), GeoEngineOptions::default())
}

#[test]
fn doors_spawn_with_java_defaults() {
    let log = Log::default();
    let mut w = world(&log);
    let templates = doors(&format!(
        "{}{}{}",
        door_xml(1, r#"default_status="open" group="g""#),
        door_xml(2, r#"group="g" targetable="false" hp_showable="false""#),
        door_xml(3, "")
    ));
    let ids = ids();
    assert_eq!(w.spawn_doors(&templates, &ids).unwrap(), 3);
    assert_eq!(w.doors().count(), 3);
    assert_eq!(ids.used_count(), 3, "one object id per door");
    let d1 = w.door_by_door_id(1).unwrap();
    assert!(d1.open, "default_status open");
    assert!(!w.door_by_door_id(3).unwrap().open);
    assert_eq!((d1.max_hp(), d1.current_hp()), (1000, 1000.0));
    assert_eq!(d1.damage(), 0);
    assert_eq!(d1.mesh_index, 1);
    assert!(
        ids.contains(d1.object_id) && d1.object_id.0 >= RUST_FIRST_OBJECT_ID,
        "ids come from the Rust range"
    );
    assert_eq!(
        w.door_group("g").unwrap().iter().copied().collect::<Vec<_>>(),
        [1, 2]
    );
    assert!(w.door_group("nope").is_none());
    // Spawned: in the object table, in a region, at the template position.
    let entry = w.object(d1.object_id).unwrap();
    assert!(entry.spawned && entry.region.is_some());
    assert_eq!(
        w.position_of(d1.object_id),
        Some((X, Y, Z - 10)),
        "pos z is min(pos z, nodeZ)"
    );
}

#[test]
fn a_player_coming_into_view_gets_the_door_packets() {
    let log = Log::default();
    let mut w = world(&log);
    let templates = doors(&format!(
        "{}{}",
        door_xml(10, r#"targetable="false" hp_showable="false""#),
        door_xml(11, r#"emitter_id="77""#)
    ));
    w.spawn_doors(&templates, &ids()).unwrap();
    let (a, b) = (
        w.door_by_door_id(10).unwrap().object_id,
        w.door_by_door_id(11).unwrap().object_id,
    );
    enter(&mut w, &log, 1, X + 300, Y);
    let out = log.take();
    // Door.sendInfo per door: StaticObjectInfo, DoorStatusUpdate, and OnEventTrigger if it has
    // an emitter. Region order then id order: door 10 was spawned first, so its id is lower.
    assert_eq!(
        door_names_to(&out, 1),
        [
            "STATIC_OBJECT",
            "DOOR_STATUS_UPDATE",
            "STATIC_OBJECT",
            "DOOR_STATUS_UPDATE",
            "EVENT_TRIGGER"
        ]
    );
    let infos = static_infos(&out, 1);
    assert_eq!(
        infos[0],
        StaticObjectInfo {
            static_object_id: 10,
            object_id: a.0,
            kind: StaticObjectInfo::KIND_DOOR,
            targetable: false,
            mesh_index: 1,
            closed: true,
            is_enemy: false,
            current_hp: 1000,
            max_hp: 1000,
            show_hp: false,
            damage_grade: 0,
        }
    );
    assert!(
        infos[1].targetable && infos[1].show_hp,
        "defaults: targetable, hp shown"
    );
    assert_eq!(infos[1].object_id, b.0);
    let st = statuses(&out, 1);
    assert_eq!(
        st[0],
        DoorStatusUpdate {
            object_id: a.0,
            closed: true,
            damage: 0,
            is_enemy: false,
            door_id: 10,
            current_hp: 1000,
            max_hp: 1000
        }
    );
    // enabled = !open
    assert_eq!(
        triggers(&out, 1),
        [OnEventTrigger {
            emitter_id: 77,
            enabled: true
        }]
    );
}

#[test]
fn a_gm_can_target_doors_the_template_hides() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(&doors(&door_xml(10, r#"targetable="false""#)), &ids())
        .unwrap();
    enter_as(&mut w, &log, 1, X + 300, Y, 100);
    enter(&mut w, &log, 2, X + 300, Y);
    let out = log.take();
    assert!(
        static_infos(&out, 1)[0].targetable,
        "StaticObjectInfo(door, player.isGM())"
    );
    assert!(!static_infos(&out, 2)[0].targetable);
}

#[test]
fn doors_are_seen_from_the_surrounding_regions_only() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(&doors(&door_xml(10, "")), &ids()).unwrap();
    // Two regions (4096+ units) away: not visible.
    enter(&mut w, &log, 1, X + 2_048 * 2 + 600, Y);
    assert!(door_names_to(&log.take(), 1).is_empty());
    // Crossing into the neighbouring region brings the door into view (switchRegion).
    w.set_position(ObjectId(1), X + 2_048, Y, Z);
    assert_eq!(
        door_names_to(&log.take(), 1),
        ["STATIC_OBJECT", "DOOR_STATUS_UPDATE"]
    );
    // And out again: DeleteObject for the door.
    let id = w.door_by_door_id(10).unwrap().object_id;
    w.set_position(ObjectId(1), X + 2_048 * 2 + 600, Y, Z);
    assert_eq!(deletes(&log.take(), 1), [id.0]);
}

#[test]
fn opening_and_closing_tells_the_players_who_see_the_door() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(&doors(&door_xml(10, "")), &ids()).unwrap();
    let id = w.door_by_door_id(10).unwrap().object_id;
    enter(&mut w, &log, 1, X + 300, Y);
    enter(&mut w, &log, 2, X + 30_000, Y);
    log.take();

    w.open_door(id);
    let out = log.take();
    assert!(w.door(id).unwrap().open);
    assert_eq!(door_names_to(&out, 1), ["STATIC_OBJECT", "DOOR_STATUS_UPDATE"]);
    assert!(!static_infos(&out, 1)[0].closed);
    assert!(!statuses(&out, 1)[0].closed);
    assert!(door_names_to(&out, 2).is_empty(), "too far away");

    // Opening an open door does nothing (Java: `if (!isOpen())`).
    w.open_door(id);
    assert!(log.take().is_empty());

    w.close_door(id);
    let out = log.take();
    assert!(!w.door(id).unwrap().open);
    assert!(statuses(&out, 1)[0].closed && static_infos(&out, 1)[0].closed);
    w.close_door(id);
    assert!(log.take().is_empty());
}

#[test]
fn a_group_opens_and_closes_together() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(
        &doors(&format!(
            "{}{}{}",
            door_xml(1, r#"group="g""#),
            door_xml(2, r#"group="g" default_status="open""#),
            door_xml(3, "")
        )),
        &ids(),
    )
    .unwrap();
    enter(&mut w, &log, 1, X + 300, Y);
    log.take();
    let first = w.door_by_door_id(1).unwrap().object_id;
    w.open_door(first);
    let out = log.take();
    assert!(w.door_by_door_id(1).unwrap().open && w.door_by_door_id(2).unwrap().open);
    assert!(!w.door_by_door_id(3).unwrap().open);
    // Door 2 was open already: only door 1 changes (manageGroupOpen: `if (door.isOpen() != open)`).
    assert_eq!(statuses(&out, 1).len(), 1);
    w.close_door(w.door_by_door_id(2).unwrap().object_id);
    assert!(!w.door_by_door_id(1).unwrap().open && !w.door_by_door_id(2).unwrap().open);
    assert_eq!(statuses(&log.take(), 1).len(), 2);
}

#[test]
fn an_open_event_reaches_the_child_door() {
    let log = Log::default();
    let mut w = world(&log);
    let ids = ids();
    w.spawn_doors(
        &doors(&format!(
            "{}{}",
            door_xml(1, r#"child_id_event="2""#),
            door_xml(
                2,
                r#"master_open_event="act_open" master_close_event="act_close""#
            )
        )),
        &ids,
    )
    .unwrap();
    let (a, b) = (
        w.door_by_door_id(1).unwrap().object_id,
        w.door_by_door_id(2).unwrap().object_id,
    );
    w.open_door(a);
    assert!(w.door(a).unwrap().open && w.door(b).unwrap().open);
    w.close_door(a);
    assert!(!w.door(a).unwrap().open && !w.door(b).unwrap().open);
    // A child that does not exist is only a warning.
    w.spawn_doors(&doors(&door_xml(5, r#"child_id_event="999""#)), &ids)
        .unwrap();
    let orphan = w.door_by_door_id(5).unwrap().object_id;
    w.open_door(orphan);
    assert!(w.door(orphan).unwrap().open);
}

#[test]
fn a_hurt_door_reports_its_damage_grade() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(&doors(&door_xml(1, "")), &ids()).unwrap();
    let id = w.door_by_door_id(1).unwrap().object_id;
    let mut door = w.door(id).unwrap().clone();
    // 6 - ceil(hp / max * 6): 1000/1000 -> 0, 999/1000 -> 0 ... see Door.getDamage.
    for (hp, grade) in [
        (1000.0, 0),
        (999.0, 0),
        (833.0, 1),
        (500.0, 3),
        (1.0, 5),
        (0.0, 6),
    ] {
        door.core.cur_hp = hp;
        assert_eq!(door.damage(), grade, "hp {hp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Geodata checks

/// The wall of `door_xml` runs along y; a segment along x through (X, Y) crosses both faces.
fn through() -> (Location, Location) {
    (loc(X - 100, Y, Z), loc(X + 100, Y, Z))
}

#[test]
fn a_closed_door_blocks_movement_and_an_open_one_does_not() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(&doors(&door_xml(1, "")), &ids()).unwrap();
    let id = w.door_by_door_id(1).unwrap().object_id;
    let engine = flat_engine();
    let (from, to) = through();

    assert!(!engine.can_move_to_target_with(&w.obstacles(), from, to, 0));
    // A segment beside the wall is free.
    assert!(engine.can_move_to_target_with(
        &w.obstacles(),
        loc(X - 100, Y + 200, Z),
        loc(X + 100, Y + 200, Z),
        0
    ));
    // The geoengine's own checks, with the snapshot installed as its hook.
    let snapshot = w.door_snapshot();
    assert_eq!(snapshot.door_count(), 1);
    let guarded = flat_engine().with_obstacles(snapshot);
    assert!(!guarded.can_move_to_target(from, to, 0));
    assert!(!guarded.can_see_target_in_instance(from, to, 0));
    assert_eq!(guarded.valid_location(from, to, 0), from, "stays where it was");

    w.open_door(id);
    assert!(engine.can_move_to_target_with(&w.obstacles(), from, to, 0));
    // The old snapshot still shows the door closed: it is a copy.
    assert!(!guarded.can_move_to_target(from, to, 0));
    let fresh = flat_engine().with_obstacles(w.door_snapshot());
    assert!(fresh.can_move_to_target(from, to, 0));
    assert!(fresh.can_see_target_in_instance(from, to, 0));
    assert_eq!(w.door_snapshot().door_count(), 0);

    w.close_door(id);
    assert!(!engine.can_move_to_target_with(&w.obstacles(), from, to, 0));
    assert_eq!(w.door_snapshot().door_count(), 1);
}

#[test]
fn line_of_sight_needs_two_faces_but_movement_one() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(&doors(&door_xml(1, "")), &ids()).unwrap();
    let obstacles = w.obstacles();
    // Ends inside the wall: one face crossed.
    let (from, inside) = (loc(X - 100, Y, Z), loc(X, Y, Z));
    assert!(obstacles.doors_between(from, inside, 0, false));
    assert!(!obstacles.doors_between(from, inside, 0, true), "doubleFaceCheck");
    // Through the wall: both faces.
    let (from, to) = through();
    assert!(obstacles.doors_between(from, to, 0, true));
    assert!(obstacles.doors_between(from, to, 0, false));
}

#[test]
fn only_height_collision_instance_and_death_decide() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(
        &doors(&format!(
            "{}{}",
            door_xml(1, ""),
            door_xml(2, r#"check_collision="false""#)
        )),
        &ids(),
    )
    .unwrap();
    let (from, to) = through();
    let a = w.door_by_door_id(1).unwrap().object_id;
    {
        let o = w.obstacles();
        assert!(o.doors_between(from, to, 0, false));
        // nodeZ = Z - 10, height 150: the door spans (Z - 10, Z + 140) exclusive.
        assert!(!o.doors_between(loc(from.x, from.y, Z + 140), loc(to.x, to.y, Z + 140), 0, false));
        assert!(!o.doors_between(loc(from.x, from.y, Z - 10), loc(to.x, to.y, Z - 10), 0, false));
        assert!(o.doors_between(loc(from.x, from.y, Z + 139), loc(to.x, to.y, Z + 139), 0, false));
        // Another instance: Java asks the instance's doors (none exist yet) or skips by id.
        assert!(!o.doors_between(from, to, 1, false));
        assert!(!o.doors_between(from, to, -1, false));
    }
    // A dead door does not block; door 2 never did (check_collision false), so only door 1 counts.
    w.set_door_dead(a, true);
    assert!(!w.obstacles().doors_between(from, to, 0, false));
    assert!(!w.door_snapshot().doors_between(from, to, 0, false));
    w.set_door_dead(a, false);
    assert!(w.obstacles().doors_between(from, to, 0, false));
    assert!(w.door_snapshot().doors_between(from, to, 0, false));
}

#[test]
fn the_check_reads_the_door_list_of_the_start_region_like_java() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(&doors(&door_xml(1, "")), &ids()).unwrap();
    let target = loc(X - 100, Y, Z);
    // The door is in the lists of its own region and the 26 around it.
    for start_x in [X + 100, X + 2_500] {
        assert!(
            w.obstacles().doors_between(loc(start_x, Y, Z), target, 0, false),
            "start x {start_x}"
        );
    }
    // Two regions away the list is empty, although the segment would cross the wall.
    let far = loc(X + 2_048 * 2 + 600, Y, Z);
    assert!(!w.obstacles().doors_between(far, target, 0, false));
    assert!(!w.door_snapshot().doors_between(far, target, 0, false));
    // Outside the world there is no region and so no door.
    assert!(
        !w.obstacles()
            .doors_between(loc(2_000_000, 0, 0), target, 0, false)
    );
}

#[test]
fn the_snapshot_answers_what_the_world_answers() {
    let log = Log::default();
    let mut w = world(&log);
    // A few walls around two region borders, some open, one dead, plus two fences.
    let mut xml = String::new();
    for (i, (dx, dy)) in [(0, 0), (900, 300), (-1_500, 700), (2_100, -400), (400, 1_800)]
        .into_iter()
        .enumerate()
    {
        let id = 100 + i32::try_from(i).unwrap();
        let (x, y) = (X + dx, Y + dy);
        xml.push_str(&format!(
            r#"<door id="{id}" name="d{id}" height="150" pos="{x},{y},{Z}" node1="{},{}" node2="{},{}"
                node3="{},{}" node4="{},{}" nodeZ="{}" default_status="{}"/>"#,
            x - 10,
            y - 80,
            x + 10,
            y - 80,
            x + 10,
            y + 80,
            x - 10,
            y + 80,
            Z - 10,
            if i == 2 { "open" } else { "close" }
        ));
    }
    let ids = ids();
    w.spawn_doors(&doors(&xml), &ids).unwrap();
    let dead = w.door_by_door_id(103).unwrap().object_id;
    w.set_door_dead(dead, true);
    let fences = FenceTemplates::from_xml(&format!(
        r#"<list><fence name="f" x="{}" y="{}" z="{Z}" width="300" length="200" height="1"/>
        <fence name="g" x="{}" y="{}" z="{Z}" width="120" length="500" height="1" state="OPENED"/></list>"#,
        X - 600,
        Y + 100,
        X + 700,
        Y
    ))
    .unwrap();
    w.spawn_fences(&fences, &ids).unwrap();
    let snapshot = w.door_snapshot();
    assert_eq!(snapshot.door_count(), 3, "5 doors - 1 open - 1 dead");
    assert_eq!(snapshot.fence_count(), 1, "the opened fence does not block");

    let mut seed = 12_345_u64;
    let mut next = move |range: i32| {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        i32::try_from((seed >> 33) % u64::try_from(range).unwrap()).unwrap()
    };
    let mut blocked = 0;
    for _ in 0..20_000 {
        let from = loc(
            X - 4_000 + next(8_000),
            Y - 2_500 + next(5_000),
            Z - 60 + next(180),
        );
        let to = loc(
            from.x - 1_500 + next(3_000),
            from.y - 1_500 + next(3_000),
            Z - 60 + next(180),
        );
        for double in [false, true] {
            let a = w.obstacles().doors_between(from, to, 0, double);
            assert_eq!(
                a,
                snapshot.doors_between(from, to, 0, double),
                "{from:?} -> {to:?}"
            );
            blocked += usize::from(a);
        }
        assert_eq!(
            w.obstacles().fence_between(from, to, 0),
            snapshot.fence_between(from, to, 0),
            "{from:?} -> {to:?}"
        );
    }
    assert!(blocked > 100, "the sample must hit doors ({blocked})");
}

#[test]
fn a_snapshot_is_shared_until_something_changes() {
    let log = Log::default();
    let mut w = world(&log);
    w.spawn_doors(&doors(&door_xml(1, "")), &ids()).unwrap();
    let id = w.door_by_door_id(1).unwrap().object_id;
    let first = w.door_snapshot();
    assert!(Arc::ptr_eq(&first, &w.door_snapshot()));
    w.open_door(id);
    let second = w.door_snapshot();
    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!((first.door_count(), second.door_count()), (1, 0));
    // The snapshot can be moved to another thread (path workers).
    let handle = std::thread::spawn(move || second.door_count());
    assert_eq!(handle.join().unwrap(), 0);
}

// ---------------------------------------------------------------------------------------------
// Fences

fn fence_templates(extra: &str) -> FenceTemplates {
    FenceTemplates::from_xml(&format!(
        r#"<list><fence name="demo" x="{X}" y="{Y}" z="{Z}" width="200" length="200" height="3" {extra}/></list>"#
    ))
    .unwrap()
}

#[test]
fn a_fence_gets_its_ids_packets_and_blocks_while_closed() {
    let log = Log::default();
    let mut w = world(&log);
    let ids = ids();
    assert_eq!(w.spawn_fences(&fence_templates(""), &ids).unwrap(), 1);
    assert_eq!(ids.used_count(), 3, "the fence and height - 1 pieces");
    let fence = w.fences().next().unwrap().clone();
    assert_eq!(fence.height_fences.len(), 2);
    assert_eq!(
        (fence.x_min, fence.x_max, fence.y_min, fence.y_max),
        (X - 100, X + 100, Y - 100, Y + 100)
    );

    enter(&mut w, &log, 1, X + 300, Y);
    let out = log.take();
    let infos = fence_infos(&out, 1);
    assert_eq!(infos.len(), 3);
    assert_eq!(infos[0].object_id, fence.object_id.0);
    assert_eq!(infos[1].object_id, fence.height_fences[0].0);
    assert_eq!(infos[2].object_id, fence.height_fences[1].0);
    assert!(
        infos
            .iter()
            .all(|i| i.state == 2 && (i.x, i.y, i.z) == (X, Y, Z) && (i.width, i.length) == (200, 200))
    );

    // Closed: a segment into the fence area is blocked, one beside it is not, and neither is one
    // above it (z must be within 100 of the fence).
    let into = (loc(X - 300, Y, Z), loc(X, Y, Z));
    let beside = (loc(X - 300, Y + 400, Z), loc(X - 250, Y + 400, Z));
    let above = (loc(X - 300, Y, Z + 150), loc(X, Y, Z + 150));
    let o = w.obstacles();
    assert!(o.fence_between(into.0, into.1, 0));
    assert!(!o.fence_between(beside.0, beside.1, 0));
    assert!(!o.fence_between(above.0, above.1, 0));
    assert!(!o.fence_between(into.0, into.1, 1), "another instance");
    let engine = flat_engine();
    assert!(!engine.can_move_to_target_with(&o, into.0, into.1, 0));
    assert!(engine.can_move_to_target_with(&o, beside.0, beside.1, 0));
    // Fully inside the rectangle: Java skips it.
    assert!(!o.fence_between(loc(X - 50, Y, Z), loc(X + 50, Y, Z), 0));

    // Opened: not geodata enabled; the players are sent the new state.
    w.set_fence_state(fence.object_id, FenceState::Opened);
    let out = log.take();
    let infos = fence_infos(&out, 1);
    assert_eq!(infos.len(), 3);
    assert!(infos.iter().all(|i| i.state == 1));
    assert!(!w.obstacles().fence_between(into.0, into.1, 0));
    assert!(!w.door_snapshot().fence_between(into.0, into.1, 0));
    w.set_fence_state(fence.object_id, FenceState::ClosedHidden);
    assert!(w.obstacles().fence_between(into.0, into.1, 0));
    assert!(w.door_snapshot().fence_between(into.0, into.1, 0));
    log.take();

    // Deleting the fence deletes the height pieces first, then the fence.
    w.remove_fence(fence.object_id);
    let out = log.take();
    assert_eq!(
        deletes(&out, 1),
        [
            fence.height_fences[0].0,
            fence.height_fences[1].0,
            fence.object_id.0
        ]
    );
    assert!(w.fence(fence.object_id).is_none());
    assert!(!w.obstacles().fence_between(into.0, into.1, 0));
}

// ---------------------------------------------------------------------------------------------
// Static objects

#[test]
fn static_objects_are_sent_as_static_object_info() {
    let log = Log::default();
    let mut w = world(&log);
    let templates = StaticObjectTemplates::from_xml(&format!(
        r#"<list><object id="19210010" name="throne" type="1" x="{}" y="{Y}" z="{Z}"/>
        <object id="17250001" name="map" type="0" x="{}" y="{Y}" z="{Z}" texture="t" map_x="3" map_y="4"/></list>"#,
        X + 50,
        X + 80
    ))
    .unwrap();
    let ids = ids();
    assert_eq!(w.spawn_static_objects(&templates, &ids).unwrap(), 2);
    assert_eq!(ids.used_count(), 2);
    enter(&mut w, &log, 1, X, Y);
    let out = log.take();
    let infos = static_infos(&out, 1);
    assert_eq!(infos.len(), 2);
    // Java's StaticObjectInfo(StaticObject): type 0 whatever the object, open, targetable, no hp.
    let object = w
        .static_objects()
        .find(|o| o.static_object_id == 19_210_010)
        .unwrap();
    assert_eq!(
        infos[0],
        StaticObjectInfo::static_object(19_210_010, object.object_id.0, 0)
    );
    assert_eq!(infos[0].kind, 0);
    let map = w
        .static_objects()
        .find(|o| o.static_object_id == 17_250_001)
        .unwrap();
    assert_eq!((map.texture.as_str(), map.map_x, map.map_y), ("t", 3, 4));

    // setMeshIndex tells the viewers again.
    w.set_static_object_mesh_index(map.object_id, 1);
    let out = log.take();
    let infos = static_infos(&out, 1);
    assert_eq!((infos[0].static_object_id, infos[0].mesh_index), (17_250_001, 1));
}

// ---------------------------------------------------------------------------------------------
// The real datapack

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"), PathBuf::from);
    if dir.join("data").join("Doors.xml").is_file() {
        Some(dir)
    } else {
        println!("skipped: {} has no data/Doors.xml", dir.display());
        None
    }
}

#[test]
fn the_real_datapack_spawns_every_door_fence_and_static_object() {
    let Some(root) = game_dir() else { return };
    let log = Log::default();
    let mut w = world(&log);
    let ids = ids();
    let started = std::time::Instant::now();
    let doors = DoorTemplates::load(&root);
    let fences = FenceTemplates::load(&root);
    let objects = StaticObjectTemplates::load(&root);
    assert_eq!(w.spawn_doors(&doors, &ids).unwrap(), 1301);
    assert_eq!(w.spawn_fences(&fences, &ids).unwrap(), 1);
    assert_eq!(w.spawn_static_objects(&objects, &ids).unwrap(), 51);
    let spawn_time = started.elapsed();
    println!("spawned 1301 doors, 1 fence, 51 static objects in {spawn_time:?}");
    assert_eq!(w.doors().count(), 1301);
    assert_eq!(
        w.doors().filter(|d| d.open).count(),
        42,
        "default_status=open doors"
    );
    // 1301 doors + 1 fence (+2 height pieces) + 51 static objects.
    assert_eq!(ids.used_count(), 1301 + 3 + 51);
    // The snapshot holds the closed doors that collide.
    let blocking = w
        .doors()
        .filter(|d| !d.open && d.template.check_collision && d.template.node_x[0] != 0)
        .count();
    assert_eq!(w.door_snapshot().door_count(), blocking);
    assert_eq!(w.door_snapshot().fence_count(), 1);
}

#[test]
fn a_real_olympiad_door_blocks_a_walk_across_it() {
    let Some(root) = game_dir() else { return };
    let log = Log::default();
    let mut w = world(&log);
    let doors = DoorTemplates::load(&root);
    w.spawn_doors(&doors, &ids()).unwrap();
    // olympiad_door_046: closed, nodes x -154371..-154346, y -252765..-252510, nodeZ -3341, 150 high.
    let (from, to) = (loc(-154_450, -252_600, -3_300), loc(-154_250, -252_600, -3_300));
    let engine = flat_engine();
    assert!(!engine.can_move_to_target_with(&w.obstacles(), from, to, 0));
    let id = w.door_by_door_id(15_100_001).unwrap().object_id;
    w.open_door(id);
    assert!(engine.can_move_to_target_with(&w.obstacles(), from, to, 0));
}
