//! The community board: `RequestShowBoard`, `RequestBBSwrite`, and the pages of the board
//! handlers.
// Java: handler/CommunityBoardHandler.java, handler/IParseBoardHandler.java
// Java: network/clientpackets/{RequestShowBoard,RequestBBSwrite}.java, util/HtmlUtil.java (sendCBHtml)
// Java: dist/game/data/scripts/handlers/bypass/communityboard/*.java
//
// # How a command finds its board
// Java keeps every board's command words and picks the board whose word the command starts with
// (`cmd.toLowerCase().startsWith(word)`, over a `HashMap`, so which board wins an overlap is
// left to the hash order). The words overlap in one place only: `_bbstop` (HomeBoard) is a prefix
// of `_bbstopics` (MemoBoard). The longest word wins here ([`Board::find`]).
//
// # Where things live
// - `mod.rs`: the settings, the last bypass of each player, the entry points and the small boards
//   (homepage, mail, memo, friends), which only show a page.
// - `home.rs`, `favorites.rs`, `region.rs`, `clan_board.rs`: one board each.
//
// # Database
// Only the favorites (`bbs_favorites`) are rows; the rest is read from the world. The favorites
// go out as `PersistCommand::BbsFavorite` / `BbsFavoriteRequest` and come back as
// `WorldCommand::BbsFavoritesLoaded` (D-15); the persistence task applies them in order, so a list
// read after an add or a delete includes it.
//
// # Left out, with the card that brings it
// - `DropSearchBoard` (`_bbs_search_item`, `_bbs_search_drop`, `_bbs_npc_trace`): it needs the drop
//   rates with the player's bonus multipliers and the premium rates (`PlayerStat.getBonusDrop*`),
//   and `Custom/dropsearch` pages; with `NpcViewMod`, which shares that code (S-HAND-02).
// - The custom commands of `HomeBoard` (`_bbsmultisell`, `_bbsexcmultisell`, `_bbssell`,
//   `_bbsteleport`, `_bbsbuff`, `_bbsheal`, `_bbsdelevel`, `_bbspremium`): `CommunityBoard.ini`
//   keeps their lists (`CommunityTeleportList`, `CommunityAvailableBuffs`) as maps the generated
//   config does not read yet, and premium is M9. A command of that kind is "not handled", as in a
//   Java server that has not registered it.

mod clan_board;
mod favorites;
mod home;
mod region;

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ShowBoard, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

pub use favorites::{FavoriteAnswer, FavoriteEntry, FavoriteRead, FavoriteWrite};

use crate::world::World;

/// `CommunityBoardConfig` and the two board keys of `General.ini`; `Default` is the Java default of
/// each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityBoardSettings {
    /// `GeneralConfig.ENABLE_COMMUNITY_BOARD` (`EnableCommunityBoard`).
    pub enabled: bool,
    /// `GeneralConfig.BBS_DEFAULT` (`BBSDefault`): the command `RequestShowBoard` runs.
    pub bbs_default: String,
    /// `CUSTOM_CB_ENABLED` (`CustomCommunityBoard`): the home page of `Custom/` with a navigation.
    pub custom_cb_enabled: bool,
    /// `COMMUNITYBOARD_PEACE_ONLY` (`CommunityBoardPeaceOnly`).
    pub peace_only: bool,
    /// `COMMUNITYBOARD_KARMA_DISABLED` (`CommunityKarmaDisabled`).
    pub karma_disabled: bool,
}

impl Default for CommunityBoardSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            bbs_default: "_bbshome".to_owned(),
            custom_cb_enabled: false,
            peace_only: false,
            karma_disabled: true,
        }
    }
}

/// What the community board keeps in the world (Java `CommunityBoardHandler._bypasses`).
#[derive(Debug, Default)]
pub struct CommunityBoardWorld {
    pub(crate) settings: CommunityBoardSettings,
    /// `addBypass(player, title, bypass)`: `"<title>&<bypass>"`, the page "add to favorites" saves.
    bypasses: HashMap<ObjectId, String>,
}

/// The boards (Java: one `IParseBoardHandler` each).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Board {
    Home,
    Homepage,
    Region,
    Favorite,
    Clan,
    Mail,
    Memo,
    Friends,
}

