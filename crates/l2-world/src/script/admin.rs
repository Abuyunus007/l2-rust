//! The admin words of the script systems: `//show_quests`, `//quest_info` (S-API-03),
//! `//quest_reload`, `//script_load`, `//script_unload`, `//charquestmenu`, `//setcharquest`
//! and `//reload quest` (S-ADMIN-08).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminQuest.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminShowQuests.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminReload.java (`quest`)
//
// # Reload and load with compiled-in scripts (D-16)
// Java recompiles the script file. A Rust script is compiled in, so `//quest_reload` is
// unload-then-load of the same code (`World::reload_script`): `onSave`, the timers and the
// listeners go, the script leaves the lists, and the constructor (`setup`) and `onLoad` run
// again. `//script_load <file>` takes the file name (path and `.java`/`.rs` ignored) as the
// script's name and loads an unloaded script (or reloads a loaded one, like Java's replace).
// A change of the Rust code needs a restart.
//
// # No database inside the world (D-15)
// `//charquestmenu` reads `character_quests` with a SELECT in Java. The target is online, and
// its in-memory quest states are the same rows (every change is written through), so the pages
// are built from them. A state that is `CREATED` has no `<state>` row, so it is not listed.
//
// # Other differences
// - `//quest_info` prints the "Path" of the Java class (its package); here it is the folder of
//   the Rust module, which is the script's `path()`.

