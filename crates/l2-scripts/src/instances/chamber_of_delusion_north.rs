//! Chamber of Delusion North, instance 130.
// Java: dist/game/data/scripts/instances/ChambersOfDelusion/ChamberOfDelusionNorth.java

use super::chamber::{ChamberData, chamber_script};

// NPCs
const ENTRANCE_GATEKEEPER: i32 = 32661;
const ROOM_GATEKEEPER_FIRST: i32 = 32679;
const ROOM_GATEKEEPER_LAST: i32 = 32683;
const AENKINEL: i32 = 25693;
const BOX: i32 = 18838;

// Misc
const ENTER_POINTS: [(i32, i32, i32); 5] = [
    (-108976, -207772, -6720),
    (-108976, -206972, -6720),
    (-108960, -209164, -6720),
    (-108048, -207340, -6720),
    (-108048, -209020, -6720), // Raid room
];
const INSTANCEID: i32 = 130;

const DATA: ChamberData = ChamberData {
    instance_id: INSTANCEID,
    entrance_gatekeeper: ENTRANCE_GATEKEEPER,
    room_gatekeeper_first: ROOM_GATEKEEPER_FIRST,
    room_gatekeeper_last: ROOM_GATEKEEPER_LAST,
    aenkinel: AENKINEL,
    box_id: BOX,
    enter_points: &ENTER_POINTS,
};

/// Java `instances.ChambersOfDelusion.ChamberOfDelusionNorth`.
pub struct ChamberOfDelusionNorth;

chamber_script!(ChamberOfDelusionNorth, "ChamberOfDelusionNorth", DATA);
