#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loads the real Java zone and map region files and checks them against the real Java
//! `ZoneManager` / `MapRegionData`. Skipped when the folder does not exist (CI).
//!
//! The folder defaults to `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.
//!
//! The expected numbers come from running the unmodified Java classes (`ZoneManager.getInstance()`,
//! `MapRegionData.getInstance()`, JDK 25) over the same files: Java's own log line is
//! `Loaded 32 zone classes and 2860 zones.` followed by `Last static id 200224.`, and
//! `Loaded 57 map regions.`. The "oracle" tests below rebuild a text dump that Java produced
//! (membership of all 17 x 18 zone grid cells in iteration order, 40000 point queries, 60000
//! rectangle queries, the map tile index) and compare its FNV-1a hash with the hash of Java's
//! output, so a single changed answer anywhere fails the test.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;

use l2_data::map_region::MapRegions;
use l2_data::zone::{Zones, first_spawn};
use l2_data::{Location, Zone};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    if dir.join("data").join("zones").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The zones of the grid, one per id, by id (what Java's oracle collected from its cells).
fn unique_zones(zones: &Zones) -> Vec<&Zone> {
    let mut all = BTreeMap::new();
    for x in 0..17 {
        for y in 0..18 {
            for zone in zones.region_zones((x - 9) << 15, (y - 8) << 15) {
                all.entry(zone.id).or_insert(zone);
            }
        }
    }
    all.into_values().collect()
}

/// `zone type#id` for each zone at a point, in Java's order.
fn zones_at_text(zones: &Zones, x: i32, y: i32, z: i32) -> String {
    zones
        .zones_at(x, y, z)
        .iter()
        .map(|zone| format!("{}#{}", zone.zone_type, zone.id))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn loads_every_zone_with_javas_counts() {
    let Some(dir) = game_dir() else { return };
    let zones = Zones::load(&dir);
    assert_eq!(zones.len(), 2860);
    assert_eq!(zones.class_count(), 32, "31 registered classes plus NoPvPZone");
    assert_eq!(zones.last_static_id(), Some(200_224));
    // Java's `_classZones` sizes. TaxZone, SSQZone & co. are not Java classes and never load;
    // the `enabled="false"` files (tax, territory war) add nothing either.
    let expected = [
        ("ArenaZone", 17),
        ("BossZone", 8),
        ("CastleZone", 9),
        ("ClanHallZone", 43),
        ("ConditionZone", 1081),
        ("DamageZone", 36),
        ("DerbyTrackZone", 8),
        ("EffectZone", 220),
        ("FishingZone", 43),
        ("FlagPvPZone", 0),
        ("FortZone", 21),
        ("HqZone", 61),
        ("JailZone", 3),
        ("LandingZone", 69),
        ("MotherTreeZone", 16),
        ("NoLandingZone", 9),
        ("NoPvPZone", 16),
        ("NoRestartZone", 80),
        ("NoStoreZone", 28),
        ("NoSummonFriendZone", 29),
        ("OlympiadStadiumZone", 160),
        ("PeaceZone", 121),
        ("ResidenceHallTeleportZone", 12),
        ("ResidenceTeleportZone", 9),
        ("ResidenceZone", 0),
        ("RespawnZone", 147),
        ("ScriptZone", 83),
        ("SiegableHallZone", 6),
        ("SiegeZone", 36),
        ("SwampZone", 20),
        ("TownZone", 22),
        ("WaterZone", 447),
    ];
    for (zone_type, count) in expected {
        assert_eq!(zones.all_of_type(zone_type).len(), count, "{zone_type}");
    }
    assert_eq!(expected.iter().map(|e| e.1).sum::<usize>(), 2860);
    assert_eq!(
        zones.spawn_territory_count(),
        0,
        "no NpcSpawnTerritory in data/zones"
    );
}

#[test]
fn town_zones_keep_their_raw_parameters() {
    let Some(dir) = game_dir() else { return };
    let zones = Zones::load(&dir);
    let town = zones.get(11022).unwrap();
    assert_eq!(town.zone_type, "TownZone");
    assert!(town.int_param("townId").is_some());
    assert!(town.int_param("taxById").is_some());
    // Respawn zones carry race to restart point pairs.
    let race_zone = zones.get_by_name("monster_race_0").unwrap();
    assert_eq!(race_zone.zone_type, "RespawnZone");
    assert_eq!(race_zone.respawn_point("HUMAN"), Some("monster_race"));
    assert_eq!(race_zone.respawn_point("KAMAEL"), Some("monster_race"));
    assert_eq!(race_zone.respawn_point("BEAST"), None);
    // Spawn points of a castle zone, with the chaotic/other/banish lists.
    let castle = &zones.all_of_type("CastleZone");
    assert!(castle.iter().all(|z| !z.spawns.spawns.is_empty()));
    assert!(castle.iter().any(|z| !z.spawns.banish.is_empty()));
}

#[test]
fn towns_contain_their_restart_points_in_javas_order() {
    let Some(dir) = game_dir() else { return };
    let zones = Zones::load(&dir);
    // (region, first normal restart point, Java's `getZones` answer there).
    let cases = [
        (
            (-83990, 243336, -3700),
            "FishingZone#300357 TownZone#11022 PeaceZone#301860",
        ),
        (
            (45487, 49767, -2950),
            "PeaceZone#301865 TownZone#11023 MotherTreeZone#300341 FishingZone#300370",
        ),
        (
            (12501, 16768, -4500),
            "FishingZone#300373 PeaceZone#301864 TownZone#11021",
        ),
        ((-44316, -113136, -80), "PeaceZone#301893"),
        (
            (116176, -182448, -1400),
            "TownZone#11026 FishingZone#300360 PeaceZone#301897",
        ),
        (
            (82480, 149087, -3350),
            "TownZone#11020 PeaceZone#301883 FishingZone#300362",
        ),
        (
            (146494, 30584, -2420),
            "TownZone#11031 PeaceZone#301905 PeaceZone#301906",
        ),
        ((38096, -46976, 899), "PeaceZone#301937 TownZone#11033"),
        (
            (-83520, 150560, -3000),
            "TownZone#11025 FishingZone#300379 PeaceZone#301869",
        ),
        (
            (19025, 145245, -3107),
            "TownZone#11028 FishingZone#300369 PeaceZone#301876 PeaceZone#301877",
        ),
        ((-118092, 46955, 342), "PeaceZone#301954 TownZone#11038"),
        (
            (107568, 218256, -3624),
            "TownZone#11034 FishingZone#300361 PeaceZone#301932",
        ),
        ((148560, -55904, -2779), "TownZone#11032 PeaceZone#301935"),
        ((88240, -142736, -1341), "PeaceZone#301952 TownZone#11035"),
        ((80304, 56241, -1500), "PeaceZone#301900 TownZone#11029"),
        (
            (-14288, 122752, -3000),
            "FishingZone#300374 TownZone#11027 PeaceZone#301866 PeaceZone#301867",
        ),
        ((116127, 76015, -2729), "PeaceZone#301902 TownZone#11030"),
    ];
    for ((x, y, z), expected) in cases {
        assert_eq!(zones_at_text(&zones, x, y, z), expected, "at {x},{y},{z}");
        assert!(
            zones.zone_at(x, y, z, "PeaceZone").is_some(),
            "every town restart point is in a peace zone"
        );
    }
    // Outside the world, below the Z band and in the middle of nowhere.
    assert_eq!(zones_at_text(&zones, 0, 0, 0), "");
    assert_eq!(
        zones_at_text(&zones, -83990, 243336, -9000),
        "",
        "far below Talking Island"
    );
    assert_eq!(zones_at_text(&zones, i32::MAX, i32::MIN, 0), "");
    // `isInsideZone(x, y)` tests at the zone's top Z, so a point can be inside for (x, y) only.
    let town = zones.get(11022).unwrap();
    assert!(town.is_inside_zone_xy(-83990, 243336));
    assert!(!town.is_inside_zone(-83990, 243336, 100_000));
}

/// Java's grid: which zones sit in each of the 17 x 18 world cells, in `ConcurrentHashMap` order.
fn grid_dump(zones: &Zones) -> String {
    let mut s = String::new();
    writeln!(s, "size {}", zones.len()).unwrap();
    for x in 0..17 {
        for y in 0..18 {
            write!(s, "cell {x} {y}:").unwrap();
            for zone in zones.region_zones((x - 9) << 15, (y - 8) << 15) {
                write!(s, " {}", zone.id).unwrap();
            }
            writeln!(s).unwrap();
        }
    }
    s
}

#[test]
fn oracle_zone_grid_membership_and_order() {
    let Some(dir) = game_dir() else { return };
    let zones = Zones::load(&dir);
    assert_eq!(fnv1a(&grid_dump(&zones)), 0x06f5_2be0_6e9c_971f);
}

/// Java's own pseudo-random generator for the point and rectangle queries (an LCG, not `Rnd`).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

#[test]
fn oracle_point_queries() {
    let Some(dir) = game_dir() else { return };
    let zones = Zones::load(&dir);
    let list = unique_zones(&zones);
    let mut s = String::new();
    writeln!(s, "zones {}", list.len()).unwrap();
    for z in &list {
        let c = z.form.center_point();
        let d = z.distance_to_zone(c.x + 500, c.y - 300);
        writeln!(
            s,
            "z {} {} {} {} {} {} {} {} {}",
            z.id,
            z.zone_type,
            c.x,
            c.y,
            c.z,
            z.form.low_z(),
            z.form.high_z(),
            d.to_bits() as i64,
            z.is_inside_zone_xy(c.x, c.y)
        )
        .unwrap();
    }
    let mut rng = Lcg(987_654_321);
    for _ in 0..40_000 {
        let z = list[(rng.next() % list.len() as u64) as usize];
        let c = z.form.center_point();
        let dx = (rng.next() % 6001) as i32 - 3000;
        let dy = (rng.next() % 6001) as i32 - 3000;
        let lo = z.form.low_z().min(z.form.high_z());
        let hi = z.form.low_z().max(z.form.high_z());
        let pz = lo - 20 + (rng.next() % (hi - lo + 41) as u64) as i32;
        let (px, py) = (c.x + dx, c.y + dy);
        write!(s, "p {px} {py} {pz}:").unwrap();
        for q in zones.zones_at(px, py, pz) {
            write!(s, " {}", q.id).unwrap();
        }
        write!(s, " |").unwrap();
        for q in zones.zones_at_xy(px, py) {
            write!(s, " {}", q.id).unwrap();
        }
        writeln!(s).unwrap();
    }
    assert_eq!(list.len(), 2860);
    assert_eq!(fnv1a(&s), 0x9cf4_9162_14f5_aaca);
}

#[test]
fn oracle_rectangle_intersections() {
    let Some(dir) = game_dir() else { return };
    let zones = Zones::load(&dir);
    let list = unique_zones(&zones);
    let mut s = String::new();
    let mut rng = Lcg(1_122_334_455);
    let mut hits = 0;
    for _ in 0..60_000 {
        let z = list[(rng.next() % list.len() as u64) as usize];
        let c = z.form.center_point();
        let cx = c.x + (rng.next() % 4001) as i32 - 2000;
        let cy = c.y + (rng.next() % 4001) as i32 - 2000;
        let mut w = (rng.next() % 1200) as i32;
        let h = (rng.next() % 1200) as i32;
        if rng.next() % 7 == 0 {
            w = (rng.next() % 40) as i32;
        }
        let ax1 = cx - w;
        let ax2 = cx + w * if rng.next() % 5 == 0 { -1 } else { 1 };
        let ay1 = cy - h;
        let ay2 = cy + h;
        let hit = z.form.intersects_rectangle(ax1, ax2, ay1, ay2);
        hits += i32::from(hit);
        writeln!(s, "{} {ax1} {ax2} {ay1} {ay2} {hit}", z.id).unwrap();
    }
    writeln!(s, "hits {hits}").unwrap();
    assert_eq!(hits, 26_036);
    assert_eq!(fnv1a(&s), 0xcd16_7876_ae7a_f2d1);
}

#[test]
fn loads_every_map_region_with_javas_count() {
    let Some(dir) = game_dir() else { return };
    let regions = MapRegions::load(&dir);
    assert_eq!(regions.len(), 57);
    let talking = regions.get("talking_island_town").unwrap();
    assert_eq!(talking.town, "Talking Island Town");
    assert_eq!((talking.loc_id, talking.castle, talking.bbs), (910, 1, 1));
    assert_eq!(talking.spawn_locs.len(), 14);
    assert_eq!(talking.chaotic_spawn_locs.len(), 20);
    assert_eq!(
        talking.spawn_locs[0],
        Location {
            x: -83990,
            y: 243336,
            z: -3700
        }
    );
    let elf = regions.get("elf_town").unwrap();
    assert_eq!(elf.banned_point("DARK_ELF"), Some("darkelf_town"));
    assert_eq!(elf.banned_point("HUMAN"), None);
}

#[test]
fn map_tiles_and_town_names() {
    let Some(dir) = game_dir() else { return };
    let regions = MapRegions::load(&dir);
    // Talking Island: tile (17, 25).
    assert_eq!(MapRegions::map_region_x(-83990), 17);
    assert_eq!(MapRegions::map_region_y(243336), 25);
    let region = regions.region_at(-83990, 243336).unwrap();
    assert_eq!(region.name, "talking_island_town");
    assert_eq!(regions.map_region_loc_id(-83990, 243336), 910);
    assert_eq!(regions.closest_town_name(-83990, 243336), "Talking Island Town");
    assert_eq!(regions.area_castle(-83990, 243336), 1);
    // Giran, Aden and the dark elf village.
    assert_eq!(regions.closest_town_name(82480, 149087), "Giran Castle Town");
    assert_eq!(regions.map_region_loc_id(82480, 149087), 918);
    assert_eq!(regions.closest_town_name(146494, 30584), "Aden Castle Town");
    assert_eq!(regions.region_at(12501, 16768).unwrap().name, "darkelf_town");
    // Off the map: no region, locId 0, Java's default town name, the default region's bbs.
    assert!(regions.region_at(0, 1_000_000).is_none());
    assert_eq!(regions.map_region_loc_id(0, 1_000_000), 0);
    assert_eq!(regions.closest_town_name(0, 1_000_000), "Aden Castle Town");
    assert_eq!(regions.area_castle(0, 1_000_000), 0);
    assert_eq!(regions.bbs(0, 1_000_000), 1);
    assert_eq!(regions.region_town("elf_town"), Some("Elven Town"));
    // Tiles claimed by two regions go to the one that is later in Java's HashMap order (checked
    // against Java's own tile index in the oracle test below).
    let tile = |tx: i32, ty: i32| {
        regions
            .region_at((tx - 20) * 32768 + 5, (ty - 18) * 32768 + 5)
            .map(|r| r.name.clone())
    };
    assert_eq!(
        tile(17, 19).as_deref(),
        Some("darkelf_town"),
        "also claimed by kamael_town"
    );
    assert_eq!(
        tile(16, 20).as_deref(),
        Some("kamael_town"),
        "also claimed by gludin_town"
    );
    assert_eq!(
        tile(19, 16).as_deref(),
        Some("rune_town"),
        "also claimed by orc_town"
    );
    assert_eq!(
        tile(20, 17).as_deref(),
        Some("primeval_island"),
        "also claimed by rune_town"
    );
    assert_eq!(
        tile(16, 10).as_deref(),
        Some("near_kamaroka"),
        "four regions claim it"
    );
}

/// Java's tile index and region table.
fn map_dump(m: &MapRegions) -> String {
    let mut s = String::new();
    writeln!(s, "regions {}", m.len()).unwrap();
    let mut regions: Vec<_> = m.iter().collect();
    regions.sort_by(|a, b| a.name.cmp(&b.name));
    // `Race` ordinal order, as Java's `TreeMap` over the `EnumMap` prints it.
    let order = ["HUMAN", "ELF", "DARK_ELF", "ORC", "DWARF", "KAMAEL"];
    for r in &regions {
        let first = r
            .spawn_locs
            .first()
            .map_or("-".to_string(), |l| format!("{},{},{}", l.x, l.y, l.z));
        let mut banned: Vec<_> = r.banned_races.iter().collect();
        banned.sort_by_key(|(race, _)| order.iter().position(|o| o == race).unwrap());
        let banned = if banned.is_empty() {
            "-".to_string()
        } else {
            let items: Vec<_> = banned.iter().map(|(a, p)| format!("{a}={p}")).collect();
            format!("{{{}}}", items.join(", "))
        };
        writeln!(
            s,
            "r {} | {} | {} {} {} | spawn {} other {} chaotic {} banish {} | first {first} | banned {banned}",
            r.name,
            r.town,
            r.loc_id,
            r.castle,
            r.bbs,
            r.spawn_locs.len(),
            r.other_spawn_locs.len(),
            r.chaotic_spawn_locs.len(),
            r.banish_spawn_locs.len()
        )
        .unwrap();
    }
    for x in 0..32 {
        for y in 0..32 {
            let px = ((x - 20) << 15) + 5;
            let py = ((y - 18) << 15) + 5;
            let name = m.region_at(px, py).map(|r| r.name.clone());
            writeln!(s, "t {x} {y} {}", name.unwrap_or_else(|| "null".into())).unwrap();
        }
    }
    for x in (-3..40).step_by(3) {
        for y in (-3..40).step_by(3) {
            let px = ((x - 20) << 15) + 77;
            let py = ((y - 18) << 15) + 99;
            writeln!(s, "l {px} {py} {}", m.map_region_loc_id(px, py)).unwrap();
        }
    }
    s
}

#[test]
fn oracle_map_region_table_and_tile_index() {
    let Some(dir) = game_dir() else { return };
    let regions = MapRegions::load(&dir);
    assert_eq!(fnv1a(&map_dump(&regions)), 0xc2ca_cbf9_1d35_4132);
}

#[test]
fn restart_points_follow_javas_rules() {
    let Some(dir) = game_dir() else { return };
    let zones = Zones::load(&dir);
    let regions = MapRegions::load(&dir);
    let mut first = first_spawn;
    let loc = |x, y, z| Some(Location { x, y, z });

    // Inside a region: its first normal restart point, or the first chaotic one with karma.
    assert_eq!(
        regions.nearest_town_respawn(
            &zones,
            Location {
                x: -83900,
                y: 243300,
                z: -3700
            },
            Some("HUMAN"),
            &mut first
        ),
        loc(-83990, 243336, -3700)
    );
    assert_eq!(
        regions.nearest_karma_respawn(
            &zones,
            Location {
                x: -83900,
                y: 243300,
                z: -3700
            },
            "HUMAN",
            false,
            &mut first
        ),
        loc(-79411, 240677, -3450)
    );
    // Dark elves are not welcome in the elf town: they restart in the dark elf village.
    assert_eq!(
        regions.nearest_town_respawn(
            &zones,
            Location {
                x: 45487,
                y: 49767,
                z: -2950
            },
            Some("ELF"),
            &mut first
        ),
        loc(45487, 49767, -2950)
    );
    assert_eq!(
        regions.nearest_town_respawn(
            &zones,
            Location {
                x: 45487,
                y: 49767,
                z: -2950
            },
            Some("DARK_ELF"),
            &mut first
        ),
        loc(12501, 16768, -4500)
    );
    // A random source picks by index, within the list.
    let last = regions
        .nearest_town_respawn(
            &zones,
            Location {
                x: -83900,
                y: 243300,
                z: -3700,
            },
            Some("HUMAN"),
            &mut |len| len - 1,
        )
        .unwrap();
    assert_eq!(last, regions.get("talking_island_town").unwrap().spawn_locs[13]);
    // Off the map the default region (talking_island_town) answers.
    assert_eq!(
        regions.nearest_town_respawn(
            &zones,
            Location {
                x: 0,
                y: 1_000_000,
                z: 0
            },
            Some("ORC"),
            &mut first
        ),
        loc(-83990, 243336, -3700)
    );

    // A RespawnZone overrides the region: inside the monster race track every race goes to
    // the "monster_race" restart point.
    let zone = zones.zone_at(13000, 182000, -3500, "RespawnZone").unwrap();
    assert_eq!(zone.name.as_deref(), Some("monster_race_0"));
    assert_eq!(
        regions.nearest_town_respawn(
            &zones,
            Location {
                x: 13000,
                y: 182000,
                z: -3500
            },
            Some("HUMAN"),
            &mut first
        ),
        loc(11892, 181700, -3560)
    );
    // A creature that is not a player cannot read the zone's race table: the default region.
    assert_eq!(
        regions.nearest_town_respawn(
            &zones,
            Location {
                x: 13000,
                y: 182000,
                z: -3500
            },
            None,
            &mut first
        ),
        loc(-83990, 243336, -3700)
    );

    // Restart region helpers.
    assert_eq!(regions.restart_region_loc_id("HUMAN", Some("elf_town")), 914);
    assert_eq!(regions.restart_region_loc_id("DARK_ELF", Some("elf_town")), 915);
    assert_eq!(
        regions.restart_region_loc_id("HUMAN", Some("no_such_region")),
        910
    );
    assert_eq!(
        regions.restart_region_spawn_loc("DARK_ELF", Some("elf_town"), &mut first),
        loc(12501, 16768, -4500)
    );
    // The flying fallback region exists in this datapack.
    assert!(regions.get("union_base_of_kserth").is_some());
}
