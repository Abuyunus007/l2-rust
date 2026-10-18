//! What the world needs to know about the datapack and the configuration, without owning
//! either: [`StaticData`] (item templates, skills, class templates, access levels) and
//! [`WorldSettings`] (the `.ini` values the enter-world and chat code reads).
//!
//! `l2-world` has no XML or config loaders (MODULE_MAP): the game server implements
//! [`StaticData`] over the loaded `Datapack` and fills [`WorldSettings`] from `GameConfig`.
//! Tests plug in small fakes. [`NoData`] and `WorldSettings::default()` are the "nothing loaded"
//! and "Java defaults" versions, so a bare `World::new` still works.

use crate::player::PlayerData;

/// Java `Race.DWARF.ordinal()`.
pub const RACE_DWARF: i32 = 4;

/// The part of an item template the packets need (Java `ItemTemplate`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStatic {
    /// `getDisplayId()`: the id the client is shown.
    pub display_id: i32,
    /// `getType2()`: 0 weapon, 1 shield/armor, 2 accessory, 3 quest item, 4 adena, 5 other.
    pub type2: i16,
    /// `getBodyPart().getMask()`.
    pub body_part: i32,
    /// `isQuestItem()`: such items go into `ExQuestItemList`, not `ItemList`.
    pub quest_item: bool,
    /// `getWeight()`.
    pub weight: i32,
    /// `getSharedReuseGroup()`.
    pub shared_reuse_group: i32,
    /// `isWeapon()`.
    pub weapon: bool,
}

impl ItemStatic {
    /// What Java reports for an item whose template is unknown to this server: the item shown
    /// as itself, an "other" item (type2 5) with no body part.
    pub fn unknown(item_id: i32) -> Self {
        Self {
            display_id: item_id,
            type2: 5,
            body_part: 0,
            quest_item: false,
            weight: 0,
            shared_reuse_group: -1,
            weapon: false,
        }
    }
}

/// An access level (`config/AccessLevels.xml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessLevelStatic {
    pub name_color: i32,
    pub title_color: i32,
    pub is_gm: bool,
}

impl AccessLevelStatic {
    /// Level 0 ("User") of the shipped `AccessLevels.xml`.
    pub const USER: Self = Self {
        name_color: 0x00FF_FFFF,
        title_color: 0x00EC_F9A2,
        is_gm: false,
    };

    /// Java `new AccessLevel()`, which `AdminData.getAccessLevel` makes for a level the file does
    /// not list: white name and title, no GM rights.
    pub const UNREGISTERED: Self = Self {
        name_color: 0x00FF_FFFF,
        title_color: 0x00FF_FFFF,
        is_gm: false,
    };
}

/// The title colour a character has when nothing sets one (Java
/// `PlayerAppearance.DEFAULT_TITLE_COLOR`, `0xECF9A2`).
pub const DEFAULT_TITLE_COLOR: i32 = 0x00EC_F9A2;

/// The numbers `UserInfo` prints that Java computes from the stat system (`Calculator`,
/// templates, equipment, skills, effects).
///
/// The stat system is not ported yet (M4): [`StaticData::derive_stats`] returns the values of an
/// unequipped, buff-less character built from the class template (see the game server's
/// implementation for exactly what is applied).
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedStats {
    pub str: i32,
    pub dex: i32,
    pub con: i32,
    pub int: i32,
    pub wit: i32,
    pub men: i32,
    /// `(int) getPAtk(null)`.
    pub p_atk: i32,
    /// `(int) getPAtkSpd()`.
    pub p_atk_spd: i32,
    /// `(int) getPDef(null)`.
    pub p_def: i32,
    /// `getEvasionRate(null)`.
    pub evasion: i32,
    /// `getAccuracy()`.
    pub accuracy: i32,
    /// `getCriticalHit(null, null)`.
    pub critical: i32,
    /// `(int) getMAtk(null, null)`.
    pub m_atk: i32,
    /// `getMAtkSpd()`.
    pub m_atk_spd: i32,
    /// `(int) getMDef(null, null)`.
    pub m_def: i32,
    /// `Math.round(getRunSpeed() / moveMultiplier)` and the other three speeds.
    pub run_speed: i32,
    pub walk_speed: i32,
    pub swim_run_speed: i32,
    pub swim_walk_speed: i32,
    /// `getMovementSpeedMultiplier()`.
    pub move_multiplier: f64,
    /// `getAttackSpeedMultiplier()` (a Java `float`).
    pub attack_speed_multiplier: f32,
    /// `getCollisionRadius()` / `getCollisionHeight()` (Java `float`s).
    pub collision_radius: f32,
    pub collision_height: f32,
    /// `getMaxLoad()`.
    pub max_load: i32,
}