use std::collections::{BTreeMap, BTreeSet};

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExShowQuestMark, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::ctx::ScriptCtx;
use super::qs::{self, Qs};
use super::setup::{RegisterType, ScriptId};
use super::state::{CREATED, QuestState, QuestWrite, STARTED, STATE_VAR, state_name};
use crate::admin::html::AdminHtml;
use crate::admin::{AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminQuest.ADMIN_COMMANDS` (the words this port has).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    handlers.register("admin_show_quests", admin_show_quests);
    handlers.register("admin_quest_info", admin_quest_info);
    handlers.register("admin_quest_reload", admin_quest_reload);
    handlers.register("admin_script_load", admin_script_load);
    handlers.register("admin_script_unload", admin_script_unload);
    handlers.register("admin_charquestmenu", admin_char_quests);
    handlers.register("admin_setcharquest", admin_char_quests);
    super::hellbound_api::register(handlers);
    super::gracia_admin::register(handlers);
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
    // Java walks `quest.getListeners()` in registration order: the text lists the types in that
    // order (the `TreeSet` only counts them), and stops after the eleventh new one.
    let mut seen = BTreeSet::new();
    for kind in world.scripts.listeners.listener_kinds_in_order(script) {
        let (ordinal, name) = kind.java_event_type();
        if seen.insert(ordinal) {
            events.push_str(&format!(", {name}"));
            counter += 1;
        }
        if counter > 10 {
            counter = 0;
            break;
        }
    }
    let type_count = seen.len();
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
    // Java: `_questTimers` is a `HashMap` of lists by name; the `break` leaves one list only.
    let mut by_timer_name: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for (name, npc, player, repeating) in world.quest_timers_of(script) {
        by_timer_name
            .entry(name)
            .or_default()
            .push((npc, player, repeating));
    }
    for (name, list) in by_timer_name {
        for (npc, player, repeating) in list {
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

// ------------------------------------------------------------------ //quest_reload and friends

/// Java `AdminQuest.findScript`: a number is a quest id, anything else a name.
fn find_script(world: &World, script: &str) -> Result<Option<ScriptId>, JavaException> {
    if is_numeric(script) {
        return Ok(world.scripts.by_quest_id(parse_int(script)?));
    }
    Ok(world.scripts.by_name(script))
}

/// Java `admin_quest_reload <name or id>`.
fn admin_quest_reload(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let Some(script) = st.next_token() else {
        world.send_sys_message(gm, "Usage: //quest_reload <questName> or <questId>");
        return Ok(());
    };
    let Some(id) = find_script(world, script)? else {
        world.send_sys_message(gm, &format!("The script {script} couldn't be found!"));
        return Ok(());
    };
    world.reload_script(id);
    world.send_sys_message(gm, "Script successfully reloaded.");
    Ok(())
}

/// The script a `//script_load` argument names: the file name without folders and extension.
fn script_name_of_path(path: &str) -> &str {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.strip_suffix(".java")
        .or_else(|| file.strip_suffix(".rs"))
        .unwrap_or(file)
}

/// Java `admin_script_load <path>`: see the module note for what loading means here.
fn admin_script_load(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let Some(path) = st.next_token() else {
        world.send_sys_message(gm, "Usage: //script_load path/to/script.java");
        return Ok(());
    };
    match world.scripts.find_any(script_name_of_path(path)) {
        Some(id) => {
            // Java compiles the file again and replaces a script of the same name
            // (`ScriptManager.addQuest`: `old.unload()`).
            world.reload_script(id);
            world.send_sys_message(gm, "Script loaded successfully!");
        }
        None => {
            world.send_sys_message(gm, "Failed to load script!");
            warn!("Failed to load script {path}!");
        }
    }
    Ok(())
}

/// Java `admin_script_unload <name or id>`.
fn admin_script_unload(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let Some(script) = st.next_token() else {
        // NOTE(java-quirk): the usage text names `//script_load`.
        world.send_sys_message(gm, "Usage: //script_load path/to/script.java");
        return Ok(());
    };
    let Some(id) = find_script(world, script)? else {
        world.send_sys_message(gm, &format!("The script {script} couldn't be found!"));
        return Ok(());
    };
    world.unload_script(id);
    world.send_sys_message(gm, "Script successfully unloaded!");
    Ok(())
}

/// Java `AdminReload` case `quest`: `//reload quest [name or id]`. The caller sends the warning
/// that follows every reload.
pub(crate) fn reload_quest(world: &mut World, gm: ObjectId, value: Option<&str>) -> Outcome {
    let gm_name = world.object_name(gm).unwrap_or_default();
    let Some(value) = value else {
        // `reloadAllScripts`: every script goes, then every script loads again.
        for id in world.scripts.ids().collect::<Vec<_>>() {
            world.unload_script(id);
        }
        for id in world.scripts.all_ids().collect::<Vec<_>>() {
            world.load_script(id);
        }
        world.send_sys_message(gm, "All scripts have been reloaded.");
        world.broadcast_message_to_gms(&format!("{gm_name}: Reloaded Quests."));
        return Ok(());
    };
    if is_numeric(value) {
        let quest_id = parse_int(value)?;
        if let Some(id) = world.scripts.by_quest_id(quest_id) {
            world.reload_script(id);
        }
        world.broadcast_message_to_gms(&format!("{gm_name}: Reloaded Quest ID:{quest_id}."));
    } else {
        if let Some(id) = world.scripts.by_name(value) {
            world.reload_script(id);
        }
        // NOTE(java-quirk): the broadcast does not depend on the script being found.
        world.broadcast_message_to_gms(&format!("{gm_name}: Reloaded Quest Name:{value}."));
    }
    Ok(())
}

// ------------------------------------------------------------ //charquestmenu, //setcharquest

const STATES: [&str; 3] = ["CREATED", "STARTED", "COMPLETED"];

const BUTTON: &str = "back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"";

/// Java `String.split(" ")`: trailing empty strings are dropped.
fn java_split(command: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = command.split(' ').collect();
    while parts.len() > 1 && parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

fn null_pointer() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

/// Java `AdminShowQuests.onCommand` (both words).
fn admin_char_quests(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let params = java_split(command);
    let mut val: [Option<String>; 4] = Default::default();
    let target = if params.len() > 1 {
        let target = world.player_by_name(params[1]);
        if params.len() > 2 {
            match params[2] {
                "0" => val = [Some("var".into()), Some("Start".into()), None, None],
                "1" => val = [Some("var".into()), Some("Started".into()), None, None],
                "2" => val = [Some("var".into()), Some("Completed".into()), None, None],
                "3" => val[0] = Some("full".into()),
                _ => {}
            }
            if params[2].contains('_') {
                val[0] = Some("name".into());
                val[1] = Some(params[2].to_owned());
            }
            if params.len() > 3 && params[3] == "custom" {
                val[0] = Some("custom".into());
                val[1] = Some(params[2].to_owned());
            }
        }
        target
    } else {
        world.target_of(gm).filter(|t| world.is_player(*t))
    };
    let Some(target) = target else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    };
    if command.starts_with("admin_charquestmenu") {
        if val[0].is_some() {
            show_quest_menu(world, target, gm, &val);
        } else {
            show_first_quest_menu(world, target, gm);
        }
    } else if command.starts_with("admin_setcharquest") && params.len() >= 5 {
        val[0] = Some(params[2].to_owned());
        val[1] = Some(params[3].to_owned());
        val[2] = Some(params[4].to_owned());
        if params.len() == 6 {
            val[3] = Some(params[5].to_owned());
        }
        set_quest_var(world, target, gm, val)?;
    }
    Ok(())
}

fn player_name(world: &World, id: ObjectId) -> String {
    world
        .players
        .get(&id)
        .map(|p| p.data.name.clone())
        .unwrap_or_default()
}

fn show_first_quest_menu(world: &mut World, target: ObjectId, actor: ObjectId) {
    let name = player_name(world, target);
    let id = target.0;
    let mut m = format!(
        "<html><body><table width=270><tr><td width=45><button value=\"Main\" action=\"bypass admin_admin\" width=45 height=21 {BUTTON}></td><td width=180><center>Player: {name}</center></td><td width=45><button value=\"Back\" action=\"bypass admin_admin6\" width=45 height=21 {BUTTON}></td></tr></table>"
    );
    m.push_str(&format!(
        "Quest Menu for <font color=\"LEVEL\">{name}</font> (ID:{id})<br><center>"
    ));
    m.push_str(&format!(
        "<table width=250><tr><td><button value=\"CREATED\" action=\"bypass -h admin_charquestmenu {name} 0\" width=85 height=21 {BUTTON}></td></tr>"
    ));
    m.push_str(&format!(
        "<tr><td><button value=\"STARTED\" action=\"bypass -h admin_charquestmenu {name} 1\" width=85 height=21 {BUTTON}></td></tr>"
    ));
    m.push_str(&format!(
        "<tr><td><button value=\"COMPLETED\" action=\"bypass -h admin_charquestmenu {name} 2\" width=85 height=21 {BUTTON}></td></tr>"
    ));
    m.push_str(&format!(
        "<tr><td><br><button value=\"All\" action=\"bypass -h admin_charquestmenu {name} 3\" width=85 height=21 {BUTTON}></td></tr>"
    ));
    m.push_str("<tr><td><br><br>Manual Edit by Quest number:<br></td></tr>");
    m.push_str(&format!(
        "<tr><td><edit var=\"qn\" width=50 height=15><br><button value=\"Edit\" action=\"bypass -h admin_charquestmenu {name} $qn custom\" width=50 height=21 {BUTTON}></td></tr>"
    ));
    m.push_str("</table></center></body></html>");
    world.send_admin_html(actor, &AdminHtml::from_html(&m));
}

/// The state of a player's quest by name.
fn quest_state_of<'a>(world: &'a World, player: ObjectId, name: &str) -> Option<&'a QuestState> {
    world.scripts.players.get(&player)?.get(name)
}

