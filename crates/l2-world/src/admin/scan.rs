//! `//scan [id=<npcId>] [name=<text>] [radius=<n> | range=<n>] [page=<n>]`: the NPCs around the GM
//! in `scan.htm` (15 a page, each with a Delete link), and `//deleteNpcByObjectId objectId=<oid>`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminScan.java
//
// # Left out, with the system that brings it
// - The order of the list: Java takes the NPCs of the surrounding regions in their region order;
//   here they are listed by object id.
// - `SpawnData.deleteSpawn` does not write the datapack, as in `//delete`; the raid boss table is
//   `RaidBossSpawnManager.deleteSpawn` (M8-BOSS-01).

use l2_util::ObjectId;

use super::edit_char::next_prev_pager;
use super::html::build_page;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::target::distance_2d;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// One NPC of the scan: object id, template id, name, place and the distance from the GM.
type ScanRow = (ObjectId, i32, String, (i32, i32, i32), i64);

/// Java `AdminScan.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_scan", admin_scan),
        ("admin_deleteNpcByObjectId", admin_delete_npc_by_object_id),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminScan.DEFAULT_RADIUS`.
const DEFAULT_RADIUS: i32 = 1000;
/// The NPCs of a page of the scan.
const PER_PAGE: i32 = 15;

/// Java `parseInt(command, paramName, defaultValue)` and `parseString`: the first `name=` in the
/// command (its value runs to the next white space), `None` when there is none.
fn param<'a>(command: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=");
    let start = command.find(&key)? + key.len();
    let rest = &command[start..];
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

/// Java `parseInt(command, paramName, defaultValue)`: an unreadable value is the default.
fn int_param(command: &str, name: &str, default: i32) -> i32 {
    param(command, name)
        .and_then(|value| parse_int(value.trim()).ok())
        .unwrap_or(default)
}

/// Java `AdminScan.onCommand`, `admin_scan`.
fn admin_scan(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let id = int_param(command, "id", 0);
    let name = param(command, "name").map(|n| n.trim().to_lowercase());
    let radius = int_param(command, "radius", int_param(command, "range", DEFAULT_RADIUS));
    let page = int_param(command, "page", 0);

    let mut npcs: Vec<ObjectId> = world
        .visible_objects_in_range(gm, radius)
        .into_iter()
        .filter(|id| world.npcs.contains_key(id))
        .collect();
    npcs.sort_by_key(|id| id.0);
    let npcs: Vec<ObjectId> = npcs
        .into_iter()
        .filter(|npc| {
            let Some(template) = world.npcs.get(npc).map(|n| n.npc_id) else {
                return false;
            };
            if id > 0 {
                return template == id;
            }
            match &name {
                Some(prefix) => world
                    .object_name(*npc)
                    .unwrap_or_default()
                    .to_lowercase()
                    .starts_with(prefix.as_str()),
                None => true,
            }
        })
        .collect();

    let gm_position = world.position_of(gm).unwrap_or((0, 0, 0));
    let rows: Vec<ScanRow> = npcs
        .iter()
        .map(|npc| {
            let template = world.npcs.get(npc).map_or(0, |n| n.npc_id);
            let position = world.position_of(*npc).unwrap_or((0, 0, 0));
            let name = world.object_name(*npc).unwrap_or_default();
            let distance = distance_2d(gm_position, position).round() as i64;
            (*npc, template, name, position, distance)
        })
        .collect();

    let bypass = "bypass -h admin_scan";
    let result = build_page(&rows, PER_PAGE, bypass, page, |row, sb| {
        let (object_id, template, name, (x, y, z), distance) = row;
        let shown = if name.is_empty() {
            "No name NPC"
        } else {
            name.as_str()
        };
        sb.push_str("<tr>");
        sb.push_str(&format!("<td width=\"45\">{template}</td>"));
        sb.push_str(&format!(
            "<td><a action=\"bypass -h admin_move_to {x} {y} {z}\">{shown}</a></td>"
        ));
        sb.push_str(&format!("<td width=\"60\">{}</td>", format_adena(*distance)));
        sb.push_str(&format!(
            "<td width=\"54\"><a action=\"bypass -h admin_deleteNpcByObjectId objectId={}\"><font color=\"LEVEL\">Delete</font></a></td>",
            object_id.0
        ));
        sb.push_str("</tr>");
    });
    let mut html = world.admin_html_file(gm, "data/html/admin/scan.htm");
    let pager = if result.pages > 1 {
        format!(
            "<center><table width=\"100%\" cellspacing=0><tr>{}</tr></table></center>",
            next_prev_pager(bypass, page, result.pages - 1)
        )
    } else {
        String::new()
    };
    html.replace("%pages%", &pager);
    html.replace("%data%", &result.body);
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `FormatUtil.formatAdena` (`NumberFormat.getIntegerInstance(ENGLISH)`): thousands with a
/// comma.
fn format_adena(amount: i64) -> String {
    let digits = amount.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if amount < 0 { format!("-{out}") } else { out }
}

/// Java `AdminScan.onCommand`, `admin_deleteNpcByObjectId objectId=<oid>`.
fn admin_delete_npc_by_object_id(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `!st.hasMoreElements()`: the command word alone.
    if command.trim() == "admin_deleteNpcByObjectId" {
        world.send_sys_message(gm, "Usage: //deletenpcbyobjectid objectId=<object_id>");
        return Ok(());
    }
    let object_id = int_param(command, "objectId", 0);
    if object_id == 0 {
        world.send_sys_message(gm, "objectId is not set or invalid!");
        return Ok(());
    }
    let npc = ObjectId(object_id);
    let Some(index) = world.npcs.get(&npc).map(|n| n.spawn) else {
        world.send_sys_message(gm, "NPC does not exist or object_id does not belong to an NPC");
        return Ok(());
    };
    let name = world.object_name(npc).unwrap_or_default();
    let template_id = world.npcs.get(&npc).map(|n| n.npc_id);
    world.delete_npc(npc);
    if let Some(index) = index {
        world.stop_respawn(index);
        // `RaidBossSpawnManager.isDefined(spawn.getId())`: the raid spawn and its row go (M8-BOSS-01).
        if let Some(id) = template_id.filter(|id| world.raid_boss_defined(*id)) {
            world.raid_delete_spawn(id, true);
        }
    }
    world.send_message(gm, &format!("{name} has been deleted."));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_are_found_like_the_java_regex() {
        let command = "admin_scan id=5 radius=300 name=Big";
        assert_eq!(param(command, "id"), Some("5"));
        assert_eq!(param(command, "name"), Some("Big"));
        assert_eq!(int_param(command, "radius", 1), 300);
        assert_eq!(int_param(command, "range", 7), 7);
        assert_eq!(
            int_param("admin_scan id=x", "id", 9),
            9,
            "a bad number is the default"
        );
        assert_eq!(param("admin_scan id=", "id"), None, "the value needs a character");
    }

    #[test]
    fn adena_has_thousand_separators() {
        assert_eq!(format_adena(0), "0");
        assert_eq!(format_adena(999), "999");
        assert_eq!(format_adena(1000), "1,000");
        assert_eq!(format_adena(1_234_567), "1,234,567");
        assert_eq!(format_adena(-2500), "-2,500");
    }
}
