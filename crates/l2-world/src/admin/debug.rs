//! `//debug`: the debug menu and the packet, door, geodata and movement switches.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminDebug.java
//
// Doors and geodata run the commands `//showdoors` and `//geogrid` again every time the GM has
// moved 15 units; movement draws the path of the walk the GM is on with `ExServerPrimitive`
// lines. The Java tasks are `ThreadPool.scheduleAtFixedRate`; here they are timers of the wheel
// that schedule themselves again.
//
// NOTE: `//debug packets` switches Java's console packet logging (`DevelopmentConfig.DEBUG_*`).
// The Rust server has no such logging (the packet inspector of the admin GUI replaces it), so the
// switch only keeps the state the menu shows.

use std::collections::HashMap;

use l2_geo::GeoLocation;
use l2_protocol_game::server::{ExServerPrimitive, NpcHtmlMessage, PrimitiveColor};
use l2_util::ObjectId;

use super::control::AdminTimer;
use super::{AdminHandlers, JavaException};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// `DEBUG_DOOR_DELAY`.
const DOOR_DELAY_MS: u64 = 3000;
/// `DEBUG_GEO_DELAY`.
const GEO_DELAY_MS: u64 = 1500;
/// `DEBUG_MOVE_DELAY`.
const MOVE_DELAY_MS: u64 = 100;
/// The distance the GM must move before the drawing is renewed.
const RENEW_DISTANCE: f64 = 15.0;
/// Java `Color.BLUE`, the nodes of a geodata path (the packet type has no such constant).
const BLUE: PrimitiveColor = PrimitiveColor(0xFF00_00FF_u32 as i32);

/// The timers of `//debug`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugTimer {
    /// The periodic check of `//debug doors`.
    Doors(ObjectId),
    /// The periodic check of `//debug geodata`.
    Geo(ObjectId),
    /// The periodic check of `//debug movement`.
    Movement(ObjectId),
    /// `//showdoors off`, 3.1 seconds after the switch went off.
    DoorsOff(ObjectId),
    /// `//geogrid off`, 1.6 seconds after the switch went off.
    GeoOff(ObjectId),
    /// `clearMoveLine`, 1.6 seconds after the switch went off.
    ClearMoveLine(ObjectId),
}

/// A running switch: its task and the place of the GM when it last drew.
#[derive(Debug)]
struct Running {
    timer: TimerId,
    location: Option<(i32, i32, i32)>,
}

/// The maps of `AdminDebug`.
#[derive(Default)]
pub(crate) struct DebugState {
    doors: HashMap<ObjectId, Running>,
    geo: HashMap<ObjectId, Running>,
    movement: HashMap<ObjectId, Running>,
    /// `PLAYER_MOVE_TO_LOCATIONS`.
    move_to: HashMap<ObjectId, (i32, i32, i32)>,
    /// `PLAYER_MOVE_PATHS`.
    move_paths: HashMap<ObjectId, Vec<GeoLocation>>,
}

/// Java `AdminDebug` registration.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    handlers.register("admin_debug", admin_debug);
}

/// 2D distance, Java `LocationUtil.calculateDistance(a, b, false, false)`.
fn distance(a: (i32, i32, i32), b: (i32, i32, i32)) -> f64 {
    let (dx, dy) = (f64::from(a.0) - f64::from(b.0), f64::from(a.1) - f64::from(b.1));
    (dx * dx + dy * dy).sqrt()
}

