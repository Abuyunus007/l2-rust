//! The GM Territory War commands: `//territory_war`, `//territory_war_time`,
//! `//territory_war_start`, `//territory_war_end` and `//territory_wards_list`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminTerritoryWar.java
//
// A GM can run a test war on this world whatever `SiegeDatabaseWrites` says (D-51): the start
// command marks the start as a GM's, the war runs here, and its rows are written only with the
// switch (the new start date of `//territory_war_time` too).

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;

use super::TerritoryWrite;
use crate::admin::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::siege::{civil_from_days, days_from_civil, days_in_month, java_date_string};
use crate::world::World;

type Outcome = Result<(), JavaException>;

const DAY_MS: i64 = 86_400_000;

/// Java `AdminTerritoryWar.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_territory_war", admin_territory_war),
        ("admin_territory_war_time", admin_territory_war),
        ("admin_territory_war_start", admin_territory_war),
        ("admin_territory_war_end", admin_territory_war),
        ("admin_territory_wards_list", admin_territory_war),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminTerritoryWar.onCommand`.
fn admin_territory_war(world: &mut World, gm: ObjectId, command_value: &str) -> Outcome {
    let mut st = Tokens::new(command_value);
    let command = st.next_token().unwrap_or("");
    if command == "admin_territory_war" {
        world.show_admin_html(gm, "territorywar.htm");
    } else if command.eq_ignore_ascii_case("admin_territory_war_time") {
        if st.has_more() && !set_war_time(world, gm, &mut st)? {
            return Ok(());
        }
        show_siege_time_page(world, gm);
    } else if command.eq_ignore_ascii_case("admin_territory_war_start") {
        // D-51: a GM's start runs the war in this world.
        world.territory.gm_start = true;
        let now = world.tw_now_ms();
        world.tw_set_start_time(now);
    } else if command.eq_ignore_ascii_case("admin_territory_war_end") {
        let now = world.tw_now_ms();
        let length = world.territory.settings.war_length_ms;
        world.tw_set_start_time(now - length);
    } else if command.eq_ignore_ascii_case("admin_territory_wards_list") {
        show_wards_list(world, gm);
    }
    Ok(())
}

/// One calendar field of the start date changes (`month` adds, the others set); a date in the
/// past is refused. `false` where Java returns before the page.
fn set_war_time(world: &mut World, gm: ObjectId, st: &mut Tokens<'_>) -> Result<bool, JavaException> {
    let now = world.tw_now_ms();
    let start = world.territory.start_tw_date;
    let offset = world.siege_offset_at(start);
    let local = start + offset;
    let (mut year, mut month, mut day) = civil_from_days(local.div_euclid(DAY_MS));
    let ms_of_day = local.rem_euclid(DAY_MS);
    let (mut hour, mut minute) = (ms_of_day / 3_600_000, (ms_of_day / 60_000) % 60);
    let rest = ms_of_day % 60_000;
    let val = st.next_token().unwrap_or_default();
    let mut value = || -> Result<i64, JavaException> {
        let token = st
            .next_token()
            .ok_or_else(|| JavaException("java.util.NoSuchElementException".to_owned()))?;
        parse_int(token).map(i64::from)
    };
    match val {
        "month" => {
            // `cal.get(MONTH) + n` (0 to 11).
            let new = (month - 1) + value()?;
            if !(0..=11).contains(&new) {
                world.send_sys_message(
                    gm,
                    "Unable to change Siege Date - Incorrect month value only 0-11 is accepted!",
                );
                return Ok(false);
            }
            month = new + 1;
        }
        "day" => {
            let new = value()?;
            let max = days_in_month(year, month);
            if !(1..=max).contains(&new) {
                world.send_sys_message(
                    gm,
                    &format!("Unable to change Siege Date - Incorrect day value only 1-{max} is accepted!"),
                );
                return Ok(false);
            }
            day = new;
        }
        "hour" => {
            let new = value()?;
            if !(0..=23).contains(&new) {
                world.send_sys_message(
                    gm,
                    "Unable to change Siege Date - Incorrect hour value only 0-23 is accepted!",
                );
                return Ok(false);
            }
            hour = new;
        }
        "min" => {
            let new = value()?;
            if !(0..=59).contains(&new) {
                world.send_sys_message(
                    gm,
                    "Unable to change Siege Date - Incorrect minute value only 0-59 is accepted!",
                );
                return Ok(false);
            }
            minute = new;
        }
        _ => {}
    }
    // The lenient calendar: a day past the month's end rolls into the next month.
    let max = days_in_month(year, month);
    if day > max {
        day -= max;
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    let new_local = days_from_civil(year, month, day) * DAY_MS + hour * 3_600_000 + minute * 60_000 + rest;
    let new_date = new_local - world.siege_offset_at(new_local - offset);
    if new_date < now {
        world.send_sys_message(gm, "Unable to change TW Date!");
    } else if new_date != start {
        world.tw_set_start_time(new_date);
        world.territory.next_tw_start_date = new_date;
        world.territory_write(TerritoryWrite::NextTwStartDate(new_date));
    }
    Ok(true)
}

/// `showSiegeTimePage`: `territorywartime.htm` with the start date.
fn show_siege_time_page(world: &mut World, gm: ObjectId) {
    let start = world.territory.start_tw_date;
    let mut html = world.admin_html_file(gm, "data/html/admin/territorywartime.htm");
    html.replace("%time%", &java_date_string(start, world.siege_offset_at(start)));
    world.send_admin_html(gm, &html);
}

/// `admin_territory_wards_list`: where each ward of the war is, with a button to go there.
fn show_wards_list(world: &mut World, gm: ObjectId) {
    let mut sb = String::from(
        "<html><title>Territory War</title><body><br><center><font color=\"LEVEL\">Active Wards List:</font></center>",
    );
    if world.is_tw_in_progress() {
        let wards = world.territory.wards.clone();
        let templates = world.item_templates();
        for ward in wards {
            let npc = ward
                .npc
                .and_then(|n| world.position_of(n).map(|pos| (world.creature_name(n), pos)));
            let row = match (npc, ward.player) {
                (Some((name, (x, y, z))), _) => Some((name, (x, y, z))),
                (None, Some(player)) => world.position_of(player).map(|pos| {
                    let item = templates
                        .as_deref()
                        .and_then(|t| t.get(ward.item_id))
                        .map(|t| t.name.clone())
                        .unwrap_or_default();
                    let name = world
                        .players
                        .get(&player)
                        .map(|p| p.data.name.clone())
                        .unwrap_or_default();
                    (format!("{item} - {name}"), pos)
                }),
                (None, None) => None,
            };
            if let Some((label, (x, y, z))) = row {
                sb.push_str("<table width=270><tr>");
                sb.push_str(&format!("<td width=135 ALIGN=\"LEFT\">{label}</td>"));
                sb.push_str(&format!(
                    "<td width=135 ALIGN=\"RIGHT\"><button value=\"TeleTo\" action=\"bypass -h admin_move_to {x} {y} {z}\" width=50 height=20 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_ct1.button_df\"></td>"
                ));
                sb.push_str("</tr></table>");
            }
        }
        sb.push_str("<br><center><button value=\"Back\" action=\"bypass -h admin_territory_war\" width=50 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>");
    } else {
        sb.push_str("<br><br><center>The Ward List is empty!<br>TW has probably NOT started!");
        sb.push_str("<br><button value=\"Back\" action=\"bypass -h admin_territory_war\" width=50 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>");
    }
    // `new NpcHtmlMessage(0, 1)`.
    if let Some(message) = NpcHtmlMessage::new_item(0, 1, &sb) {
        world.send_html(gm, &message);
    }
}
