//! The immutable datapack: tables loaded once from the Java server's XML files.
//!
//! [`Datapack::load`] is the single entry point; every loader module adds one field to it.
//! How a loader is written is described in [`loader`]; [`player_template`] is the reference.
//!
//! Tables are plain data behind `Arc`, so the world loop and the network tasks can share them
//! without locks (D-10: nothing here is mutable game state).

pub mod class_list;
pub mod experience;
pub mod initial_equipment;
pub mod initial_shortcuts;
pub mod item;
pub mod loader;
pub mod player_template;
pub mod skill_tree;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use class_list::{ClassId, ClassInfo, ClassList, Race};
pub use experience::ExperienceTable;
pub use initial_equipment::{InitialEquipment, InitialEquipments};
pub use initial_shortcuts::{
    InitialShortcut, InitialShortcuts, MacroCommand, MacroPreset, MacroType, ShortcutType,
};
pub use item::{ItemKind, ItemTemplate, ItemTemplates};
pub use loader::DataError;
pub use player_template::{Collision, DEFAULT_MAX_PLAYER_LEVEL, Location, PlayerTemplate, PlayerTemplates};
pub use skill_tree::{SkillLearn, SkillTrees};

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
}

impl Datapack {
    /// Loads all tables below `root` (the folder that contains `data/`), logging Java's count
    /// line for each. Like Java, a broken or missing file is logged and skipped, not fatal; the
    /// only error is a `root` that has no `data` folder at all.
    pub fn load(root: &Path) -> Result<Self, DataError> {
        if !root.join("data").is_dir() {
            return Err(DataError::MissingRoot(root.to_path_buf()));
        }
        // The highest player level comes from experience.xml (Java reads it there too). The
        // constant is only the fallback for a missing or broken experience file.
        let experience = ExperienceTable::load(root);
        let max_level = experience.max_level().unwrap_or(DEFAULT_MAX_PLAYER_LEVEL);
        Ok(Self {
            root: root.to_path_buf(),
            class_list: Arc::new(ClassList::load(root)),
            experience: Arc::new(experience),
            initial_equipment: Arc::new(InitialEquipments::load(root)),
            initial_shortcuts: Arc::new(InitialShortcuts::load(root)),
            player_templates: Arc::new(PlayerTemplates::load(root, max_level)),
            skill_trees: Arc::new(SkillTrees::load(root)),
            // Java's code default for `CustomItemsLoad` is false (the shipped General.ini sets True);
            // there is no config in this crate yet.
            items: Arc::new(ItemTemplates::load(root, false)),
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
    fn empty_data_folder_loads_empty_tables() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("data")).unwrap();
        let pack = Datapack::load(dir.path()).unwrap();
        assert!(pack.class_list.is_empty());
        assert!(pack.experience.is_empty());
        assert!(pack.initial_equipment.is_empty());
        assert!(pack.player_templates.is_empty());
        assert!(pack.items.is_empty());
    }
}
