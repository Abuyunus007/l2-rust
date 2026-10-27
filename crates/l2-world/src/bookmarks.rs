//! Teleport bookmarks ("My Teleports"): the list, saving the current location, changing and
//! deleting an entry, and teleporting to one (L-04).
// Java: network/clientpackets/{RequestBookMarkSlotInfo,RequestSaveBookMarkSlot,
// Java: RequestModifyBookMarkSlot,RequestDeleteBookMarkSlot,RequestTeleportBookMark,
// Java: RequestChangeBookMarkSlot}.java
// Java: network/serverpackets/ExGetBookMarkInfoPacket.java
// Java: entity/actor/holders/player/TeleportBookmark.java
// Java: entity/actor/Player.java (teleportBookmarkAdd/Modify/Delete/Go/Condition,
// Java: restoreTeleportBookmark)
//
// # Where a bookmark lives
// `PlayerProfile::bookmarks`: the rows of `character_tpbookmark` the session read at
// `CharacterSelect` (Java `restoreTeleportBookmark`), kept sorted by id (Java keeps a
// `ConcurrentHashMap<Integer, TeleportBookmark>`, whose small int keys iterate in ascending
// order). `PlayerProfile::bookmark_slot` is how many a character may have (`characters.bookmarkslot`).
//
// # Database
// Java writes at once: an INSERT when a bookmark is saved, an UPDATE when it is changed and a
// DELETE when it is removed. They go out as `PersistCommand::Bookmark` (D-15). A full character
// store does not touch the table, like Java.
//
// # Left out
// - `_inOlympiadMode`, `_isIn7sDungeon` and the dimensional rift of the party (M8: the Olympiad,
//   the Seven Signs dungeon and the rift are not in the world yet), and `isInBoat()` /
//   `isInAirShip()` (no boats, see `actions.rs`).
// - `ZoneId::NoBookmark` is read like every zone; the zone files decide who has it.
// - The instance of the player after a bookmark teleport: see `teleport_bookmark_go`.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::enterworld::TeleportBookmark;
use l2_protocol_game::server::{ExGetBookMarkInfoPacket, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL};
use crate::teleport::TeleportTarget;
use crate::world::World;
use crate::zones::ZoneId;

/// Java `Player.teleportBookmarkGo`: the item that is used up by a bookmark teleport (a "My
/// Teleport Scroll").
pub const TELEPORT_ITEM_ID: i32 = 13016;

/// The default `BookmarkConsumeItemId` that is the "My Teleport Flag" (Java compares with 20033
/// to pick the message).
pub const TELEPORT_FLAG_ITEM_ID: i32 = 20033;

/// One teleport bookmark: Java `TeleportBookmark`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkData {
    /// `Id`: the slot number, 1 and up.
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub icon: i32,
    /// `tag`; a NULL in the table is an empty tag here.
    pub tag: String,
    pub name: String,
}

/// A client request about the bookmarks, with the raw wire values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookmarkInput {
    /// Java `RequestBookMarkSlotInfo`.
    SlotInfo,
    /// Java `RequestSaveBookMarkSlot`.
    Save { name: String, icon: i32, tag: String },
    /// Java `RequestModifyBookMarkSlot`.
    Modify {
        id: i32,
        name: String,
        icon: i32,
        tag: String,
    },
    /// Java `RequestDeleteBookMarkSlot`.
    Delete { id: i32 },
    /// Java `RequestTeleportBookMark`.
    Teleport { id: i32 },
    /// Java `RequestChangeBookMarkSlot`: reads nothing and does nothing.
    ChangeSlot,
}

/// A `character_tpbookmark` row change with Java's statement (`Player.INSERT_TP_BOOKMARK`,
/// `UPDATE_TP_BOOKMARK`, `DELETE_TP_BOOKMARK`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookmarkWrite {
    /// `INSERT INTO character_tpbookmark (charId,Id,x,y,z,icon,tag,name) values (?,?,?,?,?,?,?,?)`.
    Add { char_id: i32, bookmark: BookmarkData },
    /// `UPDATE character_tpbookmark SET icon=?,tag=?,name=? where charId=? AND Id=?`.
    Update {
        char_id: i32,
        id: i32,
        icon: i32,
        tag: String,
        name: String,
    },
    /// `DELETE FROM character_tpbookmark WHERE charId=? AND Id=?`.
    Delete { char_id: i32, id: i32 },
}

/// `Teleport type` of Java's `teleportBookmarkCondition(int type)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConditionType {
    /// 0: using a bookmark.
    Go,
    /// 1: saving one.
    Save,
}

impl World {
    // ------------------------------------------------------------------ the list

    /// Java `new ExGetBookMarkInfoPacket(player)`.
    pub(crate) fn bookmark_info_packet(&self, id: ObjectId) -> ExGetBookMarkInfoPacket {
        let Some(player) = self.players.get(&id) else {
            return ExGetBookMarkInfoPacket::default();
        };
        ExGetBookMarkInfoPacket {
            slot: player.data.profile.bookmark_slot,
            bookmarks: player
                .data
                .profile
                .bookmarks
                .iter()
                .map(|b| TeleportBookmark {
                    id: b.id,
                    x: b.x,
                    y: b.y,
                    z: b.z,
                    name: b.name.clone(),
                    icon: b.icon,
                    tag: b.tag.clone(),
                })
                .collect(),
        }
    }

