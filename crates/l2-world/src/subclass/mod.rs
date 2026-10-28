//! Subclasses: the `character_subclasses` rows of a character, adding, replacing and switching a
//! class (`Player.addSubClass`, `modifySubClass`, `setActiveClass`) and the `Subclass` bypass of
//! the village masters.
// Java: entity/actor/Player.java (addSubClass, modifySubClass, setActiveClass, setClassTemplate,
//       isSubClassActive, getTotalSubClasses, getBaseClass, restoreSubClassData, storeCharSub,
//       storeCharBase's base exp/level/sp, canLogout, isAllowedToEnchantSkills)
// Java: entity/actor/holders/player/SubClassHolder.java
// Java: entity/actor/stat/PlayerStat.java (getBaseExp/getBaseLevel/getBaseSp, setExp/setLevel of
//       an active subclass)
// Java: entity/actor/instance/VillageMaster*.java (the `Subclass` bypass, `village_master.rs`)
//
// # Where the values live
// `PlayerData::level`/`exp`/`sp` (and `CreatureCore::level`) are always the values of the active
// class, like Java's `PlayerStat` getters, which read the active `SubClassHolder`. The holders sit
// in `PlayerProfile::subclasses` (they ride every snapshot, `storeCharSub` writes them): the holder
// of the active class index keeps its class id up to date, but its level, exp and sp are the ones
// of the last switch; [`stored_subclasses`] puts the live values in. While a subclass is active the
// base class values wait in `PlayerSubclasses::base` (`storeCharBase` writes them).
//
// # The switch and the database (D-15)
// Java's `setActiveClass` stores the character and then reads the skills, buffs, hennas,
// shortcuts and the dwarven recipe book of the new class index from the database. The world never
// waits for a read, so the switch has two halves (`switch.rs`): the first one stores and asks the
// persistence task for the rows of the new index (`PersistCommand::ClassIndexRequest`, queued
// after the store, so the rows include it); the answer (`WorldCommand::ClassIndexLoaded`) runs the
// rest. The class lock (Java `_subclassLock`, [`World::is_changing_class`]) stays on in between,
// so a logout, a restart, a skill enchant or a second switch waits like Java's would. The new
// class id and class index are set by the second half; until then a store writes the old class's
// rows, which is what the database already holds.
//
// # Left out, with the system that brings it
// - The class index of `character_quests`: the H5 table has no class index column (the quest
//   states belong to the character, whichever class is active), so there is nothing to keep.
// - `ON_PLAYER_PROFESSION_CHANGE` / `ON_PLAYER_PROFESSION_CANCEL` (script events, D-05).
// - `_autoUseSettings` (auto play is not ported).
// - The passive skills of items that are not worn (`Item.giveSkillsToOwner` of
//   `applyItemSkills` for `hasPassiveSkills` items in the bag) are not modelled anywhere.

pub mod categories;
mod script_api;
mod switch;
mod village_master;

use std::collections::BTreeMap;

use crate::effects::SavedEffect;
use crate::player::{PlayerData, ShortcutData, SkillData};
use crate::summon::servitor::SavedSummonEffect;

pub use village_master::{SubclassRules, VillageMasterKind};

// The class types the class change scripts of `l2-scripts` read (it reaches `l2-data` only
// through the world, MODULE_MAP).
pub use l2_data::stats::CategoryType;
pub use l2_data::{ClassId, Race};

/// The `Player.ini` values subclasses read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubclassSettings {
    /// `MaxSubclass` (Java `PlayerConfig.MAX_SUBCLASS`).
    pub max_subclass: i32,
    /// `BaseSubclassLevel`: the level a new subclass starts at and the lowest it goes.
    pub base_subclass_level: i32,
    /// `MaxSubclassLevel`.
    pub max_subclass_level: i32,
    /// `AltSubClassWithoutQuests`.
    pub alt_game_subclass_without_quests: bool,
    /// `AltSubclassEverywhere`.
    pub alt_game_subclass_everywhere: bool,
    /// `SubclassStoreSkillCooltime`: whether the store of a switch keeps the buffs.
    pub subclass_store_skill_cooltime: bool,
    /// `FeeDeleteSubClassSkills`: the adena Avant-Garde takes to cancel the certifications.
    pub fee_delete_subclass_skills: i32,
    /// `AltTransformationWithoutQuest` (Java `ALLOW_TRANSFORM_WITHOUT_QUEST`).
    pub allow_transform_without_quest: bool,
    /// `General.ini` `SkillCheckEnable`, `SkillCheckRemove`, `SkillCheckGM`: the certification
    /// skill check of the `SubClassSkills` validator at login.
    pub skill_check_enable: bool,
    pub skill_check_remove: bool,
    pub skill_check_gm: bool,
}

impl Default for SubclassSettings {
    fn default() -> Self {
        Self {
            max_subclass: 3,
            base_subclass_level: 40,
            max_subclass_level: 80,
            alt_game_subclass_without_quests: false,
            alt_game_subclass_everywhere: false,
            subclass_store_skill_cooltime: false,
            fee_delete_subclass_skills: 10_000_000,
            allow_transform_without_quest: false,
            skill_check_enable: false,
            skill_check_remove: false,
            skill_check_gm: true,
        }
    }
}

