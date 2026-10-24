//! `//manor`: the manor status page, with the current mode, the next mode change and the manor
//! cost of every castle for this period and the next one.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminManor.java
// Java: util/FormatUtil.java (formatAdena)

use l2_util::ObjectId;

use crate::admin::{AdminHandler, AdminHandlers, JavaException};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminManor.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_manor", admin_manor)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminManor.onCommand`: `manor.htm` with the status, the change and one block of four
/// rows per castle.
fn admin_manor(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let mut html = world.admin_html_file(gm, "data/html/admin/manor.htm");
    html.replace("%status%", world.manor_current_mode_name());
    html.replace("%change%", &world.manor_next_mode_change_text());

    let mut rows = String::new();
    for castle in world.castles() {
        let id = castle.id.0;
        rows.push_str(&format!(
            "<tr><td>Name:</td><td><font color=008000>{}</font></td></tr>",
            castle.name
        ));
        rows.push_str(&format!(
            "<tr><td>Current period cost:</td><td><font color=FF9900>{} Adena</font></td></tr>",
            format_adena(world.manor_cost(id, false))
        ));
        rows.push_str(&format!(
            "<tr><td>Next period cost:</td><td><font color=FF9900>{} Adena</font></td></tr>",
            format_adena(world.manor_cost(id, true))
        ));
        rows.push_str(
            "<tr><td><font color=808080>--------------------------</font></td>\
<td><font color=808080>--------------------------</font></td></tr>",
        );
    }
    html.replace("%castleInfo%", &rows);
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `FormatUtil.formatAdena`: `NumberFormat.getIntegerInstance(Locale.ENGLISH)`, the digits in
/// groups of three separated by commas, a minus sign in front of a negative amount.
fn format_adena(amount: i64) -> String {
    let digits = amount.unsigned_abs().to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    if amount < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

#[cfg(test)]
mod tests {
    use super::format_adena;

    #[test]
    fn adena_is_grouped_by_thousands_like_java() {
        assert_eq!(format_adena(0), "0");
        assert_eq!(format_adena(999), "999");
        assert_eq!(format_adena(1_000), "1,000");
        assert_eq!(format_adena(1_234_567), "1,234,567");
        assert_eq!(format_adena(-1_234_567), "-1,234,567");
        assert_eq!(format_adena(i64::MAX), "9,223,372,036,854,775,807");
    }
}
