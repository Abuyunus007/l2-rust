//! `BoatManager` and the ferry scripts' engine.
// Java: managers/BoatManager.java
// Java: dist/game/data/scripts/vehicles/{BoatTalkingGludin,BoatGiranTalking,BoatGludinRune,
//       BoatRunePrimeval,BoatInnadrilTour}.java (`main`, `run`)
//
// The five scripts are one machine over data (`boat_routes.rs`): a `case` is a [`BoatStep`] and
// runs its statements in Java's order. A case without a delay is run again by the boat's arrival at
// the end of its path (`Vehicle.moveToNextRoutePoint` -> `runEngine(10)`), as in Java.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, CreatureSay, PlaySound};
use l2_protocol_game::server::chat::{SayBody, SenderField};
use l2_util::ObjectId;
use tracing::warn;

use super::boat_routes::BOAT_ROUTES;
use super::route::{BoatStep, Cast, Pt};
use super::{Engine, VehicleKind};
use crate::world::World;

/// The `801` of every `new CreatureSay(ChatType.BOAT, 801, id)`.
const BOAT_SAY_CHAR_ID: i32 = 801;
/// `if (_shoutCount > 35) _shoutCount = 0`: a delayed boat shouts again after 36 checks of 5 s.
const SHOUT_REPEAT: i32 = 35;
/// `ThreadPool.schedule(this, 5000)` of a busy dock.
const BUSY_RETRY_MS: u64 = 5000;

impl World {
    /// The `main` of the five boat scripts: each creates its boat, hands it the script and starts
    /// it 3 minutes later. Nothing happens with `AllowBoat = False` (`getNewBoat` is `null`).
    pub fn spawn_boats(&mut self) {
        if !self.vehicle.settings.allow_boat {
            return;
        }
        for (index, route) in BOAT_ROUTES.iter().enumerate() {
            // `BoatManager.getNewBoat(boatId, x, y, z, heading)`
            let Some(boat) =
                self.create_vehicle(VehicleKind::Boat, (route.x, route.y, route.z), route.heading, None, |_| {})
            else {
                warn!("{}: no boat could be created.", route.name);
                continue;
            };
            self.vehicle.boats.push(boat);
            if let Some(v) = self.veh_mut(boat) {
                v.engine = Some(Engine::Boat {
                    route: index,
                    cycle: 0,
                    shout_count: 0,
                });
            }
            self.vehicle_run_engine(boat, route.start_delay_ms);
            for (dock, busy) in route.start_docks {
                self.dock_ship(*dock, *busy);
            }
        }
    }

    /// Java `BoatManager.dockShip(dock, value)`: a dock number outside the three flags is
    /// ignored (the exception is caught).
    pub(crate) fn dock_ship(&mut self, dock: i32, busy: bool) {
        // NOTE(java-quirk): `_docksBusy` has 3 entries and `RUNE_HARBOR` is 3, so the Rune harbor
        // can be neither locked nor busy.
        if let Some(flag) = usize::try_from(dock)
            .ok()
            .and_then(|d| self.vehicle.docks_busy.get_mut(d))
        {
            *flag = busy;
        }
    }

    /// Java `BoatManager.dockBusy(dock)`: `false` for a dock outside the flags.
    pub fn dock_busy(&self, dock: i32) -> bool {
        usize::try_from(dock)
            .ok()
            .and_then(|d| self.vehicle.docks_busy.get(d))
            .copied()
            .unwrap_or(false)
    }

    /// `new CreatureSay(ChatType.BOAT, 801, id)`.
    fn boat_say(id: SystemMessageId) -> CreatureSay {
        CreatureSay {
            sender_object_id: 0,
            chat_type: ChatType::Boat,
            sender: SenderField::CharId(BOAT_SAY_CHAR_ID),
            message_id: id.id(),
            body: SayBody::Parameters(Vec::new()),
        }
    }

