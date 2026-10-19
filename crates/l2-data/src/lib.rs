//! The immutable datapack: tables loaded once from the Java server's XML files.
//!
//! [`Datapack::load`] is the single entry point; every loader module adds one field to it.
//! How a loader is written is described in [`loader`]; [`player_template`] is the reference.
//!
//! Tables are plain data behind `Arc`, so the world loop and the network tasks can share them
//! without locks (D-10: nothing here is mutable game state).

pub mod armor_set;
pub mod class_list;
pub mod door;
pub mod enchant_hp_bonus;
pub mod experience;
pub mod fence;
pub mod henna;
pub mod hit_condition_bonus;
pub mod html;
pub mod initial_equipment;
pub mod initial_shortcuts;
pub mod item;
pub mod loader;
pub mod map_region;
pub mod npc;
pub mod player_template;
pub mod skill_tree;
pub mod spawn;
pub mod static_object;
pub mod stats;
pub mod teleporter;
pub mod zone;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use armor_set::{ArmorSet, ArmorSets};
pub use class_list::{ClassId, ClassInfo, ClassList, Race};
pub use door::{DoorTemplate, DoorTemplates};
pub use enchant_hp_bonus::EnchantHpBonus;
pub use experience::ExperienceTable;
pub use fence::{FenceState, FenceTemplate, FenceTemplates};
pub use henna::{Henna, Hennas};
pub use hit_condition_bonus::HitConditionBonus;
pub use html::HtmCache;
pub use initial_equipment::{InitialEquipment, InitialEquipments};
pub use initial_shortcuts::{
    InitialShortcut, InitialShortcuts, MacroCommand, MacroPreset, MacroType, ShortcutType,
};
pub use item::{ItemKind, ItemTemplate, ItemTemplates};
pub use loader::DataError;
pub use map_region::{MapRegion, MapRegions};
pub use npc::{NpcLoadOptions, NpcTemplate, NpcTemplates};
pub use player_template::{
    Collision, DEFAULT_MAX_PLAYER_LEVEL, Location, PAPERDOLL_TOTAL_SLOTS, PlayerTemplate, PlayerTemplates,
};
pub use skill_tree::{SkillLearn, SkillTrees};
pub use spawn::{SpawnData, SpawnLoadOptions};
pub use static_object::{StaticObjectTemplate, StaticObjectTemplates};
pub use stats::{Condition, FuncKind, FuncTemplate, Stat};
pub use teleporter::{TeleportHolder, TeleportLocation, TeleportType, TeleporterData};
pub use zone::{SpawnTerritory, Zone, ZoneForm, Zones};

/// Everything loaded from a datapack folder.
#[derive(Debug, Clone)]
pub struct Datapack {
    /// The folder that contains `data/` (the Java server's working directory, `dist/game`).
    pub root: PathBuf,
    /// `data/stats/players/classList.xml`.
    pub class_list: Arc<ClassList>,
    /// `data/stats/players/experience.xml`.
    pub experience: Arc<ExperienceTable>,
    /// `data/stats/players/initialEquipment.xml`.
    pub initial_equipment: Arc<InitialEquipments>,
    /// `data/stats/players/initialShortcuts.xml`.
    pub initial_shortcuts: Arc<InitialShortcuts>,
    /// `data/stats/players/templates/**`.
    pub player_templates: Arc<PlayerTemplates>,
    /// `data/stats/players/skillTrees/**`.
    pub skill_trees: Arc<SkillTrees>,
    /// `data/stats/items/*.xml`.
    pub items: Arc<ItemTemplates>,
    /// `data/stats/npcs/*.xml`.
    pub npcs: Arc<NpcTemplates>,
    /// `data/zones/*.xml`.
    pub zones: Arc<Zones>,
    /// `data/mapregion/*.xml`.
    pub map_regions: Arc<MapRegions>,
    /// Every `.htm` / `.html` file below the root (Java `HtmCache`, always preloaded).
    pub html: Arc<HtmCache>,
    /// `data/teleporters/**`: gatekeeper lists by NPC id.
    pub teleporters: Arc<TeleporterData>,
    /// `data/spawns/**/*.xml`.
    pub spawns: Arc<SpawnData>,
    /// `data/Doors.xml`.
    pub doors: Arc<DoorTemplates>,
    /// `data/FenceData.xml`.
    pub fences: Arc<FenceTemplates>,
    /// `data/StaticObjects.xml`.
    pub static_objects: Arc<StaticObjectTemplates>,
    /// `data/stats/armorsets/*.xml`: armor set pieces, set skills and stat bonuses.
    pub armor_sets: Arc<ArmorSets>,
    /// `data/stats/enchantHPBonus.xml`: HP bonus per enchant level and crystal grade.
    pub enchant_hp_bonus: Arc<EnchantHpBonus>,
    /// `data/stats/hitConditionBonus.xml`: the hit percentages for height, side, night and rain.
    pub hit_condition_bonus: Arc<HitConditionBonus>,
    /// `data/stats/hennaList.xml`: hennas (dyes) by dye id.
    pub hennas: Arc<Hennas>,
}