/// Java `AdminDebug.onCommand`.
fn admin_debug(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `new StringTokenizer(command.toLowerCase(), " ")`, the command word skipped.
    let lowered = command.to_lowercase();
    let mut tokens = lowered.split(' ').filter(|t| !t.is_empty());
    tokens.next();
    let Some(sub) = tokens.next() else {
        show_menu(world, gm)?;
        return Ok(());
    };
    let value = tokens.next();
    let menu = command.contains("menu");
    match sub {
        "packet" | "packets" => {
            let enabled = match value {
                None => !world.admin.ctl.settings.debug_client_packets,
                Some(value) => value == "on",
            };
            set_packet_debugging(world, gm, enabled);
        }
        "door" | "doors" => {
            let enabled = value.map_or(!world.admin.ctl.debug.doors.contains_key(&gm), |v| v == "on");
            set_door_debugging(world, gm, enabled);
        }
        "geo" | "geodata" => {
            let enabled = value.map_or(!world.admin.ctl.debug.geo.contains_key(&gm), |v| v == "on");
            set_geodata_debugging(world, gm, enabled);
        }
        "move" | "movement" | "path" | "pathfind" => {
            let enabled = value.map_or(!world.admin.ctl.debug.movement.contains_key(&gm), |v| v == "on");
            set_movement_debugging(world, gm, enabled);
        }
        _ => {
            world.send_sys_message(gm, "Usage: //debug <parameter> <value>");
            return Ok(());
        }
    }
    // `if (command.contains("menu")) showMenu` only exists with a value.
    if value.is_some() && menu {
        show_menu(world, gm)?;
    }
    Ok(())
}

/// Java `AdminDebug.showMenu`.
fn show_menu(world: &mut World, gm: ObjectId) -> Outcome {
    let Some(mut content) = world.get_htm(gm, "data/html/admin/debug.htm") else {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    };
    let state = &world.admin.ctl.debug;
    let (packets_status, packets) = if world.admin.ctl.settings.debug_client_packets {
        ("Disable", "packets off")
    } else {
        ("Enable", "packets on")
    };
    content = content
        .replace("%packets_status%", packets_status)
        .replace("%packets%", packets);
    let (status, doors) = if state.doors.contains_key(&gm) {
        ("Disable", "doors off")
    } else {
        ("Enable", "doors on")
    };
    content = content
        .replace("%doors_status%", status)
        .replace("%doors%", doors);
    let (status, geodata) = if state.geo.contains_key(&gm) {
        ("Disable", "geodata off")
    } else {
        ("Enable", "geodata on")
    };
    content = content
        .replace("%geodata_status%", status)
        .replace("%geodata%", geodata);
    let (status, movement) = if state.movement.contains_key(&gm) {
        ("Disable", "movement off")
    } else {
        ("Enable", "movement on")
    };
    content = content
        .replace("%movement_status%", status)
        .replace("%movement%", movement);
    // `new NpcHtmlMessage()` + `setHtml(content)`.
    if let Some(message) = NpcHtmlMessage::new(0, &content) {
        world.send_html(gm, &message);
    }
    Ok(())
}

fn set_packet_debugging(world: &mut World, gm: ObjectId, enabled: bool) {
    world.admin.ctl.settings.debug_client_packets = enabled;
    world.send_sys_message(
        gm,
        &format!(
            "Packet debugging on console is {}",
            if enabled { "enabled." } else { "disabled." }
        ),
    );
}

impl World {
    fn debug_schedule(&mut self, timer: DebugTimer, delay_ms: u64) -> TimerId {
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        self.timers
            .schedule_at(due, TimerKind::Admin(AdminTimer::Debug(timer)))
    }

    fn debug_position(&self, id: ObjectId) -> Option<(i32, i32, i32)> {
        self.players.get(&id).map(|p| (p.core.x, p.core.y, p.core.z))
    }

    /// Stops the switches of a player who left the world (Java notices at the next task run).
    pub(crate) fn debug_forget(&mut self, id: ObjectId) {
        let state = &mut self.admin.ctl.debug;
        let timers: Vec<TimerId> = [&mut state.doors, &mut state.geo, &mut state.movement]
            .into_iter()
            .filter_map(|map| map.remove(&id))
            .map(|running| running.timer)
            .collect();
        state.move_to.remove(&id);
        state.move_paths.remove(&id);
        for timer in timers {
            self.timers.cancel(timer);
        }
    }

    /// Dispatch of `AdminTimer::Debug`.
    pub(crate) fn on_debug_timer(&mut self, timer: DebugTimer) {
        match timer {
            DebugTimer::Doors(id) => self.debug_doors_step(id),
            DebugTimer::Geo(id) => self.debug_geo_step(id),
            DebugTimer::Movement(id) => self.debug_movement_step(id),
            DebugTimer::DoorsOff(id) => self.admin_command(id, "admin_showdoors off", false),
            DebugTimer::GeoOff(id) => self.admin_command(id, "admin_geogrid off", false),
            DebugTimer::ClearMoveLine(id) => self.clear_move_line(id),
        }
    }

