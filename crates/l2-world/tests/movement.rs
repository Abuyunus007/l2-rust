#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Player movement: MoveToLocation -> checks -> per-tick updates -> broadcasts, geodata,
//! pathfinding through the request service, ValidatePosition corrections, and the recorded Java
//! session (`fixtures/captures/java-gs-session.jsonl`).
//! Java: Creature.moveToLocation / updatePosition / stopMove, MovementTaskManager, the client
//! packets MoveToLocation / ValidatePosition / CannotMoveAnymore.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_geo::block::BLOCK_CELLS;
use l2_geo::cell::NSWE_ALL;
use l2_geo::coords::{world_x, world_y};
use l2_geo::region::REGION_BLOCKS;
use l2_geo::{
    GeoEngine, GeoEngineOptions, GeoSet, Location, NoObstacles, PathFinder, PathOptions, PathRequest,
    PathResult, Region,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{MoveToLocation, StopMove, ValidateLocation};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Debug, Clone, PartialEq)]
struct Out {
    to: i32,
    bytes: Vec<u8>,
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
        self.1.0.lock().unwrap().push(Out { to: self.0, bytes });
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

struct RecOutbox;

impl Outbox for RecOutbox {
    fn persist(&mut self, _command: PersistCommand) {}

    fn ls_event(&mut self, _event: LsEvent) {}
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn names_to(out: &[Out], id: i32) -> Vec<String> {
    out.iter()
        .filter(|o| o.to == id)
        .map(|o| name_of(&o.bytes))
        .collect()
}

fn decode_all<T: Decode>(out: &[Out], to: i32, name: &str) -> Vec<T> {
    out.iter()
        .filter(|o| o.to == to && name_of(&o.bytes) == name)
        .map(|o| T::decode(&mut PacketReader::new(&o.bytes[1..])).unwrap())
        .collect()
}

fn moves_to(out: &[Out], to: i32) -> Vec<MoveToLocation> {
    decode_all(out, to, "MOVE_TO_LOCATION")
}

fn stops_to(out: &[Out], to: i32) -> Vec<StopMove> {
    decode_all(out, to, "STOP_MOVE")
}

fn validates_to(out: &[Out], to: i32) -> Vec<ValidateLocation> {
    decode_all(out, to, "VALIDATE_LOCATION")
}

/// Open ground in the Talking Island area (no geodata).
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

fn player(id: i32, x: i32, y: i32, z: i32) -> PlayerData {
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
        z,
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

/// The class template of the test character (class 0): only its collision, 9 x 23 like the
/// Human Fighter's; everything else keeps Java's template defaults (run 120, walk 50).
struct ClassData(StatData);

impl ClassData {
    fn new() -> Self {
        let templates = l2_data::PlayerTemplates::from_xml(
            r#"<list><classId>0</classId><staticData>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>10</cp></level></lvlUpgainData></list>"#,
            85,
        )
        .unwrap();
        Self(StatData {
            player_templates: Arc::new(templates),
            ..StatData::default()
        })
    }
}

impl StaticData for ClassData {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

fn new_world() -> World {
    let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox));
    world.set_static_data(Box::new(ClassData::new()));
    world.set_settings(WorldSettings {
        // Every region is active, so the geodata checks run without waiting for the neighbour timer.
        grids_always_on: true,
        ..WorldSettings::default()
    });
    world
}

fn enter(world: &mut World, log: &Log, data: PlayerData) {
    let id = data.object_id.0;
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(data),
        sink: Box::new(Sink(id, log.clone())),
        ip: None,
    });
}

fn input(world: &mut World, id: i32, input: ClientInput) {
    world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(id).unwrap()),
        input,
    });
}

fn move_to(world: &mut World, id: i32, target: (i32, i32, i32), origin: (i32, i32, i32), mode: i32) {
    input(
        world,
        id,
        ClientInput::MoveTo {
            target_x: target.0,
            target_y: target.1,
            target_z: target.2,
            origin_x: origin.0,
            origin_y: origin.1,
            origin_z: origin.2,
            movement_mode: mode,
        },
    );
}

fn validate(world: &mut World, id: i32, x: i32, y: i32, z: i32) {
    input(world, id, ClientInput::ValidatePosition { x, y, z, heading: 0 });
}

fn pos(world: &World, id: i32) -> (i32, i32, i32) {
    world.position_of(ObjectId(id)).unwrap()
}

fn tick(world: &mut World, n: u32) {
    for _ in 0..n {
        world.tick();
    }
}

/// Ticks until the player stands still; panics after `max` ticks.
fn tick_until_stopped(world: &mut World, id: i32, max: u32) -> u32 {
    for n in 1..=max {
        world.tick();
        if !world.is_moving(ObjectId(id)) {
            return n;
        }
    }
    panic!("still moving after {max} ticks");
}

fn world_with_player(log: &Log) -> World {
    let mut world = new_world();
    enter(&mut world, log, player(1, X, Y, Z));
    log.take();
    world
}

// ---------------------------------------------------------------------------------- straight walk

