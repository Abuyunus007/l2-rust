#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Teleports (`teleToLocation`, `Appearing`), zone membership of players (flags, zone messages,
//! the compass icon) and the nearest-town lookups.
//! Java: Creature.teleToLocation, Player.onTeleported, clientpackets/Appearing, ZoneType,
//! ZoneRegion, Player.revalidateZone, MapRegionData.
//!
//! The zones come from small XML files written to a temporary datapack root; the checks against
//! the real datapack and the recorded Java session skip when `C:\l2-rust\l2jmobius` or the
//! fixture is missing.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use l2_data::{MapRegions, Zones};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{
    DeleteObject, ExSetCompassZoneCode, StopMove, SystemMessage, TeleportToLocation,
    enterworld::EtcStatusUpdate,
};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

/// Packets sent to one player: `(object id, bytes)`.
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

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}

    fn ls_event(&mut self, _event: LsEvent) {}
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

/// The packet names `to` received, in order.
fn names_to(out: &[(i32, Vec<u8>)], to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

/// The decoded packets of one name `to` received.
fn decoded<P: Decode>(out: &[(i32, Vec<u8>)], to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn compass_to(out: &[(i32, Vec<u8>)], to: i32) -> Vec<i32> {
    decoded::<ExSetCompassZoneCode>(out, to, "EX_SET_COMPASS_ZONE_CODE")
        .into_iter()
        .map(|p| p.zone_type)
        .collect()
}

fn messages_to(out: &[(i32, Vec<u8>)], to: i32) -> Vec<i32> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|m| m.id.id())
        .collect()
}

const Z: i32 = -3_104;

fn player(id: i32, x: i32, y: i32, race: i32) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{id}"),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race,
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

fn world(log: &Log, seed: u64) -> World {
    let _ = log;
    World::new(WorldRng::from_seed(seed), Box::new(NullOutbox))
}

fn enter(world: &mut World, log: &Log, id: i32, x: i32, y: i32, race: i32) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(player(id, x, y, race)),
        sink: Box::new(Sink(id, log.clone())),
        ip: None,
    });
}

fn appearing(world: &mut World, id: i32) {
    world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(id).unwrap()),
        input: ClientInput::Appearing,
    });
}

/// A datapack root in the temp folder with `data/zones/test.xml`; removed on drop.
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str, zone_xml: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-tele-{tag}-{}", std::process::id()));
        let zones = dir.join("data").join("zones");
        std::fs::create_dir_all(&zones).unwrap();
        std::fs::write(
            zones.join("test.xml"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n{zone_xml}\n</list>\n"
            ),
        )
        .unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cuboid(id: i32, ty: &str, x1: i32, y1: i32, x2: i32, y2: i32, stats: &str) -> String {
    format!(
        "<zone id=\"{id}\" name=\"z{id}\" type=\"{ty}\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
         {stats}<node X=\"{x1}\" Y=\"{y1}\"/><node X=\"{x2}\" Y=\"{y2}\"/></zone>\n"
    )
}

fn world_with_zones(log: &Log, tag: &str, xml: &str) -> World {
    let root = TempRoot::new(tag, xml);
    let zones = Arc::new(Zones::load(root.path()));
    let regions = Arc::new(MapRegions::load(root.path()));
    let mut w = world(log, 1);
    w.set_zone_data(zones, regions);
    w
}