    /// One run of the door task.
    fn debug_doors_step(&mut self, id: ObjectId) {
        let Some(here) = self.debug_position(id) else {
            self.debug_forget(id);
            return;
        };
        let timer = self.debug_schedule(DebugTimer::Doors(id), DOOR_DELAY_MS);
        let Some(running) = self.admin.ctl.debug.doors.get_mut(&id) else {
            self.timers.cancel(timer);
            return;
        };
        running.timer = timer;
        let moved = running
            .location
            .is_none_or(|old| distance(here, old) > RENEW_DISTANCE);
        if moved {
            running.location = Some(here);
            self.admin_command(id, "admin_showdoors", false);
        }
    }

    /// One run of the geodata task.
    fn debug_geo_step(&mut self, id: ObjectId) {
        let Some(here) = self.debug_position(id) else {
            self.debug_forget(id);
            return;
        };
        let timer = self.debug_schedule(DebugTimer::Geo(id), GEO_DELAY_MS);
        let has_path = self.admin.ctl.debug.move_paths.contains_key(&id);
        let Some(running) = self.admin.ctl.debug.geo.get_mut(&id) else {
            self.timers.cancel(timer);
            return;
        };
        running.timer = timer;
        let moved = running
            .location
            .is_none_or(|old| distance(here, old) > RENEW_DISTANCE);
        if !has_path && moved {
            running.location = Some(here);
            self.admin_command(id, "admin_geogrid", false);
        }
    }

    /// One run of the movement task.
    fn debug_movement_step(&mut self, id: ObjectId) {
        let Some(here) = self.debug_position(id) else {
            self.debug_forget(id);
            return;
        };
        let timer = self.debug_schedule(DebugTimer::Movement(id), MOVE_DELAY_MS);
        let Some(running) = self.admin.ctl.debug.movement.get_mut(&id) else {
            self.timers.cancel(timer);
            return;
        };
        running.timer = timer;
        let location = running.location;
        let drawn = self.admin.ctl.debug.move_to.contains_key(&id)
            || self.admin.ctl.debug.move_paths.contains_key(&id);
        if location.is_none_or(|old| distance(here, old) > RENEW_DISTANCE) {
            self.draw_move_line(id);
        } else if drawn {
            self.clear_move_line(id);
        }
    }

    /// Java `AdminDebug.drawMoveLine`.
    fn draw_move_line(&mut self, id: ObjectId) {
        let Some(here) = self.debug_position(id) else {
            return;
        };
        let Some(mv) = self.players.get(&id).and_then(|p| p.movement.mv.clone()) else {
            // `!player.isMoving()`
            if let Some(running) = self.admin.ctl.debug.movement.get_mut(&id) {
                running.location = Some(here);
            }
            return;
        };
        let destination = (mv.x_destination, mv.y_destination, mv.z_destination);
        match mv.geo_path {
            Some(path) => {
                let previous = self.admin.ctl.debug.move_paths.get(&id);
                if previous.is_none_or(|old| *old != path) {
                    let mut primitive = ExServerPrimitive::new("DebugMove", here.0, here.1, -16000);
                    primitive.add_line(PrimitiveColor::GREEN, here, destination);
                    for pair in path.windows(2) {
                        let (current, next) = (pair[0], pair[1]);
                        primitive.add_line(
                            BLUE,
                            (current.x(), current.y(), current.z()),
                            (next.x(), next.y(), next.z()),
                        );
                    }
                    self.send(id, &primitive);
                    self.admin.ctl.debug.move_paths.insert(id, path);
                }
            }
            None => {
                let far = self
                    .admin
                    .ctl
                    .debug
                    .move_to
                    .get(&id)
                    .is_none_or(|old| distance(destination, *old) > RENEW_DISTANCE);
                if far {
                    let mut primitive = ExServerPrimitive::new("DebugMove", here.0, here.1, -16000);
                    primitive.add_line(PrimitiveColor::GREEN, here, destination);
                    self.send(id, &primitive);
                    self.admin.ctl.debug.move_paths.remove(&id);
                    self.admin.ctl.debug.move_to.insert(id, destination);
                }
            }
        }
    }

