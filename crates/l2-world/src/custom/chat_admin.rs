//! `config/Custom/ChatModeration.ini`: the voiced commands `.banchat` and `.unbanchat` of the
//! moderators (`ChatAdmin`, on by default in Java).
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/ChatAdmin.java

use l2_util::ObjectId;

use super::misc::tokens;
use crate::punishments::{NewPunishment, PunishmentAffect, PunishmentType};
use crate::world::{World, unix_ms_now};

/// `StringUtil.isNumeric`: not empty, only digits.
// NOTE: Java's `Character.isDigit` also accepts the digits of other scripts; only ASCII is read
// here.
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

impl World {
    /// `AdminData.hasAccess(command, level)` for a player.
    fn chat_admin_has_access(&self, command: &str, level: i32) -> bool {
        self.data
            .admin_rights()
            .is_some_and(|rights| rights.has_access(command, level))
    }

    /// `ChatAdmin.onCommand`. The result is dropped by the chat (the text was taken either way).
    pub(crate) fn chat_admin_voiced(
        &mut self,
        player: ObjectId,
        command: &str,
        params: Option<&str>,
    ) -> bool {
        let Some(level) = self.players.get(&player).map(|p| p.data.access_level) else {
            return false;
        };
        if !self.chat_admin_has_access(command, level) {
            return false;
        }
        match command {
            "banchat" | "chatban" => self.chat_admin_ban(player, command, params),
            "unbanchat" | "chatunban" => self.chat_admin_unban(player, params),
            // The handler is only found for the four words; another `command` (a trailing
            // space) falls out of Java's `switch` and answers `true`.
            _ => true,
        }
    }

    fn chat_admin_ban(&mut self, player: ObjectId, command: &str, params: Option<&str>) -> bool {
        let Some(params) = params else {
            self.send_sys_message(player, "Usage: .banchat name [minutes]");
            return true;
        };
        let parts = tokens(params);
        let Some(name) = parts.first() else {
            return true;
        };
        let mut expiration_minutes: i64 = 0;
        if let Some(token) = parts.get(1)
            && is_numeric(token)
        {
            // NOTE(java-quirk): `Integer.parseInt` throws for a number above `Integer.MAX_VALUE`;
            // the exception leaves the handler without a word to the moderator.
            match token.parse::<i32>() {
                Ok(minutes) => expiration_minutes = i64::from(minutes),
                Err(_) => return true,
            }
        }

        let Some(obj_id) = self.char_id_by_name(name).filter(|id| *id > 0) else {
            self.send_sys_message(player, "Player not found!");
            return false;
        };
        let target = ObjectId(obj_id);
        let Some((target_name, target_level)) = self
            .players
            .get(&target)
            .map(|p| (p.data.name.clone(), p.data.access_level))
        else {
            self.send_sys_message(player, "Player not online!");
            return false;
        };
        if self.is_chat_banned(target) {
            self.send_sys_message(player, "Player is already punished!");
            return false;
        }
        if target == player {
            self.send_sys_message(player, "You can't ban yourself!");
            return false;
        }
        if self.is_gm(target) {
            self.send_sys_message(player, "You can't ban a GM!");
            return false;
        }
        if self.chat_admin_has_access(command, target_level) {
            self.send_sys_message(player, "You can't ban moderator!");
            return false;
        }

        let moderator = self.chat_admin_name(player);
        // NOTE(java-quirk): without minutes the expiration is "now", not 0 (forever): the task
        // is made with `currentTimeMillis() + 0`, which the punishment manager treats as a
        // time that has just passed.
        let expiration = unix_ms_now().wrapping_add(expiration_minutes.wrapping_mul(1000 * 60));
        self.start_punishment(NewPunishment::new(
            obj_id,
            PunishmentAffect::Character,
            PunishmentType::ChatBan,
            expiration,
            "Chat banned by moderator",
            &moderator,
        ));
        if expiration_minutes > 0 {
            self.send_sys_message(
                player,
                &format!("Player {target_name} chat banned for {expiration_minutes} minutes."),
            );
        } else {
            self.send_sys_message(player, &format!("Player {target_name} chat banned forever."));
        }
        self.send_message(target, &format!("Chat banned by moderator {moderator}"));
        true
    }

    fn chat_admin_unban(&mut self, player: ObjectId, params: Option<&str>) -> bool {
        let Some(params) = params else {
            self.send_sys_message(player, "Usage: .unbanchat name");
            return true;
        };
        let parts = tokens(params);
        let Some(name) = parts.first() else {
            return true;
        };
        let Some(obj_id) = self.char_id_by_name(name).filter(|id| *id > 0) else {
            self.send_sys_message(player, "Player not found!");
            return false;
        };
        let target = ObjectId(obj_id);
        let Some(target_name) = self.players.get(&target).map(|p| p.data.name.clone()) else {
            self.send_sys_message(player, "Player not online!");
            return false;
        };
        if !self.is_chat_banned(target) {
            self.send_sys_message(player, "Player is not chat banned!");
            return false;
        }
        self.stop_punishment(
            &obj_id.to_string(),
            PunishmentAffect::Character,
            PunishmentType::ChatBan,
        );
        self.send_sys_message(player, &format!("Player {target_name} chat unbanned."));
        let moderator = self.chat_admin_name(player);
        self.send_message(target, &format!("Chat unbanned by moderator {moderator}"));
        true
    }

    fn chat_admin_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }
}