#[test]
fn a_walk_moves_one_step_per_game_tick_with_javas_numbers() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    move_to(&mut world, 1, (X + 1000, Y, Z), (X, Y, Z), 1);

    // MoveToLocation: destination first, then the current position; to the player itself.
    let out = log.take();
    assert_eq!(names_to(&out, 1), ["MOVE_TO_LOCATION"]);
    let first = moves_to(&out, 1)[0];
    assert_eq!(
        first,
        MoveToLocation {
            object_id: 1,
            dest_x: X + 1000,
            dest_y: Y,
            dest_z: Z,
            x: X,
            y: Y,
            z: Z
        }
    );
    assert!(world.is_moving(ObjectId(1)));
    assert_eq!(world.player(ObjectId(1)).unwrap().core.heading, 0, "facing east");

    // Speed 120 (run speed 120, multiplier 1), collision radius 9, 100 ms per step:
    //   delta = 1000 - 9 = 991, fraction = 12 / 991, x += 1000 * 12 / 991 = 12.1089
    //   delta = 987.8911 - 9 = 978.8911, fraction = 12 / 978.8911, x += 12.1103 -> 24.2192
    // The position is `(int) xAccurate`, which truncates toward zero: X is negative here, so
    // -71325.89 becomes -71325 (+13) and -71313.78 becomes -71313 (+25).
    // A game tick is two world ticks, so the position only changes on every second tick.
    let mut seen = Vec::new();
    for _ in 0..6 {
        world.tick();
        seen.push(pos(&world, 1).0 - X);
    }
    assert_eq!(seen, [0, 13, 13, 25, 25, 37]);

    // Same numbers from an independent run of the formula.
    let mut acc = f64::from(X);
    for step in 1..=3 {
        let dx = f64::from(X + 1000) - acc;
        let delta = (dx * dx).sqrt() - 9.0;
        acc += dx * (12.0 / delta);
        assert_eq!(acc as i32 - X, 12 * step + 1, "step {step}");
    }
    assert_eq!(pos(&world, 1).1, Y);
}

#[test]
fn arriving_sets_the_destination_broadcasts_once_and_goes_idle() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    move_to(&mut world, 1, (X + 100, Y + 50, Z), (X, Y, Z), 1);
    log.take();

    let ticks = tick_until_stopped(&mut world, 1, 100);
    assert!(ticks > 4);
    assert_eq!(pos(&world, 1), (X + 100, Y + 50, Z));
    let out = log.take();
    // Java sends the arrival MoveToLocation (cur == dest) and no StopMove.
    assert_eq!(names_to(&out, 1), ["MOVE_TO_LOCATION"]);
    let arrival = moves_to(&out, 1)[0];
    assert_eq!(
        (arrival.x, arrival.y, arrival.z),
        (arrival.dest_x, arrival.dest_y, arrival.dest_z)
    );
    assert_eq!((arrival.x, arrival.y), (X + 100, Y + 50));
    assert_eq!(
        world.player(ObjectId(1)).unwrap().movement.intention,
        Intention::Idle
    );
    // Heading of atan2(50, 100) in client units.
    let expected = (50.0_f64.atan2(100.0).to_degrees() * 182.044_444_444) as i32;
    assert_eq!(world.player(ObjectId(1)).unwrap().core.heading, expected);
}

#[test]
fn a_target_equal_to_the_origin_just_stops() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    move_to(&mut world, 1, (X + 500, Y, Z), (X, Y, Z), 1);
    log.take();
    move_to(&mut world, 1, (X + 7, Y, Z), (X + 7, Y, Z), 1);
    let out = log.take();
    assert_eq!(names_to(&out, 1), ["STOP_MOVE", "ACTION_FAIL"]);
    let stop = stops_to(&out, 1)[0];
    assert_eq!((stop.x, stop.y, stop.z), (X, Y, Z));
    assert!(!world.is_moving(ObjectId(1)));
}

#[test]
fn a_huge_distance_is_refused() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    move_to(&mut world, 1, (X + 9901, Y, Z), (X, Y, Z), 1);
    assert_eq!(names_to(&log.take(), 1), ["ACTION_FAIL"]);
    assert!(!world.is_moving(ObjectId(1)));
    // 9900 is still allowed (> 98010000 is the refusal).
    move_to(&mut world, 1, (X + 9900, Y, Z), (X, Y, Z), 1);
    assert!(world.is_moving(ObjectId(1)));
}

#[test]
fn clicking_where_you_stand_is_arrival_and_action_failed() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    // Target differs from the origin, but is where the server has the player.
    move_to(&mut world, 1, (X, Y, Z), (X + 30, Y, Z), 1);
    assert_eq!(names_to(&log.take(), 1), ["ACTION_FAIL"]);
    assert!(!world.is_moving(ObjectId(1)));
}

