//! The html pages of the companion hire: plain functions of what the page shows.

use super::classes::CompanionClass;
use super::role::Role;
use crate::custom::class_buffer::{BUTTON, grouped};

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
    /// The classes of the companions he has, in the order they were hired.
    pub squad: &'a [&'static str],
    /// The most companions a player may have.
    pub max: usize,
    /// The level a player needs to hire.
    pub min_level: i32,
    /// A line at the top: what the last click did or why it did not.
    pub note: Option<&'a str>,
}

fn button(mode: Mode, label: &str, click: &str) -> String {
    format!(
        "<button value=\"{label}\" action=\"{}\" {BUTTON}><br>",
        mode.action(click)
    )
}

/// The page: the classes to hire by role, then the companions the player has.
pub fn main_page(page: &MainPage<'_>) -> String {
    let mut html = String::from("<html><body><center><font color=\"LEVEL\">Companions</font></center><br>");
    if let Some(note) = page.note {
        html.push_str(&format!("<font color=\"FF9900\">{note}</font><br>"));
    }
    html.push_str(&format!(
        "A companion fights at your side, grows with your level and comes back when you log in again. \
         It costs <font color=\"LEVEL\">{} Adena</font> ({} for every level of yours). You can have {}, \
         from level {}.<br>",
        grouped(page.fee),
        grouped(page.fee_per_level),
        page.max,
        page.min_level
    ));
    let full = page.squad.len() >= page.max;
    if page.choices.is_empty() {
        html.push_str(&format!(
            "<br>You can hire companions from level {}.<br>",
            page.min_level
        ));
    } else if full {
        html.push_str("<br>Dismiss one to hire another.<br>");
    } else {
        html.push_str("<br>Click a class to hire it:<br>");
        for role in Role::ALL {
            let classes: Vec<&&CompanionClass> = page.choices.iter().filter(|c| c.role == role).collect();
            if classes.is_empty() {
                continue;
            }
            let links: Vec<String> = classes
                .iter()
                .map(|c| {
                    format!(
                        "<a action=\"{}\">{}</a>",
                        page.mode.action(&format!("hire:{}", c.id)),
                        c.name
                    )
                })
                .collect();
            html.push_str(&format!(
                "<font color=\"LEVEL\">{}</font> <font color=\"808080\">{}</font><br1>{}<br>",
                role.title(),
                role.about(),
                links.join(" | ")
            ));
        }
    }
    if !page.squad.is_empty() {
        html.push_str(&format!(
            "<br><center><font color=\"LEVEL\">Your companions ({} of {})</font><br>",
            page.squad.len(),
            page.max
        ));
        for (index, class) in page.squad.iter().enumerate() {
            html.push_str(&button(
                page.mode,
                &format!("Dismiss the {class}"),
                &format!("dismiss:{index}"),
            ));
        }
        html.push_str(&button(page.mode, "Dismiss all", "dismissall"));
        html.push_str("</center>");
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
        squad: &'a [&'static str],
        choices: &'a [&'static CompanionClass],
        note: Option<&'a str>,
    ) -> MainPage<'a> {
        MainPage {
            mode,
            fee: i64::from(level) * 5_000,
            fee_per_level: 5_000,
            choices,
            squad,
            max: 3,
            min_level: 20,
            note,
        }
    }

    #[test]
    fn the_page_offers_the_classes_of_the_tier_by_role_with_the_price() {
        for level in [23, 50, 80] {
            let classes = choices(level);
            let html = main_page(&page(Mode::Npc, level, &[], &classes, None));
            for class in &classes {
                assert!(
                    html.contains(&format!(
                        "bypass -h Script Companions hire:{}\">{}</a>",
                        class.id, class.name
                    )),
                    "{level}: {}",
                    class.name
                );
            }
            assert!(html.contains("5,000 for every level"));
            assert!(!html.contains("dismiss"), "nobody to dismiss yet");
            // The client shows pages up to 8 KB.
            assert!(html.len() < 8_000, "{level}: {}", html.len());
        }
        let html = main_page(&page(Mode::Npc, 23, &[], &choices(23), None));
        assert!(html.contains(">Monk</a>") && html.contains(">Artisan</a>") && !html.contains(">Tyrant</a>"));
        assert!(html.find("Tank").unwrap() < html.find(">Human Knight</a>").unwrap());
    }

    #[test]
    fn the_command_page_links_to_the_command() {
        let classes = choices(45);
        let html = main_page(&page(Mode::Command, 45, &["Tyrant"], &classes, None));
        assert!(
            html.contains("bypass -h voice .companion hire 48\">Tyrant</a>"),
            "{html}"
        );
        assert!(html.contains("bypass -h voice .companion dismiss 0"));
        assert!(html.contains("bypass -h voice .companion dismissall"));
        assert!(!html.contains("Script Companions"));
    }

    #[test]
    fn the_hired_ones_can_be_dismissed_one_by_one_or_all_at_once() {
        let classes = choices(45);
        let html = main_page(&page(Mode::Npc, 45, &["Paladin", "Bishop"], &classes, None));
        assert!(html.contains("dismiss:0") && html.contains("dismiss:1") && !html.contains("dismiss:2"));
        assert!(html.contains("Dismiss the Paladin") && html.contains("Dismiss the Bishop"));
        assert!(html.contains("dismissall") && html.contains("(2 of 3)"));
        assert!(html.contains("hire:"), "there is room for one more");
    }

    #[test]
    fn a_full_squad_or_a_low_level_cannot_hire() {
        let classes = choices(45);
        let full = main_page(&page(
            Mode::Npc,
            45,
            &["Paladin", "Bishop", "Tyrant"],
            &classes,
            None,
        ));
        assert!(!full.contains("hire:") && full.contains("dismiss:2"));
        let low = main_page(&page(Mode::Npc, 12, &[], &[], None));
        assert!(!low.contains("hire:") && low.contains("from level 20"));
    }

    #[test]
    fn a_note_is_shown_at_the_top() {
        let html = main_page(&page(Mode::Npc, 40, &[], &[], Some("You need 200,000 Adena.")));
        assert!(html.find("You need").unwrap() < html.find("A companion fights").unwrap());
    }
}
