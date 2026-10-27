//! The five ferry routes, transcribed from the Java boat scripts (`dist/game/data/scripts/vehicles/`).
//!
//! The coordinates, speeds, messages, ticket items and delays are the scripts' numbers; the engine
//! that plays them is `boat.rs`. A test (`tests/vehicle.rs`) compares every path with the Java
//! source when the reference folder is there.
// Java: dist/game/data/scripts/vehicles/{BoatTalkingGludin,BoatGiranTalking,BoatGludinRune,
//       BoatRunePrimeval,BoatInnadrilTour}.java

use l2_protocol_game::SystemMessageId;

use super::route::{BoatRoute, BoatStep, Broadcast, Cast, PathPoint, Pay, Pt, WaitDock};

/// `new VehiclePathPoint(x, y, z, moveSpeed, rotationSpeed)`.
const fn p(x: i32, y: i32, z: i32, move_speed: i32, rotation_speed: i32) -> PathPoint {
    PathPoint::new(x, y, z, move_speed, rotation_speed)
}

// ---- BoatTalkingGludin ----

pub static TALKING_GLUDIN_TALKING_TO_GLUDIN: [PathPoint; 10] = [
    p(-121385, 261660, -3610, 180, 800),
    p(-127694, 253312, -3610, 200, 800),
    p(-129274, 237060, -3610, 250, 800),
    p(-114688, 139040, -3610, 200, 800),
    p(-109663, 135704, -3610, 180, 800),
    p(-102151, 135704, -3610, 180, 800),
    p(-96686, 140595, -3610, 180, 800),
    p(-95686, 147718, -3610, 180, 800),
    p(-95686, 148718, -3610, 180, 800),
    p(-95686, 149718, -3610, 150, 800),
];

pub static TALKING_GLUDIN_GLUDIN_DOCK: [PathPoint; 1] = [
    p(-95686, 150514, -3610, 150, 800),
];

pub static TALKING_GLUDIN_GLUDIN_TO_TALKING: [PathPoint; 9] = [
    p(-95686, 155514, -3610, 180, 800),
    p(-95686, 185514, -3610, 250, 800),
    p(-60136, 238816, -3610, 200, 800),
    p(-60520, 259609, -3610, 180, 1800),
    p(-65344, 261460, -3610, 180, 1800),
    p(-83344, 261560, -3610, 180, 1800),
    p(-88344, 261660, -3610, 180, 1800),
    p(-92344, 261660, -3610, 150, 1800),
    p(-94242, 261659, -3610, 150, 1800),
];

pub static TALKING_GLUDIN_TALKING_DOCK: [PathPoint; 1] = [
    p(-96622, 261660, -3610, 150, 1800),
];

