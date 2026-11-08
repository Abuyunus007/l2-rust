//! The html pages of the companion hire: plain functions of what the page shows.

use l2_world::CompanionCombatMode;

use super::classes::CompanionClass;
use super::role::Role;
use crate::custom::class_buffer::grouped;

/// Where the page is shown: at a gatekeeper (the links name the script) or after `.companion`
/// anywhere (the links are the command again: a script link needs an NPC next to the player).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Npc,
    Command,
}

impl Mode {
    /// The bypass of a click: `hire:47` at a gatekeeper, `.companion hire 47` anywhere.
    pub fn action(self, click: &str) -> String {
        match self {
            Mode::Npc => format!("bypass -h Script Companions {click}"),
            Mode::Command => format!("bypass -h voice .companion {}", click.replace(':', " ")),
        }
    }
}

/// What the main page shows.
pub struct MainPage<'a> {
    pub mode: Mode,
    /// What one companion costs the player.
    pub fee: i64,
    /// What it costs for each of his levels.
    pub fee_per_level: i64,
    /// The classes he can hire (those of his class tier).
    pub choices: &'a [&'static CompanionClass],
    /// The companions he has, in the order they were hired: class name and role.
    pub squad: &'a [(&'static str, Role)],
    /// How his companions fight (`None`: he has none).
    pub combat: Option<CompanionCombatMode>,
    /// The most companions a player may have.
    pub max: usize,
    /// The level a player needs to hire.
    pub min_level: i32,
    /// A line at the top: what the last click did or why it did not.
    pub note: Option<&'a str>,
}

/// The width of the page's tables (the NPC window is about 280 wide).
const WIDTH: i32 = 280;
/// The class buttons per row (two: the long names, "Female Soul Breaker", fit).
const PER_ROW: usize = 2;

/// A button of `width` (the client's standard button texture).
fn button(mode: Mode, label: &str, click: &str, width: i32) -> String {
    format!(
        "<button value=\"{label}\" action=\"{}\" width={width} height=23 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\">",
        mode.action(click)
    )
}

/// A thin grey line across the page.
fn line() -> String {
    format!("<img src=\"L2UI.SquareGray\" width={WIDTH} height=1><br1>")
}

/// A section title: a gold line of text between two grey lines.
fn title(text: &str) -> String {
    format!(
        "{}<table width={WIDTH} bgcolor=\"000000\"><tr><td align=center><font color=\"LEVEL\">{text}</font></td></tr></table>{}",
        line(),
        line()
    )
}

