//! The character admin commands of `AdminEditChar` that were not ported yet: the info, list and
//! find pages (`//character_info`, `//character_list`, `//show_characters`, `//find_character`,
//! `//find_ip`, `//find_account`, `//find_dualbox`, `//edit_character`, `//current_player`) and
//! the target edits (`//setfame`, `//rec`, `//setclass`, `//settitle`, `//changename`,
//! `//setsex`, `//settcolor`, `//fullfood`, `//setnoble`, `//set_hp`, `//set_mp`, `//set_cp`).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEditChar.java
//
// # Left out, with the system that brings it
// - `//setcolor` (the name colour): a player's name colour is not stored in the world (only the
//   colour of the access level), so the word is not registered and answers "does not exist".
// - `//tracert` and `//strict_find_dualbox`: the five trace hops of `EnterWorld` are parsed but
//   not handed to the world, so these words are not registered.
// - The texture refresh of `//setsex` and of a `//setclass` that changes sex or race: Java
//   transforms the player into 105 and untransforms it 200 ms later (`ThreadPool.schedule`), and
//   the timer wheel has no such task yet. The refresh is skipped.
// - `//setclass` does the class id, the base class, the academy graduation and the store, but not
//   `setClassTemplate`, the skill and henna changes, the `MagicSkillUse` effect and the party and
//   clan class icons (the class change itself is not ported yet, see `clan/academy.rs`).
// - `//setnoble` sets the flag only: the noble skill tree is not in the world.
// - `//setfame` does not check `EnableFameSystem` (not in the world settings); the default is on.
// - `//changename` checks the name against the GM's Java `CharInfoTable` lookup only (online and
//   the startup table), not the name format of character creation (`CnameTemplate`, which lives
//   with the game server's config).
// - `%access%` of the info page shows the level only: the names of `AccessLevels.xml` are not
//   kept by the rights table. `%protocol%` shows `N/A` for a connected client (the protocol version
//   is not in the world), `%hwid%` shows `Unknown` (no hardware info exists), `%autoplay%` is empty
//   (AutoPlay is not ported).
// - `%inst%` links to `admin_instance_spawns`, which belongs to the instances (M8).

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::str::FromStr;

