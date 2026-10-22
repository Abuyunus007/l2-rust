//! The friend list and the block list (M6-FRIEND-01): the friend requests, the friend list
//! window, the friend status notices, the private messages between friends, the `/block`
//! commands and the message refusal mode.
// Java: entity/actor/holders/player/BlockList.java
// Java: network/clientpackets/{RequestFriendInvite,RequestAnswerFriendInvite,RequestFriendDel,
// Java: RequestFriendList,RequestSendFriendMsg,RequestBlock}.java
// Java: entity/actor/Player.java (restoreFriendList, notifyFriends, get/setMessageRefusal)
// Java: network/clientpackets/EnterWorld.java (FriendList, YOUR_FRIEND_S1_JUST_LOGGED_IN)
//
// # Shape
// The lists of every online character are in `FriendWorld`, keyed by the character's object id
// (D-11): the other characters are ids, with the name copied when the list is loaded. The rows
// come from the session at enter world (`PlayerProfile::friends` and `block_list`), so this module
// never reads the database. Every row change goes out as `PersistCommand::Friend` in the order the
// world makes it, with Java's statement (`character_friends`, see `FriendWrite`).
//
// # Left out, with the card that brings it
// - The olympiad rule of `RequestFriendInvite` (`isInOlympiadMode`): M8.
// - The chat log line of `GeneralConfig.LOG_CHAT` in `RequestSendFriendMsg`: the world has no
//   chat log yet.
// - `BlockList.isInBlockList(ownerId, targetId)` for an owner that is offline (Java loads its rows
//   from the database): `is_blocked` answers for an online owner only. The mail card reads rows.
// - The GM check of `RequestBlock` (`CharInfoTable.getAccessLevelById`) for an offline character:
//   the world keeps no access levels of characters that are not online.
// - The `EX_REQUEST_EX_FRIEND_LIST_EXTENDED` packet (the friend list with class and level): not
//   part of this card.

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::enterworld::{FriendList, FriendListEntry};
use l2_protocol_game::server::{
    FriendAddRequest, FriendPacket, FriendStatusPacket, L2FriendSay, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::info;

use crate::outbox::PersistCommand;
use crate::world::World;

/// Java `RequestBlock` types.
const BLOCK: i32 = 0;
const UNBLOCK: i32 = 1;
const BLOCKLIST: i32 = 2;
const ALLBLOCK: i32 = 3;
const ALLUNBLOCK: i32 = 4;

/// Java `RequestSendFriendMsg`: the longest message, in UTF-16 code units.
const MAX_FRIEND_MESSAGE_UNITS: usize = 300;

/// One row of a friend or a block list: the other character and its name, `None` when no
/// character has the id (Java's `CharInfoTable` has no name for it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendEntry {
    pub object_id: i32,
    pub name: Option<String>,
}

/// A `character_friends` row change for the persistence task, with Java's statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FriendWrite {
    /// `RequestAnswerFriendInvite`: `INSERT INTO character_friends (charId, friendId) VALUES
    /// (first, second), (second, first)`.
    AddFriends { first: ObjectId, second: ObjectId },
    /// `RequestFriendDel`: both directions, whatever their relation.
    RemoveFriends { first: ObjectId, second: ObjectId },
    /// `BlockList.updateInDB(target, true)`: `relation` 1.
    AddBlock { owner: ObjectId, target: ObjectId },
    /// `BlockList.updateInDB(target, false)`.
    RemoveBlock { owner: ObjectId, target: ObjectId },
}

/// The lists of one online character: Java `Player._friendList`, `BlockList._blockList` and
/// `Player._messageRefusal`.
#[derive(Debug, Default)]
pub(crate) struct SocialLists {
    /// In the order the rows were read, then the order of the changes made since.
    // NOTE(java-quirk): Java keeps these in a `ConcurrentHashMap` key set, whose iteration order is
    // not reproduced; the order here is the load order.
    pub(crate) friends: Vec<FriendEntry>,
    pub(crate) blocked: Vec<FriendEntry>,
    /// Java `_messageRefusal`: set by `/allblock`, never stored (a new login starts with false).
    pub(crate) message_refusal: bool,
}

impl SocialLists {
    fn has_friend(&self, id: ObjectId) -> bool {
        self.friends.iter().any(|e| e.object_id == id.0)
    }

    fn has_blocked(&self, id: ObjectId) -> bool {
        self.blocked.iter().any(|e| e.object_id == id.0)
    }

