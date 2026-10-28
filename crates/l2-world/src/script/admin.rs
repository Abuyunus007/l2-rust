//! `//show_quests` and `//quest_info`: the GM's view of what the scripts listen to (S-API-03).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminQuest.java
//
// # Left out
// - `//quest_reload`, `//script_load` and `//script_unload` need the script engine to reload code
//   (D-16: the scripts are compiled in, a change is a restart).
// - `//quest_info` prints the "Path" of the Java class (its package); here it is the folder of
//   the Rust module, which is the script's `path()`.

use std::collections::BTreeSet;

use l2_util::ObjectId;

use super::setup::RegisterType;
use crate::admin::html::AdminHtml;
use crate::admin::{AdminHandlers, JavaException};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminQuest.ADMIN_COMMANDS` (the words this port has).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    handlers.register("admin_show_quests", admin_show_quests);
    handlers.register("admin_quest_info", admin_quest_info);
}

const PAGE: &str = "data/html/admin/npc-quests.htm";

fn admin_show_quests(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(target) = world.target_of(gm) else {
        world.send_sys_message(gm, "Get a target first.");
        return Ok(());
    };
    if !world.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
        world.send_sys_message(gm, "Invalid Target.");
        return Ok(());
    }
    // `creature.getListeners(type)` of the NPC template: the scripts with an NPC-type listener
    // for its id (a player has none of its own). Sorted by name, like Java's `TreeSet`.
    let mut names = BTreeSet::new();
    if let Some(npc) = world.npcs.get(&target) {
        for script in world.scripts.ids() {
            let hears = world
                .scripts
                .listeners
                .registered_all(script)
                .iter()
                .any(|(kind, id)| kind.register_type() == RegisterType::Npc && *id == npc.npc_id);
            if hears {
                names.insert(world.scripts.name_of(script));
            }
        }
    }
    let mut sb = String::new();
    for name in names {
        sb.push_str(&format!(
            "<tr><td colspan=\"4\"><font color=\"LEVEL\"><a action=\"bypass -h admin_quest_info {name}\">{name}</a></font></td></tr>"
        ));
    }
    let mut page = world.admin_html_file(gm, PAGE);
    page.replace("%quests%", &sb);
    page.replace("%objid%", &target.0.to_string());
    page.replace("%questName%", "");
    world.send_admin_html(gm, &page);
    Ok(())
}

/// One framed line of the info page.
fn row(label: &str, value: &str) -> String {
    format!(
        "<tr><td colspan=\"4\"><table width=270 border=0 bgcolor=131210><tr><td width=270><font color=\"LEVEL\">{label}:</font> <font color=00FF00>{value}</font></td></tr></table></td></tr>"
    )
}

fn admin_quest_info(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `command.startsWith("admin_quest_info ")`.
    let Some(quest_name) = command.strip_prefix("admin_quest_info ") else {
        return Ok(());
    };
    let Some(script) = world.scripts.by_name(quest_name) else {
        world.send_sys_message(
            gm,
            &format!("Couldn't find quest or script with name {quest_name} !"),
        );
        return Ok(());
    };
    let mut counter = 0;
    let mut events = String::new();
    let mut kinds: Vec<(u32, &'static str)> = world
        .scripts
        .listeners
        .registered_kinds(script)
        .into_iter()
        .map(|k| k.java_event_type())
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    let type_count = kinds.len();
    for (_, name) in &kinds {
        events.push_str(&format!(", {name}"));
        counter += 1;
        if counter > 10 {
            counter = 0;
            break;
        }
    }
    let mut npc_ids = BTreeSet::new();
    for (kind, id) in world.scripts.listeners.registered_all(script) {
        if kind.register_type() == RegisterType::Npc {
            npc_ids.insert(id);
        }
    }
    let mut npcs = String::new();
    for id in &npc_ids {
        npcs.push_str(&format!(", {id}"));
        counter += 1;
        if counter > 50 {
            counter = 0;
            break;
        }
    }
    if !events.is_empty() {
        events = format!("{type_count}: {}", &events[2..]);
    }
    if !npcs.is_empty() {
        npcs = format!("{}: {}", npc_ids.len(), &npcs[2..]);
    }
    let quest_items = world.scripts.quest_items(script).to_vec();
    let mut items = String::new();
    if !quest_items.is_empty() {
        for id in &quest_items {
            items.push_str(&format!(", {id}"));
            counter += 1;
            if counter > 20 {
                counter = 0;
                break;
            }
        }
        items = format!("{}:{}", quest_items.len(), &items[2..]);
    }
    let mut timers = String::new();
    for (name, npc, player, repeating) in world.quest_timers_of(script) {
        let player_text = player.map_or_else(
            || "null".to_owned(),
            |p| {
                let name = world.players.get(&p).map_or("", |pl| pl.data.name.as_str());
                format!("Player:{name}[{}]", p.0)
            },
        );
        let npc_text = npc.map_or_else(
            || "null".to_owned(),
            |n| {
                let t = world.npc_world.template(n.id);
                format!(
                    "{}:{}({})[{}]",
                    t.map_or("Npc", |t| t.npc_type.as_str()),
                    t.map_or("", |t| t.name.as_str()),
                    n.id,
                    n.object_id.0
                )
            },
        );
        timers.push_str(&format!(
            "<tr><td colspan=\"4\"><table width=270 border=0 bgcolor=131210><tr><td width=270><font color=\"LEVEL\">{name}:</font> <font color=00FF00>Active: true Repeatable: {repeating} Player: {player_text} Npc: {npc_text}</font></td></tr></table></td></tr>"
        ));
        counter += 1;
        if counter > 10 {
            break;
        }
    }
    let name = world.scripts.name_of(script);
    let mut sb = String::new();
    sb.push_str(&row("ID", &world.scripts.quest_id_of(script).to_string()));
    sb.push_str(&row("Name", name));
    sb.push_str(&row("Descr", world.script_path_of(script)));
    sb.push_str(&row("Path", world.script_path_of(script)));
    sb.push_str(&row("Events", &events));
    if !npcs.is_empty() {
        sb.push_str(&row("NPCs", &npcs));
    }
    if !items.is_empty() {
        sb.push_str(&row("Items", &items));
    }
    if !timers.is_empty() {
        sb.push_str(&row("Timers", ""));
        sb.push_str(&timers);
    }
    let mut page: AdminHtml = world.admin_html_file(gm, PAGE);
    page.replace("%quests%", &sb);
    page.replace(
        "%questName%",
        &format!(
            "<table><tr><td width=\"50\" align=\"left\"><a action=\"bypass -h admin_quest_reload {name}\">Reload</a></td> <td width=\"150\"  align=\"center\"><a action=\"bypass -h admin_quest_info {name}\">{name}</a></td> <td width=\"50\" align=\"right\"><a action=\"bypass -h admin_script_unload {name}\">Unload</a></td></tr></table>"
        ),
    );
    world.send_admin_html(gm, &page);
    Ok(())
}
