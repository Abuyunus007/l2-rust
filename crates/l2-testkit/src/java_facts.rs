//! Facts read from the Java source text, committed as JSON (T-VEC-01).
//!
//! `cargo run -p codegen -- java-facts` reads the `.java` files of the L2JMobius checkout and
//! writes `java-vectors/out/java_facts/*.json`. Tests that compare the port with those sources
//! load the JSON here, so they run without the checkout. A missing file is a test failure: the
//! files are committed.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::vectors::workspace_root;

/// Path of a fact file: `java-vectors/out/java_facts/<name>.json`.
pub fn fact_path(name: &str) -> PathBuf {
    workspace_root()
        .join("java-vectors")
        .join("out")
        .join("java_facts")
        .join(format!("{name}.json"))
}

/// Loads and deserializes `java-vectors/out/java_facts/<name>.json`.
///
/// Panics with the file path when the file is missing or doesn't match `T`.
pub fn load_fact<T: DeserializeOwned>(name: &str) -> T {
    let path = fact_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fact file {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "fact file {} doesn't match the expected shape: {e}",
            path.display()
        )
    })
}

/// A `VehiclePathPoint`: `[x, y, z, move_speed, rotation_speed]` (the three argument constructor
/// is already expanded to its defaults, 350 and 4000).
pub type PathPoint = [i32; 5];

/// `handlers.json`: the class lists of `dist/game/data/scripts/handlers`.
#[derive(Debug, Deserialize)]
pub struct Handlers {
    /// `bypass/npc` class to the words of its `COMMANDS` array.
    pub bypass_npc: BTreeMap<String, Vec<String>>,
    /// `bypass/communityboard` class to the words of its `COMMAND` array.
    pub communityboard: BTreeMap<String, Vec<String>>,
    /// `items` classes.
    pub items: Vec<String>,
    /// `chat/commands/user` class to the integer-only lines of its file (the command ids).
    pub user: BTreeMap<String, Vec<i32>>,
    /// `chat/commands/voiced` classes.
    pub voiced: Vec<String>,
    /// `actions/click` classes.
    pub click: Vec<String>,
    /// `actions/shiftclick` classes.
    pub shiftclick: Vec<String>,
    /// `punishments` classes.
    pub punishments: Vec<String>,
}

/// `boat_routes.json`: every `VehiclePathPoint[] NAME` of `scripts/vehicles/<File>.java`.
#[derive(Debug, Deserialize)]
pub struct BoatRoutes {
    /// File name (no extension) to array name to path.
    pub files: BTreeMap<String, BTreeMap<String, Vec<PathPoint>>>,
}

/// One dock controller of `ai/areas/Gracia/vehicles`.
#[derive(Debug, Deserialize)]
pub struct AirShipController {
    pub arrival: Vec<PathPoint>,
    pub depart: Vec<PathPoint>,
    pub teleports: Vec<Vec<PathPoint>>,
    pub dock_zone: i32,
    pub controller_id: i32,
    pub location: i32,
}

/// `air_ship_routes.json`.
#[derive(Debug, Deserialize)]
pub struct AirShipRoutes {
    /// Controller folder name (`SoDController`, ...) to its facts.
    pub controllers: BTreeMap<String, AirShipController>,
    /// `AirShipGludioGracia` array name to path.
    pub gludio_gracia: BTreeMap<String, Vec<PathPoint>>,
}

/// `player_classes.json`: `PlayerClass.java`.
#[derive(Debug, Deserialize)]
pub struct PlayerClasses {
    /// `(class id, race)` with the race as the Java constant (`HUMAN`, `DARK_ELF`, ...).
    pub races: Vec<(u16, String)>,
}

pub fn handlers() -> Handlers {
    load_fact("handlers")
}

pub fn boat_routes() -> BoatRoutes {
    load_fact("boat_routes")
}

pub fn air_ship_routes() -> AirShipRoutes {
    load_fact("air_ship_routes")
}

pub fn player_classes() -> PlayerClasses {
    load_fact("player_classes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_facts_have_the_expected_shape() {
        let h = handlers();
        assert!(h.bypass_npc.contains_key("Loto"));
        assert_eq!(h.user["Unstuck"], vec![52]);
        assert_eq!(h.click.len(), 10);
        assert_eq!(boat_routes().files["BoatInnadrilTour"]["TOUR"][0].len(), 5);
        assert_eq!(air_ship_routes().controllers.len(), 4);
        assert_eq!(player_classes().races.len(), 103);
    }

    #[test]
    #[should_panic(expected = "cannot read fact file")]
    fn a_missing_fact_file_is_a_failure() {
        let _: serde_json::Value = load_fact("does_not_exist");
    }
}
