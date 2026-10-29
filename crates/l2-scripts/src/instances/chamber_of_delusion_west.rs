//! Chamber of Delusion West, instance 128.
// Java: dist/game/data/scripts/instances/ChambersOfDelusion/ChamberOfDelusionWest.java

use super::chamber::{ChamberData, chamber_script};

// NPCs
const ENTRANCE_GATEKEEPER: i32 = 32659;
const ROOM_GATEKEEPER_FIRST: i32 = 32669;
const ROOM_GATEKEEPER_LAST: i32 = 32673;
const AENKINEL: i32 = 25691;
const BOX: i32 = 18838;

// Misc
const ENTER_POINTS: [(i32, i32, i32); 5] = [
    (-108960, -218892, -6720),
    (-108976, -218028, -6720),
    (-108960, -220204, -6720),
    (-108032, -218428, -6720),
    (-108032, -220140, -6720), // Raid room
];
const INSTANCEID: i32 = 128;

const DATA: ChamberData = ChamberData {
    instance_id: INSTANCEID,
    entrance_gatekeeper: ENTRANCE_GATEKEEPER,
    room_gatekeeper_first: ROOM_GATEKEEPER_FIRST,
    room_gatekeeper_last: ROOM_GATEKEEPER_LAST,
    aenkinel: AENKINEL,
    box_id: BOX,
    enter_points: &ENTER_POINTS,
};

/// Java `instances.ChambersOfDelusion.ChamberOfDelusionWest`.
pub struct ChamberOfDelusionWest;

chamber_script!(ChamberOfDelusionWest, "ChamberOfDelusionWest", DATA);