/// The zone layout of most tests, all in the zone cell around the origin except `far`.
fn test_zones() -> String {
    [
        // Peace area with a town inside, a mother tree for elves, and a disabled peace zone.
        cuboid(1, "PeaceZone", 100, 100, 2_000, 2_000, ""),
        cuboid(2, "TownZone", 100, 100, 2_000, 2_000, ""),
        cuboid(
            3,
            "MotherTreeZone",
            100,
            100,
            2_000,
            2_000,
            "<stat name=\"enterMsgId\" val=\"114\"/><stat name=\"leaveMsgId\" val=\"115\"/>\
             <stat name=\"affectedRace\" val=\"1\"/>",
        ),
        cuboid(4, "ArenaZone", 5_000, 1_000, 6_000, 2_000, ""),
        cuboid(5, "EffectZone", 8_000, 1_000, 9_000, 2_000, ""),
        cuboid(
            6,
            "PeaceZone",
            11_000,
            1_000,
            12_000,
            2_000,
            "<stat name=\"default_enabled\" val=\"false\"/>",
        ),
        cuboid(
            7,
            "ConditionZone",
            14_000,
            1_000,
            15_000,
            2_000,
            "<stat name=\"NoBookmark\" val=\"true\"/>",
        ),
        // Another zone cell (x >> 15 differs): where the teleport tests go.
        cuboid(8, "PeaceZone", 100_000, 100, 102_000, 2_000, ""),
    ]
    .concat()
}

// ---------------------------------------------------------------------------- teleport

const X: i32 = -71_338;
const Y: i32 = 258_271;
/// Far enough from `X`/`Y` to be outside the 3 x 3 regions around it.
const DX: i32 = X + 30_000;

#[test]
fn teleport_sends_java_packets_in_java_order() {
    let log = Log::default();
    let mut w = world(&log, 1);
    enter(&mut w, &log, 1, X, Y, 0);
    enter(&mut w, &log, 2, X + 100, Y, 0);
    enter(&mut w, &log, 3, DX + 100, Y, 0);
    log.take();

    w.tele_to_location(ObjectId(1), TeleportTarget::at(DX, Y, Z).with_heading(1234));
    let out = log.take();
    // StopMove to everybody who sees the player, DeleteObject from decayMe, then
    // TeleportToLocation to the player and to those who saw it leave.
    assert_eq!(
        names_to(&out, 1),
        ["STOP_MOVE", "DELETE_OBJECT", "TELEPORT_TO_LOCATION"]
    );
    assert_eq!(
        names_to(&out, 2),
        ["STOP_MOVE", "DELETE_OBJECT", "TELEPORT_TO_LOCATION"]
    );
    assert!(names_to(&out, 3).is_empty());
    assert_eq!(decoded::<DeleteObject>(&out, 1, "DELETE_OBJECT")[0].object_id, 2);
    assert_eq!(decoded::<DeleteObject>(&out, 2, "DELETE_OBJECT")[0].object_id, 1);
    let stop = decoded::<StopMove>(&out, 2, "STOP_MOVE")[0];
    assert_eq!((stop.object_id, stop.x, stop.y, stop.z), (1, X, Y, Z));
    for to in [1, 2] {
        // Java adds 5 to Z; the heading is the argument.
        assert_eq!(
            decoded::<TeleportToLocation>(&out, to, "TELEPORT_TO_LOCATION")[0],
            TeleportToLocation {
                object_id: 1,
                x: DX,
                y: Y,
                z: Z + 5,
                heading: 1234
            }
        );
    }

    // Out of the world until the client answers, but online and at the new position.
    assert!(w.is_teleporting(ObjectId(1)));
    let entry = w.object(ObjectId(1)).unwrap();
    assert!(!entry.spawned);
    assert_eq!(w.position_of(ObjectId(1)), Some((DX, Y, Z + 5)));
    assert_eq!(w.player(ObjectId(1)).unwrap().core.heading, 1234);
    assert!(w.visible_players(ObjectId(2)).is_empty());

    // Appearing: spawnMe (the player sees the target's surroundings, they see the player),
    // zones, broadcastInfo, then UserInfo.
    appearing(&mut w, 1);
    let out = log.take();
    assert_eq!(
        names_to(&out, 1),
        [
            "CHAR_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "RELATION_CHANGED",
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO"
        ]
    );
    // The new neighbour hears about the player twice: Java's spawnMe and broadcastInfo.
    assert_eq!(
        names_to(&out, 3),
        [
            "CHAR_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "RELATION_CHANGED",
            "CHAR_INFO",
            "EX_BR_EXTRA_USER_INFO"
        ]
    );
    assert!(names_to(&out, 2).is_empty());
    assert!(!w.is_teleporting(ObjectId(1)));
    assert!(w.object(ObjectId(1)).unwrap().spawned);
    assert_eq!(w.visible_players(ObjectId(3)), [ObjectId(1)]);
}

