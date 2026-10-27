//! `ClanBoard`: the clan community (`_bbsclan`, `_bbsclan_clanlist`, `_bbsclan_clanhome`, the clan
//! notice pages) and its `Write Notice` button.
// Java: dist/game/data/scripts/handlers/bypass/communityboard/ClanBoard.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::warn;

use super::center_message;
use crate::world::World;

/// Java `SystemMessageId.THERE_ARE_NO_COMMUNITIES_IN_MY_CLAN_...`.
const NO_COMMUNITY: SystemMessageId =
    SystemMessageId::ThereAreNoCommunitiesInMyClanClanCommunitiesAreAllowedForClansWithSkillLevelsOf2AndHigher;

/// The clan list head.
const LIST_HEAD_1: &str = "<html><body><br><br><center><br1><br1><table border=0 cellspacing=0 cellpadding=0><tr><td FIXWIDTH=15>&nbsp;</td><td width=610 height=30 align=left><a action=\"bypass _bbshome\">HOME</a> &nbsp;&gt;<a action=\"bypass _bbsclan_clanlist\"> CLAN COMMUNITY </a></td></tr></table><table border=0 cellspacing=0 cellpadding=0 width=610 bgcolor=434343><tr><td height=10></td></tr><tr><td fixWIDTH=5></td><td fixWIDTH=600><a action=\"bypass _bbsclan_clanhome;";
const LIST_HEAD_2: &str = "\">[GO TO MY CLAN]</a>&nbsp;&nbsp;</td><td fixWIDTH=5></td></tr><tr><td height=10></td></tr></table><br><table border=0 cellspacing=0 cellpadding=2 bgcolor=5A5A5A width=610><tr><td FIXWIDTH=5></td><td FIXWIDTH=200 align=center>CLAN NAME</td><td FIXWIDTH=200 align=center>CLAN LEADER</td><td FIXWIDTH=100 align=center>CLAN LEVEL</td><td FIXWIDTH=100 align=center>CLAN MEMBERS</td><td FIXWIDTH=5></td></tr></table><img src=\"L2UI.Squareblank\" width=\"1\" height=\"5\">";
/// The clan list tail (the search form: a button that writes to `RequestBBSwrite`).
const LIST_TAIL: &str = "</tr></table><table border=0 cellspacing=0 cellpadding=0><tr><td width=610><img src=\"sek.cbui141\" width=\"610\" height=\"1\"></td></tr></table><table border=0><tr><td><combobox width=65 var=keyword list=\"Name;Ruler\"></td><td><edit var = \"Search\" width=130 height=11 length=\"16\"></td><td><button value=\"&$420;\" action=\"Write 5 -1 0 Search keyword keyword\" back=\"l2ui_ch3.smallbutton2_down\" width=65 height=20 fore=\"l2ui_ch3.smallbutton2\"> </td> </tr></table><br><br></center></body></html>";

/// One clan of the list: what `clanList` reads.
struct ListRow {
    id: i32,
    name: String,
    leader: String,
    level: i32,
    members: usize,
}