static TALKING_GLUDIN_STEPS: [BoatStep; 18] = [
    // case 0
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillLeaveForGludinHarborInFiveMinutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 1
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillLeaveForGludinHarborInOneMinute), Cast::Msg(SystemMessageId::ThoseWishingToRideTheFerryShouldMakeHasteToGetOn)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(40000),
    },
    // case 2
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillBeLeavingSoonForGludinHarbor)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(20000),
    },
    // case 3
    BoatStep {
        wait_dock: None,
        dock_ops: &[(1, false)],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryIsLeavingForGludinHarbor)] }),
        boat_sound: Some(Pt(-96622, 261660, -3610)),
        pay: Some(Pay { item_id: 1074, count: 1, oust: (-96777, 258970, -3623) }),
        exec: Some(&TALKING_GLUDIN_TALKING_TO_GLUDIN),
        next_ms: Some(300000),
    },
    // case 4
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromTalkingIslandWillArriveAtGludinHarborInApproximately10Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 5
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromTalkingIslandWillBeArrivingAtGludinHarborInApproximately5Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 6
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromTalkingIslandWillBeArrivingAtGludinHarborInApproximately1Minute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: None,
    },
    // case 7
    BoatStep {
        wait_dock: Some(WaitDock { dock: 2, near: (Pt(-95686, 150514, -3610), Pt(-96622, 261660, -3610)), busy: SystemMessageId::TheFerryFromTalkingIslandToGludinHarborHasBeenDelayed }),
        dock_ops: &[],
        broadcast: None,
        boat_sound: None,
        pay: None,
        exec: Some(&TALKING_GLUDIN_GLUDIN_DOCK),
        next_ms: None,
    },
    // case 8
    BoatStep {
        wait_dock: None,
        dock_ops: &[(2, true)],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryHasArrivedAtGludinHarbor), Cast::Msg(SystemMessageId::TheFerryWillLeaveForTalkingIslandHarborAfterAnchoringForTenMinutes)] }),
        boat_sound: Some(Pt(-95686, 150514, -3610)),
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 9
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillLeaveForTalkingIslandHarborInFiveMinutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 10
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillLeaveForTalkingIslandHarborInOneMinute), Cast::Msg(SystemMessageId::ThoseWishingToRideTheFerryShouldMakeHasteToGetOn)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(40000),
    },
    // case 11
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillBeLeavingSoonForTalkingIslandHarbor)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(20000),
    },
    // case 12
    BoatStep {
        wait_dock: None,
        dock_ops: &[(2, false)],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryIsLeavingForTalkingIslandHarbor)] }),
        boat_sound: Some(Pt(-95686, 150514, -3610)),
        pay: Some(Pay { item_id: 1075, count: 1, oust: (-90015, 150422, -3610) }),
        exec: Some(&TALKING_GLUDIN_GLUDIN_TO_TALKING),
        next_ms: Some(150000),
    },
    // case 13
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGludinHarborWillBeArrivingAtTalkingIslandInApproximately10Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 14
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGludinHarborWillBeArrivingAtTalkingIslandInApproximately5Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 15
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGludinHarborWillBeArrivingAtTalkingIslandInApproximately1Minute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: None,
    },
    // case 16
    BoatStep {
        wait_dock: Some(WaitDock { dock: 1, near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), busy: SystemMessageId::TheFerryFromGludinHarborToTalkingIslandHasBeenDelayed }),
        dock_ops: &[],
        broadcast: None,
        boat_sound: None,
        pay: None,
        exec: Some(&TALKING_GLUDIN_TALKING_DOCK),
        next_ms: None,
    },
    // case 17
    BoatStep {
        wait_dock: None,
        dock_ops: &[(1, true)],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryHasArrivedAtTalkingIslandHarbor), Cast::Msg(SystemMessageId::TheFerryWillLeaveForGludinHarborAfterAnchoringForTenMinutes)] }),
        boat_sound: Some(Pt(-96622, 261660, -3610)),
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
];

// ---- BoatGiranTalking ----

pub static GIRAN_TALKING_GIRAN_TO_TALKING: [PathPoint; 19] = [
    p(51914, 189023, -3610, 150, 800),
    p(60567, 189789, -3610, 150, 800),
    p(63732, 197457, -3610, 200, 800),
    p(63732, 219946, -3610, 250, 800),
    p(62008, 222240, -3610, 250, 1200),
    p(56115, 226791, -3610, 250, 1200),
    p(40384, 226432, -3610, 300, 800),
    p(37760, 226432, -3610, 300, 800),
    p(27153, 226791, -3610, 300, 800),
    p(12672, 227535, -3610, 300, 800),
    p(-1808, 228280, -3610, 300, 800),
    p(-22165, 230542, -3610, 300, 800),
    p(-42523, 235205, -3610, 300, 800),
    p(-68451, 259560, -3610, 250, 800),
    p(-70848, 261696, -3610, 200, 800),
    p(-83344, 261610, -3610, 200, 800),
    p(-88344, 261660, -3610, 180, 800),
    p(-92344, 261660, -3610, 180, 800),
    p(-94242, 261659, -3610, 150, 800),
];

pub static GIRAN_TALKING_TALKING_DOCK: [PathPoint; 1] = [
    p(-96622, 261660, -3610, 150, 800),
];

