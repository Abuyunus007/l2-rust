//! Runtime geodata edits against the Java vector `java-vectors/out/geo_edit.json` (L-07).
//!
//! The vector was made by the real `Region` class on a synthetic region (generator
//! `GeoEditVectors`), so this test needs no geodata folder and runs in CI: load, save untouched,
//! the `setNearestNswe` / `unsetNearestNswe` script (with `convertFlatToComplex`), save again,
//! then reload the saved file through `GeoEngine::reload_region`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use l2_geo::cell::{NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};
use l2_geo::{GeoEngine, GeoEngineOptions, GeoLoadSpec, GeoSet, Region};
use l2_testkit::bytes::{assert_bytes_eq, from_hex};
use l2_testkit::vectors::load;
use serde::Deserialize;

#[derive(Deserialize)]
struct EditFile {
    region: String,
    input_len: usize,
    input: Vec<Run>,
    edits: Vec<Edit>,
    saved_len: usize,
    saved: Vec<Run>,
}

#[derive(Deserialize)]
struct Run {
    count: usize,
    hex: String,
}

#[derive(Deserialize)]
struct Edit {
    op: String,
    geo_x: i32,
    geo_y: i32,
    world_z: i32,
    nswe: u8,
    block_type_after: u8,
    nearest_z_after: i32,
    nswe_after: u8,
}

fn expand(runs: &[Run]) -> Vec<u8> {
    let mut out = Vec::new();
    for run in runs {
        let block = from_hex(&run.hex);
        for _ in 0..run.count {
            out.extend_from_slice(&block);
        }
    }
    out
}

fn open_bits(engine: &GeoEngine, gx: i32, gy: i32, z: i32) -> u8 {
    [NSWE_EAST, NSWE_WEST, NSWE_SOUTH, NSWE_NORTH]
        .into_iter()
        .filter(|&dir| engine.check_nearest_nswe(gx, gy, z, dir))
        .fold(0, |acc, dir| acc | dir)
}

#[test]
fn edits_and_saves_match_java_byte_for_byte() {
    let f: EditFile = load("geo_edit");
    assert_eq!(f.region, "20_18");
    let input = expand(&f.input);
    assert_eq!(input.len(), f.input_len);
    let saved = expand(&f.saved);
    assert_eq!(saved.len(), f.saved_len);

    // An untouched region saves to the bytes it came from.
    let region = Region::parse(&input).unwrap();
    assert_bytes_eq(&region.to_bytes(), &input);

    let mut set = GeoSet::new();
    assert!(set.insert(20, 18, region));
    // The world holds its engine in an Arc and edits it copy-on-write; a snapshot taken before
    // the edits (a path worker's) must not change.
    let mut engine = Arc::new(GeoEngine::new(set, GeoEngineOptions::shipped()));
    let snapshot = Arc::clone(&engine);

    for (i, e) in f.edits.iter().enumerate() {
        let ctx = format!(
            "edit {i}: {} {} at ({}, {}, z {})",
            e.op, e.nswe, e.geo_x, e.geo_y, e.world_z
        );
        let geo = Arc::make_mut(&mut engine);
        match e.op.as_str() {
            "set" => geo.set_nearest_nswe(e.geo_x, e.geo_y, e.world_z, e.nswe),
            "unset" => geo.unset_nearest_nswe(e.geo_x, e.geo_y, e.world_z, e.nswe),
            other => panic!("unknown op {other}"),
        }
        let region = engine.region_at(e.geo_x, e.geo_y).unwrap();
        assert_eq!(
            region.block_type(e.geo_x, e.geo_y) as u8,
            e.block_type_after,
            "block type {ctx}"
        );
        assert_eq!(
            engine.nearest_z(e.geo_x, e.geo_y, e.world_z),
            e.nearest_z_after,
            "nearest z {ctx}"
        );
        assert_eq!(
            open_bits(&engine, e.geo_x, e.geo_y, e.world_z),
            e.nswe_after,
            "nswe {ctx}"
        );
    }

    let edited = engine.set().region(20, 18).unwrap();
    assert_bytes_eq(&edited.to_bytes(), &saved);
    assert_bytes_eq(&snapshot.set().region(20, 18).unwrap().to_bytes(), &input);

    // Save, then reload the saved file into a fresh engine: same bytes again.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("20_18.l2j");
    std::fs::write(&path, b"old file, replaced").unwrap();
    edited.save_to_file(&path).unwrap();
    assert_bytes_eq(&std::fs::read(&path).unwrap(), &saved);

    let mut reloaded =
        GeoEngine::new(GeoSet::new(), GeoEngineOptions::shipped()).with_source(GeoLoadSpec::new(dir.path()));
    assert!(!reloaded.has_geo_pos(20 * 2048, 18 * 2048));
    assert!(reloaded.reload_region(20, 18));
    assert_bytes_eq(&reloaded.set().region(20, 18).unwrap().to_bytes(), &saved);
    // Reloading a region that exists replaces it as well.
    assert!(reloaded.reload_region(20, 18));
    // No file, a negative region or no source: false.
    assert!(!reloaded.reload_region(21, 18));
    assert!(!reloaded.reload_region(-1, 18));
    let mut sourceless = GeoEngine::new(GeoSet::new(), GeoEngineOptions::shipped());
    assert!(!sourceless.reload_region(20, 18));
}

/// The real files of both geodata sets save back to themselves (skips without the folders).
#[test]
fn real_regions_save_to_their_own_bytes() {
    for set in ["geodata-download", "geodata-converted"] {
        for name in ["22_22", "14_23", "16_10", "12_24", "13_21"] {
            let path = std::path::PathBuf::from(r"C:\l2-rust")
                .join(set)
                .join(format!("{name}.l2j"));
            let Ok(bytes) = std::fs::read(&path) else {
                eprintln!("SKIP: {} not found", path.display());
                continue;
            };
            let region = Region::parse(&bytes).unwrap();
            assert!(
                region.to_bytes() == bytes,
                "{} does not save to its own bytes",
                path.display()
            );
        }
    }
}

#[test]
fn a_broken_file_keeps_the_old_region() {
    let f: EditFile = load("geo_edit");
    let input = expand(&f.input);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("20_18.l2j"), &input[..input.len() - 1]).unwrap();
    let mut set = GeoSet::new();
    assert!(set.insert(20, 18, Region::parse(&input).unwrap()));
    let mut engine =
        GeoEngine::new(set, GeoEngineOptions::shipped()).with_source(GeoLoadSpec::new(dir.path()));
    assert!(!engine.reload_region(20, 18));
    assert_bytes_eq(&engine.set().region(20, 18).unwrap().to_bytes(), &input);
}
