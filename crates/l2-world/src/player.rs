//! Players and the data shared by every creature (D-17: data structs, no inheritance).
//!
//! Java: `entity/actor/Creature.java`, `Playable.java`, `Player.java`. Only what M2 needs is
//! here; later milestones add fields (stats, effects, target, AI state…).

use std::net::IpAddr;

use l2_util::ObjectId;

/// What every creature has (players now, NPCs in M3).
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureCore {
    pub object_id: ObjectId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub level: i32,
    pub cur_hp: f64,
    pub max_hp: f64,
    pub cur_mp: f64,
    pub max_mp: f64,
    pub cur_cp: f64,
    pub max_cp: f64,
}

/// One item of a player (inventory or worn). Mirrors the `items` table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemData {
    pub object_id: ObjectId,
    /// Template id (`items.item_id`).
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
    /// `items.loc`: "INVENTORY", "PAPERDOLL", "WAREHOUSE", …
    pub loc: String,
    /// `items.loc_data`: the paperdoll slot when worn.
    pub loc_data: i32,
    pub custom_type1: i32,
    pub custom_type2: i32,
    /// `items.mana_left` (shadow items), -1 = none.
    pub mana_left: i32,
    /// `items.time` (limited-time items), -1 = none.
    pub time: i64,
}

/// One learned skill (`character_skills`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillData {
    pub skill_id: i32,
    pub skill_level: i32,
}

/// One shortcut (`character_shortcuts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutData {
    pub slot: i32,
    pub page: i32,
    /// Java `ShortcutType` ordinal (item, skill, action, macro, recipe, bookmark).
    pub kind: i32,
    pub shortcut_id: i32,
    pub level: i32,
}

/// A character as loaded from (and stored to) the database. The session task builds it before
/// `EnterWorld`; the world hands snapshots of it to the persistence task.
///
/// Field names follow the `characters` columns (Java `Player.restore` / `store`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerData {
    pub object_id: ObjectId,
    pub account: String,
    pub name: String,
    pub title: String,
    pub class_id: i32,
    pub base_class: i32,
    pub race: i32,
    pub sex: i32,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub level: i32,
    pub exp: i64,
    pub sp: i64,
    pub karma: i32,
    pub pvp_kills: i32,
    pub pk_kills: i32,
    pub access_level: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub cur_hp: f64,
    pub max_hp: f64,
    pub cur_mp: f64,
    pub max_mp: f64,
    pub cur_cp: f64,
    pub max_cp: f64,
    /// Total time online in seconds (`characters.onlinetime`).
    pub online_time: i64,
    pub items: Vec<ItemData>,
    pub skills: Vec<SkillData>,
    pub shortcuts: Vec<ShortcutData>,
}

/// A player inside the world.
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub core: CreatureCore,
    pub data: PlayerData,
    /// World tick at which the player entered (for online time and admin views).
    pub entered_at_tick: u64,
    /// Unix time (ms) at which the player entered. Set by the world; only the admin view uses it.
    pub entered_at_unix_ms: i64,
    /// The client's address when the session entered (`None` if unknown).
    pub client_ip: Option<IpAddr>,
}

impl Player {
    pub fn from_data(data: PlayerData, tick: u64) -> Self {
        let core = CreatureCore {
            object_id: data.object_id,
            x: data.x,
            y: data.y,
            z: data.z,
            heading: data.heading,
            level: data.level,
            cur_hp: data.cur_hp,
            max_hp: data.max_hp,
            cur_mp: data.cur_mp,
            max_mp: data.max_mp,
            cur_cp: data.cur_cp,
            max_cp: data.max_cp,
        };
        Self {
            core,
            data,
            entered_at_tick: tick,
            entered_at_unix_ms: 0,
            client_ip: None,
        }
    }

    /// The data to store, with the live values (position, hp…) copied back.
    pub fn snapshot(&self) -> PlayerData {
        let mut data = self.data.clone();
        let c = &self.core;
        data.x = c.x;
        data.y = c.y;
        data.z = c.z;
        data.heading = c.heading;
        data.level = c.level;
        data.cur_hp = c.cur_hp;
        data.cur_mp = c.cur_mp;
        data.cur_cp = c.cur_cp;
        data
    }
}

/// One line of the admin "online players" list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlinePlayer {
    pub object_id: ObjectId,
    pub account: String,
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    /// Seconds since the player entered the world.
    pub online_secs: u64,
    /// Unix time (ms) at which the player entered the world.
    pub online_since_unix_ms: i64,
    /// The client's address (`None` when the session didn't report one).
    pub ip: Option<IpAddr>,
}
