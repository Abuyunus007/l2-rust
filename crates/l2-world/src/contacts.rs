//! The mail contact list and the friend list of the mail window (L-04).
// Java: entity/actor/holders/player/ContactList.java
// Java: network/clientpackets/{RequestExShowContactList,RequestExAddContactToContactList,
// Java: RequestExDeleteContactFromContactList,RequestExFriendListExtended}.java
// Java: network/serverpackets/{ExShowContactList,ExConfirmAddingContact,FriendListExtended}.java
//
// # Where the contacts live
// `PlayerProfile::contacts`: the names of the contacts, in the order the session read the rows of
// `character_contacts` (Java `ContactList.restore`: a contact without a name, the character itself
// and unknown ids are skipped), then the order of the additions. Java keeps a `Set<String>` whose
// iteration order is not reproduced (NOTE(java-quirk)).
//
// # Database
// Java writes at once: `REPLACE INTO character_contacts` for an added contact and `DELETE` for a
// removed one. They go out as `PersistCommand::Contact` (D-15). Names are turned into ids with
// `World::char_id_by_name` (Java `CharInfoTable.getIdByName`).
//
// # The friend list of the mail window
// `FriendListExtended` shows class and level. For a friend who is in the world they come from the
// world; for one who is not, Java reads the `characters` row at once. That read goes out as
// `PersistCommand::FriendSummariesRequest` and comes back as `WorldCommand::FriendSummariesLoaded`
// (D-15); the packet is built from the friend list of that moment.
//
// Every request is ignored with `AllowMail = False`, like Java.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExConfirmAddingContact, ExShowContactList, FriendListExtended, FriendListExtendedEntry, SystemMessage,
    SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::debug;

use crate::outbox::PersistCommand;
use crate::world::World;

/// Java `ContactList.add`: the most contacts a character may have.
pub const MAX_CONTACTS: usize = 100;

/// A client request about the contact list, with the raw wire values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContactsInput {
    /// Java `RequestExShowContactList`.
    Show,
    /// Java `RequestExAddContactToContactList`.
    Add { name: String },
    /// Java `RequestExDeleteContactFromContactList`.
    Delete { name: String },
    /// Java `RequestExFriendListExtended`.
    FriendListExtended,
}

/// A `character_contacts` row change with Java's statement (`ContactList.QUERY_ADD`,
/// `QUERY_REMOVE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactWrite {
    /// `REPLACE INTO character_contacts (charId, contactId) VALUES (?, ?)`.
    Add { char_id: i32, contact_id: i32 },
    /// `DELETE FROM character_contacts WHERE charId = ? and contactId = ?`.
    Remove { char_id: i32, contact_id: i32 },
}

/// The `characters` row `FriendListExtended` shows of a friend who is not in the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendSummary {
    pub object_id: i32,
    pub name: String,
    /// `characters.online == 1`.
    pub online: bool,
    pub class_id: i32,
    pub level: i32,
}

impl World {
    // ------------------------------------------------------------------ the packets

    pub(crate) fn on_contacts_input(&mut self, id: ObjectId, input: ContactsInput) {
        if !self.mail.settings.allow_mail || !self.players.contains_key(&id) {
            return;
        }
        match input {
            ContactsInput::Show => self.send_contact_list(id),
            ContactsInput::Add { name } => {
                let added = self.contact_add(id, &name);
                self.send(id, &ExConfirmAddingContact { name, added });
            }
            ContactsInput::Delete { name } => self.contact_remove(id, &name),
            ContactsInput::FriendListExtended => self.request_friend_list_extended(id),
        }
    }

    /// Java `new ExShowContactList(player)`.
    pub(crate) fn contact_list_packet(&self, id: ObjectId) -> ExShowContactList {
        ExShowContactList {
            contacts: self
                .players
                .get(&id)
                .map(|p| p.data.profile.contacts.clone())
                .unwrap_or_default(),
        }
    }

    fn send_contact_list(&self, id: ObjectId) {
        self.send(id, &self.contact_list_packet(id));
    }

    // ------------------------------------------------------------------ the list