pub static GIRAN_TALKING_TALKING_TO_GIRAN: [PathPoint; 18] = [
    p(-113925, 261660, -3610, 150, 800),
    p(-126107, 249116, -3610, 180, 800),
    p(-126107, 234499, -3610, 180, 800),
    p(-126107, 219882, -3610, 180, 800),
    p(-109414, 204914, -3610, 180, 800),
    p(-92807, 204914, -3610, 180, 800),
    p(-80425, 216450, -3610, 250, 800),
    p(-68043, 227987, -3610, 250, 800),
    p(-63744, 231168, -3610, 250, 800),
    p(-60844, 231369, -3610, 250, 1800),
    p(-44915, 231369, -3610, 200, 800),
    p(-28986, 231369, -3610, 200, 800),
    p(8233, 207624, -3610, 200, 800),
    p(21470, 201503, -3610, 180, 800),
    p(40058, 195383, -3610, 180, 800),
    p(43022, 193793, -3610, 150, 800),
    p(45986, 192203, -3610, 150, 800),
    p(48950, 190613, -3610, 150, 800),
];

static GIRAN_TALKING_STEPS: [BoatStep; 19] = [
    // case 0
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillLeaveForTalkingIslandHarborInFiveMinutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 1
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillLeaveForTalkingIslandHarborInOneMinute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(40000),
    },
    // case 2
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillBeLeavingSoonForTalkingIslandHarbor)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(20000),
    },
    // case 3
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryIsLeavingForTalkingIslandHarbor), Cast::Msg(SystemMessageId::TheFerryFromGiranHarborWillBeArrivingAtTalkingIslandInApproximately15Minutes)] }),
        boat_sound: Some(Pt(48950, 190613, -3610)),
        pay: Some(Pay { item_id: 3946, count: 1, oust: (46763, 187041, -3451) }),
        exec: Some(&GIRAN_TALKING_GIRAN_TO_TALKING),
        next_ms: Some(250000),
    },
    // case 4
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGiranHarborWillBeArrivingAtTalkingIslandInApproximately10Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 5
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGiranHarborWillBeArrivingAtTalkingIslandInApproximately5Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 6
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGiranHarborWillBeArrivingAtTalkingIslandInApproximately1Minute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: None,
    },
    // case 7
    BoatStep {
        wait_dock: Some(WaitDock { dock: 1, near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), busy: SystemMessageId::TheFerryFromGiranHarborToTalkingIslandHasBeenDelayed }),
        dock_ops: &[],
        broadcast: None,
        boat_sound: None,
        pay: None,
        exec: Some(&GIRAN_TALKING_TALKING_DOCK),
        next_ms: None,
    },
    // case 8
    BoatStep {
        wait_dock: None,
        dock_ops: &[(1, true)],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryHasArrivedAtTalkingIslandHarbor), Cast::Msg(SystemMessageId::TheFerryWillLeaveForGiranHarborAfterAnchoringForTenMinutes)] }),
        boat_sound: Some(Pt(-96622, 261660, -3610)),
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 9
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillLeaveForGiranHarborInFiveMinutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 10
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillLeaveForGiranHarborInOneMinute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(40000),
    },
    // case 11
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryWillBeLeavingSoonForGiranHarbor)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(20000),
    },
    // case 12
    BoatStep {
        wait_dock: None,
        dock_ops: &[(1, false)],
        broadcast: Some(Broadcast { near: (Pt(-96622, 261660, -3610), Pt(48950, 190613, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryIsLeavingForGiranHarbor)] }),
        boat_sound: Some(Pt(-96622, 261660, -3610)),
        pay: Some(Pay { item_id: 3945, count: 1, oust: (-96777, 258970, -3623) }),
        exec: Some(&GIRAN_TALKING_TALKING_TO_GIRAN),
        next_ms: Some(200000),
    },
    // case 13
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromTalkingIslandWillBeArrivingAtGiranHarborInApproximately20Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 14
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromTalkingIslandWillBeArrivingAtGiranHarborInApproximately15Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 15
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromTalkingIslandWillBeArrivingAtGiranHarborInApproximately10Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 16
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromTalkingIslandWillBeArrivingAtGiranHarborInApproximately5Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 17
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromTalkingIslandWillBeArrivingAtGiranHarborInApproximately1Minute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: None,
    },
    // case 18
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(48950, 190613, -3610), Pt(-96622, 261660, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryHasArrivedAtGiranHarbor), Cast::Msg(SystemMessageId::TheFerryWillLeaveForTalkingIslandHarborAfterAnchoringForTenMinutes)] }),
        boat_sound: Some(Pt(48950, 190613, -3610)),
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
];

