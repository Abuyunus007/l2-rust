//! Checks `l2-geo` against the Java vectors `java-vectors/out/geo_*.json` (M1-JV-03).
//!
//! The vectors were produced by the real `Region` class on the real `.l2j` files, so the tests need
//! the geodata folders. They skip with a printed message when a folder or file is missing (CI has
//! neither). The sample coordinates are global geo cell coordinates.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use l2_geo::cell::{NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};
use l2_geo::{GeoSet, Region};
use l2_testkit::vectors::{load, workspace_root};
use serde::Deserialize;

const REGIONS: [&str; 5] = ["22_22", "14_23", "16_10", "12_24", "13_21"];

#[derive(Deserialize)]
struct GeoFile {
    set: String,
    region: String,
    file: String,
    samples_count: usize,
    block_counts: Counts,
    samples: Vec<Sample>,
}

#[derive(Deserialize)]
struct Counts {
    flat: u32,
    complex: u32,
    multilayer: u32,
}

#[derive(Deserialize)]
struct Sample {
    geo_x: i32,
    geo_y: i32,
    world_z: i32,
    block_type: u8,
    nearest_z: i32,
    next_lower_z: i32,
    next_higher_z: i32,
    nswe: Nswe,
}

#[derive(Deserialize)]
struct Nswe {
    east: bool,
    west: bool,
    south: bool,
    north: bool,
}

fn set_dir(set: &str) -> PathBuf {
    let name = match set {
        "download" => "geodata-download",
        "converted" => "geodata-converted",
        other => panic!("unknown set {other}"),
    };
    PathBuf::from(r"C:\l2-rust").join(name)
}

/// Loads a region file, or prints a skip message and returns None.
fn region_file(set: &str, file: &str) -> Option<Region> {
    let path = set_dir(set).join(file);
    if !path.exists() {
        eprintln!("SKIP: {} not found", path.display());
        return None;
    }
    Some(Region::load_file(&path).unwrap())
}

fn check_vector(set: &str, region: &str) {
    let f: GeoFile = load(&format!("geo_{set}_{region}"));
    assert_eq!(f.set, set);
    assert_eq!(f.region, region);
    let Some(r) = region_file(set, &f.file) else {
        return;
    };

    let counts = r.block_counts();
    assert_eq!(
        (counts.flat, counts.complex, counts.multilayer),
        (
            f.block_counts.flat,
            f.block_counts.complex,
            f.block_counts.multilayer
        ),
        "block counts of {set} {region}"
    );

    assert_eq!(f.samples.len(), f.samples_count);
    for (i, s) in f.samples.iter().enumerate() {
        let ctx = format!(
            "{set} {region} sample {i}: ({}, {}, z {})",
            s.geo_x, s.geo_y, s.world_z
        );
        assert_eq!(
            r.block_type(s.geo_x, s.geo_y) as u8,
            s.block_type,
            "block_type {ctx}"
        );
        assert_eq!(
            r.nearest_z(s.geo_x, s.geo_y, s.world_z),
            s.nearest_z,
            "nearest_z {ctx}"
        );
        assert_eq!(
            r.next_lower_z(s.geo_x, s.geo_y, s.world_z),
            s.next_lower_z,
            "next_lower_z {ctx}"
        );
        assert_eq!(
            r.next_higher_z(s.geo_x, s.geo_y, s.world_z),
            s.next_higher_z,
            "next_higher_z {ctx}"
        );
        for (dir, name, expected) in [
            (NSWE_EAST, "east", s.nswe.east),
            (NSWE_WEST, "west", s.nswe.west),
            (NSWE_SOUTH, "south", s.nswe.south),
            (NSWE_NORTH, "north", s.nswe.north),
        ] {
            assert_eq!(
                r.check_nearest_nswe(s.geo_x, s.geo_y, s.world_z, dir),
                expected,
                "nswe {name} {ctx}"
            );
        }
        // nearest_nswe is the OR of the four single-direction checks.
        let combined = r.nearest_nswe(s.geo_x, s.geo_y, s.world_z);
        let expected = [
            (s.nswe.east, NSWE_EAST),
            (s.nswe.west, NSWE_WEST),
            (s.nswe.south, NSWE_SOUTH),
            (s.nswe.north, NSWE_NORTH),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .fold(0, |acc, (_, bit)| acc | bit);
        assert_eq!(combined, expected, "nearest_nswe {ctx}");
    }
}

#[test]
fn vectors_download() {
    for region in REGIONS {
        check_vector("download", region);
    }
}

#[test]
fn vectors_converted() {
    for region in REGIONS {
        check_vector("converted", region);
    }
}

/// `docs/analysis/05-geodata-comparison.csv`: columns a_* are the downloaded set, b_* the converted one.
fn csv_counts(set: &str, region: &str) -> (u32, u32, u32) {
    let path = workspace_root().join("docs/analysis/05-geodata-comparison.csv");
    let text = std::fs::read_to_string(path).unwrap();
    let line = text
        .lines()
        .find(|l| l.split(',').next() == Some(region))
        .unwrap_or_else(|| panic!("region {region} not in the CSV"));
    let c: Vec<u32> = line
        .split(',')
        .skip(1)
        .take(6)
        .map(|v| v.parse().unwrap())
        .collect();
    match set {
        "download" => (c[0], c[1], c[2]),
        _ => (c[3], c[4], c[5]),
    }
}

#[test]
fn block_counts_match_the_comparison_csv() {
    for set in ["download", "converted"] {
        for region in REGIONS {
            let Some(r) = region_file(set, &format!("{region}.l2j")) else {
                return;
            };
            let c = r.block_counts();
            assert_eq!(
                (c.flat, c.complex, c.multilayer),
                csv_counts(set, region),
                "{set} {region}"
            );
        }
    }
}

/// Parses every file of both sets and checks the memory bound. Slow (about 2 x 203 files), so it is
/// ignored by default: `cargo test -p l2-geo -- --ignored`.
#[test]
#[ignore = "needs C:\\l2-rust\\geodata-* and takes a while"]
fn every_file_of_both_sets_parses() {
    for set in ["download", "converted"] {
        let dir = set_dir(set);
        if !dir.is_dir() {
            eprintln!("SKIP: {} not found", dir.display());
            continue;
        }
        let mut worst = 0.0f64;
        let mut files = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path: PathBuf = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("l2j") {
                continue;
            }
            let size = std::fs::metadata(&path).unwrap().len() as f64;
            let r = Region::load_file(&path).unwrap_or_else(|e| panic!("{e}"));
            let ratio = r.heap_bytes() as f64 / size;
            assert!(ratio < 3.0, "{} uses {ratio:.2}x its file size", path.display());
            worst = worst.max(ratio);
            files += 1;
        }
        eprintln!("{set}: {files} files parsed, worst memory ratio {worst:.2}x");
        assert_eq!(files, 203, "{set}");

        let set_loaded = GeoSet::load_dir(Path::new(&dir)).unwrap();
        assert_eq!(set_loaded.loaded_count(), 203, "{set}");
    }
}