#[test]
fn a_new_click_replaces_the_walk_and_a_dead_player_cannot_walk() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    move_to(&mut world, 1, (X + 1000, Y, Z), (X, Y, Z), 1);
    tick(&mut world, 4);
    log.take();
    let (cx, cy, _) = pos(&world, 1);
    move_to(&mut world, 1, (X, Y + 800, Z), (cx, cy, Z), 1);
    let m = moves_to(&log.take(), 1)[0];
    assert_eq!((m.dest_x, m.dest_y), (X, Y + 800));
    assert_eq!((m.x, m.y), (cx, cy));

    let mut world = new_world();
    let mut dead = player(1, X, Y, Z);
    dead.cur_hp = 0.0;
    let log = Log::default();
    enter(&mut world, &log, dead);
    log.take();
    move_to(&mut world, 1, (X + 100, Y, Z), (X, Y, Z), 1);
    assert_eq!(names_to(&log.take(), 1), ["ACTION_FAIL"]);
}

#[test]
fn others_in_view_get_the_broadcasts_and_far_players_do_not() {
    let log = Log::default();
    let mut world = new_world();
    enter(&mut world, &log, player(1, X, Y, Z));
    enter(&mut world, &log, player(2, X + 300, Y, Z));
    enter(&mut world, &log, player(3, X + 40_000, Y + 40_000, Z));
    log.take();
    move_to(&mut world, 1, (X + 100, Y, Z), (X, Y, Z), 1);
    let out = log.take();
    assert_eq!(names_to(&out, 1), ["MOVE_TO_LOCATION"]);
    assert_eq!(names_to(&out, 2), ["MOVE_TO_LOCATION"]);
    assert!(names_to(&out, 3).is_empty());
    tick_until_stopped(&mut world, 1, 100);
    let out = log.take();
    assert_eq!(names_to(&out, 2), ["MOVE_TO_LOCATION"], "the arrival");
}

#[test]
fn crossing_a_region_border_switches_regions_with_the_visibility_packets() {
    // Regions are 2048 wide. A walks east out of region R into R + 1; B stands in R - 1, which
    // sees R but not R + 1. (Region index = (x >> 11) + 144, so 20480 is the start of one.)
    let log = Log::default();
    let mut world = new_world();
    let border = 2048 * 10;
    enter(&mut world, &log, player(1, border - 40, Y, Z));
    enter(&mut world, &log, player(2, border - 2 * 2048 + 100, Y, Z));
    assert!(world.visible_players(ObjectId(1)).contains(&ObjectId(2)));
    log.take();
    move_to(&mut world, 1, (border + 400, Y, Z), (border - 40, Y, Z), 1);
    tick_until_stopped(&mut world, 1, 200);
    let out = log.take();
    assert!(names_to(&out, 2).contains(&"DELETE_OBJECT".to_owned()));
    assert!(names_to(&out, 1).contains(&"DELETE_OBJECT".to_owned()));
    assert!(!world.visible_players(ObjectId(1)).contains(&ObjectId(2)));
}

// ---------------------------------------------------------------------------------- stop / blocked

#[test]
fn cannot_move_anymore_stops_where_the_server_is() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    move_to(&mut world, 1, (X + 1000, Y, Z), (X, Y, Z), 1);
    tick(&mut world, 6);
    log.take();
    let here = pos(&world, 1);
    input(
        &mut world,
        1,
        ClientInput::CannotMoveAnymore {
            x: X + 999,
            y: Y + 77,
            z: Z,
            heading: 1234,
        },
    );
    // Intention MOVE_TO -> idle -> StopMove at the server position (the client's report is not
    // adopted in that case).
    let out = log.take();
    assert_eq!(names_to(&out, 1), ["STOP_MOVE"]);
    let stop = stops_to(&out, 1)[0];
    assert_eq!((stop.x, stop.y, stop.z), here);
    assert!(!world.is_moving(ObjectId(1)));
    tick(&mut world, 10);
    assert_eq!(pos(&world, 1), here);
    // Not moving: nothing to stop, nothing sent.
    input(
        &mut world,
        1,
        ClientInput::CannotMoveAnymore {
            x: 1,
            y: 1,
            z: 1,
            heading: 0,
        },
    );
    assert!(log.take().is_empty());
}

// ---------------------------------------------------------------------------------- geodata

/// World coordinates of region 20_18 start at (0, 0).
const RX: i32 = 20 * 2048;
const RY: i32 = 18 * 2048;
/// A wall one cell thick at geo x = RX + 1000, y = RY + 960 .. RY + 1040 (flat ground at z = 0).
const WALL_CELL_X: i32 = RX + 1000;

fn wall_engine(pathfinding: i32) -> Arc<GeoEngine> {
    let blocks: Vec<(i32, i32)> = (960 / 8..=1040 / 8).map(|by| (1000 / 8, by)).collect();
    let wall = |x: i32, y: i32| x == WALL_CELL_X && (RY + 960..RY + 1040).contains(&y);
    let mut bytes = Vec::new();
    for block in 0..REGION_BLOCKS {
        let bx = (block / 256) as i32;
        let by = (block % 256) as i32;
        if blocks.contains(&(bx, by)) {
            bytes.push(1);
            for cell in 0..(BLOCK_CELLS as i32) {
                let cx = RX + bx * 8 + cell / 8;
                let cy = RY + by * 8 + cell % 8;
                let nswe = if wall(cx, cy) { 0 } else { NSWE_ALL };
                bytes.extend_from_slice(&u16::from(nswe).to_le_bytes());
            }
        } else {
            bytes.push(0);
            bytes.extend_from_slice(&0i16.to_le_bytes());
        }
    }
    let mut set = GeoSet::new();
    assert!(set.insert(20, 18, Region::parse(&bytes).unwrap()));
    Arc::new(GeoEngine::new(
        set,
        GeoEngineOptions {
            pathfinding,
            avoid_obstructed_path_nodes: true,
        },
    ))
}

