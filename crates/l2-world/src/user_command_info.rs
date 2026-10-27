//! The information user commands: `/time`, `/clanpenalty`, `/clanwarlist` (and its two siblings)
//! and `/siegestatus`.
// Java: dist/game/data/scripts/handlers/chat/commands/user/{Time,ClanPenalty,ClanWarsList,SiegeStatus}.java
// Java: network/clientpackets/BypassUserCmd.java (the dispatch, see `user_command.rs`)
//
// # Left out
// - `/mybirthday` (126): `MyBirthday` reads `Player.getCreateDate()` (`characters.createDate`),
//   which the character load does not carry yet.
// - `/unstuck` (52): the escape cast with the 5 minute `2099` skill, the Soul of Escape animation and
//   the timer that teleports to the town; the cast engine needs a player cast that stays
//   interruptible for that long (own card).
// - `/clanwarlist` reads the clan war sets the clan table keeps (`Clan.at_war_with` and
//   `at_war_attackers`, the same rows as `clan_wars`) instead of three joins over `clan_data` and
//   `clan_wars`: a war the Java game server declared after this server started shows after the
//   next start (D-50 would need a read request for a list that rarely changes).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::clanhall::siege::schedule::format_date_time;
use crate::world::{World, game_time_minutes};

/// User command 77 (`Time.java`).
pub const TIME_COMMAND: i32 = 77;
/// User command 88: the clans the player's clan declared war on (`ClanWarsList.java`).
pub const CLAN_WARS_ATTACK_COMMAND: i32 = 88;
/// User command 89: the clans that declared war on the player's clan.
pub const CLAN_WARS_UNDER_ATTACK_COMMAND: i32 = 89;
/// User command 90: the clans at war with the player's clan on both sides.
pub const CLAN_WARS_WAR_COMMAND: i32 = 90;
/// User command 99 (`SiegeStatus.java`).
pub const SIEGE_STATUS_COMMAND: i32 = 99;
/// User command 100 (`ClanPenalty.java`).
pub const CLAN_PENALTY_COMMAND: i32 = 100;

/// `ServerTimeConfig` as far as `/time` reads it. `Default` is the Java default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserCommandInfoSettings {
    /// `ServerTimeConfig.DISPLAY_SERVER_TIME` (`DisplayServerTime`).
    pub display_server_time: bool,
}

/// What the information user commands keep in the world.
#[derive(Debug, Default)]
pub struct UserCommandInfoWorld {
    pub(crate) settings: UserCommandInfoSettings,
}

/// `Time.java`: the game time as `h` and `m` (the minutes padded to two digits).
fn game_time_texts(game_minutes: i32) -> (String, String) {
    let hours = (game_minutes / 60) % 24;
    let minutes = game_minutes % 60;
    (hours.to_string(), format!("{minutes:02}"))
}

/// `Time.SDF` (`H:mm.`) of a local time of day in milliseconds.
fn server_time_text(local_ms: i64) -> String {
    let minutes = local_ms.rem_euclid(86_400_000) / 60_000;
    format!("{}:{:02}.", minutes / 60, minutes % 60)
}

/// `ClanPenalty.DATE_FORMAT` (`yyyy-MM-dd`) of an instant in the local zone.
fn penalty_date(epoch_ms: i64) -> String {
    let text = format_date_time(epoch_ms, l2_util::time::local_offset_millis(epoch_ms));
    text.get(..10).unwrap_or(&text).to_owned()
}

/// The page of `ClanPenalty`: one row per penalty that is still on, or the "no penalty" row.
fn clan_penalty_html(rows: &[(&str, i64)]) -> String {
    let mut html = String::from(
        "<html><body><center><table width=270 border=0 bgcolor=111111><tr><td width=170>Penalty</td><td width=100 align=center>Expiration Date</td></tr></table><table width=270 border=0><tr>",
    );
    for (text, expiry) in rows {
        html.push_str(&format!(
            "<td width=170>{text}</td><td width=100 align=center>{}</td>",
            penalty_date(*expiry)
        ));
    }
    if rows.is_empty() {
        html.push_str("<td width=170>No penalty is imposed.</td><td width=100 align=center></td>");
    }
    html.push_str("</tr></table><img src=\"L2UI.SquareWhite\" width=270 height=1></center></body></html>");
    html
}

