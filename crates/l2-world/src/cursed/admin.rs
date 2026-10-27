//! The cursed weapon admin commands: `//cw_info`, `//cw_info_menu`, `//cw_reload`, `//cw_remove`,
//! `//cw_goto` and `//cw_add`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminCursedWeapons.java
//
// # Left out
// - `//cw_reload` (Java `CursedWeaponsManager.reload()`) reads `CursedWeapons.xml` and the
//   `cursed_weapons` table again; the world never reads files or the database (D-15). The GM is
//   told to restart the server. `//reload cw` is in the not-reloadable list for the same reason.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::admin::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric};
use crate::teleport::TeleportTarget;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// `MS_PER_MINUTE` of `getDuration() * 60000` (an `int` product in Java's `setEndTime` argument).
const MS_PER_MINUTE: i32 = 60_000;

/// Java `AdminCursedWeapons` registrations.
pub fn register_admin(handlers: &mut AdminHandlers) {
    let handler: AdminHandler = admin_cursed_weapons;
    for command in [
        "admin_cw_info",
        "admin_cw_remove",
        "admin_cw_goto",
        "admin_cw_reload",
        "admin_cw_add",
        "admin_cw_info_menu",
    ] {
        handlers.register(command, handler);
    }
}

/// Java `AdminCursedWeapons.onCommand`.
fn admin_cursed_weapons(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_cw_info") {
        if !command.contains("menu") {
            cw_info(world, gm);
        } else {
            cw_info_menu(world, gm);
        }
    } else if command.starts_with("admin_cw_reload") {
        // `cwm.reload()`.
        world.send_sys_message(
            gm,
            "The cursed weapons cannot be reloaded while the server runs: restart the server.",
        );
    } else {
        cw_by_id(world, gm, command);
    }
    Ok(())
}

/// `admin_cw_info`: one block of system messages per weapon.
fn cw_info(world: &mut World, gm: ObjectId) {
    world.send_sys_message(gm, "====== Cursed Weapons: ======");
    let now = world.cursed_now_ms();
    for index in 0..world.cursed.weapons.len() {
        let weapon = world.cursed.weapons[index].clone();
        world.send_sys_message(gm, &format!("> {} ({})", weapon.name, weapon.item_id));
        let minutes = (weapon.end_time - now) / 60_000;
        if weapon.is_activated {
            let holder = weapon
                .player
                .and_then(|p| world.players.get(&p))
                .map(|p| p.data.name.clone());
            world.send_sys_message(
                gm,
                &format!("  Player holding: {}", holder.as_deref().unwrap_or("null")),
            );
            world.send_sys_message(gm, &format!("    Player karma: {}", weapon.player_karma));
            world.send_sys_message(gm, &format!("    Time Remaining: {minutes} min."));
            world.send_sys_message(gm, &format!("    Kills : {}", weapon.nb_kills));
        } else if weapon.is_dropped {
            world.send_sys_message(gm, "  Lying on the ground.");
            world.send_sys_message(gm, &format!("    Time Remaining: {minutes} min."));
            world.send_sys_message(gm, &format!("    Kills : {}", weapon.nb_kills));
        } else {
            world.send_sys_message(gm, "  Don't exist in the world.");
        }
        world.send(gm, &SystemMessage::new(SystemMessageId::Empty3));
    }
}

/// The two buttons of a weapon that exists (`Remove` and `Go`).
fn remove_go_buttons(item_id: i32) -> String {
    format!(
        "<button value=\"Remove\" action=\"bypass -h admin_cw_remove {item_id}\" width=73 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>\
<td><button value=\"Go\" action=\"bypass -h admin_cw_goto {item_id}\" width=73 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr>"
    )
}

