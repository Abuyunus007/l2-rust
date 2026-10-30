//! `data/scripts/ai`: the AI scripts (`areas`, `bosses`, `others`).

pub mod areas;
pub mod bosses;
pub mod others;

use l2_world::ScriptRegistry;
use l2_world::subclass::categories::ClassMasterConfig;

/// Registers every AI script, folder by folder in Java's order, with the class master built from
/// the loaded `config/ClassMaster.xml`.
pub(crate) fn register_with(r: &mut ScriptRegistry, class_master: ClassMasterConfig) {
    areas::register(r);
    bosses::register(r);
    others::register_with(r, class_master);
}
