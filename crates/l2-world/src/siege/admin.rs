//! The GM siege commands: `//siege [castle]`, `//add_attacker`, `//add_defender`, `//add_guard`,
//! `//list_siege_clans`, `//clear_siege_list`, `//move_defenders`, `//spawn_doors`,
//! `//startsiege`, `//endsiege`, `//setsiegetime`, `//setcastle`, `//removecastle`. A GM starts a
//! test siege with them (D-51).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSiege.java (the castle part)
//
// The clan hall words of `AdminSiege` (`//clanhall...`) are the clan hall cards' (M7-CH-01,
// M7-CHSIEGE-01: `admin/chsiege.rs`); the castle list page already lists the halls.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::admin::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::residence::{CastleId, ClanHallKind};
use crate::world::World;

type Outcome = Result<(), JavaException>;

const DAY_MS: i64 = 86_400_000;

/// The castle words of Java `AdminSiege`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_siege", admin_siege),
        ("admin_add_attacker", admin_siege),
        ("admin_add_defender", admin_siege),
        ("admin_add_guard", admin_siege),
        ("admin_list_siege_clans", admin_siege),
        ("admin_clear_siege_list", admin_siege),
        ("admin_move_defenders", admin_siege),
        ("admin_spawn_doors", admin_siege),
        ("admin_endsiege", admin_siege),
        ("admin_startsiege", admin_siege),
        ("admin_setsiegetime", admin_siege),
        ("admin_setcastle", admin_siege),
        ("admin_removecastle", admin_siege),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

fn null_pointer() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

/// Java `AdminSiege.onCommand` for the castle words.
fn admin_siege(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    let word = st.next_token().unwrap_or_default();
    let Some(value) = st.next_token() else {
        show_castle_select_page(world, gm);
        return Ok(());
    };
    let player = world.target_of(gm).filter(|t| world.is_player(*t));
    let castle = world.castle_by_name(value).ok_or_else(null_pointer)?;
    let invalid_target = |world: &World| {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
    };
    match word {
        "admin_add_attacker" => match player {
            None => invalid_target(world),
            Some(player) => world.siege_register_attacker(castle, player, true),
        },
        "admin_add_defender" => match player {
            None => invalid_target(world),
            Some(player) => world.siege_register_defender(castle, player, true),
        },
        "admin_add_guard" => {
            if let Some(npc) = st.next_token()
                && is_numeric(npc)
            {
                let npc_id = parse_int(npc)?;
                world.siege_add_siege_guard(castle, gm, npc_id);
                return Ok(());
            }
            // If doesn't have more tokens or token is not a number.
            world.send_sys_message(gm, "Usage: //add_guard castle npcId");
        }
        "admin_clear_siege_list" => world.siege_clear_siege_clan(castle),
        "admin_endsiege" => world.end_siege(castle),
        "admin_list_siege_clans" => world.send_siege_info(gm, castle),
        "admin_move_defenders" => world.send_sys_message(gm, "Not implemented yet."),
        "admin_setcastle" => match player.filter(|p| world.clan_of(*p).is_some()) {
            None => invalid_target(world),
            Some(player) => {
                let clan = world.clan_of(player).map(|c| c.id);
                world.castle_set_owner(castle, clan);
            }
        },
        "admin_removecastle" => {
            let owner = world.castle(castle).map_or(0, |c| c.owner_id);
            if world.clan(owner).is_some() {
                world.castle_remove_owner(castle, Some(owner));
            } else {
                world.send_sys_message(gm, "Unable to remove castle.");
            }
        }
        "admin_setsiegetime" => {
            if st.has_more() {
                set_siege_time(world, gm, castle, &mut st)?;
            }
            show_siege_time_page(world, gm, castle);
        }
        "admin_spawn_doors" => world.castle_spawn_door(castle, false),
        "admin_startsiege" => world.start_siege(castle),
        _ => {
            let name = world.castle(castle).map(|c| c.name.clone()).unwrap_or_default();
            show_siege_page(world, gm, &name);
        }
    }
    Ok(())
}

