//! Players and the data shared by every creature (D-17: data structs, no inheritance).
//!
//! Java: `entity/actor/Creature.java`, `Playable.java`, `Player.java`. Only what M2 needs is
//! here; later milestones add fields (effects, AI state…).

use std::collections::BTreeMap;
use std::net::IpAddr;

use l2_util::ObjectId;

use crate::data::DEFAULT_TITLE_COLOR;
use crate::stats::Calculators;

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
    /// The last `getMaxHp()` of the stat engine (Java `PlayerStat._oldMaxHp`, the "stats watch"):
    /// refreshed when the stats change (`World::refresh_max_vitals`), read by `StatusUpdate`.
    pub max_hp: f64,
    pub cur_mp: f64,
    /// The last `getMaxMp()` (see `max_hp`).
    pub max_mp: f64,
    pub cur_cp: f64,
    /// The last `getMaxCp()` (see `max_hp`).
    pub max_cp: f64,
    /// Java `_calculators`: the stat functions (see `stats/`).
    pub calculators: Calculators,
    /// Java `_skills`: every skill the creature knows as `(id, level)`, one per id, in the order
    /// they were added (learned skills, item and armor set skills, penalty skills).
    pub skills: Vec<(i32, i32)>,
    /// Java `EffectList._passives`: the passive skills whose effects are applied.
    pub passives: Vec<(i32, i32)>,
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

/// One command of a macro (Java `MacroCmd`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCmdData {
    /// Java `MacroType` ordinal.
    pub kind: i32,
    pub d1: i32,
    pub d2: i32,
    pub cmd: String,
}

/// One macro (`character_macroses`, Java `Macro`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroData {
    pub id: i32,
    pub icon: i32,
    pub name: String,
    pub descr: String,
    pub acronym: String,
    pub commands: Vec<MacroCmdData>,
}

/// The load-only part of a character: values the enter-world packets show, read from tables the
/// Java game server also owns (D-39). The persistence task never writes any of it, so it only
/// travels database -> world.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerProfile {
    /// `characters.fame`.
    pub fame: i32,
    /// `characters.nobless`.
    pub noble: bool,
    /// `characters.title_color`.
    pub title_color: i32,
    /// `characters.vitality_points`.
    pub vitality_points: i32,
    /// `characters.bookmarkslot`.
    pub bookmark_slot: i32,
    /// `characters.death_penalty_level`.
    pub death_penalty_level: i32,
    /// Java `_expBeforeDeath`: the exp the character had before a death took some. Java never
    /// reads the column back (it is 0 after a login) but writes the live value with every
    /// store, so this one is written too.
    pub exp_before_death: i64,
    /// `character_reco_bonus.rec_have` / `rec_left` / `time_left` (ms of the Nevit hourglass).
    pub recom_have: i32,
    pub recom_left: i32,
    pub recom_time_left_ms: i64,
    /// `character_macroses`.
    pub macros: Vec<MacroData>,
    /// `character_variables` (Java `PlayerVariables`).
    pub variables: BTreeMap<String, String>,
    /// `character_hennas`: the dye id (`symbol_id`) of slots 1 to 3 of the active class, 0 for an
    /// empty slot (Java `restoreHenna`).
    pub hennas: [i32; 3],
}

impl Default for PlayerProfile {
    fn default() -> Self {
        Self {
            fame: 0,
            noble: false,
            title_color: DEFAULT_TITLE_COLOR,
            vitality_points: 0,
            bookmark_slot: 0,
            death_penalty_level: 0,
            exp_before_death: 0,
            recom_have: 0,
            recom_left: 0,
            recom_time_left_ms: 0,
            macros: Vec::new(),
            variables: BTreeMap::new(),
            hennas: [0; 3],
        }
    }
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
    /// Values loaded for the enter-world packets that the persistence task never writes.
    pub profile: PlayerProfile,
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
    /// Java `hasEnteredWorld()`: set once the enter-world packets went out. `UserInfo` tells the
    /// client it can equip a cloak only before that.
    pub entered_world: bool,
    /// Java `_recomHave` / `_recomLeft` and the milliseconds the Nevit hourglass has left; loaded
    /// from the profile when the player enters (`Player.checkRecoBonusTask`), 0 before that.
    pub recom_have: i32,
    pub recom_left: i32,
    pub hourglass_ms: i64,
    /// Java `getKnownRelations()`: the relation (mask, auto-attackable) each viewer was last told
    /// about this player, so `RelationChanged` only goes out when it changes.
    pub known_relations: std::collections::HashMap<ObjectId, (i32, bool)>,
    /// Java `_move`, the movement-related fields of `Creature`/`Player` and the move intention of
    /// `PlayerAI` (see `movement.rs`).
    pub movement: crate::movement::MovementState,
    /// The zones the player is inside and the flags they set (see `zones.rs`).
    pub zones: crate::zones::PlayerZones,
    /// Java `isTeleporting()` and the teleport watchdog (see `teleport.rs`).
    pub tele: crate::teleport::TeleportState,
    /// Java `getTarget()`: the object the player has selected (see `target.rs`).
    pub target: Option<ObjectId>,
    /// The dialogs the player was shown and the NPC it clicked last (see `dialog.rs`).
    pub talk: crate::dialog::PlayerTalk,
    /// Java `_inventoryUpdate` waiting for its 100 ms timer (see `item_ops.rs`).
    pub pending_inventory: crate::item_ops::PendingInventoryUpdate,
    /// Java `_curWeightPenalty` (`refreshOverloaded`).
    pub weight_penalty: i32,
    /// Java `_expertiseWeaponPenalty` / `_expertiseArmorPenalty` (`refreshExpertisePenalty`).
    pub expertise_weapon_penalty: i32,
    pub expertise_armor_penalty: i32,
    /// The delayed sends of the inventory code (see `inventory/notify.rs`).
    pub inv_tasks: crate::inventory::InventoryTasks,
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
            // The world builds the calculators when the player enters (`init_player_stats`).
            calculators: Calculators::default(),
            skills: Vec::new(),
            passives: Vec::new(),
        };
        let movement = crate::movement::MovementState::at(data.x, data.y, data.z);
        Self {
            core,
            data,
            entered_at_tick: tick,
            entered_at_unix_ms: 0,
            client_ip: None,
            entered_world: false,
            recom_have: 0,
            recom_left: 0,
            hourglass_ms: 0,
            known_relations: std::collections::HashMap::new(),
            movement,
            zones: crate::zones::PlayerZones::default(),
            tele: crate::teleport::TeleportState::default(),
            target: None,
            talk: crate::dialog::PlayerTalk::default(),
            pending_inventory: crate::item_ops::PendingInventoryUpdate::default(),
            weight_penalty: 0,
            expertise_weapon_penalty: 0,
            expertise_armor_penalty: 0,
            inv_tasks: crate::inventory::InventoryTasks::default(),
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
        // Java `storeCharBase` writes `getMaxHp()` ...: the stat engine's values.
        data.max_hp = c.max_hp;
        data.max_mp = c.max_mp;
        data.max_cp = c.max_cp;
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