    /// The name this list holds for `id`, friend or blocked.
    fn name_of(&self, id: ObjectId) -> Option<String> {
        self.friends
            .iter()
            .chain(self.blocked.iter())
            .find(|e| e.object_id == id.0)
            .and_then(|e| e.name.clone())
    }
}

/// The lists of every online character.
#[derive(Debug, Default)]
pub struct FriendWorld {
    pub(crate) lists: HashMap<ObjectId, SocialLists>,
}

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

impl World {
    // ------------------------------------------------------------------ lifecycle

    /// Installs the lists a character loaded (`PlayerProfile::friends` and `block_list`). Called
    /// when the character enters the world, like Java's `Player.restore`.
    pub(crate) fn load_social_lists(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let friends = std::mem::take(&mut player.data.profile.friends);
        let blocked = std::mem::take(&mut player.data.profile.block_list);
        self.friends.lists.insert(
            id,
            SocialLists {
                friends,
                blocked,
                message_refusal: false,
            },
        );
    }

    /// Drops the lists of a character that left (the object goes with its state).
    pub(crate) fn forget_social_lists(&mut self, id: ObjectId) {
        self.friends.lists.remove(&id);
    }

    // ------------------------------------------------------------------ queries

    /// Java `BlockList.isBlocked(listOwner, target)`: the owner refuses all messages, or has the
    /// target on its block list. An owner that is not online blocks nobody here (see the module
    /// comment).
    pub fn is_blocked(&self, owner: ObjectId, target: ObjectId) -> bool {
        self.friends
            .lists
            .get(&owner)
            .is_some_and(|l| l.message_refusal || l.has_blocked(target))
    }

    /// Java `Player.getMessageRefusal()`.
    pub fn message_refusal(&self, id: ObjectId) -> bool {
        self.friends.lists.get(&id).is_some_and(|l| l.message_refusal)
    }

    /// Java `Player.getFriendList().contains(friend)` of `owner`.
    pub fn is_friend_of(&self, owner: ObjectId, friend: ObjectId) -> bool {
        self.friends
            .lists
            .get(&owner)
            .is_some_and(|l| l.has_friend(friend))
    }

    /// Java `CharInfoTable.getNameById`: the name of an online character, else the name the lists
    /// of `owner` hold for it.
    fn name_of_character(&self, owner: ObjectId, id: ObjectId) -> Option<String> {
        if let Some(player) = self.players.get(&id) {
            return Some(player.data.name.clone());
        }
        self.friends.lists.get(&owner).and_then(|l| l.name_of(id))
    }

    /// The name of a character as the packets write it: `name_of_character`, else `fallback`
    /// (the name the packet carried).
    fn name_or(&self, owner: ObjectId, id: ObjectId, fallback: &str) -> String {
        self.name_of_character(owner, id)
            .unwrap_or_else(|| fallback.to_owned())
    }

