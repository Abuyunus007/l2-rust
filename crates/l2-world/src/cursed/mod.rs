//! Cursed weapons (M8-CURSED-01): Zariche and Akamanah. The drop from a monster, the pickup that
//! binds the weapon to a player, the form and the passive skill of the owner, the kill count, the
//! life timer, the drop on the owner's death and the end of the weapon.
// Java: managers/CursedWeaponsManager.java
// Java: entity/actor/holders/player/CursedWeapon.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminCursedWeapons.java
// Java: network/clientpackets/RequestCursedWeaponList.java, RequestCursedWeaponLocation.java
//
// # How it fits together
// - [`CursedWorld`] (in `World::cursed`) holds the settings and one [`CursedWeapon`] per entry of
//   `CursedWeapons.xml`, in the order of Java's `HashMap`. A weapon's state is the item it is
//   (the ground item or the inventory item, by object id), its owner (by id), its kill count and
//   its end time (unix milliseconds, like the `cursed_weapons.endTime` column).
// - `weapon.rs`: the `CursedWeapon` methods (`checkDrop`, `dropIt`, `activate`, `endOfLife`,
//   `increaseKills`, `giveSkill`, `doTransform`, the timer) and the manager's entry points called
//   from the rest of the world.
// - `admin.rs`: `//cw_info`, `//cw_info_menu`, `//cw_remove`, `//cw_goto`, `//cw_add`,
//   `//cw_reload`.
// - `packets.rs`: `RequestCursedWeaponList` and `RequestCursedWeaponLocation`.
//
// # Side by side (D-39, D-53)
// The cursed weapon and its item are global: the Java game server keeps the same `cursed_weapons`
// rows. With `CycleDatabaseWrites = False` this server
// - never drops a cursed weapon from a monster (the drop chance check is skipped),
// - treats the rows of the table as "not on this server": they are not read, and no row of the
//   table is written or removed (a weapon a GM gives out with `//cw_add` lives in memory only),
// - leaves a cursed weapon item a character carries into this server alone (Java's enter-world
//   rule that destroys one would delete the item row Sieghardt's owner still needs).
// The per-character rows (`items`, `characters.karma`) of a weapon that ends while its owner is
// offline are written as Java does.
//
// # Left out, with the system that brings it
// - `//cw_reload` (and `//reload cw`) re-reads `CursedWeapons.xml` and the table in Java; this
//   world never reads files or the database (D-15). The GM is told to restart the server.
// - Custom events (`isOnEvent`, TvT, CtF, DM) refusing a cursed weapon owner: the event scripts
//   are D-05.

mod admin;
mod packets;
mod weapon;

use l2_util::ObjectId;

use crate::timer::TimerId;

pub use admin::register_admin;
pub use packets::CursedInput;

/// The configuration the cursed weapons read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursedSettings {
    /// Java `GeneralConfig.ALLOW_CURSED_WEAPONS` (`AllowCursedWeapons`, default True): without it
    /// the manager loads nothing.
    pub allow_cursed_weapons: bool,
    /// `CycleDatabaseWrites` of `config/SideBySide.ini` (D-53, `game_server::cycle_database_writes`).
    pub cycle_database_writes: bool,
    /// Tests: the unix time (ms) the world clock starts at, so end times are reproducible.
    /// `None` reads the system clock.
    pub clock_override_unix_ms: Option<i64>,
}

/// One `cursed_weapons` row (Java `CursedWeapon.saveData`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursedRow {
    pub item_id: i32,
    pub char_id: i32,
    pub player_karma: i32,
    pub player_pk_kills: i32,
    pub nb_kills: i32,
    /// `endTime`: unix milliseconds.
    pub end_time: i64,
}

/// The rows the game server reads for [`World::load_cursed_weapons`]. Empty without the table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CursedLoad {
    pub rows: Vec<CursedRow>,
}

/// A write for the persistence task (`PersistCommand::Cursed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursedWrite {
    /// Java `CursedWeapon.saveData`: `DELETE FROM cursed_weapons WHERE itemId = ?`, then (when the
    /// weapon is held, `row` is `Some`) the `INSERT`. Only sent with `CycleDatabaseWrites` (D-53).
    Save { item_id: i32, row: Option<CursedRow> },
    /// Java `CursedWeaponsManager.removeFromDb`. Only sent with `CycleDatabaseWrites` (D-53).
    Remove { item_id: i32 },
    /// Java `CursedWeapon.endOfLife` for an owner who is offline: `DELETE FROM items WHERE
    /// owner_id=? AND item_id=?`, then `UPDATE characters SET karma=?, pkkills=? WHERE charId=?`.
    /// The character's own rows, so they are written whatever the switch says.
    OfflineEnd {
        player_id: i32,
        item_id: i32,
        karma: i32,
        pk_kills: i32,
    },
}

/// The timers of the cursed weapons (`TimerKind::Cursed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursedTimer {
    /// Java `RemoveTask`: every `durationLost * 12000` ms the weapon ends when its time is up.
    /// By item id.
    Remove(i32),
    /// `activate`: `ThreadPool.schedule(() -> _player.broadcastPacket(new SocialAction(.., 17)),
    /// 300)`. By item id (Java reads the weapon's `_player` when it runs).
    Social(i32),
    /// `doTransform` of a player that was transformed: the form 500 ms after the old one ended.
    /// By item id.
    Transform(i32),
}

