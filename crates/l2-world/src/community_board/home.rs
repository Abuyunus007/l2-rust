//! `HomeBoard`: the top page of the board (`_bbshome`, `_bbstop`, `_bbstop;<page>.html`).
// Java: dist/game/data/scripts/handlers/bypass/communityboard/HomeBoard.java
//
// The custom commands (`_bbsmultisell`, `_bbsteleport`, `_bbsbuff`, `_bbsheal`, ...) are left out,
// see the module note of `mod.rs`.

use l2_util::ObjectId;

use super::favorites::FavoriteRead;
use crate::outbox::PersistCommand;
use crate::world::World;
use crate::zones::ZoneId;

/// `HomeBoard.NAVIGATION_PATH`.
const NAVIGATION_PATH: &str = "data/html/CommunityBoard/Custom/navigation.html";

impl World {
    /// Java `HomeBoard.onCommand`.
    pub(super) fn home_board_command(&mut self, id: ObjectId, command: &str) {
        let settings = &self.community_board.settings;
        let (custom, karma_disabled, peace_only) = (
            settings.custom_cb_enabled,
            settings.karma_disabled,
            settings.peace_only,
        );
        // The combat check of `COMBAT_CHECK` only applies to the custom commands, none of which
        // is registered here.
        if karma_disabled && self.karma_of_player(id) > 0 {
            self.send_message(id, "Players with Karma cannot use the Community Board.");
            return;
        }
        if peace_only && !self.is_inside_zone(id, ZoneId::Peace) {
            self.send_message(id, "Community Board cannot be used out of peace zone.");
            return;
        }

        let custom_path = if custom { "Custom/" } else { "" };
        if command == "_bbshome" || command == "_bbstop" {
            self.board_add_bypass(id, "Home", command);
            if custom {
                let html = self.get_htm(id, "data/html/CommunityBoard/Custom/home.html");
                self.finish_home_page(id, html);
            } else {
                // `%fav_count%` is a database read: the page goes out when the answer is in.
                self.outbox.persist(PersistCommand::BbsFavoriteRequest {
                    player: id,
                    kind: FavoriteRead::Count,
                });
            }
        } else if command.starts_with("_bbstop;") {
            let path = command.replace("_bbstop;", "");
            if !path.is_empty() && path.ends_with(".html") {
                let html = self.get_htm(id, &format!("data/html/CommunityBoard/{custom_path}{path}"));
                self.finish_home_page(id, html);
            }
        }
    }

    /// The end of `HomeBoard.onCommand`: `%navigation%` for the custom board, then the page.
    fn finish_home_page(&mut self, id: ObjectId, html: Option<String>) {
        let Some(mut html) = html else {
            // Java: the null page is sent as nothing (or throws before it).
            return;
        };
        if self.community_board.settings.custom_cb_enabled {
            // `returnHtml.replace("%navigation%", navigation)` throws on a missing navigation.
            let Some(navigation) = self.get_htm(id, NAVIGATION_PATH) else {
                return;
            };
            html = html.replace("%navigation%", &navigation);
        }
        self.send_cb_html(id, &html);
    }

    /// The `_bbshome` page once the number of favorites is known (`getFavoriteCount`: 0 when the
    /// read failed).
    pub(super) fn home_page_with_count(&mut self, id: ObjectId, favorites: i64) {
        let Some(html) = self.get_htm(id, "data/html/CommunityBoard/home.html") else {
            return;
        };
        let clan_count = self.clans().count();
        let html = html
            .replace("%fav_count%", &favorites.to_string())
            // `getRegionCount`: `return 0; // TODO: Implement.`
            .replace("%region_count%", "0")
            .replace("%clan_count%", &clan_count.to_string());
        self.send_cb_html(id, &html);
    }
}
