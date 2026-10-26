//! `//config_server` and `//setconfig`: the rates and the element enchant chances that the GM
//! page changes while the server runs.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminAdmin.java (showConfigPage and
// the `admin_setconfig` branch)
//
// The values live in the world's settings (`WorldSettings`), which only these keys change at run
// time. They are not written back to the `.ini` files, like in Java.

use l2_util::ObjectId;

use super::edit_char::java_parse_double;
use super::html::AdminHtml;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens};
use crate::world::World;

type Outcome = Result<(), JavaException>;

pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_config_server", admin_config_server),
        ("admin_setconfig", admin_setconfig),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

fn admin_config_server(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // Java `command.equals("admin_config_server")`: with arguments no branch matches.
    if command == "admin_config_server" {
        show_config_page(world, gm);
    }
    Ok(())
}

fn admin_setconfig(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let result = (|| {
        let name = st.next_token()?;
        let value = st.next_token()?;
        // `Float.parseFloat(pValue)`; the check before it is `Float.valueOf(pValue) == null`, which
        // never is null: a bad number throws and ends in the usage message.
        let parsed = java_parse_double(value)? as f32;
        Some((name, value, parsed))
    })();
    match result {
        Some((name, value, parsed)) => {
            let s = &mut world.settings;
            match name {
                "RateXp" => s.exp.rate_xp = parsed,
                "RateSp" => s.exp.rate_sp = parsed,
                "RateDropSpoil" => s.drops.rate_spoil_drop_chance_multiplier = parsed,
                "EnchantChanceElementStone" => s.enchant.enchant_chance_element_stone = f64::from(parsed),
                "EnchantChanceElementCrystal" => s.enchant.enchant_chance_element_crystal = f64::from(parsed),
                "EnchantChanceElementJewel" => s.enchant.enchant_chance_element_jewel = f64::from(parsed),
                "EnchantChanceElementEnergy" => s.enchant.enchant_chance_element_energy = f64::from(parsed),
                // A name the switch does not know still gets the confirmation.
                _ => {}
            }
            world.send_sys_message(gm, &format!("Config parameter {name} set to {value}"));
        }
        None => world.send_sys_message(gm, "Usage: //setconfig <parameter> <value>"),
    }
    show_config_page(world, gm);
    Ok(())
}

/// Java `AdminAdmin.showConfigPage`.
fn show_config_page(world: &mut World, gm: ObjectId) {
    let s = &world.settings;
    let row = |label: &str, value: String, var: &str, key: &str| {
        format!(
            "<tr><td><font color=\"LEVEL\">{label}</font> = {value}</td><td><edit var=\"{var}\" width=40 height=15></td><td><button value=\"Set\" action=\"bypass -h admin_setconfig {key} ${var}\" width=40 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr>"
        )
    };
    let mut html = String::from("<html><title>L2J :: Config</title><body>");
    html.push_str("<center><table width=270><tr><td width=60><button value=\"Main\" action=\"bypass admin_admin\" width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td width=150>Config Server Panel</td><td width=60><button value=\"Back\" action=\"bypass admin_admin4\" width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table></center><br>");
    html.push_str(
        "<center><table width=260><tr><td width=140></td><td width=40></td><td width=40></td></tr>",
    );
    html.push_str("<tr><td><font color=\"00AA00\">Drop:</font></td><td></td><td></td></tr>");
    html.push_str(&row(
        "Rate EXP",
        format!("{:?}", s.exp.rate_xp),
        "param1",
        "RateXp",
    ));
    html.push_str(&row(
        "Rate SP",
        format!("{:?}", s.exp.rate_sp),
        "param2",
        "RateSp",
    ));
    html.push_str(&row(
        "Rate Drop Spoil",
        format!("{:?}", s.drops.rate_spoil_drop_chance_multiplier),
        "param4",
        "RateDropSpoil",
    ));
    html.push_str("<tr><td width=140></td><td width=40></td><td width=40></td></tr>");
    html.push_str("<tr><td><font color=\"00AA00\">Enchant:</font></td><td></td><td></td></tr>");
    html.push_str(&row(
        "Enchant Element Stone",
        format!("{:?}", s.enchant.enchant_chance_element_stone),
        "param8",
        "EnchantChanceElementStone",
    ));
    html.push_str(&row(
        "Enchant Element Crystal",
        format!("{:?}", s.enchant.enchant_chance_element_crystal),
        "param9",
        "EnchantChanceElementCrystal",
    ));
    html.push_str(&row(
        "Enchant Element Jewel",
        format!("{:?}", s.enchant.enchant_chance_element_jewel),
        "param10",
        "EnchantChanceElementJewel",
    ));
    html.push_str(&row(
        "Enchant Element Energy",
        format!("{:?}", s.enchant.enchant_chance_element_energy),
        "param11",
        "EnchantChanceElementEnergy",
    ));
    html.push_str("</table></body></html>");
    world.send_admin_html(gm, &AdminHtml::from_html(&html));
}
