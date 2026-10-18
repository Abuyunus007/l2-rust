//! Checks the `GeoEngine` port against the Java vectors `java-vectors/out/geo_engine_*.json`
//! (M3-GEO-01), produced by the real `GeoEngine`, `GridLineIterator2D` and `GridLineIterator3D`.
//!
//! * `geo_engine_coords.json` needs no geodata: coordinate conversion and both line iterators run
//!   everywhere, CI included.
//! * `geo_engine_download.json` / `geo_engine_converted.json` load the **whole world** of the
//!   matching geodata set (`C:\l2-rust\geodata-*`) through `GeoEngine::load` and replay every
//!   query. They skip with a printed message when the folder is missing (CI has none).
//!
//! Each case is a string `"<inputs> > <outputs>"` of space separated integers (booleans 0/1); see
//! the `note` in the JSON files.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use l2_geo::cell::{NSWE_EAST, NSWE_NORTH, NSWE_NORTH_EAST, NSWE_NORTH_WEST, NSWE_SOUTH};
use l2_geo::cell::{NSWE_SOUTH_EAST, NSWE_SOUTH_WEST, NSWE_WEST};
use l2_geo::coords::{geo_x, geo_y, world_x, world_y};
use l2_geo::{
    GeoEngine, GeoEngineOptions, GeoLoadSpec, GeoOverride, GridLineIterator2D, GridLineIterator3D, Location,
};
use l2_testkit::vectors::load;
use serde::Deserialize;

/// A whole world is ~1 GB (download) to ~1.5 GB (converted) in memory: never two at once.
static ONE_WORLD_AT_A_TIME: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
struct CoordsFile {
    geo_x: Vec<String>,
    geo_y: Vec<String>,
    world_x: Vec<String>,
    world_y: Vec<String>,
    line2d: Vec<String>,
    line3d: Vec<String>,
}

#[derive(Deserialize)]
struct SetFile {
    set: String,
    regions: Vec<String>,
    height: Vec<String>,
    nswe: Vec<String>,
    los: Vec<String>,
    can_move: Vec<String>,
    valid_location: Vec<String>,
    variants: Vec<Variant>,
}

#[derive(Deserialize)]
struct Variant {
    pathfinding: i32,
    avoid_obstructed_path_nodes: bool,
    can_move: Vec<String>,
    valid_location: Vec<String>,
    completely_blocked: Vec<String>,
}

/// Splits `"a b c > d e"` into the integers before and after the `>`.
fn case(text: &str) -> (Vec<i32>, Vec<i32>) {
    let (input, output) = text
        .split_once('>')
        .unwrap_or_else(|| panic!("no '>' in case {text:?}"));
    let ints = |s: &str| -> Vec<i32> {
        s.split_whitespace()
            .map(|t| {
                t.parse()
                    .unwrap_or_else(|_| panic!("bad integer {t:?} in {text:?}"))
            })
            .collect()
    };
    (ints(input), ints(output))
}

fn b(value: bool) -> i32 {
    i32::from(value)
}

fn loc(v: &[i32]) -> Location {
    Location::new(v[0], v[1], v[2])
}

// ---------------------------------------------------------------------------------- coordinates

#[test]
fn coordinate_conversion_matches_java() {
    let f: CoordsFile = load("geo_engine_coords");
    assert!(f.geo_x.len() > 300 && f.world_x.len() > 300);
    for (cases, what, convert) in [
        (&f.geo_x, "getGeoX", geo_x as fn(i32) -> i32),
        (&f.geo_y, "getGeoY", geo_y),
        (&f.world_x, "getWorldX", world_x),
        (&f.world_y, "getWorldY", world_y),
    ] {
        for text in cases {
            let (input, output) = case(text);
            assert_eq!(convert(input[0]), output[0], "{what} case {text}");
        }
    }
}

#[test]
fn line_iterators_match_java() {
    let f: CoordsFile = load("geo_engine_coords");
    assert!(f.line2d.len() > 100 && f.line3d.len() > 100);
    for text in &f.line2d {
        let (head, points) = text.split_once('>').unwrap();
        let v: Vec<i32> = head.split_whitespace().map(|t| t.parse().unwrap()).collect();
        let mut it = GridLineIterator2D::new(v[0], v[1], v[2], v[3]);
        let mut got = Vec::new();
        while it.advance() {
            got.push(format!("{},{}", it.x(), it.y()));
        }
        let expected: Vec<&str> = points.split_whitespace().collect();
        assert_eq!(got, expected, "line2d case {text}");
    }
    for text in &f.line3d {
        let (head, points) = text.split_once('>').unwrap();
        let v: Vec<i32> = head.split_whitespace().map(|t| t.parse().unwrap()).collect();
        let mut it = GridLineIterator3D::new(v[0], v[1], v[2], v[3], v[4], v[5]);
        let mut got = Vec::new();
        while it.advance() {
            got.push(format!("{},{},{}", it.x(), it.y(), it.z()));
        }
        let expected: Vec<&str> = points.split_whitespace().collect();
        assert_eq!(got, expected, "line3d case {text}");
    }
}