// ---- BoatGludinRune ----

pub static GLUDIN_RUNE_GLUDIN_TO_RUNE: [PathPoint; 27] = [
    p(-95686, 155514, -3610, 150, 800),
    p(-98112, 159040, -3610, 150, 800),
    p(-104192, 160608, -3610, 200, 1800),
    p(-109952, 159616, -3610, 250, 1800),
    p(-112768, 154784, -3610, 290, 1800),
    p(-114688, 139040, -3610, 290, 1800),
    p(-115232, 134368, -3610, 290, 1800),
    p(-113888, 121696, -3610, 290, 1800),
    p(-107808, 104928, -3610, 290, 1800),
    p(-97152, 75520, -3610, 290, 800),
    p(-85536, 67264, -3610, 290, 1800),
    p(-64640, 55840, -3610, 290, 1800),
    p(-60096, 44672, -3610, 290, 1800),
    p(-52672, 37440, -3610, 290, 1800),
    p(-46144, 33184, -3610, 290, 1800),
    p(-36096, 24928, -3610, 290, 1800),
    p(-33792, 8448, -3610, 290, 1800),
    p(-23776, 3424, -3610, 290, 1000),
    p(-12000, -1760, -3610, 290, 1000),
    p(672, 480, -3610, 290, 1800),
    p(15488, 200, -3610, 290, 1000),
    p(24736, 164, -3610, 290, 1000),
    p(32192, -1156, -3610, 290, 1000),
    p(39200, -8032, -3610, 270, 1000),
    p(44320, -25152, -3610, 270, 1000),
    p(40576, -31616, -3610, 250, 800),
    p(36819, -35315, -3610, 220, 800),
];

pub static GLUDIN_RUNE_RUNE_DOCK: [PathPoint; 1] = [
    p(34381, -37680, -3610, 200, 800),
];

pub static GLUDIN_RUNE_RUNE_TO_GLUDIN: [PathPoint; 22] = [
    p(32750, -39300, -3610, 150, 800),
    p(27440, -39328, -3610, 180, 1000),
    p(21456, -34272, -3610, 200, 1000),
    p(6608, -29520, -3610, 250, 800),
    p(4160, -27828, -3610, 270, 800),
    p(2432, -25472, -3610, 270, 1000),
    p(-8000, -16272, -3610, 220, 1000),
    p(-18976, -9760, -3610, 290, 800),
    p(-23776, 3408, -3610, 290, 800),
    p(-33792, 8432, -3610, 290, 800),
    p(-36096, 24912, -3610, 290, 800),
    p(-46144, 33184, -3610, 290, 800),
    p(-52688, 37440, -3610, 290, 800),
    p(-60096, 44672, -3610, 290, 800),
    p(-64640, 55840, -3610, 290, 800),
    p(-85552, 67248, -3610, 290, 800),
    p(-97168, 85264, -3610, 290, 800),
    p(-107824, 104912, -3610, 290, 800),
    p(-102151, 135704, -3610, 290, 800),
    p(-96686, 140595, -3610, 290, 800),
    p(-95686, 147717, -3610, 250, 800),
    p(-95686, 148218, -3610, 200, 800),
];

pub static GLUDIN_RUNE_GLUDIN_DOCK: [PathPoint; 1] = [
    p(-95686, 150514, -3610, 150, 800),
];