impl World {
    /// Installs `DisplayServerTime`.
    pub fn set_user_command_info_settings(&mut self, settings: UserCommandInfoSettings) {
        self.user_command_info.settings = settings;
    }

    /// The commands of this module: `true` when `command` is one of them.
    pub(crate) fn user_command_info(&mut self, id: ObjectId, command: i32) -> bool {
        match command {
            TIME_COMMAND => self.user_command_time(id),
            CLAN_WARS_ATTACK_COMMAND | CLAN_WARS_UNDER_ATTACK_COMMAND | CLAN_WARS_WAR_COMMAND => {
                self.user_command_clan_wars(id, command);
            }
            SIEGE_STATUS_COMMAND => self.user_command_siege_status(id),
            CLAN_PENALTY_COMMAND => self.user_command_clan_penalty(id),
            _ => return false,
        }
        true
    }

    /// Java `Time.onCommand`.
    fn user_command_time(&mut self, id: ObjectId) {
        let (h, m) = game_time_texts(game_time_minutes(l2_util::time::now_millis()));
        let night = self.game_clock_night.unwrap_or(false);
        let message = SystemMessage::new(if night {
            SystemMessageId::TheCurrentTimeIsS1S22
        } else {
            SystemMessageId::TheCurrentTimeIsS1S2
        })
        .with(SystemMessageParam::Text(h))
        .with(SystemMessageParam::Text(m));
        self.send(id, &message);
        if self.user_command_info.settings.display_server_time {
            let text = server_time_text(l2_util::time::local_now_millis());
            self.send_message(id, &format!("Server time is {text}"));
        }
    }

    /// Java `ClanPenalty.onCommand`.
    fn user_command_clan_penalty(&mut self, id: ObjectId) {
        let now = l2_util::time::now_millis();
        let state = self.player_clan_state(id);
        let mut rows: Vec<(&str, i64)> = Vec::new();
        if state.clan_join_expiry_time > now {
            rows.push(("Unable to join a clan.", state.clan_join_expiry_time));
        }
        if state.clan_create_expiry_time > now {
            rows.push(("Unable to create a clan.", state.clan_create_expiry_time));
        }
        if let Some(clan) = self.clan_of(id)
            && clan.char_penalty_expiry_time > now
        {
            rows.push(("Unable to invite a clan member.", clan.char_penalty_expiry_time));
        }
        if let Some(message) = NpcHtmlMessage::new(0, &clan_penalty_html(&rows)) {
            self.send_html(id, &message);
        }
    }

    /// Java `ClanWarsList.onCommand`.
    fn user_command_clan_wars(&mut self, id: ObjectId, command: i32) {
        let Some(clan) = self.clan_of(id) else {
            self.send(id, &SystemMessage::new(SystemMessageId::NotJoinedInAnyClan));
            return;
        };
        let (declared, attackers) = (clan.at_war_with.clone(), clan.at_war_attackers.clone());
        let (header, list): (_, Vec<i32>) = match command {
            CLAN_WARS_ATTACK_COMMAND => (
                SystemMessageId::ClansYouVeDeclaredWarOn,
                declared.iter().copied().filter(|c| !attackers.contains(c)).collect(),
            ),
            CLAN_WARS_UNDER_ATTACK_COMMAND => (
                SystemMessageId::ClansThatHaveDeclaredWarOnYou,
                attackers.iter().copied().filter(|c| !declared.contains(c)).collect(),
            ),
            _ => (
                SystemMessageId::WarList,
                declared.iter().copied().filter(|c| attackers.contains(c)).collect(),
            ),
        };
        self.send(id, &SystemMessage::new(header));
        for other in list {
            // The joins only return clans that still exist in `clan_data`.
            let Some(other) = self.clan(other) else {
                continue;
            };
            let message = if other.ally_id > 0 {
                SystemMessage::new(SystemMessageId::S1S2Alliance)
                    .with(SystemMessageParam::Text(other.name.clone()))
                    .with(SystemMessageParam::Text(other.ally_name.clone().unwrap_or_default()))
            } else {
                SystemMessage::new(SystemMessageId::S1NoAllianceExists)
                    .with(SystemMessageParam::Text(other.name.clone()))
            };
            self.send(id, &message);
        }
        self.send(id, &SystemMessage::new(SystemMessageId::Empty3));
    }