/// Java `SubClassHolder`: one `character_subclasses` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubClassHolder {
    /// `class_id`.
    pub class_id: i32,
    /// `exp`.
    pub exp: i64,
    /// `sp`.
    pub sp: i64,
    /// `level`.
    pub level: i32,
    /// `class_index`: 1 for the first subclass.
    pub class_index: i32,
}

/// Level, exp and sp of one class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassStats {
    pub level: i32,
    pub exp: i64,
    pub sp: i64,
}

/// Java `Player._subClasses` and the base class values while a subclass is active (see the module
/// notes). Part of the profile, so the snapshots carry it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerSubclasses {
    /// The holders by class index (Java's map of `Integer` keys iterates 1, 2, 3).
    pub subs: BTreeMap<i32, SubClassHolder>,
    /// The base class level, exp and sp while a subclass is active (`characters.level`, `exp`,
    /// `sp`); `None` while the base class is active (they are `PlayerData`'s then).
    pub base: Option<ClassStats>,
}

/// The `character_subclasses` rows `storeCharSub` writes: every holder, the active one with the
/// live level, exp and sp.
pub fn stored_subclasses(data: &PlayerData) -> Vec<SubClassHolder> {
    let active = data.profile.class_index;
    data.profile
        .subclasses
        .subs
        .values()
        .map(|sub| {
            if sub.class_index == active {
                SubClassHolder {
                    exp: data.exp,
                    sp: data.sp,
                    level: data.level,
                    ..*sub
                }
            } else {
                *sub
            }
        })
        .collect()
}

/// Java `getStat().getBaseLevel()`, `getBaseExp()`, `getBaseSp()`: what `storeCharBase` writes to
/// `characters`. `None` for a snapshot with an active subclass that does not carry the base
/// values (the store keeps the row's).
pub fn base_class_stats(data: &PlayerData) -> Option<ClassStats> {
    if data.profile.class_index == 0 {
        return Some(ClassStats {
            level: data.level,
            exp: data.exp,
            sp: data.sp,
        });
    }
    data.profile.subclasses.base
}

/// A write of the subclass tables with Java's statements (`PersistCommand::Subclass`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubclassWrite {
    /// `Player.addSubClass`: `INSERT INTO character_subclasses (charId,class_id,exp,sp,level,
    /// class_index) VALUES (?,?,?,?,?,?)`, then `storeSkill` (`REPLACE INTO character_skills
    /// (charId,skill_id,skill_level,class_index) VALUES (?,?,?,?)`) for each skill of the class
    /// tree up to level 40, under the new class index. Java stores no skill when the insert fails.
    Add {
        char_id: i32,
        holder: SubClassHolder,
        skills: Vec<SkillData>,
    },
    /// `Player.modifySubClass`, the database part, in Java's order: `DELETE FROM character_hennas
    /// WHERE charId=? AND slot=? AND class_index=?` for each slot in `henna_slots`, then the
    /// class index's `character_shortcuts`, `character_skills_save`, `character_skills` and its
    /// `character_subclasses` row.
    Delete {
        char_id: i32,
        class_index: i32,
        /// NOTE(java-quirk): the slots where the *active* class wears a dye (`getHenna(slot)`),
        /// whatever the class index being deleted wears.
        henna_slots: Vec<i32>,
    },
}

/// What `setActiveClass` reads for the new class index (the answer to
/// `PersistCommand::ClassIndexRequest`). A read that fails is logged by the persistence task and
/// comes back empty, like Java's catch blocks leave the part empty.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassIndexLoad {
    /// `restoreSkills`: `character_skills` of the index.
    pub skills: Vec<SkillData>,
    /// `restoreEffects`: `character_skills_save` of the index, in buff order (the persistence task
    /// deletes the rows after reading them, like Java).
    pub effects: Vec<SavedEffect>,
    /// `restoreHenna`: the dye id of slots 1 to 3, 0 for an empty slot.
    pub hennas: [i32; 3],
    /// `Shortcuts.restoreMe`: `character_shortcuts` of the index.
    pub shortcuts: Vec<ShortcutData>,
    /// `restoreRecipeBook(false)`: the dwarven recipes of the index (`type = 1`), in row order.
    pub dwarven_recipes: Vec<i32>,
    /// The `character_summon_skills_save` rows of the index (Java reads them when a servitor
    /// spawns; here with the rest of the index, like at the login).
    pub summon_effects: Vec<SavedSummonEffect>,
}

/// The world-only class change state of a player (Java `_subclassLock`), see `switch.rs`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubclassState {
    /// Java `_subclassLock`.
    pub(crate) lock: bool,
    /// The second half of a switch that waits for the rows of the new class index.
    pub(crate) pending: Option<switch::PendingSwitch>,
}

/// What a caller of `setActiveClass` sends after it (the switch may finish one database read
/// later, so these wait for it).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AfterSwitch {
    /// A system message.
    Message(l2_protocol_game::SystemMessageId),
    /// `player.sendMessage(text)`.
    Text(String),
    /// A village master page (through `send_html`, for the bypass cache).
    Html(l2_protocol_game::server::NpcHtmlMessage),
}