#[test]
fn appearing_without_a_teleport_only_refreshes_user_info() {
    let log = Log::default();
    let mut w = world(&log, 1);
    enter(&mut w, &log, 1, X, Y, 0);
    log.take();
    appearing(&mut w, 1);
    assert_eq!(names_to(&log.take(), 1), ["USER_INFO", "EX_BR_EXTRA_USER_INFO"]);
}

#[test]
fn a_player_who_is_not_online_is_not_teleported() {
    let log = Log::default();
    let mut w = world(&log, 1);
    enter(&mut w, &log, 1, X, Y, 0);
    log.take();
    w.tele_to_location(ObjectId(99), TeleportTarget::at(DX, Y, Z));
    assert!(log.take().is_empty());
}

#[test]
fn random_offset_stays_within_bounds_and_is_reproducible() {
    let run = |seed: u64| {
        let log = Log::default();
        let mut w = world(&log, seed);
        enter(&mut w, &log, 1, X, Y, 0);
        w.tele_to_location(ObjectId(1), TeleportTarget::at(DX, Y, Z).with_random_offset(50));
        w.position_of(ObjectId(1)).unwrap()
    };
    let (x, y, z) = run(7);
    assert!((x - DX).abs() <= 50 && (y - Y).abs() <= 50, "{x} {y}");
    assert_eq!(z, Z + 5);
    // Java re-rolls an offset that lands closer than twice the collision radius (18 here).
    let dist = (f64::from(x - DX).powi(2) + f64::from(y - Y).powi(2)).sqrt();
    assert!(dist >= 18.0, "{dist}");
    assert_eq!((x, y, z), run(7));
    assert!((1..20).any(|seed| run(seed) != (x, y, z)));
}

#[test]
fn a_zero_offset_still_goes_through_the_offset_code_without_moving() {
    // OffsetOnTeleportEnabled is on by default; Rnd.get(0, 0) is 0 and draws nothing.
    let log = Log::default();
    let mut w = world(&log, 1);
    enter(&mut w, &log, 1, X, Y, 0);
    w.tele_to_location(ObjectId(1), TeleportTarget::at(DX, Y + 7, Z));
    assert_eq!(w.position_of(ObjectId(1)), Some((DX, Y + 7, Z + 5)));
}

#[test]
fn instance_zero_is_entered_and_negative_keeps_the_instance() {
    let log = Log::default();
    let mut w = world(&log, 1);
    enter(&mut w, &log, 1, X, Y, 0);
    w.tele_to_location(ObjectId(1), TeleportTarget::at(DX, Y, Z).with_instance(0));
    assert_eq!(w.object(ObjectId(1)).unwrap().instance_id, 0);
    w.tele_to_location(ObjectId(1), TeleportTarget::at(DX, Y, Z).with_instance(5));
    // No instance 5 exists: Java's setInstanceId returns without changing anything.
    assert_eq!(w.object(ObjectId(1)).unwrap().instance_id, 0);
}

