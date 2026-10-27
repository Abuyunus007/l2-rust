//! `RegionBoard`: the castles and their owners (`_bbsloc`, `_bbsloc;<region>`).
// Java: dist/game/data/scripts/handlers/bypass/communityboard/RegionBoard.java
//
// Java reads the clan halls of a region with `SELECT id, name, ownerId, location FROM clanhall
// WHERE location = ?`; the world already holds the same rows (the auctionable halls, loaded at the
// start and re-read with the residences, D-50), so the page is built from them. Conquerable halls
// (`siegable_clanhall`) are not rows of that table and are left out.

use l2_util::ObjectId;
use tracing::warn;

use crate::clanhall::siege::schedule::format_date_time;
use crate::world::World;

/// `RegionBoard.REGIONS`: the system message ids (`&@<id>;` names) of the nine castle towns.
const REGIONS: [i32; 9] = [1049, 1052, 1053, 1057, 1060, 1059, 1248, 1247, 1056];

/// `SIEGE_FORMAT` (`dd-MM-yyyy HH:mm`) of an instant, with the offset of the zone.
fn siege_format(epoch_ms: i64, offset_ms: i64) -> String {
    // "yyyy-MM-dd HH:mm:ss" re-ordered.
    let text = format_date_time(epoch_ms, offset_ms);
    match (text.get(0..4), text.get(5..7), text.get(8..10), text.get(11..16)) {
        (Some(year), Some(month), Some(day), Some(time)) => format!("{day}-{month}-{year} {time}"),
        _ => text,
    }
}

/// The link to a clan's home page, or `NPC` for a castle nobody owns.
fn clan_link(id: i32, name: &str) -> String {
    format!("<a action=\"bypass _bbsclan_clanhome;{id}\">{name}</a>")
}

impl World {
    /// Java `RegionBoard.onCommand`.
    pub(super) fn region_board_command(&mut self, id: ObjectId, command: &str) {
        if command == "_bbsloc" {
            self.board_add_bypass(id, "Region", command);
            self.region_list_page(id);
        } else if command.starts_with("_bbsloc;") {
            self.board_add_bypass(id, "Region>", command);
            let region = command.replace("_bbsloc;", "");
            // `StringUtil.isNumeric`: every character is a digit.
            if region.is_empty() || !region.chars().all(|c| c.is_ascii_digit()) {
                warn!("RegionBoard: {id} sent an invalid region bypass {command}!");
                return;
            }
            // `Integer.parseInt` throws on a number too big.
            let Ok(region) = region.parse::<usize>() else {
                warn!("RegionBoard: {id} sent an invalid region bypass {command}!");
                return;
            };
            self.region_show_page(id, region);
        }
    }

    /// The `_bbsloc` page: `region_list.html` once per castle inside `region.html`.
    fn region_list_page(&mut self, id: ObjectId) {
        let (Some(list), Some(page)) = (
            self.get_htm(id, "data/html/CommunityBoard/region_list.html"),
            self.get_htm(id, "data/html/CommunityBoard/region.html"),
        ) else {
            return;
        };
        let mut rows = String::new();
        for (index, region_name) in REGIONS.iter().enumerate() {
            // `castles.get(i)` throws when the server has fewer castles.
            let Some(castle) = self.castles().get(index) else {
                break;
            };
            let clan = self.clan(castle.owner_id);
            let owner = clan.map_or_else(|| "NPC".to_owned(), |c| clan_link(c.id, &c.name));
            let ally = clan
                .and_then(|c| c.ally_name.clone())
                .unwrap_or_else(|| "None".to_owned());
            // `castle.getTaxRate() * 100` is a `double`.
            let tax = format!("{:?}%", castle.tax_rate * 100.0);
            rows.push_str(
                &list
                    .replace("%region_id%", &index.to_string())
                    .replace("%region_name%", &region_name.to_string())
                    .replace("%region_owning_clan%", &owner)
                    .replace("%region_owning_clan_alliance%", &ally)
                    .replace("%region_tax_rate%", &tax),
            );
        }
        self.send_cb_html(id, &page.replace("%region_list%", &rows));
    }

    /// The `_bbsloc;<region>` page: one castle and the clan halls of its region.
    fn region_show_page(&mut self, id: ObjectId, region: usize) {
        let castles = self.castles();
        let Some(castle) = castles.get(region) else {
            warn!("RegionBoard: {id} requested invalid region ID {region}!");
            return;
        };
        let Some(page) = self.get_htm(id, "data/html/CommunityBoard/region_show.html") else {
            return;
        };
        let clan = self.clan(castle.owner_id);
        let lord = clan.map_or_else(|| "NPC".to_owned(), |c| self.clan_leader_name(c.id));
        let owner = clan.map_or_else(|| "NPC".to_owned(), |c| clan_link(c.id, &c.name));
        let ally = clan
            .and_then(|c| c.ally_name.clone())
            .unwrap_or_else(|| "None".to_owned());
        let siege_date = siege_format(
            castle.siege_date,
            l2_util::time::local_offset_millis(castle.siege_date),
        );
        let tax = format!("{:?}%", castle.tax_rate * 100.0);

        let mut halls = String::from(
            "<br><br><table width=610 bgcolor=A7A19A><tr><td width=5></td><td width=200>Clan Hall Name</td><td width=200>Owning Clan</td><td width=200>Clan Leader Name</td><td width=5></td></tr></table><br1>",
        );
        let in_region: Vec<_> = self
            .clan_halls()
            .filter(|h| !h.is_siegable_hall() && h.location == castle.name)
            .collect();
        if in_region.is_empty() {
            halls.push_str(
                "<table width=610><tr><td align=\"left\">There are no clan halls in this region.</td></tr></table>",
            );
        } else {
            for hall in in_region {
                let owner_clan = self.clan(hall.owner_id);
                halls.push_str("<table><tr><td width=5></td><td width=200>");
                halls.push_str(&hall.name);
                halls.push_str("</td><td width=200>");
                halls.push_str(&owner_clan.map_or_else(|| "None".to_owned(), |c| clan_link(c.id, &c.name)));
                halls.push_str("</td><td width=200>");
                halls
                    .push_str(&owner_clan.map_or_else(|| "None".to_owned(), |c| self.clan_leader_name(c.id)));
                halls.push_str(
                    "</td><td width=5></td></tr></table><br1><img src=\"L2UI.Squaregray\" width=605 height=1><br1>",
                );
            }
        }

        let html = page
            .replace("%regionName%", &castle.name)
            .replace("%tax%", &tax)
            .replace("%lord%", &lord)
            .replace("%clanName%", &owner)
            .replace("%allyName%", &ally)
            .replace("%siegeDate%", &siege_date)
            .replace("%hallsList%", &halls);
        self.send_cb_html(id, &html);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn siege_date_is_day_first_without_seconds() {
        // 2026-10-27 13:45:09 UTC.
        let ms = 1_793_108_709_000;
        assert_eq!(siege_format(ms, 0), "27-10-2026 13:45");
        assert_eq!(siege_format(ms, 3_600_000), "27-10-2026 14:45");
    }

    #[test]
    fn clan_link_is_the_java_anchor() {
        assert_eq!(
            clan_link(268_500_001, "Alpha"),
            "<a action=\"bypass _bbsclan_clanhome;268500001\">Alpha</a>"
        );
    }
}
