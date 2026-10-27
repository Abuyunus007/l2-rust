#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The walking routes: the real `data/Routes.xml` (counts and spot checks).
//!
//! Skipped with a printed message when the folder does not exist (CI). The folder defaults to
//! `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.

use std::path::PathBuf;

use l2_data::Routes;
use l2_data::route::{REPEAT_GO_FIRST, REPEAT_RANDOM, REPEAT_TELE_FIRST};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"), PathBuf::from);
    if dir.join("data").join("Routes.xml").is_file() {
        Some(dir)
    } else {
        println!("skipped: {} has no data/Routes.xml", dir.display());
        None
    }
}

#[test]
fn routes_xml_counts_and_spot_checks() {
    let Some(root) = game_dir() else { return };
    let text = std::fs::read_to_string(root.join("data/Routes.xml")).unwrap();
    // Every `<route` element with a point loads (the file has 123).
    let routes = Routes::from_xml(&text, &|_| true).unwrap();
    assert_eq!(routes.len(), text.matches("<route ").count());
    assert_eq!(routes.len(), 123);

    let giran = routes.get("FPC_Giran_Evi").unwrap();
    assert_eq!(giran.repeat_type, REPEAT_GO_FIRST);
    assert!(giran.repeat_walk);
    assert_eq!(giran.nodes[0].delay, 10);
    assert!(giran.nodes[0].run);
    assert_eq!(
        (giran.nodes[0].x, giran.nodes[0].y, giran.nodes[0].z),
        (83_485, 147_998, -3_407)
    );
    // The target: NPC 80000 at the first point.
    assert!(routes.is_targeted(80_000));
    assert_eq!(
        routes.route_at(80_000, (83_485, 147_998, -3_407)),
        Some("FPC_Giran_Evi")
    );

    let styles: Vec<i8> = routes.iter().map(|r| r.repeat_type).collect();
    assert_eq!(styles.iter().filter(|s| **s == REPEAT_RANDOM).count(), 4);
    assert_eq!(styles.iter().filter(|s| **s == REPEAT_TELE_FIRST).count(), 5);
    // The random style is never "repeat walk" (Java's WalkRoute only accepts 0 to 2).
    assert!(
        routes
            .iter()
            .filter(|r| r.repeat_type == REPEAT_RANDOM)
            .all(|r| !r.repeat_walk)
    );
}