// ---------------------------------------------------------------------------------- engine

fn set_dir(set: &str) -> PathBuf {
    PathBuf::from(r"C:\l2-rust").join(match set {
        "download" => "geodata-download",
        "converted" => "geodata-converted",
        other => panic!("unknown set {other}"),
    })
}

/// Loads the whole world of a set and prints time and memory, or prints a skip message.
fn load_world(set: &str, spec: &GeoLoadSpec) -> Option<GeoEngine> {
    if !spec.geodata_path.is_dir() {
        eprintln!("SKIP: {} not found", spec.geodata_path.display());
        return None;
    }
    let start = Instant::now();
    let (engine, report) = GeoEngine::load(spec, GeoEngineOptions::shipped());
    eprintln!(
        "{set}: loaded {} regions in {:.2?}, {} MiB of region data, {} failed, {} overridden",
        report.loaded,
        start.elapsed(),
        engine.heap_bytes() / (1024 * 1024),
        report.failed.len(),
        report.overridden.len()
    );
    assert!(
        report.failed.is_empty(),
        "region files failed to load: {:?}",
        report.failed
    );
    assert_eq!(report.loaded, 203, "both sets have 203 region files");
    assert!(!report.pathfinding_disabled);
    Some(engine)
}

fn check_height(engine: &GeoEngine, text: &str, ctx: &str) {
    let (i, o) = case(text);
    let (x, y, z) = (i[0], i[1], i[2]);
    let got = [
        engine.height(x, y, z),
        b(engine.has_geo(x, y)),
        engine.spawn_height(x, y, z),
        geo_x(x),
        geo_y(y),
        engine.nearest_z(geo_x(x), geo_y(y), z),
        engine.next_lower_z(geo_x(x), geo_y(y), z),
        engine.next_higher_z(geo_x(x), geo_y(y), z),
    ];
    assert_eq!(got.as_slice(), o.as_slice(), "{ctx} height case {text}");
}

const NSWE_PROBES: [u8; 8] = [
    NSWE_EAST,
    NSWE_WEST,
    NSWE_SOUTH,
    NSWE_NORTH,
    NSWE_NORTH_EAST,
    NSWE_NORTH_WEST,
    NSWE_SOUTH_EAST,
    NSWE_SOUTH_WEST,
];

fn check_nswe(engine: &GeoEngine, text: &str, ctx: &str) {
    let (i, o) = case(text);
    let mut mask = 0;
    for (bit, probe) in NSWE_PROBES.iter().enumerate() {
        if engine.check_nearest_nswe_anti_corner_cut(i[0], i[1], i[2], *probe) {
            mask |= 1 << bit;
        }
    }
    let got = [mask, b(engine.is_completely_blocked(i[0], i[1], i[2]))];
    assert_eq!(got.as_slice(), o.as_slice(), "{ctx} nswe case {text}");
}

fn check_can_move(engine: &GeoEngine, text: &str, ctx: &str) {
    let (i, o) = case(text);
    let got = b(engine.can_move_to_target(loc(&i[0..3]), loc(&i[3..6]), i[6]));
    assert_eq!(got, o[0], "{ctx} can_move case {text}");
}

fn check_valid(engine: &GeoEngine, text: &str, ctx: &str) {
    let (i, o) = case(text);
    let got = engine.valid_location(loc(&i[0..3]), loc(&i[3..6]), i[6]);
    assert_eq!(
        [got.x, got.y, got.z].as_slice(),
        o.as_slice(),
        "{ctx} valid_location case {text}"
    );
}