#[test]
fn the_watchdog_completes_a_teleport_whose_client_never_answers() {
    let log = Log::default();
    let mut w = world(&log, 1);
    w.set_settings(WorldSettings {
        teleport: TeleportSettings {
            teleport_watchdog_timeout: 1,
            ..TeleportSettings::default()
        },
        ..WorldSettings::default()
    });
    enter(&mut w, &log, 1, X, Y, 0);
    w.tele_to_location(ObjectId(1), TeleportTarget::at(DX, Y, Z));
    log.take();
    // 1 s = 20 ticks.
    for _ in 0..19 {
        w.tick();
    }
    assert!(w.is_teleporting(ObjectId(1)));
    w.tick();
    assert!(!w.is_teleporting(ObjectId(1)));
    assert!(w.object(ObjectId(1)).unwrap().spawned);
    // No Appearing, so no UserInfo.
    assert!(!names_to(&log.take(), 1).contains(&"USER_INFO".to_string()));
}

#[test]
fn a_late_appearing_after_the_watchdog_changes_nothing() {
    let log = Log::default();
    let mut w = world(&log, 1);
    w.set_settings(WorldSettings {
        teleport: TeleportSettings {
            teleport_watchdog_timeout: 1,
            ..TeleportSettings::default()
        },
        ..WorldSettings::default()
    });
    enter(&mut w, &log, 1, X, Y, 0);
    w.tele_to_location(ObjectId(1), TeleportTarget::at(DX, Y, Z));
    for _ in 0..20 {
        w.tick();
    }
    log.take();
    appearing(&mut w, 1);
    assert_eq!(names_to(&log.take(), 1), ["USER_INFO", "EX_BR_EXTRA_USER_INFO"]);
}

#[test]
fn creature_set_xyz_refuses_zero_and_positions_outside_the_zone_grid() {
    let log = Log::default();
    let mut w = world(&log, 1);
    enter(&mut w, &log, 1, X, Y, 0);
    // "0, 0 is not a valid location."
    w.set_position(ObjectId(1), 0, 0, Z);
    assert_eq!(w.position_of(ObjectId(1)), Some((X, Y, Z)));
    // Beyond the last zone cell ("Prevent moving to nonexistent regions").
    w.set_position(ObjectId(1), 300_000, Y, Z);
    assert_eq!(w.position_of(ObjectId(1)), Some((X, Y, Z)));
    w.set_position(ObjectId(1), X + 10, Y, Z);
    assert_eq!(w.position_of(ObjectId(1)), Some((X + 10, Y, Z)));
}

// ------------------------------------------------------------------------------ zones

#[test]
fn entering_the_world_in_a_peace_zone_sets_flags_and_the_compass() {
    let log = Log::default();
    let mut w = world_with_zones(&log, "enter", &test_zones());
    enter(&mut w, &log, 1, 1_000, 1_000, 0);
    let out = log.take();
    assert_eq!(compass_to(&out, 1), [12]);
    // A human is not an elf: no mother tree message.
    assert!(messages_to(&out, 1).iter().all(|id| *id != 114));
    assert!(w.is_inside_zone(ObjectId(1), ZoneId::Peace));
    assert!(w.is_inside_zone(ObjectId(1), ZoneId::Town));
    assert!(!w.is_inside_zone(ObjectId(1), ZoneId::MotherTree));
    assert!(!w.is_inside_zone(ObjectId(1), ZoneId::Pvp));
    // The compass goes out where Java's spawnMe runs: after the NPCs and players around, before
    // ExRotation.
    let names = names_to(&out, 1);
    let compass = names
        .iter()
        .position(|n| n == "EX_SET_COMPASS_ZONE_CODE")
        .unwrap();
    assert_eq!(names[compass + 1], "EX_ROTATION");
    assert_eq!(names[compass - 1], "QUEST_LIST");

    // An elf also gets the mother tree message, before the compass (zone order, then compass).
    enter(&mut w, &log, 2, 1_000, 1_000, 1);
    let out = log.take();
    assert_eq!(messages_to(&out, 2).iter().filter(|id| **id == 114).count(), 1);
    assert_eq!(compass_to(&out, 2), [12]);
    assert!(w.is_inside_zone(ObjectId(2), ZoneId::MotherTree));
    assert_eq!(
        w.player(ObjectId(2)).unwrap().zones.members().len(),
        3,
        "peace, town, mother tree; the disabled zone is not entered"
    );
}