impl Default for DerivedStats {
    fn default() -> Self {
        Self {
            str: 40,
            dex: 30,
            con: 43,
            int: 21,
            wit: 11,
            men: 25,
            p_atk: 4,
            p_atk_spd: 300,
            p_def: 80,
            evasion: 0,
            accuracy: 0,
            critical: 40,
            m_atk: 6,
            m_atk_spd: 333,
            m_def: 41,
            run_speed: 120,
            walk_speed: 80,
            swim_run_speed: 50,
            swim_walk_speed: 50,
            move_multiplier: 1.0,
            attack_speed_multiplier: 1.0,
            collision_radius: 9.0,
            collision_height: 23.0,
            max_load: 69_000,
        }
    }
}

/// Everything static the world reads. Implemented over the datapack by the game server.
pub trait StaticData: Send + Sync {
    /// The template of an item, `None` if the item id is unknown.
    fn item(&self, item_id: i32) -> Option<ItemStatic>;

    /// Java `Skill.isPassive()` (operate type `P`). `false` for a skill that is not known.
    fn skill_is_passive(&self, skill_id: i32) -> bool;

    /// `(exp - expForLevel(level)) / (expForLevel(level + 1) - expForLevel(level))`, computed the
    /// way `UserInfo` does it (see `exp_percent` in `l2-protocol-game`).
    fn exp_percent(&self, exp: i64, level: i32) -> f64;

    /// The stats shown in `UserInfo` for this character (see [`DerivedStats`]).
    fn derive_stats(&self, player: &PlayerData) -> DerivedStats;

    /// The access level with this id. An id the file does not list gives Java's default
    /// `AccessLevel` ([`AccessLevelStatic::UNREGISTERED`]).
    fn access_level(&self, level: i32) -> AccessLevelStatic;

    /// `PlayerTemplate.getSafeFallHeight()` (`baseSafeFall`) of the player's class; Java's default
    /// when the template does not set it is 333.
    fn safe_fall_height(&self, _player: &PlayerData) -> i32 {
        333
    }
}

impl<T: StaticData + ?Sized> StaticData for std::sync::Arc<T> {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        (**self).item(item_id)
    }

    fn skill_is_passive(&self, skill_id: i32) -> bool {
        (**self).skill_is_passive(skill_id)
    }

    fn exp_percent(&self, exp: i64, level: i32) -> f64 {
        (**self).exp_percent(exp, level)
    }

    fn derive_stats(&self, player: &PlayerData) -> DerivedStats {
        (**self).derive_stats(player)
    }

    fn access_level(&self, level: i32) -> AccessLevelStatic {
        (**self).access_level(level)
    }

    fn safe_fall_height(&self, player: &PlayerData) -> i32 {
        (**self).safe_fall_height(player)
    }
}

/// A [`StaticData`] with nothing loaded. Every item is an unknown "other" item.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoData;

impl StaticData for NoData {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }

    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }

    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }

    fn derive_stats(&self, _player: &PlayerData) -> DerivedStats {
        DerivedStats::default()
    }

    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
}

/// Java `ChatBroadcastType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatBroadcast {
    On,
    Off,
    Gm,
    Global,
}

impl ChatBroadcast {
    /// `Enum.valueOf(ChatBroadcastType.class, text)`: `None` for a name Java would throw on.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ON" => Some(Self::On),
            "OFF" => Some(Self::Off),
            "GM" => Some(Self::Gm),
            "GLOBAL" => Some(Self::Global),
            _ => None,
        }
    }
}