/// World position of the middle of a geo cell, at ground height 0.
fn cell(geo_x: i32, geo_y: i32) -> (i32, i32, i32) {
    (world_x(geo_x), world_y(geo_y), 0)
}

fn geo_world(log: &Log, pathfinding: i32, start: (i32, i32, i32)) -> World {
    let mut world = new_world();
    world.set_geo(wall_engine(pathfinding));
    enter(&mut world, log, player(1, start.0, start.1, start.2));
    log.take();
    world
}

#[test]
fn a_straight_move_into_a_wall_ends_before_it() {
    // No path service: Java finds no path, so `getValidLocation` corrects the destination.
    let log = Log::default();
    let start = cell(WALL_CELL_X - 30, RY + 1000);
    let mut world = geo_world(&log, 2, start);
    let target = cell(WALL_CELL_X + 30, RY + 1000);
    move_to(&mut world, 1, target, start, 1);
    let m = moves_to(&log.take(), 1)[0];
    // The middle of the cell in front of the wall.
    assert_eq!(m.dest_x, world_x(WALL_CELL_X - 1));
    assert!(m.dest_x > start.0);
    assert_eq!(m.dest_y, start.1);
    assert_eq!(m.dest_z, 0, "a player keeps the z it asked for");
    tick_until_stopped(&mut world, 1, 200);
    assert_eq!(pos(&world, 1).0, m.dest_x);
}

#[test]
fn a_move_free_of_obstacles_is_not_changed_by_the_geodata() {
    let log = Log::default();
    let start = cell(WALL_CELL_X - 30, RY + 500);
    let mut world = geo_world(&log, 0, start);
    let target = cell(WALL_CELL_X + 30, RY + 500);
    move_to(&mut world, 1, target, start, 1);
    let m = moves_to(&log.take(), 1)[0];
    assert_eq!((m.dest_x, m.dest_y, m.dest_z), target);
}

#[test]
fn a_completely_blocked_target_is_refused() {
    let log = Log::default();
    let start = cell(WALL_CELL_X - 30, RY + 1000);
    let mut world = geo_world(&log, 2, start);
    let target = cell(WALL_CELL_X, RY + 1000);
    move_to(&mut world, 1, target, start, 1);
    assert_eq!(names_to(&log.take(), 1), ["ACTION_FAIL"]);
    assert!(!world.is_moving(ObjectId(1)));
}

// ---------------------------------------------------------------------------------- pathfinding

#[derive(Clone, Default)]
struct Requests(Arc<Mutex<Vec<PathRequest>>>);

struct RecordingRequests(Requests, bool);

impl PathRequests for RecordingRequests {
    fn request(&mut self, request: PathRequest) -> bool {
        self.0.0.lock().unwrap().push(request);
        self.1
    }

    fn cancel(&mut self, _requester: u64) {}
}

/// Runs a request the way a pool worker does.
fn search(engine: &GeoEngine, request: &PathRequest) -> PathResult {
    let mut finder = PathFinder::new(PathOptions::shipped()).unwrap();
    let path = finder.find_path_detailed(
        engine,
        &NoObstacles,
        request.from,
        request.to,
        request.instance_id,
        request.playable,
    );
    PathResult {
        requester: request.requester,
        id: request.id,
        path,
        elapsed: std::time::Duration::ZERO,
    }
}

fn path_found(world: &mut World, result: PathResult) {
    world.handle(WorldCommand::PathFound {
        requester: result.requester,
        request_id: result.id,
        result,
    });
}

fn path_world(log: &Log, start: (i32, i32, i32), requests: &Requests) -> (World, Arc<GeoEngine>) {
    let engine = wall_engine(2);
    let mut world = new_world();
    world.set_geo(Arc::clone(&engine));
    world.set_path_requests(Box::new(RecordingRequests(requests.clone(), true)));
    enter(&mut world, log, player(1, start.0, start.1, start.2));
    log.take();
    (world, engine)
}

