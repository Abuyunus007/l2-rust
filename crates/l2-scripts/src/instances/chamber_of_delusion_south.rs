//! Chamber of Delusion South, instance 129.
// Java: dist/game/data/scripts/instances/ChambersOfDelusion/ChamberOfDelusionSouth.java

use super::chamber::{ChamberData, chamber_script};

// NPCs
const ENTRANCE_GATEKEEPER: i32 = 32660;
const ROOM_GATEKEEPER_FIRST: i32 = 32674;
const ROOM_GATEKEEPER_LAST: i32 = 32678;
const AENKINEL: i32 = 25692;
const BOX: i32 = 18838;

// Misc
const ENTER_POINTS: [(i32, i32, i32); 5] = [
    (-122368, -207820, -6720),
    (-122368, -206940, -6720),
    (-122368, -209116, -6720),
    (-121456, -207356, -6720),
    (-121440, -209004, -6720), // Raid room
];
const INSTANCEID: i32 = 129;

const DATA: ChamberData = ChamberData {
    instance_id: INSTANCEID,
    entrance_gatekeeper: ENTRANCE_GATEKEEPER,
    room_gatekeeper_first: ROOM_GATEKEEPER_FIRST,
    room_gatekeeper_last: ROOM_GATEKEEPER_LAST,
    aenkinel: AENKINEL,
    box_id: BOX,
    enter_points: &ENTER_POINTS,
};

/// Java `instances.ChambersOfDelusion.ChamberOfDelusionSouth`.
pub struct ChamberOfDelusionSouth;

chamber_script!(ChamberOfDelusionSouth, "ChamberOfDelusionSouth", DATA);