/// The configuration values the world reads (Java `PlayerConfig`, `GeneralConfig`).
/// `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldSettings {
    /// `PlayerConfig.NEVIT_ENABLED` (`NevitEnabled`).
    pub nevit_enabled: bool,
    /// `PlayerConfig.STORE_UI_SETTINGS` (`StoreCharUiSettings`).
    pub store_ui_settings: bool,
    /// `MaximumSlotsForNoDwarf`, `MaximumSlotsForDwarf`, `MaximumSlotsForGMPlayer`.
    pub inventory_maximum_no_dwarf: i32,
    pub inventory_maximum_dwarf: i32,
    pub inventory_maximum_gm: i32,
    /// `MaximumSlotsForQuestItems`.
    pub inventory_maximum_quest_items: i32,
    /// `MaximumWarehouseSlotsForNoDwarf`, `...ForDwarf`, `...ForClan`.
    pub warehouse_slots_no_dwarf: i32,
    pub warehouse_slots_dwarf: i32,
    pub warehouse_slots_clan: i32,
    /// `MaxPvtStoreSellSlotsDwarf/Other`, `MaxPvtStoreBuySlotsDwarf/Other`.
    pub max_pvtstoresell_slots_dwarf: i32,
    pub max_pvtstoresell_slots_other: i32,
    pub max_pvtstorebuy_slots_dwarf: i32,
    pub max_pvtstorebuy_slots_other: i32,
    /// `DwarfRecipeLimit`, `CommonRecipeLimit`.
    pub dwarf_recipe_limit: i32,
    pub common_recipe_limit: i32,
    /// `GeneralConfig.MINIMUM_CHAT_LEVEL` (`MinimumChatLevel`).
    pub minimum_chat_level: i32,
    /// `GeneralConfig.DEFAULT_GLOBAL_CHAT` (`GlobalChat`).
    pub global_chat: ChatBroadcast,
    /// `GeneralConfig.DEFAULT_TRADE_CHAT` (`TradeChat`).
    pub trade_chat: ChatBroadcast,
    /// `GeneralConfig.GM_HERO_AURA` (`GMHeroAura`).
    pub gm_hero_aura: bool,
    /// `GeneralConfig.LOG_CHAT` (`LogChat`).
    pub log_chat: bool,
    /// `GeneralConfig.GRIDS_ALWAYS_ON` (`GridsAlwaysOn`): every region starts active.
    pub grids_always_on: bool,
    /// `GeneralConfig.GRID_NEIGHBOR_TURNON_TIME` (`GridNeighborTurnOnTime`), seconds.
    pub grid_neighbor_turnon_secs: i32,
    /// `GeneralConfig.GRID_NEIGHBOR_TURNOFF_TIME` (`GridNeighborTurnOffTime`), seconds.
    pub grid_neighbor_turnoff_secs: i32,
    /// `PlayerConfig.ENABLE_KEYBOARD_MOVEMENT` (`KeyboardMovement`).
    pub keyboard_movement: bool,
    /// `PlayerConfig.PLAYER_MOVEMENT_BLOCK_TIME` (`NpcTalkBlockingTime` seconds, kept in ms).
    pub player_movement_block_time_ms: i32,
    /// The values zone types read (`PeaceZoneMode`, `JailIsPvp`, ...).
    pub zones: crate::zones::ZoneSettings,
    /// The values `teleToLocation` and the restart point lookups read.
    pub teleport: crate::teleport::TeleportSettings,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            nevit_enabled: true,
            store_ui_settings: true,
            inventory_maximum_no_dwarf: 80,
            inventory_maximum_dwarf: 100,
            inventory_maximum_gm: 250,
            inventory_maximum_quest_items: 100,
            warehouse_slots_no_dwarf: 100,
            warehouse_slots_dwarf: 120,
            warehouse_slots_clan: 150,
            max_pvtstoresell_slots_dwarf: 4,
            max_pvtstoresell_slots_other: 3,
            max_pvtstorebuy_slots_dwarf: 5,
            max_pvtstorebuy_slots_other: 4,
            dwarf_recipe_limit: 50,
            common_recipe_limit: 50,
            minimum_chat_level: 20,
            grids_always_on: false,
            grid_neighbor_turnon_secs: 1,
            grid_neighbor_turnoff_secs: 90,
            keyboard_movement: true,
            player_movement_block_time_ms: 0,
            global_chat: ChatBroadcast::On,
            trade_chat: ChatBroadcast::On,
            gm_hero_aura: false,
            log_chat: false,
            zones: crate::zones::ZoneSettings::default(),
            teleport: crate::teleport::TeleportSettings::default(),
        }
    }
}