#[test]
fn a_blocked_line_asks_for_a_path_and_follows_it_around_the_wall() {
    let log = Log::default();
    let requests = Requests::default();
    let start = cell(WALL_CELL_X - 10, RY + 1000);
    let (mut world, engine) = path_world(&log, start, &requests);
    let target = cell(WALL_CELL_X + 10, RY + 1000);
    move_to(&mut world, 1, target, start, 1);

    // The request is out, nothing moves and nothing is sent until the answer is in (D-14).
    assert!(log.take().is_empty());
    assert!(!world.is_moving(ObjectId(1)));
    assert!(world.player(ObjectId(1)).unwrap().movement.is_waiting_for_path());
    let sent = requests.0.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].requester, 1);
    assert_eq!(
        (sent[0].from, sent[0].to),
        (
            Location::new(start.0, start.1, start.2),
            Location::new(target.0, target.1, target.2)
        )
    );
    assert!(sent[0].playable);

    let result = search(&engine, &sent[0]);
    let nodes = result.path.clone().unwrap();
    assert!(nodes.len() > 1, "the line is blocked, so there is a detour");
    path_found(&mut world, result);
    assert!(!world.player(ObjectId(1)).unwrap().movement.is_waiting_for_path());

    // First segment: to the first node of the path.
    let first = moves_to(&log.take(), 1);
    assert_eq!(first.len(), 1);
    assert_eq!((first[0].dest_x, first[0].dest_y), (nodes[0].x(), nodes[0].y()));
    assert_eq!((first[0].x, first[0].y), (start.0, start.1));

    // Walk it all: every node gives one MoveToLocation (cur = where the previous segment ended),
    // the last one goes to the exact clicked point, and the walk ends there.
    let mut moves = first;
    let wall_x = world_x(WALL_CELL_X);
    for _ in 0..3000 {
        world.tick();
        let (px, py, _) = pos(&world, 1);
        // Never inside the wall's cell.
        assert!(!((px - wall_x).abs() < 8 && (world_y(RY + 960)..world_y(RY + 1040)).contains(&py)));
        moves.extend(moves_to(&log.take(), 1));
        if !world.is_moving(ObjectId(1)) {
            break;
        }
    }
    assert!(!world.is_moving(ObjectId(1)), "arrived");
    assert_eq!(pos(&world, 1), (target.0, target.1, target.2));
    // The first segment, one per further node (forced broadcast), and the arrival.
    assert_eq!(moves.len(), nodes.len() + 1);
    let last = moves.last().unwrap();
    assert_eq!((last.dest_x, last.dest_y), (target.0, target.1));
    assert_eq!((last.x, last.y), (target.0, target.1), "the arrival broadcast");
    // Each segment starts where the previous one ended.
    for pair in moves.windows(2) {
        assert_eq!((pair[1].x, pair[1].y), (pair[0].dest_x, pair[0].dest_y));
    }
    // The path really goes around: some segment passes the wall's end in y.
    assert!(
        moves
            .iter()
            .any(|m| m.dest_y < world_y(RY + 960) || m.dest_y > world_y(RY + 1040))
    );
}

#[test]
fn a_stale_path_result_is_dropped() {
    let log = Log::default();
    let requests = Requests::default();
    let start = cell(WALL_CELL_X - 10, RY + 1000);
    let (mut world, engine) = path_world(&log, start, &requests);
    let target = cell(WALL_CELL_X + 10, RY + 1000);
    move_to(&mut world, 1, target, start, 1);
    // A second click (another blocked one, a different cell) replaces the first request.
    let target2 = cell(WALL_CELL_X + 11, RY + 1010);
    move_to(&mut world, 1, target2, start, 1);
    let sent = requests.0.lock().unwrap().clone();
    assert_eq!(sent.len(), 2);
    assert!(sent[1].id > sent[0].id);

    path_found(&mut world, search(&engine, &sent[0]));
    assert!(log.take().is_empty(), "the old answer is ignored");
    assert!(!world.is_moving(ObjectId(1)));
    path_found(&mut world, search(&engine, &sent[1]));
    let m = moves_to(&log.take(), 1);
    assert_eq!(m.len(), 1);
    assert!(world.is_moving(ObjectId(1)));
}

#[test]
fn stopping_while_the_path_is_out_drops_the_answer() {
    let log = Log::default();
    let requests = Requests::default();
    let start = cell(WALL_CELL_X - 10, RY + 1000);
    let (mut world, engine) = path_world(&log, start, &requests);
    let target = cell(WALL_CELL_X + 10, RY + 1000);
    move_to(&mut world, 1, target, start, 1);
    let sent = requests.0.lock().unwrap().clone();
    input(
        &mut world,
        1,
        ClientInput::CannotMoveAnymore {
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
        },
    );
    assert!(!world.player(ObjectId(1)).unwrap().movement.is_waiting_for_path());
    path_found(&mut world, search(&engine, &sent[0]));
    assert!(log.take().is_empty());
    assert!(!world.is_moving(ObjectId(1)));
}

#[test]
fn a_refused_request_or_a_missing_service_walks_straight() {
    // Java: no path found -> direct movement.
    for with_service in [true, false] {
        let log = Log::default();
        let engine = wall_engine(2);
        let mut world = new_world();
        world.set_geo(Arc::clone(&engine));
        if with_service {
            world.set_path_requests(Box::new(RecordingRequests(Requests::default(), false)));
        }
        let start = cell(WALL_CELL_X - 10, RY + 1000);
        enter(&mut world, &log, player(1, start.0, start.1, start.2));
        log.take();
        let target = cell(WALL_CELL_X + 10, RY + 1000);
        move_to(&mut world, 1, target, start, 1);
        let m = moves_to(&log.take(), 1);
        assert_eq!(m.len(), 1, "with_service = {with_service}");
        // No path: direct movement, with the destination corrected before the wall.
        assert_eq!((m[0].dest_x, m[0].dest_y), (world_x(WALL_CELL_X - 1), target.1));
    }
}

