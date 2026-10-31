#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Door, fence and static object data: the real Java files (counts and spot checks).
//!
//! Skipped with a printed message when the folder does not exist (CI). The folder defaults to
//! `dist/game`; set `L2JMOBIUS_GAME_DIR` to use another.

use std::path::PathBuf;

use l2_data::door::{OPEN_BY_CLICK, OPEN_BY_TIME};
use l2_data::{DoorTemplates, FenceState, FenceTemplates, StaticObjectTemplates};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR").map_or_else(
        || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")),
        PathBuf::from,
    );
    if dir.join("data").join("Doors.xml").is_file() {
        Some(dir)
    } else {
        println!("skipped: {} has no data/Doors.xml", dir.display());
        None
    }
}

#[test]
fn doors_xml_counts_and_spot_checks() {
    let Some(root) = game_dir() else { return };
    let doors = DoorTemplates::load(&root);
    // Java log: "DoorData: Loaded 1301 doors." (1301 <door> elements, all ids distinct)
    assert_eq!(doors.len(), 1301);
    assert_eq!(doors.iter().count(), 1301);
    assert_eq!(doors.iter().filter(|d| d.open_by_default).count(), 42);
    assert_eq!(doors.iter().filter(|d| d.group_name.is_some()).count(), 13);

    let d = doors.get(12_240_001).unwrap();
    assert_eq!(d.name, "gate_of_destruction001_01");
    assert_eq!(
        (d.pos_x, d.pos_y, d.pos_z),
        (-239_263, 219_986, -10_164),
        "min(pos z, nodeZ)"
    );
    assert_eq!(d.node_x, [-239_240, -239_240, -239_284, -239_283]);
    assert_eq!(d.node_y, [219_844, 220_034, 220_034, 219_844]);
    assert_eq!((d.node_z, d.height), (-10_164, 150));
}

#[test]
fn doors_xml_collision_and_timer_fields() {
    let Some(root) = game_dir() else { return };
    let doors = DoorTemplates::load(&root);
    // node1 -> node2 differ by 0 in x, so the radius is the minimum 20.
    assert_eq!(doors.get(12_240_001).unwrap().collision_radius, 20);
    let timed = doors.get(17_150_007).unwrap();
    assert!(timed.has_open_method(OPEN_BY_TIME));
    assert_eq!(
        (timed.open_time, timed.random_time, timed.close_time),
        (20, 10, 15)
    );
    assert_eq!(timed.child_door_id, 17_150_008);
    assert!(!timed.show_hp && !timed.targetable);
    assert_eq!(timed.base_hp_max, 3_258_432.0);
    // Doors without baseHpMax get DoorData's preset 1.
    let without_hp = doors.iter().filter(|d| d.base_hp_max == 1.0).count();
    assert_eq!(without_hp, 1301 - 1278);
    assert!(doors.iter().any(|d| d.has_open_method(OPEN_BY_CLICK)));
}

#[test]
fn fence_data_xml() {
    let Some(root) = game_dir() else { return };
    let fences = FenceTemplates::load(&root);
    // Java log: "FenceData: Loaded 1 fences." (the shipped demo fence)
    assert_eq!(fences.len(), 1);
    let f = fences.iter().next().unwrap();
    assert_eq!(f.name, "demo");
    assert_eq!((f.x, f.y, f.z), (-114_552, -251_256, -2_992));
    assert_eq!((f.width, f.length, f.height), (100, 100, 3));
    assert_eq!(f.state, FenceState::Closed);
}

#[test]
fn static_objects_xml() {
    let Some(root) = game_dir() else { return };
    let objects = StaticObjectTemplates::load(&root);
    // 51 live <object> elements (the rest of the file is commented out); Java keys them by their
    // own object id, so none collapse.
    assert_eq!(objects.len(), 51);
    let kinds = |k: i32| objects.iter().filter(|o| o.kind == k).count();
    assert_eq!((kinds(0), kinds(1), kinds(2), kinds(3)), (14, 9, 7, 21));
    let map = objects.iter().find(|o| o.id == 17_250_001).unwrap();
    assert_eq!(map.name, "town_map_talking_1");
    assert_eq!((map.x, map.y, map.z), (-82_873, 244_808, -3_717));
    assert_eq!(
        (map.texture.as_str(), map.map_x, map.map_y),
        ("town_map_talking_t00", 365, 152)
    );
    let throne = objects.iter().find(|o| o.id == 19_210_010).unwrap();
    assert_eq!((throne.kind, throne.texture.as_str()), (1, "none"));
}