/// The variable rows with their Set and Del buttons (the order of the rows of a SELECT without
/// ORDER BY is not defined; they are sorted by name here).
fn var_rows(world: &World, target: ObjectId, qname: &str, shown_name: &str) -> String {
    let player = player_name(world, target);
    let mut rows = String::new();
    if let Some(qs) = quest_state_of(world, target, qname) {
        let sorted: BTreeMap<&String, &String> = qs.vars.iter().collect();
        for (var, value) in sorted {
            if var == STATE_VAR {
                continue;
            }
            rows.push_str(&format!(
                "<tr><td>{var}</td><td>{value}</td><td><edit var=\"var{var}\" width=80 height=15></td><td><button value=\"Set\" action=\"bypass -h admin_setcharquest {player} {shown_name} {var} $var{var}\" width=30 height=15 {BUTTON}></td><td><button value=\"Del\" action=\"bypass -h admin_setcharquest {player} {shown_name} {var} delete\" width=30 height=15 {BUTTON}></td></tr>"
            ));
        }
    }
    rows
}

/// Java `showQuestMenu`: any failure is "There was an error." (the `catch (Exception)`).
fn show_quest_menu(world: &mut World, target: ObjectId, actor: ObjectId, val: &[Option<String>; 4]) {
    match build_quest_menu(world, target, val) {
        Ok(html) => world.send_admin_html(actor, &AdminHtml::from_html(&html)),
        Err(e) => {
            world.send_message(actor, "There was an error.");
            warn!("AdminShowQuests: {e}");
        }
    }
}