static GLUDIN_RUNE_STEPS: [BoatStep; 20] = [
    // case 0
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::DepartureForRuneHarborWillTakePlaceInFiveMinutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 1
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::DepartureForRuneHarborWillTakePlaceInOneMinute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(40000),
    },
    // case 2
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::MakeHasteWeWillBeDepartingForGludinHarborShortly2)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(20000),
    },
    // case 3
    BoatStep {
        wait_dock: None,
        dock_ops: &[(2, false)],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::WeAreNowDepartingForGludinHarborHoldOnAndEnjoyTheRide)] }),
        boat_sound: Some(Pt(-95686, 150514, -3610)),
        pay: Some(Pay { item_id: 7905, count: 1, oust: (-90015, 150422, -3610) }),
        exec: Some(&GLUDIN_RUNE_GLUDIN_TO_RUNE),
        next_ms: Some(250000),
    },
    // case 4
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGludinHarborWillBeArrivingAtRuneHarborInApproximately15Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 5
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGludinHarborWillBeArrivingAtRuneHarborInApproximately10Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 6
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGludinHarborWillBeArrivingAtRuneHarborInApproximately5Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 7
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromGludinHarborWillBeArrivingAtRuneHarborInApproximately1Minute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: None,
    },
    // case 8
    BoatStep {
        wait_dock: Some(WaitDock { dock: 3, near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), busy: SystemMessageId::TheFerryFromGludinHarborToRuneHarborHasBeenDelayed }),
        dock_ops: &[],
        broadcast: None,
        boat_sound: None,
        pay: None,
        exec: Some(&GLUDIN_RUNE_RUNE_DOCK),
        next_ms: None,
    },
    // case 9
    BoatStep {
        wait_dock: None,
        dock_ops: &[(3, true)],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::WelcomeToRuneHarbor), Cast::Msg(SystemMessageId::TheFerryWillLeaveForGludinHarborAfterAnchoringForTenMinutes)] }),
        boat_sound: Some(Pt(34381, -37680, -3610)),
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 10
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::DepartureForGludinHarborWillTakePlaceInFiveMinutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 11
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::DepartureForGludinHarborWillTakePlaceInOneMinute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(40000),
    },
    // case 12
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::MakeHasteWeWillBeDepartingForGludinHarborShortly)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(20000),
    },
    // case 13
    BoatStep {
        wait_dock: None,
        dock_ops: &[(3, false)],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(-95686, 150514, -3610)), items: &[Cast::Msg(SystemMessageId::WeAreNowDepartingForGludinHarborHoldOnAndEnjoyTheRide)] }),
        boat_sound: Some(Pt(34381, -37680, -3610)),
        pay: Some(Pay { item_id: 7904, count: 1, oust: (34513, -38009, -3640) }),
        exec: Some(&GLUDIN_RUNE_RUNE_TO_GLUDIN),
        next_ms: Some(60000),
    },
    // case 14
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromRuneHarborWillBeArrivingAtGludinHarborInApproximately15Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 15
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromRuneHarborWillBeArrivingAtGludinHarborInApproximately10Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 16
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromRuneHarborWillBeArrivingAtGludinHarborInApproximately5Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 17
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryFromRuneHarborWillBeArrivingAtGludinHarborInApproximately1Minute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: None,
    },
    // case 18
    BoatStep {
        wait_dock: Some(WaitDock { dock: 2, near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), busy: SystemMessageId::TheFerryFromRuneHarborToGludinHarborHasBeenDelayed }),
        dock_ops: &[],
        broadcast: None,
        boat_sound: None,
        pay: None,
        exec: Some(&GLUDIN_RUNE_GLUDIN_DOCK),
        next_ms: None,
    },
    // case 19
    BoatStep {
        wait_dock: None,
        dock_ops: &[(2, true)],
        broadcast: Some(Broadcast { near: (Pt(-95686, 150514, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryHasArrivedAtGludinHarbor), Cast::Msg(SystemMessageId::DepartureForRuneHarborWillTakePlaceAfterAnchoringForTenMinutes)] }),
        boat_sound: Some(Pt(-95686, 150514, -3610)),
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
];

// ---- BoatRunePrimeval ----

pub static RUNE_PRIMEVAL_RUNE_TO_PRIMEVAL: [PathPoint; 12] = [
    p(32750, -39300, -3610, 180, 800),
    p(27440, -39328, -3610, 250, 1000),
    p(19616, -39360, -3610, 270, 1000),
    p(3840, -38528, -3610, 270, 1000),
    p(1664, -37120, -3610, 270, 1000),
    p(896, -34560, -3610, 180, 1800),
    p(832, -31104, -3610, 180, 180),
    p(2240, -29132, -3610, 150, 1800),
    p(4160, -27828, -3610, 150, 1800),
    p(5888, -27279, -3610, 150, 1800),
    p(7000, -27279, -3610, 150, 1800),
    p(10342, -27279, -3610, 150, 1800),
];