use l2_data::ClassId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NicknameChanged, PartySmallWindowDeleteAll, SystemMessage};
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::html::{PageResult, build_page};
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::outbox::PersistCommand;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminEditChar.ADMIN_COMMANDS` (the words this module registers).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_current_player", admin_current_player),
        ("admin_character_info", admin_character_info),
        ("admin_character_list", admin_character_list),
        ("admin_show_characters", admin_show_characters),
        ("admin_find_character", admin_find_character),
        ("admin_find_ip", admin_find_ip),
        ("admin_find_account", admin_find_account),
        ("admin_edit_character", admin_edit_character),
        ("admin_find_dualbox", admin_find_dualbox),
        ("admin_setfame", admin_setfame),
        ("admin_rec", admin_rec),
        ("admin_setclass", admin_setclass),
        ("admin_settitle", admin_settitle),
        ("admin_changename", admin_changename),
        ("admin_setsex", admin_setsex),
        ("admin_settcolor", admin_settcolor),
        ("admin_fullfood", admin_fullfood),
        ("admin_setnoble", admin_setnoble),
        ("admin_set_hp", admin_set_hp),
        ("admin_set_mp", admin_set_mp),
        ("admin_set_cp", admin_set_cp),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

fn null_pointer() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

/// The target when it is a player (Java `activeChar.getTarget()` and `isPlayer()`).
fn target_player(world: &World, gm: ObjectId) -> Option<ObjectId> {
    world.target_of(gm).filter(|t| world.is_player(*t))
}

/// The target when it is a creature (Java `target.isCreature()`).
fn target_creature(world: &World, gm: ObjectId) -> Option<ObjectId> {
    world
        .target_of(gm)
        .filter(|t| world.objects.get(t).is_some_and(|e| e.kind.is_creature()))
}

fn invalid_target(world: &World, gm: ObjectId) {
    world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
}

/// Java `World.getPlayers()` sorted by `Player.getUptime()`: the most recently entered first.
/// Ties (two logins in the same millisecond) keep the object id order.
fn players_by_uptime(world: &World) -> Vec<ObjectId> {
    let mut entered: Vec<(i64, ObjectId)> = world
        .players
        .values()
        .map(|p| (p.entered_at_unix_ms, p.core.object_id))
        .collect();
    entered.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.0.cmp(&b.1.0)));
    entered.into_iter().map(|(_, id)| id).collect()
}

/// `client.getIp()` of a connected player, `None` when the player has no client here.
fn client_ip_string(world: &World, id: ObjectId) -> Option<String> {
    if world.is_offline_trader(id) {
        return None;
    }
    world
        .players
        .get(&id)
        .and_then(|p| p.client_ip)
        .map(|ip| ip.to_string())
}

/// Java `client.isDetached()`: an offline trader has no client any more.
fn is_detached(world: &World, id: ObjectId) -> bool {
    world.is_offline_trader(id)
}

// ------------------------------------------------------------------------------- list pages

/// Java `PageBuilder` with `NextPrevPageHandler` and `BypassParserFormatter` (`bypass page=N`)
/// and `ButtonsStyle`: the `<<  <  Page: n/m  >  >>` row of the character list.
pub(crate) fn next_prev_pager(bypass: &str, current: i32, pages: i32) -> String {
    let link = |page: i32, name: &str, enabled: bool| -> String {
        if enabled {
            format!("<td>{name}</td>")
        } else {
            format!(
                "<td><button action=\"{bypass} page={page}\" value=\"{name}\" width=\"40\" height=\"15\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>"
            )
        }
    };
    let separator = "<td align=center> | </td>";
    let mut out = String::new();
    out.push_str(&link(0, "&lt;&lt;", current - 1 < 0));
    out.push_str(separator);
    out.push_str(&link(current - 1, "&lt;", current <= 0));
    out.push_str(separator);
    out.push_str(&format!(
        "<td align=\"center\">Page: {}/{}</td>",
        current + 1,
        pages + 1
    ));
    out.push_str(separator);
    out.push_str(&link(current + 1, "&gt;", current >= pages));
    out.push_str(separator);
    out.push_str(&link(pages, "&gt;&gt;", current + 1 > pages));
    out
}

/// Java `AdminEditChar.listCharacters(activeChar, page)`: `charlist.htm`, 20 characters a page.
fn list_characters(world: &mut World, gm: ObjectId, page: i32) {
    let players = players_by_uptime(world);
    let rows: Vec<(String, bool, String, i32)> = players
        .iter()
        .filter_map(|id| {
            let p = world.players.get(id)?;
            let class = world
                .admin
                .class_list
                .get_by_id(p.data.class_id)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            Some((
                p.data.name.clone(),
                world.is_offline_trader(*id),
                class,
                p.data.level,
            ))
        })
        .collect();
    let result: PageResult = build_page(&rows, 20, "bypass -h admin_show_characters", page, |row, sb| {
        let (name, offline, class, level) = row;
        sb.push_str("<tr>");
        let shown = if *offline {
            format!("<font color=\"808080\">{name}</font>")
        } else {
            name.clone()
        };
        sb.push_str(&format!(
            "<td width=80><a action=\"bypass -h admin_character_info {name}\">{shown}</a></td>"
        ));
        sb.push_str(&format!("<td width=110>{class}</td><td width=40>{level}</td>"));
        sb.push_str("</tr>");
    });
    let pager = if result.pages > 1 {
        format!(
            "<table width=280 cellspacing=0><tr>{}</tr></table>",
            next_prev_pager("bypass -h admin_show_characters", page, result.pages - 1)
        )
    } else {
        String::new()
    };
    let mut html = world.admin_html_file(gm, "data/html/admin/charlist.htm");
    html.replace("%pages%", &pager);
    html.replace("%players%", &result.body);
    world.send_admin_html(gm, &html);
}

/// Java `admin_character_list`: `listCharacters(activeChar, 0)`.
fn admin_character_list(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    list_characters(world, gm, 0);
    Ok(())
}

/// Java `admin_show_characters`: `command.substring(22).replace("page=", "")`, then the page. A
/// command too short for `substring(22)` is the usage line.
fn admin_show_characters(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(value) = command.get(22..) else {
        world.send_sys_message(gm, "Usage: //show_characters <page_number>");
        return Ok(());
    };
    let page = parse_int(&value.replace("page=", ""))?;
    list_characters(world, gm, page);
    Ok(())
}

/// Java `admin_find_character`: the characters whose name contains the text (ignoring case).
fn admin_find_character(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(wanted) = command.get(21..) else {
        world.send_sys_message(gm, "Usage: //find_character <character_name>");
        list_characters(world, gm, 0);
        return Ok(());
    };
    find_character(world, gm, wanted);
    Ok(())
}

/// Java `AdminEditChar.findCharacter`: `charfind.htm` with at most 21 results (the loop stops
/// after the 21st, "more than 20" is shown).
fn find_character(world: &mut World, gm: ObjectId, wanted: &str) {
    let wanted = wanted.to_lowercase();
    let mut found = 0;
    let mut results = String::new();
    for id in players_by_uptime(world) {
        let Some(p) = world.players.get(&id) else {
            continue;
        };
        let name = p.data.name.clone();
        let level = p.data.level;
        let class_id = p.data.class_id;
        if name.to_lowercase().contains(&wanted) {
            found += 1;
            let class = world
                .admin
                .class_list
                .get_by_id(class_id)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            results.push_str(&format!(
                "<tr><td width=80><a action=\"bypass -h admin_character_info {name}\">{name}</a></td><td width=110>{class}</td><td width=40>{level}</td></tr>"
            ));
        }
        if found > 20 {
            break;
        }
    }

    let mut html = world.admin_html_file(gm, "data/html/admin/charfind.htm");
    html.replace("%results%", &results);
    let end = if found == 0 {
        "s. Please try again."
    } else if found > 20 {
        html.replace("%number%", " more than 20");
        "s.<br>Please refine your search to see all of the results."
    } else if found == 1 {
        "."
    } else {
        "s."
    };
    html.replace("%number%", &found.to_string());
    html.replace("%end%", end);
    world.send_admin_html(gm, &html);
}

/// Java `IPV4_PATTERN`: four decimal numbers 0 to 255 without leading zeros.
fn is_ipv4(text: &str) -> bool {
    Ipv4Addr::from_str(text).is_ok_and(|ip| ip.to_string() == text)
}

/// Java `admin_find_ip`: the connections of an IPv4 number (or `disconnected`). A bad value is the
/// usage line and the first page of the list.
fn admin_find_ip(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let valid = command
        .get(14..)
        .filter(|ip| *ip == "disconnected" || is_ipv4(ip))
        .map(str::to_owned);
    match valid {
        Some(ip) => find_characters_per_ip(world, gm, &ip),
        None => {
            world.send_sys_message(gm, "Usage: //find_ip <www.xxx.yyy.zzz>");
            list_characters(world, gm, 0);
        }
    }
    Ok(())
}

/// Java `AdminEditChar.findCharactersPerIp`: `ipfind.htm`, the same 21 results at most.
fn find_characters_per_ip(world: &mut World, gm: ObjectId, ip: &str) {
    let find_disconnected = ip == "disconnected";
    let mut found = 0;
    let mut results = String::new();
    for id in players_by_uptime(world) {
        let detached = is_detached(world, id);
        if detached {
            if !find_disconnected {
                continue;
            }
        } else {
            if find_disconnected {
                continue;
            }
            // A player without a client is skipped (Java `client == null`).
            match client_ip_string(world, id) {
                Some(player_ip) if player_ip == ip => {}
                _ => continue,
            }
        }
        let Some(p) = world.players.get(&id) else {
            continue;
        };
        let name = p.data.name.clone();
        let level = p.data.level;
        let class_id = p.data.class_id;
        found += 1;
        let class = world
            .admin
            .class_list
            .get_by_id(class_id)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        results.push_str(&format!(
            "<tr><td width=80><a action=\"bypass -h admin_character_info {name}\">{name}</a></td><td width=110>{class}</td><td width=40>{level}</td></tr>"
        ));
        if found > 20 {
            break;
        }
    }

    let mut html = world.admin_html_file(gm, "data/html/admin/ipfind.htm");
    html.replace("%results%", &results);
    let end = if found == 0 {
        "s. Maybe they got d/c? :)"
    } else if found > 20 {
        html.replace("%number%", &format!(" more than {found}"));
        "s.<br>In order to avoid you a client crash I won't <br1>display results beyond the 20th character."
    } else if found == 1 {
        "."
    } else {
        "s."
    };
    html.replace("%ip%", ip);
    html.replace("%number%", &found.to_string());
    html.replace("%end%", end);
    world.send_admin_html(gm, &html);
}

/// Java `admin_find_account`: the characters of the account of the named player (`accountinfo.htm`).
fn admin_find_account(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let target = command.get(19..).and_then(|name| world.player_by_name(name));
    match target {
        Some(player) => find_characters_per_account(world, gm, player),
        None => {
            world.send_sys_message(gm, "Usage: //find_account <player_name>");
            list_characters(world, gm, 0);
        }
    }
    Ok(())
}

/// Java `AdminEditChar.findCharactersPerAccount`: the names of `Player.getAccountChars()`, which
/// is the account's characters as they were read at the login.
fn find_characters_per_account(world: &mut World, gm: ObjectId, player: ObjectId) {
    let Some(p) = world.players.get(&player) else {
        return;
    };
    let account = p.data.account.clone();
    let name = p.data.name.clone();
    let chars = p
        .warehouse
        .account_chars
        .iter()
        .map(|(_, name)| name.clone())
        .collect::<Vec<_>>()
        .join("<br1>");
    let mut html = world.admin_html_file(gm, "data/html/admin/accountinfo.htm");
    html.replace("%account%", &account);
    html.replace("%player%", &name);
    html.replace("%characters%", &chars);
    world.send_admin_html(gm, &html);
}

// ------------------------------------------------------------------------ dualbox

/// Java `HashMap` iteration order for `String` keys: the table index is the spread hash of the
/// key (`String.hashCode`), buckets in index order, the keys of one bucket in insertion order.
/// The table grows from 16 while the key count is above three quarters of it.
// NOTE(java-quirk): a bucket that Java turns into a tree (eight keys in one bucket of a table of
// at least 64) iterates differently; a dualbox list never gets that big.
fn java_hashmap_order(keys: &[String]) -> Vec<String> {
    let mut capacity: u32 = 16;
    while keys.len() as u32 > capacity / 4 * 3 {
        capacity *= 2;
    }
    let mut keyed: Vec<(u32, usize, String)> = keys
        .iter()
        .enumerate()
        .map(|(order, key)| {
            let hash = java_string_hash(key);
            let spread = hash ^ (hash >> 16);
            (spread & (capacity - 1), order, key.clone())
        })
        .collect();
    keyed.sort_by_key(|(bucket, order, _)| (*bucket, *order));
    keyed.into_iter().map(|(_, _, key)| key).collect()
}

/// Java `String.hashCode()`: `s[0]*31^(n-1) + ... + s[n-1]` over the UTF-16 units, wrapping.
fn java_string_hash(text: &str) -> u32 {
    text.encode_utf16()
        .fold(0_i32, |h, unit| h.wrapping_mul(31).wrapping_add(i32::from(unit))) as u32
}

/// Java `admin_find_dualbox [n]`: the IPs with at least `n` (default 2) players on them.
fn admin_find_dualbox(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `Integer.parseInt(command.substring(19))`; a failure keeps the default 2 (Java's catch).
    let multibox = command
        .get(19..)
        .and_then(|value| parse_int(value).ok())
        .unwrap_or(2);
    if multibox < 1 {
        world.send_sys_message(gm, "Usage: //find_dualbox [number > 0]");
        return Ok(());
    }
    find_dualbox(world, gm, multibox);
    Ok(())
}

/// Java `AdminEditChar.findDualbox`: `dualbox.htm`. The IPs are listed by their count, highest
/// first; equal counts keep Java's `HashMap` order.
fn find_dualbox(world: &mut World, gm: ObjectId, multibox: i32) {
    let mut sizes: HashMap<String, i32> = HashMap::new();
    let mut counts: HashMap<String, i32> = HashMap::new();
    let mut inserted: Vec<String> = Vec::new();
    for id in players_by_uptime(world) {
        if is_detached(world, id) {
            continue;
        }
        let Some(ip) = client_ip_string(world, id) else {
            continue;
        };
        let size = sizes.entry(ip.clone()).or_insert(0);
        *size += 1;
        if *size >= multibox {
            match counts.get_mut(&ip) {
                Some(count) => *count += 1,
                None => {
                    counts.insert(ip.clone(), multibox);
                    inserted.push(ip);
                }
            }
        }
    }

    let mut keys = java_hashmap_order(&inserted);
    keys.sort_by_key(|ip| std::cmp::Reverse(counts.get(ip).copied().unwrap_or(0)));
    let mut results = String::new();
    for ip in &keys {
        let count = counts.get(ip).copied().unwrap_or(0);
        results.push_str(&format!(
            "<a action=\"bypass -h admin_find_ip {ip}\">{ip} ({count})</a><br1>"
        ));
    }
    let mut html = world.admin_html_file(gm, "data/html/admin/dualbox.htm");
    html.replace("%multibox%", &multibox.to_string());
    html.replace("%results%", &results);
    html.replace("%strict%", "");
    world.send_admin_html(gm, &html);
}

// ------------------------------------------------------------------------- info pages

/// Java `ClassListData.getClass(id).getClassName()` of a class id.
fn class_name_of(world: &World, class_id: i32) -> Result<String, JavaException> {
    world
        .admin
        .class_list
        .get_by_id(class_id)
        .map(|c| c.name.clone())
        .ok_or_else(null_pointer)
}

/// Java `PlayerClass.name()` (the enum constant) of a class id, as `String.valueOf` shows it.
fn enum_name_of(class_id: i32) -> String {
    u16::try_from(class_id)
        .ok()
        .and_then(|id| ClassId(id).enum_name())
        .map(str::to_owned)
        .unwrap_or_default()
}

/// Java `Intention.name()` of the player's current intention.
fn intention_name(world: &World, id: ObjectId) -> &'static str {
    use crate::movement::Intention as Ai;
    match world.intention_of(id) {
        Ai::Idle => "IDLE",
        Ai::Active => "ACTIVE",
        Ai::Rest => "REST",
        Ai::Attack => "ATTACK",
        Ai::Cast => "CAST",
        Ai::MoveTo => "MOVE_TO",
        Ai::Follow => "FOLLOW",
        Ai::PickUp => "PICK_UP",
        Ai::Interact => "INTERACT",
    }
}

/// Java `String.format("%.2f", ((float) load / maxLoad) * 100)`: the float division, an
/// infinite or NaN result when the maximum is 0.
fn percent_text(load: i32, max_load: i32) -> String {
    let value = (load as f32 / max_load as f32) * 100.0;
    if value.is_nan() {
        "NaN".to_owned()
    } else if value.is_infinite() {
        "Infinity".to_owned()
    } else {
        format!("{:.2}", f64::from(value))
    }
}

/// Java `AdminEditChar.gatherCharacterInfo(activeChar, player, filename)`: the page of a player
/// (`charinfo.htm` or `charedit.htm`) with the player's numbers.
fn gather_character_info(world: &mut World, gm: ObjectId, player: ObjectId, file: &str) -> Outcome {
    // The client's address. Java reports a missing or detached client to the GM and keeps `N/A`.
    let mut ip = "N/A".to_owned();
    if is_detached(world, player) {
        world.send_sys_message(gm, "Client is detached.");
    } else if let Some(text) = client_ip_string(world, player) {
        ip = text;
    } else {
        world.send_sys_message(gm, "Client is null.");
    }

    let Some(p) = world.players.get(&player) else {
        return Ok(());
    };
    let data = p.data.clone();
    let core = p.core.clone();
    let has_clan = world.clan_of(player).map(|c| c.name.clone());
    let clan_id_object = player.0;
    let class_name = class_name_of(world, data.class_id)?;
    let base_class_name = class_name_of(world, data.base_class)?;
    let stats = world.stats(player).ok_or_else(null_pointer)?;
    let max_load = stats.max_load();
    let current_load = stats.current_load();
    let (max_hp, max_mp, max_cp) = (stats.max_hp(), stats.max_mp(), stats.max_cp());
    let p_atk = stats.p_atk(None) as i32;
    let m_atk = stats.m_atk(None, None) as i32;
    let p_def = stats.p_def(None) as i32;
    let m_def = stats.m_def(None, None) as i32;
    let accuracy = stats.accuracy();
    let evasion = stats.evasion_rate(None);
    let critical = stats.critical_hit(None, None);
    let run_speed = stats.run_speed() as i32;
    let p_atk_spd = stats.p_atk_spd();
    let m_atk_spd = stats.m_atk_spd();
    let pvp_flag = world.pvp_flag_of(player);
    let karma = world.karma_of_player(player);
    let access = data.access_level;
    let (cur_hp, cur_mp, cur_cp) = (core.cur_hp as i32, core.cur_mp as i32, core.cur_cp as i32);
    let (x, y, z, heading) = (core.x, core.y, core.z, core.heading);
    let offline = world.is_offline_trader(player);
    let instance_id = world.objects.get(&player).map_or(0, |e| e.instance_id);
    let ai = intention_name(world, player);

    let mut html = world.admin_html_file(gm, &format!("data/html/admin/{file}"));
    let clan = match has_clan {
        Some(name) => format!("<a action=\"bypass -h admin_clan_info {clan_id_object}\">{name}</a>"),
        None => "null".to_owned(),
    };
    html.replace("%name%", &data.name);
    html.replace("%level%", &data.level.to_string());
    html.replace("%clan%", &clan);
    html.replace("%xp%", &data.exp.to_string());
    html.replace("%sp%", &data.sp.to_string());
    html.replace("%class%", &class_name);
    html.replace("%ordinal%", &data.class_id.to_string());
    html.replace("%classid%", &enum_name_of(data.class_id));
    html.replace("%baseclass%", &base_class_name);
    html.replace("%x%", &x.to_string());
    html.replace("%y%", &y.to_string());
    html.replace("%z%", &z.to_string());
    html.replace("%heading%", &heading.to_string());
    html.replace("%currenthp%", &cur_hp.to_string());
    html.replace("%maxhp%", &max_hp.to_string());
    html.replace("%karma%", &karma.to_string());
    html.replace("%currentmp%", &cur_mp.to_string());
    html.replace("%maxmp%", &max_mp.to_string());
    html.replace("%pvpflag%", &pvp_flag.to_string());
    html.replace("%currentcp%", &cur_cp.to_string());
    html.replace("%maxcp%", &max_cp.to_string());
    html.replace("%pvpkills%", &data.pvp_kills.to_string());
    html.replace("%pkkills%", &data.pk_kills.to_string());
    html.replace("%currentload%", &current_load.to_string());
    html.replace("%maxload%", &max_load.to_string());
    html.replace("%percent%", &percent_text(current_load, max_load));
    html.replace("%patk%", &p_atk.to_string());
    html.replace("%matk%", &m_atk.to_string());
    html.replace("%pdef%", &p_def.to_string());
    html.replace("%mdef%", &m_def.to_string());
    html.replace("%accuracy%", &accuracy.to_string());
    html.replace("%evasion%", &evasion.to_string());
    html.replace("%critical%", &critical.to_string());
    html.replace("%runspeed%", &run_speed.to_string());
    html.replace("%patkspd%", &(p_atk_spd as i32).to_string());
    html.replace("%matkspd%", &m_atk_spd.to_string());
    // NOTE: the access level name is not in the rights table (see the module note).
    html.replace("%access%", &access.to_string());
    html.replace("%account%", &data.account);
    html.replace("%ip%", &ip);
    // The protocol version is not kept by the world (see the module note).
    html.replace(
        "%protocol%",
        if world.players.get(&player).and_then(|p| p.client_ip).is_some() && !offline {
            "N/A"
        } else {
            "NULL"
        },
    );
    // No hardware info exists (HWID is not ported): Java shows `Unknown` for a client without it.
    html.replace("%hwid%", "Unknown");
    html.replace("%ai%", ai);
    // AutoPlay is not ported: the line is empty as with `ENABLE_AUTO_PLAY = false`.
    html.replace("%autoplay%", "");
    // NOTE(java-quirk): the Java link goes to `admin_instance_spawns` (instances, M8).
    html.replace(
        "%inst%",
        &if instance_id != 0 {
            format!(
                "<tr><td>InstanceId:</td><td><a action=\"bypass -h admin_instance_spawns {instance_id}\">{instance_id}</a></td></tr>"
            )
        } else {
            String::new()
        },
    );
    html.replace("%noblesse%", if data.profile.noble { "Yes" } else { "No" });
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `AdminEditChar.showCharacterInfo(activeChar, targetPlayer)`: the named player (and the
/// target becomes that player), else the target. Nothing when neither is a player.
fn show_character_info(world: &mut World, gm: ObjectId, named: Option<ObjectId>) -> Outcome {
    let player = match named {
        Some(player) => {
            world.set_target(gm, Some(player));
            player
        }
        None => match target_player(world, gm) {
            Some(player) => player,
            None => return Ok(()),
        },
    };
    gather_character_info(world, gm, player, "charinfo.htm")
}

/// Java `admin_current_player`: the GM's own information, and the GM becomes the target.
fn admin_current_player(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    show_character_info(world, gm, Some(gm))
}

/// Java `admin_character_info [name]`.
fn admin_character_info(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    if words.len() > 1 {
        let named = world.player_by_name(words[1]);
        return show_character_info(world, gm, named);
    }
    if target_player(world, gm).is_some() {
        return show_character_info(world, gm, None);
    }
    invalid_target(world, gm);
    Ok(())
}

/// Java `admin_edit_character [name]`: `charedit.htm` of the named player, or of the target. A
/// name that nobody is online with shows nothing, as in Java.
fn admin_edit_character(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    if words.len() > 1 {
        if let Some(player) = world.player_by_name(words[1]) {
            gather_character_info(world, gm, player, "charedit.htm")?;
        }
    } else if let Some(player) = target_player(world, gm) {
        gather_character_info(world, gm, player, "charedit.htm")?;
    } else {
        invalid_target(world, gm);
    }
    Ok(())
}

// ------------------------------------------------------------------------- the edits

/// Java `admin_setfame <fame>`: `Player.setFame` (clamped), the player's `UserInfo` and two
/// messages. See the module note for the `EnableFameSystem` check.
fn admin_setfame(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(fame) = command.get(14..).and_then(|v| parse_int(v).ok()) else {
        world.send_sys_message(gm, "Usage: //setfame <new_fame_value>");
        return Ok(());
    };
    let Some(target) = target_player(world, gm) else {
        invalid_target(world, gm);
        return Ok(());
    };
    world.set_fame(target, fame);
    world.broadcast_user_info(target);
    world.send_message(target, &format!("A GM changed your Reputation points to {fame}"));
    let name = world.object_name(target).unwrap_or_default();
    world.send_message(gm, &format!("{name}'s Fame changed to {fame}"));
    Ok(())
}

/// Java `admin_rec <number>`: the recommendations of the target player, the vote window when
/// Nevit is on, the two messages, and the store of the two counters.
fn admin_rec(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(rec) = command.get(10..).and_then(|v| parse_int(v).ok()) else {
        world.send_sys_message(gm, "Usage: //rec number");
        return Ok(());
    };
    let Some(target) = target_player(world, gm) else {
        invalid_target(world, gm);
        return Ok(());
    };
    world.set_recom_have(target, rec);
    world.broadcast_user_info(target);
    if world.settings.nevit_enabled {
        world.send_vote_system_info(target);
    }
    world.send_message(target, &format!("A GM changed your Recommend points to {rec}"));
    let name = world.object_name(target).unwrap_or_default();
    world.send_message(gm, &format!("{name}'s Recommend changed to {rec}"));
    world.store_recommendation_values(target);
    Ok(())
}

/// Java `Integer.decode("0x" + text)`: hex digits only (Java refuses a sign after the prefix),
/// the value must fit an `int`.
fn decode_hex_color(text: &str) -> Option<i32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    i32::try_from(value).ok()
}

/// Java `admin_settitle <text>`: the title of the target (cut to 20 characters when it is longer
/// than 21, Java's `Creature.setTitle`), then its title is broadcast.
// NOTE(java-quirk): `Creature.setTitle` cuts a player's title to `substring(0, 20)` when it is
// longer than 21 characters, so a title of 22 characters keeps 20 of them.
fn admin_settitle(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(text) = command.get(15..) else {
        world.send_sys_message(gm, "You need to specify the new title.");
        return Ok(());
    };
    let Some(target) = target_player(world, gm) else {
        return Ok(());
    };
    let units: Vec<u16> = text.encode_utf16().collect();
    let title = if units.len() > 21 {
        String::from_utf16_lossy(&units[..20])
    } else {
        text.to_owned()
    };
    if let Some(player) = world.players.get_mut(&target) {
        player.data.title = title;
    }
    world.send_message(target, "Your title has been changed by a GM");
    broadcast_title_info(world, target);
    Ok(())
}

/// Java `Player.broadcastTitleInfo()`: `UserInfo`, then the new title to everybody who sees the
/// player (`NicknameChanged`). The same body as the world's own `broadcastTitleInfo`.
fn broadcast_title_info(world: &mut World, id: ObjectId) {
    world.send_user_info(id);
    let Some(title) = world.players.get(&id).map(|p| p.data.title.clone()) else {
        return;
    };
    world.broadcast_with_self(
        id,
        &NicknameChanged {
            object_id: id.0,
            title,
        },
    );
}

/// Java `admin_settcolor <hex>`: the title colour of the target.
fn admin_settcolor(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `command.substring(16)` first, as in Java: a missing value is the message even without a target.
    let Some(text) = command.get(16..) else {
        world.send_sys_message(gm, "You need to specify a valid new color.");
        return Ok(());
    };
    let Some(target) = target_player(world, gm) else {
        return Ok(());
    };
    let Some(color) = decode_hex_color(text) else {
        world.send_sys_message(gm, "You need to specify a valid new color.");
        return Ok(());
    };
    if let Some(player) = world.players.get_mut(&target) {
        player.data.profile.title_color = color;
    }
    world.send_message(target, "Your title color has been changed by a GM");
    world.broadcast_user_info(target);
    Ok(())
}

/// Java `admin_changename <new name>`: the name of the target, stored at once, then the party and
/// clan windows show the new name. A name another character has is refused.
fn admin_changename(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(new_name) = command.get(17..) else {
        world.send_sys_message(gm, "Usage: //setname new_name_for_target");
        return Ok(());
    };
    let Some(target) = target_player(world, gm) else {
        return Ok(());
    };
    if world.char_id_by_name(new_name).is_some() {
        world.send_sys_message(gm, &format!("Warning, player {new_name} already exists"));
        return Ok(());
    }
    if let Some(player) = world.players.get_mut(&target) {
        player.data.name = new_name.to_owned();
    }
    store_me(world, target);
    world.send_sys_message(gm, &format!("Changed name to {new_name}"));
    world.send_message(target, "Your name has been changed by a GM.");
    world.broadcast_user_info(target);

    // The party windows of the other members: the target's line is deleted and added again.
    if let Some(party) = world.party_of(target) {
        world.broadcast_party_except(party, target, &PartySmallWindowDeleteAll);
        for member in world.party_members(party) {
            if member == target {
                continue;
            }
            if let Some(window) = world.party_small_window_all(member, party) {
                world.send(member, &window);
            }
        }
    }
    if let Some(clan) = world.clan_of(target).map(|c| c.id) {
        world.broadcast_clan_status(clan);
    }
    Ok(())
}

/// Java `Player.storeMe()`: the whole character is written with the next outbox turn.
fn store_me(world: &mut World, id: ObjectId) {
    if let Some(snapshot) = world.store_snapshot(id) {
        world
            .outbox
            .persist(PersistCommand::StorePlayer(Box::new(snapshot)));
    }
}

/// Java `admin_setsex`: the sex of the target flips. The texture refresh is not ported (see the
/// module note).
fn admin_setsex(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(target) = target_player(world, gm) else {
        return Ok(());
    };
    if let Some(player) = world.players.get_mut(&target) {
        // `PlayerAppearance.isFemale()`: `sex` 1 is female, 0 male (`characters.sex`).
        player.data.sex = if player.data.sex == 1 { 0 } else { 1 };
    }
    world.send_message(target, "Your gender has been changed by a GM");
    world.broadcast_user_info(target);
    Ok(())
}

/// Java `admin_fullfood`: a pet's food bar is filled up.
fn admin_fullfood(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    match world.target_of(gm).filter(|t| world.is_pet(*t)) {
        Some(pet) => {
            let max = world.pet_max_fed(pet);
            world.pet_set_current_fed(pet, max);
            world.broadcast_status_update(pet);
        }
        None => invalid_target(world, gm),
    }
    Ok(())
}

/// Java `admin_setnoble`: the noble flag of the target, or of the GM with no target. The noble
/// skills are not ported (see the module note).
fn admin_setnoble(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let player = match world.target_of(gm) {
        None => Some(gm),
        Some(_) => target_player(world, gm),
    };
    let Some(player) = player else {
        return Ok(());
    };
    if let Some(p) = world.players.get_mut(&player) {
        p.data.profile.noble = !p.data.profile.noble;
    }
    if player != gm {
        let name = world.object_name(player).unwrap_or_default();
        world.send_sys_message(gm, &format!("You've changed nobless status of: {name}"));
    }
    world.send_message(player, "GM changed your nobless status!");
    Ok(())
}

/// Java `Double.parseDouble(text)` for the forms an admin types: optional surrounding white space,
/// a sign, digits with an optional point and exponent, an optional `d`/`f` suffix, `NaN` and
/// `Infinity`.
fn java_parse_double(text: &str) -> Option<f64> {
    let text = text.trim_matches(|c: char| c <= ' ');
    let body = text.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(text);
    let unsigned = body.strip_prefix(['+', '-']).unwrap_or(body);
    if unsigned == "NaN" || unsigned == "Infinity" {
        return body.parse::<f64>().ok().or(match unsigned {
            "NaN" => Some(f64::NAN),
            _ if body.starts_with('-') => Some(f64::NEG_INFINITY),
            _ => Some(f64::INFINITY),
        });
    }
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(at) => (&unsigned[..at], Some(&unsigned[at + 1..])),
        None => (unsigned, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, frac)) => {
            (whole.is_empty() || digits(whole))
                && (frac.is_empty() || digits(frac))
                && !(whole.is_empty() && frac.is_empty())
        }
        None => digits(mantissa),
    };
    let exponent_ok = match exponent {
        None => true,
        Some(e) => digits(e.strip_prefix(['+', '-']).unwrap_or(e)),
    };
    if !mantissa_ok || !exponent_ok {
        return None;
    }
    body.parse::<f64>().ok()
}

/// Java `admin_set_hp|mp|cp <value>`: the current vital of the target creature, then its
/// `UserInfo` when it is a player. A creature that is not the target is the invalid target.
fn set_vital(world: &mut World, gm: ObjectId, command: &str, vital: Vital) -> Outcome {
    let words = java_split_space(command);
    let usage = match vital {
        Vital::Hp => "Usage: //set_hp 1000",
        Vital::Mp => "Usage: //set_mp 1000",
        Vital::Cp => "Usage: //set_cp 1000",
    };
    let Some(target) = target_creature(world, gm) else {
        invalid_target(world, gm);
        return Ok(());
    };
    let Some(value) = words.get(1).and_then(|text| java_parse_double(text)) else {
        world.send_sys_message(gm, usage);
        return Ok(());
    };
    match vital {
        Vital::Hp => {
            world.set_current_hp(target, value, true);
        }
        Vital::Mp => {
            world.set_current_mp(target, value, true);
        }
        Vital::Cp => {
            world.set_current_cp(target, value, true);
        }
    }
    if world.is_player(target) {
        world.send_user_info(target);
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Vital {
    Hp,
    Mp,
    Cp,
}

fn admin_set_hp(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    set_vital(world, gm, command, Vital::Hp)
}

fn admin_set_mp(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    set_vital(world, gm, command, Vital::Mp)
}

fn admin_set_cp(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    set_vital(world, gm, command, Vital::Cp)
}

/// Java `admin_setclass <classId>`: the class of the target. A class that is not in the enum, or
/// the class it already has, shows the usage line. The academy graduation runs first, as in Java.
/// The texture refresh and the class template are not ported (see the module note).
fn admin_setclass(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(text) = command.get(15..) else {
        world.show_admin_html(gm, "setclass/human_fighter.htm");
        return Ok(());
    };
    let Some(new_class) = parse_int(text.trim()).ok() else {
        world.send_sys_message(gm, "Usage: //setclass <valid_new_classid>");
        return Ok(());
    };
    let Some(target) = target_player(world, gm) else {
        return Ok(());
    };
    let valid = u16::try_from(new_class)
        .ok()
        .map(ClassId)
        .is_some_and(|c| c.enum_name().is_some());
    let current = world.players.get(&target).map_or(-1, |p| p.data.class_id);
    if !valid || current == new_class {
        world.send_sys_message(gm, "Usage: //setclass <valid_new_classid>");
        return Ok(());
    }

    // Java `isSubClassActive()` before the class changes (the world's rule: the class is not the
    // base class).
    let sub_active = world
        .players
        .get(&target)
        .is_some_and(|p| p.data.class_id != p.data.base_class);
    world.academy_graduation(target, new_class);
    let kamael_female = [124, 126, 129, 130, 133, 134];
    let kamael_male = [123, 125, 127, 128, 131, 132];
    if let Some(player) = world.players.get_mut(&target) {
        player.data.class_id = new_class;
        if !sub_active {
            player.data.base_class = new_class;
        }
        // Java's sex rule for a Kamael: the class decides the sex (the appearance is changed here,
        // the texture refresh is not).
        if player.data.race == 5 {
            if kamael_male.contains(&new_class) {
                player.data.sex = 0;
            } else if kamael_female.contains(&new_class) {
                player.data.sex = 1;
            }
        }
    }
    let new_name = class_name_of(world, new_class)?;
    store_me(world, target);
    world.send_message(target, &format!("A GM changed your class to {new_name}."));
    world.broadcast_user_info(target);
    let name = world.object_name(target).unwrap_or_default();
    world.send_message(gm, &format!("{name} is a {new_name}."));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colour_is_java_integer_decode() {
        assert_eq!(decode_hex_color("FF00FF"), Some(0xFF00FF));
        assert_eq!(decode_hex_color("7fffffff"), Some(i32::MAX));
        // A value above `Integer.MAX_VALUE` does not fit, a sign after `0x` is refused.
        assert_eq!(decode_hex_color("80000000"), None);
        assert_eq!(decode_hex_color("-5"), None);
        assert_eq!(decode_hex_color("+5"), None);
        assert_eq!(decode_hex_color(""), None);
        assert_eq!(decode_hex_color("GG"), None);
    }

    #[test]
    fn double_is_java_parse_double() {
        assert_eq!(java_parse_double("12"), Some(12.0));
        assert_eq!(java_parse_double(" 3.5 "), Some(3.5));
        assert_eq!(java_parse_double("1e3"), Some(1000.0));
        assert_eq!(java_parse_double("-2.5E-1"), Some(-0.25));
        assert_eq!(java_parse_double("5d"), Some(5.0));
        assert_eq!(java_parse_double("7F"), Some(7.0));
        assert_eq!(java_parse_double(".5"), Some(0.5));
        assert!(java_parse_double("NaN").is_some_and(f64::is_nan));
        assert_eq!(java_parse_double("-Infinity"), Some(f64::NEG_INFINITY));
        for bad in ["", ".", "abc", "1e", "1e+", "0x10", "inf", "nan", "1.2.3", "--1"] {
            assert_eq!(java_parse_double(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn ipv4_is_the_java_pattern() {
        assert!(is_ipv4("10.0.0.1"));
        assert!(is_ipv4("0.0.0.0"));
        assert!(is_ipv4("255.249.199.100"));
        for bad in ["010.0.0.1", "256.1.1.1", "1.2.3", "1.2.3.4.5", "a.b.c.d", ""] {
            assert!(!is_ipv4(bad), "{bad}");
        }
    }

    #[test]
    fn percent_is_a_float_with_two_decimals() {
        assert_eq!(percent_text(50, 100), "50.00");
        assert_eq!(percent_text(1, 3), "33.33");
        assert_eq!(percent_text(0, 0), "NaN");
        assert_eq!(percent_text(1, 0), "Infinity");
    }

    #[test]
    fn string_hash_is_java_hash_code() {
        assert_eq!(java_string_hash("a"), 97);
        assert_eq!(java_string_hash("hello"), 99_162_322);
        // "Aa" and "BB" collide in Java.
        assert_eq!(java_string_hash("Aa"), java_string_hash("BB"));
    }

    #[test]
    fn hashmap_order_is_by_bucket_then_insertion() {
        // "a" is bucket 1 and "b" bucket 2 of a 16 table: "b" inserted first still comes second.
        let keys = vec!["b".to_owned(), "a".to_owned()];
        assert_eq!(java_hashmap_order(&keys), vec!["a".to_owned(), "b".to_owned()]);
        // Two keys of one bucket keep their insertion order.
        let keys = vec!["BB".to_owned(), "Aa".to_owned()];
        assert_eq!(java_hashmap_order(&keys), vec!["BB".to_owned(), "Aa".to_owned()]);
        let keys = vec!["Aa".to_owned(), "BB".to_owned()];
        assert_eq!(java_hashmap_order(&keys), vec!["Aa".to_owned(), "BB".to_owned()]);
    }
}