impl Board {
    /// The command words of every board (Java `getCommandList()`).
    const COMMANDS: &'static [(&'static str, Board)] = &[
        ("_bbshome", Board::Home),
        ("_bbstop", Board::Home),
        ("_bbslink", Board::Homepage),
        ("_bbsloc", Board::Region),
        ("_bbsgetfav", Board::Favorite),
        ("bbs_add_fav", Board::Favorite),
        ("_bbsdelfav_", Board::Favorite),
        ("_bbsclan", Board::Clan),
        ("_bbsclan_list", Board::Clan),
        ("_bbsclan_clanhome", Board::Clan),
        ("_maillist", Board::Mail),
        ("_bbsmemo", Board::Memo),
        ("_bbstopics", Board::Memo),
        ("_friendlist", Board::Friends),
        ("_friendblocklist", Board::Friends),
    ];

    /// Java `CommunityBoardHandler.getHandler(cmd)`.
    // NOTE(java-quirk): Java's `HashMap` iteration decides which board a command that several
    // words are a prefix of belongs to (`_bbstopics` is both `_bbstop` and `_bbstopics`). The
    // longest word wins here, so `_bbstopics` is the memo board's.
    pub(crate) fn find(command: &str) -> Option<Board> {
        let lower = command.to_lowercase();
        Self::COMMANDS
            .iter()
            .filter(|(word, _)| lower.starts_with(&word.to_lowercase()))
            .max_by_key(|(word, _)| word.len())
            .map(|(_, board)| *board)
    }
}

/// `separateAndSend("<html><body><br><br><center>...</center><br><br></body></html>")`.
fn center_message(text: &str) -> String {
    format!("<html><body><br><br><center>{text}</center><br><br></body></html>")
}

impl World {
    /// Installs the board settings (`General.ini`, `Custom/CommunityBoard.ini`).
    pub fn set_community_board_settings(&mut self, settings: CommunityBoardSettings) {
        self.community_board.settings = settings;
    }

    /// The board settings.
    pub fn community_board_settings(&self) -> &CommunityBoardSettings {
        &self.community_board.settings
    }

    /// Java `CommunityBoardHandler.isCommunityBoardCommand(cmd)`.
    pub fn is_community_board_command(&self, command: &str) -> bool {
        Board::find(command).is_some()
    }

    /// Java `RequestShowBoard.runImpl`: the page `BBSDefault` names.
    pub(crate) fn on_request_show_board(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        let command = self.community_board.settings.bbs_default.clone();
        self.board_handle_parse_command(id, &command);
    }