fn check_set(set: &str) {
    let f: SetFile = load(&format!("geo_engine_{set}"));
    assert_eq!(f.set, set);
    let _guard = ONE_WORLD_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some(engine) = load_world(set, &GeoLoadSpec::new(set_dir(set))) else {
        return;
    };

    // Regions: present / absent as listed.
    for r in &f.regions {
        let (name, present) = r.split_once(" > ").unwrap();
        let (x, y) = name.split_once('_').unwrap();
        let (x, y): (u32, u32) = (x.parse().unwrap(), y.parse().unwrap());
        assert_eq!(
            engine.set().region(x, y).is_some(),
            present == "1",
            "region {name} of {set}"
        );
    }

    let ctx = set;
    f.height.iter().for_each(|t| check_height(&engine, t, ctx));
    f.nswe.iter().for_each(|t| check_nswe(&engine, t, ctx));
    for text in &f.los {
        let (i, o) = case(text);
        let (from, to) = (loc(&i[0..3]), loc(&i[3..6]));
        let got = b(engine.can_see_target_in_instance(from, to, i[6]));
        assert_eq!(got, o[0], "{ctx} los case {text}");
        assert_eq!(
            b(engine.can_see_target(from, to)),
            o[0],
            "{ctx} los (geodata only) case {text}"
        );
        assert_eq!(
            b(engine.can_see_target_between_instances(from, i[6], to, i[6])),
            o[0],
            "{ctx} los (same instance) case {text}"
        );
    }
    f.can_move.iter().for_each(|t| check_can_move(&engine, t, ctx));
    f.valid_location.iter().for_each(|t| check_valid(&engine, t, ctx));

    // One config flag flipped at a time (the engine is consumed and handed back with new options).
    assert_eq!(f.variants.len(), 2);
    let mut engine = engine;
    let mut variant_cases = 0;
    for v in &f.variants {
        engine = engine.with_options(GeoEngineOptions {
            pathfinding: v.pathfinding,
            avoid_obstructed_path_nodes: v.avoid_obstructed_path_nodes,
        });
        let vctx = format!(
            "{set} variant pathfinding={} avoid={}",
            v.pathfinding, v.avoid_obstructed_path_nodes
        );
        v.can_move.iter().for_each(|t| check_can_move(&engine, t, &vctx));
        v.valid_location
            .iter()
            .for_each(|t| check_valid(&engine, t, &vctx));
        for text in &v.completely_blocked {
            let (i, o) = case(text);
            assert_eq!(
                b(engine.is_completely_blocked(i[0], i[1], i[2])),
                o[0],
                "{vctx} blocked case {text}"
            );
        }
        variant_cases += v.can_move.len() + v.valid_location.len() + v.completely_blocked.len();
    }

    eprintln!(
        "{set}: {} height, {} nswe, {} los, {} can_move, {} valid_location, {variant_cases} variant cases OK",
        f.height.len(),
        f.nswe.len(),
        f.los.len(),
        f.can_move.len(),
        f.valid_location.len(),
    );
}

#[test]
fn engine_matches_java_on_the_download_set() {
    check_set("download");
}

#[test]
fn engine_matches_java_on_the_converted_set() {
    check_set("converted");
}

/// D-26 on real data: the download set as `GeoDataPath`, two regions taken from the converted
/// set. Single-cell queries (height, spawn height, layers) must give the converted set's Java
/// answer inside the overridden regions and the download set's answer everywhere else.
#[test]
fn override_regions_answer_like_their_own_set() {
    let download = set_dir("download");
    let converted = set_dir("converted");
    if !download.is_dir() || !converted.is_dir() {
        eprintln!("SKIP: geodata folders not found");
        return;
    }
    let _guard = ONE_WORLD_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let over = GeoOverride::new(&converted, "12_24,17_10");
    assert_eq!(over.regions, vec![(12, 24), (17, 10)]);
    let spec = GeoLoadSpec::new(&download).with_override(Some(over));
    let Some(engine) = load_world("download+override", &spec) else {
        return;
    };

    let from_download: SetFile = load("geo_engine_download");
    let from_converted: SetFile = load("geo_engine_converted");
    let region_of = |x: i32, y: i32| (geo_x(x) / 2048, geo_y(y) / 2048);
    let overridden = |x: i32, y: i32| matches!(region_of(x, y), (12, 24) | (17, 10));
    let (mut from_override, mut from_main) = (0, 0);
    for text in &from_converted.height {
        let (i, _) = case(text);
        if overridden(i[0], i[1]) {
            check_height(&engine, text, "override (converted answers)");
            from_override += 1;
        }
    }
    for text in &from_download.height {
        let (i, _) = case(text);
        if !overridden(i[0], i[1]) {
            check_height(&engine, text, "override (download answers)");
            from_main += 1;
        }
    }
    assert!(
        from_override > 100 && from_main > 400,
        "{from_override} / {from_main} cases"
    );
}