#[test]
fn walking_through_zones_updates_flags_messages_and_compass_once() {
    let log = Log::default();
    let mut w = world_with_zones(&log, "walk", &test_zones());
    enter(&mut w, &log, 2, 1_000, 1_000, 1);
    log.take();
    let id = ObjectId(2);

    // Moving less than 100 units from the last check does not look at the zones.
    w.set_position(id, 1_050, 1_000, Z);
    assert!(log.take().is_empty());

    // Out of the peace area: the elf leaves the mother tree (message 115), the icon goes
    // general. Staying out sends nothing more.
    w.set_position(id, 3_000, 1_000, Z);
    let out = log.take();
    assert_eq!(messages_to(&out, 2), [115]);
    assert_eq!(compass_to(&out, 2), [15]);
    assert!(!w.is_inside_zone(id, ZoneId::Peace));
    assert!(!w.is_inside_zone(id, ZoneId::Town));
    assert!(!w.is_inside_zone(id, ZoneId::MotherTree));
    w.set_position(id, 3_500, 1_000, Z);
    assert!(log.take().is_empty());

    // Arena: combat zone message, PvP icon.
    w.set_position(id, 5_500, 1_500, Z);
    let out = log.take();
    assert_eq!(messages_to(&out, 2), [283]);
    assert_eq!(compass_to(&out, 2), [14]);
    assert!(w.is_inside_zone(id, ZoneId::Pvp));

    // Effect zone: the altered icon and the danger icon in EtcStatusUpdate; the arena says
    // goodbye.
    w.set_position(id, 8_500, 1_500, Z);
    let out = log.take();
    assert_eq!(messages_to(&out, 2), [284]);
    assert_eq!(compass_to(&out, 2), [8]);
    let etc = decoded::<EtcStatusUpdate>(&out, 2, "ETC_STATUS_UPDATE");
    assert_eq!(etc.len(), 1);
    assert!(etc[0].danger_area);
    assert!(w.is_inside_zone(id, ZoneId::Altered));
    assert!(w.is_inside_zone(id, ZoneId::DangerArea));
    assert!(!w.is_inside_zone(id, ZoneId::Pvp));

    // Out of it: the danger icon is cleared.
    w.set_position(id, 10_000, 1_500, Z);
    let out = log.take();
    let etc = decoded::<EtcStatusUpdate>(&out, 2, "ETC_STATUS_UPDATE");
    assert_eq!(etc.len(), 1);
    assert!(!etc[0].danger_area);
    assert_eq!(compass_to(&out, 2), [15]);

    // A zone that is disabled by default is never entered.
    w.set_position(id, 11_500, 1_500, Z);
    assert!(compass_to(&log.take(), 2).is_empty());
    assert!(!w.is_inside_zone(id, ZoneId::Peace));

    // A condition zone sets its flag only.
    w.set_position(id, 14_500, 1_500, Z);
    assert!(log.take().is_empty());
    assert!(w.is_inside_zone(id, ZoneId::NoBookmark));
    assert!(!w.is_inside_zone(id, ZoneId::NoItemDrop));
}

#[test]
fn pvp_does_not_count_inside_a_peace_zone() {
    // Java isInsideZone(PVP): the PvP counter only counts with no peace and no no-PvP zone.
    let log = Log::default();
    let xml = [
        cuboid(1, "PeaceZone", 100, 100, 2_000, 2_000, ""),
        cuboid(2, "ArenaZone", 100, 100, 2_000, 2_000, ""),
    ]
    .concat();
    let mut w = world_with_zones(&log, "pvp", &xml);
    enter(&mut w, &log, 1, 1_000, 1_000, 0);
    assert!(w.is_inside_zone(ObjectId(1), ZoneId::Peace));
    assert!(!w.is_inside_zone(ObjectId(1), ZoneId::Pvp));
    // The icon follows: peace, not PvP.
    assert_eq!(compass_to(&log.take(), 1), [12]);
}

