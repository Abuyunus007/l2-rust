//! The GM commands of the conquerable hall sieges: `//chsiege_startSiege`, `_endsSiege`,
//! `_setSiegeDate`, `_addAttacker`, `_removeAttacker`, `_clearAttackers`, `_listAttackers` and
//! `_forwardSiege`, and the page they all end with (`siegablehall.htm`). A GM starts a test siege
//! with them (D-51).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminCHSiege.java
//
// `admin_chsiege_siegablehall` (the page of a hall) is on the castle list page of `//siege`.
// `Clan Hall` words of `AdminSiege` (`//clanhall...`) are `admin/clanhall.rs` (M7-ADMIN-06).

use l2_util::ObjectId;
use tracing::warn;

use super::{AdminHandler, AdminHandlers, JavaException};
use crate::clanhall::SiegeStatus;
use crate::clanhall::siege::schedule::{date_at, java_date_string, year_of};
use crate::residence::ClanHallId;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_chsiege_siegablehall", admin_chsiege),
        ("admin_chsiege_startSiege", admin_chsiege),
        ("admin_chsiege_endsSiege", admin_chsiege),
        ("admin_chsiege_setSiegeDate", admin_chsiege),
        ("admin_chsiege_addAttacker", admin_chsiege),
        ("admin_chsiege_removeAttacker", admin_chsiege),
        ("admin_chsiege_clearAttackers", admin_chsiege),
        ("admin_chsiege_listAttackers", admin_chsiege),
        ("admin_chsiege_forwardSiege", admin_chsiege),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// `AdminCHSiege.parseInt`: 0 for a text that is no number (with a warning).
fn parse_int(text: &str) -> i32 {
    super::parse_int(text).unwrap_or_else(|e| {
        warn!("Problem with AdminCHSiege: {e}");
        0
    })
}