    /// Java `Player.isInvisible()` (a GM's invisibility).
    fn is_invisible(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|e| e.invisible)
    }

    /// The name of an online character, empty when it is not online.
    fn own_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// The access level of a character, for the GM check of `RequestBlock`. Only online characters
    /// have one here; an offline character reads as a user (see the module comment).
    fn access_level_of(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.data.access_level)
    }

    fn persist_friend(&mut self, write: FriendWrite) {
        self.outbox.persist(PersistCommand::Friend(write));
    }

    // ------------------------------------------------------------------ enter and leave

    /// Java `EnterWorld.runImpl`: `FriendList(player)`, the packet the friends' window reads.
    pub(crate) fn friend_list_packet(&self, id: ObjectId) -> FriendList {
        let Some(lists) = self.friends.lists.get(&id) else {
            return FriendList::default();
        };
        // NOTE(java-quirk): Java writes a friend whose name is unknown with a null name (the packet
        // would not encode it); the friend is left out here instead.
        let friends = lists
            .friends
            .iter()
            .filter_map(|entry| {
                let name = self.name_of_character(id, ObjectId(entry.object_id))?;
                Some(FriendListEntry {
                    object_id: entry.object_id,
                    name,
                    online: self.players.contains_key(&ObjectId(entry.object_id)),
                })
            })
            .collect();
        FriendList { friends }
    }

    /// Java `EnterWorld.runImpl`: `YOUR_FRIEND_S1_JUST_LOGGED_IN` to each friend who is online.
    pub(crate) fn friends_announce_login(&self, id: ObjectId) {
        let name = self.own_name(id);
        for friend in self.online_friends_of(id) {
            self.send(
                friend,
                &SystemMessage::new(SystemMessageId::YourFriendS1JustLoggedIn).with(text(&name)),
            );
        }
    }

    /// Java `Player.notifyFriends()` at login: the status of the character goes to each friend
    /// who is online.
    pub(crate) fn friends_notify_online(&self, id: ObjectId) {
        let name = self.own_name(id);
        for friend in self.online_friends_of(id) {
            self.send(
                friend,
                &FriendStatusPacket {
                    online: true,
                    name: name.clone(),
                    object_id: id.0,
                },
            );
        }
    }

    /// Java `Player.deleteMe` -> `notifyFriends()`: the friends who are online see the character
    /// go offline. Called after the character left the world tables, so it is not online here.
    pub(crate) fn friends_notify_offline(&self, id: ObjectId, name: &str) {
        for friend in self.online_friends_of(id) {
            self.send(
                friend,
                &FriendStatusPacket {
                    online: false,
                    name: name.to_owned(),
                    object_id: id.0,
                },
            );
        }
    }

    /// The friends of `id` who are online, in list order.
    fn online_friends_of(&self, id: ObjectId) -> Vec<ObjectId> {
        self.friends
            .lists
            .get(&id)
            .map(|l| {
                l.friends
                    .iter()
                    .map(|e| ObjectId(e.object_id))
                    .filter(|f| self.players.contains_key(f))
                    .collect()
            })
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------ friends

    /// Java `RequestFriendInvite.runImpl`.
    pub(crate) fn on_request_friend_invite(&mut self, id: ObjectId, name: &str) {
        if !self.players.contains_key(&id) {
            return;
        }
        // `World.getPlayer(_name)`: an online character, and not invisible.
        let friend = self.player_by_name(name).filter(|f| !self.is_invisible(*f));
        let Some(friend) = friend else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheUserWhoRequestedToBecomeFriendsIsNotFoundInTheGame),
            );
            return;
        };
        if friend == id {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotAddYourselfToYourOwnFriendList),
            );
            return;
        }
        // Java: `player.isInOlympiadMode() || friend.isInOlympiadMode()`: the olympiad is M8.
        // The friend blocks the player: Java tells the player only that it is in the target's
        // block list.
        if self.is_blocked(friend, id) {
            self.send_message(id, "You are in target's block list.");
            return;
        }
        if self.is_blocked(id, friend) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveBlockedC1).with(text(&self.own_name(friend))),
            );
            return;
        }
        if self.is_friend_of(id, friend) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThisPlayerIsAlreadyRegisteredOnYourFriendsList),
            );
            return;
        }
        // Java `friend.isProcessingRequest()`: the name in the message is the one typed.
        if self.is_processing_request(friend) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater).with(text(name)),
            );
            return;
        }
        self.on_transaction_request(id, friend);
        let requestor_name = self.own_name(id);
        self.send(friend, &FriendAddRequest { requestor_name });
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouVeRequestedC1ToBeOnYourFriendsList).with(text(name)),
        );
    }

    /// Java `RequestAnswerFriendInvite.runImpl`.
    pub(crate) fn on_request_answer_friend_invite(&mut self, id: ObjectId, response: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        let Some(requestor) = self.active_requester(id) else {
            return;
        };
        if self.is_friend_of(id, requestor) || self.is_friend_of(requestor, id) {
            // Java answers the requestor, and leaves the request open.
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::ThisPlayerIsAlreadyRegisteredOnYourFriendsList),
            );
            return;
        }
        if response == 1 {
            let own_name = self.own_name(id);
            let requestor_name = self.name_or(id, requestor, "");
            let requestor_online = self.players.contains_key(&requestor);
            self.persist_friend(FriendWrite::AddFriends {
                first: requestor,
                second: id,
            });
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::ThatPersonHasBeenSuccessfullyAddedToYourFriendList),
            );
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::S1HasBeenAddedToYourFriendsList).with(text(&own_name)),
            );
            if let Some(lists) = self.friends.lists.get_mut(&requestor) {
                lists.friends.push(FriendEntry {
                    object_id: id.0,
                    name: Some(own_name.clone()),
                });
            }
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::S1HasJoinedAsAFriend).with(text(&requestor_name)),
            );
            if let Some(lists) = self.friends.lists.get_mut(&id) {
                lists.friends.push(FriendEntry {
                    object_id: requestor.0,
                    name: Some(requestor_name.clone()),
                });
            }
            self.send(
                id,
                &FriendPacket {
                    added: true,
                    object_id: requestor.0,
                    name: requestor_name,
                    online: requestor_online,
                },
            );
            self.send(
                requestor,
                &FriendPacket {
                    added: true,
                    object_id: id.0,
                    name: own_name,
                    online: true,
                },
            );
        } else {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::YouHaveFailedToAddAFriendToYourFriendsList),
            );
        }
        self.set_active_requester(id, None);
        self.on_transaction_response(requestor);
    }

    /// Java `RequestFriendList.runImpl`.
    pub(crate) fn on_request_friend_list(&self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        self.send(id, &SystemMessage::new(SystemMessageId::FriendsList));
        let friends: Vec<ObjectId> = self
            .friends
            .lists
            .get(&id)
            .map(|l| l.friends.iter().map(|e| ObjectId(e.object_id)).collect())
            .unwrap_or_default();
        for friend in friends {
            // Java skips a friend whose name is unknown.
            let Some(name) = self.name_of_character(id, friend) else {
                continue;
            };
            let id_message = if self.players.contains_key(&friend) {
                SystemMessageId::S1CurrentlyOnline
            } else {
                SystemMessageId::S1CurrentlyOffline
            };
            self.send(id, &SystemMessage::new(id_message).with(text(&name)));
        }
        self.send(id, &SystemMessage::new(SystemMessageId::Empty3));
    }

    /// Java `RequestFriendDel.runImpl`.
    pub(crate) fn on_request_friend_del(&mut self, id: ObjectId, name: &str) {
        if !self.players.contains_key(&id) {
            return;
        }
        // `CharInfoTable.getIdByName`: online characters first, then the names of the database.
        let target = self
            .char_id_by_name(name)
            .map(ObjectId)
            .filter(|t| self.is_friend_of(id, *t));
        let Some(target) = target else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1IsNotOnYourFriendList).with(text(name)),
            );
            return;
        };
        self.persist_friend(FriendWrite::RemoveFriends {
            first: id,
            second: target,
        });
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::S1HasBeenDeletedFromYourFriendsList).with(text(name)),
        );
        if let Some(lists) = self.friends.lists.get_mut(&id) {
            lists.friends.retain(|e| e.object_id != target.0);
        }
        let target_name = self.name_or(id, target, name);
        self.send(
            id,
            &FriendPacket {
                added: false,
                object_id: target.0,
                name: target_name,
                online: self.players.contains_key(&target),
            },
        );
        // Java: `World.getPlayer(_name)`: the other side hears about it when it is online.
        if self.players.contains_key(&target) {
            if let Some(lists) = self.friends.lists.get_mut(&target) {
                lists.friends.retain(|e| e.object_id != id.0);
            }
            let own_name = self.own_name(id);
            self.send(
                target,
                &FriendPacket {
                    added: false,
                    object_id: id.0,
                    name: own_name,
                    online: true,
                },
            );
        }
    }

    /// Java `RequestSendFriendMsg.runImpl`.
    pub(crate) fn on_request_send_friend_msg(&mut self, id: ObjectId, message: &str, receiver: &str) {
        if !self.players.contains_key(&id) {
            return;
        }
        if message.is_empty() || message.encode_utf16().count() > MAX_FRIEND_MESSAGE_UNITS {
            return;
        }
        // `World.getPlayer(_receiver)`, and the receiver must have the sender as a friend.
        let target = self
            .player_by_name(receiver)
            .filter(|t| self.is_friend_of(*t, id));
        let Some(target) = target else {
            self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            return;
        };
        let sender = self.own_name(id);
        self.send(
            target,
            &L2FriendSay {
                sender,
                receiver: receiver.to_owned(),
                message: message.to_owned(),
            },
        );
    }

    // ------------------------------------------------------------------ block list

    /// Java `RequestBlock.runImpl`.
    pub(crate) fn on_request_block(&mut self, id: ObjectId, kind: i32, name: Option<&str>) {
        if !self.players.contains_key(&id) {
            return;
        }
        match kind {
            BLOCK | UNBLOCK => {
                let Some(name) = name else {
                    return;
                };
                // `CharInfoTable.getIdByName`: a name that is not found gives -1.
                let Some(target) = self.char_id_by_name(name).filter(|t| *t > 0).map(ObjectId) else {
                    self.send(
                        id,
                        &SystemMessage::new(SystemMessageId::YouHaveFailedToRegisterTheUserToYourIgnoreList),
                    );
                    return;
                };
                if self.access_level_of(target) > 0 {
                    self.send(
                        id,
                        &SystemMessage::new(SystemMessageId::YouMayNotImposeABlockOnAGm),
                    );
                    return;
                }
                if target == id {
                    return;
                }
                if kind == BLOCK {
                    self.add_to_block_list(id, target, name);
                } else {
                    self.remove_from_block_list(id, target, name);
                }
            }
            BLOCKLIST => self.send_block_list_to_owner(id),
            ALLBLOCK => {
                self.send(id, &SystemMessage::new(SystemMessageId::MessageRefusalMode));
                self.set_message_refusal(id, true);
            }
            ALLUNBLOCK => {
                self.send(id, &SystemMessage::new(SystemMessageId::MessageAcceptanceMode));
                self.set_message_refusal(id, false);
            }
            other => info!("Unknown 0xA9 block type: {other}"),
        }
    }

    /// Java `Player.setMessageRefusal`: the flag, then `EtcStatusUpdate`.
    fn set_message_refusal(&mut self, id: ObjectId, value: bool) {
        if let Some(lists) = self.friends.lists.get_mut(&id) {
            lists.message_refusal = value;
        }
        self.send_etc_status_update(id);
    }

    /// Java `BlockList.addToBlockList(listOwner, targetId)`.
    fn add_to_block_list(&mut self, owner: ObjectId, target: ObjectId, typed_name: &str) {
        let is_friend = self.is_friend_of(owner, target);
        let already = self
            .friends
            .lists
            .get(&owner)
            .is_some_and(|l| l.has_blocked(target));
        if is_friend {
            self.send(
                owner,
                &SystemMessage::new(SystemMessageId::ThisPlayerIsAlreadyRegisteredOnYourFriendsList),
            );
            return;
        }
        if already {
            self.send_message(owner, "Already in ignore list.");
            return;
        }
        let char_name = self.name_or(owner, target, typed_name);
        if let Some(lists) = self.friends.lists.get_mut(&owner) {
            lists.blocked.push(FriendEntry {
                object_id: target.0,
                name: Some(char_name.clone()),
            });
        }
        self.persist_friend(FriendWrite::AddBlock { owner, target });
        self.send(
            owner,
            &SystemMessage::new(SystemMessageId::S1HasBeenAddedToYourIgnoreList).with(text(&char_name)),
        );
        if self.players.contains_key(&target) {
            let owner_name = self.own_name(owner);
            self.send(
                target,
                &SystemMessage::new(SystemMessageId::S1HasPlacedYouOnHisHerIgnoreList)
                    .with(text(&owner_name)),
            );
        }
    }

    /// Java `BlockList.removeFromBlockList(listOwner, targetId)`.
    fn remove_from_block_list(&mut self, owner: ObjectId, target: ObjectId, typed_name: &str) {
        let listed = self
            .friends
            .lists
            .get(&owner)
            .is_some_and(|l| l.has_blocked(target));
        if !listed {
            self.send(
                owner,
                &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget),
            );
            return;
        }
        if let Some(lists) = self.friends.lists.get_mut(&owner) {
            lists.blocked.retain(|e| e.object_id != target.0);
        }
        self.persist_friend(FriendWrite::RemoveBlock { owner, target });
        let char_name = self.name_or(owner, target, typed_name);
        self.send(
            owner,
            &SystemMessage::new(SystemMessageId::S1HasBeenRemovedFromYourIgnoreList).with(text(&char_name)),
        );
    }

    /// Java `BlockList.sendListToOwner(listOwner)`.
    fn send_block_list_to_owner(&self, owner: ObjectId) {
        self.send(owner, &SystemMessage::new(SystemMessageId::IgnoreList));
        let blocked: Vec<Option<String>> = self
            .friends
            .lists
            .get(&owner)
            .map(|l| {
                l.blocked
                    .iter()
                    .map(|e| self.name_of_character(owner, ObjectId(e.object_id)))
                    .collect()
            })
            .unwrap_or_default();
        for (index, name) in blocked.iter().enumerate() {
            // Java concatenates a null name as "null".
            let name = name.as_deref().unwrap_or("null");
            self.send_message(owner, &format!("{}. {name}", index + 1));
        }
        self.send(owner, &SystemMessage::new(SystemMessageId::Empty3));
    }
}