#[test]
fn a_search_without_a_path_walks_straight_to_the_corrected_target() {
    let log = Log::default();
    let requests = Requests::default();
    let start = cell(WALL_CELL_X - 10, RY + 1000);
    let (mut world, engine) = path_world(&log, start, &requests);
    let target = cell(WALL_CELL_X + 10, RY + 1000);
    move_to(&mut world, 1, target, start, 1);
    let sent = requests.0.lock().unwrap().clone();
    let mut result = search(&engine, &sent[0]);
    result.path = Err(l2_geo::PathFailure::NoPath);
    path_found(&mut world, result);
    let m = moves_to(&log.take(), 1);
    assert_eq!((m[0].dest_x, m[0].dest_y), (world_x(WALL_CELL_X - 1), target.1));
}

// ---------------------------------------------------------------------------------- ValidatePosition

#[test]
fn validate_position_inside_the_tolerance_changes_nothing() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    validate(&mut world, 1, X + 20, Y - 10, Z + 26);
    assert!(log.take().is_empty());
    assert_eq!(pos(&world, 1), (X, Y, Z));
    let p = world.player(ObjectId(1)).unwrap();
    assert_eq!((p.movement.client_x, p.movement.client_z), (X + 20, Z + 26));
    assert_eq!(p.movement.last_server_position, (X, Y, Z));
}

#[test]
fn validate_position_far_off_in_xy_sends_validate_location_and_adopts_the_client() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    // 550 units: 302500 > 250000 and < 360000 -> ValidateLocation; 550 > the speed 120 -> the
    // server also adopts the client's position (and ValidateLocation went out before that).
    validate(&mut world, 1, X + 550, Y, Z);
    let out = log.take();
    let v = validates_to(&out, 1);
    assert_eq!(v.len(), 1);
    assert_eq!(
        (v[0].x, v[0].y, v[0].z),
        (X, Y, Z),
        "the position before the correction"
    );
    assert_eq!(pos(&world, 1), (X + 550, Y, Z));
    // 700 units: 490000 >= 360000 -> no ValidateLocation, but still out of sync.
    validate(&mut world, 1, X + 550 + 700, Y, Z);
    assert!(log.take().is_empty());
    assert_eq!(pos(&world, 1), (X + 1250, Y, Z));
}

#[test]
fn validate_position_just_out_of_sync_adopts_the_client_position_silently() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    validate(&mut world, 1, X + 121, Y, Z);
    assert!(log.take().is_empty());
    assert_eq!(pos(&world, 1), (X + 121, Y, Z));
    // Exactly the speed (120) is still in sync.
    validate(&mut world, 1, X + 121 + 120, Y, Z);
    assert_eq!(pos(&world, 1), (X + 121, Y, Z));
}

#[test]
fn validate_position_z_corrections() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    // First report: the client z is not yet known (0), so 300 above is not trusted: ValidateLocation.
    validate(&mut world, 1, X, Y, Z + 300);
    let out = log.take();
    assert_eq!(validates_to(&out, 1).len(), 1);
    // Now client z is -2804; the same report is within 800 of it, so the server adopts z.
    validate(&mut world, 1, X, Y, Z + 300);
    assert!(log.take().is_empty());
    assert_eq!(pos(&world, 1).2, Z + 300);
    // 1500 or more is never adopted.
    validate(&mut world, 1, X, Y, Z + 300 + 1500);
    assert_eq!(validates_to(&log.take(), 1).len(), 1);
}

#[test]
fn validate_position_ignores_zero_and_absurd_reports() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    validate(&mut world, 1, 0, 0, Z);
    assert!(log.take().is_empty(), "0/0 is ignored");
    // An abnormal z: Java `teleToLocation(lastServerPosition)` (M3-TELE-01).
    validate(&mut world, 1, X, Y, 20_001);
    let out = log.take();
    assert!(
        out.iter().any(|o| o.to == 1 && o.bytes[0] == 0x22),
        "TeleportToLocation to the last server position"
    );
    // Java `teleToLocation` lifts the target by 5 ("z += 5").
    assert_eq!(pos(&world, 1), (X, Y, Z + 5));
    assert_eq!(world.player(ObjectId(1)).unwrap().movement.client_z, 0);
}