/// `admin_cw_info_menu`: `data/html/admin/cwinfo.htm` with a table per weapon.
fn cw_info_menu(world: &mut World, gm: ObjectId) {
    let now = world.cursed_now_ms();
    let mut reply = String::with_capacity(world.cursed.weapons.len() * 300);
    for weapon in world.cursed.weapons() {
        let item_id = weapon.item_id;
        reply.push_str("<table width=270><tr><td>Name:</td><td>");
        reply.push_str(&weapon.name);
        reply.push_str("</td></tr>");
        let minutes = (weapon.end_time - now) / 60_000;
        if weapon.is_activated {
            let holder = weapon
                .player
                .and_then(|p| world.players.get(&p))
                .map(|p| p.data.name.clone());
            reply.push_str("<tr><td>Weilder:</td><td>");
            reply.push_str(holder.as_deref().unwrap_or("null"));
            reply.push_str("</td></tr>");
            reply.push_str("<tr><td>Karma:</td><td>");
            reply.push_str(&weapon.player_karma.to_string());
            reply.push_str("</td></tr>");
            reply.push_str("<tr><td>Kills:</td><td>");
            reply.push_str(&format!("{}/{}", weapon.player_pk_kills, weapon.nb_kills));
            reply.push_str("</td></tr><tr><td>Time remaining:</td><td>");
            reply.push_str(&minutes.to_string());
            reply.push_str(" min.</td></tr>");
            reply.push_str("<tr><td>");
            reply.push_str(&remove_go_buttons(item_id));
        } else if weapon.is_dropped {
            reply.push_str(
                "<tr><td>Position:</td><td>Lying on the ground</td></tr><tr><td>Time remaining:</td><td>",
            );
            reply.push_str(&minutes.to_string());
            reply.push_str(" min.</td></tr><tr><td>Kills:</td><td>");
            reply.push_str(&weapon.nb_kills.to_string());
            reply.push_str("</td></tr><tr><td>");
            reply.push_str(&remove_go_buttons(item_id));
        } else {
            reply.push_str("<tr><td>Position:</td><td>Doesn't exist.</td></tr><tr><td><button value=\"Give to Target\" action=\"bypass -h admin_cw_add ");
            reply.push_str(&item_id.to_string());
            reply.push_str("\" width=130 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td></td></tr>");
        }
        reply.push_str("</table><br>");
    }
    let mut html = world.admin_html_file(gm, "data/html/admin/cwinfo.htm");
    html.replace("%cwinfo%", &reply);
    world.send_admin_html(gm, &html);
}

/// `admin_cw_remove`, `admin_cw_goto` and `admin_cw_add`: the weapon is the first parameter, an
/// item id or a part of its name (`_` for a space).
fn cw_by_id(world: &mut World, gm: ObjectId, command: &str) {
    let mut st = Tokens::new(command);
    st.next_token();
    let mut id = 0;
    let mut usage = false;
    match st.next_token() {
        None => usage = true,
        Some(parameter) if is_numeric(parameter) => match parameter.parse::<i32>() {
            Ok(value) => id = value,
            // `Integer.parseInt` throws for a number past an int.
            Err(_) => usage = true,
        },
        Some(parameter) => {
            let wanted = parameter.replace('_', " ").to_lowercase();
            if let Some(found) = world
                .cursed
                .weapons()
                .iter()
                .find(|w| w.name.to_lowercase().contains(&wanted))
            {
                id = found.item_id;
            }
        }
    }
    if usage {
        world.send_sys_message(gm, "Usage: //cw_remove|//cw_goto|//cw_add <itemid|name>");
    }
    let Some(index) = world.cursed.index_of(id).filter(|_| !usage) else {
        world.send_sys_message(gm, "Unknown cursed weapon ID.");
        return;
    };

    if command.starts_with("admin_cw_remove ") {
        world.cursed_end_of_life(index);
    } else if command.starts_with("admin_cw_goto ") {
        cw_go_to(world, gm, index);
    } else if command.starts_with("admin_cw_add") {
        if world.cursed.weapons[index].is_active() {
            world.send_sys_message(gm, "This cursed weapon is already active.");
        } else {
            let target = world.target_of(gm).filter(|t| world.is_player(*t)).unwrap_or(gm);
            world.add_item(target, id, 1, true);
            // `cw.setEndTime(now + duration * 60000)`, then `reActivate()`.
            let duration = world.cursed.weapons[index].duration;
            world.cursed.weapons[index].end_time = world
                .cursed_now_ms()
                .saturating_add(i64::from(duration.wrapping_mul(MS_PER_MINUTE)));
            world.cursed_re_activate(index);
        }
    } else {
        world.send_sys_message(gm, "Unknown command.");
    }
}

/// Java `CursedWeapon.goTo(player)`.
fn cw_go_to(world: &mut World, gm: ObjectId, index: usize) {
    let weapon = world.cursed.weapons[index].clone();
    let holder = weapon.player.filter(|_| weapon.is_activated).and_then(|p| {
        world
            .players
            .get(&p)
            .map(|pl| (pl.core.x, pl.core.y, pl.core.z, pl.core.heading))
    });
    let on_ground = weapon
        .item
        .filter(|_| weapon.is_dropped)
        .and_then(|i| world.ground.items.get(&i).map(|g| (g.x, g.y, g.z, 0)));
    // `teleToLocation(location, true)`: a random offset, the instance of the location.
    let offset = world.settings.teleport.max_offset_on_teleport;
    match holder.or(on_ground) {
        // Go to the player holding the weapon, or to the item on the ground.
        Some((x, y, z, heading)) => world.tele_to_location(
            gm,
            TeleportTarget::at(x, y, z)
                .with_heading(heading)
                .with_instance(0)
                .with_random_offset(offset),
        ),
        None => world.send_message(gm, &format!("{} isn't in the World.", weapon.name)),
    }
}