pub static RUNE_PRIMEVAL_PRIMEVAL_TO_RUNE: [PathPoint; 8] = [
    p(15528, -27279, -3610, 180, 800),
    p(22304, -29664, -3610, 290, 800),
    p(33824, -26880, -3610, 290, 800),
    p(38848, -21792, -3610, 240, 1200),
    p(43424, -22080, -3610, 180, 1800),
    p(44320, -25152, -3610, 180, 1800),
    p(40576, -31616, -3610, 250, 800),
    p(36819, -35315, -3610, 220, 800),
];

pub static RUNE_PRIMEVAL_RUNE_DOCK: [PathPoint; 1] = [
    p(34381, -37680, -3610, 220, 800),
];

static RUNE_PRIMEVAL_STEPS: [BoatStep; 5] = [
    // case 0
    BoatStep {
        wait_dock: None,
        dock_ops: &[(3, false)],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(10342, -27279, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryIsNowDepartingRuneHarborForPrimevalIsle), Cast::Sound(Pt(34381, -37680, -3610))] }),
        boat_sound: None,
        pay: Some(Pay { item_id: 8925, count: 1, oust: (34513, -38009, -3640) }),
        exec: Some(&RUNE_PRIMEVAL_RUNE_TO_PRIMEVAL),
        next_ms: None,
    },
    // case 1
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(10342, -27279, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryHasArrivedAtPrimevalIsle), Cast::Msg(SystemMessageId::TheFerryWillLeaveForRuneHarborAfterAnchoringForThreeMinutes), Cast::Sound(Pt(10342, -27279, -3610))] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(180000),
    },
    // case 2
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(10342, -27279, -3610), Pt(34381, -37680, -3610)), items: &[Cast::Msg(SystemMessageId::TheFerryIsNowDepartingPrimevalIsleForRuneHarbor), Cast::Sound(Pt(10342, -27279, -3610))] }),
        boat_sound: None,
        pay: Some(Pay { item_id: 8924, count: 1, oust: (10447, -24982, -3664) }),
        exec: Some(&RUNE_PRIMEVAL_PRIMEVAL_TO_RUNE),
        next_ms: None,
    },
    // case 3
    BoatStep {
        wait_dock: Some(WaitDock { dock: 3, near: (Pt(34381, -37680, -3610), Pt(10342, -27279, -3610)), busy: SystemMessageId::TheFerryFromPrimevalIsleToRuneHarborHasBeenDelayed }),
        dock_ops: &[],
        broadcast: None,
        boat_sound: None,
        pay: None,
        exec: Some(&RUNE_PRIMEVAL_RUNE_DOCK),
        next_ms: None,
    },
    // case 4
    BoatStep {
        wait_dock: None,
        dock_ops: &[(3, true)],
        broadcast: Some(Broadcast { near: (Pt(34381, -37680, -3610), Pt(10342, -27279, -3610)), items: &[Cast::Msg(SystemMessageId::WelcomeToRuneHarbor), Cast::Msg(SystemMessageId::TheFerryWillLeaveForPrimevalIsleAfterAnchoringForThreeMinutes), Cast::Sound(Pt(34381, -37680, -3610))] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(180000),
    },
];

// ---- BoatInnadrilTour ----

pub static INNADRIL_TOUR_TOUR: [PathPoint; 32] = [
    p(105129, 226240, -3610, 150, 800),
    p(90604, 238797, -3610, 150, 800),
    p(74853, 237943, -3610, 150, 800),
    p(68207, 235399, -3610, 150, 800),
    p(63226, 230487, -3610, 150, 800),
    p(61843, 224797, -3610, 150, 800),
    p(61822, 203066, -3610, 150, 800),
    p(59051, 197685, -3610, 150, 800),
    p(54048, 195298, -3610, 150, 800),
    p(41609, 195687, -3610, 150, 800),
    p(35821, 200284, -3610, 150, 800),
    p(35567, 205265, -3610, 150, 800),
    p(35617, 222471, -3610, 150, 800),
    p(37932, 226588, -3610, 150, 800),
    p(42932, 229394, -3610, 150, 800),
    p(74324, 245231, -3610, 150, 800),
    p(81872, 250314, -3610, 150, 800),
    p(101692, 249882, -3610, 150, 800),
    p(107907, 256073, -3610, 150, 800),
    p(112317, 257133, -3610, 150, 800),
    p(126273, 255313, -3610, 150, 800),
    p(128067, 250961, -3610, 150, 800),
    p(128520, 238249, -3610, 150, 800),
    p(126428, 235072, -3610, 150, 800),
    p(121843, 234656, -3610, 150, 800),
    p(120096, 234268, -3610, 150, 800),
    p(118572, 233046, -3610, 150, 800),
    p(117671, 228951, -3610, 150, 800),
    p(115936, 226540, -3610, 150, 800),
    p(113628, 226240, -3610, 150, 800),
    p(111300, 226240, -3610, 150, 800),
    p(111264, 226240, -3610, 150, 800),
];

static INNADRIL_TOUR_STEPS: [BoatStep; 10] = [
    // case 0
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatWillLeaveInFiveMinutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 1
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatWillLeaveInOneMinute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(40000),
    },
    // case 2
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatWillBeLeavingSoon)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(20000),
    },
    // case 3
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatIsLeaving), Cast::Sound(Pt(111264, 226240, -3610))] }),
        boat_sound: None,
        pay: Some(Pay { item_id: 0, count: 1, oust: (107092, 219098, -3952) }),
        exec: Some(&INNADRIL_TOUR_TOUR),
        next_ms: Some(650000),
    },
    // case 4
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatWillArriveInApproximately20Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 5
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatWillArriveInApproximately15Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 6
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatWillArriveInApproximately10Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
    // case 7
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatWillArriveInApproximately5Minutes)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(240000),
    },
    // case 8
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatWillArriveInApproximately1Minute)] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: None,
    },
    // case 9
    BoatStep {
        wait_dock: None,
        dock_ops: &[],
        broadcast: Some(Broadcast { near: (Pt(111264, 226240, -3610), Pt(111264, 226240, -3610)), items: &[Cast::Msg(SystemMessageId::TheInnadrilPleasureBoatHasArrivedItWillAnchorForTenMinutes), Cast::Sound(Pt(111264, 226240, -3610))] }),
        boat_sound: None,
        pay: None,
        exec: None,
        next_ms: Some(300000),
    },
];