/// The page: the costs and the combat mode, the companions the player has, then the classes to
/// hire by role (icon, role, what it does, one button per class).
pub fn main_page(page: &MainPage<'_>) -> String {
    let mut html = String::from(
        "<html><body><center><img src=\"L2UI_CH3.herotower_deco\" width=256 height=32><br1>\
         <font color=\"LEVEL\">Companions</font></center><br1>",
    );
    if let Some(note) = page.note {
        html.push_str(&format!(
            "<table width={WIDTH} bgcolor=\"332211\"><tr><td align=center><font color=\"FF9900\">{note}</font></td></tr></table><br1>"
        ));
    }
    // The facts: price, places, combat mode.
    let mode_text = match page.combat {
        Some(CompanionCombatMode::Aggressive) => "<font color=\"FF6655\">Aggressive</font>",
        Some(CompanionCombatMode::Passive) => "<font color=\"66CC66\">Passive</font>",
        None => "<font color=\"808080\">-</font>",
    };
    html.push_str(&format!(
        "<table width={WIDTH} bgcolor=\"111111\">\
         <tr><td width=100><font color=\"808080\">Price</font></td><td><font color=\"LEVEL\">{} Adena</font> <font color=\"808080\">({} per level)</font></td></tr>\
         <tr><td><font color=\"808080\">Companions</font></td><td>{} of {}</td></tr>\
         <tr><td><font color=\"808080\">Combat mode</font></td><td>{mode_text}</td></tr></table>",
        grouped(page.fee),
        grouped(page.fee_per_level),
        page.squad.len(),
        page.max
    ));
    html.push_str(
        "<font color=\"808080\">A companion fights at your side, grows with your level and comes back when you log in again.</font><br>",
    );

    // The companions he has.
    if !page.squad.is_empty() {
        html.push_str(&title(&format!(
            "Your companions ({} of {})",
            page.squad.len(),
            page.max
        )));
        html.push_str(&format!("<table width={WIDTH}>"));
        for (index, (class, role)) in page.squad.iter().enumerate() {
            html.push_str(&format!(
                "<tr><td width=36><img src=\"{}\" width=32 height=32></td><td width=150>{class}<br1><font color=\"808080\">{}</font></td><td width=90>{}</td></tr>",
                role.icon(),
                role.title(),
                button(page.mode, "Dismiss", &format!("dismiss:{index}"), 80)
            ));
        }
        html.push_str("</table><br1>");
        html.push_str(&format!(
            "<table width={WIDTH}><tr><td>{}</td><td>{}</td><td>{}</td></tr></table>",
            button(page.mode, "Aggressive", "setmode:aggressive", 90),
            button(page.mode, "Passive", "setmode:passive", 90),
            button(page.mode, "Dismiss all", "dismissall", 90),
        ));
    }

    // The classes to hire.
    let full = page.squad.len() >= page.max;
    if page.choices.is_empty() {
        html.push_str(&title("Hire a companion"));
        html.push_str(&format!(
            "<center>You can hire companions from level {}.</center>",
            page.min_level
        ));
    } else if full {
        html.push_str(&title("Hire a companion"));
        html.push_str("<center>All places are taken: dismiss one to hire another.</center>");
    } else {
        html.push_str(&title("Hire a companion"));
        for role in Role::ALL {
            let classes: Vec<&&CompanionClass> = page.choices.iter().filter(|c| c.role == role).collect();
            if classes.is_empty() {
                continue;
            }
            html.push_str(&format!(
                "<table width={WIDTH} bgcolor=\"000000\"><tr><td width=36><img src=\"{}\" width=32 height=32></td><td><font color=\"LEVEL\">{}</font><br1><font color=\"808080\">{}</font></td></tr></table>",
                role.icon(),
                role.title(),
                role.about()
            ));
            html.push_str(&format!("<table width={WIDTH}>"));
            for row in classes.chunks(PER_ROW) {
                html.push_str("<tr>");
                for class in row {
                    html.push_str(&format!(
                        "<td>{}</td>",
                        button(page.mode, class.name, &format!("hire:{}", class.id), 135)
                    ));
                }
                html.push_str("</tr>");
            }
            html.push_str("</table><br1>");
        }
    }
    html.push_str("</body></html>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom::companions::classes::choices;

    fn page<'a>(
        mode: Mode,
        level: i32,
        squad: &'a [(&'static str, Role)],
        choices: &'a [&'static CompanionClass],
        note: Option<&'a str>,
    ) -> MainPage<'a> {
        MainPage {
            mode,
            fee: i64::from(level) * 5_000,
            fee_per_level: 5_000,
            choices,
            squad,
            combat: (!squad.is_empty()).then_some(CompanionCombatMode::Passive),
            max: 3,
            min_level: 20,
            note,
        }
    }

    /// The hire button of a class.
    fn hire_button(mode: Mode, class: &CompanionClass) -> String {
        format!(
            "<button value=\"{}\" action=\"{}\"",
            class.name,
            mode.action(&format!("hire:{}", class.id))
        )
    }

    #[test]
    fn the_page_offers_the_classes_of_the_tier_by_role_with_the_price() {
        for level in [23, 50, 80] {
            let classes = choices(level);
            let html = main_page(&page(Mode::Npc, level, &[], &classes, None));
            for class in &classes {
                assert!(
                    html.contains(&hire_button(Mode::Npc, class)),
                    "{level}: {}",
                    class.name
                );
            }
            assert!(html.contains("5,000 per level"));
            assert!(!html.contains("dismiss"), "nobody to dismiss yet");
            // Every role that has a class shows its icon.
            for role in Role::ALL {
                if classes.iter().any(|c| c.role == role) {
                    assert!(html.contains(role.icon()), "{level}: {}", role.title());
                }
            }
            // A page longer than 17,200 characters is cut (`MAX_HTML_UNITS` of the packet, Java's
            // "this will crash the client"); the command's longer links stay well below too.
            assert!(html.len() < 12_000, "{level}: {}", html.len());
            let html = main_page(&page(
                Mode::Command,
                level,
                &[("Paladin", Role::Tank)],
                &classes,
                None,
            ));
            assert!(html.len() < 12_000, "{level} command: {}", html.len());
        }
        let html = main_page(&page(Mode::Npc, 23, &[], &choices(23), None));
        assert!(
            html.contains("value=\"Monk\"")
                && html.contains("value=\"Artisan\"")
                && !html.contains("value=\"Tyrant\"")
        );
        assert!(html.find(">Tank<").unwrap() < html.find("value=\"Human Knight\"").unwrap());
    }

    #[test]
    fn the_command_page_links_to_the_command() {
        let classes = choices(45);
        let html = main_page(&page(
            Mode::Command,
            45,
            &[("Tyrant", Role::Warrior)],
            &classes,
            None,
        ));
        assert!(
            html.contains("<button value=\"Tyrant\" action=\"bypass -h voice .companion hire 48\""),
            "{html}"
        );
        assert!(html.contains("bypass -h voice .companion dismiss 0"));
        assert!(html.contains("bypass -h voice .companion dismissall"));
        assert!(html.contains("bypass -h voice .companion setmode aggressive"));
        assert!(!html.contains("Script Companions"));
    }

    #[test]
    fn the_hired_ones_show_with_their_role_and_can_be_dismissed() {
        let classes = choices(45);
        let squad = [("Paladin", Role::Tank), ("Bishop", Role::Healer)];
        let html = main_page(&page(Mode::Npc, 45, &squad, &classes, None));
        assert!(html.contains("dismiss:0") && html.contains("dismiss:1") && !html.contains("dismiss:2"));
        assert!(html.contains("Paladin<br1>") && html.contains("Bishop<br1>"));
        assert!(html.contains(Role::Tank.icon()) && html.contains(Role::Healer.icon()));
        assert!(html.contains("dismissall") && html.contains("(2 of 3)"));
        assert!(html.contains("setmode:aggressive") && html.contains("setmode:passive"));
        assert!(html.contains(">Passive</font>"), "the mode is shown");
        assert!(html.contains("hire:"), "there is room for one more");
    }

    #[test]
    fn a_full_squad_or_a_low_level_cannot_hire() {
        let classes = choices(45);
        let squad = [
            ("Paladin", Role::Tank),
            ("Bishop", Role::Healer),
            ("Tyrant", Role::Warrior),
        ];
        let full = main_page(&page(Mode::Npc, 45, &squad, &classes, None));
        assert!(
            !full.contains("hire:") && full.contains("dismiss:2") && full.contains("All places are taken")
        );
        let low = main_page(&page(Mode::Npc, 12, &[], &[], None));
        assert!(!low.contains("hire:") && low.contains("from level 20"));
        assert!(!low.contains("setmode"), "no companions, no mode buttons");
    }

    #[test]
    fn a_note_is_shown_at_the_top() {
        let html = main_page(&page(Mode::Npc, 40, &[], &[], Some("You need 200,000 Adena.")));
        assert!(html.find("You need").unwrap() < html.find("Price").unwrap());
    }
}