/// The `_bbsclan_clanlist` page for `index` (1 and up) over `clans` (Java iterates
/// `ClanTable.getClans()`), for a player of clan `own_clan_id` (0 without a clan).
// NOTE(java-quirk): the loop stops at `i > (index + 1) * 7` and shows from `(index - 1) * 7` on, so a
// page holds up to 15 clans while the pager counts pages of 8; both are kept.
fn clan_list_html(rows: &[ListRow], index: i32, own_clan_id: i32) -> String {
    let index = index.max(1);
    let mut html = String::with_capacity(2048);
    html.push_str(LIST_HEAD_1);
    html.push_str(&own_clan_id.to_string());
    html.push_str(LIST_HEAD_2);
    for (i, row) in (0_i64..).zip(rows.iter()) {
        if i > (i64::from(index) + 1) * 7 {
            break;
        }
        let shown = i >= (i64::from(index) - 1) * 7;
        if shown {
            html.push_str(&format!(
                "<img src=\"L2UI.SquareBlank\" width=\"610\" height=\"3\"><table border=0 cellspacing=0 cellpadding=0 width=610><tr> <td FIXWIDTH=5></td><td FIXWIDTH=200 align=center><a action=\"bypass _bbsclan_clanhome;{}\">{}</a></td><td FIXWIDTH=200 align=center>{}</td><td FIXWIDTH=100 align=center>{}</td><td FIXWIDTH=100 align=center>{}</td><td FIXWIDTH=5></td></tr><tr><td height=5></td></tr></table><img src=\"L2UI.SquareBlank\" width=\"610\" height=\"3\"><img src=\"L2UI.SquareGray\" width=\"610\" height=\"1\">",
                row.id, row.name, row.leader, row.level, row.members
            ));
        }
    }
    html.push_str("<img src=\"L2UI.SquareBlank\" width=\"610\" height=\"2\"><table cellpadding=0 cellspacing=2 border=0><tr>");
    if index == 1 {
        html.push_str("<td><button action=\"\" back=\"l2ui_ch3.prev1_down\" fore=\"l2ui_ch3.prev1\" width=16 height=16 ></td>");
    } else {
        // NOTE(java-quirk): this action has no `bypass ` prefix, so the client sends it as typed.
        html.push_str(&format!(
            "<td><button action=\"_bbsclan_clanlist;{}\" back=\"l2ui_ch3.prev1_down\" fore=\"l2ui_ch3.prev1\" width=16 height=16 ></td>",
            index - 1
        ));
    }
    let count = i32::try_from(rows.len()).unwrap_or(i32::MAX);
    let mut pages = count / 8;
    if pages * 8 != count {
        pages += 1;
    }
    for page in 1..=pages {
        if page == index {
            html.push_str(&format!("<td> {page} </td>"));
        } else {
            html.push_str(&format!(
                "<td><a action=\"bypass _bbsclan_clanlist;{page}\"> {page} </a></td>"
            ));
        }
    }
    if index == pages {
        html.push_str("<td><button action=\"\" back=\"l2ui_ch3.next1_down\" fore=\"l2ui_ch3.next1\" width=16 height=16 ></td>");
    } else {
        html.push_str(&format!(
            "<td><button action=\"bypass _bbsclan_clanlist;{}\" back=\"l2ui_ch3.next1_down\" fore=\"l2ui_ch3.next1\" width=16 height=16 ></td>",
            index + 1
        ));
    }
    html.push_str(LIST_TAIL);
    html
}

/// The `_bbsclan_clanhome` page of a clan of level 2 or more.
fn clan_home_html(clan_id: i32, name: &str, level: i32, members: usize, leader: &str, ally: &str) -> String {
    format!(
        "<html><body><center><br><br><br1><br1><table border=0 cellspacing=0 cellpadding=0><tr><td FIXWIDTH=15>&nbsp;</td><td width=610 height=30 align=left><a action=\"bypass _bbshome\">HOME</a> &nbsp;&gt; <a action=\"bypass _bbsclan_clanlist\"> CLAN COMMUNITY </a> &nbsp;&gt; <a action=\"bypass _bbsclan_clanhome;{clan_id}\"> &amp;$802; </a></td></tr></table><table border=0 cellspacing=0 cellpadding=0 width=610 bgcolor=434343><tr><td height=10></td></tr><tr><td fixWIDTH=5></td><td fixwidth=600><a action=\"bypass _bbsclan_clanhome;{clan_id};announce\">[CLAN ANNOUNCEMENT]</a> <a action=\"bypass _bbsclan_clanhome;{clan_id};cbb\">[CLAN BULLETIN BOARD]</a><a action=\"bypass _bbsclan_clanhome;{clan_id};cmail\">[CLAN MAIL]</a>&nbsp;&nbsp;<a action=\"bypass _bbsclan_clannotice_edit;{clan_id};cnotice\">[CLAN NOTICE]</a>&nbsp;&nbsp;</td><td fixWIDTH=5></td></tr><tr><td height=10></td></tr></table><table border=0 cellspacing=0 cellpadding=0 width=610><tr><td height=10></td></tr><tr><td fixWIDTH=5></td><td fixwidth=290 valign=top></td><td fixWIDTH=5></td><td fixWIDTH=5 align=center valign=top><img src=\"l2ui.squaregray\" width=2  height=128></td><td fixWIDTH=5></td><td fixwidth=295><table border=0 cellspacing=0 cellpadding=0 width=295><tr><td fixWIDTH=100 align=left>CLAN NAME</td><td fixWIDTH=195 align=left>{name}</td></tr><tr><td height=7></td></tr><tr><td fixWIDTH=100 align=left>CLAN LEVEL</td><td fixWIDTH=195 align=left height=16>{level}</td></tr><tr><td height=7></td></tr><tr><td fixWIDTH=100 align=left>CLAN MEMBERS</td><td fixWIDTH=195 align=left height=16>{members}</td></tr><tr><td height=7></td></tr><tr><td fixWIDTH=100 align=left>CLAN LEADER</td><td fixWIDTH=195 align=left height=16>{leader}</td></tr><tr><td height=7></td></tr><tr><td height=7></td></tr><tr><td fixWIDTH=100 align=left>ALLIANCE</td><td fixWIDTH=195 align=left height=16>{ally}</td></tr></table></td><td fixWIDTH=5></td></tr><tr><td height=10></td></tr></table><img src=\"l2ui.squareblank\" width=\"1\" height=\"5\"><img src=\"l2ui.squaregray\" width=\"610\" height=\"1\"><br></center><br> <br></body></html>"
    )
}

