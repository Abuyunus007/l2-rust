//! Chamber of Delusion Tower, instance 132.
// Java: dist/game/data/scripts/instances/ChambersOfDelusion/ChamberOfDelusionTower.java

use super::chamber::{ChamberData, chamber_script};

// NPCs
const ENTRANCE_GATEKEEPER: i32 = 32663;
const ROOM_GATEKEEPER_FIRST: i32 = 32693;
const ROOM_GATEKEEPER_LAST: i32 = 32701;
const AENKINEL: i32 = 25695;
const BOX: i32 = 18823;

// Misc
const ENTER_POINTS: [(i32, i32, i32); 9] = [
    (-108976, -153372, -6688),
    (-108960, -152524, -6688),
    (-107088, -155052, -6688),
    (-107104, -154236, -6688),
    (-108048, -151244, -6688),
    (-107088, -152956, -6688),
    (-108992, -154604, -6688),
    (-108032, -152892, -6688),
    (-108048, -154572, -6688), // Raid room
];
const INSTANCEID: i32 = 132;

const DATA: ChamberData = ChamberData {
    instance_id: INSTANCEID,
    entrance_gatekeeper: ENTRANCE_GATEKEEPER,
    room_gatekeeper_first: ROOM_GATEKEEPER_FIRST,
    room_gatekeeper_last: ROOM_GATEKEEPER_LAST,
    aenkinel: AENKINEL,
    box_id: BOX,
    enter_points: &ENTER_POINTS,
};

/// Java `instances.ChambersOfDelusion.ChamberOfDelusionTower`.
pub struct ChamberOfDelusionTower;

chamber_script!(ChamberOfDelusionTower, "ChamberOfDelusionTower", DATA);