fn build_quest_menu(
    world: &World,
    target: ObjectId,
    val: &[Option<String>; 4],
) -> Result<String, JavaException> {
    let name = player_name(world, target);
    let id = target.0;
    let arg = val[1].clone().unwrap_or_default();
    let mut m = String::from("<html><body>");
    match val[0].as_deref().unwrap_or_default() {
        "full" => {
            m.push_str(&format!(
                "<table width=250><tr><td>Full Quest List for <font color=\"LEVEL\">{name}</font> (ID:{id})</td></tr>"
            ));
            if let Some(quests) = world.scripts.players.get(&target) {
                for qs in quests.iter().filter(|qs| qs.state() != CREATED) {
                    let q = qs.quest_name();
                    m.push_str(&format!(
                        "<tr><td><a action=\"bypass -h admin_charquestmenu {name} {q}\">{q}</a></td></tr>"
                    ));
                }
            }
            m.push_str("</table></body></html>");
        }
        "name" => {
            let state =
                quest_state_of(world, target, &arg).map_or("CREATED", |qs| STATES[qs.state() as usize]);
            m.push_str(&format!(
                "Character: <font color=\"LEVEL\">{name}</font><br>Quest: <font color=\"LEVEL\">{arg}</font><br>State: <font color=\"LEVEL\">{state}</font><br><br>"
            ));
            m.push_str("<center><table width=250><tr><td>Var</td><td>Value</td><td>New Value</td><td>&nbsp;</td></tr>");
            m.push_str(&var_rows(world, target, &arg, &arg));
            m.push_str("</table><br><br><table width=250><tr><td>Repeatable quest:</td><td>Unrepeatable quest:</td></tr>");
            m.push_str(&format!(
                "<tr><td><button value=\"Quest Complete\" action=\"bypass -h admin_setcharquest {name} {arg} state COMPLETED 1\" width=120 height=21 {BUTTON}></td>"
            ));
            m.push_str(&format!(
                "<td><button value=\"Quest Complete\" action=\"bypass -h admin_setcharquest {name} {arg} state COMPLETED 0\" width=120 height=21 {BUTTON}></td></tr>"
            ));
            m.push_str(&format!(
                "</table><br><br><font color=\"ff0000\">Delete Quest from DB:</font><br><button value=\"Quest Delete\" action=\"bypass -h admin_setcharquest {name} {arg} state DELETE\" width=120 height=21 {BUTTON}>"
            ));
            m.push_str("</center></body></html>");
        }
        "var" => {
            m.push_str(&format!(
                "Character: <font color=\"LEVEL\">{name}</font><br>Quests with state: <font color=\"LEVEL\">{arg}</font><br>"
            ));
            m.push_str("<table width=250>");
            if let Some(quests) = world.scripts.players.get(&target) {
                // Only a state with a `<state>` row counts; a `CREATED` one has none.
                for qs in quests
                    .iter()
                    .filter(|qs| qs.state() != CREATED && state_name(qs.state()) == arg)
                {
                    let q = qs.quest_name();
                    m.push_str(&format!(
                        "<tr><td><a action=\"bypass -h admin_charquestmenu {name} {q}\">{q}</a></td></tr>"
                    ));
                }
            }
            m.push_str("</table></body></html>");
        }
        "custom" => {
            let qnumber = parse_int(&arg)?;
            let script = world.scripts.by_quest_id(qnumber);
            let qname = script.map(|s| world.scripts.name_of(s));
            let qs = qname.and_then(|n| quest_state_of(world, target, n));
            let state = qs.map_or("N/A", |qs| STATES[qs.state() as usize]);
            match (qname, qs) {
                (Some(qname), Some(_)) => {
                    m.push_str(&format!(
                        "Character: <font color=\"LEVEL\">{name}</font><br>Quest: <font color=\"LEVEL\">{qname}</font><br>State: <font color=\"LEVEL\">{state}</font><br><br>"
                    ));
                    m.push_str("<center><table width=250><tr><td>Var</td><td>Value</td><td>New Value</td><td>&nbsp;</td></tr>");
                    m.push_str(&var_rows(world, target, qname, qname));
                    m.push_str("</table><br><br><table width=250><tr><td>Repeatable quest:</td><td>Unrepeatable quest:</td></tr>");
                    m.push_str(&format!(
                        "<tr><td><button value=\"Quest Complete\" action=\"bypass -h admin_setcharquest {name} {qname} state COMPLETED 1\" width=100 height=21 {BUTTON}></td>"
                    ));
                    m.push_str(&format!(
                        "<td><button value=\"Quest Complete\" action=\"bypass -h admin_setcharquest {name} {qname} state COMPLETED 0\" width=100 height=21 {BUTTON}></td></tr>"
                    ));
                    m.push_str(&format!(
                        "</table><br><br><font color=\"ff0000\">Delete Quest from DB:</font><br><button value=\"Quest Delete\" action=\"bypass -h admin_setcharquest {name} {qname} state DELETE\" width=100 height=21 {BUTTON}>"
                    ));
                    m.push_str("</center></body></html>");
                }
                (Some(qname), None) => {
                    m.push_str(&format!(
                        "Character: <font color=\"LEVEL\">{name}</font><br>Quest: <font color=\"LEVEL\">{qname}</font><br>State: <font color=\"LEVEL\">{state}</font><br><br>"
                    ));
                    m.push_str("<center>Start this Quest for player:<br>");
                    m.push_str(&format!(
                        "<button value=\"Create Quest\" action=\"bypass -h admin_setcharquest {name} {qnumber} state CREATE\" width=100 height=21 {BUTTON}><br><br>"
                    ));
                    m.push_str("<font color=\"ee0000\">Only for Unrepeateble quests:</font><br>");
                    m.push_str(&format!(
                        "<button value=\"Create & Complete\" action=\"bypass -h admin_setcharquest {name} {qnumber} state CC\" width=130 height=21 {BUTTON}><br><br>"
                    ));
                    m.push_str("</center></body></html>");
                }
                (None, _) => {
                    m.push_str(&format!(
                        "<center><font color=\"ee0000\">Quest with number </font><font color=\"LEVEL\">{qnumber}</font><font color=\"ee0000\"> doesn't exist!</font></center></body></html>"
                    ));
                }
            }
        }
        _ => {}
    }
    Ok(m)
}

