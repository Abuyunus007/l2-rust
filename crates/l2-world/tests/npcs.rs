#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! NPC spawning and `NpcInfo`: synthetic spawn files for the counting rules, the real datapack
//! and the recorded Java session (both skipped when the reference folders are missing).
//! Java: data/xml/SpawnData.java, entity/spawns/*.java, network/serverpackets/AbstractNpcInfo.java.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use l2_data::{NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{DeleteObject, NpcInfo};
use l2_world::*;

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Vec<(i32, Vec<u8>)> {
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

/// `statBonus.xml` for the tests that need the real multipliers.
#[derive(Default)]
struct Bonus(HashMap<(BaseStat, i32), f64>);

impl Bonus {
    fn load(root: &Path) -> Self {
        let text = std::fs::read_to_string(root.join("data/stats/statBonus.xml")).unwrap();
        let mut table = HashMap::new();
        let mut section = None;
        for line in text.lines() {
            let line = line.trim();
            section = match line {
                "<STR>" => Some(BaseStat::Str),
                "<INT>" => Some(BaseStat::Int),
                "<DEX>" => Some(BaseStat::Dex),
                "<WIT>" => Some(BaseStat::Wit),
                "<CON>" => Some(BaseStat::Con),
                "<MEN>" => Some(BaseStat::Men),
                _ => section,
            };
            if let (Some(stat), Some(rest)) = (section, line.strip_prefix("<stat value=\"")) {
                let (value, rest) = rest.split_once('"').unwrap();
                let bonus = rest.split("bonus=\"").nth(1).unwrap().split('"').next().unwrap();
                table.insert((stat, value.parse().unwrap()), bonus.parse().unwrap());
            }
        }
        Self(table)
    }
}

impl StaticData for Bonus {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn derive_stats(&self, _player: &PlayerData) -> DerivedStats {
        DerivedStats::default()
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn base_stat_bonus(&self, stat: BaseStat, value: i32) -> f64 {
        self.0.get(&(stat, value)).copied().unwrap_or(0.0)
    }
}

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

fn enter(world: &mut World, log: &Log, id: i32, x: i32, y: i32, z: i32) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(player(id, x, y, z)),
        sink: Box::new(Sink(id, log.clone())),
        ip: None,
    });
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode_npc_info(raw: &[u8]) -> NpcInfo {
    NpcInfo::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

fn new_world(log: &Log) -> World {
    let mut world = World::new(WorldRng::from_seed(7), Box::new(NullOutbox));
    let _ = log;
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world
}

// ------------------------------------------------------------------ synthetic spawn files

const TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="1000" type="Folk" level="10" name="Gatekeeper" usingServerSideName="true">
    <collision><radius normal="8"/><height normal="23"/></collision>
  </npc>
  <npc id="2000" type="Monster" level="20"><collision><radius normal="9"/><height normal="20"/></collision></npc>
  <npc id="2001" type="Monster" level="21"/>
  <npc id="3000" type="Pet" level="5"/>
  <npc id="4000" type="Monster" level="30"><parameters>
    <minions name="Privates"><npc id="2001" count="3" respawnTime="10"/></minions></parameters></npc>
  <npc id="5000" type="FriendlyMob" level="9"/>
</list>"#;

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

fn spawn_file(body: &str) -> String {
    format!(r#"<?xml version="1.0"?><list enabled="true">{body}</list>"#)
}

/// Writes the spawn files below a temporary datapack root and loads them.
fn load_spawns(files: &[(&str, String)]) -> (tempfile::TempDir, Arc<NpcTemplates>, Arc<SpawnData>) {
    let dir = tempfile::tempdir().unwrap();
    let spawns_dir = dir.path().join("data/spawns");
    std::fs::create_dir_all(&spawns_dir).unwrap();
    for (name, body) in files {
        std::fs::write(spawns_dir.join(name), body).unwrap();
    }
    let npcs = Arc::new(NpcTemplates::from_xml(TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
    (dir, npcs, spawns)
}

fn world_with(files: &[(&str, String)]) -> (World, Log, tempfile::TempDir) {
    let log = Log::default();
    let (dir, npcs, spawns) = load_spawns(files);
    let mut world = new_world(&log);
    world.set_npc_data(npcs, spawns);
    (world, log, dir)
}

#[test]
fn fixed_spawns_count_like_java_and_pets_are_skipped() {
    let body = spawn_file(&format!(
        r#"<spawn><npc id="1000" x="{X}" y="{Y}" z="{Z}" heading="1234"/></spawn>
           <spawn><npc id="2000" x="{X}" y="{Y}" z="{Z}" count="3" respawnDelay="30"/></spawn>
           <spawn><npc id="3000" x="{X}" y="{Y}" z="{Z}"/></spawn>
           <spawn><npc id="99999" x="{X}" y="{Y}" z="{Z}"/></spawn>"#
    ));
    let (mut world, _log, _dir) = world_with(&[("a.xml", body)]);
    let report = world.spawn_all_npcs(false);
    // 1 + 3; the Pet is dropped (Java's Spawn constructor throws), the unknown NPC never loads.
    assert_eq!(report.spawn_count, 4);
    assert_eq!(report.fixed, 4);
    assert_eq!(report.skipped_type, 1);
    assert_eq!(world.npc_count(), 4);
}

#[test]
fn a_fixed_spawn_keeps_its_place_and_heading() {
    let body = spawn_file(&format!(
        r#"<spawn><npc id="1000" x="{X}" y="{Y}" z="{Z}" heading="1234"/></spawn>"#
    ));
    let (mut world, _log, _dir) = world_with(&[("a.xml", body)]);
    world.spawn_all_npcs(false);
    let npc = world.npc(world.npc_ids()[0]).unwrap();
    assert_eq!(
        (npc.core.x, npc.core.y, npc.core.z, npc.core.heading),
        (X, Y, Z, 1234)
    );
    assert_eq!(npc.npc_id, 1000);
    assert_eq!(npc.core.level, 10);
}

#[test]
fn territory_spawns_land_inside_the_territory_with_a_random_heading() {
    let body = spawn_file(
        r#"<spawn zone="t1">
             <territory minZ="-3300" maxZ="-2900">
               <node x="-71500" y="258100"/><node x="-71200" y="258100"/>
               <node x="-71200" y="258500"/><node x="-71500" y="258500"/>
             </territory>
             <npc id="2000" count="20" heading="5"/>
           </spawn>"#,
    );
    let (mut world, _log, _dir) = world_with(&[("a.xml", body)]);
    let report = world.spawn_all_npcs(false);
    assert_eq!(report.spawn_count, 20);
    let mut headings = std::collections::HashSet::new();
    for id in world.npc_ids() {
        let n = world.npc(id).unwrap();
        assert!((-71_500..=-71_200).contains(&n.core.x), "x {}", n.core.x);
        assert!((258_100..=258_500).contains(&n.core.y), "y {}", n.core.y);
        assert!(n.core.heading < 61_794);
        headings.insert(n.core.heading);
    }
    // NOTE(java-quirk) kept: the XML heading is dropped for territory spawns.
    assert!(headings.len() > 10, "headings are random: {headings:?}");
}

#[test]
fn the_same_seed_gives_the_same_places() {
    let body = spawn_file(
        r#"<spawn zone="t1">
             <territory minZ="-3300" maxZ="-2900">
               <node x="-71500" y="258100"/><node x="-71200" y="258100"/>
               <node x="-71200" y="258500"/><node x="-71500" y="258500"/>
             </territory>
             <npc id="2000" count="10"/>
           </spawn>"#,
    );
    let places = |files: &[(&str, String)]| {
        let (mut world, _log, _dir) = world_with(files);
        world.spawn_all_npcs(false);
        let mut v: Vec<_> = world
            .npc_ids()
            .into_iter()
            .map(|id| {
                let n = world.npc(id).unwrap();
                (n.core.x, n.core.y, n.core.heading)
            })
            .collect();
        v.sort_unstable();
        v
    };
    assert_eq!(places(&[("a.xml", body.clone())]), places(&[("a.xml", body)]));
}

#[test]
fn group_selection_counts() {
    // all: every entry whole, but the group cap (maximumNpc) skips entries that no longer fit.
    let all = r#"<spawn zone="g1" selection="all" maximumNpc="4">
        <territory minZ="-3300" maxZ="-2900"><node x="-71500" y="258100"/><node x="-71200" y="258100"/>
          <node x="-71200" y="258500"/><node x="-71500" y="258500"/></territory>
        <npc id="2000" count="3"/><npc id="2001" count="3"/><npc id="1000" count="1"/></spawn>"#;
    // random: one entry rolled, whole quota, the cap is not read.
    let random = r#"<spawn zone="g1" selection="random" maximumNpc="1">
        <territory minZ="-3300" maxZ="-2900"><node x="-71500" y="258100"/><node x="-71200" y="258100"/>
          <node x="-71200" y="258500"/><node x="-71500" y="258500"/></territory>
        <npc id="2000" count="5"/><npc id="2001" count="5"/></spawn>"#;
    // randomFill: maximumNpc rolls, one unit each, a roll into a full entry is lost.
    let fill = r#"<spawn zone="g1" selection="randomFill" maximumNpc="6">
        <territory minZ="-3300" maxZ="-2900"><node x="-71500" y="258100"/><node x="-71200" y="258100"/>
          <node x="-71200" y="258500"/><node x="-71500" y="258500"/></territory>
        <npc id="2000" count="1"/><npc id="2001" count="1"/></spawn>"#;
    let count = |xml: &str| {
        let (mut world, _log, _dir) = world_with(&[("a.xml", spawn_file(xml))]);
        let report = world.spawn_all_npcs(false);
        assert_eq!(report.spawn_count, report.grouped);
        assert_eq!(world.npc_count(), report.grouped);
        report.grouped
    };
    // 3 fits (3 <= 4), 3 more would be 6 > 4, the single 1000 makes 4.
    assert_eq!(count(all), 4);
    assert_eq!(count(random), 5);
    let filled = count(fill);
    assert!(
        (1..=2).contains(&filled),
        "each of the two entries holds one unit: {filled}"
    );
}

#[test]
fn day_and_night_spawns_follow_the_period() {
    let body = spawn_file(&format!(
        r#"<spawn><npc id="1000" x="{X}" y="{Y}" z="{Z}" periodOfDay="day"/></spawn>
           <spawn><npc id="2000" x="{X}" y="{Y}" z="{Z}" periodOfDay="night"/></spawn>"#
    ));
    let (mut world, _log, _dir) = world_with(&[("a.xml", body)]);
    let report = world.spawn_all_npcs(false);
    // Java counts both declared spawns, but only the day one exists.
    assert_eq!(
        (report.spawn_count, report.day_night, report.day_night_created),
        (2, 2, 1)
    );
    assert_eq!(world.npc_count(), 1);
    assert_eq!(world.set_night(true), 1);
    assert_eq!(world.npc_count(), 1);
    let id = world.npc_ids()[0];
    assert_eq!(world.npc(id).unwrap().npc_id, 2000);
}

#[test]
fn master_monsters_bring_their_minions() {
    let body = spawn_file(&format!(
        r#"<spawn><npc id="4000" x="{X}" y="{Y}" z="{Z}" heading="77"/></spawn>"#
    ));
    let (mut world, _log, _dir) = world_with(&[("a.xml", body)]);
    let report = world.spawn_all_npcs(false);
    assert_eq!(report.spawn_count, 1, "minions are not in Java's count");
    assert_eq!((report.minions, world.npc_count()), (3, 4));
    let master = world
        .npc_ids()
        .into_iter()
        .find(|id| world.npc(*id).unwrap().npc_id == 4000)
        .unwrap();
    for id in world.npc_ids() {
        let n = world.npc(id).unwrap();
        if id != master {
            assert_eq!((n.npc_id, n.leader, n.core.heading), (2001, Some(master), 77));
            let (dx, dy) = (n.core.x - X, n.core.y - Y);
            assert!(dx.abs() <= 500 && dy.abs() <= 500, "{dx} {dy}");
        }
    }
    assert!(world.delete_npc(master));
    assert_eq!(world.npc_count(), 0, "minions go with their master");
}

#[test]
fn players_get_npc_info_when_they_arrive_and_delete_object_when_it_goes() {
    let body = spawn_file(&format!(
        r#"<spawn><npc id="1000" x="{X}" y="{Y}" z="{Z}"/></spawn>
           <spawn><npc id="2000" x="{X}" y="{Y}" z="{Z}"/></spawn>
           <spawn><npc id="5000" x="{X}" y="{Y}" z="{Z}"/></spawn>
           <spawn><npc id="1000" x="{}" y="{Y}" z="{Z}"/></spawn>"#,
        X + 200_000 - 100_000
    ));
    let (mut world, log, _dir) = world_with(&[("a.xml", body)]);
    world.spawn_all_npcs(false);
    // Nobody around: nothing is sent while spawning.
    assert!(log.take().is_empty());

    enter(&mut world, &log, 10, X + 10, Y, Z);
    let infos: Vec<NpcInfo> = log
        .take()
        .into_iter()
        .filter(|(_, b)| name_of(b) == "NPC_INFO")
        .map(|(_, b)| decode_npc_info(&b))
        .collect();
    // The far one is in another region and is not told.
    assert_eq!(infos.len(), 3);
    let folk = infos.iter().find(|i| i.name == "Gatekeeper").unwrap();
    assert!(!folk.attackable && folk.show_name && folk.targetable);
    assert_eq!((folk.x, folk.y, folk.z), (X, Y, Z));
    let monster = infos.iter().find(|i| i.collision_height == 20.0).unwrap();
    assert!(monster.attackable, "monsters are auto attackable for players");
    assert_eq!(monster.collision_radius, 9.0);

    let id = world
        .npc_ids()
        .into_iter()
        .find(|id| world.npc(*id).unwrap().npc_id == 2000)
        .unwrap();
    assert!(world.delete_npc(id));
    let out = log.take();
    assert_eq!(out.len(), 1);
    assert_eq!(name_of(&out[0].1), "DELETE_OBJECT");
    assert_eq!(
        DeleteObject::decode(&mut PacketReader::new(&out[0].1[1..]))
            .unwrap()
            .object_id,
        id.0
    );
    assert!(!world.delete_npc(id), "already gone");
}

#[test]
fn a_new_npc_next_to_a_player_is_announced_at_once() {
    let (mut world, log, _dir) = world_with(&[("a.xml", spawn_file(""))]);
    world.spawn_all_npcs(false);
    enter(&mut world, &log, 10, X, Y, Z);
    log.take();
    let id = world.spawn_npc_at(1000, X + 5, Y, Z, 99).unwrap();
    let out = log.take();
    assert_eq!(out.len(), 1);
    let info = decode_npc_info(&out[0].1);
    assert_eq!((info.object_id, info.x, info.heading), (id.0, X + 5, 99));
}

#[test]
fn friendly_mobs_are_attackable_for_players_with_karma_only() {
    let (mut world, log, _dir) = world_with(&[("a.xml", spawn_file(""))]);
    enter(&mut world, &log, 10, X, Y, Z);
    let id = world.spawn_npc_at(5000, X + 5, Y, Z, 0).unwrap();
    assert!(!decode_npc_info(&world.npc_info_bytes(id, ObjectId(10)).unwrap()).attackable);
    world.player_mut(ObjectId(10)).unwrap().data.karma = 5;
    assert!(decode_npc_info(&world.npc_info_bytes(id, ObjectId(10)).unwrap()).attackable);
}

// ------------------------------------------------------------------ the reference folders

const REFERENCE: &str = r"C:\l2-rust\l2jmobius\dist\game";

fn reference_root() -> Option<PathBuf> {
    let root = PathBuf::from(REFERENCE);
    if root.join("data/stats/npcs").is_dir() && root.join("data/spawns").is_dir() {
        Some(root)
    } else {
        eprintln!("skipped: {REFERENCE} is not available");
        None
    }
}

#[test]
fn the_real_spawn_files_spawn_and_report_their_numbers() {
    let Some(root) = reference_root() else { return };
    let started = Instant::now();
    let npcs = Arc::new(NpcTemplates::load(&root, &NpcLoadOptions::default()));
    let spawns = Arc::new(SpawnData::load(&root, &npcs, &SpawnLoadOptions::default()));
    let loaded = started.elapsed();

    let mut world = World::new(WorldRng::from_seed(11), Box::new(NullOutbox));
    world.set_static_data(Box::new(Bonus::load(&root)));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_npc_data(Arc::clone(&npcs), Arc::clone(&spawns));
    let started = Instant::now();
    let report = world.spawn_all_npcs(false);
    let spawn_time = started.elapsed();
    eprintln!(
        "loaded in {loaded:?}, spawned in {spawn_time:?}: {report:?}; declared {} spawns, {} fixed planned",
        spawns.len(),
        spawns.planned_fixed_spawns()
    );

    // Every non-group spawn of a type Java can build is either started or a day/night spawn.
    let creatable_fixed = spawns
        .spawns()
        .iter()
        .filter(|s| s.group.is_none())
        .filter(|s| {
            npcs.get(s.npc_id)
                .is_some_and(|t| NpcClass::from_type(&t.npc_type).is_some())
        })
        .count();
    assert_eq!(report.fixed + report.day_night + report.failed, creatable_fixed);
    assert_eq!(
        report.fixed + report.day_night + report.grouped,
        report.spawn_count
    );
    assert_eq!(world.npc_count(), report.npcs);
    assert_eq!(
        world.npc_count(),
        report.fixed + report.grouped + report.day_night_created + report.minions
    );
}

// ------------------------------------------------------------------ the recorded Java session

/// The `NPC_INFO` packets of the recording.
fn recorded_npc_infos() -> Option<Vec<NpcInfo>> {
    let dir = std::env::var_os("L2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"));
    let text = std::fs::read_to_string(dir.join("java-gs-session.jsonl")).ok()?;
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        if v["channel"] == "game" && v["dir"] == "s2c" && v["name"] == "NPC_INFO" {
            out.push(decode_npc_info(&hex::decode(v["raw"].as_str()?).unwrap()));
        }
    }
    Some(out)
}

/// What the world sends for the NPCs of the recording equals what Java sent, field by field.
///
/// Compared: everything but the object id (Java's ids come from its own range). The NPC's place
/// and heading are the recorded ones (random territory points and headings cannot be compared);
/// the template, the speeds, the attack speed multiplier, the collision and the flags come out
/// of the datapack and the stat bonus table. Packets that were re-sent for an NPC that moved or
/// attacked (running / in combat set, position changed) are only compared where the recorded NPC
/// stood still.
#[test]
fn npc_info_equals_the_java_session() {
    let Some(root) = reference_root() else { return };
    let Some(recorded) = recorded_npc_infos() else {
        eprintln!("skipped: no recording");
        return;
    };
    let npcs = Arc::new(NpcTemplates::load(&root, &NpcLoadOptions::default()));
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(3), Box::new(NullOutbox));
    world.set_static_data(Box::new(Bonus::load(&root)));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_npc_data(Arc::clone(&npcs), Arc::new(SpawnData::default()));

    let (mut compared, mut skipped, mut skill_diffs) = (0, 0, 0);
    let mut seen = std::collections::HashSet::new();
    for (n, rec) in recorded.iter().enumerate() {
        // A moving or fighting NPC is announced again with its state; the first sight of each
        // object is its spawn state.
        if !seen.insert(rec.object_id) {
            continue;
        }
        let Some(template) = npcs.get(rec.display_id) else {
            skipped += 1;
            continue;
        };
        if template.display_id != rec.display_id {
            skipped += 1;
            continue;
        }
        let viewer = 9_000 + i32::try_from(n).unwrap();
        enter(&mut world, &log, viewer, rec.x, rec.y, rec.z);
        log.take();
        let id = world
            .spawn_npc_at(template.id, rec.x, rec.y, rec.z, rec.heading)
            .unwrap();
        let sent = log.take();
        assert_eq!(sent.len(), 1, "one NpcInfo for the viewer");
        let mut mine = decode_npc_info(&sent[0].1);
        assert_eq!(mine.object_id, id.0);
        mine.object_id = rec.object_id;
        compared += 1;
        if mine != *rec {
            let mut allowed = mine.clone();
            // Passive skills of the template change the attack and move speeds (stat system,
            // M4); the recorded values are Java's, ours are the template times the bonus.
            if !template.skills.is_empty() {
                allowed.p_atk_spd = rec.p_atk_spd;
                allowed.m_atk_spd = rec.m_atk_spd;
                allowed.attack_speed_multiplier = rec.attack_speed_multiplier;
                allowed.run_speed = rec.run_speed;
                allowed.walk_speed = rec.walk_speed;
                allowed.move_multiplier = rec.move_multiplier;
            }
            assert_eq!(
                allowed, *rec,
                "NPC {} (display id {}) differs beyond the passive skills",
                template.id, rec.display_id
            );
            skill_diffs += 1;
            eprintln!(
                "NPC {} has skills: Java {} / {} / {:.4}, ours {} / {} / {:.4} (pAtkSpd, mAtkSpd, attack speed multiplier)",
                template.id,
                rec.p_atk_spd,
                rec.m_atk_spd,
                rec.attack_speed_multiplier,
                mine.p_atk_spd,
                mine.m_atk_spd,
                mine.attack_speed_multiplier
            );
        }
        world.delete_npc(id);
        world.handle(WorldCommand::Disconnected {
            session: SessionId(u64::try_from(viewer).unwrap()),
        });
    }
    eprintln!(
        "{compared} NpcInfo packets compared, {skill_diffs} differ only by skills, {skipped} skipped (no template with that display id)"
    );
    assert!(compared > 20, "only {compared} compared");
}