/// The head of the notice pages.
fn notice_head(clan_id: i32) -> String {
    format!(
        "<html><body><br><br><table border=0 width=610><tr><td width=10></td><td width=600 align=left><a action=\"bypass _bbshome\">HOME</a> &gt; <a action=\"bypass _bbsclan_clanlist\"> CLAN COMMUNITY </a>  &gt; <a action=\"bypass _bbsclan_clanhome;{clan_id}\"> &amp;$802; </a></td></tr></table>"
    )
}

/// The editor of the clan leader: the on/off switch and the multiedit.
fn notice_editor_html(clan_id: i32, enabled: bool) -> String {
    let mut html = notice_head(clan_id);
    html.push_str("<br><br><center><table width=610 border=0 cellspacing=0 cellpadding=0><tr><td fixwidth=610><font color=\"AAAAAA\">The Clan Notice function allows the clan leader to send messages through a pop-up window to clan members at login.</font> </td></tr><tr><td height=20></td></tr>");
    if enabled {
        html.push_str("<tr><td fixwidth=610> Clan Notice Function:&nbsp;&nbsp;&nbsp;on&nbsp;&nbsp;&nbsp;/&nbsp;&nbsp;&nbsp;<a action=\"bypass _bbsclan_clannotice_disable\">off</a>");
    } else {
        html.push_str("<tr><td fixwidth=610> Clan Notice Function:&nbsp;&nbsp;&nbsp;<a action=\"bypass _bbsclan_clannotice_enable\">on</a>&nbsp;&nbsp;&nbsp;/&nbsp;&nbsp;&nbsp;off");
    }
    html.push_str("</td></tr></table><img src=\"L2UI.Squaregray\" width=\"610\" height=\"1\"><br> <br><table width=610 border=0 cellspacing=2 cellpadding=0><tr><td>Edit Notice: </td></tr><tr><td height=5></td></tr><tr><td><MultiEdit var =\"Content\" width=610 height=100></td></tr></table><br><table width=610 border=0 cellspacing=0 cellpadding=0><tr><td height=5></td></tr><tr><td align=center FIXWIDTH=65><button value=\"&$140;\" action=\"Write Notice Set _ Content Content Content\" back=\"l2ui_ch3.smallbutton2_down\" width=65 height=20 fore=\"l2ui_ch3.smallbutton2\" ></td><td align=center FIXWIDTH=45></td><td align=center FIXWIDTH=500></td></tr></table></center></body></html>");
    html
}

/// The page of a member that is not the leader: the notice, when it is on.
fn notice_view_html(clan_id: i32, enabled: bool, notice: &str) -> String {
    let mut html = notice_head(clan_id);
    html.push_str("<img src=\"L2UI.squareblank\" width=\"1\" height=\"10\"><center><table border=0 cellspacing=0 cellpadding=0><tr><td>You are not your clan's leader, and therefore cannot change the clan notice</td></tr></table>");
    if enabled {
        html.push_str(&format!(
            "<table border=0 cellspacing=0 cellpadding=0><tr><td>The current clan notice:</td></tr><tr><td fixwidth=5></td><td FIXWIDTH=600 align=left>{notice}</td><td fixqqwidth=5></td></tr></table>"
        ));
    }
    html.push_str("</center></body></html>");
    html
}