#[test]
fn a_big_drop_with_geodata_sends_validate_location_and_stops_trusting_z_for_a_second() {
    let log = Log::default();
    let start = cell(WALL_CELL_X - 40, RY + 500);
    let mut world = geo_world(&log, 0, start);
    // Safe fall height is 333 by default; a report 400 below is a fall.
    validate(&mut world, 1, start.0, start.1, start.2 - 400);
    let out = log.take();
    assert_eq!(validates_to(&out, 1).len(), 1);
    // For the next second the reports are skipped (no correction at all).
    validate(&mut world, 1, start.0 + 500, start.1, start.2);
    assert!(log.take().is_empty());
    assert_eq!(pos(&world, 1).0, start.0);
    tick(&mut world, 21);
    validate(&mut world, 1, start.0 + 500, start.1, start.2);
    assert_eq!(pos(&world, 1).0, start.0 + 500, "trusted again");
}

#[test]
fn validate_position_while_walking_keeps_the_walk() {
    let log = Log::default();
    let mut world = world_with_player(&log);
    move_to(&mut world, 1, (X + 1000, Y, Z), (X, Y, Z), 1);
    tick(&mut world, 6);
    log.take();
    let (x, y, z) = pos(&world, 1);
    validate(&mut world, 1, x + 30, y, z);
    assert!(log.take().is_empty());
    assert!(world.is_moving(ObjectId(1)));
    assert_eq!(pos(&world, 1), (x, y, z));
    assert_eq!(
        world.player(ObjectId(1)).unwrap().movement.last_server_position,
        (x, y, z)
    );
}

// ---------------------------------------------------------------------------------- cursor keys

#[test]
fn cursor_key_movement_syncs_to_the_client_and_broadcasts_validations() {
    let log = Log::default();
    let mut world = new_world();
    enter(&mut world, &log, player(1, X, Y, Z));
    enter(&mut world, &log, player(2, X + 100, Y, Z));
    log.take();
    // Mode 0: the origin the client reports (close by) is adopted first.
    move_to(&mut world, 1, (X + 600, Y, Z), (X + 25, Y + 5, Z), 0);
    let m = moves_to(&log.take(), 1);
    assert_eq!((m[0].x, m[0].y), (X + 25, Y + 5));
    let p = world.player(ObjectId(1)).unwrap();
    assert!(p.movement.cursor_key_movement);
    assert_eq!(p.movement.last_server_position, (X + 25, Y + 5, Z));
    tick(&mut world, 4);
    log.take();
    // While the keys are held, every validation is adopted and broadcast to everybody.
    validate(&mut world, 1, X + 140, Y + 5, Z);
    let out = log.take();
    assert_eq!(names_to(&out, 1), ["VALIDATE_LOCATION"]);
    assert_eq!(names_to(&out, 2), ["VALIDATE_LOCATION"]);
    assert_eq!(pos(&world, 1), (X + 140, Y + 5, Z));
    // A mouse click ends the key mode.
    move_to(&mut world, 1, (X + 900, Y, Z), (X + 140, Y + 5, Z), 1);
    assert!(!world.player(ObjectId(1)).unwrap().movement.cursor_key_movement);
}

#[test]
fn keyboard_movement_can_be_switched_off() {
    let log = Log::default();
    let mut world = new_world();
    world.set_settings(WorldSettings {
        grids_always_on: true,
        keyboard_movement: false,
        ..WorldSettings::default()
    });
    enter(&mut world, &log, player(1, X, Y, Z));
    log.take();
    move_to(&mut world, 1, (X + 600, Y, Z), (X, Y, Z - 1), 0);
    assert!(log.take().is_empty(), "Java returns without a word");
    assert!(!world.is_moving(ObjectId(1)));
}

// ---------------------------------------------------------------------------------- overload

struct Heavy;

impl StaticData for Heavy {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        // Two of them weigh more than the 69000 a character without class data can carry
        // (Java `getMaxLoad()`: floor(CON bonus 1 * 69000 * AltWeightLimit 1)).
        Some(ItemStatic {
            weight: 50_000,
            ..ItemStatic::unknown(item_id)
        })
    }

    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }

    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }

    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
}

#[test]
fn an_overloaded_player_cannot_walk() {
    let log = Log::default();
    let mut world = new_world();
    world.set_static_data(Box::new(Heavy));
    let mut data = player(1, X, Y, Z);
    data.items = vec![ItemData {
        object_id: ObjectId(500),
        item_id: 57,
        count: 2,
        enchant_level: 0,
        loc: "INVENTORY".into(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
    }];
    enter(&mut world, &log, data);
    log.take();
    move_to(&mut world, 1, (X + 100, Y, Z), (X, Y, Z), 1);
    assert_eq!(names_to(&log.take(), 1), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert!(!world.is_moving(ObjectId(1)));
}

// ---------------------------------------------------------------------------------- Java session

#[derive(Debug, Clone)]
struct Rec {
    dir: String,
    name: String,
    raw: Vec<u8>,
}

fn capture() -> Option<Vec<Rec>> {
    let dir = std::env::var_os("L2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"));
    let path = dir.join("java-gs-session.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: {} is not readable", path.display());
        return None;
    };
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["channel"] != "game" {
            continue;
        }
        if let (Some(dir), Some(name), Some(raw)) = (v["dir"].as_str(), v["name"].as_str(), v["raw"].as_str())
        {
            out.push(Rec {
                dir: dir.into(),
                name: name.into(),
                raw: hex::decode(raw).unwrap(),
            });
        }
    }
    Some(out)
}