    /// Java `BoatManager.broadcastPackets(point1, point2, packets...)`: every player within
    /// `BoatBroadcastRadius` (x and y only) of the first place, else of the second one, gets all
    /// the packets.
    fn boat_broadcast(&self, boat: ObjectId, near: (Pt, Pt), items: &[Cast]) {
        let radius = f64::from(self.vehicle.settings.boat_broadcast_radius);
        let rad_sq = radius * radius;
        let packets: Vec<Vec<u8>> = items
            .iter()
            .map(|item| match item {
                Cast::Msg(id) => l2_net::Encode::to_bytes(&Self::boat_say(*id)),
                Cast::Sound(at) => l2_net::Encode::to_bytes(&ship_sound(boat, *at)),
            })
            .collect();
        let inside = |px: i32, py: i32, point: Pt| {
            let dx = f64::from(px.wrapping_sub(point.0));
            let dy = f64::from(py.wrapping_sub(point.1));
            dx * dx + dy * dy < rad_sq
        };
        for player in self.players.values() {
            let (px, py) = (player.core.x, player.core.y);
            if inside(px, py, near.0) || inside(px, py, near.1) {
                for bytes in &packets {
                    self.send_bytes(player.core.object_id, bytes.clone());
                }
            }
        }
    }

    /// The `run()` of a ferry script.
    pub(crate) fn boat_engine_run(&mut self, boat: ObjectId) {
        let Some(Engine::Boat {
            route,
            cycle,
            shout_count,
        }) = self.veh(boat).and_then(|v| v.engine.clone())
        else {
            return;
        };
        let Some(steps) = BOAT_ROUTES.get(route).map(|r| r.steps) else {
            return;
        };
        let Some(step) = steps.get(cycle).copied() else {
            return;
        };
        let mut shout_count = shout_count;
        if self.boat_step(boat, &step, &mut shout_count) {
            // `return` out of the busy dock check: the case comes again, `_cycle` stays.
            self.set_boat_engine(boat, cycle, shout_count);
            return;
        }
        // `_shoutCount = 0; _cycle++; if (_cycle > last) _cycle = 0;`
        let next = if cycle + 1 >= steps.len() { 0 } else { cycle + 1 };
        self.set_boat_engine(boat, next, 0);
    }

    fn set_boat_engine(&mut self, boat: ObjectId, cycle: usize, shout_count: i32) {
        if let Some(Engine::Boat {
            cycle: c,
            shout_count: s,
            ..
        }) = self.veh_mut(boat).and_then(|v| v.engine.as_mut())
        {
            *c = cycle;
            *s = shout_count;
        }
    }

    /// One `case`. `true` when the dock is busy and the case waits.
    fn boat_step(&mut self, boat: ObjectId, step: &BoatStep, shout_count: &mut i32) -> bool {
        if let Some(wait) = &step.wait_dock
            && self.dock_busy(wait.dock)
        {
            if *shout_count == 0 {
                self.boat_broadcast(boat, wait.near, &[Cast::Msg(wait.busy)]);
            }
            *shout_count += 1;
            if *shout_count > SHOUT_REPEAT {
                *shout_count = 0;
            }
            self.vehicle_run_engine(boat, BUSY_RETRY_MS);
            return true;
        }
        for (dock, busy) in step.dock_ops {
            self.dock_ship(*dock, *busy);
        }
        if let Some(broadcast) = &step.broadcast {
            self.boat_broadcast(boat, broadcast.near, broadcast.items);
        }
        if let Some(at) = step.boat_sound {
            self.broadcast_packet(boat, &ship_sound(boat, at));
        }
        if let Some(pay) = step.pay {
            self.vehicle_pay_for_ride(boat, pay);
        }
        if let Some(path) = step.exec {
            self.vehicle_execute_path(boat, Some(path));
        }
        if let Some(ms) = step.next_ms {
            self.vehicle_run_engine(boat, ms);
        }
        false
    }
}

/// `new PlaySound(0, "itemsound.ship_arrival_departure", 1, boatId, x, y, z)`.
fn ship_sound(boat: ObjectId, at: Pt) -> PlaySound {
    PlaySound {
        unknown1: 0,
        sound_file: "itemsound.ship_arrival_departure".to_owned(),
        unknown3: 1,
        unknown4: boat.0,
        unknown5: at.0,
        unknown6: at.1,
        unknown7: at.2,
        unknown8: 0,
    }
}