    /// Java `CommunityBoardHandler.handleParseCommand(command, player)`.
    pub(crate) fn board_handle_parse_command(&mut self, id: ObjectId, command: &str) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.community_board.settings.enabled {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheCommunityServerIsCurrentlyOffline),
            );
            return;
        }
        let Some(board) = Board::find(command) else {
            warn!("CommunityBoardHandler: Couldn't find parse handler for command {command}!");
            return;
        };
        self.board_on_command(board, id, command);
    }

    /// `IParseBoardHandler.onCommand(command, player)` of a board.
    fn board_on_command(&mut self, board: Board, id: ObjectId, command: &str) {
        match board {
            Board::Home => self.home_board_command(id, command),
            Board::Homepage => self.homepage_board_command(id),
            Board::Region => self.region_board_command(id, command),
            Board::Favorite => self.favorite_board_command(id, command),
            Board::Clan => self.clan_board_command(id, command),
            Board::Mail => self.mail_board_command(id, command),
            Board::Memo => self.memo_board_command(id, command),
            Board::Friends => self.friends_board_command(id, command),
        }
    }

    /// Java `RequestBBSwrite.runImpl` → `CommunityBoardHandler.handleWriteCommand`.
    pub(crate) fn on_request_bbs_write(&mut self, id: ObjectId, url: &str, args: &[String; 5]) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.community_board.settings.enabled {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheCommunityServerIsCurrentlyOffline),
            );
            return;
        }
        let command = match url {
            "Topic" => "_bbstop",
            // TODO in Java: "Implement."
            "Post" => "_bbspos",
            "Region" => "_bbsloc",
            "Notice" => "_bbsclan",
            _ => {
                self.send_cb_html(
                    id,
                    &center_message(&format!("The command: {url} is not implemented yet.")),
                );
                return;
            }
        };
        let Some(board) = Board::find(command) else {
            warn!("CommunityBoardHandler: Couldn't find write handler for command {command}!");
            return;
        };
        match board {
            // `ClanBoard.writeCommunityBoardCommand`: the clan notice.
            Board::Clan => self.clan_board_write(id, &args[2]),
            // `RegionBoard`, `MailBoard` and `MemoBoard` implement `IWriteBoardHandler` with a
            // `// TODO: Implement.` that does nothing.
            Board::Region | Board::Mail | Board::Memo => {}
            Board::Home | Board::Homepage | Board::Favorite | Board::Friends => {
                warn!("CommunityBoardHandler: {board:?}Board doesn't implement write!");
            }
        }
    }

    /// Java `CommunityBoardHandler.addBypass(player, title, bypass)`.
    pub(crate) fn board_add_bypass(&mut self, id: ObjectId, title: &str, bypass: &str) {
        self.community_board
            .bypasses
            .insert(id, format!("{title}&{bypass}"));
    }

    /// Java `CommunityBoardHandler.removeBypass(player)`.
    pub(crate) fn board_remove_bypass(&mut self, id: ObjectId) -> Option<String> {
        self.community_board.bypasses.remove(&id)
    }

    /// `CommunityBoardHandler.separateAndSend(HtmCache.getHtm(player, path), player)`. A page the
    /// cache does not know is `null` in Java, which sends nothing.
    pub(crate) fn board_send_page(&mut self, id: ObjectId, path: &str) {
        if let Some(html) = self.get_htm(id, path) {
            self.send_cb_html(id, &html);
        }
    }

    /// `HomepageBoard.onCommand`.
    fn homepage_board_command(&mut self, id: ObjectId) {
        self.board_send_page(id, "data/html/CommunityBoard/homepage.html");
    }

    /// `MailBoard.onCommand`.
    fn mail_board_command(&mut self, id: ObjectId, command: &str) {
        self.board_add_bypass(id, "Mail Command", command);
        self.board_send_page(id, "data/html/CommunityBoard/mail.html");
    }

    /// `MemoBoard.onCommand`.
    fn memo_board_command(&mut self, id: ObjectId, command: &str) {
        self.board_add_bypass(id, "Memo Command", command);
        self.board_send_page(id, "data/html/CommunityBoard/memo.html");
    }

    /// `FriendsBoard.onCommand`.
    fn friends_board_command(&mut self, id: ObjectId, command: &str) {
        if command == "_friendlist" {
            self.board_add_bypass(id, "Friends List", command);
            self.board_send_page(id, "data/html/CommunityBoard/friends_list.html");
        } else if command == "_friendblocklist" {
            self.board_add_bypass(id, "Ignore list", command);
            self.board_send_page(id, "data/html/CommunityBoard/friends_block_list.html");
        }
    }

    /// `HtmlUtil.sendCBHtml(player, html, fillMultiEdit)`: the page in one `ShowBoard` ("1001"), then
    /// the arguments packet that fills the multiedit field with `text`.
    pub(crate) fn send_cb_html_multi_edit(&mut self, id: ObjectId, html: &str, text: &str) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.talk.clear(crate::dialog::HtmlActionScope::CommBoardHtml);
        crate::dialog::build_html_action_cache(
            &mut player.talk,
            crate::dialog::HtmlActionScope::CommBoardHtml,
            0,
            html,
        );
        let name = player.data.name.clone();
        let account = player.data.account.clone();
        self.send(id, &ShowBoard::page(Some(html), "1001"));
        // `System.lineSeparator()`: the text uses the separator of the machine the server runs on.
        let separator = if cfg!(windows) { "\r\n" } else { "\n" };
        let args = [
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            &name,
            &id.0.to_string(),
            &account,
            "9",
            " ",
            " ",
            &text.replace("<br>", separator),
            "0",
            "0",
            "0",
            "0",
        ]
        .map(str::to_owned);
        self.send(id, &ShowBoard::arguments(&args));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_find_their_board() {
        assert_eq!(Board::find("_bbshome"), Some(Board::Home));
        assert_eq!(Board::find("_bbstop;Custom/a.html"), Some(Board::Home));
        assert_eq!(Board::find("_bbsloc;3"), Some(Board::Region));
        assert_eq!(Board::find("_bbsdelfav_12"), Some(Board::Favorite));
        assert_eq!(Board::find("_bbsclan_clanlist;2"), Some(Board::Clan));
        assert_eq!(Board::find("_bbsclan_clannotice_edit;1"), Some(Board::Clan));
        assert_eq!(Board::find("_friendblocklist"), Some(Board::Friends));
        assert_eq!(
            Board::find("_BBSHOME"),
            Some(Board::Home),
            "Java compares lower case"
        );
        assert_eq!(Board::find("_bbsunknown"), None);
        assert_eq!(Board::find("admin_x"), None);
    }

    #[test]
    fn the_overlap_goes_to_the_longer_word() {
        assert_eq!(Board::find("_bbstopics"), Some(Board::Memo));
        assert_eq!(Board::find("_bbstop"), Some(Board::Home));
    }

    #[test]
    fn not_implemented_message_is_the_java_string() {
        assert_eq!(
            center_message("The command: X is not implemented yet."),
            "<html><body><br><br><center>The command: X is not implemented yet.</center><br><br></body></html>"
        );
    }
}
