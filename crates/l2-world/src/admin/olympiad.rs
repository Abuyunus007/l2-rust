//! The Olympiad words of the admin page: `//checkolympiad`, `//saveolymp` and `//endolympiad`
//! (M8-ADMIN-07).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminAdmin.java
//
// `//sethero` and `//givehero` are `olympiad/hero.rs` (M8-HERO-01).

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;

use super::punishments::format_date;
use super::{AdminHandler, AdminHandlers, JavaException};
use crate::punishments::format_duration;
use crate::world::World;

type Outcome = Result<(), JavaException>;

pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_checkolympiad", admin_checkolympiad),
        ("admin_saveolymp", admin_saveolymp),
        ("admin_endolympiad", admin_endolympiad),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `.replaceAll(",? \\d+ seconds?", "")` of one unit: every ", 12 seconds" / " 1 second" goes.
fn strip_unit(text: &str, unit: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        // Try the match at `i`: optional ',' then ' ' digits ' ' unit optional 's'.
        let mut j = i;
        if bytes[j] == b',' {
            j += 1;
        }
        let matched = (|| {
            if bytes.get(j) != Some(&b' ') {
                return None;
            }
            let mut k = j + 1;
            let digits = k;
            while bytes.get(k).is_some_and(u8::is_ascii_digit) {
                k += 1;
            }
            if k == digits || bytes.get(k) != Some(&b' ') {
                return None;
            }
            k += 1;
            if !text.get(k..)?.starts_with(unit) {
                return None;
            }
            k += unit.len();
            if bytes.get(k) == Some(&b's') {
                k += 1;
            }
            Some(k)
        })();
        match matched {
            Some(end) => i = end,
            None => {
                let ch_len = text[i..].chars().next().map_or(1, char::len_utf8);
                out.push_str(&text[i..i + ch_len]);
                i += ch_len;
            }
        }
    }
    out
}

/// `TimeUtil.formatDuration(millis)` without the seconds and milliseconds.
fn minutes_only(millis: i64) -> String {
    strip_unit(&strip_unit(&format_duration(millis), "second"), "millisecond")
}

/// The page of `//checkolympiad` for the given clock and spans (Java builds it inline).
pub(crate) fn olympiad_page(now: i64, end_millis: i64, week_millis: i64) -> String {
    let mut sb = String::new();
    sb.push_str("<html><title>Olympiad Period</title><body>");
    sb.push_str("<center><table width=280>");
    sb.push_str("<tr><td width=60><button value=\"Main\" action=\"bypass admin_admin\" width=60 height=15 back=\"sek.cbui94\" fore=\"sek.cbui92\"></td>");
    sb.push_str("<td width=150><center>Olympiad Info</center></td>");
    sb.push_str("<td width=60><button value=\"Back\" action=\"bypass admin_admin2\" width=60 height=15 back=\"sek.cbui94\" fore=\"sek.cbui92\"></td></tr>");
    sb.push_str("</table></center><br>");

    // Current Server Time.
    sb.push_str("<center><table width=280 bgcolor=\"000000\">");
    sb.push_str("<tr><td width=120>Current Time:</td><td width=140><font color=\"LEVEL\">");
    sb.push_str(&format_date(now));
    sb.push_str("</font></td></tr></table></center><br><br>");

    // Olympiad End Time.
    sb.push_str("<center><table width=280 bgcolor=\"000000\">");
    sb.push_str("<tr><td width=120>Olympiad Period Ends:</td><td width=120><font color=\"LEVEL\">");
    sb.push_str(&format_date(now.saturating_add(end_millis)));
    sb.push_str("</font></td></tr>");
    sb.push_str("<tr><td width=120>Time Left:</td><td width=120><font color=\"LEVEL\">");
    sb.push_str(&minutes_only(end_millis));
    sb.push_str("</font></td></tr></table></center><br>");

    // Weekly Change.
    sb.push_str("<center><table width=280 bgcolor=\"000000\">");
    sb.push_str("<tr><td width=120>Next Weekly Change:</td><td width=120><font color=\"LEVEL\">");
    sb.push_str(&format_date(now.saturating_add(week_millis)));
    sb.push_str("</font></td></tr>");
    sb.push_str("<tr><td width=120>Time Left:</td><td width=120><font color=\"LEVEL\">");
    sb.push_str(&minutes_only(week_millis));
    sb.push_str("</font></td></tr></table></center><br>");

    sb.push_str("</body></html>");
    sb
}

/// Java `admin_checkolympiad`.
fn admin_checkolympiad(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    // `System.currentTimeMillis()`; the Olympiad's clock is the same unless a test overrides it.
    let now = world.olympiad_now_ms();
    let html = olympiad_page(
        now,
        world.olympiad_millis_to_olympiad_end(),
        world.olympiad_millis_to_week_change(),
    );
    if let Some(message) = NpcHtmlMessage::new(0, &html) {
        world.send_html(gm, &message);
    }
    Ok(())
}

/// Java `admin_saveolymp`.
fn admin_saveolymp(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.save_olympiad_status();
    world.send_sys_message(gm, "olympiad system saved.");
    Ok(())
}

/// Java `admin_endolympiad`.
fn admin_endolympiad(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.olympiad_manual_select_heroes();
    world.send_sys_message(gm, "Heroes formed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_and_milliseconds_are_stripped() {
        assert_eq!(
            minutes_only(86_400_000 + 2 * 3_600_000 + 3 * 60_000 + 4_000 + 5),
            "1 day, 2 hours, 3 minutes"
        );
        assert_eq!(minutes_only(61_000), "1 minute");
        // NOTE(java-quirk): the regex needs a space in front, so a text that starts with the
        // unit (a span under a minute) keeps it.
        assert_eq!(minutes_only(1_000), "1 second");
        assert_eq!(minutes_only(0), "0 milliseconds");
    }
}