/// The boats `main` of each script creates, in the order of the script files.
pub static BOAT_ROUTES: [BoatRoute; 5] = [
    BoatRoute {
        name: "BoatTalkingGludin",
        boat_id: 1,
        x: -96622,
        y: 261660,
        z: -3610,
        heading: 32768,
        steps: &TALKING_GLUDIN_STEPS,
        start_delay_ms: 180000,
        start_docks: &[(1, true)],
    },
    BoatRoute {
        name: "BoatGiranTalking",
        boat_id: 2,
        x: 48950,
        y: 190613,
        z: -3610,
        heading: 60800,
        steps: &GIRAN_TALKING_STEPS,
        start_delay_ms: 180000,
        start_docks: &[],
    },
    BoatRoute {
        name: "BoatGludinRune",
        boat_id: 3,
        x: -95686,
        y: 150514,
        z: -3610,
        heading: 16723,
        steps: &GLUDIN_RUNE_STEPS,
        start_delay_ms: 180000,
        start_docks: &[(2, true)],
    },
    BoatRoute {
        name: "BoatRunePrimeval",
        boat_id: 5,
        x: 34381,
        y: -37680,
        z: -3610,
        heading: 40785,
        steps: &RUNE_PRIMEVAL_STEPS,
        start_delay_ms: 180000,
        start_docks: &[(3, true)],
    },
    BoatRoute {
        name: "BoatInnadrilTour",
        boat_id: 4,
        x: 111264,
        y: 226240,
        z: -3610,
        heading: 32768,
        steps: &INNADRIL_TOUR_STEPS,
        start_delay_ms: 180000,
        start_docks: &[],
    },
];
