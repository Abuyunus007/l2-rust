//! Checks the pathfinding port against the Java vectors `java-vectors/out/geo_path_*.json`
//! (M3-GEO-02), produced by the real `PathFinding.findPath` (real `NodeBuffer` and `GeoLocation`)
//! on the whole-world `GeoEngine` of each geodata set.
//!
//! Each test loads the **whole world** of one set (`C:\l2-rust\geodata-*`) once and replays every
//! case of every config variant, then runs the shipped-config cases again through a [`PathPool`]
//! (several threads). They skip with a printed message when the folder is missing (CI has none).
//!
//! A case is `"x y z tx ty tz instance playable > result"`; the result is `null` or
//! `"<count> nodeX,nodeY,geoHeight,nswe ..."`; see the `note` in the JSON files.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use l2_geo::coords::geo_x;
use l2_geo::{
    GeoEngine, GeoEngineOptions, GeoLoadSpec, GeoLocation, Location, NoObstacles, PathFailure, PathFinder,
    PathOptions, PathPool, PathPoolConfig, PathRequest, PathResult,
};
use l2_testkit::vectors::load;
use serde::Deserialize;

/// A whole world is ~1 GB (download) to ~1.5 GB (converted) in memory: never two at once.
static ONE_WORLD_AT_A_TIME: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
struct PathFile {
    set: String,
    buffers: String,
    variants: Vec<Variant>,
}

#[derive(Deserialize)]
struct Variant {
    name: String,
    advanced_diagonal_strategy: bool,
    low_weight: String,
    medium_weight: String,
    high_weight: String,
    diagonal_weight: String,
    max_postfilter_passes: i32,
    cases: Vec<String>,
}

impl Variant {
    fn options(&self, buffers: &str) -> PathOptions {
        PathOptions {
            buffer_sizes: PathOptions::parse_buffers(buffers).unwrap(),
            low_weight: self.low_weight.parse().unwrap(),
            medium_weight: self.medium_weight.parse().unwrap(),
            high_weight: self.high_weight.parse().unwrap(),
            diagonal_weight: self.diagonal_weight.parse().unwrap(),
            advanced_diagonal_strategy: self.advanced_diagonal_strategy,
            max_postfilter_passes: self.max_postfilter_passes,
        }
    }
}

/// One parsed case.
struct Case {
    text: String,
    from: Location,
    to: Location,
    instance_id: i32,
    playable: bool,
    expected: Option<Vec<(i32, i32, i32, u8)>>,
}

fn parse_case(text: &str) -> Case {
    let (input, result) = text
        .split_once(" > ")
        .unwrap_or_else(|| panic!("no '>' in case {text:?}"));
    let v: Vec<i32> = input
        .split_whitespace()
        .map(|t| {
            t.parse()
                .unwrap_or_else(|_| panic!("bad integer {t:?} in {text:?}"))
        })
        .collect();
    let expected = if result == "null" {
        None
    } else {
        let mut parts = result.split_whitespace();
        let count: usize = parts.next().unwrap().parse().unwrap();
        let nodes: Vec<(i32, i32, i32, u8)> = parts
            .map(|n| {
                let f: Vec<i32> = n.split(',').map(|t| t.parse().unwrap()).collect();
                (f[0], f[1], f[2], f[3] as u8)
            })
            .collect();
        assert_eq!(nodes.len(), count, "node count in {text:?}");
        Some(nodes)
    };
    Case {
        text: text.to_string(),
        from: Location::new(v[0], v[1], v[2]),
        to: Location::new(v[3], v[4], v[5]),
        instance_id: v[6],
        playable: v[7] == 1,
        expected,
    }
}

fn nodes(path: &[GeoLocation]) -> Vec<(i32, i32, i32, u8)> {
    path.iter()
        .map(|n| (n.node_x(), n.node_y(), n.z(), n.nswe()))
        .collect()
}