/// The handle on the quest state `name` of `player` (`getQuestState(name)`), if there is one.
fn qs_handle(world: &World, player: ObjectId, name: &str) -> Option<Qs> {
    let state = quest_state_of(world, player, name)?;
    let script = world.scripts.find_any(state.quest_name())?;
    Some(Qs { player, script })
}

/// `new QuestState(quest, player, CREATED)` of `getQuest(Integer.parseInt(text))`: the player
/// gets a fresh `CREATED` state, replacing any other.
fn new_quest_state(world: &mut World, player: ObjectId, text: &str) -> Result<Qs, JavaException> {
    let quest_id = parse_int(text)?;
    let script = world.scripts.by_quest_id(quest_id).ok_or_else(null_pointer)?;
    let name = world.scripts.name_of(script);
    world
        .scripts
        .players
        .entry(player)
        .or_default()
        .insert(QuestState::new(name, CREATED));
    Ok(Qs { player, script })
}

/// The `QuestList` and `ExShowQuestMark` Java sends after a change.
fn send_quest_update(world: &mut World, target: ObjectId, script: ScriptId) {
    world.send_quest_list(target);
    let quest_id = world.scripts.quest_id_of(script);
    world.send(target, &ExShowQuestMark { quest_id });
}

/// Java `setQuestVar`: `val` is `[quest, variable or "state", value, flag]`.
fn set_quest_var(
    world: &mut World,
    target: ObjectId,
    actor: ObjectId,
    mut val: [Option<String>; 4],
) -> Result<(), JavaException> {
    let quest = val[0].clone().unwrap_or_default();
    let variable = val[1].clone().unwrap_or_default();
    let value = val[2].clone().unwrap_or_default();
    let qs = qs_handle(world, target, &quest);
    if variable == "state" {
        match value.as_str() {
            "COMPLETED" => {
                let qs = qs.ok_or_else(null_pointer)?;
                let repeatable = val[3].as_deref().ok_or_else(null_pointer)? == "1";
                qs.exit_quest(&mut ScriptCtx::new(world, qs.script), repeatable, false);
            }
            "DELETE" => {
                let qs = qs.ok_or_else(null_pointer)?;
                // `Quest.deleteQuestInDb(qs, true)`, then `exitQuest(true)`, which deletes again
                // when the quest is started.
                let name = world.scripts.name_of(qs.script).to_owned();
                qs::quest_write(
                    world,
                    QuestWrite::DeleteQuest {
                        char_id: target.0,
                        name,
                        repeatable: true,
                    },
                );
                qs.exit_quest(&mut ScriptCtx::new(world, qs.script), true, false);
                send_quest_update(world, target, qs.script);
            }
            "CREATE" => {
                let qs = new_quest_state(world, target, &quest)?;
                let mut ctx = ScriptCtx::new(world, qs.script);
                qs.set_state(&mut ctx, STARTED);
                qs.set_cond(&mut ctx, 1, false);
                send_quest_update(world, target, qs.script);
                val[0] = Some(world.scripts.name_of(qs.script).to_owned());
            }
            "CC" => {
                let qs = new_quest_state(world, target, &quest)?;
                // NOTE(java-quirk): the fresh state is `CREATED`, and `exitQuest` does nothing
                // for a quest that is not started, so "Create & Complete" only creates it.
                qs.exit_quest(&mut ScriptCtx::new(world, qs.script), false, false);
                send_quest_update(world, target, qs.script);
                val[0] = Some(world.scripts.name_of(qs.script).to_owned());
            }
            _ => {}
        }
    } else {
        let qs = qs.ok_or_else(null_pointer)?;
        let mut ctx = ScriptCtx::new(world, qs.script);
        if value == "delete" {
            qs.unset(&mut ctx, &variable);
        } else {
            qs.set(&mut ctx, &variable, &value);
        }
        send_quest_update(world, target, qs.script);
    }
    world.send_message(actor, "");
    let page = [Some("name".to_owned()), val[0].take(), None, None];
    show_quest_menu(world, target, actor, &page);
    Ok(())
}