impl World {
    /// Java `ClanBoard.onCommand`.
    pub(super) fn clan_board_command(&mut self, id: ObjectId, command: &str) {
        if command == "_bbsclan" {
            self.board_add_bypass(id, "Clan", command);
            if self.clan_of(id).is_none_or(|c| c.level < 2) {
                self.clan_list(id, 1);
            } else {
                self.clan_home_of_player(id);
            }
        } else if command.starts_with("_bbsclan_clanlist") {
            self.board_add_bypass(id, "Clan List", command);
            if command == "_bbsclan_clanlist" {
                self.clan_list(id, 1);
            } else if command.starts_with("_bbsclan_clanlist;") {
                // `Integer.parseInt(command.split(";")[1])`; any failure shows the first page.
                match command.split(';').nth(1).and_then(|v| v.parse::<i32>().ok()) {
                    Some(index) => self.clan_list(id, index),
                    None => {
                        self.clan_list(id, 1);
                        warn!("ClanBoard: {id} send invalid clan list bypass {command}!");
                    }
                }
            }
        } else if command.starts_with("_bbsclan_clanhome") {
            self.board_add_bypass(id, "Clan Home", command);
            if command == "_bbsclan_clanhome" {
                self.clan_home_of_player(id);
            } else if command.starts_with("_bbsclan_clanhome;") {
                match command.split(';').nth(1).and_then(|v| v.parse::<i32>().ok()) {
                    Some(clan_id) => self.clan_home(id, clan_id),
                    None => {
                        self.clan_home_of_player(id);
                        warn!("ClanBoard: {id} send invalid clan home bypass {command}!");
                    }
                }
            }
        } else if command.starts_with("_bbsclan_clannotice_edit;") {
            self.board_add_bypass(id, "Clan Edit", command);
            let clan_id = self.clan_id_of(id);
            self.clan_notice(id, clan_id);
        } else if command.starts_with("_bbsclan_clannotice_enable") {
            self.board_add_bypass(id, "Clan Notice Enable", command);
            self.clan_notice_switch(id, true);
        } else if command.starts_with("_bbsclan_clannotice_disable") {
            self.board_add_bypass(id, "Clan Notice Disable", command);
            self.clan_notice_switch(id, false);
        } else {
            self.send_cb_html(
                id,
                &center_message(&format!("Command {command} need development.")),
            );
        }
    }

    /// `clan.setNoticeEnabled(value)` and the notice page.
    fn clan_notice_switch(&mut self, id: ObjectId, enabled: bool) {
        let clan_id = self.clan_id_of(id);
        if self.clan(clan_id).is_some() {
            self.set_clan_notice_enabled(clan_id, enabled);
        }
        self.clan_notice(id, clan_id);
    }