fn set_dir(set: &str) -> PathBuf {
    PathBuf::from(r"C:\l2-rust").join(match set {
        "download" => "geodata-download",
        "converted" => "geodata-converted",
        other => panic!("unknown set {other}"),
    })
}

#[derive(Default)]
struct Timing {
    paths: usize,
    total: Duration,
    long_paths: usize,
    long_total: Duration,
    long_max: Duration,
    longest_cells: i32,
    limit_searches: usize,
    limit_max: Duration,
}

fn check_set(set: &str) {
    let file: PathFile = load(&format!("geo_path_{set}"));
    assert_eq!(file.set, set);
    let spec = GeoLoadSpec::new(set_dir(set));
    if !spec.geodata_path.is_dir() {
        eprintln!("SKIP: {} not found", spec.geodata_path.display());
        return;
    }
    let _guard = ONE_WORLD_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let start = Instant::now();
    let (engine, report) = GeoEngine::load(&spec, GeoEngineOptions::shipped());
    assert!(
        report.failed.is_empty(),
        "region files failed to load: {:?}",
        report.failed
    );
    assert_eq!(report.loaded, 203);
    eprintln!("{set}: world loaded in {:.2?}", start.elapsed());
    let engine = Arc::new(engine);

    let mut failures = [0usize; 4]; // NoGeo, BufferTooSmall, NoPath, IterationLimit
    let mut found = 0;
    let mut empty = 0;
    for variant in &file.variants {
        let mut finder = PathFinder::new(variant.options(&file.buffers)).unwrap();
        let mut timing = Timing::default();
        for text in &variant.cases {
            let case = parse_case(text);
            let begin = Instant::now();
            let got = finder.find_path_detailed(
                &engine,
                &NoObstacles,
                case.from,
                case.to,
                case.instance_id,
                case.playable,
            );
            let took = begin.elapsed();
            match (&got, &case.expected) {
                (Ok(path), Some(expected)) => {
                    assert_eq!(
                        &nodes(path),
                        expected,
                        "{set}/{} path differs for case {text}",
                        variant.name
                    );
                    // The world coordinates of the nodes are the conversion of the cell.
                    for n in path {
                        assert_eq!(n.x(), l2_geo::coords::world_x(n.node_x()));
                        assert_eq!(n.y(), l2_geo::coords::world_y(n.node_y()));
                    }
                    if path.is_empty() {
                        empty += 1;
                    } else {
                        found += 1;
                    }
                }
                (Err(why), None) => match why {
                    PathFailure::NoGeo => failures[0] += 1,
                    PathFailure::BufferTooSmall { .. } => failures[1] += 1,
                    PathFailure::NoPath => failures[2] += 1,
                    PathFailure::IterationLimit => {
                        failures[3] += 1;
                        timing.limit_searches += 1;
                        timing.limit_max = timing.limit_max.max(took);
                    }
                },
                (got, expected) => panic!(
                    "{set}/{}: Java says {}, Rust says {} for case {text}",
                    variant.name,
                    if expected.is_some() { "a path" } else { "null" },
                    match got {
                        Ok(p) => format!("a path of {} nodes", p.len()),
                        Err(e) => format!("{e:?}"),
                    }
                ),
            }

            if got.is_ok() {
                let cells = (geo_x(case.from.x) - geo_x(case.to.x))
                    .abs()
                    .max((l2_geo::coords::geo_y(case.from.y) - l2_geo::coords::geo_y(case.to.y)).abs());
                timing.paths += 1;
                timing.total += took;
                if cells >= 100 {
                    timing.long_paths += 1;
                    timing.long_total += took;
                    timing.long_max = timing.long_max.max(took);
                    timing.longest_cells = timing.longest_cells.max(cells);
                }
            }
        }
        eprintln!(
            "{set}/{}: {} cases ok; found paths {}: mean {:.2?}; paths of 100+ cells {}: mean {:.2?}, max {:.2?} (longest {} cells); {} searches hit the 7000 node limit, slowest {:.2?}",
            variant.name,
            variant.cases.len(),
            timing.paths,
            timing.total / timing.paths.max(1) as u32,
            timing.long_paths,
            timing.long_total / timing.long_paths.max(1) as u32,
            timing.long_max,
            timing.longest_cells,
            timing.limit_searches,
            timing.limit_max,
        );
    }
    eprintln!(
        "{set}: paths {found}, empty paths {empty}, no geodata {}, buffer too small {}, no path {}, iteration limit {}",
        failures[0], failures[1], failures[2], failures[3]
    );
    // The vectors must exercise every outcome, otherwise they prove little.
    assert!(found > 400, "too few paths found: {found}");
    assert!(empty > 0, "no start == target case");
    assert!(failures[0] > 0, "no 'no geodata' case");
    assert!(failures[1] > 0, "no 'buffer too small' case");
    assert!(failures[2] + failures[3] > 0, "no 'no path' case");

    check_pool(set, &file, &engine);
}