fn ints(raw: &[u8]) -> Vec<i32> {
    raw[1..]
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// The clicks of the Java session in a stretch of open ground, answered by the world: the
/// MoveToLocation the Java server sent has the same destination and start, the arrival comes with
/// destination = position, and a ValidatePosition close to the server position changes nothing.
#[test]
fn the_world_answers_the_recorded_walks_like_the_java_server() {
    let Some(rows) = capture() else { return };
    // The first walk: c2s MoveToLocation, then the s2c MoveToLocation for the same destination.
    let first_c2s = rows
        .iter()
        .position(|r| r.dir == "c2s" && r.name == "MOVE_TO_LOCATION")
        .expect("a click in the recording");
    let click = ints(&rows[first_c2s].raw); // target xyz, origin xyz, mode
    let target = (click[0], click[1], click[2]);
    let reply = rows[first_c2s..]
        .iter()
        .find(|r| {
            r.dir == "s2c"
                && r.name == "MOVE_TO_LOCATION"
                && ints(&r.raw)[1..4] == [target.0, target.1, target.2]
        })
        .expect("Java's answer");
    let java_reply = ints(&reply.raw);
    let (object_id, java_start) = (java_reply[0], (java_reply[4], java_reply[5], java_reply[6]));

    // The character stands where Java had it.
    let log = Log::default();
    let mut world = new_world();
    enter(
        &mut world,
        &log,
        player(object_id, java_start.0, java_start.1, java_start.2),
    );
    log.take();
    let origin = (click[3], click[4], click[5]);
    move_to(&mut world, object_id, target, origin, click[6]);
    let m = moves_to(&log.take(), object_id);
    assert_eq!(m.len(), 1);
    assert_eq!(
        [
            m[0].object_id,
            m[0].dest_x,
            m[0].dest_y,
            m[0].dest_z,
            m[0].x,
            m[0].y,
            m[0].z
        ],
        java_reply[..7],
        "MoveToLocation fields"
    );

    // The next ValidatePosition of the client (a few units off): nothing happens.
    let validate_row = rows[first_c2s..]
        .iter()
        .find(|r| r.dir == "c2s" && r.name == "VALIDATE_POSITION")
        .expect("a validation");
    let v = ints(&validate_row.raw);
    validate(&mut world, object_id, v[0], v[1], v[2]);
    assert!(log.take().is_empty(), "Java sent nothing for it either");
    assert_eq!(pos(&world, object_id), java_start);

    // The walk ends with a MoveToLocation whose position is the destination, like Java's.
    tick_until_stopped(&mut world, object_id, 400);
    let out = log.take();
    assert_eq!(
        names_to(&out, object_id),
        ["MOVE_TO_LOCATION"],
        "no StopMove at the end"
    );
    let arrival = moves_to(&out, object_id)[0];
    let java_arrival = rows[first_c2s..]
        .iter()
        .filter(|r| r.dir == "s2c" && r.name == "MOVE_TO_LOCATION")
        .map(|r| ints(&r.raw))
        .find(|i| i[0] == object_id && i[1..4] == i[4..7])
        .expect("Java's arrival");
    assert_eq!(
        [
            arrival.object_id,
            arrival.dest_x,
            arrival.dest_y,
            arrival.dest_z,
            arrival.x,
            arrival.y,
            arrival.z
        ],
        java_arrival[..7]
    );

    // The next click, answered from the position the server has now (Java's did not move
    // either on the ValidatePosition in between).
    let second_c2s = rows[first_c2s + 1..]
        .iter()
        .position(|r| r.dir == "c2s" && r.name == "MOVE_TO_LOCATION")
        .map(|p| p + first_c2s + 1)
        .expect("a second click");
    let click = ints(&rows[second_c2s].raw);
    let target2 = (click[0], click[1], click[2]);
    let java_reply2 = rows[second_c2s..]
        .iter()
        .filter(|r| r.dir == "s2c" && r.name == "MOVE_TO_LOCATION")
        .map(|r| ints(&r.raw))
        .find(|i| i[0] == object_id && i[1..4] == [target2.0, target2.1, target2.2])
        .expect("Java's second answer");
    // The client's validation before the second click.
    let v2 = rows[first_c2s..second_c2s]
        .iter()
        .rev()
        .find(|r| r.dir == "c2s" && r.name == "VALIDATE_POSITION")
        .map(|r| ints(&r.raw));
    if let Some(v2) = v2 {
        validate(&mut world, object_id, v2[0], v2[1], v2[2]);
        log.take();
    }
    move_to(
        &mut world,
        object_id,
        target2,
        (click[3], click[4], click[5]),
        click[6],
    );
    let m = moves_to(&log.take(), object_id);
    assert_eq!(
        [m[0].dest_x, m[0].dest_y, m[0].dest_z, m[0].x, m[0].y, m[0].z],
        java_reply2[1..7],
        "second MoveToLocation fields"
    );
}