/// The config values that change what the loaders read. The game server builds it from
/// `GameConfig` (`options_from_config` in `game-server`).
///
/// [`DatapackOptions::default`] holds Java's *code* defaults, not the values of the shipped
/// `.ini` files, so [`Datapack::load`] behaves as it always did. Each field names its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatapackOptions {
    /// `General.ini` `CustomItemsLoad` (default false): also load `data/stats/items/custom`.
    pub custom_items_load: bool,
    /// `General.ini` `CustomNpcData` (default false): also load `data/stats/npcs/custom`.
    pub custom_npc_data: bool,
    /// `Custom/FakePlayers.ini` `EnableFakePlayers` (default false): fake player NPCs are
    /// loaded and may spawn. Used by both the NPC and the spawn loader.
    pub fake_players_enabled: bool,
    /// `Development.ini` `NoSpawns` (default false): spawns are not loaded at all.
    pub no_spawns: bool,
    /// `NPC.ini` `MaxAggroRange` (default 450).
    pub max_aggro_range: i32,
    /// `NPC.ini` `DefaultCorpseTime` (default 7).
    pub default_corpse_time: i32,
    /// `General.ini` `HtmCache` (default true). False is Java's lazy mode. The Rust HTML cache
    /// preloads in both cases (see `HtmCache::load`), so this only changes the log note.
    pub htm_cache: bool,
    /// `General.ini` `CheckHtmlEncoding` (default true): log html files with non ASCII text
    /// outside `data/lang`.
    pub check_html_encoding: bool,
}

impl Default for DatapackOptions {
    /// Java's code defaults, taken from the loaders' own option structs.
    fn default() -> Self {
        let npc = NpcLoadOptions::default();
        Self {
            custom_items_load: false,
            custom_npc_data: npc.custom_npc_data,
            fake_players_enabled: npc.fake_players_enabled,
            no_spawns: false,
            max_aggro_range: npc.max_aggro_range,
            default_corpse_time: npc.default_corpse_time,
            htm_cache: true,
            check_html_encoding: true,
        }
    }
}

impl Datapack {
    /// Loads all tables below `root` (the folder that contains `data/`), with Java's code
    /// defaults. See [`Datapack::load_with`].
    pub fn load(root: &Path) -> Result<Self, DataError> {
        Self::load_with(root, &DatapackOptions::default())
    }

    /// Loads all tables below `root` (the folder that contains `data/`), logging Java's count
    /// line for each. Like Java, a broken or missing file is logged and skipped, not fatal; the
    /// only error is a `root` that has no `data` folder at all.
    pub fn load_with(root: &Path, options: &DatapackOptions) -> Result<Self, DataError> {
        if !root.join("data").is_dir() {
            return Err(DataError::MissingRoot(root.to_path_buf()));
        }
        // The highest player level comes from experience.xml (Java reads it there too). The
        // constant is only the fallback for a missing or broken experience file.
        let experience = ExperienceTable::load(root);
        let max_level = experience.max_level().unwrap_or(DEFAULT_MAX_PLAYER_LEVEL);
        // Java loads NpcData before SpawnData (SpawnData checks every spawn against it).
        let npc_options = NpcLoadOptions {
            custom_npc_data: options.custom_npc_data,
            fake_players_enabled: options.fake_players_enabled,
            max_aggro_range: options.max_aggro_range,
            default_corpse_time: options.default_corpse_time,
        };
        let npcs = NpcTemplates::load(root, &npc_options);
        let spawn_options = SpawnLoadOptions {
            fake_players_enabled: options.fake_players_enabled,
            no_spawns: options.no_spawns,
        };
        let spawns = SpawnData::load(root, &npcs, &spawn_options);
        Ok(Self {
            root: root.to_path_buf(),
            class_list: Arc::new(ClassList::load(root)),
            experience: Arc::new(experience),
            initial_equipment: Arc::new(InitialEquipments::load(root)),
            initial_shortcuts: Arc::new(InitialShortcuts::load(root)),
            player_templates: Arc::new(PlayerTemplates::load(root, max_level)),
            skill_trees: Arc::new(SkillTrees::load(root)),
            items: Arc::new(ItemTemplates::load(root, options.custom_items_load)),
            zones: Arc::new(Zones::load(root)),
            map_regions: Arc::new(MapRegions::load(root)),
            // Java `HtmCache = False` is the lazy mode, so the lazy flag is the inverse.
            html: Arc::new(HtmCache::load_with(
                root,
                !options.htm_cache,
                options.check_html_encoding,
            )),
            teleporters: Arc::new(TeleporterData::load(root)),
            npcs: Arc::new(npcs),
            spawns: Arc::new(spawns),
            doors: Arc::new(DoorTemplates::load(root)),
            fences: Arc::new(FenceTemplates::load(root)),
            static_objects: Arc::new(StaticObjectTemplates::load(root)),
            armor_sets: Arc::new(ArmorSets::load(root)),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::load(root)),
            hit_condition_bonus: Arc::new(HitConditionBonus::load(root)),
            hennas: Arc::new(Hennas::load(root)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_without_data_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Datapack::load(dir.path()),
            Err(DataError::MissingRoot(_))
        ));
    }

