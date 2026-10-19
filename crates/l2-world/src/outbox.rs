//! How the world talks to the outside: per-player packet sinks and one outbox for the rest.
//!
//! Both are traits so the world runs synchronously in tests (recording implementations) and
//! the game server plugs in real connections and channels. Implementations must never block:
//! they hand the value to a queue and return.

use crate::command::LeaveReason;
use crate::player::{ItemData, PlayerData};
use l2_util::ObjectId;

/// One player's connection.
pub trait PacketSink: Send {
    /// Queues an encoded game packet (plaintext; the connection encrypts it).
    fn send(&self, bytes: Vec<u8>);

    /// The player left the world. `Logout`/`Kicked`/`ServerShutdown` close the connection
    /// after the queued packets; `Restart` keeps it open for the character screen.
    fn left_world(&self, reason: LeaveReason);
}

/// Writes for the persistence task (D-15). Applied in order per character.
#[derive(Debug, Clone, PartialEq)]
pub enum PersistCommand {
    /// Store the character's current state (logout, restart, autosave, shutdown).
    StorePlayer(Box<PlayerData>),
    /// The player left. Sent *after* the final `StorePlayer`, so the persistence task can tell
    /// the login server `PlayerLogout` only once the data is safe.
    ///
    /// `logout` is false when the connection stays (a restart to the character screen): the
    /// character is marked offline, but the account is still on this server for the login server.
    PlayerLeft {
        account: String,
        object_id: ObjectId,
        logout: bool,
    },
    /// Java `Item.updateDatabase()`: item rows of one character that changed, written when they
    /// change (equip, unequip, destroy, drop) and not only at the next full store. `upserts` are
    /// the whole rows (an `UPDATE` when the row exists, else an `INSERT`, like Java's
    /// `_existsInDb`), `removed` are rows Java deletes (`removeFromDb`). Applied in order with
    /// the character's other commands.
    ItemChanges {
        owner: ObjectId,
        upserts: Vec<ItemData>,
        removed: Vec<ObjectId>,
    },
    /// Java `ItemsOnGroundManager.run()` (only with `SaveDroppedItem`): `DELETE FROM
    /// itemsonground`, then one `INSERT` per row.
    SaveGroundItems(Vec<crate::ground_item::GroundItemRow>),
}

/// Messages for the login server link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsEvent {
    /// Java `LoginServerThread.sendLogin` → `PlayerInGame`.
    PlayerInGame { account: String },
}

/// Everything the world produces that is not a packet for a player.
pub trait Outbox: Send {
    fn persist(&mut self, command: PersistCommand);
    fn ls_event(&mut self, event: LsEvent);
}