/// The same cases of the shipped variant through a pool of four threads.
fn check_pool(set: &str, file: &PathFile, engine: &Arc<GeoEngine>) {
    let variant = &file.variants[0];
    assert_eq!(variant.name, "shipped");
    let cases: Vec<Case> = variant.cases.iter().map(|t| parse_case(t)).collect();

    let (tx, rx) = mpsc::channel::<PathResult>();
    let mut config = PathPoolConfig::new(4);
    config.queue_capacity = cases.len();
    config.options = variant.options(&file.buffers);
    let pool = PathPool::with_config(config, Arc::clone(engine), Arc::new(tx)).unwrap();

    let begin = Instant::now();
    for (n, case) in cases.iter().enumerate() {
        let request = PathRequest::new(n as u64, 1, case.from, case.to, Arc::new(NoObstacles))
            .with_instance(case.instance_id)
            .with_playable(case.playable);
        pool.request(request).unwrap();
    }
    let mut results: Vec<Option<PathResult>> = (0..cases.len()).map(|_| None).collect();
    for _ in 0..cases.len() {
        let result = rx.recv_timeout(Duration::from_secs(120)).unwrap();
        let slot = result.requester as usize;
        assert!(results[slot].is_none(), "two results for case {slot}");
        results[slot] = Some(result);
    }
    eprintln!(
        "{set}: {} cases through a pool of 4 threads in {:.2?}",
        cases.len(),
        begin.elapsed()
    );
    for (case, result) in cases.iter().zip(results) {
        let result = result.unwrap();
        assert_eq!(result.id, 1);
        match (&case.expected, result.path) {
            (Some(expected), Ok(path)) => assert_eq!(&nodes(&path), expected, "pool: case {}", case.text),
            (None, Err(_)) => {}
            (expected, got) => panic!("pool: expected {expected:?}, got {got:?} for case {}", case.text),
        }
    }
    assert_eq!(pool.stats().delivered, cases.len() as u64);
}

#[test]
fn paths_match_java_download() {
    check_set("download");
}

#[test]
fn paths_match_java_converted() {
    check_set("converted");
}

#[test]
fn vector_files_have_the_expected_shape() {
    // Needs no geodata: runs on CI.
    for set in ["download", "converted"] {
        let file: PathFile = load(&format!("geo_path_{set}"));
        assert_eq!(file.set, set);
        assert_eq!(
            file.variants.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            ["shipped", "advanced_diagonal", "odd_weights", "no_postfilter"]
        );
        for variant in &file.variants {
            assert!(
                variant.cases.len() > 100,
                "{set}/{} has {} cases",
                variant.name,
                variant.cases.len()
            );
            variant.cases.iter().for_each(|c| {
                parse_case(c);
            });
            PathFinder::new(variant.options(&file.buffers)).unwrap();
        }
    }
}
