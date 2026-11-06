//! The html pages of the companion hire: plain functions of what the page shows.

use super::role::Role;
use crate::custom::class_buffer::{BUTTON, grouped};

/// What the main page shows.
pub struct MainPage<'a> {
    /// What one companion costs the player.
    pub fee: i64,
    /// The roles the player has hired, in the order they were.
    pub squad: &'a [Role],
    /// The most companions a player may have.
    pub max: usize,
    /// The level a player needs to hire.
    pub min_level: i32,
    /// A line at the top: what the last click did or why it did not.
    pub note: Option<&'a str>,
}

fn button(label: &str, event: &str) -> String {
    format!("<button value=\"{label}\" action=\"bypass -h Script Companions {event}\" {BUTTON}><br>")
}

/// The page: the roles to hire, then the companions the player has and how to send them away.
pub fn main_page(page: &MainPage<'_>) -> String {
    let mut html = String::from("<html><body><center><font color=\"LEVEL\">Companions</font></center><br>");
    if let Some(note) = page.note {
        html.push_str(&format!("<font color=\"FF9900\">{note}</font><br>"));
    }
    html.push_str(&format!(
        "A companion fights at your side, has your level and goes when you log out, die, or leave \
         the party you hired it in. It costs <font color=\"LEVEL\">{} Adena</font> ({} for every level \
         of yours). You can have {} of them, from level {}.<br><br>",
        grouped(page.fee),
        grouped(super::FEE_PER_LEVEL),
        page.max,
        page.min_level
    ));
    html.push_str("<center>");
    let full = page.squad.len() >= page.max;
    for role in Role::ALL {
        html.push_str(&format!(
            "<font color=\"LEVEL\">{}</font><br1>{}<br1>",
            role.title(),
            role.about()
        ));
        if full {
            html.push_str("<br>");
        } else {
            html.push_str(&button(
                &format!("Hire {} ({} Adena)", role.title(), grouped(page.fee)),
                &format!("hire:{}", role.key()),
            ));
        }
    }
    if !page.squad.is_empty() {
        html.push_str(&format!(
            "<br><font color=\"LEVEL\">Your companions ({} of {})</font><br>",
            page.squad.len(),
            page.max
        ));
        for (index, role) in page.squad.iter().enumerate() {
            html.push_str(&button(
                &format!("Dismiss the {}", role.title()),
                &format!("dismiss:{index}"),
            ));
        }
        html.push_str(&button("Dismiss all", "dismissall"));
    }
    html.push_str("</center></body></html>");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page<'a>(squad: &'a [Role], note: Option<&'a str>) -> MainPage<'a> {
        MainPage {
            fee: 200_000,
            squad,
            max: 3,
            min_level: 20,
            note,
        }
    }

    #[test]
    fn the_page_offers_every_role_with_the_price() {
        let html = main_page(&page(&[], None));
        for role in Role::ALL {
            assert!(
                html.contains(&format!("Script Companions hire:{}", role.key())),
                "{}",
                role.key()
            );
            assert!(html.contains(role.about()));
        }
        assert!(html.contains("200,000 Adena"));
        assert!(html.contains("5,000 for every level"));
        assert!(!html.contains("dismiss"), "nobody to dismiss yet");
        assert!(html.len() < 5_000, "{}", html.len());
    }

    #[test]
    fn the_hired_ones_can_be_dismissed_one_by_one_or_all_at_once() {
        let html = main_page(&page(&[Role::Tank, Role::Healer], None));
        assert!(html.contains("dismiss:0") && html.contains("dismiss:1"));
        assert!(!html.contains("dismiss:2"));
        assert!(html.contains("Dismiss the Tank") && html.contains("Dismiss the Healer"));
        assert!(html.contains("dismissall"));
        assert!(html.contains("(2 of 3)"));
        assert!(html.contains("hire:warrior"), "there is room for one more");
    }

    #[test]
    fn a_full_squad_cannot_hire_more() {
        let html = main_page(&page(&[Role::Tank, Role::Healer, Role::Warrior], None));
        assert!(!html.contains("hire:"));
        assert!(html.contains("dismiss:2"));
    }

    #[test]
    fn a_note_is_shown_at_the_top() {
        let html = main_page(&page(&[], Some("You need 200,000 Adena.")));
        assert!(html.contains("You need 200,000 Adena."));
        assert!(html.find("You need").unwrap() < html.find("A companion fights").unwrap());
    }
}
