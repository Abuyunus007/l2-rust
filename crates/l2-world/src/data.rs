//! What the world needs to know about the datapack and the configuration, without owning
//! either: [`StaticData`] (item templates, skills, the stat engine's tables, access levels) and
//! [`WorldSettings`] (the `.ini` values the enter-world and chat code reads).
//!
//! `l2-world` has no XML or config loaders (MODULE_MAP): the game server implements
//! [`StaticData`] over the loaded `Datapack` and fills [`WorldSettings`] from `GameConfig`.
//! Tests plug in small fakes. [`NoData`] and `WorldSettings::default()` are the "nothing loaded"
//! and "Java defaults" versions, so a bare `World::new` still works.

use crate::admin::AdminRights;
use crate::player::PlayerData;
use crate::stats::{StatData, StatSettings};

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

/// Everything static the world reads. Implemented over the datapack by the game server.
pub trait StaticData: Send + Sync {
    /// The template of an item, `None` if the item id is unknown.
    fn item(&self, item_id: i32) -> Option<ItemStatic>;

    /// Java `Skill.isPassive()` (operate type `P`). `false` for a skill that is not known.
    fn skill_is_passive(&self, skill_id: i32) -> bool;

    /// `(exp - expForLevel(level)) / (expForLevel(level + 1) - expForLevel(level))`, computed the
    /// way `UserInfo` does it (see `exp_percent` in `l2-protocol-game`).
    fn exp_percent(&self, exp: i64, level: i32) -> f64;

    /// The access level with this id. An id the file does not list gives Java's default
    /// `AccessLevel` ([`AccessLevelStatic::UNREGISTERED`]).
    fn access_level(&self, level: i32) -> AccessLevelStatic;

    /// `AdminData.getMasterAccessLevel()`: the highest level the file lists (0 without one). A new
    /// character gets it when `EverybodyHasAdminRights` is on (`Player.getAccessLevel`).
    fn master_access_level(&self) -> i32 {
        0
    }

    /// `AccessLevel.canTakeAggro()` (`takeAggro` of `AccessLevels.xml`, default true): whether
    /// monsters aggress a player of this level. The default (no access file) takes aggro; the
    /// game server reads `takeAggro` over the datapack.
    fn can_take_aggro(&self, _level: i32) -> bool {
        true
    }

    /// `PlayerTemplate.getSafeFallHeight()` (`baseSafeFall`) of the player's class; Java's default
    /// when the template does not set it is 333.
    fn safe_fall_height(&self, _player: &PlayerData) -> i32 {
        333
    }

    /// The datapack tables of the stat engine (item functions, class templates, armor sets,
    /// `statBonus.xml`, ...). The default is "nothing loaded": every template value is Java's
    /// `CreatureTemplate` default and every base stat bonus is 1.
    fn stat_data(&self) -> Option<&StatData> {
        None
    }

    /// The transformations of the datapack (`data/stats/transformations`, M6-TRANS-01); none
    /// loaded by default.
    fn transforms(&self) -> Option<&std::sync::Arc<l2_data::Transforms>> {
        None
    }

    /// Java `ItemTemplate.getName()`, `None` for an unknown item.
    fn item_name(&self, _item_id: i32) -> Option<String> {
        None
    }

    /// Java `ItemTemplate.isStackable()`.
    fn item_stackable(&self, _item_id: i32) -> bool {
        false
    }

    /// The admin command rights (`config/AccessLevels.xml` + `config/AdminCommands.xml`). The
    /// default is "nothing loaded": no GM may run any command.
    fn admin_rights(&self) -> Option<&AdminRights> {
        None
    }

    /// Java `ItemTemplate.isAllowSelfResurrection()` (`allow_self_resurrection`): the item is a
    /// self-resurrection scroll or feather.
    fn item_allows_self_resurrection(&self, _item_id: i32) -> bool {
        false
    }

    /// Java `AccessLevel.allowFixedRes()` (`allowFixedRes` of `AccessLevels.xml`).
    fn access_level_allows_fixed_res(&self, _level: i32) -> bool {
        false
    }

    /// Java `AccessLevel.allowAltG()` (`allowAltg` of `AccessLevels.xml`, default false): whether a
    /// GM of this level may look at a player with Alt+G.
    fn access_level_allows_alt_g(&self, _level: i32) -> bool {
        false
    }

    /// Java `AccessLevel.canGainExp()` (`gainExp` of `AccessLevels.xml`, default true).
    fn access_level_can_gain_exp(&self, _level: i32) -> bool {
        true
    }

    /// Java `AccessLevel.allowTransaction()` (`allowTransaction` of `AccessLevels.xml`, default
    /// true): whether a player of this level may trade.
    fn access_level_allows_transaction(&self, _level: i32) -> bool {
        true
    }

    /// Java `OptionData.getOptions(id)`: the augmentation option with this id. The default (no
    /// datapack) knows none, so an augmented item gives no bonus.
    fn augmentation_option(&self, _id: i32) -> Option<&l2_data::AugmentationOption> {
        None
    }