    fn send_bookmark_info(&self, id: ObjectId) {
        self.send(id, &self.bookmark_info_packet(id));
    }

    /// The bookmark of a character, cloned.
    fn bookmark_of(&self, id: ObjectId, bookmark_id: i32) -> Option<BookmarkData> {
        self.players
            .get(&id)?
            .data
            .profile
            .bookmarks
            .iter()
            .find(|b| b.id == bookmark_id)
            .cloned()
    }

    // ------------------------------------------------------------------ the packets

    pub(crate) fn on_bookmark_input(&mut self, id: ObjectId, input: BookmarkInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            // `player.sendPacket(new ExGetBookMarkInfoPacket(player))`
            BookmarkInput::SlotInfo => self.send_bookmark_info(id),
            BookmarkInput::Save { name, icon, tag } => {
                let Some((x, y, z)) = self.position_of(id) else {
                    return;
                };
                self.teleport_bookmark_add(id, x, y, z, icon, tag, name);
            }
            BookmarkInput::Modify {
                id: slot,
                name,
                icon,
                tag,
            } => {
                self.teleport_bookmark_modify(id, slot, icon, tag, name);
            }
            BookmarkInput::Delete { id: slot } => self.teleport_bookmark_delete(id, slot),
            BookmarkInput::Teleport { id: slot } => self.teleport_bookmark_go(id, slot),
            BookmarkInput::ChangeSlot => {}
        }
    }

    // ------------------------------------------------------------------ the rules

    /// Java `Player.teleportBookmarkCondition(type)`.
    fn teleport_bookmark_condition(&self, id: ObjectId, kind: ConditionType) -> bool {
        use SystemMessageId as M;
        let refuse = |message: SystemMessageId| {
            self.send(id, &SystemMessage::new(message));
            false
        };
        if self.is_in_combat(id) {
            return refuse(M::YouCannotUseMyTeleportsDuringABattle);
        }
        if self.is_in_siege(id) || self.siege_state_of(id) != 0 {
            return refuse(
                M::YouCannotUseMyTeleportsWhileParticipatingALargeScaleBattleSuchAsACastleSiegeFortressSiegeOrHideoutSiege,
            );
        }
        if self.is_in_duel(id) || self.is_starting_duel(id) {
            return refuse(M::YouCannotUseMyTeleportsDuringADuel);
        }
        if self.is_flying(id) {
            return refuse(M::YouCannotUseMyTeleportsWhileFlying);
        }
        // `_inOlympiadMode` (M8-OLY-01).
        if self.is_in_olympiad_mode(id) {
            return refuse(M::YouCannotUseMyTeleportsWhileParticipatingInAnOlympiadMatch);
        }
        if self.is_paralyzed(id) {
            return refuse(M::YouCannotUseMyTeleportsWhileYouAreInAPetrifiedOrParalyzedState);
        }
        if self.is_dead(id) {
            return refuse(M::YouCannotUseMyTeleportsWhileYouAreDead);
        }
        if kind == ConditionType::Save && (self.is_in_7s_dungeon(id) || self.is_in_dimensional_rift(id)) {
            return refuse(M::YouCannotUseMyTeleportsToReachThisArea);
        }
        if self.is_water_task_running(id) {
            return refuse(M::YouCannotUseMyTeleportsUnderwater);
        }
        if kind == ConditionType::Save
            && [
                ZoneId::Siege,
                ZoneId::ClanHall,
                ZoneId::Jail,
                ZoneId::Castle,
                ZoneId::NoSummonFriend,
                ZoneId::Fort,
            ]
            .into_iter()
            .any(|zone| self.is_inside_zone(id, zone))
        {
            return refuse(M::YouCannotUseMyTeleportsToReachThisArea);
        }
        // `isInBoat()` and `isInAirShip()` are always false here (no boats).
        if self.is_inside_zone(id, ZoneId::NoBookmark) {
            return refuse(match kind {
                ConditionType::Go => M::YouCannotUseMyTeleportsInThisArea,
                ConditionType::Save => M::YouCannotUseMyTeleportsToReachThisArea,
            });
        }
        true
    }

    /// Java `ItemContainer.getInventoryItemCount(itemId, enchantLevel)` on the inventory (worn items
    /// included): the count of a stackable item, else how many there are. A negative
    /// `enchant_level` matches any.
    fn inventory_item_count_enchant(&self, id: ObjectId, item_id: i32, enchant_level: i32) -> i64 {
        let Some(player) = self.players.get(&id) else {
            return 0;
        };
        let stackable = self.data.item_stackable(item_id);
        let mut count = 0;
        for item in player.data.items.iter().filter(|i| {
            i.item_id == item_id
                && (i.enchant_level == enchant_level || enchant_level < 0)
                && (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL)
        }) {
            if stackable {
                return item.count;
            }
            count += 1;
        }
        count
    }

    /// Java `Player.teleportBookmarkAdd(x, y, z, icon, tag, name)`.
    #[allow(clippy::too_many_arguments)]
    fn teleport_bookmark_add(
        &mut self,
        id: ObjectId,
        x: i32,
        y: i32,
        z: i32,
        icon: i32,
        tag: String,
        name: String,
    ) {
        if !self.teleport_bookmark_condition(id, ConditionType::Save) {
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let slots = player.data.profile.bookmark_slot;
        let used = i32::try_from(player.data.profile.bookmarks.len()).unwrap_or(i32::MAX);
        if used >= slots {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveNoSpaceToSaveTheTeleportLocation),
            );
            return;
        }
        let consume = self.settings.bookmark_consume_item_id;
        if consume > 0 {
            if self.inventory_item_count_enchant(id, consume, -1) == 0 {
                let message = if consume == TELEPORT_FLAG_ITEM_ID {
                    SystemMessageId::YouCannotBookmarkThisLocationBecauseYouDoNotHaveAMyTeleportFlag
                } else {
                    SystemMessageId::IncorrectItemCount2
                };
                self.send(id, &SystemMessage::new(message));
                return;
            }
            // `destroyItem(NONE, getItemByItemId(itemId).getObjectId(), 1, null, true)`
            if let Some(item) = self.inventory_item_by_item_id(id, consume).map(|i| i.object_id) {
                self.destroy_item_by_object_id(id, item, 1, true);
            }
        }
        // The first free slot number from 1 up (the size check above guarantees one).
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let mut slot = 1;
        while slot <= slots && player.data.profile.bookmarks.iter().any(|b| b.id == slot) {
            slot += 1;
        }
        let bookmark = BookmarkData {
            id: slot,
            x,
            y,
            z,
            icon,
            tag,
            name,
        };
        player.data.profile.bookmarks.retain(|b| b.id != slot);
        player.data.profile.bookmarks.push(bookmark.clone());
        player.data.profile.bookmarks.sort_by_key(|b| b.id);
        self.outbox
            .persist(crate::outbox::PersistCommand::Bookmark(BookmarkWrite::Add {
                char_id: id.0,
                bookmark,
            }));
        self.send_bookmark_info(id);
    }

    /// Java `Player.teleportBookmarkModify(id, icon, tag, name)`: the list is sent even for an
    /// unknown bookmark.
    fn teleport_bookmark_modify(&mut self, id: ObjectId, slot: i32, icon: i32, tag: String, name: String) {
        if let Some(player) = self.players.get_mut(&id)
            && let Some(bookmark) = player.data.profile.bookmarks.iter_mut().find(|b| b.id == slot)
        {
            bookmark.icon = icon;
            bookmark.tag.clone_from(&tag);
            bookmark.name.clone_from(&name);
            self.outbox
                .persist(crate::outbox::PersistCommand::Bookmark(BookmarkWrite::Update {
                    char_id: id.0,
                    id: slot,
                    icon,
                    tag,
                    name,
                }));
        }
        self.send_bookmark_info(id);
    }

    /// Java `Player.teleportBookmarkDelete(id)`: an unknown bookmark sends nothing.
    fn teleport_bookmark_delete(&mut self, id: ObjectId, slot: i32) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let before = player.data.profile.bookmarks.len();
        player.data.profile.bookmarks.retain(|b| b.id != slot);
        if player.data.profile.bookmarks.len() == before {
            return;
        }
        self.outbox
            .persist(crate::outbox::PersistCommand::Bookmark(BookmarkWrite::Delete {
                char_id: id.0,
                id: slot,
            }));
        self.send_bookmark_info(id);
    }

    /// Java `Player.teleportBookmarkGo(id)`.
    fn teleport_bookmark_go(&mut self, id: ObjectId, slot: i32) {
        if !self.teleport_bookmark_condition(id, ConditionType::Go) {
            return;
        }
        if self.inventory_item_count_enchant(id, TELEPORT_ITEM_ID, 0) == 0 {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotTeleportBecauseYouDoNotHaveATeleportItem),
            );
            return;
        }
        // NOTE(java-quirk): "S1 has disappeared" goes out before the bookmark is looked up, so an
        // unknown bookmark id costs nothing but still shows the message.
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::S1HasDisappeared)
                .with(SystemMessageParam::ItemName(TELEPORT_ITEM_ID)),
        );
        if let Some(bookmark) = self.bookmark_of(id, slot) {
            // `getItemByItemId(13016)` is the first item of that id, whatever its enchant.
            if let Some(item) = self
                .inventory_item_by_item_id(id, TELEPORT_ITEM_ID)
                .map(|i| i.object_id)
            {
                self.destroy_item_by_object_id(id, item, 1, false);
            }
            // `teleToLocation(bookmark, false)`: a `Location` has heading 0 and instance 0, so the
            // character leaves the instance it is in.
            self.tele_to_location(
                id,
                TeleportTarget::at(bookmark.x, bookmark.y, bookmark.z).with_instance(0),
            );
        }
        self.send_bookmark_info(id);
    }
}