    #[test]
    fn default_options_are_java_code_defaults() {
        let options = DatapackOptions::default();
        assert!(!options.custom_items_load);
        assert!(!options.custom_npc_data);
        assert!(!options.fake_players_enabled);
        assert!(!options.no_spawns);
        assert_eq!((options.max_aggro_range, options.default_corpse_time), (450, 7));
        assert!(options.htm_cache);
        assert!(options.check_html_encoding);
    }

    #[test]
    fn custom_folders_load_only_when_the_options_say_so() {
        let dir = tempfile::tempdir().unwrap();
        let items = dir.path().join("data/stats/items/custom");
        let npcs = dir.path().join("data/stats/npcs/custom");
        std::fs::create_dir_all(&items).unwrap();
        std::fs::create_dir_all(&npcs).unwrap();
        std::fs::write(
            items.join("custom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?><list><item id="900001" type="EtcItem" name="Custom Item"/></list>"#,
        )
        .unwrap();
        std::fs::write(
            npcs.join("custom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?><list><npc id="900001" name="Custom Npc"/></list>"#,
        )
        .unwrap();

        let defaults = Datapack::load(dir.path()).unwrap();
        assert!(defaults.items.get(900_001).is_none());
        assert!(defaults.npcs.get(900_001).is_none());

        let custom = DatapackOptions {
            custom_items_load: true,
            custom_npc_data: true,
            ..DatapackOptions::default()
        };
        let pack = Datapack::load_with(dir.path(), &custom).unwrap();
        assert_eq!(
            pack.items.get(900_001).map(|item| item.name.as_str()),
            Some("Custom Item")
        );
        assert_eq!(
            pack.npcs.get(900_001).map(|npc| npc.name.as_str()),
            Some("Custom Npc")
        );
    }

    #[test]
    fn no_spawns_skips_the_spawn_files() {
        let dir = tempfile::tempdir().unwrap();
        let spawns = dir.path().join("data/spawns");
        std::fs::create_dir_all(&spawns).unwrap();
        std::fs::write(
            spawns.join("spawn.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?><list enabled="true"></list>"#,
        )
        .unwrap();

        let loaded = Datapack::load(dir.path()).unwrap();
        assert_eq!(loaded.spawns.parsed_files(), 1);
        let options = DatapackOptions {
            no_spawns: true,
            ..DatapackOptions::default()
        };
        let skipped = Datapack::load_with(dir.path(), &options).unwrap();
        assert_eq!(skipped.spawns.parsed_files(), 0);
    }

    #[test]
    fn empty_data_folder_loads_empty_tables() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("data")).unwrap();
        let pack = Datapack::load(dir.path()).unwrap();
        assert!(pack.class_list.is_empty());
        assert!(pack.experience.is_empty());
        assert!(pack.initial_equipment.is_empty());
        assert!(pack.player_templates.is_empty());
        assert!(pack.items.is_empty());
        assert!(pack.npcs.is_empty());
        assert!(pack.armor_sets.is_empty());
        assert!(pack.enchant_hp_bonus.is_empty());
        assert!(pack.hennas.is_empty());
    }
}