    /// Java `AdminDebug.clearMoveLine`.
    fn clear_move_line(&mut self, id: ObjectId) {
        let moving = self.players.get(&id).is_some_and(|p| p.movement.mv.is_some());
        if self.admin.ctl.debug.move_paths.contains_key(&id) && moving {
            return;
        }
        let Some(here) = self.debug_position(id) else {
            return;
        };
        let mut primitive = ExServerPrimitive::new("DebugMove", here.0, here.1, -16000);
        primitive.add_line(
            PrimitiveColor::BLACK,
            (here.0, here.1, -16000),
            (here.0, here.1, -16000),
        );
        self.send(id, &primitive);
        self.admin.ctl.debug.move_to.remove(&id);
        self.admin.ctl.debug.move_paths.remove(&id);
    }
}

/// Java `AdminDebug.setDoorDebugging`.
fn set_door_debugging(world: &mut World, gm: ObjectId, enabled: bool) {
    if enabled {
        if !world.admin.ctl.debug.doors.contains_key(&gm) {
            world.admin_command(gm, "admin_showdoors", false);
            let location = world.debug_position(gm);
            let timer = world.debug_schedule(DebugTimer::Doors(gm), DOOR_DELAY_MS);
            world
                .admin
                .ctl
                .debug
                .doors
                .insert(gm, Running { timer, location });
        }
    } else if let Some(running) = world.admin.ctl.debug.doors.remove(&gm) {
        world.timers.cancel(running.timer);
        world.debug_schedule(DebugTimer::DoorsOff(gm), DOOR_DELAY_MS + 100);
    }
    world.send_sys_message(
        gm,
        &format!(
            "Door debugging is {}",
            if enabled { "enabled." } else { "disabled." }
        ),
    );
}

/// Java `AdminDebug.setGeodataDebugging`.
fn set_geodata_debugging(world: &mut World, gm: ObjectId, enabled: bool) {
    if enabled {
        if !world.admin.ctl.debug.geo.contains_key(&gm) {
            world.admin_command(gm, "admin_geogrid", false);
            let location = world.debug_position(gm);
            let timer = world.debug_schedule(DebugTimer::Geo(gm), GEO_DELAY_MS);
            world.admin.ctl.debug.geo.insert(gm, Running { timer, location });
        }
    } else if let Some(running) = world.admin.ctl.debug.geo.remove(&gm) {
        world.timers.cancel(running.timer);
        world.debug_schedule(DebugTimer::GeoOff(gm), GEO_DELAY_MS + 100);
    }
    world.send_sys_message(
        gm,
        &format!(
            "Geodata debugging is {}",
            if enabled { "enabled." } else { "disabled." }
        ),
    );
}

/// Java `AdminDebug.setMovementDebugging`.
fn set_movement_debugging(world: &mut World, gm: ObjectId, enabled: bool) {
    if enabled {
        if !world.admin.ctl.debug.movement.contains_key(&gm) {
            if world.players.get(&gm).is_some_and(|p| p.movement.mv.is_some()) {
                world.send_sys_message(gm, "Cannot start debugging while moving.");
                return;
            }
            // `drawMoveLine` before the task exists: the GM stands still, so it only notes the
            // place.
            let location = world.debug_position(gm);
            let timer = world.debug_schedule(DebugTimer::Movement(gm), MOVE_DELAY_MS);
            world
                .admin
                .ctl
                .debug
                .movement
                .insert(gm, Running { timer, location });
        }
    } else if let Some(running) = world.admin.ctl.debug.movement.remove(&gm) {
        world.timers.cancel(running.timer);
        // NOTE(java-quirk): Java forgets the place with `PLAYER_MOVE_LOCATIONS.remove`; the
        // running record held it here.
        world.debug_schedule(DebugTimer::ClearMoveLine(gm), GEO_DELAY_MS + 100);
    }
    world.send_sys_message(
        gm,
        &format!(
            "Movement debugging is {}",
            if enabled { "enabled." } else { "disabled." }
        ),
    );
}