    /// Java `ClanBoard.writeCommunityBoardCommand`: the only write that reaches this board is
    /// `Write Notice Set _ Content Content Content`, whose text is `arg3`.
    pub(super) fn clan_board_write(&mut self, id: ObjectId, notice: &str) {
        let clan_id = self.clan_id_of(id);
        if clan_id != 0 && self.clan(clan_id).is_some() && self.is_clan_leader(id) {
            self.set_clan_notice(clan_id, notice);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourClanNoticeHasBeenSaved),
            );
        }
    }

    /// Java `ClanBoard.clanNotice(player, clanId)`.
    fn clan_notice(&mut self, id: ObjectId, clan_id: i32) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        if clan.level < 2 {
            self.send(id, &SystemMessage::new(NO_COMMUNITY));
            self.clan_board_command(id, "_bbsclan_clanlist");
            return;
        }
        let (enabled, notice) = (clan.notice_enabled, clan.notice_text());
        if self.is_clan_leader(id) {
            let html = notice_editor_html(clan_id, enabled);
            self.send_cb_html_multi_edit(id, &html, &notice);
        } else {
            let html = notice_view_html(clan_id, enabled, &notice);
            self.send_cb_html(id, &html);
        }
    }

    /// Java `ClanBoard.clanList(player, index)`.
    fn clan_list(&mut self, id: ObjectId, index: i32) {
        let rows: Vec<ListRow> = self
            .clans()
            .map(|c| ListRow {
                id: c.id,
                name: c.name.clone(),
                leader: self.clan_leader_name(c.id),
                level: c.level,
                members: c.members_count(),
            })
            .collect();
        let html = clan_list_html(&rows, index, self.clan_id_of(id));
        self.send_cb_html(id, &html);
    }

    /// `clanHome(player)`: the player's own clan. Java throws a `NullPointerException` for a player
    /// without a clan, which sends nothing.
    fn clan_home_of_player(&mut self, id: ObjectId) {
        let clan_id = self.clan_id_of(id);
        if clan_id != 0 {
            self.clan_home(id, clan_id);
        }
    }

    /// Java `ClanBoard.clanHome(player, clanId)`.
    fn clan_home(&mut self, id: ObjectId, clan_id: i32) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        if clan.level < 2 {
            self.send(id, &SystemMessage::new(NO_COMMUNITY));
            self.clan_board_command(id, "_bbsclan_clanlist");
            return;
        }
        let html = clan_home_html(
            clan_id,
            &clan.name,
            clan.level,
            clan.members_count(),
            &self.clan_leader_name(clan_id),
            clan.ally_name.as_deref().unwrap_or(""),
        );
        self.send_cb_html(id, &html);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(n: i32) -> Vec<ListRow> {
        (1..=n)
            .map(|i| ListRow {
                id: 100 + i,
                name: format!("Clan{i}"),
                leader: format!("Boss{i}"),
                level: 3,
                members: 5,
            })
            .collect()
    }

    #[test]
    fn list_page_one_shows_the_first_rows_and_a_blank_prev_button() {
        let html = clan_list_html(&rows(3), 1, 0);
        assert!(html.starts_with(LIST_HEAD_1));
        assert!(html.contains("bypass _bbsclan_clanhome;0\">[GO TO MY CLAN]"));
        for i in 1..=3 {
            assert!(html.contains(&format!("_bbsclan_clanhome;{}\">Clan{i}</a>", 100 + i)));
        }
        // One page: a blank prev and a blank next button around the single page number.
        assert!(html.contains("<td><button action=\"\" back=\"l2ui_ch3.prev1_down\""));
        assert!(html.contains("<td> 1 </td>"));
        assert!(html.contains("<td><button action=\"\" back=\"l2ui_ch3.next1_down\""));
        assert!(html.ends_with(LIST_TAIL));
    }

    #[test]
    fn list_pager_counts_pages_of_eight() {
        let html = clan_list_html(&rows(17), 2, 7);
        // 17 clans make 3 pages (17 / 8, rounded up).
        assert!(html.contains("<td><a action=\"bypass _bbsclan_clanlist;1\"> 1 </a></td>"));
        assert!(html.contains("<td> 2 </td>"));
        assert!(html.contains("<td><a action=\"bypass _bbsclan_clanlist;3\"> 3 </a></td>"));
        // The previous button has no `bypass ` prefix (Java), the next one has.
        assert!(html.contains("<td><button action=\"_bbsclan_clanlist;1\" back=\"l2ui_ch3.prev1_down\""));
        assert!(
            html.contains("<td><button action=\"bypass _bbsclan_clanlist;3\" back=\"l2ui_ch3.next1_down\"")
        );
    }

    #[test]
    fn list_window_follows_the_java_loop() {
        // Page 2 shows rows from index 7 on (`(index - 1) * 7`) up to the row after
        // `(index + 1) * 7 = 21` is passed: rows 8..=22 of 30, 15 rows.
        let html = clan_list_html(&rows(30), 2, 0);
        assert!(
            !html.contains(">Clan7</a>"),
            "row 7 (index 6) is before the window"
        );
        assert!(html.contains(">Clan8</a>"));
        assert!(html.contains(">Clan22</a>"));
        assert!(!html.contains(">Clan23</a>"));
    }

    #[test]
    fn home_page_has_the_clan_values() {
        let html = clan_home_html(9, "Alpha", 4, 12, "Boss", "Allies");
        assert!(html.contains("_bbsclan_clanhome;9;announce\">[CLAN ANNOUNCEMENT]"));
        assert!(html.contains("_bbsclan_clannotice_edit;9;cnotice\">[CLAN NOTICE]"));
        assert!(html.contains("CLAN NAME</td><td fixWIDTH=195 align=left>Alpha</td>"));
        assert!(html.contains("CLAN MEMBERS</td><td fixWIDTH=195 align=left height=16>12</td>"));
        assert!(html.contains("ALLIANCE</td><td fixWIDTH=195 align=left height=16>Allies</td>"));
    }

    #[test]
    fn notice_pages() {
        let on = notice_editor_html(9, true);
        assert!(on.contains(
            "on&nbsp;&nbsp;&nbsp;/&nbsp;&nbsp;&nbsp;<a action=\"bypass _bbsclan_clannotice_disable\">off</a>"
        ));
        let off = notice_editor_html(9, false);
        assert!(off.contains("<a action=\"bypass _bbsclan_clannotice_enable\">on</a>"));
        assert!(off.contains("<MultiEdit var =\"Content\" width=610 height=100>"));
        let view = notice_view_html(9, true, "Hello");
        assert!(view.contains("The current clan notice:"));
        assert!(view.contains("align=left>Hello</td>"));
        assert!(!notice_view_html(9, false, "Hello").contains("The current clan notice:"));
    }
}