    /// Java `SiegeStatus.onCommand`.
    fn user_command_siege_status(&mut self, id: ObjectId) {
        let refused = SystemMessage::new(
            SystemMessageId::OnlyAClanLeaderThatIsANoblesseCanViewTheSiegeWarStatusWindowDuringASiegeWar,
        );
        if !self.is_noble(id) || !self.is_clan_leader(id) {
            self.send(id, &refused);
            return;
        }
        let clan_id = self.clan_id_of(id);
        let castle = self
            .castle_sieges()
            .find(|siege| {
                siege.in_progress && (siege.is_attacker(Some(clan_id)) || siege.is_defender(Some(clan_id)))
            })
            .map(|siege| siege.castle_id);
        let Some(castle) = castle else {
            self.send(id, &refused);
            return;
        };
        let mut members = String::new();
        for member in self.clan_online_members(clan_id, 0) {
            let name = self.object_name(member).unwrap_or_default();
            let inside = self
                .position_of(member)
                .is_some_and(|(x, y, z)| self.castle_contains(castle, x, y, z));
            members.push_str(&format!(
                "<tr><td width=170>{name}</td><td width=100>{}</td></tr>",
                if inside {
                    "Castle Siege in Progress"
                } else {
                    "No Castle Siege Area"
                }
            ));
        }
        let mut html = self.admin_html_file(id, "data/html/siege/siege_status.htm");
        html.replace("%kill_count%", &self.clan_siege_kills(clan_id).to_string());
        html.replace("%death_count%", &self.clan_siege_deaths(clan_id).to_string());
        html.replace("%member_list%", &members);
        self.send_admin_html(id, &html);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_time_pads_the_minutes_only() {
        assert_eq!(game_time_texts(0), ("0".to_owned(), "00".to_owned()));
        assert_eq!(game_time_texts(7 * 60 + 5), ("7".to_owned(), "05".to_owned()));
        assert_eq!(game_time_texts(23 * 60 + 59), ("23".to_owned(), "59".to_owned()));
    }

    #[test]
    fn server_time_is_h_mm_dot() {
        assert_eq!(server_time_text(0), "0:00.");
        assert_eq!(server_time_text(9 * 3_600_000 + 7 * 60_000 + 59_000), "9:07.");
        assert_eq!(server_time_text(86_400_000 + 23 * 3_600_000 + 59 * 60_000), "23:59.");
    }

    #[test]
    fn penalty_page_is_the_java_string() {
        assert_eq!(
            clan_penalty_html(&[]),
            "<html><body><center><table width=270 border=0 bgcolor=111111><tr><td width=170>Penalty</td><td width=100 align=center>Expiration Date</td></tr></table><table width=270 border=0><tr><td width=170>No penalty is imposed.</td><td width=100 align=center></td></tr></table><img src=\"L2UI.SquareWhite\" width=270 height=1></center></body></html>"
        );
        let html = clan_penalty_html(&[("Unable to join a clan.", 1_793_108_709_000)]);
        assert!(html.contains("<td width=170>Unable to join a clan.</td><td width=100 align=center>2026-10-2"));
        assert!(!html.contains("No penalty is imposed."));
    }
}