/// `String.split(" ")`: the empty strings at the end are dropped.
fn split(command: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = command.split(' ').collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// Java `AdminCHSiege.onCommand`.
fn admin_chsiege(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let parts = split(command);
    if world.clanhall.siege.settings.no_quests {
        world.send_sys_message(gm, "AltDevNoQuests = true; Clan Hall Sieges are disabled!");
        return Ok(());
    }
    if parts.len() < 2 {
        world.send_sys_message(gm, "You have to specify the hall id at least");
        return Ok(());
    }
    let Some(hall) = get_hall(world, parts[1], gm) else {
        world.send_sys_message(
            gm,
            &format!("Could not find he desired siegable hall ({})", parts[1]),
        );
        return Ok(());
    };
    if !world.hall_has_siege(hall) {
        world.send_sys_message(gm, "The given hall does not have any attached siege!");
        return Ok(());
    }
    match parts[0] {
        "admin_chsiege_startSiege" => start_siege(world, gm, hall),
        "admin_chsiege_endsSiege" => {
            if !world.hall_is_in_siege(hall) {
                world.send_sys_message(gm, "The requested clan hall is not in siege!");
            } else {
                world.hall_end_siege(hall);
            }
        }
        "admin_chsiege_setSiegeDate" => set_siege_date(world, gm, hall, &parts),
        "admin_chsiege_addAttacker" => {
            if world.hall_is_in_siege(hall) {
                world.send_sys_message(gm, "The clan hall is in siege, cannot add attackers now.");
                return Ok(());
            }
            if let Some(clan) = pick_clan(world, gm, hall, &parts, true) {
                world.hall_add_attacker(hall, clan);
            }
        }
        "admin_chsiege_removeAttacker" => {
            if world.hall_is_in_siege(hall) {
                world.send_sys_message(gm, "The clan hall is in siege, cannot remove attackers now.");
                return Ok(());
            }
            if let Some(clan) = pick_clan(world, gm, hall, &parts, false) {
                world.hall_remove_attacker(hall, clan);
            }
        }
        "admin_chsiege_clearAttackers" => {
            if world.hall_is_in_siege(hall) {
                world.send_sys_message(
                    gm,
                    "The requested hall is in siege right now, cannot clear attacker list!",
                );
            } else {
                world.hall_clear_attackers(hall);
            }
        }
        "admin_chsiege_listAttackers" => {
            if let Some(packet) = world.hall_siege_info_packet(gm, hall) {
                world.send(gm, &packet);
            }
        }
        "admin_chsiege_forwardSiege" => {
            world.hall_cancel_siege_task(hall);
            match world.hall_siege_status(hall) {
                Some(SiegeStatus::Registering) => world.hall_prepare_owner(hall),
                Some(SiegeStatus::WaitingBattle) => world.hall_start_siege(hall),
                Some(SiegeStatus::Running) => world.hall_end_siege(hall),
                None => {}
            }
        }
        _ => {}
    }
    send_siegable_hall_page(world, gm, parts[1], hall);
    Ok(())
}

/// `getHall(id, gm)`: the conquerable hall, with the GM told why not.
fn get_hall(world: &World, id: &str, gm: ObjectId) -> Option<ClanHallId> {
    let ch = parse_int(id);
    if ch == 0 {
        world.send_message(gm, "Wrong clan hall id, unparseable id!");
        return None;
    }
    if !world.is_siegable_hall_id(ch) {
        world.send_message(gm, "Could not find the clan hall.");
        return None;
    }
    Some(ClanHallId(ch))
}

/// `admin_chsiege_startSiege`: the owner becomes an attacker, the hall free.
fn start_siege(world: &mut World, gm: ObjectId, hall: ClanHallId) {
    if world.hall_is_in_siege(hall) {
        world.send_sys_message(gm, "The requested clan hall is alredy in siege!");
        return;
    }
    let owner = world.clan_hall(hall).map_or(0, |h| h.owner_id);
    if world.clan(owner).is_some() {
        world.clan_hall_free(hall);
        if let Some(clan) = world.clan_mut(owner) {
            clan.hideout_id = 0;
        }
        world.hall_add_attacker(hall, owner);
    }
    world.hall_start_siege(hall);
}

/// `admin_chsiege_setSiegeDate <id> DD-MM-YYYY;HH:MM`.
fn set_siege_date(world: &mut World, gm: ObjectId, hall: ClanHallId, parts: &[&str]) {
    if !world.hall_is_registering(hall) {
        world.send_sys_message(gm, "Cannot change siege date while hall is in siege");
        return;
    }
    if parts.len() < 3 {
        world.send_sys_message(gm, "The date format is incorrect. Try again.");
        return;
    }
    let raw: Vec<&str> = parts[2].split(';').collect();
    if raw.len() < 2 {
        world.send_sys_message(gm, "You have to specify this format DD-MM-YYYY;HH:MM");
        return;
    }
    let day: Vec<&str> = raw[0].split('-').collect();
    let hour: Vec<&str> = raw[1].split(':').collect();
    if day.len() < 3 || hour.len() < 2 {
        world.send_sys_message(gm, "Incomplete day, hour or both!");
        return;
    }
    let d = parse_int(day[0]);
    // `month` counts from 0 in Java's `Calendar`.
    let month = parse_int(day[1]) - 1;
    let year = parse_int(day[2]);
    let h = parse_int(hour[0]);
    let min = parse_int(hour[1]);
    let now = world.siege_now_ms();
    let offset = world.siege_offset_at(now);
    // NOTE(java-quirk): January is refused (`month <= 0`), and "March 29" is (`month == 2`).
    if (month == 2 && d > 28)
        || d > 31
        || d <= 0
        || month <= 0
        || month > 12
        || i64::from(year) < year_of(now, offset)
    {
        world.send_sys_message(gm, "Wrong day/month/year gave!");
    } else if h <= 0 || h > 24 || !(0..60).contains(&min) {
        world.send_sys_message(gm, "Wrong hour/minutes gave!");
    } else {
        let when = date_at(
            now,
            (i64::from(year), i64::from(month) + 1, i64::from(d)),
            (i64::from(h), i64::from(min)),
            &|t| world.siege_offset_at(t),
        );
        if when > now {
            let name = world.clan_hall(hall).map(|c| c.name.clone()).unwrap_or_default();
            world.send_message(
                gm,
                &format!(
                    "{name} siege: {}",
                    java_date_string(when, world.siege_offset_at(when))
                ),
            );
            world.hall_set_next_siege_date(hall, when);
        } else {
            world.send_sys_message(gm, "The given time is in the past!");
        }
    }
}

/// The clan of `addAttacker` and `removeAttacker`: the one named, or the target's.
fn pick_clan(world: &World, gm: ObjectId, hall: ClanHallId, parts: &[&str], adding: bool) -> Option<i32> {
    if parts.len() < 3 {
        let Some(raw) = world.target_of(gm) else {
            world.send_sys_message(gm, "You must target a clan member of the attacker!");
            return None;
        };
        if !world.is_player(raw) {
            world.send_sys_message(gm, "You must target a player with clan!");
            return None;
        }
        let Some(clan) = world.clan_of(raw).map(|c| c.id) else {
            world.send_sys_message(gm, "Your target does not have any clan!");
            return None;
        };
        let attacker = world.hall_siege_is_attacker(hall, clan);
        if adding && attacker {
            world.send_sys_message(gm, "Your target's clan is alredy participating!");
            return None;
        }
        if !adding && !attacker {
            world.send_sys_message(gm, "Your target's clan is not participating!");
            return None;
        }
        Some(clan)
    } else {
        let Some(clan) = world.clan_by_name(parts[2]).map(|c| c.id) else {
            world.send_sys_message(gm, "The given clan does not exist!");
            return None;
        };
        let attacker = world.hall_siege_is_attacker(hall, clan);
        if adding && attacker {
            world.send_sys_message(gm, "The given clan is alredy participating!");
            return None;
        }
        if !adding && !attacker {
            world.send_sys_message(gm, "The given clan is not participating!");
            return None;
        }
        Some(clan)
    }
}

/// `sendSiegableHallPage`.
fn send_siegable_hall_page(world: &mut World, gm: ObjectId, hall_id: &str, hall: ClanHallId) {
    let Some((name, owner)) = world.clan_hall(hall).map(|h| (h.name.clone(), h.owner_id)) else {
        return;
    };
    let mut html = world.admin_html_file(gm, "data/html/admin/siegablehall.htm");
    html.replace("%clanhallId%", hall_id);
    html.replace("%clanhallName%", &name);
    let owner_name = if owner > 0 {
        world
            .clan(owner)
            .map_or_else(|| "No Owner".to_owned(), |c| c.name.clone())
    } else {
        "No Owner".to_owned()
    };
    html.replace("%clanhallOwner%", &owner_name);
    world.send_admin_html(gm, &html);
}
