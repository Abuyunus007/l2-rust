//! The names and access levels the mail needs: Java `CharInfoTable.getIdByName`, `getNameById`
//! and `getAccessLevelById`, answered from the online players first and the startup table after.
// Java: data/sql/CharInfoTable.java

use l2_util::ObjectId;

use crate::world::World;

/// A row of `CharInfoTable`'s startup query (`SELECT charId, char_name, accesslevel FROM
/// characters`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailCharacter {
    pub id: i32,
    pub name: String,
    pub access_level: i32,
}

impl World {
    /// Java `CharInfoTable`'s constructor: the names and access levels of the characters in the
    /// database, and Java `MailManager`'s: the mails and their attachments. Call once at startup,
    /// before players enter.
    pub fn load_mail(&mut self, load: super::MailLoad) {
        for character in load.characters {
            self.mail_remember(character.id, &character.name, character.access_level);
        }
        let count = load.messages.len();
        for mut message in load.messages {
            // `Item.restoreFromDb` returns null for a template the datapack does not know.
            self.drop_unknown_items(&mut message.attachments);
            let id = message.id;
            if self.mail.messages.insert(id, message).is_none() {
                self.mail.order.push(id);
            }
        }
        tracing::info!("MailManager: Loaded {count} messages.");
        // `MessageDeletionTaskManager.getInstance()` runs from the first `add`.
        self.mail_start_deletion_task();
    }

    /// Replaces the values of `General.ini` / `FloodProtector.ini` the mail reads.
    pub fn set_mail_settings(&mut self, settings: super::MailSettings) {
        self.mail.settings = settings;
    }

    /// Java `CharInfoTable.addName(player)` and the startup rows.
    fn mail_remember(&mut self, id: i32, name: &str, access_level: i32) {
        if let Some((old, _)) = self.mail.names.get(&id) {
            let old = old.to_lowercase();
            self.mail.ids_by_name.remove(&old);
        }
        self.mail.names.insert(id, (name.to_owned(), access_level));
        self.mail.ids_by_name.insert(name.to_lowercase(), id);
    }

    /// Java `CharacterSelect`: `CharInfoTable.addName(cha)`. A character that was created after
    /// the server started is known from here on.
    pub(crate) fn mail_remember_player(&mut self, id: ObjectId) {
        let Some(p) = self.players.get(&id) else {
            return;
        };
        let (name, level) = (p.data.name.clone(), p.data.access_level);
        self.mail_remember(id.0, &name, level);
    }

    /// Java `CharInfoTable.getIdByName`: `-1` when nobody has the name.
    pub(crate) fn mail_char_id_by_name(&self, name: &str) -> i32 {
        if name.is_empty() {
            return -1;
        }
        if let Some(online) = self.player_by_name(name) {
            return online.0;
        }
        self.mail
            .ids_by_name
            .get(&name.to_lowercase())
            .copied()
            .unwrap_or(-1)
    }

    /// Java `CharInfoTable.getNameById`: `None` for an id below 1 or an unknown character.
    pub(crate) fn mail_char_name(&self, id: i32) -> Option<String> {
        if id <= 0 {
            return None;
        }
        if let Some(p) = self.players.get(&ObjectId(id)) {
            return Some(p.data.name.clone());
        }
        self.mail.names.get(&id).map(|(name, _)| name.clone())
    }

    /// Java `CharInfoTable.getAccessLevelById`: 0 for an unknown character.
    pub(crate) fn mail_char_access_level(&self, id: i32) -> i32 {
        if id <= 0 {
            return 0;
        }
        if let Some(p) = self.players.get(&ObjectId(id)) {
            return p.data.access_level;
        }
        self.mail.names.get(&id).map_or(0, |(_, level)| *level)
    }
}
