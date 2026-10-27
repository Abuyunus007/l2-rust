//! `FavoriteBoard`: the favorite links (`bbs_favorites`).
// Java: dist/game/data/scripts/handlers/bypass/communityboard/FavoriteBoard.java
// Java: dist/game/data/scripts/handlers/bypass/communityboard/HomeBoard.java (getFavoriteCount)
//
// # Request and answer (the pattern for a board page that needs a table)
// Java reads and writes the table inside `onCommand`. The world cannot wait for the database
// (D-15), so a command sends a write ([`FavoriteWrite`]) and/or a read request
// ([`FavoriteRead`]) through the outbox; the persistence task applies them in order and answers a
// read with `WorldCommand::BbsFavoritesLoaded`, which builds the page ([`World::on_bbs_favorites_loaded`]).
// A read sent after a write sees it, which is Java's "Callback" (`onCommand("_bbsgetfav")`).

use l2_util::ObjectId;
use tracing::warn;

use crate::clanhall::siege::schedule::format_date_time;
use crate::outbox::PersistCommand;
use crate::world::World;

/// One row of `bbs_favorites`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FavoriteEntry {
    /// `favId`.
    pub fav_id: i64,
    /// `favTitle`.
    pub title: String,
    /// `favBypass`.
    pub bypass: String,
    /// `favAddDate` in epoch milliseconds.
    pub added_ms: i64,
}

/// A write of `bbs_favorites` with Java's statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FavoriteWrite {
    /// `REPLACE INTO bbs_favorites(playerId, favTitle, favBypass) VALUES(?, ?, ?)`.
    Add {
        player_id: i32,
        title: String,
        bypass: String,
    },
    /// `DELETE FROM bbs_favorites WHERE playerId=? AND favId=?`.
    Delete { player_id: i32, fav_id: i32 },
}

/// What a page needs from `bbs_favorites`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoriteRead {
    /// `SELECT ... WHERE playerId=? ORDER BY favAddDate DESC`: the `_bbsgetfav` page.
    List,
    /// `SELECT COUNT(*) ...`: the `%fav_count%` of the home page.
    Count,
}

/// The answer to a [`FavoriteRead`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FavoriteAnswer {
    List(Vec<FavoriteEntry>),
    Count(i64),
}

impl World {
    /// Java `FavoriteBoard.onCommand`.
    pub(super) fn favorite_board_command(&mut self, id: ObjectId, command: &str) {
        // None of these commands can be added to favorites (no `addBypass`).
        if command.starts_with("_bbsgetfav") {
            self.outbox.persist(PersistCommand::BbsFavoriteRequest {
                player: id,
                kind: FavoriteRead::List,
            });
        } else if command.starts_with("bbs_add_fav") {
            let Some(bypass) = self.board_remove_bypass(id) else {
                return;
            };
            // `bypass.split("&", 2)`: the title ends at the first `&`.
            let Some((title, link)) = bypass.split_once('&') else {
                warn!("FavoriteBoard: Couldn't add favorite link, {bypass} it's not a valid bypass!");
                return;
            };
            self.outbox
                .persist(PersistCommand::BbsFavorite(FavoriteWrite::Add {
                    player_id: id.0,
                    title: title.trim_matches(|c: char| c <= ' ').to_owned(),
                    bypass: link.trim_matches(|c: char| c <= ' ').to_owned(),
                }));
            // Callback.
            self.favorite_board_command(id, "_bbsgetfav");
        } else if command.starts_with("_bbsdelfav_") {
            let fav_id = command.replace("_bbsdelfav_", "");
            // `StringUtil.isNumeric`: every character is a digit.
            if fav_id.is_empty() || !fav_id.chars().all(|c| c.is_ascii_digit()) {
                warn!("FavoriteBoard: Couldn't delete favorite link, {fav_id} it's not a valid ID!");
                return;
            }
            // `Integer.parseInt` throws on a number too big; Java's catch logs and goes on.
            let Ok(fav_id) = fav_id.parse::<i32>() else {
                warn!("FavoriteBoard: Couldn't delete favorite link ID {fav_id} for {id}");
                return;
            };
            self.outbox
                .persist(PersistCommand::BbsFavorite(FavoriteWrite::Delete {
                    player_id: id.0,
                    fav_id,
                }));
            // Callback.
            self.favorite_board_command(id, "_bbsgetfav");
        }
    }

    /// `WorldCommand::BbsFavoritesLoaded`: the read the board asked for is in.
    pub(crate) fn on_bbs_favorites_loaded(
        &mut self,
        player: ObjectId,
        kind: FavoriteRead,
        result: Result<FavoriteAnswer, ()>,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        match (kind, result) {
            (FavoriteRead::List, Ok(FavoriteAnswer::List(rows))) => self.send_favorite_page(player, &rows),
            (FavoriteRead::List, _) => warn!("FavoriteBoard: Couldn't load favorite links for {player}"),
            (FavoriteRead::Count, Ok(FavoriteAnswer::Count(count))) => {
                self.home_page_with_count(player, count)
            }
            (FavoriteRead::Count, _) => {
                warn!("FavoriteBoard: Coudn't load favorites count for {player}");
                self.home_page_with_count(player, 0);
            }
        }
    }

    /// The `_bbsgetfav` page: `favorite_list.html` once per row inside `favorite.html`.
    fn send_favorite_page(&mut self, player: ObjectId, rows: &[FavoriteEntry]) {
        let (Some(list), Some(page)) = (
            self.get_htm(player, "data/html/CommunityBoard/favorite_list.html"),
            self.get_htm(player, "data/html/CommunityBoard/favorite.html"),
        ) else {
            warn!("FavoriteBoard: Couldn't load favorite links for {player}");
            return;
        };
        let mut links = String::new();
        for row in rows {
            let link = list
                .replace("%fav_bypass%", &row.bypass)
                .replace("%fav_title%", &row.title)
                .replace(
                    "%fav_add_date%",
                    &format_date_time(row.added_ms, l2_util::time::local_offset_millis(row.added_ms)),
                )
                .replace("%fav_id%", &row.fav_id.to_string());
            links.push_str(&link);
        }
        let html = page.replace("%fav_list%", &links);
        self.send_cb_html(player, &html);
    }
}