    /// Java `ContactList.add(name)`; `true` also when the INSERT failed, like Java.
    fn contact_add(&mut self, id: ObjectId, name: &str) -> bool {
        let Some(player) = self.players.get(&id) else {
            return false;
        };
        let contacts = &player.data.profile.contacts;
        let refuse = |world: &Self, message: SystemMessageId| {
            world.send(id, &SystemMessage::new(message));
            false
        };
        if contacts.iter().any(|c| c == name) {
            return refuse(self, SystemMessageId::TheNameAlreadyExistsOnTheAddedList);
        }
        if player.data.name == name {
            return refuse(self, SystemMessageId::YouCannotAddYourOwnName);
        }
        if contacts.len() >= MAX_CONTACTS {
            return refuse(
                self,
                SystemMessageId::TheMaximumNumberOfNames100HasBeenReachedYouCannotRegisterAnyMore,
            );
        }
        let contact_id = self.char_id_by_name(name).unwrap_or(-1);
        if contact_id < 1 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheNameS1DoesnTExistPleaseTryAnotherName)
                    .with(SystemMessageParam::Text(name.to_owned())),
            );
            return false;
        }
        // `equalsIgnoreCase`
        let lowered = name.to_lowercase();
        if contacts.iter().any(|c| c.to_lowercase() == lowered) {
            return refuse(self, SystemMessageId::TheNameAlreadyExistsOnTheAddedList);
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.contacts.push(name.to_owned());
        }
        self.outbox.persist(PersistCommand::Contact(ContactWrite::Add {
            char_id: id.0,
            contact_id,
        }));
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::S1WasSuccessfullyAddedToYourContactList)
                .with(SystemMessageParam::Text(name.to_owned())),
        );
        true
    }

    /// Java `ContactList.remove(name)`.
    fn contact_remove(&mut self, id: ObjectId, name: &str) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if !player.data.profile.contacts.iter().any(|c| c == name) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheNameIsNotCurrentlyRegistered),
            );
            return;
        }
        let contact_id = self.char_id_by_name(name).unwrap_or(-1);
        if contact_id < 1 {
            // "TODO: Message?"
            return;
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.contacts.retain(|c| c != name);
        }
        self.outbox.persist(PersistCommand::Contact(ContactWrite::Remove {
            char_id: id.0,
            contact_id,
        }));
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::S1WasSuccessfullyDeletedFromYourContactList)
                .with(SystemMessageParam::Text(name.to_owned())),
        );
    }

    // ------------------------------------------------------------------ the friend list

    /// The friends of a character, in list order.
    fn friend_ids_of(&self, id: ObjectId) -> Vec<i32> {
        self.friends
            .lists
            .get(&id)
            .map(|l| l.friends.iter().map(|f| f.object_id).collect())
            .unwrap_or_default()
    }

    /// Java `new FriendListExtended(player)`: asks for the rows of the friends who are not in the
    /// world, or answers at once when there are none.
    fn request_friend_list_extended(&mut self, id: ObjectId) {
        let offline: Vec<i32> = self
            .friend_ids_of(id)
            .into_iter()
            .filter(|friend| !self.players.contains_key(&ObjectId(*friend)))
            .collect();
        if offline.is_empty() {
            self.send_friend_list_extended(id, &[]);
        } else {
            self.outbox.persist(PersistCommand::FriendSummariesRequest {
                requester: id,
                friends: offline,
            });
        }
    }

    /// `WorldCommand::FriendSummariesLoaded`: the rows are in, so the packet can be built. A
    /// friend whose row could not be read is left out, like Java's empty `catch`.
    pub(crate) fn on_friend_summaries_loaded(&mut self, requester: ObjectId, rows: Vec<FriendSummary>) {
        if !self.players.contains_key(&requester) {
            return;
        }
        self.send_friend_list_extended(requester, &rows);
    }

    fn send_friend_list_extended(&self, id: ObjectId, rows: &[FriendSummary]) {
        let mut friends = Vec::new();
        for friend in self.friend_ids_of(id) {
            if let Some(online) = self.players.get(&ObjectId(friend)) {
                friends.push(FriendListExtendedEntry {
                    object_id: friend,
                    name: online.data.name.clone(),
                    online: true,
                    class_id: online.data.class_id,
                    level: online.data.level,
                });
            } else if let Some(row) = rows.iter().find(|r| r.object_id == friend) {
                friends.push(FriendListExtendedEntry {
                    object_id: friend,
                    name: row.name.clone(),
                    online: row.online,
                    class_id: row.class_id,
                    level: row.level,
                });
            } else {
                debug!("No characters row for friend {friend} of {id}; left out of the mail friend list.");
            }
        }
        self.send(id, &FriendListExtended { friends });
    }
}
