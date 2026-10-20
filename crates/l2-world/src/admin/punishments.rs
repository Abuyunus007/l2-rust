//! The punishment commands: `//punishment`, `//punishment_add`, `//punishment_remove` and the
//! shortcuts `//ban_acc`, `//ban_char`, `//ban_chat`, `//jail`... with their `//unban_*` pairs.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminPunishment.java
//
// # Left out, with the system that brings it
// - `CharInfoTable` is the startup name table plus the characters who entered since
//   (`World::char_id_by_name`): a character created after the server started and never online
//   is "not found" until the next restart.
// - The check of `ServerConfig.GAME_SERVER_HOSTS` when banning an IP ("You cannot ban your
//   gameserver's address!"): the world does not know the host list.
// - `InetAddress.getByName` resolves host names; here only an IP address is accepted.
// - The `HWID` buttons work, but no player has a hardware id, so nothing is ever affected.

use std::net::IpAddr;
use std::str::FromStr;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::punishments::{NewPunishment, PunishmentAffect, PunishmentType};
use crate::world::{World, unix_ms_now};

type Outcome = Result<(), JavaException>;

const PUNISHMENT: &str = "data/html/admin/punishments/punishment.htm";
const PUNISHMENT_INFO: &str = "data/html/admin/punishments/punishment-info.htm";
const PUNISHMENT_PLAYER: &str = "data/html/admin/punishments/punishment-player.htm";

/// Java `AdminPunishment.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    const COMMANDS: [&str; 13] = [
        "admin_punishment",
        "admin_punishment_add",
        "admin_punishment_remove",
        "admin_ban_acc",
        "admin_unban_acc",
        "admin_ban_hwid",
        "admin_unban_hwid",
        "admin_ban_chat",
        "admin_unban_chat",
        "admin_ban_char",
        "admin_unban_char",
        "admin_jail",
        "admin_unjail",
    ];
    let handler: AdminHandler = admin_punishment;
    for command in COMMANDS {
        handlers.register(command, handler);
    }
}

