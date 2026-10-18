//! Shape checks for the committed `java-vectors/out/geo_*.json` files (M1-JV-03).
//!
//! These tests only prove that the file parses, that the block counts add up, and that the sampled
//! values obey the invariants of the Java geodata API. The geo crates (M1-GEO-*) check the actual values.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_testkit::vectors::load;
use serde::Deserialize;

/// Cells per region side (`IRegion.REGION_CELLS_X`).
const REGION_CELLS: i32 = 2048;
/// Blocks per region (`IRegion.REGION_BLOCKS`).
const REGION_BLOCKS: u32 = 65536;
/// Fixed world Z values the generator samples (plus the cell's own nearest Z).
const FIXED_Z: [i32; 5] = [-16000, -2000, 0, 2000, 16000];
/// Samples per file (`GeoVectors.SAMPLES`).
const SAMPLES: usize = 2000;

#[derive(Deserialize)]
struct GeoFile {
    source: String,
    generated_by: String,
    note: String,
    set: String,
    region: String,
    file: String,
    seed: i64,
    samples_count: usize,
    block_counts: BlockCounts,
    samples: Vec<Sample>,
}

#[derive(Deserialize)]
struct BlockCounts {
    flat: u32,
    complex: u32,
    multilayer: u32,
}

#[derive(Deserialize)]
struct Sample {
    geo_x: i32,
    geo_y: i32,
    world_z: i32,
    z_source: String,
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

#[test]
fn geo_download_22_22_shape() {
    let f: GeoFile = load("geo_download_22_22");

    assert_eq!(f.generated_by, "java-vectors");
    assert!(!f.source.is_empty());
    assert!(!f.note.is_empty());
    assert_eq!(f.set, "download");
    assert_eq!(f.region, "22_22");
    assert_eq!(f.file, "22_22.l2j");
    assert_eq!(f.seed, 20_260_401 + 22 * 100 + 22);

    let counts = &f.block_counts;
    assert_eq!(counts.flat + counts.complex + counts.multilayer, REGION_BLOCKS);
    assert_eq!(
        (counts.flat, counts.complex, counts.multilayer),
        (5594, 55767, 4175)
    );

    assert_eq!(f.samples_count, SAMPLES);
    assert_eq!(f.samples.len(), SAMPLES);

    // Region (22, 22) covers geo cells [22 * 2048, 22 * 2048 + 2047] on both axes.
    let base = 22 * REGION_CELLS;
    for s in &f.samples {
        assert!(
            (base..base + REGION_CELLS).contains(&s.geo_x),
            "geo_x {}",
            s.geo_x
        );
        assert!(
            (base..base + REGION_CELLS).contains(&s.geo_y),
            "geo_y {}",
            s.geo_y
        );
        assert!(s.block_type <= 2, "block_type {}", s.block_type);
        match s.z_source.as_str() {
            "fixed" => assert!(FIXED_Z.contains(&s.world_z), "world_z {}", s.world_z),
            "own_nearest" => {}
            other => panic!("unknown z_source {other}"),
        }
        // Region::getNextLowerZ / getNextHigherZ never leave the requested Z on the wrong side.
        assert!(
            s.next_lower_z <= s.world_z,
            "next_lower_z {} > world_z {}",
            s.next_lower_z,
            s.world_z
        );
        assert!(
            s.next_higher_z >= s.world_z,
            "next_higher_z {} < world_z {}",
            s.next_higher_z,
            s.world_z
        );

        // FlatBlock.checkNearestNswe is always true. Flat and complex cells have one height, so the
        // next lower/higher Z is that height whenever it lies on the matching side of world_z.
        if s.block_type == 0 {
            assert!(
                s.nswe.east && s.nswe.west && s.nswe.south && s.nswe.north,
                "flat cell without NSWE"
            );
        }
        if s.block_type <= 1 {
            if s.nearest_z <= s.world_z {
                assert_eq!(s.next_lower_z, s.nearest_z, "flat/complex next_lower_z");
            }
            if s.nearest_z >= s.world_z {
                assert_eq!(s.next_higher_z, s.nearest_z, "flat/complex next_higher_z");
            }
        }
    }
}