/// The `admin_setsiegetime` branch: one calendar field of the siege date changes; a date in the
/// past is refused.
fn set_siege_time(world: &mut World, gm: ObjectId, castle: CastleId, st: &mut Tokens<'_>) -> Outcome {
    let Some(siege_date) = world.castle(castle).map(|c| c.siege_date) else {
        return Ok(());
    };
    let now = world.siege_now_ms();
    let offset = world.siege_offset_at(siege_date);
    let local = siege_date + offset;
    let (mut year, mut month, mut day) = civil_from_days(local.div_euclid(DAY_MS));
    let ms_of_day = local.rem_euclid(DAY_MS);
    let (mut hour, mut minute) = (ms_of_day / 3_600_000, (ms_of_day / 60_000) % 60);
    let rest = ms_of_day % 60_000;
    let field = st.next_token().unwrap_or_default();
    let mut value =
        || -> Result<i32, JavaException> { parse_int(st.next_token().ok_or_else(no_such_element)?) };
    match field {
        "month" => {
            // `calendar.get(MONTH) + n`, then `set(MONTH, month)` (0 to 11).
            let new = (month - 1) + i64::from(value()?);
            if !(0..=11).contains(&new) {
                world.send_sys_message(
                    gm,
                    "Unable to change Siege Date - Incorrect month value only 0-11 is accepted!",
                );
                return Ok(());
            }
            month = new + 1;
        }
        "day" => {
            let new = i64::from(value()?);
            let max = days_in_month(year, month);
            if !(1..=max).contains(&new) {
                world.send_sys_message(
                    gm,
                    &format!("Unable to change Siege Date - Incorrect day value only 1-{max} is accepted!"),
                );
                return Ok(());
            }
            day = new;
        }
        "hour" => {
            let new = i64::from(value()?);
            if !(0..=23).contains(&new) {
                world.send_sys_message(
                    gm,
                    "Unable to change Siege Date - Incorrect hour value only 0-23 is accepted!",
                );
                return Ok(());
            }
            hour = new;
        }
        "min" => {
            let new = i64::from(value()?);
            if !(0..=59).contains(&new) {
                world.send_sys_message(
                    gm,
                    "Unable to change Siege Date - Incorrect minute value only 0-59 is accepted!",
                );
                return Ok(());
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
        world.send_sys_message(gm, "Unable to change Siege Date");
    } else if new_date != siege_date {
        if let Some(c) = world.castle_mut(castle) {
            c.siege_date = new_date;
        }
        world.siege_save_siege_date(castle);
        let name = world.castle(castle).map(|c| c.name.clone()).unwrap_or_default();
        world.send_sys_message(
            gm,
            &format!("Castle siege time for castle {name} has been changed."),
        );
    }
    Ok(())
}

fn no_such_element() -> JavaException {
    JavaException("java.util.NoSuchElementException".to_owned())
}

/// `showCastleSelectPage`: the castles, the conquerable halls and the clan halls (owned, then
/// free).
fn show_castle_select_page(world: &mut World, gm: ObjectId) {
    let mut html = world.admin_html_file(gm, "data/html/admin/castles.htm");
    let rows = |cells: Vec<String>, per_row: usize| {
        let mut out = String::new();
        let mut i = 0;
        for cell in cells {
            out.push_str(&cell);
            i += 1;
            if i > per_row {
                out.push_str("</tr><tr>");
                i = 0;
            }
        }
        out
    };
    let castles: Vec<String> = world
        .castles()
        .iter()
        .map(|c| {
            format!(
                "<td fixwidth=90><a action=\"bypass -h admin_siege {0}\">{0}</a></td>",
                c.name
            )
        })
        .collect();
    let halls = |siegable: bool, owned: Option<bool>| -> Vec<String> {
        world
            .clan_halls()
            .filter(|h| matches!(h.kind, ClanHallKind::Siegable { .. }) == siegable)
            .filter(|h| owned.is_none_or(|o| (h.owner_id > 0) == o))
            .map(|h| {
                if siegable {
                    format!(
                        "<td fixwidth=90><a action=\"bypass -h admin_chsiege_siegablehall {}\">{}</a></td>",
                        h.id.0, h.name
                    )
                } else {
                    format!(
                        "<td fixwidth=134><a action=\"bypass -h admin_clanhall {}\">{}</a></td>",
                        h.id.0, h.name
                    )
                }
            })
            .collect()
    };
    let castle_cells = rows(castles, 2);
    let siegable = rows(halls(true, None), 1);
    let owned = rows(halls(false, Some(true)), 1);
    let free = rows(halls(false, Some(false)), 1);
    html.replace("%castles%", &castle_cells);
    html.replace("%siegableHalls%", &siegable);
    html.replace("%clanhalls%", &owned);
    html.replace("%freeclanhalls%", &free);
    world.send_admin_html(gm, &html);
}

/// `showSiegePage`: `castle.htm` with the castle's name.
fn show_siege_page(world: &mut World, gm: ObjectId, name: &str) {
    let mut html = world.admin_html_file(gm, "data/html/admin/castle.htm");
    html.replace("%castleName%", name);
    world.send_admin_html(gm, &html);
}

/// `showSiegeTimePage`: the siege date and the links of the next Saturday and Sunday.
// M8: `SevenSigns.isDateInSealValidPeriod(newDay)` moves the days a week on outside the seal
// validation period; until the Seven Signs exist the period is valid.
fn show_siege_time_page(world: &mut World, gm: ObjectId, castle: CastleId) {
    let Some((name, siege_date)) = world.castle(castle).map(|c| (c.name.clone(), c.siege_date)) else {
        return;
    };
    let mut html = world.admin_html_file(gm, "data/html/admin/castlesiegetime.htm");
    html.replace("%castleName%", &name);
    html.replace(
        "%time%",
        &java_date_string(siege_date, world.siege_offset_at(siege_date)),
    );
    let now = world.siege_now_ms();
    let local_day = (now + world.siege_offset_at(now)).div_euclid(DAY_MS);
    // 0 = Sunday ... 6 = Saturday.
    let weekday = (local_day + 4).rem_euclid(7);
    let (saturday, sunday) = if weekday == 0 {
        // `isSunday`: the Sunday is today, the Saturday 13 days later.
        (local_day + 13, local_day)
    } else {
        // `set(DAY_OF_WEEK, SATURDAY)`, the Sunday after it.
        let saturday = local_day + (6 - weekday);
        (saturday, saturday + 1)
    };
    let link = |day: i64| day_of_year(day).to_string();
    // `get(MONTH) + "/" + get(DAY_OF_MONTH)`: the month counts from 0.
    let label = |day: i64| {
        let (_, month, dom) = civil_from_days(day);
        format!("{}/{}", month - 1, dom)
    };
    html.replace("%sundaylink%", &link(sunday));
    html.replace("%sunday%", &label(sunday));
    html.replace("%saturdaylink%", &link(saturday));
    html.replace("%saturday%", &label(saturday));
    world.send_admin_html(gm, &html);
}

/// Java `Date.toString()`: `EEE MMM dd HH:mm:ss zzz yyyy`. The zone is written as `GMT+hh:mm`
/// (the machine's zone name is not known here).
fn java_date_string(utc: i64, offset: i64) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let local = utc + offset;
    let day = local.div_euclid(DAY_MS);
    let ms = local.rem_euclid(DAY_MS);
    let (year, month, dom) = civil_from_days(day);
    let weekday = usize::try_from((day + 4).rem_euclid(7)).unwrap_or(0);
    let month_name = usize::try_from(month - 1)
        .ok()
        .and_then(|m| MONTHS.get(m))
        .copied()
        .unwrap_or("");
    let sign = if offset < 0 { '-' } else { '+' };
    let offset_minutes = (offset / 60_000).abs();
    format!(
        "{} {} {:02} {:02}:{:02}:{:02} GMT{}{:02}:{:02} {}",
        DAYS[weekday],
        month_name,
        dom,
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60,
        sign,
        offset_minutes / 60,
        offset_minutes % 60,
        year
    )
}

/// Days since 1970-01-01 to `(year, month 1..12, day 1..31)` (proleptic Gregorian).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// `(year, month 1..12, day)` to days since 1970-01-01.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(year: i64, month: i64) -> i64 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    days_from_civil(next_year, next_month, 1) - days_from_civil(year, month, 1)
}

/// `Calendar.DAY_OF_YEAR` of a local day.
fn day_of_year(day: i64) -> i64 {
    let (year, _, _) = civil_from_days(day);
    day - days_from_civil(year, 1, 1) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_civil_dates_round_trip() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        let day = days_from_civil(2026, 11, 1);
        assert_eq!(civil_from_days(day), (2026, 11, 1));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(day_of_year(days_from_civil(2026, 2, 1)), 32);
        // 2026-11-01 16:00 UTC was a Sunday.
        assert_eq!(
            java_date_string(1_793_548_800_789, 0),
            "Sun Nov 01 16:00:00 GMT+00:00 2026"
        );
    }
}