#[test]
fn a_zone_that_does_not_admit_players_or_this_level_is_not_entered() {
    let log = Log::default();
    let xml = [
        cuboid(
            1,
            "PeaceZone",
            100,
            100,
            2_000,
            2_000,
            "<stat name=\"targetClass\" val=\"Npc\"/>",
        ),
        cuboid(
            2,
            "TownZone",
            100,
            100,
            2_000,
            2_000,
            "<stat name=\"affectedLvlMin\" val=\"10\"/>",
        ),
    ]
    .concat();
    let mut w = world_with_zones(&log, "rules", &xml);
    enter(&mut w, &log, 1, 1_000, 1_000, 0);
    assert!(w.player(ObjectId(1)).unwrap().zones.members().is_empty());
    assert_eq!(compass_to(&log.take(), 1), [15]);
}

#[test]
fn teleporting_out_of_a_zone_cell_leaves_its_zones_at_once_and_enters_the_new_ones_on_appearing() {
    let log = Log::default();
    let mut w = world_with_zones(&log, "cells", &test_zones());
    enter(&mut w, &log, 1, 1_000, 1_000, 0);
    log.take();
    let id = ObjectId(1);

    // The target is in another zone cell: Creature.setXYZ drops the old cell's zones, and the
    // revalidation at the end of teleToLocation sends the icon (the new zone waits for the
    // teleport to complete).
    w.tele_to_location(id, TeleportTarget::at(100_500, 1_000, Z));
    let out = log.take();
    assert_eq!(
        names_to(&out, 1),
        ["STOP_MOVE", "TELEPORT_TO_LOCATION", "EX_SET_COMPASS_ZONE_CODE"]
    );
    assert_eq!(compass_to(&out, 1), [15]);
    assert!(!w.is_inside_zone(id, ZoneId::Peace));
    assert!(!w.is_inside_zone(id, ZoneId::Town));
    assert!(w.player(ObjectId(1)).unwrap().zones.members().is_empty());

    appearing(&mut w, 1);
    let out = log.take();
    assert_eq!(compass_to(&out, 1), [12]);
    assert!(w.is_inside_zone(id, ZoneId::Peace));
    assert!(!w.is_inside_zone(id, ZoneId::Town));
}

#[test]
fn teleporting_inside_a_zone_cell_changes_the_zones_when_the_client_appears() {
    let log = Log::default();
    let mut w = world_with_zones(&log, "samecell", &test_zones());
    enter(&mut w, &log, 1, 1_000, 1_000, 0);
    log.take();
    let id = ObjectId(1);

    // Same cell: the player stays in the zones of the old position until Appearing.
    w.tele_to_location(id, TeleportTarget::at(5_500, 1_500, Z));
    let out = log.take();
    assert!(compass_to(&out, 1).is_empty());
    assert!(w.is_inside_zone(id, ZoneId::Peace));

    appearing(&mut w, 1);
    let out = log.take();
    assert_eq!(compass_to(&out, 1), [14]);
    assert_eq!(messages_to(&out, 1), [283]);
    assert!(!w.is_inside_zone(id, ZoneId::Peace));
    assert!(w.is_inside_zone(id, ZoneId::Pvp));
}

#[test]
fn without_zone_data_there_are_no_zone_packets() {
    let log = Log::default();
    let mut w = world(&log, 1);
    enter(&mut w, &log, 1, 1_000, 1_000, 0);
    w.set_position(ObjectId(1), 5_000, 1_000, Z);
    assert!(compass_to(&log.take(), 1).is_empty());
}

// ------------------------------------------------------------ real data and Java captures