/// `StringUtil.implode(values, ";")` of the names.
fn implode<const N: usize>(names: [&'static str; N]) -> String {
    names.join(";")
}

fn type_names() -> String {
    implode(PunishmentType::ALL.map(PunishmentType::name))
}

fn affect_names() -> String {
    implode(PunishmentAffect::ALL.map(PunishmentAffect::name))
}

/// `new SystemMessage(S1_DOES_NOT_EXIST)` with the name.
fn does_not_exist(world: &World, gm: ObjectId, name: &str) {
    world.send(
        gm,
        &SystemMessage::new(SystemMessageId::S1DoesNotExist).with(SystemMessageParam::Text(name.to_owned())),
    );
}

/// `findCharId(key)`: the id of a character name, else the key.
fn find_char_id(world: &World, key: &str) -> String {
    match world.char_id_by_name(key) {
        Some(id) if id > 0 => id.to_string(),
        _ => key.to_owned(),
    }
}

/// `DATE_FORMATTER.format(Instant.ofEpochMilli(expiration).atZone(ZoneId.systemDefault()))` with
/// the pattern `dd/MM/yyyy HH:mm:ss`.
fn format_date(epoch_ms: i64) -> String {
    let local = epoch_ms.saturating_add(l2_util::time::local_offset_millis(epoch_ms));
    let seconds = local.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{day:02}/{month:02}/{year:04} {:02}:{:02}:{:02}",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// Days since 1970-01-01 to (year, month, day) in the proleptic Gregorian calendar.
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

/// Java `AdminPunishment.onCommand`.
fn admin_punishment(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    let Some(cmd) = st.next_token() else {
        return Ok(());
    };
    match cmd {
        "admin_punishment" => {
            if !st.has_more() {
                show_page(world, gm, PUNISHMENT, |page| {
                    page.replace("%punishments%", &type_names())
                        .replace("%affects%", &affect_names())
                });
            } else {
                match st.next_token().unwrap_or_default() {
                    "info" => return info_page(world, gm, &mut st),
                    "player" => return player_page(world, gm, &mut st),
                    _ => {}
                }
            }
        }
        "admin_punishment_add" => return punishment_add(world, gm, cmd, &mut st),
        "admin_punishment_remove" => return punishment_remove(world, gm, cmd, &mut st),
        "admin_ban_char" => {
            return shortcut_add(
                world,
                gm,
                &mut st,
                PunishmentAffect::Character,
                PunishmentType::Ban,
                "Banned by admin",
            );
        }
        "admin_unban_char" => {
            return shortcut_remove(
                world,
                gm,
                &mut st,
                PunishmentAffect::Character,
                PunishmentType::Ban,
            );
        }
        "admin_ban_acc" => {
            return shortcut_add(
                world,
                gm,
                &mut st,
                PunishmentAffect::Account,
                PunishmentType::Ban,
                "Banned by admin",
            );
        }
        "admin_unban_acc" => {
            return shortcut_remove(world, gm, &mut st, PunishmentAffect::Account, PunishmentType::Ban);
        }
        "admin_ban_hwid" => {
            return shortcut_add(
                world,
                gm,
                &mut st,
                PunishmentAffect::Hwid,
                PunishmentType::Ban,
                "Banned by admin",
            );
        }
        "admin_unban_hwid" => {
            return shortcut_remove(world, gm, &mut st, PunishmentAffect::Hwid, PunishmentType::Ban);
        }
        "admin_ban_chat" => {
            return shortcut_add(
                world,
                gm,
                &mut st,
                PunishmentAffect::Character,
                PunishmentType::ChatBan,
                "Chat banned by admin",
            );
        }
        "admin_unban_chat" => {
            return shortcut_remove(
                world,
                gm,
                &mut st,
                PunishmentAffect::Character,
                PunishmentType::ChatBan,
            );
        }
        "admin_jail" => {
            return shortcut_add(
                world,
                gm,
                &mut st,
                PunishmentAffect::Character,
                PunishmentType::Jail,
                "Jailed by admin",
            );
        }
        "admin_unjail" => {
            return shortcut_remove(
                world,
                gm,
                &mut st,
                PunishmentAffect::Character,
                PunishmentType::Jail,
            );
        }
        _ => {}
    }
    Ok(())
}

/// `String.format("admin_punishment_add %s %s %s %s %s", name, affect, type, 0, reason)`.
fn shortcut_add(
    world: &mut World,
    gm: ObjectId,
    st: &mut Tokens<'_>,
    affect: PunishmentAffect,
    kind: PunishmentType,
    reason: &str,
) -> Outcome {
    if st.has_more() {
        let name = st.next_token().unwrap_or_default();
        return admin_punishment(
            world,
            gm,
            &format!(
                "admin_punishment_add {name} {} {} 0 {reason}",
                affect.name(),
                kind.name()
            ),
        );
    }
    Ok(())
}

/// `String.format("admin_punishment_remove %s %s %s", name, affect, type)`.
fn shortcut_remove(
    world: &mut World,
    gm: ObjectId,
    st: &mut Tokens<'_>,
    affect: PunishmentAffect,
    kind: PunishmentType,
) -> Outcome {
    if st.has_more() {
        let name = st.next_token().unwrap_or_default();
        return admin_punishment(
            world,
            gm,
            &format!("admin_punishment_remove {name} {} {}", affect.name(), kind.name()),
        );
    }
    Ok(())
}

/// A page of the panel, or Java's warning when the file is missing. `NpcHtmlMessage(0, 1, html)`.
fn show_page(world: &mut World, gm: ObjectId, path: &str, fill: impl FnOnce(String) -> String) {
    match world.get_htm(gm, path) {
        Some(content) => {
            if let Some(message) = NpcHtmlMessage::new_item(0, 1, &fill(content)) {
                world.send_html(gm, &message);
            }
        }
        None => warn!("AdminPunishment: {path} is missing."),
    }
}

/// The `info` sub command.
fn info_page(world: &mut World, gm: ObjectId, st: &mut Tokens<'_>) -> Outcome {
    let key = st.next_token();
    let af = st.next_token();
    let (Some(key), Some(af)) = (key, af) else {
        world.send_sys_message(gm, "Not enough data specified!");
        return Ok(());
    };
    let name = key;
    let Some(affect) = PunishmentAffect::by_name(af) else {
        world.send_sys_message(gm, "Incorrect value specified for affect type!");
        return Ok(());
    };
    let mut key = key.to_owned();
    if affect == PunishmentAffect::Character {
        // Check if the character name exists.
        if world.char_id_by_name(name).is_none() {
            does_not_exist(world, gm, name);
            return Ok(());
        }
        // Swap the name of the character with it's id.
        key = find_char_id(world, &key);
    }

    let mut rows = String::new();
    for kind in PunishmentType::ALL {
        if world.has_punishment(&key, affect, kind) {
            let expiration = world.punishment_expiration(&key, affect, kind);
            let expire = if expiration > 0 {
                format_date(expiration)
            } else {
                "never".to_owned()
            };
            rows.push_str(&format!(
                "<tr><td><font color=\"LEVEL\">{}</font></td><td>{expire}</td><td><a action=\"bypass -h admin_punishment_remove {name} {} {}\">Remove</a></td></tr>",
                kind.name(),
                affect.name(),
                kind.name()
            ));
        }
    }
    show_page(world, gm, PUNISHMENT_INFO, |page| {
        page.replace("%player_name%", name)
            .replace("%punishments%", &rows)
            .replace("%affects%", &affect_names())
            .replace("%affect_type%", affect.name())
    });
    Ok(())
}

/// The `player` sub command.
fn player_page(world: &mut World, gm: ObjectId, st: &mut Tokens<'_>) -> Outcome {
    let mut target = None;
    if st.has_more() {
        let player_name = st.next_token().unwrap_or_default();
        if player_name.is_empty() && !world.target_of(gm).is_some_and(|t| world.is_player(t)) {
            return admin_punishment(world, gm, "admin_punishment");
        }
        target = world.player_by_name(player_name);
    }
    if target.is_none() && !world.target_of(gm).is_some_and(|t| world.is_player(t)) {
        world.send_sys_message(gm, "You must target player!");
        return Ok(());
    }
    let Some(target) = target.or_else(|| world.target_of(gm)) else {
        return Ok(());
    };
    let Some(player) = world.players.get(&target) else {
        return Ok(());
    };
    let name = player.data.name.clone();
    let account = player.data.account.clone();
    let ip = player.client_ip.map_or_else(String::new, |ip| ip.to_string());
    show_page(world, gm, PUNISHMENT_PLAYER, |page| {
        page.replace("%player_name%", &name)
            .replace("%punishments%", &type_names())
            .replace("%acc%", &account)
            .replace("%char%", &name)
            .replace("%ip%", &ip)
            // No hardware info.
            .replace("%hwid%", "Unknown")
    });
    Ok(())
}

/// The `admin_punishment_add` branch.
fn punishment_add(world: &mut World, gm: ObjectId, cmd: &str, st: &mut Tokens<'_>) -> Outcome {
    // Add new punishment.
    let key = st.next_token();
    let af = st.next_token();
    let t = st.next_token();
    let exp = st.next_token();
    let mut reason = st.next_token().map(str::to_owned);

    // Let's grab the other part of the reason if there is..
    if let Some(reason) = reason.as_mut() {
        while let Some(token) = st.next_token() {
            reason.push(' ');
            reason.push_str(token);
        }
        if !reason.is_empty() {
            *reason = reason
                .replace('$', "\\$")
                .replace("\r\n", "<br1>")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
        }
    }

    let name = key;
    let (Some(key), Some(af), Some(t), Some(exp), Some(reason)) = (key, af, t, exp, reason) else {
        world.send_sys_message(gm, "Please fill all the fields!");
        return Ok(());
    };
    let name = name.unwrap_or(key);

    if !is_numeric(exp) && exp != "-1" {
        world.send_sys_message(gm, "Incorrect value specified for expiration time!");
        return Ok(());
    }

    let mut expiration_time = i64::from(parse_int(exp)?);
    if expiration_time > 0 {
        expiration_time = unix_ms_now().saturating_add(expiration_time * 60 * 1000);
    }

    let (Some(affect), Some(kind)) = (PunishmentAffect::by_name(af), PunishmentType::by_name(t)) else {
        world.send_sys_message(gm, "Incorrect value specified for affect/punishment type!");
        return Ok(());
    };

    let mut key = key.to_owned();
    if affect == PunishmentAffect::Character {
        // Check if the character name exists.
        if world.char_id_by_name(name).is_none() {
            does_not_exist(world, gm, name);
            return Ok(());
        }
        // Swap the name of the character with it's id.
        key = find_char_id(world, &key);
    } else if affect == PunishmentAffect::Ip {
        match IpAddr::from_str(&key) {
            Ok(addr) if addr.is_loopback() => {
                world.send_sys_message(gm, "You've entered an incorrect IP address!");
                world.send_message(gm, "You cannot ban any local address!");
                return Ok(());
            }
            Ok(_) => {}
            Err(_) => {
                world.send_sys_message(gm, "You've entered an incorrect IP address!");
                world.send_message(gm, &format!("{key}: Name or service not known"));
                return Ok(());
            }
        }
    }

    // Check if we already put the same punishment on that guy ^^
    if world.has_punishment(&key, affect, kind) {
        world.send_sys_message(gm, "Target is already affected by that punishment.");
        return Ok(());
    }

    // Punish him!
    let gm_name = world.object_name(gm).unwrap_or_default();
    world.start_punishment(NewPunishment::new(
        &key,
        affect,
        kind,
        expiration_time,
        &reason,
        &gm_name,
    ));
    world.send_sys_message(
        gm,
        &format!(
            "Punishment {} have been applied to: {} {name}!",
            kind.name(),
            affect.name()
        ),
    );
    audit(world, gm, cmd, affect, name);
    admin_punishment(
        world,
        gm,
        &format!("admin_punishment info {name} {}", affect.name()),
    )
}

/// The `admin_punishment_remove` branch.
fn punishment_remove(world: &mut World, gm: ObjectId, cmd: &str, st: &mut Tokens<'_>) -> Outcome {
    // Remove punishment.
    let key = st.next_token();
    let af = st.next_token();
    let t = st.next_token();
    let (Some(key), Some(af), Some(t)) = (key, af, t) else {
        world.send_sys_message(gm, "Not enough data specified!");
        return Ok(());
    };
    let name = key;
    let (Some(affect), Some(kind)) = (PunishmentAffect::by_name(af), PunishmentType::by_name(t)) else {
        world.send_sys_message(gm, "Incorrect value specified for affect/punishment type!");
        return Ok(());
    };

    let mut key = key.to_owned();
    if affect == PunishmentAffect::Character {
        // Check if the character name exists.
        if world.char_id_by_name(name).is_none() {
            does_not_exist(world, gm, name);
            return Ok(());
        }
        // Swap the name of the character with it's id.
        key = find_char_id(world, &key);
    }

    if !world.has_punishment(&key, affect, kind) {
        world.send_sys_message(gm, "Target is not affected by that punishment!");
        return Ok(());
    }

    world.stop_punishment(&key, affect, kind);
    world.send_sys_message(
        gm,
        &format!(
            "Punishment {} have been stopped to: {} {name}!",
            kind.name(),
            affect.name()
        ),
    );
    audit(world, gm, cmd, affect, name);
    admin_punishment(
        world,
        gm,
        &format!("admin_punishment info {name} {}", affect.name()),
    )
}

/// `GMAudit.logAction(activeChar.getName() + " [" + activeChar.getObjectId() + "]", cmd, affect.name(), name)`
/// (the file target comes with the logging card, D-25).
fn audit(world: &World, gm: ObjectId, cmd: &str, affect: PunishmentAffect, name: &str) {
    let gm_name = world.object_name(gm).unwrap_or_default();
    info!(target: "gmaudit", "{gm_name} [{}]>{cmd}>{}>{name}", gm.0, affect.name());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_dd_mm_yyyy() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn names_are_joined_with_semicolons() {
        assert_eq!(type_names(), "BAN;CHAT_BAN;PARTY_BAN;JAIL");
        assert_eq!(affect_names(), "ACCOUNT;CHARACTER;IP;HWID");
    }
}