    /// Java `EnchantSkillGroupsData`: the enchant groups and the enchant routes of the skills.
    /// The default (no datapack) has none, so no skill is enchantable.
    fn enchant_skill_groups(&self) -> Option<std::sync::Arc<l2_data::EnchantSkillGroupsData>> {
        None
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

    fn access_level(&self, level: i32) -> AccessLevelStatic {
        (**self).access_level(level)
    }

    fn master_access_level(&self) -> i32 {
        (**self).master_access_level()
    }

    fn can_take_aggro(&self, level: i32) -> bool {
        (**self).can_take_aggro(level)
    }

    fn safe_fall_height(&self, player: &PlayerData) -> i32 {
        (**self).safe_fall_height(player)
    }

    fn stat_data(&self) -> Option<&StatData> {
        (**self).stat_data()
    }

    fn transforms(&self) -> Option<&std::sync::Arc<l2_data::Transforms>> {
        (**self).transforms()
    }

    fn item_name(&self, item_id: i32) -> Option<String> {
        (**self).item_name(item_id)
    }

    fn item_stackable(&self, item_id: i32) -> bool {
        (**self).item_stackable(item_id)
    }

    fn admin_rights(&self) -> Option<&AdminRights> {
        (**self).admin_rights()
    }

    fn item_allows_self_resurrection(&self, item_id: i32) -> bool {
        (**self).item_allows_self_resurrection(item_id)
    }

    fn access_level_allows_fixed_res(&self, level: i32) -> bool {
        (**self).access_level_allows_fixed_res(level)
    }

    fn access_level_can_gain_exp(&self, level: i32) -> bool {
        (**self).access_level_can_gain_exp(level)
    }

    fn access_level_allows_transaction(&self, level: i32) -> bool {
        (**self).access_level_allows_transaction(level)
    }

    fn augmentation_option(&self, id: i32) -> Option<&l2_data::AugmentationOption> {
        (**self).augmentation_option(id)
    }

    fn enchant_skill_groups(&self) -> Option<std::sync::Arc<l2_data::EnchantSkillGroupsData>> {
        (**self).enchant_skill_groups()
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
    /// `GeneralConfig.GMAUDIT` (`GMAudit`): one `gmaudit` log line per admin command.
    pub gm_audit: bool,
    /// `GeneralConfig.GM_STARTUP_BUILDER_HIDE` (`GMStartupBuilderHide`): `sendSysMessage` goes to
    /// the chat window as a "SYS" line instead of a system message.
    pub gm_startup_builder_hide: bool,
    /// `GeneralConfig.GM_STARTUP_INVULNERABLE` (`GMStartupInvulnerable`): a GM with `admin_invul`
    /// access is invulnerable from the moment they enter the world.
    pub gm_startup_invulnerable: bool,
    /// `GeneralConfig.GM_STARTUP_SILENCE` (`GMStartupSilence`): a GM with `admin_silence` access
    /// enters in silence mode.
    pub gm_startup_silence: bool,
    /// `GeneralConfig.GM_STARTUP_DIET_MODE` (`GMStartupDietMode`): a GM with `admin_diet` access
    /// enters in diet mode.
    pub gm_startup_diet_mode: bool,
    /// `PlayerConfig.SILENCE_MODE_EXCLUDE` (`SilenceModeExclude`): a player in silence mode can be
    /// answered by the players he whispered to.
    pub silence_mode_exclude: bool,
    /// `GeneralConfig.USE_SUPER_HASTE_AS_GM_SPEED` (`UseSuperHasteAsGMSpeed`): `//gmspeed` gives the
    /// super haste instead of the speed multiplier.
    pub use_super_haste_as_gm_speed: bool,
    /// `GeneralConfig.GM_GIVE_SPECIAL_SKILLS` (`GMGiveSpecialSkills`): the GM skills of a GM login.
    pub gm_give_special_skills: bool,
    /// `GeneralConfig.GM_GIVE_SPECIAL_AURA_SKILLS` (`GMGiveSpecialAuraSkills`): the GM aura skills.
    pub gm_give_special_aura_skills: bool,
    /// `GeneralConfig.GRIDS_ALWAYS_ON` (`GridsAlwaysOn`): every region starts active.
    pub grids_always_on: bool,
    /// `GeneralConfig.GRID_NEIGHBOR_TURNON_TIME` (`GridNeighborTurnOnTime`), seconds.
    pub grid_neighbor_turnon_secs: i32,
    /// `GeneralConfig.BOOKMARK_CONSUME_ITEM_ID` (`BookmarkConsumeItemId`): the item a saved
    /// teleport bookmark uses up (0 or less: none).
    pub bookmark_consume_item_id: i32,
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
    /// The values NPC dialogs and gatekeepers read.
    pub talk: crate::dialog::TalkSettings,
    /// The caps and multipliers the stat getters read.
    pub stats: StatSettings,
    /// The values the attack code reads (M4-COMBAT-01).
    pub combat: crate::combat::CombatSettings,
    /// The values equipping, destroying and dropping items read.
    pub inventory: crate::inventory::InventorySettings,
    /// The values dying and coming back read (M4-DEATH-01).
    pub death: crate::death::DeathSettings,
    /// The values ground items and their pickup read (M4-DROP-01).
    pub ground: crate::ground_item::GroundItemSettings,
    /// The drop rates and level gap rules of `Rates.ini` (M4-DROP-01).
    pub drops: crate::rewards::drop::DropSettings,
    /// The values exp, SP and levels read (M4-EXP-01).
    pub exp: crate::rewards::exp::ExpSettings,
    /// The values vitality reads (M4-VIT-01).
    pub vitality: crate::rewards::vitality::VitalitySettings,
    /// The values Nevit's blessing reads besides `nevit_enabled` (M4-VIT-01).
    pub nevit: crate::rewards::nevit::NevitSettings,
    /// The buff slot limits and the effect tick length (M5-EFFECT-01).
    pub effects: crate::effects::EffectSettings,
    /// The values learning skills reads (M5-LEARN-01).
    pub learn: crate::skills::learn::LearnSettings,
    /// The values casting reads (M5-CAST-01).
    pub cast: crate::skills::cast::CastSettings,
    /// The values parties read (M6-PARTY-01).
    pub party: crate::party::PartySettings,
    /// The PvP flag times, the anti feed rules and the PvP title colours (M6-PVP-01).
    pub pvp: crate::pvp::PvpSettings,
    /// The values the NPC shops read (M6-SHOP-01).
    pub shop: crate::shop::ShopSettings,
    /// The freight price and slots, the clan warehouse rule, the sorted lists (M6-WH-01).
    pub warehouse: crate::warehouse::WarehouseSettings,
    /// The crafting switches and rates (M6-CRAFT-01).
    pub craft: crate::craft::CraftSettings,
    /// The private store ranges and packet limit (M6-STORE-01).
    pub store: crate::trade::StoreSettings,
    /// The enchant and over-enchant settings (M6-ENCHANT-01).
    pub enchant: crate::enchant::EnchantSettings,
    /// The augmentation chances, blacklist and PvP rule (M6-AUGMENT-01).
    pub augment: crate::augment::AugmentSettings,
    /// The fishing switches and the championship prizes (M6-FU-08).
    pub fishing: crate::fishing::FishingSettings,
    /// Rust-only (`SideBySide.ini`, `DailyResetDatabaseWrites`, D-36; M5-FU-03): whether the daily
    /// reset runs the global `character_reco_bonus` `UPDATE`s. False while the Java game server
    /// shares the database and runs them itself (D-39).
    pub daily_reset_database_writes: bool,
    /// The values of the L-02 packets (the wear shop, the dispel of dances), see `leftover.rs`.
    pub leftover: crate::leftover::LeftoverSettings,
    /// The subclass values of `Player.ini` (S-SUB-01).
    pub subclass: crate::subclass::SubclassSettings,
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
            bookmark_consume_item_id: -1,
            grid_neighbor_turnoff_secs: 90,
            keyboard_movement: true,
            player_movement_block_time_ms: 0,
            global_chat: ChatBroadcast::On,
            trade_chat: ChatBroadcast::On,
            gm_hero_aura: false,
            log_chat: false,
            gm_audit: false,
            gm_startup_builder_hide: false,
            gm_startup_invulnerable: false,
            gm_startup_silence: false,
            gm_startup_diet_mode: false,
            silence_mode_exclude: false,
            use_super_haste_as_gm_speed: false,
            gm_give_special_skills: false,
            gm_give_special_aura_skills: false,
            zones: crate::zones::ZoneSettings::default(),
            teleport: crate::teleport::TeleportSettings::default(),
            talk: crate::dialog::TalkSettings::default(),
            stats: StatSettings::default(),
            combat: crate::combat::CombatSettings::default(),
            inventory: crate::inventory::InventorySettings::default(),
            death: crate::death::DeathSettings::default(),
            ground: crate::ground_item::GroundItemSettings::default(),
            drops: crate::rewards::drop::DropSettings::default(),
            exp: crate::rewards::exp::ExpSettings::default(),
            vitality: crate::rewards::vitality::VitalitySettings::default(),
            nevit: crate::rewards::nevit::NevitSettings::default(),
            effects: crate::effects::EffectSettings::default(),
            learn: crate::skills::learn::LearnSettings::default(),
            cast: crate::skills::cast::CastSettings::default(),
            party: crate::party::PartySettings::default(),
            pvp: crate::pvp::PvpSettings::default(),
            shop: crate::shop::ShopSettings::default(),
            warehouse: crate::warehouse::WarehouseSettings::default(),
            craft: crate::craft::CraftSettings::default(),
            store: crate::trade::StoreSettings::default(),
            enchant: crate::enchant::EnchantSettings::default(),
            augment: crate::augment::AugmentSettings::default(),
            fishing: crate::fishing::FishingSettings::default(),
            daily_reset_database_writes: false,
            leftover: crate::leftover::LeftoverSettings::default(),
            subclass: crate::subclass::SubclassSettings::default(),
        }
    }
}