fn datapack_root() -> Option<PathBuf> {
    let root = PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game");
    if root.join("data").join("zones").is_dir() {
        Some(root)
    } else {
        eprintln!("skipped: {} has no data/zones", root.display());
        None
    }
}

fn capture_records(name: &str) -> Option<Vec<Vec<u8>>> {
    let path = std::env::var_os("L2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"))
        .join("java-gs-session.jsonl");
    let text = std::fs::read_to_string(&path).ok()?;
    let records: Vec<Vec<u8>> = text
        .lines()
        .skip(1)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|v| v["channel"] == "game" && v["dir"] == "s2c" && v["name"] == name)
        .filter_map(|v| v["raw"].as_str().map(|raw| hex::decode(raw).unwrap()))
        .collect();
    Some(records)
}

#[test]
fn entering_at_the_elf_start_gives_what_the_java_server_sent() {
    let Some(root) = datapack_root() else {
        return;
    };
    let Some(compass) = capture_records("EX_SET_COMPASS_ZONE_CODE") else {
        eprintln!("skipped: no Java capture");
        return;
    };
    let messages = capture_records("SYSTEM_MESSAGE").unwrap();
    // The recording: the elf of the capture stands at 45633, 41465, -3534 in the mother tree
    // zone of the start area; Java sends the mother tree message, then the peace icon.
    let java_message = &messages[0];
    assert_eq!(compass.len(), 1);

    let log = Log::default();
    let mut w = world(&log, 1);
    w.set_zone_data(Arc::new(Zones::load(&root)), Arc::new(MapRegions::load(&root)));
    let mut data = player(1, 45_633, 41_465, 1);
    data.z = -3_534;
    w.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(data),
        sink: Box::new(Sink(1, log.clone())),
        ip: None,
    });
    let out: Vec<Vec<u8>> = log.take().into_iter().map(|(_, b)| b).collect();
    let at = out
        .iter()
        .position(|b| b == java_message)
        .expect("mother tree message");
    assert_eq!(
        out[at + 1],
        compass[0],
        "the compass icon follows the zone message"
    );
    assert_eq!(name_of(&out[at + 2]), "EX_ROTATION");
}

#[test]
fn restart_points_come_from_the_map_regions() {
    let Some(root) = datapack_root() else {
        return;
    };
    let regions = Arc::new(MapRegions::load(&root));
    let ti = regions
        .get("talking_island_town")
        .expect("Talking Island region")
        .clone();
    assert!(!ti.spawn_locs.is_empty());

    let log = Log::default();
    let mut w = world(&log, 1);
    w.set_zone_data(Arc::new(Zones::load(&root)), regions);
    // Talking Island village.
    enter(&mut w, &log, 1, -84_318, 244_579, 0);
    let (x, y, z) = w.nearest_town_respawn(ObjectId(1)).unwrap();
    assert!(
        ti.spawn_locs.iter().any(|l| (l.x, l.y, l.z) == (x, y, z)),
        "{x} {y} {z}"
    );

    // Without RandomRespawnInTownEnabled it is always the first point.
    w.set_settings(WorldSettings {
        teleport: TeleportSettings {
            random_respawn_in_town_enabled: false,
            ..TeleportSettings::default()
        },
        ..WorldSettings::default()
    });
    let first = &ti.spawn_locs[0];
    assert_eq!(
        w.nearest_town_respawn(ObjectId(1)),
        Some((first.x, first.y, first.z))
    );

    // teleToLocation(TeleportWhereType.TOWN): the restart point with the random offset.
    w.tele_to_location_where(ObjectId(1), TeleportWhereType::Town);
    let (px, py, pz) = w.position_of(ObjectId(1)).unwrap();
    assert!(
        (px - first.x).abs() <= 50 && (py - first.y).abs() <= 50,
        "{px} {py}"
    );
    assert_eq!(pz, first.z + 5);
    assert!(w.is_teleporting(ObjectId(1)));
}
