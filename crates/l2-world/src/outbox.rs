//! How the world talks to the outside: per-player packet sinks and one outbox for the rest.
//!
//! Both are traits so the world runs synchronously in tests (recording implementations) and
//! the game server plugs in real connections and channels. Implementations must never block:
//! they hand the value to a queue and return.

use crate::command::LeaveReason;
use crate::player::{ItemData, PlayerData, SkillData};
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
    /// Java `Player.storeSkill` / `removeSkill`: `character_skills` rows of one character that
    /// changed (a level up that teaches or takes skills), written when they change and not only
    /// at the next full store. `upserts` are an `UPDATE` of the level when the row exists, else a
    /// `REPLACE`, like Java; `removed` are skill ids whose row Java deletes. Applied in order with
    /// the character's other commands.
    SkillChanges {
        owner: ObjectId,
        upserts: Vec<SkillData>,
        removed: Vec<i32>,
    },
    /// Java `Player.storeRecommendationValues()` (a vote, `RecoGiveTask`):
    /// `REPLACE INTO character_reco_bonus (charId,rec_have,rec_left) VALUES (?,?,?)`. The
    /// `REPLACE` drops the old row, so `time_left` falls back to the column default 0 (a Java
    /// quirk the Rust side keeps). Applied in order with the character's other commands.
    RecoValues {
        owner: ObjectId,
        rec_have: i32,
        rec_left: i32,
    },
    /// Java `DailyResetManager.resetRecommends()`, the database half: the two `UPDATE`s of
    /// `character_reco_bonus` for every character (`rec_left` 20, `time_left` one hour,
    /// `rec_have` lowered by 20, not below 0). D-39: the Java game server runs the same
    /// statements on its own daily reset.
    ResetRecommends,
    /// Java `PlayerVariables.storeMe()`: the `character_variables` rows of one online character
    /// that changed, written now instead of with the next store (the daily reset does this for
    /// every player). `upserts` are `(var, value)` pairs, `removed` the names Java deletes.
    PlayerVariables {
        owner: ObjectId,
        upserts: Vec<(String, String)>,
        removed: Vec<String>,
    },
    /// Java `PunishmentTask.onStart`: `INSERT INTO punishments`. The persistence task keeps the
    /// id the database gave the row, to find it again at `EndPunishment`.
    StorePunishment(crate::punishments::StoredPunishment),
    /// Java `PunishmentTask.onEnd`: `UPDATE punishments SET expiration = <ended_at> WHERE id = ?`.
    /// `id` is the row id of a punishment loaded at startup, 0 for one this server stored (the
    /// persistence task finds the row by `key`, `affect` and `kind`).
    EndPunishment {
        id: i32,
        key: String,
        affect: crate::punishments::PunishmentAffect,
        kind: crate::punishments::PunishmentType,
        ended_at: i64,
    },
    /// Java `PunishmentManager.changeCharAccessLevel`: `UPDATE characters SET accessLevel=? WHERE
    /// charId=?`.
    CharAccessLevel { char_id: i32, level: i32 },
    /// A clan row write with Java's statement (`clan_data`, `clan_privs`, `clan_subpledges`,
    /// `clan_notices`, the clan columns of `characters`), see `clan/`. Applied in order with the
    /// other commands.
    Clan(crate::clan::ClanWrite),
}

/// Messages for the login server link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsEvent {
    /// Java `LoginServerThread.sendLogin` → `PlayerInGame`.
    PlayerInGame { account: String },
    /// Java `LoginServerThread.sendAccessLevel` → `ChangeAccessLevel` (a ban is -1, its end 0).
    AccessLevel { account: String, level: i32 },
}

/// Everything the world produces that is not a packet for a player.
pub trait Outbox: Send {
    fn persist(&mut self, command: PersistCommand);
    fn ls_event(&mut self, event: LsEvent);
}