/// A cursed weapon (Java `CursedWeapon`): the definition of `CursedWeapons.xml` and the state.
#[derive(Debug, Clone, PartialEq)]
pub struct CursedWeapon {
    pub item_id: i32,
    pub skill_id: i32,
    pub name: String,
    pub drop_rate: i32,
    /// Minutes the weapon lives.
    pub duration: i32,
    /// Minutes a kill takes off.
    pub duration_lost: i32,
    pub disapear_chance: i32,
    pub stage_kills: i32,
    /// `SkillData.getMaxLevel(skillId)`.
    pub skill_max_level: i32,
    /// Java `_isDropped`: the weapon lies on the ground.
    pub is_dropped: bool,
    /// Java `_isActivated`: a player holds it.
    pub is_activated: bool,
    pub(crate) remove_task: Option<TimerId>,
    pub nb_kills: i32,
    /// Java `_endTime`: unix milliseconds.
    pub end_time: i64,
    /// Java `_playerId`: the character that holds it, online or not.
    pub player_id: i32,
    /// Java `_player`: the holder while it is online.
    pub player: Option<ObjectId>,
    /// Java `_item`: the weapon's item, on the ground or in the holder's inventory.
    pub item: Option<ObjectId>,
    pub player_karma: i32,
    pub player_pk_kills: i32,
    /// Java `transformationId` (0 until `doTransform` ran).
    pub(crate) transformation_id: i32,
}

impl CursedWeapon {
    pub(crate) fn from_def(def: &l2_data::CursedWeaponDef, skill_max_level: i32) -> Self {
        Self {
            item_id: def.item_id,
            skill_id: def.skill_id,
            name: def.name.clone(),
            drop_rate: def.drop_rate,
            duration: def.duration,
            duration_lost: def.duration_lost,
            disapear_chance: def.disapear_chance,
            stage_kills: def.stage_kills,
            skill_max_level,
            is_dropped: false,
            is_activated: false,
            remove_task: None,
            nb_kills: 0,
            end_time: 0,
            player_id: 0,
            player: None,
            item: None,
            player_karma: 0,
            player_pk_kills: 0,
            transformation_id: 0,
        }
    }

    /// Java `isActive()`.
    pub fn is_active(&self) -> bool {
        self.is_activated || self.is_dropped
    }

    /// Java `getLevel()`.
    // NOTE(java-quirk): `stageKills` 0 divides by zero in Java (an `ArithmeticException`); the
    // level is 0 here.
    pub fn level(&self) -> i32 {
        if self.nb_kills > self.stage_kills.wrapping_mul(self.skill_max_level) {
            return self.skill_max_level;
        }
        self.nb_kills.checked_div(self.stage_kills).unwrap_or(0)
    }

    /// The row `saveData` writes for a held weapon.
    pub(crate) fn row(&self) -> CursedRow {
        CursedRow {
            item_id: self.item_id,
            char_id: self.player_id,
            player_karma: self.player_karma,
            player_pk_kills: self.player_pk_kills,
            nb_kills: self.nb_kills,
            end_time: self.end_time,
        }
    }
}

/// The cursed weapons of the world (Java `CursedWeaponsManager`).
#[derive(Debug, Default)]
pub struct CursedWorld {
    pub(crate) settings: CursedSettings,
    /// In the order of Java's `_cursedWeapons.values()`.
    pub(crate) weapons: Vec<CursedWeapon>,
}

impl CursedWorld {
    /// Java `getCursedWeapons()`.
    pub fn weapons(&self) -> &[CursedWeapon] {
        &self.weapons
    }

    /// Java `getCursedWeapon(itemId)`.
    pub fn weapon(&self, item_id: i32) -> Option<&CursedWeapon> {
        self.weapons.iter().find(|w| w.item_id == item_id)
    }

    pub(crate) fn index_of(&self, item_id: i32) -> Option<usize> {
        self.weapons.iter().position(|w| w.item_id == item_id)
    }
}

/// Java `Player.setCursedWeaponEquippedId` / `isCursedWeaponEquipped`: the item id of the cursed
/// weapon the player holds, 0 for none. Lives in `Player::cursed_weapon_equipped_id`.
pub const NO_CURSED_WEAPON: i32 = 0;

/// Java `Player.setKarma(9999999)` of `activate`.
pub(crate) const CURSED_KARMA: i32 = 9_999_999;

/// Zariche (`CursedWeapon.doTransform`: item 8190 becomes transformation 301).
pub const ZARICHE_ID: i32 = 8190;
/// Akamanah (item 8689 becomes transformation 302).
pub const AKAMANAH_ID: i32 = 8689;
/// Java `CommonSkill.VOID_BURST` (3630, 1).
pub(crate) const VOID_BURST: (i32, i32) = (3630, 1);
/// Java `CommonSkill.VOID_FLOW` (3631, 1).
pub(crate) const VOID_FLOW: (i32, i32) = (3631, 1);
