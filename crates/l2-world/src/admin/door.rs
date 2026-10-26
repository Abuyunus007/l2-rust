//! The door admin commands: `//open <doorId>`, `//close <doorId>`, `//open` and `//close` on the
//! target door, `//openall`, `//closeall`, and `//showdoors [off]`, which draws the doors near the
//! GM with the client's debug primitives.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminDoorControl.java
//
// # Left out, with the system that brings it
// - The coliseum doors `open1`..`open4`, `close1`..`close4` (their words are not in the list of
//   `AdminDoorControl`'s `ADMIN_COMMANDS`, so they answer "does not exist" like Java).
// - The castle doors are the residence doors in the world's door table, so one pass over the
//   doors covers Java's `DoorData` and `castle.getDoors()` loops. Java opens a castle door twice
//   (`openMe` again, which broadcasts again); the second call is a no-op here.
// - The shown doors of a GM are kept in a `BTreeSet`; Java's `HashSet<Integer>` iterates in hash
//   order, which only changes the order of the erase packets.

use l2_protocol_game::server::primitive::{ExServerPrimitive, PrimitiveLine, PrimitivePoint};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// `Color.BLACK`, `Color.GREEN` and `Color.RED` as `0xRRGGBB`.
const BLACK: i32 = 0x000000;
const GREEN: i32 = 0x00FF_00;
const RED: i32 = 0xFF_0000;
/// The z of the drawing: far under the map, so the primitive sits in the sky of the GM's view.
const DRAW_Z: i32 = -16_000;

/// Java `AdminDoorControl.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_open", admin_open),
        ("admin_close", admin_close),
        ("admin_openall", admin_openall),
        ("admin_closeall", admin_closeall),
        ("admin_showdoors", admin_showdoors),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The target when it is a door (Java `target.isDoor()`).
fn target_door(world: &World, gm: ObjectId) -> Option<ObjectId> {
    world.target_of(gm).filter(|t| world.door(*t).is_some())
}

/// Java `admin_open <doorId>` (with the space): the door of that id, which is a castle door too.
/// A value that is not a number does nothing (Java logs the exception).
fn open_or_close_by_id(world: &mut World, command: &str, prefix_len: usize, open: bool) {
    let Some(value) = command.get(prefix_len..) else {
        return;
    };
    let Ok(door_id) = parse_int(value) else {
        return;
    };
    if let Some(door) = world.door_by_door_id(door_id).map(|d| d.object_id) {
        if open {
            world.open_door(door);
        } else {
            world.close_door(door);
        }
    }
}

/// Java `AdminDoorControl.onCommand` for `admin_open`: `admin_open <id>` or the target door.
fn admin_open(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_open ") {
        open_or_close_by_id(world, command, 11, true);
    } else if command == "admin_open" {
        match target_door(world, gm) {
            Some(door) => world.open_door(door),
            None => world.send_sys_message(gm, "Incorrect target."),
        }
    }
    Ok(())
}

/// Java `AdminDoorControl.onCommand` for `admin_close`: `admin_close <id>` or the target door.
fn admin_close(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_close ") {
        open_or_close_by_id(world, command, 12, false);
    } else if command == "admin_close" {
        match target_door(world, gm) {
            Some(door) => world.close_door(door),
            None => world.send_sys_message(gm, "Incorrect target."),
        }
    }
    Ok(())
}

/// Java `admin_openall`: every door opens, the castle doors too.
fn admin_openall(world: &mut World, _gm: ObjectId, _command: &str) -> Outcome {
    let doors: Vec<ObjectId> = world.doors().map(|d| d.object_id).collect();
    for door in doors {
        world.open_door(door);
    }
    Ok(())
}

/// Java `admin_closeall`: every door closes, the castle doors too.
fn admin_closeall(world: &mut World, _gm: ObjectId, _command: &str) -> Outcome {
    let doors: Vec<ObjectId> = world.doors().map(|d| d.object_id).collect();
    for door in doors {
        world.close_door(door);
    }
    Ok(())
}

/// The primitive of one door, drawn in `color`: its box and its diagonals, as Java draws them.
fn door_primitive(
    world: &World,
    gm: ObjectId,
    door: ObjectId,
    color: i32,
    name_door: i32,
) -> Option<ExServerPrimitive> {
    let (gx, gy, _) = world.position_of(gm)?;
    let d = world.door(door)?;
    let t = &d.template;
    let (zmin, zmax) = (t.node_z, t.node_z + t.height);
    let mut packet = ExServerPrimitive::new(&format!("Door{name_door}"), gx, gy, DRAW_Z);
    let line = |packet: &mut ExServerPrimitive, a: (i32, i32, i32), b: (i32, i32, i32)| {
        packet.lines.push(PrimitiveLine {
            point: PrimitivePoint {
                name: String::new(),
                color,
                name_colored: false,
                x: a.0,
                y: a.1,
                z: a.2,
            },
            x2: b.0,
            y2: b.1,
            z2: b.2,
        });
    };
    let node = |i: usize, z: i32| (t.node_x[i], t.node_y[i], z);
    // box 1
    line(&mut packet, node(0, zmin), node(1, zmin));
    line(&mut packet, node(1, zmin), node(2, zmax));
    line(&mut packet, node(2, zmax), node(3, zmax));
    line(&mut packet, node(3, zmax), node(0, zmin));
    // box 2
    line(&mut packet, node(0, zmax), node(1, zmax));
    line(&mut packet, node(1, zmax), node(2, zmin));
    line(&mut packet, node(2, zmin), node(3, zmin));
    line(&mut packet, node(3, zmin), node(0, zmax));
    // diagonals
    line(&mut packet, node(0, zmin), node(1, zmax));
    line(&mut packet, node(2, zmin), node(3, zmax));
    line(&mut packet, node(0, zmax), node(1, zmin));
    line(&mut packet, node(2, zmax), node(3, zmin));
    Some(packet)
}

/// Java `admin_showdoors` on: the doors the GM sees, each drawn once (green when open, red when
/// closed) with "Found door <id>". `admin_showdoors off` erases what was drawn.
fn admin_showdoors(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.contains("off") {
        let Some(shown) = world.admin.shown_doors.remove(&gm) else {
            return Ok(());
        };
        let (gx, gy) = world.position_of(gm).map_or((0, 0), |(x, y, _)| (x, y));
        for door_id in shown {
            let mut packet = ExServerPrimitive::new(&format!("Door{door_id}"), gx, gy, DRAW_Z);
            packet.lines.push(PrimitiveLine {
                point: PrimitivePoint {
                    name: String::new(),
                    color: BLACK,
                    name_colored: false,
                    x: gx,
                    y: gy,
                    z: DRAW_Z,
                },
                x2: gx,
                y2: gy,
                z2: DRAW_Z,
            });
            world.send(gm, &packet);
        }
        return Ok(());
    }

    let visible: Vec<ObjectId> = world.visible_objects(gm);
    for id in visible {
        let Some(door) = world.door(id) else {
            continue;
        };
        let door_id = door.door_id;
        let color = if door.open { GREEN } else { RED };
        let shown = world.admin.shown_doors.entry(gm).or_default();
        if !shown.insert(door_id) {
            continue;
        }
        if let Some(packet) = door_primitive(world, gm, id, color, door_id) {
            world.send(gm, &packet);
        }
        world.send_sys_message(gm, &format!("Found door {door_id}"));
    }
    Ok(())
}
